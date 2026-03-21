import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import * as fs from "node:fs";
import * as net from "node:net";
import * as os from "node:os";
import * as path from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { StringEnum } from "@mariozechner/pi-ai";
import { Type } from "@sinclair/typebox";

type AgentConfig = {
	name: string;
	description?: string;
	tools: string[];
	model?: string;
	systemPrompt: string;
	sourcePath: string;
};

type ChainStep = {
	agent: string;
	prompt?: string;
};

type ChainDefinition = {
	description?: string;
	steps: ChainStep[];
};

type ManifestBundle = {
	agents: AgentConfig[];
	teams: Record<string, string[]>;
	chains: Record<string, ChainDefinition>;
	validationErrors: string[];
	agentsDir: string;
};

type AgentAvailability = {
	available: boolean;
	reason?: string;
};

type JobStatus = "queued" | "running" | "success" | "fallback" | "error" | "cancelled" | "stale";

type JobMode = "dispatch" | "chain" | "parallel";

type JobRecord = {
	jobId: string;
	mode: JobMode;
	agent: string;
	agentFile: string;
	status: JobStatus;
	task: string;
	cwd: string;
	createdAt: number;
	startedAt?: number;
	finishedAt?: number;
	pid?: number;
	exitCode?: number;
	model?: string;
	tools: string[];
	outputExcerpt?: string;
	stderrExcerpt?: string;
	error?: string;
	fallbackUsed: boolean;
	manifestErrors: string[];
	chainName?: string;
	chainStepIndex?: number;
	chainStepCount?: number;
};

type PersistedJobRecord = Omit<JobRecord, "pid">;

type PersistedHandoffRecord = {
	jobId: string;
	status: JobStatus;
	agent: string;
	mode: JobMode;
	createdAt: number;
	finishedAt?: number;
	chainName?: string;
	outputExcerpt?: string;
	stderrExcerpt?: string;
	error?: string;
	fallbackUsed: boolean;
};

type RuntimeState = {
	initialized: boolean;
	ctx?: ExtensionContext;
	jobs: Map<string, JobRecord>;
	registryJobs: Map<string, JobRecord>;
	children: Map<string, ChildProcessWithoutNullStreams>;
	handoffNotified: Set<string>;
};

const ENTRY_TYPE = "aoc-subagent-job-v1";
const HANDOFF_ENTRY_TYPE = "aoc-subagent-handoff-v1";
const WIDGET_ID = "aoc-subagent-jobs";
const STATUS_ID = "aoc-subagent";
const MAX_WIDGET_LINES = 6;
const MAX_OUTPUT_CHARS = 1200;
const DEFAULT_AGENT = "insight-t1-observer";
const DETACHED_STATUS_LIMIT = 24;
const PULSE_COMMAND_TIMEOUT_MS = 3000;

const state: RuntimeState = {
	initialized: false,
	jobs: new Map(),
	registryJobs: new Map(),
	children: new Map(),
	handoffNotified: new Set(),
};

const ActionSchema = StringEnum(["dispatch", "dispatch_chain", "status", "cancel", "list_agents"] as const, {
	description: "Action to perform for the AOC subagent runtime.",
});

const SubagentParams = Type.Object({
	action: ActionSchema,
	agent: Type.Optional(Type.String({ description: "Canonical agent name from .pi/agents/*.md." })),
	chain: Type.Optional(Type.String({ description: "Canonical chain name from .pi/agents/agent-chain.yaml." })),
	task: Type.Optional(Type.String({ description: "Task prompt for detached dispatch or chain input." })),
	jobId: Type.Optional(Type.String({ description: "Detached subagent job id for status/cancel actions." })),
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
});

function now(): number {
	return Date.now();
}

function randomId(): string {
	return Math.random().toString(36).slice(2, 10);
}

function makeJobId(agent: string): string {
	return `sj_${now().toString(36)}_${sanitizeSlug(agent)}_${randomId()}`;
}

function sanitizeSlug(input: string): string {
	return input
		.toLowerCase()
		.replace(/[^a-z0-9._-]+/g, "-")
		.replace(/-+/g, "-")
		.replace(/^-|-$/g, "") || "agent";
}

function truncate(text: string | undefined, max = MAX_OUTPUT_CHARS): string | undefined {
	if (!text) return undefined;
	if (text.length <= max) return text;
	return `${text.slice(0, Math.max(0, max - 1))}…`;
}

function relative(root: string, target: string): string {
	const rel = path.relative(root, target);
	return rel && !rel.startsWith("..") ? rel : target;
}

function normalizePathArg(raw: string): string {
	return raw.startsWith("@") ? raw.slice(1) : raw;
}

function resolveScopedCwd(root: string, requested?: string): string {
	if (!requested || !requested.trim()) return root;
	const resolved = path.resolve(root, normalizePathArg(requested.trim()));
	const relativeToRoot = path.relative(root, resolved);
	if (relativeToRoot.startsWith("..") || path.isAbsolute(relativeToRoot)) {
		throw new Error(`cwd escapes project root: ${requested}`);
	}
	return resolved;
}

type PulseCommandResultPayload = {
	command: string;
	status: string;
	message?: string;
	error?: { code?: string; message?: string };
};

type PulseEnvelope = {
	version?: string | number;
	type?: string;
	session_id?: string;
	sender_id?: string;
	timestamp?: string;
	request_id?: string;
	payload?: any;
};

type DurableDetachedJob = {
	job_id: string;
	parent_job_id?: string | null;
	owner_plane?: string;
	worker_kind?: string | null;
	mode?: string;
	status?: string;
	agent?: string | null;
	team?: string | null;
	chain?: string | null;
	created_at_ms?: number;
	started_at_ms?: number | null;
	finished_at_ms?: number | null;
	current_step_index?: number | null;
	step_count?: number | null;
	output_excerpt?: string | null;
	stdout_excerpt?: string | null;
	stderr_excerpt?: string | null;
	error?: string | null;
	fallback_used?: boolean;
};

type DurableDetachedStatusResult = {
	status?: string;
	jobs?: DurableDetachedJob[];
	active_jobs?: number;
	fallback_used?: boolean;
};

type DurableDetachedCancelResult = {
	job_id: string;
	status?: string;
	summary?: string;
	cancelled?: boolean;
	fallback_used?: boolean;
};

type DurableDetachedDispatchResult = {
	status?: string;
	summary?: string;
	accepted?: boolean;
	fallback_used?: boolean;
	job?: DurableDetachedJob;
};

function currentSessionId(): string | undefined {
	const value = process.env.AOC_SESSION_ID?.trim();
	return value ? value : undefined;
}

function currentPaneId(): string | undefined {
	const value = process.env.AOC_PANE_ID?.trim() || process.env.ZELLIJ_PANE_ID?.trim();
	return value ? value : undefined;
}

function currentAgentKey(): string | undefined {
	const sessionId = currentSessionId();
	const paneId = currentPaneId();
	if (!sessionId || !paneId) return undefined;
	return `${sessionId}::${paneId}`;
}

function sessionSlug(sessionId: string): string {
	let slug = sessionId.replace(/[^A-Za-z0-9._-]/g, "-");
	while (slug.includes("--")) slug = slug.replace(/--/g, "-");
	return slug.replace(/^-|-$/g, "") || "session";
}

function resolvePulseSocketPath(): string | undefined {
	const explicit = process.env.AOC_PULSE_SOCK?.trim();
	if (explicit) return explicit;
	const sessionId = currentSessionId();
	if (!sessionId) return undefined;
	const runtimeDir = process.env.XDG_RUNTIME_DIR?.trim()
		|| (process.env.UID?.trim() ? `/run/user/${process.env.UID.trim()}` : "/tmp");
	return path.join(runtimeDir, "aoc", sessionSlug(sessionId), "pulse.sock");
}

function pulseClientId(): string {
	return `pi-subagent-${process.pid}-${randomId()}`;
}

function isTerminalCommandStatus(status: string | undefined): boolean {
	if (!status) return false;
	return status !== "accepted" && status !== "queued" && status !== "running";
}

function modeFromDurable(mode?: string): JobMode {
	switch (mode) {
		case "chain":
			return "chain";
		case "parallel":
			return "parallel";
		case "dispatch":
		default:
			return "dispatch";
	}
}

function statusFromDurable(status?: string): JobStatus {
	switch (status) {
		case "queued":
		case "running":
		case "success":
		case "fallback":
		case "error":
		case "cancelled":
		case "stale":
			return status;
		default:
			return "error";
	}
}

function mapDurableJob(job: DurableDetachedJob, root: string): JobRecord {
	const agent = job.agent || job.chain || job.team || "detached-job";
	return {
		jobId: job.job_id,
		mode: modeFromDurable(job.mode),
		agent,
		agentFile: job.agent ? relative(root, path.join(root, ".pi", "agents", `${job.agent}.md`)) : "durable-registry",
		status: statusFromDurable(job.status),
		task: "",
		cwd: root,
		createdAt: job.created_at_ms ?? now(),
		startedAt: job.started_at_ms ?? undefined,
		finishedAt: job.finished_at_ms ?? undefined,
		model: undefined,
		tools: [],
		outputExcerpt: truncate(job.output_excerpt ?? job.stdout_excerpt ?? undefined),
		stderrExcerpt: truncate(job.stderr_excerpt ?? undefined, 320),
		error: truncate(job.error ?? undefined, 320),
		fallbackUsed: Boolean(job.fallback_used),
		manifestErrors: [],
		chainName: job.chain ?? undefined,
		chainStepIndex: job.current_step_index ?? undefined,
		chainStepCount: job.step_count ?? undefined,
	};
}

async function sendPulseCommand(command: string, args: Record<string, unknown>): Promise<PulseCommandResultPayload> {
	const sessionId = currentSessionId();
	const targetAgentId = currentAgentKey();
	const socketPath = resolvePulseSocketPath();
	if (!sessionId || !targetAgentId || !socketPath) {
		throw new Error("detached registry unavailable: missing AOC session/pane/socket context");
	}

	const requestId = `subagent-${now()}-${randomId()}`;
	const senderId = pulseClientId();
	const writeEnvelope = (socket: net.Socket, type: string, payload: any, request?: string) => {
		const envelope = {
			version: "1",
			type,
			session_id: sessionId,
			sender_id: senderId,
			timestamp: new Date().toISOString(),
			request_id: request,
			payload,
		};
		socket.write(`${JSON.stringify(envelope)}\n`);
	};

	return await new Promise<PulseCommandResultPayload>((resolve, reject) => {
		const socket = net.createConnection(socketPath);
		let settled = false;
		let buffer = "";
		const finish = (error?: Error, result?: PulseCommandResultPayload) => {
			if (settled) return;
			settled = true;
			clearTimeout(timeout);
			socket.destroy();
			if (error) reject(error);
			else resolve(result!);
		};
		const timeout = setTimeout(() => finish(new Error(`pulse command timed out after ${PULSE_COMMAND_TIMEOUT_MS}ms`)), PULSE_COMMAND_TIMEOUT_MS);

		socket.on("connect", () => {
			writeEnvelope(socket, "hello", {
				client_id: senderId,
				role: "subscriber",
				capabilities: ["snapshot", "delta", "command_result"],
			});
			writeEnvelope(socket, "subscribe", { topics: ["command_result"] });
			writeEnvelope(
				socket,
				"command",
				{ command, target_agent_id: targetAgentId, args },
				requestId,
			);
		});

		socket.on("data", (chunk: Buffer) => {
			buffer += chunk.toString("utf8");
			const lines = buffer.split("\n");
			buffer = lines.pop() ?? "";
			for (const line of lines) {
				if (!line.trim()) continue;
				let envelope: PulseEnvelope;
				try {
					envelope = JSON.parse(line);
				} catch {
					continue;
				}
				if (envelope.session_id !== sessionId) continue;
				if (envelope.request_id !== requestId) continue;
				if (envelope.type !== "command_result") continue;
				const payload = envelope.payload as PulseCommandResultPayload | undefined;
				if (!payload) continue;
				if (!isTerminalCommandStatus(payload.status)) continue;
				finish(undefined, payload);
				return;
			}
		});

		socket.on("error", (error) => finish(error instanceof Error ? error : new Error(String(error))));
		socket.on("close", () => {
			if (!settled) finish(new Error("pulse socket closed before detached registry response arrived"));
		});
	});
}

async function refreshRegistryJobs(ctx: ExtensionContext, targetJobId?: string, pi?: ExtensionAPI): Promise<void> {
	const root = ctx.cwd ?? process.cwd();
	try {
		const result = await sendPulseCommand("insight_detached_status", {
			job_id: targetJobId,
			owner_plane: "delegated",
			limit: DETACHED_STATUS_LIMIT,
		});
		if (result.status !== "ok") return;
		const payload = result.message ? (JSON.parse(result.message) as DurableDetachedStatusResult) : undefined;
		const next = new Map<string, JobRecord>();
		for (const job of payload?.jobs ?? []) {
			next.set(job.job_id, mapDurableJob(job, root));
		}
		const previous = new Map(state.registryJobs);
		if (targetJobId) {
			const merged = new Map(state.registryJobs);
			for (const [jobId, job] of next.entries()) merged.set(jobId, job);
			state.registryJobs = merged;
		} else {
			state.registryJobs = next;
		}
		for (const [jobId, job] of state.registryJobs.entries()) {
			const prior = previous.get(jobId);
			if (!isTerminalJobStatus(job.status) || prior?.status === job.status) continue;
			if (pi) {
				maybeNotifyHandoff(pi, ctx, job);
			} else if (ctx.ui && !state.handoffNotified.has(jobId)) {
				ctx.ui.notify(
					`${statusIcon(job.status)} ${job.agent} finished (${job.jobId}) — /subagent-inspect ${job.jobId}`,
					job.status === "success" ? "info" : "warning",
				);
			}
		}
		updateUi(ctx);
	} catch {
		// fail open when no pulse socket / wrapper runtime is reachable
	}
}

async function startDetachedDispatchViaRegistry(
	ctx: ExtensionContext,
	agentName: string,
	task: string,
	cwdArg?: string,
	pi?: ExtensionAPI,
): Promise<JobRecord | undefined> {
	const root = ctx.cwd ?? process.cwd();
	const cwd = resolveScopedCwd(root, cwdArg);
	const result = await sendPulseCommand("insight_detached_dispatch", {
		mode: "dispatch",
		owner_plane: "delegated",
		worker_kind: "specialist",
		agent: agentName,
		input: task,
		cwd: relative(root, cwd),
		reason: "pi_subagent_extension",
	});
	if (result.status !== "ok" || !result.message) return undefined;
	const payload = JSON.parse(result.message) as DurableDetachedDispatchResult;
	if (!payload.job) return undefined;
	const job = mapDurableJob(payload.job, root);
	state.registryJobs.set(job.jobId, job);
	updateUi(ctx);
	await refreshRegistryJobs(ctx, job.jobId, pi);
	return state.registryJobs.get(job.jobId) ?? job;
}

async function startDetachedChainViaRegistry(
	ctx: ExtensionContext,
	chainName: string,
	task: string,
	cwdArg?: string,
	pi?: ExtensionAPI,
): Promise<JobRecord | undefined> {
	const root = ctx.cwd ?? process.cwd();
	const cwd = resolveScopedCwd(root, cwdArg);
	const result = await sendPulseCommand("insight_detached_dispatch", {
		mode: "chain",
		owner_plane: "delegated",
		worker_kind: "chain_step",
		chain: chainName,
		input: task,
		cwd: relative(root, cwd),
		reason: "pi_subagent_extension",
	});
	if (result.status !== "ok" || !result.message) return undefined;
	const payload = JSON.parse(result.message) as DurableDetachedDispatchResult;
	if (!payload.job) return undefined;
	const job = mapDurableJob(payload.job, root);
	state.registryJobs.set(job.jobId, job);
	updateUi(ctx);
	await refreshRegistryJobs(ctx, job.jobId, pi);
	return state.registryJobs.get(job.jobId) ?? job;
}

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

function loadManifestBundle(root: string): ManifestBundle {
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
	return { agents, teams, chains, validationErrors, agentsDir };
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

function agentAvailability(root: string, agent: AgentConfig): AgentAvailability {
	switch (agent.name) {
		case "scout-web-agent":
			return scoutAvailability(root);
		default:
			return { available: true };
	}
}

function availableAgents(bundle: ManifestBundle, root: string): AgentConfig[] {
	return bundle.agents.filter((agent) => agentAvailability(root, agent).available);
}

function availableChains(bundle: ManifestBundle, root: string): Record<string, ChainDefinition> {
	return Object.fromEntries(
		Object.entries(bundle.chains).filter(([, def]) => def.steps.every((step) => {
			const agent = bundle.agents.find((candidate) => candidate.name === step.agent);
			return agent ? agentAvailability(root, agent).available : false;
		})),
	);
}

function assertAgentAvailable(bundle: ManifestBundle, root: string, agentName: string): void {
	const agent = bundle.agents.find((candidate) => candidate.name === agentName);
	if (!agent) return;
	const availability = agentAvailability(root, agent);
	if (!availability.available) throw new Error(`Agent unavailable: ${agentName} (${availability.reason})`);
}

function assertChainAvailable(bundle: ManifestBundle, root: string, chainName: string): void {
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

function writePromptToTempFile(agentName: string, prompt: string): { dir: string; file: string } | undefined {
	if (!prompt.trim()) return undefined;
	const dir = fs.mkdtempSync(path.join(os.tmpdir(), "aoc-subagent-"));
	const file = path.join(dir, `${sanitizeSlug(agentName)}.md`);
	fs.writeFileSync(file, prompt, { encoding: "utf8", mode: 0o600 });
	return { dir, file };
}

function extractAssistantText(message: any): string {
	const content = Array.isArray(message?.content) ? message.content : [];
	return content
		.filter((part: any) => part?.type === "text" && typeof part?.text === "string")
		.map((part: any) => part.text)
		.join("\n")
		.trim();
}

function statusIcon(status: JobStatus): string {
	switch (status) {
		case "queued":
			return "…";
		case "running":
			return "▶";
		case "success":
			return "✓";
		case "fallback":
			return "△";
		case "cancelled":
			return "×";
		case "stale":
			return "!";
		case "error":
		default:
			return "✗";
	}
}

function sortJobs(jobs: Iterable<JobRecord>): JobRecord[] {
	return Array.from(jobs).sort((a, b) => b.createdAt - a.createdAt);
}

function combinedJobs(): JobRecord[] {
	const merged = new Map<string, JobRecord>();
	for (const [jobId, job] of state.registryJobs.entries()) merged.set(jobId, job);
	for (const [jobId, job] of state.jobs.entries()) merged.set(jobId, job);
	return sortJobs(merged.values());
}

function isTerminalJobStatus(status: JobStatus): boolean {
	return status === "success" || status === "fallback" || status === "error" || status === "cancelled" || status === "stale";
}

function activeJobs(): JobRecord[] {
	return combinedJobs().filter((job) => job.status === "queued" || job.status === "running");
}

function recentJobs(limit = 6): JobRecord[] {
	return combinedJobs().filter((job) => isTerminalJobStatus(job.status)).slice(0, limit);
}

function summarizeJobOutcome(job: JobRecord): string {
	const detail = job.outputExcerpt || job.error || job.stderrExcerpt || "no excerpt recorded";
	return truncate(detail, 160) || "no excerpt recorded";
}

function handoffRecord(job: JobRecord): PersistedHandoffRecord {
	return {
		jobId: job.jobId,
		status: job.status,
		agent: job.agent,
		mode: job.mode,
		createdAt: job.createdAt,
		finishedAt: job.finishedAt,
		chainName: job.chainName,
		outputExcerpt: job.outputExcerpt,
		stderrExcerpt: job.stderrExcerpt,
		error: job.error,
		fallbackUsed: job.fallbackUsed,
	};
}

function persistHandoff(pi: ExtensionAPI, job: JobRecord): void {
	if (!isTerminalJobStatus(job.status) || state.handoffNotified.has(job.jobId)) return;
	pi.appendEntry<PersistedHandoffRecord>(HANDOFF_ENTRY_TYPE, handoffRecord(job));
	state.handoffNotified.add(job.jobId);
}

function maybeNotifyHandoff(pi: ExtensionAPI, ctx: ExtensionContext | undefined, job: JobRecord): void {
	if (!ctx?.ui || !isTerminalJobStatus(job.status) || state.handoffNotified.has(job.jobId)) return;
	persistHandoff(pi, job);
	ctx.ui.notify(
		`${statusIcon(job.status)} ${job.agent} finished (${job.jobId}) — /subagent-inspect ${job.jobId}`,
		job.status === "success" ? "info" : "warning",
	);
}

function updateUi(ctx?: ExtensionContext): void {
	const runtimeCtx = ctx ?? state.ctx;
	if (!runtimeCtx?.ui) return;
	const active = activeJobs();
	const recent = recentJobs(3);
	if (active.length > 0) {
		const suffix = recent.length > 0 ? ` · recent:${recent.length}` : "";
		runtimeCtx.ui.setStatus(STATUS_ID, runtimeCtx.ui.theme.fg("accent", `subagents:${active.length}${suffix}`));
	} else if (recent.length > 0) {
		runtimeCtx.ui.setStatus(STATUS_ID, runtimeCtx.ui.theme.fg("muted", `subagents recent:${recent.length}`));
	} else {
		runtimeCtx.ui.setStatus(STATUS_ID, undefined);
	}

	const lines: string[] = [];
	if (active.length > 0) {
		lines.push("Active:");
		for (const job of active.slice(0, Math.max(1, MAX_WIDGET_LINES - 2))) {
			lines.push(`${statusIcon(job.status)} ${job.agent} ${job.jobId} · ${job.status}`);
		}
	}
	if (recent.length > 0 && lines.length < MAX_WIDGET_LINES) {
		lines.push("Recent:");
		for (const job of recent) {
			if (lines.length >= MAX_WIDGET_LINES) break;
			lines.push(`${statusIcon(job.status)} ${job.agent} ${job.jobId} · ${summarizeJobOutcome(job)}`);
		}
	}
	runtimeCtx.ui.setWidget(WIDGET_ID, lines.length > 0 ? lines : undefined, { placement: "belowEditor" });
}

function snapshotJob(job: JobRecord): PersistedJobRecord {
	const { pid: _pid, ...rest } = job;
	return rest;
}

function persistJob(pi: ExtensionAPI, job: JobRecord): void {
	pi.appendEntry<PersistedJobRecord>(ENTRY_TYPE, snapshotJob(job));
	persistHandoff(pi, job);
}

function restoreJobs(pi: ExtensionAPI, ctx: ExtensionContext): void {
	const entries = ctx.sessionManager.getEntries?.() ?? [];
	const restored = new Map<string, JobRecord>();
	let mutated = false;
	for (const entry of entries) {
		const rec = entry as any;
		if (rec?.type !== "custom" || !rec?.customType || !rec?.data) continue;
		if (rec.customType === ENTRY_TYPE) {
			const data = rec.data as PersistedJobRecord;
			restored.set(data.jobId, { ...data });
			continue;
		}
		if (rec.customType === HANDOFF_ENTRY_TYPE) {
			const data = rec.data as PersistedHandoffRecord;
			if (data?.jobId) state.handoffNotified.add(data.jobId);
		}
	}
	for (const job of restored.values()) {
		if (job.status === "queued" || job.status === "running") {
			job.status = "stale";
			job.finishedAt = job.finishedAt ?? now();
			job.error = job.error ?? "extension reloaded before detached result was observed";
			job.fallbackUsed = true;
			persistJob(pi, job);
			mutated = true;
		}
	}
	state.jobs = restored;
	state.registryJobs = new Map();
	if (mutated) updateUi(ctx);
}

function formatJob(job: JobRecord): string {
	const lines = [
		`- job: ${job.jobId}`,
		`  mode: ${job.mode}`,
		`  agent: ${job.agent}`,
		`  status: ${job.status}`,
		`  cwd: ${job.cwd}`,
		`  created_at: ${new Date(job.createdAt).toISOString()}`,
	];
	if (job.chainName) lines.push(`  chain: ${job.chainName}`);
	if (typeof job.chainStepIndex === "number" && typeof job.chainStepCount === "number") {
		lines.push(`  chain_step: ${job.chainStepIndex + 1}/${job.chainStepCount}`);
	}
	if (job.startedAt) lines.push(`  started_at: ${new Date(job.startedAt).toISOString()}`);
	if (job.finishedAt) lines.push(`  finished_at: ${new Date(job.finishedAt).toISOString()}`);
	if (typeof job.pid === "number") lines.push(`  pid: ${job.pid}`);
	if (typeof job.exitCode === "number") lines.push(`  exit_code: ${job.exitCode}`);
	if (job.model) lines.push(`  model: ${job.model}`);
	if (job.tools.length > 0) lines.push(`  tools: ${job.tools.join(",")}`);
	if (job.error) lines.push(`  error: ${job.error}`);
	if (job.outputExcerpt) lines.push(`  output: ${JSON.stringify(job.outputExcerpt)}`);
	if (job.stderrExcerpt) lines.push(`  stderr: ${JSON.stringify(job.stderrExcerpt)}`);
	return lines.join("\n");
}

function formatStatusReport(targetJobId?: string): string {
	const jobs = combinedJobs();
	if (jobs.length === 0) return "No detached subagent jobs recorded in this session or durable registry.";
	if (targetJobId) {
		const job = jobs.find((candidate) => candidate.jobId === targetJobId);
		if (!job) return `Unknown detached subagent job: ${targetJobId}`;
		const sections = [formatJob(job)];
		if (isTerminalJobStatus(job.status)) sections.push(formatHandoff(job.jobId));
		return sections.join("\n\n");
	}
	return [jobs.map(formatJob).join("\n\n"), "Recent completions:", formatRecentJobs(5)].join("\n\n");
}

function lookupJob(jobId: string): JobRecord | undefined {
	return combinedJobs().find((candidate) => candidate.jobId === jobId);
}

function formatRecentJobs(limit = 5): string {
	const jobs = recentJobs(limit);
	if (jobs.length === 0) return "No recent detached subagent completions yet.";
	return jobs
		.map((job) => `${statusIcon(job.status)} ${job.jobId} · ${job.agent} · ${job.status} · ${summarizeJobOutcome(job)}`)
		.join("\n");
}

function formatHandoff(jobId: string): string {
	const job = lookupJob(jobId);
	if (!job) return `Unknown detached subagent job: ${jobId}`;
	const lines = [
		`Detached subagent handoff`,
		`job_id: ${job.jobId}`,
		`agent: ${job.agent}`,
		`mode: ${job.mode}`,
		`status: ${job.status}`,
		`fallback_used: ${job.fallbackUsed ? "yes" : "no"}`,
	];
	if (job.chainName) lines.push(`chain: ${job.chainName}`);
	if (job.finishedAt) lines.push(`finished_at: ${new Date(job.finishedAt).toISOString()}`);
	if (job.outputExcerpt) lines.push(`result: ${job.outputExcerpt}`);
	if (job.error) lines.push(`error: ${job.error}`);
	if (job.stderrExcerpt) lines.push(`stderr: ${job.stderrExcerpt}`);
	lines.push(`next_action: review with /subagent-inspect ${job.jobId}`);
	return lines.join("\n");
}

function formatAgentCatalog(bundle: ManifestBundle, root: string): string {
	const lines: string[] = [];
	const available = availableAgents(bundle, root);
	const unavailable = bundle.agents
		.map((agent) => ({ agent, availability: agentAvailability(root, agent) }))
		.filter((entry) => !entry.availability.available);
	const chains = availableChains(bundle, root);
	lines.push(`Agents dir: ${relative(root, bundle.agentsDir)}`);
	if (available.length === 0) {
		lines.push("No currently available canonical project-local agents found.");
	} else {
		lines.push("Available agents:");
		for (const agent of available) {
			const desc = agent.description ? ` — ${agent.description}` : "";
			const tools = agent.tools.length > 0 ? ` [tools: ${agent.tools.join(",")}]` : "";
			lines.push(`- ${agent.name}${desc}${tools}`);
		}
	}
	if (unavailable.length > 0) {
		lines.push("", "Unavailable agents:");
		for (const { agent, availability } of unavailable) {
			lines.push(`- ${agent.name}: ${availability.reason ?? "unavailable"}`);
		}
	}
	if (Object.keys(bundle.teams).length > 0) {
		lines.push("", "Teams:");
		for (const [team, members] of Object.entries(bundle.teams)) {
			const filtered = members.filter((member) => {
				const agent = bundle.agents.find((candidate) => candidate.name === member);
				return agent ? agentAvailability(root, agent).available : false;
			});
			if (filtered.length > 0) lines.push(`- ${team}: ${filtered.join(", ")}`);
		}
	}
	if (Object.keys(chains).length > 0) {
		lines.push("", "Chains:");
		for (const [name, def] of Object.entries(chains)) {
			lines.push(`- ${name}: ${def.steps.map((step) => step.agent).join(" -> ")}`);
		}
	}
	if (bundle.validationErrors.length > 0) {
		lines.push("", "Validation errors:");
		for (const error of bundle.validationErrors) lines.push(`- ${error}`);
	}
	return lines.join("\n");
}

function finalizeJob(pi: ExtensionAPI, ctx: ExtensionContext | undefined, jobId: string, patch: Partial<JobRecord>): void {
	const current = state.jobs.get(jobId);
	if (!current) return;
	const previousStatus = current.status;
	const updated: JobRecord = { ...current, ...patch };
	state.jobs.set(jobId, updated);
	persistJob(pi, updated);
	if (updated.status !== previousStatus && isTerminalJobStatus(updated.status)) {
		maybeNotifyHandoff(pi, ctx, updated);
	}
	updateUi(ctx);
}

function spawnDetachedStep(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	jobId: string,
	agent: AgentConfig,
	task: string,
	cwd: string,
	onSettled?: (result: { ok: boolean; status: JobStatus; output?: string; error?: string; stderr?: string; exitCode?: number }) => void,
): void {
	const root = ctx.cwd ?? process.cwd();
	const args: string[] = ["--mode", "json", "-p", "--no-session"];
	if (agent.model) args.push("--model", agent.model);
	if (agent.tools.length > 0) args.push("--tools", agent.tools.join(","));

	const promptFile = writePromptToTempFile(agent.name, agent.systemPrompt);
	if (promptFile) args.push("--append-system-prompt", promptFile.file);
	args.push(`Task: ${task}`);

	const proc = spawn("pi", args, {
		cwd,
		stdio: ["ignore", "pipe", "pipe"],
		shell: false,
		env: {
			...process.env,
			AOC_SUBAGENT_JOB_ID: jobId,
			AOC_SUBAGENT_AGENT: agent.name,
			AOC_SUBAGENT_AGENT_FILE: agent.sourcePath,
			AOC_SUBAGENT_PARENT_SESSION_ID: String(ctx.sessionManager.getSessionId?.() ?? ""),
		},
	});
	state.children.set(jobId, proc);
	finalizeJob(pi, ctx, jobId, {
		status: "running",
		startedAt: state.jobs.get(jobId)?.startedAt ?? now(),
		pid: proc.pid,
		agent: agent.name,
		agentFile: relative(root, agent.sourcePath),
		task,
		model: agent.model,
		tools: agent.tools,
	});

	let stdoutBuffer = "";
	let stderrBuffer = "";
	let latestAssistantText = "";
	let cleanupDone = false;

	const cleanup = () => {
		if (cleanupDone) return;
		cleanupDone = true;
		state.children.delete(jobId);
		if (promptFile) {
			try {
				fs.unlinkSync(promptFile.file);
			} catch {}
			try {
				fs.rmdirSync(promptFile.dir);
			} catch {}
		}
	};

	const processLine = (line: string) => {
		if (!line.trim()) return;
		let event: any;
		try {
			event = JSON.parse(line);
		} catch {
			return;
		}
		if (event?.type === "message_end" && event?.message?.role === "assistant") {
			const text = extractAssistantText(event.message);
			if (text) {
				latestAssistantText = text;
				finalizeJob(pi, ctx, jobId, { outputExcerpt: truncate(text) });
			}
			if (event.message?.errorMessage) {
				finalizeJob(pi, ctx, jobId, { error: truncate(String(event.message.errorMessage), 320) });
			}
		}
	};

	proc.stdout.on("data", (chunk: Buffer) => {
		stdoutBuffer += chunk.toString("utf8");
		const lines = stdoutBuffer.split("\n");
		stdoutBuffer = lines.pop() ?? "";
		for (const line of lines) processLine(line);
	});

	proc.stderr.on("data", (chunk: Buffer) => {
		stderrBuffer += chunk.toString("utf8");
		finalizeJob(pi, ctx, jobId, { stderrExcerpt: truncate(stderrBuffer, 320) });
	});

	proc.on("error", (error) => {
		cleanup();
		const errorText = `failed to spawn detached pi subprocess: ${error}`;
		finalizeJob(pi, ctx, jobId, {
			status: "error",
			finishedAt: now(),
			error: errorText,
			fallbackUsed: true,
			pid: undefined,
		});
		onSettled?.({ ok: false, status: "error", error: errorText });
	});

	proc.on("close", (code) => {
		if (stdoutBuffer.trim()) processLine(stdoutBuffer);
		cleanup();
		const current = state.jobs.get(jobId);
		if (!current) return;
		if (current.status === "cancelled") {
			finalizeJob(pi, ctx, jobId, { finishedAt: now(), exitCode: code ?? undefined, pid: undefined });
			onSettled?.({ ok: false, status: "cancelled", output: current.outputExcerpt, error: current.error, stderr: current.stderrExcerpt, exitCode: code ?? undefined });
			return;
		}
		const stderrExcerpt = truncate(stderrBuffer, 320);
		const ok = (code ?? 0) === 0;
		const status: JobStatus = ok ? "success" : "fallback";
		const error = !ok
			? truncate(
					stderrExcerpt || (latestAssistantText ? `detached subagent exited with status ${code}` : `subagent produced no assistant output (status ${code})`),
					320,
				)
			: current.error;
		finalizeJob(pi, ctx, jobId, {
			status,
			finishedAt: now(),
			exitCode: code ?? undefined,
			pid: undefined,
			outputExcerpt: truncate(latestAssistantText || current.outputExcerpt),
			stderrExcerpt,
			error,
			fallbackUsed: current.fallbackUsed || !ok,
		});
		onSettled?.({ ok, status, output: truncate(latestAssistantText || current.outputExcerpt), error, stderr: stderrExcerpt, exitCode: code ?? undefined });
	});
}

function startDetachedDispatch(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	bundle: ManifestBundle,
	agentName: string,
	task: string,
	cwdArg?: string,
): JobRecord {
	const root = ctx.cwd ?? process.cwd();
	assertAgentAvailable(bundle, root, agentName);
	const agent = bundle.agents.find((candidate) => candidate.name === agentName);
	if (!agent) {
		throw new Error(
			`Unknown canonical agent: ${agentName}. Available: ${bundle.agents.map((item) => item.name).join(", ") || "none"}`,
		);
	}
	const cwd = resolveScopedCwd(root, cwdArg);
	const jobId = makeJobId(agent.name);
	const job: JobRecord = {
		jobId,
		mode: "dispatch",
		agent: agent.name,
		agentFile: relative(root, agent.sourcePath),
		status: "queued",
		task,
		cwd,
		createdAt: now(),
		model: agent.model,
		tools: agent.tools,
		fallbackUsed: bundle.validationErrors.length > 0,
		manifestErrors: [...bundle.validationErrors],
	};

	state.jobs.set(jobId, job);
	persistJob(pi, job);
	updateUi(ctx);
	spawnDetachedStep(pi, ctx, jobId, agent, task, cwd);
	return state.jobs.get(jobId)!;
}

function startDetachedChain(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	bundle: ManifestBundle,
	chainName: string,
	input: string,
	cwdArg?: string,
): JobRecord {
	const root = ctx.cwd ?? process.cwd();
	assertChainAvailable(bundle, root, chainName);
	const chain = bundle.chains[chainName];
	if (!chain) {
		throw new Error(`Unknown canonical chain: ${chainName}. Available: ${Object.keys(bundle.chains).join(", ") || "none"}`);
	}
	if (chain.steps.length === 0) throw new Error(`Chain has no steps: ${chainName}`);
	const cwd = resolveScopedCwd(root, cwdArg);
	const firstAgent = bundle.agents.find((candidate) => candidate.name === chain.steps[0].agent);
	if (!firstAgent) throw new Error(`Chain ${chainName} references unknown agent ${chain.steps[0].agent}`);
	const jobId = makeJobId(chainName);
	const job: JobRecord = {
		jobId,
		mode: "chain",
		agent: firstAgent.name,
		agentFile: relative(root, firstAgent.sourcePath),
		status: "queued",
		task: input,
		cwd,
		createdAt: now(),
		model: firstAgent.model,
		tools: firstAgent.tools,
		fallbackUsed: bundle.validationErrors.length > 0,
		manifestErrors: [...bundle.validationErrors],
		chainName,
		chainStepIndex: 0,
		chainStepCount: chain.steps.length,
	};
	state.jobs.set(jobId, job);
	persistJob(pi, job);
	updateUi(ctx);

	const runStep = (index: number, previousOutput: string) => {
		const step = chain.steps[index];
		const agent = bundle.agents.find((candidate) => candidate.name === step.agent);
		if (!agent) {
			finalizeJob(pi, ctx, jobId, {
				status: "fallback",
				finishedAt: now(),
				chainStepIndex: index,
				error: `Chain ${chainName} references unknown agent ${step.agent}`,
				fallbackUsed: true,
			});
			return;
		}
		const stepTask = (step.prompt ?? "$INPUT")
			.replace(/\$ORIGINAL/g, input)
			.replace(/\$INPUT/g, previousOutput);
		finalizeJob(pi, ctx, jobId, {
			status: "queued",
			chainStepIndex: index,
			agent: agent.name,
			agentFile: relative(root, agent.sourcePath),
			task: stepTask,
			model: agent.model,
			tools: agent.tools,
			error: undefined,
		});
		spawnDetachedStep(pi, ctx, jobId, agent, stepTask, cwd, (result) => {
			if (!result.ok) return;
			if (index + 1 >= chain.steps.length) return;
			finalizeJob(pi, ctx, jobId, {
				status: "queued",
				finishedAt: undefined,
				exitCode: undefined,
				pid: undefined,
				chainStepIndex: index + 1,
			});
			setTimeout(() => runStep(index + 1, result.output || previousOutput), 0);
		});
	};

	runStep(0, input);
	return state.jobs.get(jobId)!;
}

async function cancelJob(pi: ExtensionAPI, ctx: ExtensionContext, jobId: string): Promise<JobRecord> {
	const job = state.jobs.get(jobId) ?? state.registryJobs.get(jobId);
	if (!job) throw new Error(`Unknown detached subagent job: ${jobId}`);
	const proc = state.children.get(jobId);
	if (proc) {
		finalizeJob(pi, ctx, jobId, {
			status: "cancelled",
			finishedAt: now(),
			error: "cancelled by operator",
			fallbackUsed: true,
		});
		proc.kill("SIGTERM");
		setTimeout(() => {
			if (!proc.killed) proc.kill("SIGKILL");
		}, 1500);
		return state.jobs.get(jobId)!;
	}

	try {
		const result = await sendPulseCommand("insight_detached_cancel", { job_id: jobId, reason: "cancelled by operator" });
		if (result.status === "ok" && result.message) {
			const payload = JSON.parse(result.message) as DurableDetachedCancelResult;
			await refreshRegistryJobs(ctx, payload.job_id, pi);
			const updated = state.registryJobs.get(payload.job_id);
			if (updated) return updated;
		}
	} catch {
		// fall through to stale/local fallback below
	}

	if (state.jobs.has(jobId) && (job.status === "queued" || job.status === "running")) {
		finalizeJob(pi, ctx, jobId, {
			status: "stale",
			finishedAt: now(),
			error: "no live subprocess handle or durable runtime cancellation path available",
			fallbackUsed: true,
		});
		return state.jobs.get(jobId)!;
	}
	return job;
}

async function ensureInitialized(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
	state.ctx = ctx;
	if (!state.initialized) {
		restoreJobs(pi, ctx);
		state.initialized = true;
	}
	await refreshRegistryJobs(ctx, undefined, pi);
	updateUi(ctx);
}

function registerFixedAgentCommand(pi: ExtensionAPI, name: string, agent: string, description: string): void {
	pi.registerCommand(name, {
		description,
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const task = args?.trim() || "";
			if (!task) {
				ctx.ui.notify(`Usage: /${name} <task>`, "warning");
				return;
			}
			const bundle = loadManifestBundle(ctx.cwd ?? process.cwd());
			assertAgentAvailable(bundle, ctx.cwd ?? process.cwd(), agent);
			const job = (await startDetachedDispatchViaRegistry(ctx, agent, task, undefined, pi).catch(() => undefined))
				?? startDetachedDispatch(pi, ctx, bundle, agent, task);
			ctx.ui.notify(`Detached ${agent} queued: ${job.jobId}`, "info");
		},
	});
}

export default function aocSubagentExtension(pi: ExtensionAPI): void {
	pi.on("session_start", async (_event, ctx) => {
		await ensureInitialized(pi, ctx);
	});

	pi.on("session_switch", async (_event, ctx) => {
		state.ctx = ctx;
		await refreshRegistryJobs(ctx, undefined, pi);
		updateUi(ctx);
	});

	pi.on("session_shutdown", async () => {
		for (const [jobId, proc] of state.children.entries()) {
			try {
				proc.kill("SIGTERM");
				state.children.delete(jobId);
			} catch {
				// ignore shutdown cleanup failures
			}
		}
	});

	pi.registerTool({
		name: "aoc_subagent",
		label: "AOC Subagent",
		description: "Dispatch, inspect, and cancel AOC-native detached project subagents defined under .pi/agents.",
		promptSnippet: "Use this to launch or inspect detached AOC subagents backed by canonical .pi/agents manifests.",
		promptGuidelines: [
			"Use action=dispatch to start one detached canonical project agent when the user asks for specialist background analysis.",
			"Use explorer-agent for repo reconnaissance, code-review-agent for bounded review, testing-agent for targeted verification, and scout-web-agent for browser/site investigation when the agent-browser + managed search stack is available.",
			"Use action=dispatch_chain with a canonical chain name when the user asks for a predefined multi-step handoff flow.",
			"Use action=status to inspect detached subagent job state instead of guessing completion.",
			"Use action=cancel only when the user asks to stop a detached job.",
		],
		parameters: SubagentParams,
		async execute(_toolCallId, params, signal, _onUpdate, ctx) {
			await ensureInitialized(pi, ctx);
			if (signal?.aborted) throw new Error("aoc_subagent aborted before execution");
			const root = ctx.cwd ?? process.cwd();
			const bundle = loadManifestBundle(root);
			switch (params.action) {
				case "list_agents": {
					const text = formatAgentCatalog(bundle, root);
					return { content: [{ type: "text", text }], details: { action: params.action } };
				}
				case "status": {
					await refreshRegistryJobs(ctx, params.jobId, pi);
					const text = formatStatusReport(params.jobId);
					return { content: [{ type: "text", text }], details: { action: params.action, jobId: params.jobId } };
				}
				case "cancel": {
					if (!params.jobId) throw new Error("cancel requires jobId");
					const job = await cancelJob(pi, ctx, params.jobId);
					return {
						content: [{ type: "text", text: `Cancelled ${job.jobId} (${job.agent}) -> ${job.status}` }],
						details: { action: params.action, job },
					};
				}
				case "dispatch": {
					const agent = params.agent?.trim() || DEFAULT_AGENT;
					const task = params.task?.trim();
					if (!task) throw new Error("dispatch requires task");
					assertAgentAvailable(bundle, root, agent);
					let job = await startDetachedDispatchViaRegistry(ctx, agent, task, params.cwd, pi).catch(() => undefined);
					if (!job) {
						job = startDetachedDispatch(pi, ctx, bundle, agent, task, params.cwd);
					}
					const lines = [
						`Queued detached subagent job ${job.jobId}`,
						`mode: ${job.mode}`,
						`agent: ${job.agent}`,
						`cwd: ${relative(root, job.cwd)}`,
						`agent_file: ${job.agentFile}`,
					];
					if (bundle.validationErrors.length > 0) {
						lines.push("manifest_warnings:");
						for (const error of bundle.validationErrors) lines.push(`- ${error}`);
					}
					return { content: [{ type: "text", text: lines.join("\n") }], details: { action: params.action, job } };
				}
				case "dispatch_chain": {
					const chain = params.chain?.trim();
					const task = params.task?.trim();
					if (!chain) throw new Error("dispatch_chain requires chain");
					if (!task) throw new Error("dispatch_chain requires task");
					assertChainAvailable(bundle, root, chain);
					let job = await startDetachedChainViaRegistry(ctx, chain, task, params.cwd, pi).catch(() => undefined);
					if (!job) {
						job = startDetachedChain(pi, ctx, bundle, chain, task, params.cwd);
					}
					const lines = [
						`Queued detached subagent job ${job.jobId}`,
						`mode: ${job.mode}`,
						`chain: ${job.chainName}`,
						`cwd: ${relative(root, job.cwd)}`,
						`step_count: ${job.chainStepCount}`,
					];
					if (bundle.validationErrors.length > 0) {
						lines.push("manifest_warnings:");
						for (const error of bundle.validationErrors) lines.push(`- ${error}`);
					}
					return { content: [{ type: "text", text: lines.join("\n") }], details: { action: params.action, job } };
				}
			}
		},
	});

	pi.registerCommand("subagent-agents", {
		description: "List canonical project-local AOC subagents, teams, and chains",
		handler: async (_args, ctx) => {
			await ensureInitialized(pi, ctx);
			ctx.ui.notify(formatAgentCatalog(loadManifestBundle(ctx.cwd ?? process.cwd()), ctx.cwd ?? process.cwd()), "info");
		},
	});

	pi.registerCommand("subagent-status", {
		description: "Show detached subagent status. Usage: /subagent-status [job-id]",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			await refreshRegistryJobs(ctx, args?.trim() || undefined, pi);
			ctx.ui.notify(formatStatusReport(args?.trim() || undefined), "info");
		},
	});

	pi.registerCommand("subagent-recent", {
		description: "Show recent detached subagent completions. Usage: /subagent-recent [count]",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			await refreshRegistryJobs(ctx, undefined, pi);
			const limit = Math.max(1, Math.min(10, Number.parseInt(args?.trim() || "5", 10) || 5));
			ctx.ui.notify(formatRecentJobs(limit), "info");
		},
	});

	pi.registerCommand("subagent-inspect", {
		description: "Inspect one detached subagent job with handoff summary. Usage: /subagent-inspect <job-id>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const jobId = args?.trim();
			if (!jobId) {
				ctx.ui.notify("Usage: /subagent-inspect <job-id>", "warning");
				return;
			}
			await refreshRegistryJobs(ctx, jobId, pi);
			const job = lookupJob(jobId);
			ctx.ui.notify(job ? `${formatJob(job)}\n\n${formatHandoff(jobId)}` : `Unknown detached subagent job: ${jobId}`, job ? "info" : "warning");
		},
	});

	pi.registerCommand("subagent-handoff", {
		description: "Show a concise handoff for one detached subagent job. Usage: /subagent-handoff <job-id>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const jobId = args?.trim();
			if (!jobId) {
				ctx.ui.notify("Usage: /subagent-handoff <job-id>", "warning");
				return;
			}
			await refreshRegistryJobs(ctx, jobId, pi);
			ctx.ui.notify(formatHandoff(jobId), "info");
		},
	});

	pi.registerCommand("subagent-cancel", {
		description: "Cancel a detached subagent job. Usage: /subagent-cancel <job-id>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const jobId = args?.trim();
			if (!jobId) {
				ctx.ui.notify("Usage: /subagent-cancel <job-id>", "warning");
				return;
			}
			const job = await cancelJob(pi, ctx, jobId);
			ctx.ui.notify(`Cancelled ${job.jobId} (${job.agent})`, "info");
		},
	});

	pi.registerCommand("subagent-run", {
		description: "Dispatch one detached project-local subagent. Usage: /subagent-run <agent> :: <task>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const raw = args?.trim() || "";
			const separator = raw.indexOf("::");
			if (separator < 0) {
				ctx.ui.notify("Usage: /subagent-run <agent> :: <task>", "warning");
				return;
			}
			const agent = raw.slice(0, separator).trim() || DEFAULT_AGENT;
			const task = raw.slice(separator + 2).trim();
			if (!task) {
				ctx.ui.notify("Task text is required after ::", "warning");
				return;
			}
			const bundle = loadManifestBundle(ctx.cwd ?? process.cwd());
			assertAgentAvailable(bundle, ctx.cwd ?? process.cwd(), agent);
			const job = (await startDetachedDispatchViaRegistry(ctx, agent, task, undefined, pi).catch(() => undefined))
				?? startDetachedDispatch(pi, ctx, bundle, agent, task);
			ctx.ui.notify(`Detached subagent queued: ${job.jobId} (${job.agent})`, "info");
		},
	});

	pi.registerCommand("subagent-chain", {
		description: "Dispatch one detached canonical chain. Usage: /subagent-chain <chain> :: <task>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const raw = args?.trim() || "";
			const separator = raw.indexOf("::");
			if (separator < 0) {
				ctx.ui.notify("Usage: /subagent-chain <chain> :: <task>", "warning");
				return;
			}
			const chain = raw.slice(0, separator).trim();
			const task = raw.slice(separator + 2).trim();
			if (!chain || !task) {
				ctx.ui.notify("Both chain and task are required.", "warning");
				return;
			}
			const bundle = loadManifestBundle(ctx.cwd ?? process.cwd());
			assertChainAvailable(bundle, ctx.cwd ?? process.cwd(), chain);
			const job = (await startDetachedChainViaRegistry(ctx, chain, task, undefined, pi).catch(() => undefined))
				?? startDetachedChain(pi, ctx, bundle, chain, task);
			ctx.ui.notify(`Detached chain queued: ${job.jobId} (${job.chainName})`, "info");
		},
	});

	registerFixedAgentCommand(pi, "subagent-explore", "explorer-agent", "Dispatch explorer-agent. Usage: /subagent-explore <task>");
	registerFixedAgentCommand(pi, "subagent-review", "code-review-agent", "Dispatch code-review-agent. Usage: /subagent-review <task>");
	registerFixedAgentCommand(pi, "subagent-test", "testing-agent", "Dispatch testing-agent. Usage: /subagent-test <task>");
	registerFixedAgentCommand(pi, "subagent-scout", "scout-web-agent", "Dispatch scout-web-agent. Usage: /subagent-scout <task>");
}
