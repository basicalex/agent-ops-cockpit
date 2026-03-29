import * as path from "node:path";

export type AgentConfig = {
	name: string;
	description?: string;
	tools: string[];
	model?: string;
	systemPrompt: string;
	sourcePath: string;
};

export type ChainStep = {
	agent: string;
	prompt?: string;
};

export type ChainDefinition = {
	description?: string;
	steps: ChainStep[];
};

export type ManifestBundle = {
	agents: AgentConfig[];
	teams: Record<string, string[]>;
	chains: Record<string, ChainDefinition>;
	validationErrors: string[];
	agentsDir: string;
};

export type AgentAvailability = {
	available: boolean;
	reason?: string;
};

export type JobStatus = "queued" | "running" | "success" | "fallback" | "error" | "cancelled" | "stale";
export type JobMode = "dispatch" | "chain" | "parallel";
export type ExecutionMode = "background" | "inline_wait" | "inline_summary";

export type ToolSourceInfo = {
	path: string;
	source: string;
	scope: "user" | "project" | "temporary";
	origin: "package" | "top-level";
	baseDir?: string;
};

export type ToolTrustTier = "builtin" | "project-local" | "sdk" | "external-extension" | "unknown";

export type ToolPolicyRecord = {
	name: string;
	trustTier: ToolTrustTier;
	allowed: boolean;
	reason?: string;
	sourceInfo?: ToolSourceInfo;
};

export type JobStepResult = {
	agent: string;
	status: JobStatus;
	outputExcerpt?: string;
	stderrExcerpt?: string;
	error?: string;
};

export type SpecialistRoleName = "scout" | "planner" | "builder" | "reviewer" | "documenter" | "red-team";

export type JobRecord = {
	jobId: string;
	mode: JobMode;
	executionMode: ExecutionMode;
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
	toolPolicies?: ToolPolicyRecord[];
	specialistRole?: SpecialistRoleName;
	writeApproved?: boolean;
	contextPackUsed?: boolean;
	outputExcerpt?: string;
	stderrExcerpt?: string;
	error?: string;
	fallbackUsed: boolean;
	manifestErrors: string[];
	teamName?: string;
	stepResults?: JobStepResult[];
	chainName?: string;
	chainStepIndex?: number;
	chainStepCount?: number;
	artifactDir?: string;
	reportPath?: string;
	metaPath?: string;
	eventsPath?: string;
	promptPath?: string;
	stderrPath?: string;
};

export type PersistedJobRecord = Omit<JobRecord, "pid">;

export type PersistedHandoffRecord = {
	jobId: string;
	status: JobStatus;
	agent: string;
	mode: JobMode;
	createdAt: number;
	finishedAt?: number;
	teamName?: string;
	chainName?: string;
	outputExcerpt?: string;
	stderrExcerpt?: string;
	error?: string;
	fallbackUsed: boolean;
	reportPath?: string;
	artifactDir?: string;
};

export const DEFAULT_AGENT = "insight-t1-observer";
export const DETACHED_STATUS_LIMIT = 24;
export const PULSE_COMMAND_TIMEOUT_MS = 3000;
export const MAX_OUTPUT_CHARS = 1200;
export const ARTIFACTS_DIR = path.join(".pi", "tmp", "subagents");
export const REPORT_FILENAME = "report.md";
export const META_FILENAME = "meta.json";
export const EVENTS_FILENAME = "events.jsonl";
export const PROMPT_FILENAME = "prompt.md";
export const STDERR_FILENAME = "stderr.log";
export const INLINE_WAIT_POLL_MS = 900;
export const INLINE_WAIT_TIMEOUT_MS = 45_000;

export function now(): number {
	return Date.now();
}

export function normalizeExecutionMode(value: string | undefined): ExecutionMode {
	switch ((value ?? "").trim().toLowerCase()) {
		case "inline_wait":
		case "wait":
			return "inline_wait";
		case "inline_summary":
		case "summary":
			return "inline_summary";
		default:
			return "background";
	}
}

export function nextExecutionMode(mode: ExecutionMode): ExecutionMode {
	switch (mode) {
		case "background":
			return "inline_wait";
		case "inline_wait":
			return "inline_summary";
		case "inline_summary":
		default:
			return "background";
	}
}

export function executionModeSummary(mode: ExecutionMode): string {
	switch (mode) {
		case "background":
			return "queue immediately";
		case "inline_wait":
			return "wait for terminal status";
		case "inline_summary":
			return "wait and return concise handoff";
	}
}

export function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

export function randomId(): string {
	return Math.random().toString(36).slice(2, 10);
}

export function sanitizeSlug(input: string): string {
	return input
		.toLowerCase()
		.replace(/[^a-z0-9._-]+/g, "-")
		.replace(/-+/g, "-")
		.replace(/^-|-$/g, "") || "agent";
}

export function makeJobId(agent: string): string {
	return `sj_${now().toString(36)}_${sanitizeSlug(agent)}_${randomId()}`;
}

export function truncate(text: string | undefined, max = MAX_OUTPUT_CHARS): string | undefined {
	if (!text) return undefined;
	if (text.length <= max) return text;
	return `${text.slice(0, Math.max(0, max - 1))}…`;
}

export function relative(root: string, target: string): string {
	const rel = path.relative(root, target);
	return rel && !rel.startsWith("..") ? rel : target;
}

export function normalizePathArg(raw: string): string {
	return raw.startsWith("@") ? raw.slice(1) : raw;
}

export function resolveScopedCwd(root: string, requested?: string): string {
	if (!requested || !requested.trim()) return root;
	const resolved = path.resolve(root, normalizePathArg(requested.trim()));
	const relativeToRoot = path.relative(root, resolved);
	if (relativeToRoot.startsWith("..") || path.isAbsolute(relativeToRoot)) {
		throw new Error(`cwd escapes project root: ${requested}`);
	}
	return resolved;
}

export function isWithinDir(root: string, target?: string): boolean {
	if (!target) return false;
	const resolvedRoot = path.resolve(root);
	const resolvedTarget = path.resolve(target);
	const rel = path.relative(resolvedRoot, resolvedTarget);
	return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}
