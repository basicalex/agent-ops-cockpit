import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import * as fs from "node:fs";
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

type JobStatus = "queued" | "running" | "success" | "fallback" | "error" | "cancelled" | "stale";

type JobMode = "dispatch" | "chain";

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

type RuntimeState = {
	initialized: boolean;
	ctx?: ExtensionContext;
	jobs: Map<string, JobRecord>;
	children: Map<string, ChildProcessWithoutNullStreams>;
};

const ENTRY_TYPE = "aoc-subagent-job-v1";
const WIDGET_ID = "aoc-subagent-jobs";
const STATUS_ID = "aoc-subagent";
const MAX_WIDGET_LINES = 6;
const MAX_OUTPUT_CHARS = 1200;
const DEFAULT_AGENT = "insight-t1-observer";

const state: RuntimeState = {
	initialized: false,
	jobs: new Map(),
	children: new Map(),
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

function activeJobs(): JobRecord[] {
	return sortJobs(state.jobs.values()).filter((job) => job.status === "queued" || job.status === "running");
}

function updateUi(ctx?: ExtensionContext): void {
	const runtimeCtx = ctx ?? state.ctx;
	if (!runtimeCtx?.ui) return;
	const active = activeJobs();
	if (active.length > 0) {
		runtimeCtx.ui.setStatus(STATUS_ID, runtimeCtx.ui.theme.fg("accent", `subagents:${active.length}`));
	} else {
		runtimeCtx.ui.setStatus(STATUS_ID, undefined);
	}

	const lines = sortJobs(state.jobs.values())
		.slice(0, MAX_WIDGET_LINES)
		.map((job) => `${statusIcon(job.status)} ${job.agent} ${job.jobId} · ${job.status}`);
	runtimeCtx.ui.setWidget(WIDGET_ID, lines.length > 0 ? lines : undefined, { placement: "belowEditor" });
}

function snapshotJob(job: JobRecord): PersistedJobRecord {
	const { pid: _pid, ...rest } = job;
	return rest;
}

function persistJob(pi: ExtensionAPI, job: JobRecord): void {
	pi.appendEntry<PersistedJobRecord>(ENTRY_TYPE, snapshotJob(job));
}

function restoreJobs(pi: ExtensionAPI, ctx: ExtensionContext): void {
	const entries = ctx.sessionManager.getEntries?.() ?? [];
	const restored = new Map<string, JobRecord>();
	let mutated = false;
	for (const entry of entries) {
		const rec = entry as any;
		if (rec?.type !== "custom" || rec?.customType !== ENTRY_TYPE || !rec?.data) continue;
		const data = rec.data as PersistedJobRecord;
		restored.set(data.jobId, { ...data });
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
	const jobs = sortJobs(state.jobs.values());
	if (jobs.length === 0) return "No detached subagent jobs recorded in this session.";
	if (targetJobId) {
		const job = state.jobs.get(targetJobId);
		if (!job) return `Unknown detached subagent job: ${targetJobId}`;
		return formatJob(job);
	}
	return jobs.map(formatJob).join("\n\n");
}

function formatAgentCatalog(bundle: ManifestBundle, root: string): string {
	const lines: string[] = [];
	lines.push(`Agents dir: ${relative(root, bundle.agentsDir)}`);
	if (bundle.agents.length === 0) {
		lines.push("No canonical project-local agents found.");
	} else {
		for (const agent of bundle.agents) {
			const desc = agent.description ? ` — ${agent.description}` : "";
			const tools = agent.tools.length > 0 ? ` [tools: ${agent.tools.join(",")}]` : "";
			lines.push(`- ${agent.name}${desc}${tools}`);
		}
	}
	if (Object.keys(bundle.teams).length > 0) {
		lines.push("", "Teams:");
		for (const [team, members] of Object.entries(bundle.teams)) {
			lines.push(`- ${team}: ${members.join(", ")}`);
		}
	}
	if (Object.keys(bundle.chains).length > 0) {
		lines.push("", "Chains:");
		for (const [name, def] of Object.entries(bundle.chains)) {
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
	const updated: JobRecord = { ...current, ...patch };
	state.jobs.set(jobId, updated);
	persistJob(pi, updated);
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

function cancelJob(pi: ExtensionAPI, ctx: ExtensionContext, jobId: string): JobRecord {
	const job = state.jobs.get(jobId);
	if (!job) throw new Error(`Unknown detached subagent job: ${jobId}`);
	const proc = state.children.get(jobId);
	if (!proc) {
		if (job.status === "queued" || job.status === "running") {
			finalizeJob(pi, ctx, jobId, {
				status: "stale",
				finishedAt: now(),
				error: "no live subprocess handle available for cancellation",
				fallbackUsed: true,
			});
		}
		return state.jobs.get(jobId)!;
	}
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

function ensureInitialized(pi: ExtensionAPI, ctx: ExtensionContext): void {
	state.ctx = ctx;
	if (!state.initialized) {
		restoreJobs(pi, ctx);
		state.initialized = true;
	}
	updateUi(ctx);
}

export default function aocSubagentExtension(pi: ExtensionAPI): void {
	pi.on("session_start", async (_event, ctx) => {
		ensureInitialized(pi, ctx);
	});

	pi.on("session_switch", async (_event, ctx) => {
		state.ctx = ctx;
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
			"Use action=dispatch_chain with a canonical chain name when the user asks for the predefined multi-step insight handoff flow.",
			"Use action=status to inspect detached subagent job state instead of guessing completion.",
			"Use action=cancel only when the user asks to stop a detached job.",
		],
		parameters: SubagentParams,
		async execute(_toolCallId, params, signal, _onUpdate, ctx) {
			ensureInitialized(pi, ctx);
			if (signal?.aborted) throw new Error("aoc_subagent aborted before execution");
			const root = ctx.cwd ?? process.cwd();
			const bundle = loadManifestBundle(root);
			switch (params.action) {
				case "list_agents": {
					const text = formatAgentCatalog(bundle, root);
					return { content: [{ type: "text", text }], details: { action: params.action } };
				}
				case "status": {
					const text = formatStatusReport(params.jobId);
					return { content: [{ type: "text", text }], details: { action: params.action, jobId: params.jobId } };
				}
				case "cancel": {
					if (!params.jobId) throw new Error("cancel requires jobId");
					const job = cancelJob(pi, ctx, params.jobId);
					return {
						content: [{ type: "text", text: `Cancelled ${job.jobId} (${job.agent}) -> ${job.status}` }],
						details: { action: params.action, job },
					};
				}
				case "dispatch": {
					const agent = params.agent?.trim() || DEFAULT_AGENT;
					const task = params.task?.trim();
					if (!task) throw new Error("dispatch requires task");
					const job = startDetachedDispatch(pi, ctx, bundle, agent, task, params.cwd);
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
					const job = startDetachedChain(pi, ctx, bundle, chain, task, params.cwd);
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
			ensureInitialized(pi, ctx);
			ctx.ui.notify(formatAgentCatalog(loadManifestBundle(ctx.cwd ?? process.cwd()), ctx.cwd ?? process.cwd()), "info");
		},
	});

	pi.registerCommand("subagent-status", {
		description: "Show detached subagent status. Usage: /subagent-status [job-id]",
		handler: async (args, ctx) => {
			ensureInitialized(pi, ctx);
			ctx.ui.notify(formatStatusReport(args?.trim() || undefined), "info");
		},
	});

	pi.registerCommand("subagent-cancel", {
		description: "Cancel a detached subagent job. Usage: /subagent-cancel <job-id>",
		handler: async (args, ctx) => {
			ensureInitialized(pi, ctx);
			const jobId = args?.trim();
			if (!jobId) {
				ctx.ui.notify("Usage: /subagent-cancel <job-id>", "warning");
				return;
			}
			const job = cancelJob(pi, ctx, jobId);
			ctx.ui.notify(`Cancelled ${job.jobId} (${job.agent})`, "info");
		},
	});

	pi.registerCommand("subagent-run", {
		description: "Dispatch one detached project-local subagent. Usage: /subagent-run <agent> :: <task>",
		handler: async (args, ctx) => {
			ensureInitialized(pi, ctx);
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
			const job = startDetachedDispatch(pi, ctx, bundle, agent, task);
			ctx.ui.notify(`Detached subagent queued: ${job.jobId} (${job.agent})`, "info");
		},
	});

	pi.registerCommand("subagent-chain", {
		description: "Dispatch one detached canonical chain. Usage: /subagent-chain <chain> :: <task>",
		handler: async (args, ctx) => {
			ensureInitialized(pi, ctx);
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
			const job = startDetachedChain(pi, ctx, bundle, chain, task);
			ctx.ui.notify(`Detached chain queued: ${job.jobId} (${job.chainName})`, "info");
		},
	});
}
