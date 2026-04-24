import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import type { AgentAvailability, AgentConfig, ChainDefinition, ChainStep, ManifestBundle } from "./shared.ts";

const manifestCache = new Map<string, { key: string; bundle: ManifestBundle }>();

function parseAgentFile(contents: string, sourcePath: string): AgentConfig {
	const match = contents.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
	if (!match) throw new Error(`agent frontmatter missing in ${sourcePath}`);

	const [, frontmatter, body] = match;
	const fields = new Map<string, string>();
	for (const rawLine of frontmatter.split(/\r?\n/)) {
		const line = rawLine.trim();
		if (!line || line.startsWith("#")) continue;
		const idx = line.indexOf(":");
		if (idx < 0) continue;
		const key = line.slice(0, idx).trim();
		const value = line.slice(idx + 1).trim();
		fields.set(key, value);
	}

	const name = fields.get("name")?.trim();
	if (!name) throw new Error(`agent name missing in ${sourcePath}`);

	const tools = (fields.get("tools") ?? "")
		.split(",")
		.map((value) => value.trim())
		.filter(Boolean);

	return {
		name,
		description: fields.get("description")?.trim() || undefined,
		tools,
		model: fields.get("model")?.trim() || undefined,
		systemPrompt: body.trim(),
		sourcePath,
	};
}

function parseTeamsYaml(contents: string): Record<string, string[]> {
	const teams: Record<string, string[]> = {};
	let current: string | undefined;
	for (const rawLine of contents.split(/\r?\n/)) {
		const line = rawLine.replace(/\t/g, "    ");
		if (!line.trim() || line.trimStart().startsWith("#")) continue;
		if (/^\S[^:]*:\s*$/.test(line)) {
			current = line.slice(0, line.indexOf(":")).trim();
			teams[current] = [];
			continue;
		}
		if (current && /^\s+-\s+/.test(line)) {
			teams[current].push(line.replace(/^\s+-\s+/, "").trim());
		}
	}
	for (const [name, members] of Object.entries(teams)) {
		teams[name] = members.filter(Boolean);
	}
	return teams;
}

function parseChainsYaml(contents: string): Record<string, ChainDefinition> {
	const chains: Record<string, ChainDefinition> = {};
	let currentChain: string | undefined;
	let currentStep: ChainStep | undefined;
	for (const rawLine of contents.split(/\r?\n/)) {
		const line = rawLine.replace(/\t/g, "    ");
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith("#")) continue;

		if (/^\S[^:]*:\s*$/.test(line)) {
			currentChain = line.slice(0, line.indexOf(":")).trim();
			chains[currentChain] = { steps: [] };
			currentStep = undefined;
			continue;
		}

		if (!currentChain) continue;
		const chain = chains[currentChain];
		if (/^\s{2}description:\s*/.test(line)) {
			chain.description = trimmed.slice("description:".length).trim().replace(/^"|"$/g, "");
			continue;
		}
		if (/^\s{2}steps:\s*$/.test(line)) {
			currentStep = undefined;
			continue;
		}
		if (/^\s{4}-\s+agent:\s*/.test(line)) {
			currentStep = {
				agent: trimmed.replace(/^-\s+agent:\s*/, "").trim(),
			};
			chain.steps.push(currentStep);
			continue;
		}
		if (currentStep && /^\s{6}prompt:\s*/.test(line)) {
			currentStep.prompt = trimmed.slice("prompt:".length).trim().replace(/^"|"$/g, "");
		}
	}
	return chains;
}

function commandAvailable(command: string, args: string[]): boolean {
	try {
		const result = spawnSync(command, args, {
			stdio: "ignore",
			shell: false,
			timeout: 4000,
			env: process.env,
		});
		return !result.error && result.status === 0;
	} catch {
		return false;
	}
}

function resolveSearchCommand(root: string): string {
	const local = path.join(root, "bin", "aoc-search");
	return fs.existsSync(local) ? local : "aoc-search";
}

function scoutAvailability(root: string): AgentAvailability {
	const browserBin = process.env.AOC_AGENT_BROWSER_BIN?.trim() || "agent-browser";
	const searchToml = path.join(root, ".aoc", "search.toml");
	const composeFile = path.join(root, ".aoc", "services", "searxng", "docker-compose.yml");
	const settingsFile = path.join(root, ".aoc", "services", "searxng", "settings.yml");
	const browserSkill = path.join(root, ".pi", "skills", "agent-browser", "SKILL.md");
	const reasons: string[] = [];
	if (!fs.existsSync(browserSkill)) reasons.push("missing .pi/skills/agent-browser/SKILL.md");
	if (!commandAvailable(browserBin, ["--version"])) reasons.push(`missing browser runtime (${browserBin})`);
	if (!fs.existsSync(searchToml)) reasons.push("missing .aoc/search.toml");
	if (!fs.existsSync(composeFile)) reasons.push("missing .aoc/services/searxng/docker-compose.yml");
	if (!fs.existsSync(settingsFile)) reasons.push("missing .aoc/services/searxng/settings.yml");
	if (reasons.length === 0 && !commandAvailable(resolveSearchCommand(root), ["health"])) {
		reasons.push("aoc-search health failed");
	}
	return reasons.length === 0 ? { available: true } : { available: false, reason: reasons.join("; ") };
}

export function agentAvailability(root: string, agent: AgentConfig): AgentAvailability {
	switch (agent.name) {
		case "scout-web-agent":
			return scoutAvailability(root);
		default:
			return { available: true };
	}
}

function manifestCacheKey(root: string): string {
	const agentsDir = path.join(root, ".pi", "agents");
	if (!fs.existsSync(agentsDir)) return `missing:${agentsDir}`;
	const files = fs.readdirSync(agentsDir, { withFileTypes: true })
		.filter((entry) => entry.isFile() && (entry.name.endsWith(".md") || entry.name === "teams.yaml" || entry.name === "agent-chain.yaml"))
		.map((entry) => {
			const fullPath = path.join(agentsDir, entry.name);
			const stats = fs.statSync(fullPath);
			return `${entry.name}:${stats.mtimeMs}:${stats.size}`;
		})
		.sort();
	return `${agentsDir}|${files.join("|")}`;
}

export function loadManifestBundle(root: string): ManifestBundle {
	const key = manifestCacheKey(root);
	const cached = manifestCache.get(root);
	if (cached && cached.key === key) return cached.bundle;

	const agentsDir = path.join(root, ".pi", "agents");
	const teamsFile = path.join(agentsDir, "teams.yaml");
	const chainFile = path.join(agentsDir, "agent-chain.yaml");
	const validationErrors: string[] = [];
	const agents: AgentConfig[] = [];

	if (fs.existsSync(agentsDir)) {
		for (const entry of fs.readdirSync(agentsDir, { withFileTypes: true })) {
			if (!entry.isFile() || !entry.name.endsWith(".md")) continue;
			const fullPath = path.join(agentsDir, entry.name);
			try {
				agents.push(parseAgentFile(fs.readFileSync(fullPath, "utf8"), fullPath));
			} catch (error) {
				validationErrors.push(String(error));
			}
		}
	}

	const teams = fs.existsSync(teamsFile) ? parseTeamsYaml(fs.readFileSync(teamsFile, "utf8")) : {};
	const chains = fs.existsSync(chainFile) ? parseChainsYaml(fs.readFileSync(chainFile, "utf8")) : {};
	const agentNames = new Set(agents.map((agent) => agent.name));

	for (const [team, members] of Object.entries(teams)) {
		for (const member of members) {
			if (!agentNames.has(member)) validationErrors.push(`team ${team} references unknown agent ${member}`);
		}
	}
	for (const [chainName, def] of Object.entries(chains)) {
		if (def.steps.length === 0) validationErrors.push(`chain ${chainName} has no steps`);
		for (const step of def.steps) {
			if (!agentNames.has(step.agent)) validationErrors.push(`chain ${chainName} references unknown agent ${step.agent}`);
		}
	}

	agents.sort((a, b) => a.name.localeCompare(b.name));
	const bundle = { agents, teams, chains, validationErrors, agentsDir };
	manifestCache.set(root, { key, bundle });
	return bundle;
}

export function availableAgents(bundle: ManifestBundle, root: string): AgentConfig[] {
	return bundle.agents.filter((agent) => agentAvailability(root, agent).available);
}

export function availableChains(bundle: ManifestBundle, root: string): Record<string, ChainDefinition> {
	return Object.fromEntries(
		Object.entries(bundle.chains).filter(([, def]) => def.steps.every((step) => {
			const agent = bundle.agents.find((candidate) => candidate.name === step.agent);
			return agent ? agentAvailability(root, agent).available : false;
		})),
	);
}

export function availableTeams(bundle: ManifestBundle, root: string): Record<string, string[]> {
	return Object.fromEntries(
		Object.entries(bundle.teams).filter(([, members]) => members.length > 0 && members.every((member) => {
			const agent = bundle.agents.find((candidate) => candidate.name === member);
			return agent ? agentAvailability(root, agent).available : false;
		})),
	);
}

export function assertAgentAvailable(bundle: ManifestBundle, root: string, agentName: string): void {
	const agent = bundle.agents.find((candidate) => candidate.name === agentName);
	if (!agent) return;
	const availability = agentAvailability(root, agent);
	if (!availability.available) throw new Error(`Agent unavailable: ${agentName} (${availability.reason})`);
}

export function assertChainAvailable(bundle: ManifestBundle, root: string, chainName: string): void {
	const chain = bundle.chains[chainName];
	if (!chain) return;
	for (const step of chain.steps) {
		const agent = bundle.agents.find((candidate) => candidate.name === step.agent);
		if (!agent) continue;
		const availability = agentAvailability(root, agent);
		if (!availability.available) {
			throw new Error(`Chain unavailable: ${chainName} requires ${step.agent} (${availability.reason})`);
		}
	}
}

export function assertTeamAvailable(bundle: ManifestBundle, root: string, teamName: string): void {
	const members = bundle.teams[teamName];
	if (!members) return;
	for (const member of members) {
		const agent = bundle.agents.find((candidate) => candidate.name === member);
		if (!agent) continue;
		const availability = agentAvailability(root, agent);
		if (!availability.available) {
			throw new Error(`Team unavailable: ${teamName} requires ${member} (${availability.reason})`);
		}
	}
}
