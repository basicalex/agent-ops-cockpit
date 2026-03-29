import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import type { ExtensionAPI, ExtensionContext, Theme } from "@mariozechner/pi-coding-agent";
import { StringEnum } from "@mariozechner/pi-ai";
import { Editor, type EditorTheme, Key, Text, matchesKey, truncateToWidth, visibleWidth } from "@mariozechner/pi-tui";
import { Type } from "@sinclair/typebox";
import { persistArtifactBundle as persistArtifactBundleImpl, type ArtifactPersistenceOptions } from "./subagent/artifacts.ts";
import { availableAgents, availableChains, availableTeams, assertAgentAvailable, assertChainAvailable, assertTeamAvailable, loadManifestBundle } from "./subagent/manifests.ts";
import {
	detachedRegistryAvailability,
	type DurableDetachedCancelResult,
	type DurableDetachedDispatchResult,
	type DurableDetachedStatusResult,
	fetchMindContextPack,
	mapDurableJob,
	type MindContextPackPayload,
	renderContextPackPrelude,
	sendPulseCommand,
} from "./subagent/registry.ts";
import {
	DEFAULT_AGENT,
	DETACHED_STATUS_LIMIT,
	executionModeSummary,
	INLINE_WAIT_POLL_MS,
	INLINE_WAIT_TIMEOUT_MS,
	isWithinDir,
	makeJobId,
	nextExecutionMode,
	normalizeExecutionMode,
	now,
	relative,
	resolveScopedCwd,
	sanitizeSlug,
	sleep,
	type AgentConfig,
	type ExecutionMode,
	type JobMode,
	type JobRecord,
	type JobStatus,
	type ManifestBundle,
	type PersistedHandoffRecord,
	type PersistedJobRecord,
	type SpecialistRoleName,
	type ToolPolicyRecord,
	type ToolSourceInfo,
	type ToolTrustTier,
	truncate,
} from "./subagent/shared.ts";

type InspectorAgentEntry = {
	agent: string;
	jobs: JobRecord[];
};

type RuntimeState = {
	initialized: boolean;
	ctx?: ExtensionContext;
	jobs: Map<string, JobRecord>;
	registryJobs: Map<string, JobRecord>;
	children: Map<string, ChildProcessWithoutNullStreams>;
	handoffNotified: Set<string>;
	inspectorOpen: boolean;
	inspectorClose?: () => void;
	inspectorRequestRender?: () => void;
	inspectorRefreshPending: boolean;
	inspectorRefreshNote?: string;
};

const ENTRY_TYPE = "aoc-subagent-job-v1";
const HANDOFF_ENTRY_TYPE = "aoc-subagent-handoff-v1";
const WIDGET_ID = "aoc-subagent-jobs";
const STATUS_ID = "aoc-subagent";
const MAX_WIDGET_LINES = 6;

const state: RuntimeState = {
	initialized: false,
	jobs: new Map(),
	registryJobs: new Map(),
	children: new Map(),
	handoffNotified: new Set(),
	inspectorOpen: false,
	inspectorRefreshPending: false,
};

const ActionSchema = StringEnum(["dispatch", "dispatch_chain", "dispatch_team", "status", "cancel", "list_agents"] as const, {
	description: "Action to perform for the AOC subagent runtime.",
});

const ExecutionModeSchema = StringEnum(["background", "inline_wait", "inline_summary"] as const, {
	description: "Operator-facing execution behavior for detached delegated runs.",
});

const SubagentParams = Type.Object({
	action: ActionSchema,
	agent: Type.Optional(Type.String({ description: "Canonical agent name from .pi/agents/*.md." })),
	team: Type.Optional(Type.String({ description: "Canonical team name from .pi/agents/teams.yaml." })),
	chain: Type.Optional(Type.String({ description: "Canonical chain name from .pi/agents/agent-chain.yaml." })),
	task: Type.Optional(Type.String({ description: "Task prompt for detached dispatch, team fanout, or chain input." })),
	jobId: Type.Optional(Type.String({ description: "Detached subagent job id for status/cancel actions." })),
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
	executionMode: Type.Optional(ExecutionModeSchema),
	sessionMode: Type.Optional(Type.String({ description: "Reserved session mode request. Only fresh detached mode is currently supported." })),
});

const SpecialistRoleSchema = StringEnum(["scout", "planner", "builder", "reviewer", "documenter", "red-team"] as const, {
	description: "Canonical specialist role name.",
});

const SpecialistRoleActionSchema = StringEnum(["dispatch", "status", "cancel", "list_roles"] as const, {
	description: "Action to perform for the specialist role runtime.",
});

const SpecialistRoleParams = Type.Object({
	action: SpecialistRoleActionSchema,
	role: Type.Optional(SpecialistRoleSchema),
	task: Type.Optional(Type.String({ description: "Task prompt for explicit specialist role dispatch." })),
	jobId: Type.Optional(Type.String({ description: "Detached job id for status/cancel actions." })),
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
	executionMode: Type.Optional(ExecutionModeSchema),
	sessionMode: Type.Optional(Type.String({ description: "Reserved session mode request. Only fresh detached mode is currently supported." })),
	approveWrite: Type.Optional(Type.Boolean({ description: "Required when a role run is allowed to proceed on a write/destructive request." })),
});

type SpecialistRoleConfig = {
	role: SpecialistRoleName;
	label: string;
	agent: string;
	description: string;
	allowedTrustTiers: ToolTrustTier[];
	requiresWriteApproval: boolean;
};

const SPECIALIST_ROLES: Record<SpecialistRoleName, SpecialistRoleConfig> = {
	scout: {
		role: "scout",
		label: "Scout",
		agent: "explorer-agent",
		description: "Repo reconnaissance and fast scope mapping.",
		allowedTrustTiers: ["builtin", "project-local"],
		requiresWriteApproval: false,
	},
	planner: {
		role: "planner",
		label: "Planner",
		agent: "planner-agent",
		description: "Execution planning, sequencing, and rollout framing.",
		allowedTrustTiers: ["builtin", "project-local"],
		requiresWriteApproval: false,
	},
	builder: {
		role: "builder",
		label: "Builder",
		agent: "builder-agent",
		description: "Implementation-shape analysis with explicit write approval gates.",
		allowedTrustTiers: ["builtin", "project-local"],
		requiresWriteApproval: true,
	},
	reviewer: {
		role: "reviewer",
		label: "Reviewer",
		agent: "code-review-agent",
		description: "Bounded correctness/regression review.",
		allowedTrustTiers: ["builtin", "project-local"],
		requiresWriteApproval: false,
	},
	documenter: {
		role: "documenter",
		label: "Documenter",
		agent: "documenter-agent",
		description: "Repo-grounded documentation and operator-note guidance.",
		allowedTrustTiers: ["builtin", "project-local"],
		requiresWriteApproval: false,
	},
	"red-team": {
		role: "red-team",
		label: "Red Team",
		agent: "red-team-agent",
		description: "Adversarial review of risks, abuse cases, and trust boundaries.",
		allowedTrustTiers: ["builtin", "project-local"],
		requiresWriteApproval: true,
	},
};

function persistArtifactBundle(root: string, job: JobRecord, options?: ArtifactPersistenceOptions): JobRecord {
	return persistArtifactBundleImpl(root, job, { snapshotJob, summarizeToolPolicies }, options);
}

function allowExternalExtensionTools(): boolean {
	const value = process.env.AOC_SUBAGENT_ALLOW_EXTERNAL_EXTENSION_TOOLS?.trim()?.toLowerCase();
	return value === "1" || value === "true" || value === "yes";
}

function classifyToolPolicy(root: string, name: string, sourceInfo?: ToolSourceInfo): ToolPolicyRecord {
	if (!sourceInfo) {
		return {
			name,
			trustTier: "unknown",
			allowed: false,
			reason: "tool is not registered in the current Pi runtime",
		};
	}
	if (sourceInfo.source === "builtin") {
		return { name, trustTier: "builtin", allowed: true, sourceInfo };
	}
	if (sourceInfo.source === "sdk") {
		return {
			name,
			trustTier: "sdk",
			allowed: false,
			reason: "sdk-injected tools are not allowed for detached canonical subagents by default",
			sourceInfo,
		};
	}
	if (sourceInfo.scope === "project" || isWithinDir(root, sourceInfo.baseDir) || isWithinDir(root, sourceInfo.path)) {
		return { name, trustTier: "project-local", allowed: true, sourceInfo };
	}
	if (allowExternalExtensionTools()) {
		return {
			name,
			trustTier: "external-extension",
			allowed: true,
			reason: "allowed by AOC_SUBAGENT_ALLOW_EXTERNAL_EXTENSION_TOOLS",
			sourceInfo,
		};
	}
	return {
		name,
		trustTier: "external-extension",
		allowed: false,
		reason: "only builtin or project-local tools are allowed for detached canonical subagents",
		sourceInfo,
	};
}

function resolveToolPolicies(pi: ExtensionAPI, root: string, toolNames: string[]): ToolPolicyRecord[] {
	const allTools = (pi.getAllTools?.() ?? []) as any[];
	const toolsByName = new Map(allTools.map((tool) => [String(tool?.name), tool]));
	return toolNames.map((name) => {
		const tool = toolsByName.get(name) as any;
		return classifyToolPolicy(root, name, tool?.sourceInfo as ToolSourceInfo | undefined);
	});
}

function assertAllowedToolPolicies(toolPolicies: ToolPolicyRecord[], agentName: string): void {
	const blocked = toolPolicies.filter((policy) => !policy.allowed);
	if (blocked.length === 0) return;
	throw new Error(
		`Detached agent ${agentName} references blocked tools: ${blocked.map((policy) => `${policy.name} (${policy.reason ?? policy.trustTier})`).join(", ")}`,
	);
}

function summarizeToolPolicies(toolPolicies: ToolPolicyRecord[] | undefined): string | undefined {
	if (!toolPolicies || toolPolicies.length === 0) return undefined;
	return toolPolicies
		.map((policy) => `${policy.name}:${policy.trustTier}${policy.allowed ? "" : "!"}`)
		.join(", ");
}

function specialistRoleLabel(job: JobRecord): string | undefined {
	return job.specialistRole ? SPECIALIST_ROLES[job.specialistRole]?.label : undefined;
}

function getSpecialistRole(role: string | undefined): SpecialistRoleConfig {
	const normalized = (role?.trim().toLowerCase() || "") as SpecialistRoleName;
	const config = SPECIALIST_ROLES[normalized];
	if (!config) {
		throw new Error(`Unknown specialist role: ${role}. Available: ${Object.keys(SPECIALIST_ROLES).join(", ")}`);
	}
	return config;
}

function taskLooksWriteLike(task: string): boolean {
	return /\b(edit|modify|change|update|patch|write|rewrite|delete|remove|refactor|implement|fix)\b/i.test(task);
}

function taskLooksDestructive(task: string): boolean {
	return /\b(drop|destroy|purge|reset|truncate|force[- ]?push|rm\s+-rf|delete data)\b/i.test(task);
}

function enforceRoleApproval(role: SpecialistRoleConfig, task: string, approveWrite?: boolean): void {
	if (!role.requiresWriteApproval) return;
	if (!taskLooksWriteLike(task) && !taskLooksDestructive(task)) return;
	if (approveWrite) return;
	throw new Error(
		`${role.label} role requires approveWrite=true for write/destructive requests; dispatch remains explicit and developer-controlled`,
	);
}

const MAX_SUBAGENT_NESTING_DEPTH = 1;

function currentSubagentNestingDepth(): number {
	const raw = process.env.AOC_SUBAGENT_DEPTH?.trim();
	const parsed = Number.parseInt(raw || "", 10);
	if (Number.isFinite(parsed) && parsed >= 0) return parsed;
	return process.env.AOC_SUBAGENT_JOB_ID?.trim() ? 1 : 0;
}

function currentDelegatedParentJobId(): string | undefined {
	const value = process.env.AOC_SUBAGENT_JOB_ID?.trim();
	return value || undefined;
}

function assertSupportedSessionMode(sessionMode: string | undefined): void {
	const normalized = (sessionMode ?? "").trim().toLowerCase();
	if (!normalized || normalized === "fresh" || normalized === "detached" || normalized === "fresh_detached") return;
	throw new Error(`Unsupported delegated session mode: ${sessionMode}. Detached delegated subagents currently only support fresh detached session mode.`);
}

function assertNoUnsupportedSessionModeFlags(raw: string): void {
	const match = raw.match(/--(?:session-mode|reuse-session|inherit-session|same-session|shared-session)(?:[=\s]+([^\s]+))?/i);
	if (!match) return;
	const requested = match[1] ?? match[0];
	throw new Error(`Unsupported delegated session mode request: ${requested}. Detached delegated subagents currently only support fresh detached session mode.`);
}

function assertDelegatedRecursionAllowed(): void {
	const depth = currentSubagentNestingDepth();
	if (depth < MAX_SUBAGENT_NESTING_DEPTH) return;
	const parentJobId = currentDelegatedParentJobId();
	throw new Error(
		`Nested delegated subagent dispatch is blocked inside detached delegated runs${parentJobId ? ` (parent_job=${parentJobId})` : ""}; hand off to the parent session or use Mission Control for supervision.`,
	);
}

function assertDispatchGuardrails(sessionMode?: string): void {
	assertSupportedSessionMode(sessionMode);
	assertDelegatedRecursionAllowed();
}

function assertRoleToolPolicies(toolPolicies: ToolPolicyRecord[], role: SpecialistRoleConfig): void {
	assertAllowedToolPolicies(toolPolicies, role.agent);
	const blocked = toolPolicies.filter((policy) => !role.allowedTrustTiers.includes(policy.trustTier));
	if (blocked.length === 0) return;
	throw new Error(
		`${role.label} role references tools outside its trust policy: ${blocked.map((policy) => `${policy.name}:${policy.trustTier}`).join(", ")}`,
	);
}

function formatRoleCatalog(): string {
	return Object.values(SPECIALIST_ROLES)
		.map((role) => {
			const approval = role.requiresWriteApproval ? "write-approval" : "read-first";
			return `${role.role} -> ${role.agent} [${approval}] trust=${role.allowedTrustTiers.join("/")}\n  ${role.description}`;
		})
		.join("\n\n");
}

function requestInspectorRender(): void {
	state.inspectorRequestRender?.();
}

function setInspectorRefreshState(pending: boolean, note?: string): void {
	state.inspectorRefreshPending = pending;
	state.inspectorRefreshNote = note;
	requestInspectorRender();
}

async function refreshRegistryJobs(ctx: ExtensionContext, targetJobId?: string, pi?: ExtensionAPI): Promise<boolean> {
	const root = ctx.cwd ?? process.cwd();
	try {
		const result = await sendPulseCommand("insight_detached_status", {
			job_id: targetJobId,
			owner_plane: "delegated",
			limit: DETACHED_STATUS_LIMIT,
		});
		if (result.status !== "ok") return false;
		const payload = result.message ? (JSON.parse(result.message) as DurableDetachedStatusResult) : undefined;
		const previous = new Map(state.registryJobs);
		const next = new Map<string, JobRecord>();
		for (const job of payload?.jobs ?? []) {
			const mapped = mapDurableJob(job, root);
			const prior = previous.get(job.job_id) ?? state.jobs.get(job.job_id);
			if (prior?.task && !mapped.task) mapped.task = prior.task;
			if (prior?.cwd && mapped.cwd === root) mapped.cwd = prior.cwd;
			if (prior?.model && !mapped.model) mapped.model = prior.model;
			if (prior?.toolPolicies && !mapped.toolPolicies) mapped.toolPolicies = prior.toolPolicies;
			if (prior?.tools?.length && mapped.tools.length === 0) mapped.tools = prior.tools;
			if (prior?.specialistRole && !mapped.specialistRole) mapped.specialistRole = prior.specialistRole;
			if (prior?.executionMode) mapped.executionMode = prior.executionMode;
			if (typeof prior?.writeApproved === "boolean" && typeof mapped.writeApproved !== "boolean") mapped.writeApproved = prior.writeApproved;
			if (typeof prior?.contextPackUsed === "boolean" && typeof mapped.contextPackUsed !== "boolean") mapped.contextPackUsed = prior.contextPackUsed;
			if (prior?.artifactDir && !mapped.artifactDir) mapped.artifactDir = prior.artifactDir;
			if (prior?.stepResults?.length && !mapped.stepResults?.length) mapped.stepResults = prior.stepResults;
			if (prior?.reportPath && !mapped.reportPath) mapped.reportPath = prior.reportPath;
			if (prior?.metaPath && !mapped.metaPath) mapped.metaPath = prior.metaPath;
			if (prior?.eventsPath && !mapped.eventsPath) mapped.eventsPath = prior.eventsPath;
			if (prior?.promptPath && !mapped.promptPath) mapped.promptPath = prior.promptPath;
			if (prior?.stderrPath && !mapped.stderrPath) mapped.stderrPath = prior.stderrPath;
			next.set(job.job_id, mapped);
		}
		if (targetJobId) {
			const merged = new Map(state.registryJobs);
			for (const [jobId, job] of next.entries()) merged.set(jobId, job);
			state.registryJobs = merged;
		} else {
			state.registryJobs = next;
		}
		for (const [jobId, job] of state.registryJobs.entries()) {
			const enriched = persistArtifactBundle(root, job);
			state.registryJobs.set(jobId, enriched);
			const prior = previous.get(jobId);
			if (!isTerminalJobStatus(enriched.status) || prior?.status === enriched.status) continue;
			if (pi) {
				maybeNotifyHandoff(pi, ctx, enriched);
			} else if (ctx.ui && !state.handoffNotified.has(jobId)) {
				ctx.ui.notify(
					`${statusIcon(enriched.status)} ${enriched.agent} finished (${enriched.jobId}) — /subagent-inspect ${enriched.jobId}`,
					enriched.status === "success" ? "info" : "warning",
				);
			}
		}
		updateUi(ctx);
		return true;
	} catch {
		// fail open when no pulse socket / wrapper runtime is reachable
		return false;
	}
}

function refreshRegistryJobsInBackground(pi: ExtensionAPI, ctx: ExtensionContext): void {
	const availability = detachedRegistryAvailability();
	if (!availability.available) {
		setInspectorRefreshState(false, availability.note);
		return;
	}
	setInspectorRefreshState(true, "refreshing detached status…");
	void refreshRegistryJobs(ctx, undefined, pi)
		.then((ok) => {
			setInspectorRefreshState(false, ok ? undefined : "detached registry unavailable; showing cached jobs");
		})
		.catch(() => {
			setInspectorRefreshState(false, "detached registry unavailable; showing cached jobs");
		});
}

async function startDetachedDispatchViaRegistry(
	ctx: ExtensionContext,
	agentName: string,
	task: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
	pi?: ExtensionAPI,
): Promise<JobRecord | undefined> {
	const root = ctx.cwd ?? process.cwd();
	if (pi) {
		const bundle = loadManifestBundle(root);
		const agent = bundle.agents.find((candidate) => candidate.name === agentName);
		if (agent) {
			const toolPolicies = resolveToolPolicies(pi, root, agent.tools);
			assertAllowedToolPolicies(toolPolicies, agent.name);
		}
	}
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
	let job = mapDurableJob(payload.job, root);
	job.task = task;
	job.cwd = cwd;
	job.executionMode = executionMode;
	if (pi) {
		const bundle = loadManifestBundle(root);
		const agent = bundle.agents.find((candidate) => candidate.name === agentName);
		if (agent) {
			job.tools = agent.tools;
			job.toolPolicies = resolveToolPolicies(pi, root, agent.tools);
			job.model = agent.model;
			job = persistArtifactBundle(root, job, { prompt: task, agent });
		} else {
			job = persistArtifactBundle(root, job, { prompt: task });
		}
	} else {
		job = persistArtifactBundle(root, job, { prompt: task });
	}
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
	executionMode: ExecutionMode = "background",
	pi?: ExtensionAPI,
): Promise<JobRecord | undefined> {
	const root = ctx.cwd ?? process.cwd();
	if (pi) {
		const bundle = loadManifestBundle(root);
		const chain = bundle.chains[chainName];
		for (const step of chain?.steps ?? []) {
			const agent = bundle.agents.find((candidate) => candidate.name === step.agent);
			if (!agent) continue;
			const toolPolicies = resolveToolPolicies(pi, root, agent.tools);
			assertAllowedToolPolicies(toolPolicies, agent.name);
		}
	}
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
	let job = mapDurableJob(payload.job, root);
	job.task = task;
	job.cwd = cwd;
	job.executionMode = executionMode;
	if (pi) {
		const bundle = loadManifestBundle(root);
		const firstAgent = bundle.chains[chainName]?.steps?.[0]?.agent;
		const agent = bundle.agents.find((candidate) => candidate.name === firstAgent);
		if (agent) {
			job.tools = agent.tools;
			job.toolPolicies = resolveToolPolicies(pi, root, agent.tools);
			job.model = agent.model;
			job = persistArtifactBundle(root, job, { prompt: task, agent });
		} else {
			job = persistArtifactBundle(root, job, { prompt: task });
		}
	} else {
		job = persistArtifactBundle(root, job, { prompt: task });
	}
	state.registryJobs.set(job.jobId, job);
	updateUi(ctx);
	await refreshRegistryJobs(ctx, job.jobId, pi);
	return state.registryJobs.get(job.jobId) ?? job;
}

async function startDetachedTeamViaRegistry(
	ctx: ExtensionContext,
	teamName: string,
	task: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
	pi?: ExtensionAPI,
): Promise<JobRecord | undefined> {
	const root = ctx.cwd ?? process.cwd();
	const bundle = loadManifestBundle(root);
	const members = bundle.teams[teamName] ?? [];
	if (pi) {
		for (const member of members) {
			const agent = bundle.agents.find((candidate) => candidate.name === member);
			if (!agent) continue;
			const toolPolicies = resolveToolPolicies(pi, root, agent.tools);
			assertAllowedToolPolicies(toolPolicies, agent.name);
		}
	}
	const cwd = resolveScopedCwd(root, cwdArg);
	const result = await sendPulseCommand("insight_detached_dispatch", {
		mode: "parallel",
		owner_plane: "delegated",
		worker_kind: "team_fanout",
		team: teamName,
		input: task,
		cwd: relative(root, cwd),
		reason: "pi_subagent_extension",
	});
	if (result.status !== "ok" || !result.message) return undefined;
	const payload = JSON.parse(result.message) as DurableDetachedDispatchResult;
	if (!payload.job) return undefined;
	let job = mapDurableJob(payload.job, root);
	job.task = task;
	job.cwd = cwd;
	job.executionMode = executionMode;
	job.teamName = teamName;
	if (pi) {
		const toolPolicies = members.flatMap((member) => {
			const agent = bundle.agents.find((candidate) => candidate.name === member);
			return agent ? resolveToolPolicies(pi, root, agent.tools) : [];
		});
		job.tools = Array.from(new Set(members.flatMap((member) => bundle.agents.find((candidate) => candidate.name === member)?.tools ?? [])));
		job.toolPolicies = Array.from(new Map(toolPolicies.map((policy) => [policy.name, policy])).values());
		job = persistArtifactBundle(root, job, { prompt: task });
	} else {
		job = persistArtifactBundle(root, job, { prompt: task });
	}
	state.registryJobs.set(job.jobId, job);
	updateUi(ctx);
	await refreshRegistryJobs(ctx, job.jobId, pi);
	return state.registryJobs.get(job.jobId) ?? job;
}

async function launchAgentJob(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	agentName: string,
	task: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
	bundle?: ManifestBundle,
): Promise<JobRecord> {
	assertDispatchGuardrails();
	const root = ctx.cwd ?? process.cwd();
	const manifestBundle = bundle ?? loadManifestBundle(root);
	assertAgentAvailable(manifestBundle, root, agentName);
	return (await startDetachedDispatchViaRegistry(ctx, agentName, task, cwdArg, executionMode, pi).catch(() => undefined))
		?? startDetachedDispatch(pi, ctx, manifestBundle, agentName, task, cwdArg, executionMode);
}

async function launchTeamJob(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	teamName: string,
	task: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
	bundle?: ManifestBundle,
): Promise<JobRecord> {
	assertDispatchGuardrails();
	const root = ctx.cwd ?? process.cwd();
	const manifestBundle = bundle ?? loadManifestBundle(root);
	assertTeamAvailable(manifestBundle, root, teamName);
	return (await startDetachedTeamViaRegistry(ctx, teamName, task, cwdArg, executionMode, pi).catch(() => undefined))
		?? startDetachedTeam(pi, ctx, manifestBundle, teamName, task, cwdArg, executionMode);
}

async function launchChainJob(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	chainName: string,
	task: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
	bundle?: ManifestBundle,
): Promise<JobRecord> {
	assertDispatchGuardrails();
	const root = ctx.cwd ?? process.cwd();
	const manifestBundle = bundle ?? loadManifestBundle(root);
	assertChainAvailable(manifestBundle, root, chainName);
	return (await startDetachedChainViaRegistry(ctx, chainName, task, cwdArg, executionMode, pi).catch(() => undefined))
		?? startDetachedChain(pi, ctx, manifestBundle, chainName, task, cwdArg, executionMode);
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

function needsAttentionStatus(status: JobStatus): boolean {
	return status === "fallback" || status === "error" || status === "cancelled" || status === "stale";
}

function historyJobs(limit = 12): JobRecord[] {
	return combinedJobs().filter((job) => isTerminalJobStatus(job.status)).slice(0, limit);
}

function recentJobs(limit = 6): JobRecord[] {
	return historyJobs(limit);
}

function failureJobs(limit = 5): JobRecord[] {
	return historyJobs(Math.max(limit, 24)).filter((job) => needsAttentionStatus(job.status)).slice(0, limit);
}

function summarizeStepResults(job: JobRecord): string | undefined {
	if (!job.stepResults?.length) return undefined;
	const successCount = job.stepResults.filter((step) => step.status === "success").length;
	const cancelledCount = job.stepResults.filter((step) => step.status === "cancelled").length;
	const fallbackCount = job.stepResults.length - successCount - cancelledCount;
	return `members=${job.stepResults.length} success=${successCount} fallback=${fallbackCount} cancelled=${cancelledCount}`;
}

function formatStepResultLines(job: JobRecord, prefix = "  ", limit = 6): string[] {
	if (!job.stepResults?.length) return [];
	const lines = [
		`${prefix}step_results: ${job.stepResults.length}`,
		...job.stepResults.slice(0, limit).map((step, index) => {
			const detail = truncate(step.outputExcerpt || step.error || step.stderrExcerpt || "no excerpt recorded", 120) || "no excerpt recorded";
			return `${prefix}- ${index + 1}. ${step.agent} · ${step.status} · ${detail}`;
		}),
	];
	if (job.stepResults.length > limit) lines.push(`${prefix}- … ${job.stepResults.length - limit} more`);
	return lines;
}

function summarizeJobOutcome(job: JobRecord): string {
	const stepSummary = summarizeStepResults(job);
	if (stepSummary) return stepSummary;
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
		teamName: job.teamName,
		chainName: job.chainName,
		outputExcerpt: job.outputExcerpt,
		stderrExcerpt: job.stderrExcerpt,
		error: job.error,
		fallbackUsed: job.fallbackUsed,
		reportPath: job.reportPath,
		artifactDir: job.artifactDir,
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
	const failures = failureJobs(3);
	const failureCount = combinedJobs().filter((job) => needsAttentionStatus(job.status)).length;
	if (active.length > 0) {
		const roleRuns = active.filter((job) => Boolean(job.specialistRole)).length;
		const roleSuffix = roleRuns > 0 ? ` · roles:${roleRuns}` : "";
		const recentSuffix = recent.length > 0 ? ` · recent:${recent.length}` : "";
		const failureSuffix = failureCount > 0 ? ` · fail:${Math.min(99, failureCount)}` : "";
		runtimeCtx.ui.setStatus(STATUS_ID, runtimeCtx.ui.theme.fg("accent", `subagents:${active.length}${roleSuffix}${recentSuffix}${failureSuffix}`));
	} else if (recent.length > 0 || failureCount > 0) {
		const failureSuffix = failureCount > 0 ? ` · fail:${Math.min(99, failureCount)}` : "";
		runtimeCtx.ui.setStatus(STATUS_ID, runtimeCtx.ui.theme.fg("muted", `subagents recent:${recent.length}${failureSuffix}`));
	} else {
		runtimeCtx.ui.setStatus(STATUS_ID, undefined);
	}

	const lines: string[] = [];
	if (active.length > 0) {
		lines.push("Active:");
		for (const job of active.slice(0, Math.max(1, MAX_WIDGET_LINES - 2))) {
			const label = specialistRoleLabel(job) ?? job.agent;
			const telemetry = [job.status, job.executionMode !== "background" ? job.executionMode : undefined, job.contextPackUsed ? "ctx" : undefined, job.writeApproved ? "approved" : undefined]
				.filter(Boolean)
				.join("/");
			lines.push(`${statusIcon(job.status)} ${label} ${job.jobId} · ${telemetry}`);
		}
	}
	if (failures.length > 0 && lines.length < MAX_WIDGET_LINES) {
		lines.push("Attention:");
		for (const job of failures) {
			if (lines.length >= MAX_WIDGET_LINES) break;
			const label = specialistRoleLabel(job) ?? job.agent;
			lines.push(`${statusIcon(job.status)} ${label} ${job.jobId} · ${summarizeJobOutcome(job)}`);
		}
	}
	if (recent.length > 0 && lines.length < MAX_WIDGET_LINES) {
		lines.push("Recent:");
		for (const job of recent) {
			if (lines.length >= MAX_WIDGET_LINES) break;
			const label = specialistRoleLabel(job) ?? job.agent;
			lines.push(`${statusIcon(job.status)} ${label} ${job.jobId} · ${summarizeJobOutcome(job)}`);
		}
	}
	runtimeCtx.ui.setWidget(WIDGET_ID, lines.length > 0 ? lines : undefined, { placement: "belowEditor" });
	requestInspectorRender();
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
			restored.set(data.jobId, { ...data, executionMode: normalizeExecutionMode((data as { executionMode?: string }).executionMode) });
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
		`  execution_mode: ${job.executionMode}`,
		`  agent: ${job.agent}`,
		`  status: ${job.status}`,
		`  cwd: ${job.cwd}`,
		`  created_at: ${new Date(job.createdAt).toISOString()}`,
	];
	if (job.specialistRole) lines.push(`  specialist_role: ${job.specialistRole}`);
	if (typeof job.contextPackUsed === "boolean") lines.push(`  context_pack: ${job.contextPackUsed ? "mind-v2-attached" : "unavailable"}`);
	if (typeof job.writeApproved === "boolean") lines.push(`  write_approval: ${job.writeApproved ? "approved" : "read-first"}`);
	if (job.teamName) lines.push(`  team: ${job.teamName}`);
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
	const toolSummary = summarizeToolPolicies(job.toolPolicies);
	if (toolSummary) lines.push(`  tool_provenance: ${toolSummary}`);
	if (job.artifactDir) lines.push(`  artifacts: ${job.artifactDir}`);
	if (job.reportPath) lines.push(`  report: ${job.reportPath}`);
	if (job.metaPath) lines.push(`  meta: ${job.metaPath}`);
	if (job.eventsPath) lines.push(`  events: ${job.eventsPath}`);
	if (job.promptPath) lines.push(`  prompt: ${job.promptPath}`);
	if (job.stderrPath) lines.push(`  stderr_log: ${job.stderrPath}`);
	if (job.error) lines.push(`  error: ${job.error}`);
	if (job.outputExcerpt) lines.push(`  output: ${JSON.stringify(job.outputExcerpt)}`);
	if (job.stderrExcerpt) lines.push(`  stderr: ${JSON.stringify(job.stderrExcerpt)}`);
	lines.push(...formatStepResultLines(job));
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
	const active = activeJobs().length;
	const terminalCount = combinedJobs().filter((job) => isTerminalJobStatus(job.status)).length;
	const attentionCount = combinedJobs().filter((job) => needsAttentionStatus(job.status)).length;
	const summary = `Summary: active=${active} terminal=${terminalCount} attention=${attentionCount}`;
	return [summary, jobs.map(formatJob).join("\n\n"), "Recent history:", formatRecentJobs(6), "Recent failures:", formatFailureJobs(3)].join("\n\n");
}

function lookupJob(jobId: string): JobRecord | undefined {
	return combinedJobs().find((candidate) => candidate.jobId === jobId);
}

function formatRecentJobs(limit = 5): string {
	const jobs = historyJobs(limit);
	if (jobs.length === 0) return "No detached subagent run history yet.";
	return jobs
		.map((job) => {
			const lines = [`${statusIcon(job.status)} ${job.jobId} · ${specialistRoleLabel(job) ?? job.agent} · ${job.status} · ${inspectorTime(job)}`, `  ${summarizeJobOutcome(job)}`];
			if (job.reportPath) lines.push(`  report: ${job.reportPath}`);
			return lines.join("\n");
		})
		.join("\n");
}

function formatFailureJobs(limit = 5): string {
	const jobs = failureJobs(limit);
	if (jobs.length === 0) return "No recent detached subagent failures needing attention.";
	return jobs
		.map((job) => {
			const lines = [`${statusIcon(job.status)} ${job.jobId} · ${specialistRoleLabel(job) ?? job.agent} · ${job.status} · ${inspectorTime(job)}`, `  ${summarizeJobOutcome(job)}`];
			if (job.reportPath) lines.push(`  report: ${job.reportPath}`);
			if (job.stderrPath) lines.push(`  stderr_log: ${job.stderrPath}`);
			return lines.join("\n");
		})
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
		`execution_mode: ${job.executionMode}`,
		`status: ${job.status}`,
		`fallback_used: ${job.fallbackUsed ? "yes" : "no"}`,
	];
	if (job.specialistRole) lines.push(`specialist_role: ${job.specialistRole}`);
	if (typeof job.contextPackUsed === "boolean") lines.push(`context_pack: ${job.contextPackUsed ? "mind-v2-attached" : "unavailable"}`);
	if (typeof job.writeApproved === "boolean") lines.push(`write_approval: ${job.writeApproved ? "approved" : "read-first"}`);
	const toolSummary = summarizeToolPolicies(job.toolPolicies);
	if (toolSummary) lines.push(`tool_provenance: ${toolSummary}`);
	if (job.teamName) lines.push(`team: ${job.teamName}`);
	if (job.chainName) lines.push(`chain: ${job.chainName}`);
	const stepSummary = summarizeStepResults(job);
	if (stepSummary) lines.push(`step_summary: ${stepSummary}`);
	if (job.finishedAt) lines.push(`finished_at: ${new Date(job.finishedAt).toISOString()}`);
	if (job.artifactDir) lines.push(`artifacts: ${job.artifactDir}`);
	if (job.reportPath) lines.push(`report: ${job.reportPath}`);
	if (job.outputExcerpt) lines.push(`result: ${job.outputExcerpt}`);
	if (job.error) lines.push(`error: ${job.error}`);
	if (job.stderrExcerpt) lines.push(`stderr: ${job.stderrExcerpt}`);
	lines.push(...formatStepResultLines(job, "", 5));
	lines.push(`next_action: review with /subagent-inspect ${job.jobId}`);
	return lines.join("\n");
}

function renderSpecialistRoleCall(args: Record<string, any>, theme: Theme): Text {
	const action = typeof args.action === "string" ? args.action : "dispatch";
	const role = typeof args.role === "string" ? args.role : "scout";
	const cwd = typeof args.cwd === "string" && args.cwd.trim() ? ` @${args.cwd.trim()}` : "";
	const executionMode = normalizeExecutionMode(typeof args.executionMode === "string" ? args.executionMode : undefined);
	let text = theme.fg("toolTitle", theme.bold("aoc_specialist_role ")) + theme.fg("muted", action);
	text += " " + theme.fg("accent", role);
	if (cwd) text += " " + theme.fg("dim", cwd);
	if (executionMode !== "background") text += " " + theme.fg("dim", `[${executionMode}]`);
	return new Text(text, 0, 0);
}

function renderToolCall(args: Record<string, any>, theme: Theme): Text {
	const action = typeof args.action === "string" ? args.action : "run";
	const target = typeof args.agent === "string"
		? args.agent
		: (typeof args.team === "string"
			? args.team
			: (typeof args.chain === "string" ? args.chain : DEFAULT_AGENT));
	const cwd = typeof args.cwd === "string" && args.cwd.trim() ? ` @${args.cwd.trim()}` : "";
	const executionMode = normalizeExecutionMode(typeof args.executionMode === "string" ? args.executionMode : undefined);
	let text = theme.fg("toolTitle", theme.bold("aoc_subagent ")) + theme.fg("muted", action);
	if (target) text += " " + theme.fg("accent", target);
	if (cwd) text += " " + theme.fg("dim", cwd);
	if (executionMode !== "background") text += " " + theme.fg("dim", `[${executionMode}]`);
	return new Text(text, 0, 0);
}

function renderSpecialistRoleResult(result: any, expanded: boolean, theme: Theme): Text {
	const details = result?.details ?? {};
	if (details.action === "list_roles") {
		const text = result?.content?.[0]?.type === "text" ? String(result.content[0].text) : "";
		return new Text(text, 0, 0);
	}
	const job = details.job as JobRecord | undefined;
	const role = details.role
		? getSpecialistRole(String(details.role))
		: (job?.specialistRole ? getSpecialistRole(job.specialistRole) : undefined);
	if (job && role) {
		let text = `${theme.fg("success", statusIcon(job.status))} ${theme.fg("muted", `${role.label} · ${job.status}`)}`;
		text += `\n${theme.fg("accent", job.jobId)}`;
		const telemetry = [
			job.executionMode !== "background" ? job.executionMode : undefined,
			job.contextPackUsed ? "mind-v2" : "no-context",
			typeof job.writeApproved === "boolean" ? (job.writeApproved ? "approved" : "read-first") : undefined,
		]
			.filter(Boolean)
			.join(" · ");
		text += `\n${theme.fg("dim", `${role.agent}${telemetry ? ` · ${telemetry}` : ""}`)}`;
		const toolSummary = summarizeToolPolicies(job.toolPolicies);
		if (toolSummary) text += `\n${theme.fg("dim", toolSummary)}`;
		if (expanded && job.outputExcerpt) text += `\n${theme.fg("muted", truncate(job.outputExcerpt, 240) ?? "")}`;
		if (expanded && job.error) text += `\n${theme.fg("error", job.error)}`;
		return new Text(text, 0, 0);
	}
	return renderToolResult(result, expanded, theme);
}

function renderToolResult(result: any, expanded: boolean, theme: Theme): Text {
	const details = result?.details ?? {};
	const action = typeof details.action === "string" ? details.action : undefined;
	if (action === "list_agents") {
		const text = result?.content?.[0]?.type === "text" ? String(result.content[0].text) : "";
		const lines = text.split(/\r?\n/).filter(Boolean);
		if (expanded || lines.length <= 6) return new Text(text, 0, 0);
		return new Text(`${lines.slice(0, 6).join("\n")}\n${theme.fg("dim", `... ${lines.length - 6} more lines`)}`, 0, 0);
	}
	const job = details.job as JobRecord | undefined;
	if (job) {
		let text = `${theme.fg("success", statusIcon(job.status))} ${theme.fg("muted", `${specialistRoleLabel(job) ?? job.agent} · ${job.status}`)}`;
		text += `\n${theme.fg("accent", job.jobId)}`;
		if (job.teamName) text += `\n${theme.fg("muted", `team ${job.teamName}`)}`;
		if (job.chainName) text += `\n${theme.fg("muted", `chain ${job.chainName}`)}`;
		const telemetry = [job.executionMode !== "background" ? job.executionMode : undefined, job.contextPackUsed ? "mind-v2" : undefined, job.writeApproved ? "approved" : undefined].filter(Boolean).join(" · ");
		if (telemetry) text += `\n${theme.fg("dim", telemetry)}`;
		const toolSummary = summarizeToolPolicies(job.toolPolicies);
		if (toolSummary) text += `\n${theme.fg("dim", toolSummary)}`;
		if (expanded) {
			if (job.error) text += `\n${theme.fg("error", job.error)}`;
			if (job.outputExcerpt) text += `\n${theme.fg("muted", truncate(job.outputExcerpt, 240) ?? "")}`;
		}
		return new Text(text, 0, 0);
	}
	const text = result?.content?.[0]?.type === "text" ? String(result.content[0].text) : "";
	if (!expanded && text.includes("\n")) {
		const lines = text.split(/\r?\n/);
		if (lines.length <= 5) return new Text(text, 0, 0);
		return new Text(`${lines.slice(0, 5).join("\n")}\n${theme.fg("dim", `... ${lines.length - 5} more lines`)}`, 0, 0);
	}
	return new Text(text, 0, 0);
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

type ManagerSection = "recent" | "agents" | "teams" | "chains" | "roles";

const MANAGER_SECTIONS: ManagerSection[] = ["recent", "agents", "teams", "chains", "roles"];

function inspectorSummary(job: JobRecord): string {
	return truncate(job.outputExcerpt || job.error || job.stderrExcerpt || job.task || "no summary", 240) || "no summary";
}

function inspectorTime(job: JobRecord): string {
	const at = job.finishedAt ?? job.startedAt ?? job.createdAt;
	return new Date(at).toISOString().replace("T", " ").slice(0, 19);
}

function managerRecentJobs(limit = 12): JobRecord[] {
	return combinedJobs().slice(0, limit);
}

function managerFailureJobs(limit = 4): JobRecord[] {
	return managerRecentJobs(Math.max(limit, 12)).filter((job) => needsAttentionStatus(job.status)).slice(0, limit);
}

function managerLaunchSnippetForAgent(agent: AgentConfig): string {
	return `/subagent-run ${agent.name} :: <task>`;
}

function managerLaunchSnippetForTeam(name: string): string {
	return `/subagent-team ${name} :: <task>`;
}

function managerLaunchSnippetForChain(name: string): string {
	return `/subagent-chain ${name} :: <task>`;
}

function managerLaunchSnippetForRole(role: SpecialistRoleConfig): string {
	return role.requiresWriteApproval
		? `/specialist-run ${role.role} :: <task> [:: approve-write]`
		: `/specialist-run ${role.role} :: <task>`;
}

function managerLaunchSnippetForJob(job: JobRecord): string {
	if (job.specialistRole) return managerLaunchSnippetForRole(getSpecialistRole(job.specialistRole));
	if (job.teamName) return managerLaunchSnippetForTeam(job.teamName);
	if (job.chainName) return managerLaunchSnippetForChain(job.chainName);
	return `/subagent-run ${job.agent} :: ${job.task || "<task>"}`;
}

function recentJobsForTeam(teamName: string, limit = 3): JobRecord[] {
	return combinedJobs().filter((job) => job.teamName === teamName).slice(0, limit);
}

function recentJobsForChain(chainName: string, limit = 3): JobRecord[] {
	return combinedJobs().filter((job) => job.chainName === chainName).slice(0, limit);
}

function formatTeamDetail(root: string, name: string, members: string[]): string {
	const lines = [
		`Team: ${name}`,
		`members: ${members.length}`,
	];
	for (const [index, member] of members.entries()) {
		lines.push(`- member ${index + 1}: ${member}`);
	}
	const recent = recentJobsForTeam(name, 3);
	if (recent.length > 0) {
		lines.push("recent_runs:");
		for (const job of recent) {
			lines.push(`- ${shortJobId(job.jobId)} · ${job.status} · ${inspectorTime(job)}`);
			lines.push(`  task: ${truncate(job.task || "(none)", 140)}`);
			const stepSummary = summarizeStepResults(job);
			if (stepSummary) lines.push(`  ${stepSummary}`);
			if (job.reportPath) lines.push(`  report: ${job.reportPath}`);
		}
	}
	lines.push(`launch: ${managerLaunchSnippetForTeam(name)}`);
	lines.push(`rerun: /subagent-rerun <job-id>`);
	return lines.join("\n");
}

function formatChainDetail(root: string, name: string, chain: ChainDefinition): string {
	const lines = [
		`Chain: ${name}`,
		`steps: ${chain.steps.length}`,
	];
	if (chain.description) lines.push(`description: ${chain.description}`);
	for (const [index, step] of chain.steps.entries()) {
		lines.push(`- step ${index + 1}: ${step.agent}`);
		if (step.prompt) lines.push(`  prompt: ${truncate(step.prompt.replace(/\s+/g, " "), 140)}`);
	}
	const recent = recentJobsForChain(name, 3);
	if (recent.length > 0) {
		lines.push("recent_runs:");
		for (const job of recent) {
			lines.push(`- ${shortJobId(job.jobId)} · ${job.status} · ${inspectorTime(job)}`);
			lines.push(`  task: ${truncate(job.task || "(none)", 140)}`);
			if (job.reportPath) lines.push(`  report: ${job.reportPath}`);
		}
	}
	lines.push(`launch: ${managerLaunchSnippetForChain(name)}`);
	lines.push(`rerun: /subagent-rerun <job-id>`);
	return lines.join("\n");
}

function shortJobId(jobId: string): string {
	return jobId.length <= 14 ? jobId : `${jobId.slice(0, 14)}…`;
}

type LaunchDialogRequest =
	| {
		kind: "agent";
		agent: AgentConfig;
		initialTask?: string;
		initialCwd?: string;
		initialExecutionMode?: ExecutionMode;
	}
	| {
		kind: "team";
		teamName: string;
		members: string[];
		initialTask?: string;
		initialCwd?: string;
		initialExecutionMode?: ExecutionMode;
	}
	| {
		kind: "chain";
		chainName: string;
		chain: ChainDefinition;
		initialTask?: string;
		initialCwd?: string;
		initialExecutionMode?: ExecutionMode;
	}
	| {
		kind: "role";
		role: SpecialistRoleConfig;
		initialTask?: string;
		initialCwd?: string;
		initialExecutionMode?: ExecutionMode;
		initialApproveWrite?: boolean;
		contextPack?: MindContextPackPayload;
	};

type LaunchDialogResult = {
	task: string;
	cwdArg?: string;
	executionMode: ExecutionMode;
	approveWrite?: boolean;
};

function managerDisplayCwdValue(root: string, cwd?: string): string {
	if (!cwd) return "";
	const rel = path.relative(root, cwd);
	if (!rel) return "";
	return rel.startsWith("..") ? cwd : rel;
}

function formatContextPackStatus(pack: MindContextPackPayload | undefined): string {
	if (!pack?.rendered_lines?.some((line) => typeof line === "string" && line.trim())) return "unavailable";
	const citationCount = pack?.citations?.length ?? 0;
	return citationCount > 0 ? `available · ${citationCount} citations` : "available";
}

function formatQueuedLaunchNotice(job: JobRecord, prefix: string): string {
	const lines = [prefix, `mode: ${job.mode}`, `execution: ${job.executionMode}`, `agent: ${job.agent}`];
	if (job.teamName) lines.push(`team: ${job.teamName}`);
	if (job.chainName) lines.push(`chain: ${job.chainName}`);
	if (job.specialistRole) lines.push(`role: ${job.specialistRole}`);
	if (job.artifactDir) lines.push(`artifacts: ${job.artifactDir}`);
	if (job.reportPath) lines.push(`report: ${job.reportPath}`);
	if (typeof job.contextPackUsed === "boolean") lines.push(`context: ${job.contextPackUsed ? "mind-v2 attached" : "unavailable"}`);
	return lines.join("\n");
}

async function waitForTerminalJob(pi: ExtensionAPI, ctx: ExtensionContext, jobId: string, timeoutMs = INLINE_WAIT_TIMEOUT_MS): Promise<JobRecord | undefined> {
	const deadline = now() + timeoutMs;
	let lastSeen = lookupJob(jobId);
	while (now() < deadline) {
		const current = lookupJob(jobId) ?? lastSeen;
		if (current && isTerminalJobStatus(current.status)) return current;
		await sleep(INLINE_WAIT_POLL_MS);
		lastSeen = lookupJob(jobId) ?? lastSeen;
		if (lastSeen && isTerminalJobStatus(lastSeen.status)) return lastSeen;
		await refreshRegistryJobs(ctx, jobId, pi).catch(() => undefined);
		lastSeen = lookupJob(jobId) ?? lastSeen;
	}
	return lookupJob(jobId) ?? lastSeen;
}

function launchResultSeverity(job: JobRecord, completedInline: boolean): "info" | "warning" {
	if (!completedInline) return "info";
	return job.status === "success" ? "info" : "warning";
}

async function resolveLaunchFeedback(pi: ExtensionAPI, ctx: ExtensionContext, job: JobRecord, queuedPrefix: string): Promise<{ job: JobRecord; notice: string; level: "info" | "warning" }> {
	if (job.executionMode === "background") {
		return { job, notice: formatQueuedLaunchNotice(job, queuedPrefix), level: "info" };
	}
	const completed = await waitForTerminalJob(pi, ctx, job.jobId);
	if (completed && isTerminalJobStatus(completed.status)) {
		const notice = job.executionMode === "inline_summary"
			? formatHandoff(completed.jobId)
			: `${formatJob(completed)}\n\n${formatHandoff(completed.jobId)}`;
		return { job: completed, notice, level: launchResultSeverity(completed, true) };
	}
	const pending = completed ?? job;
	return {
		job: pending,
		notice: `${formatQueuedLaunchNotice(pending, queuedPrefix)}\ninline mode timed out after ${Math.round(INLINE_WAIT_TIMEOUT_MS / 1000)}s; continuing in background\nnext_action: /subagent-inspect ${pending.jobId}`,
		level: launchResultSeverity(pending, false),
	};
}

async function showClarifyLaunchDialog(ctx: ExtensionContext, request: LaunchDialogRequest): Promise<LaunchDialogResult | undefined> {
	const root = ctx.cwd ?? process.cwd();
	const initialTask = request.initialTask ?? "";
	const initialCwd = managerDisplayCwdValue(root, request.initialCwd);
	const initialExecutionMode = request.initialExecutionMode ?? "background";
	const requiresApproval = request.kind === "role" && request.role.requiresWriteApproval;
	const contextStatus = request.kind === "role" ? formatContextPackStatus(request.contextPack) : undefined;
	const title = request.kind === "agent"
		? `Clarify before run · ${request.agent.name}`
		: request.kind === "team"
			? `Clarify before run · ${request.teamName}`
			: request.kind === "chain"
				? `Clarify before run · ${request.chainName}`
				: `Clarify before run · ${request.role.label}`;
	const launchSnippet = request.kind === "agent"
		? managerLaunchSnippetForAgent(request.agent)
		: request.kind === "team"
			? managerLaunchSnippetForTeam(request.teamName)
			: request.kind === "chain"
				? managerLaunchSnippetForChain(request.chainName)
				: managerLaunchSnippetForRole(request.role);
	const backingAgent = request.kind === "agent"
		? request.agent.name
		: request.kind === "team"
			? request.members.join(", ") || "(unknown)"
			: request.kind === "chain"
				? request.chain.steps[0]?.agent ?? "(unknown)"
				: request.role.agent;
	const modeLine = "detached runtime";
	return ctx.ui.custom<LaunchDialogResult | undefined>(
		(tui, theme, _kb, done) => {
			let taskText = initialTask;
			let cwdText = initialCwd;
			let executionMode = initialExecutionMode;
			let approveWrite = request.kind === "role" ? Boolean(request.initialApproveWrite) : false;
			let fieldIndex = 0;
			let editingField: "task" | "cwd" | undefined;
			let notice = "";
			let cachedLines: string[] | undefined;
			const editorTheme: EditorTheme = {
				borderColor: (s) => theme.fg("accent", s),
				selectList: {
					selectedPrefix: (text) => theme.fg("accent", text),
					selectedText: (text) => theme.fg("accent", text),
					description: (text) => theme.fg("muted", text),
					scrollInfo: (text) => theme.fg("dim", text),
					noMatch: (text) => theme.fg("warning", text),
				},
			};
			const editor = new Editor(tui, editorTheme);
			const fields = () => {
				const items: Array<"task" | "cwd" | "mode" | "approval" | "dispatch"> = ["task", "cwd", "mode"];
				if (requiresApproval) items.push("approval");
				items.push("dispatch");
				return items;
			};
			const refresh = () => {
				cachedLines = undefined;
				tui.requestRender();
			};
			const activeField = () => fields()[((fieldIndex % fields().length) + fields().length) % fields().length]!;
			const beginEdit = (field: "task" | "cwd") => {
				editingField = field;
				editor.setText(field === "task" ? taskText : cwdText);
				notice = "";
				refresh();
			};
			editor.onSubmit = (value) => {
				const normalized = editingField === "cwd" ? value.trim() : value.trim();
				if (editingField === "task") taskText = normalized;
				if (editingField === "cwd") cwdText = normalized;
				editingField = undefined;
				editor.setText("");
				notice = "";
				refresh();
			};
			const submit = () => {
				const task = taskText.trim();
				if (!task) {
					notice = "Task is required before dispatch.";
					refresh();
					return;
				}
				done({ task, cwdArg: cwdText.trim() || undefined, executionMode, approveWrite: requiresApproval ? approveWrite : undefined });
			};
			const renderField = (label: string, value: string, selected: boolean) => {
				const prefix = selected ? theme.fg("accent", ">") : " ";
				return `${prefix} ${label}: ${value}`;
			};
			return {
				handleInput(data: string) {
					if (editingField) {
						if (matchesKey(data, Key.escape)) {
							editingField = undefined;
							editor.setText("");
							refresh();
							return;
						}
						editor.handleInput(data);
						refresh();
						return;
					}
					if (matchesKey(data, Key.escape) || matchesKey(data, "ctrl+c")) {
						done(undefined);
						return;
					}
					if (matchesKey(data, Key.tab) || matchesKey(data, Key.down) || data === "j") {
						fieldIndex = (fieldIndex + 1) % fields().length;
						notice = "";
						refresh();
						return;
					}
					if (matchesKey(data, "shift+tab") || matchesKey(data, Key.up) || data === "k") {
						fieldIndex = (fieldIndex - 1 + fields().length) % fields().length;
						notice = "";
						refresh();
						return;
					}
					if (data === "l") {
						notice = launchSnippet;
						refresh();
						return;
					}
					if (data === " ") {
						if (activeField() === "mode") {
							executionMode = nextExecutionMode(executionMode);
							notice = "";
							refresh();
							return;
						}
						if (activeField() === "approval") {
							approveWrite = !approveWrite;
							notice = "";
							refresh();
						}
						return;
					}
					if (matchesKey(data, Key.enter)) {
						if (activeField() === "task") {
							beginEdit("task");
							return;
						}
						if (activeField() === "cwd") {
							beginEdit("cwd");
							return;
						}
						if (activeField() === "mode") {
							executionMode = nextExecutionMode(executionMode);
							notice = "";
							refresh();
							return;
						}
						if (activeField() === "approval") {
							approveWrite = !approveWrite;
							notice = "";
							refresh();
							return;
						}
						submit();
					}
				},
				render(width: number) {
					if (cachedLines) return cachedLines;
					const lines: string[] = [];
					const add = (text = "") => lines.push(truncateToWidth(text, width, "…", true));
					const currentField = activeField();
					const writeHint = requiresApproval
						? (taskLooksWriteLike(taskText) || taskLooksDestructive(taskText) ? "required for current task wording" : "toggle only if write/destructive work is intended")
						: "read-first";
					const modeHint = executionModeSummary(executionMode);
					add(theme.fg("accent", "─".repeat(width)));
					add(theme.fg("accent", theme.bold(title)));
					add(theme.fg("muted", ` target: ${request.kind} · backing agent: ${backingAgent}`));
					add(theme.fg("dim", ` mode: ${modeLine} · execution: ${executionMode} · cwd base: ${root}`));
					if (request.kind === "agent") {
						add(theme.fg("dim", ` model: ${request.agent.model || "default"} · tools: ${request.agent.tools.join(",") || "(none)"}`));
					} else if (request.kind === "chain") {
						add(theme.fg("dim", ` steps: ${request.chain.steps.map((step) => step.agent).join(" -> ")}`));
					} else {
						add(theme.fg("dim", ` approval: ${request.role.requiresWriteApproval ? "explicit gate" : "read-first"} · trust: ${request.role.allowedTrustTiers.join("/")}`));
						add(theme.fg("dim", ` mind context: ${contextStatus}`));
					}
					lines.push("");
					add(renderField("task", taskText ? truncate(taskText.replace(/\s+/g, " "), Math.max(18, width - 12)) || "" : "(press enter to edit)", currentField === "task"));
					add(renderField("cwd", cwdText || "(repo root)", currentField === "cwd"));
					add(renderField("execution_mode", `${executionMode} · ${modeHint}`, currentField === "mode"));
					if (requiresApproval) {
						add(renderField("approve_write", approveWrite ? "yes" : "no", currentField === "approval"));
						add(theme.fg("muted", `   ${writeHint}`));
					}
					add(renderField("dispatch", "queue detached run", currentField === "dispatch"));
					add(theme.fg("dim", ` command: ${launchSnippet}`));
					if (cwdText.trim()) add(theme.fg("dim", ` cwd override: ${cwdText.trim()}`));
					lines.push("");
					if (editingField) {
						add(theme.fg("muted", ` Editing ${editingField}...`));
						for (const line of editor.render(Math.max(10, width - 2))) add(` ${line}`);
					} else if (notice) {
						add(theme.fg(notice === launchSnippet ? "muted" : "warning", notice));
					}
					lines.push("");
					add(theme.fg("dim", editingField
						? " Enter save • Esc back"
						: " Tab/↑↓ move • Enter edit/toggle/dispatch • Space toggle mode/approval • l command • Esc cancel"));
					add(theme.fg("accent", "─".repeat(width)));
					cachedLines = lines;
					return lines;
				},
				invalidate() {
					cachedLines = undefined;
				},
			};
		},
		{
			overlay: true,
			overlayOptions: {
				anchor: "center",
				width: 72,
				minWidth: 56,
				maxHeight: "80%",
			},
		},
	);
}

function launchRequestFromJob(root: string, job: JobRecord): LaunchDialogRequest | undefined {
	const bundle = loadManifestBundle(root);
	if (job.specialistRole) {
		return {
			kind: "role",
			role: getSpecialistRole(job.specialistRole),
			initialTask: job.task,
			initialCwd: job.cwd,
			initialExecutionMode: job.executionMode,
			initialApproveWrite: job.writeApproved,
		};
	}
	if (job.teamName) {
		const members = bundle.teams[job.teamName];
		if (!members) return undefined;
		return {
			kind: "team",
			teamName: job.teamName,
			members,
			initialTask: job.task,
			initialCwd: job.cwd,
			initialExecutionMode: job.executionMode,
		};
	}
	if (job.chainName) {
		const chain = bundle.chains[job.chainName];
		if (!chain) return undefined;
		return {
			kind: "chain",
			chainName: job.chainName,
			chain,
			initialTask: job.task,
			initialCwd: job.cwd,
			initialExecutionMode: job.executionMode,
		};
	}
	const agent = bundle.agents.find((candidate) => candidate.name === job.agent);
	if (!agent) return undefined;
	return {
		kind: "agent",
		agent,
		initialTask: job.task,
		initialCwd: job.cwd,
		initialExecutionMode: job.executionMode,
	};
}

async function rerunJob(pi: ExtensionAPI, ctx: ExtensionContext, jobId: string, mode: "clarify" | "as_is" = "clarify"): Promise<JobRecord | undefined> {
	await refreshRegistryJobs(ctx, jobId, pi).catch(() => undefined);
	const job = lookupJob(jobId);
	if (!job) throw new Error(`Unknown detached subagent job: ${jobId}`);
	if (!job.task?.trim()) throw new Error(`Rerun requires preserved task text for ${jobId}`);
	const root = ctx.cwd ?? process.cwd();
	const request = launchRequestFromJob(root, job);
	if (!request) throw new Error(`Unable to build rerun launch flow for ${jobId}; backing manifest is missing.`);
	if (mode === "clarify") return dispatchClarifiedLaunch(pi, ctx, request);
	const bundle = loadManifestBundle(root);
	if (request.kind === "role") {
		const dispatched = await dispatchSpecialistRole(pi, ctx, request.role.role, request.initialTask || job.task, request.initialCwd, request.initialExecutionMode, request.initialApproveWrite);
		const feedback = await resolveLaunchFeedback(pi, ctx, dispatched.job, `Specialist ${dispatched.role.label} queued: ${dispatched.job.jobId}`);
		ctx.ui.notify(feedback.notice, feedback.level);
		return feedback.job;
	}
	if (request.kind === "team") {
		const rerun = await launchTeamJob(pi, ctx, request.teamName, request.initialTask || job.task, request.initialCwd, request.initialExecutionMode, bundle);
		const feedback = await resolveLaunchFeedback(pi, ctx, rerun, `Detached team queued: ${rerun.jobId}`);
		ctx.ui.notify(feedback.notice, feedback.level);
		return feedback.job;
	}
	if (request.kind === "chain") {
		const rerun = await launchChainJob(pi, ctx, request.chainName, request.initialTask || job.task, request.initialCwd, request.initialExecutionMode, bundle);
		const feedback = await resolveLaunchFeedback(pi, ctx, rerun, `Detached chain queued: ${rerun.jobId}`);
		ctx.ui.notify(feedback.notice, feedback.level);
		return feedback.job;
	}
	const rerun = await launchAgentJob(pi, ctx, request.agent.name, request.initialTask || job.task, request.initialCwd, request.initialExecutionMode, bundle);
	const feedback = await resolveLaunchFeedback(pi, ctx, rerun, `Detached subagent queued: ${rerun.jobId}`);
	ctx.ui.notify(feedback.notice, feedback.level);
	return feedback.job;
}

async function dispatchClarifiedLaunch(pi: ExtensionAPI, ctx: ExtensionContext, request: LaunchDialogRequest): Promise<JobRecord | undefined> {
	const clarified = await showClarifyLaunchDialog(ctx, request);
	if (!clarified) return undefined;
	const root = ctx.cwd ?? process.cwd();
	const bundle = loadManifestBundle(root);
	if (request.kind === "role") {
		const dispatched = await dispatchSpecialistRole(pi, ctx, request.role.role, clarified.task, clarified.cwdArg, clarified.executionMode, clarified.approveWrite);
		const feedback = await resolveLaunchFeedback(pi, ctx, dispatched.job, `Specialist ${dispatched.role.label} queued: ${dispatched.job.jobId}`);
		ctx.ui.notify(feedback.notice, feedback.level);
		return feedback.job;
	}
	if (request.kind === "team") {
		const job = await launchTeamJob(pi, ctx, request.teamName, clarified.task, clarified.cwdArg, clarified.executionMode, bundle);
		const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached team queued: ${job.jobId}`);
		ctx.ui.notify(feedback.notice, feedback.level);
		return feedback.job;
	}
	if (request.kind === "chain") {
		const job = await launchChainJob(pi, ctx, request.chainName, clarified.task, clarified.cwdArg, clarified.executionMode, bundle);
		const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached chain queued: ${job.jobId}`);
		ctx.ui.notify(feedback.notice, feedback.level);
		return feedback.job;
	}
	const job = await launchAgentJob(pi, ctx, request.agent.name, clarified.task, clarified.cwdArg, clarified.executionMode, bundle);
	const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached subagent queued: ${job.jobId}`);
	ctx.ui.notify(feedback.notice, feedback.level);
	return feedback.job;
}

async function showSubagentInspector(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
	if (state.inspectorOpen) {
		state.inspectorClose?.();
		return Promise.resolve();
	}
	return ctx.ui.custom<void>(
		(tui, theme, _keybindings, done) => {
			const close = () => done();
			state.inspectorOpen = true;
			state.inspectorClose = close;
			state.inspectorRequestRender = () => tui.requestRender();
			return new SubagentInspector(pi, ctx, theme, close);
		},
		{
			overlay: true,
			overlayOptions: {
				anchor: "right-center",
				width: "46%",
				minWidth: 52,
				maxHeight: "88%",
				margin: { right: 1, top: 1, bottom: 1 },
				visible: (termWidth) => termWidth >= 100,
			},
		},
	).finally(() => {
		state.inspectorOpen = false;
		state.inspectorClose = undefined;
		state.inspectorRequestRender = undefined;
		state.inspectorRefreshPending = false;
		state.inspectorRefreshNote = undefined;
	});
}

class SubagentInspector {
	private sectionIndex = 0;
	private selected: Record<ManagerSection, number> = {
		recent: 0,
		agents: 0,
		teams: 0,
		chains: 0,
		roles: 0,
	};

	constructor(
		private pi: ExtensionAPI,
		private ctx: ExtensionContext,
		private theme: Theme,
		private done: () => void,
	) {}

	private root(): string {
		return this.ctx.cwd ?? state.ctx?.cwd ?? process.cwd();
	}

	private currentSection(): ManagerSection {
		return MANAGER_SECTIONS[((this.sectionIndex % MANAGER_SECTIONS.length) + MANAGER_SECTIONS.length) % MANAGER_SECTIONS.length]!;
	}

	private manifestSnapshot(): { bundle: ManifestBundle; agents: AgentConfig[]; teams: Array<[string, string[]]>; chains: Array<[string, ChainDefinition]>; roles: SpecialistRoleConfig[] } {
		const root = this.root();
		const bundle = loadManifestBundle(root);
		const agents = availableAgents(bundle, root);
		const teams = Object.entries(availableTeams(bundle, root)).sort((left, right) => left[0].localeCompare(right[0]));
		const chains = Object.entries(availableChains(bundle, root)).sort((left, right) => left[0].localeCompare(right[0]));
		const roles = Object.values(SPECIALIST_ROLES);
		return { bundle, agents, teams, chains, roles };
	}

	private items(section = this.currentSection()): Array<JobRecord | AgentConfig | SpecialistRoleConfig | [string, string[]] | [string, ChainDefinition]> {
		const snapshot = this.manifestSnapshot();
		switch (section) {
			case "recent":
				return managerRecentJobs(12);
			case "agents":
				return snapshot.agents;
			case "teams":
				return snapshot.teams;
			case "chains":
				return snapshot.chains;
			case "roles":
				return snapshot.roles;
		}
	}

	private clampSelection(section = this.currentSection()): void {
		const items = this.items(section);
		if (items.length === 0) {
			this.selected[section] = 0;
			return;
		}
		this.selected[section] = ((this.selected[section] % items.length) + items.length) % items.length;
	}

	private selectedItem(section = this.currentSection()): JobRecord | AgentConfig | SpecialistRoleConfig | [string, string[]] | [string, ChainDefinition] | undefined {
		this.clampSelection(section);
		return this.items(section)[this.selected[section]];
	}

	private notifyLaunchSnippet(text: string, heading = "Launch command"): void {
		this.ctx.ui.notify(`${heading}\n${text}`, "info");
	}

	private inspectSelectedJob(): void {
		const job = this.selectedItem("recent");
		if (!job || Array.isArray(job) || !("jobId" in job)) return;
		this.ctx.ui.notify(`${formatJob(job)}\n\n${formatHandoff(job.jobId)}`, "info");
	}

	private handoffSelectedJob(): void {
		const job = this.selectedItem("recent");
		if (!job || Array.isArray(job) || !("jobId" in job)) return;
		this.ctx.ui.notify(formatHandoff(job.jobId), "info");
	}

	private selectLatestFailure(): void {
		const recent = managerRecentJobs(12);
		const index = recent.findIndex((job) => needsAttentionStatus(job.status));
		if (index >= 0) {
			this.selected.recent = index;
			return;
		}
		this.ctx.ui.notify("No recent detached failures need attention.", "info");
	}

	private async cancelSelectedJob(): Promise<void> {
		const job = this.selectedItem("recent");
		if (!job || Array.isArray(job) || !("jobId" in job)) return;
		if (isTerminalJobStatus(job.status)) {
			this.ctx.ui.notify(`Job ${job.jobId} is already ${job.status}.`, "warning");
			return;
		}
		try {
			const updated = await cancelJob(this.pi, this.ctx, job.jobId);
			this.ctx.ui.notify(`Cancelled ${updated.jobId} (${updated.agent})`, "info");
			this.done();
		} catch (error) {
			this.ctx.ui.notify(String(error), "warning");
		}
	}

	private launchRequestFromRecentJob(job: JobRecord): LaunchDialogRequest | undefined {
		return launchRequestFromJob(this.root(), job);
	}

	private openClarifyFlow(request: LaunchDialogRequest): void {
		this.done();
		setTimeout(() => {
			void (async () => {
				try {
					const hydrated = request.kind === "role"
						? { ...request, contextPack: request.contextPack ?? await fetchMindContextPack(request.role.role, `clarify-before-run: ${request.role.role}`) }
						: request;
					await dispatchClarifiedLaunch(this.pi, this.ctx, hydrated);
				} catch (error) {
					this.ctx.ui.notify(String(error), "warning");
				}
			})();
		}, 0);
	}

	private async rerunSelectedJob(): Promise<void> {
		const selected = this.selectedItem("recent");
		if (!selected || Array.isArray(selected) || !("jobId" in selected)) return;
		if (!selected.task?.trim()) {
			this.notifyLaunchSnippet(managerLaunchSnippetForJob(selected), "Rerun requires a task; use");
			return;
		}
		const request = this.launchRequestFromRecentJob(selected);
		if (!request) {
			this.ctx.ui.notify(`Unable to build rerun launch flow for ${selected.jobId}; backing manifest is missing.`, "warning");
			return;
		}
		this.openClarifyFlow(request);
	}

	private rerunSelectedTeam(): void {
		const current = this.selectedItem("teams");
		if (!current || !Array.isArray(current)) return;
		const latest = recentJobsForTeam(current[0], 1)[0];
		if (!latest) {
			this.notifyLaunchSnippet(managerLaunchSnippetForTeam(current[0]), `No prior team run found for ${current[0]}; launch with`);
			return;
		}
		const request = this.launchRequestFromRecentJob(latest);
		if (!request) {
			this.ctx.ui.notify(`Unable to build rerun launch flow for ${latest.jobId}; backing manifest is missing.`, "warning");
			return;
		}
		this.openClarifyFlow(request);
	}

	private rerunSelectedChain(): void {
		const current = this.selectedItem("chains");
		if (!current || !Array.isArray(current)) return;
		const latest = recentJobsForChain(current[0], 1)[0];
		if (!latest) {
			this.notifyLaunchSnippet(managerLaunchSnippetForChain(current[0]), `No prior chain run found for ${current[0]}; launch with`);
			return;
		}
		const request = this.launchRequestFromRecentJob(latest);
		if (!request) {
			this.ctx.ui.notify(`Unable to build rerun launch flow for ${latest.jobId}; backing manifest is missing.`, "warning");
			return;
		}
		this.openClarifyFlow(request);
	}

	handleInput(data: string): void {
		const section = this.currentSection();
		const items = this.items(section);
		if (matchesKey(data, "escape") || matchesKey(data, "ctrl+c") || matchesKey(data, "alt+a")) {
			this.done();
			return;
		}
		if (data === "1") {
			this.sectionIndex = 0;
			return;
		}
		if (data === "2") {
			this.sectionIndex = 1;
			return;
		}
		if (data === "3") {
			this.sectionIndex = 2;
			return;
		}
		if (data === "4") {
			this.sectionIndex = 3;
			return;
		}
		if (data === "5") {
			this.sectionIndex = 4;
			return;
		}
		if (matchesKey(data, "tab") || matchesKey(data, "right")) {
			this.sectionIndex = (this.sectionIndex + 1) % MANAGER_SECTIONS.length;
			return;
		}
		if (matchesKey(data, "shift+tab") || matchesKey(data, "left")) {
			this.sectionIndex = (this.sectionIndex - 1 + MANAGER_SECTIONS.length) % MANAGER_SECTIONS.length;
			return;
		}
		if (items.length === 0) {
			if (matchesKey(data, "return") || matchesKey(data, "space")) this.done();
			return;
		}
		if (matchesKey(data, "down") || data === "j") {
			this.selected[section] = (this.selected[section] + 1) % items.length;
			return;
		}
		if (matchesKey(data, "up") || data === "k") {
			this.selected[section] = (this.selected[section] - 1 + items.length) % items.length;
			return;
		}
		if (section === "recent" && (data === "i" || matchesKey(data, "return") || matchesKey(data, "space"))) {
			this.inspectSelectedJob();
			return;
		}
		if (section === "recent" && data === "h") {
			this.handoffSelectedJob();
			return;
		}
		if (section === "recent" && data === "c") {
			void this.cancelSelectedJob();
			return;
		}
		if (section === "recent" && data === "r") {
			void this.rerunSelectedJob();
			return;
		}
		if (section === "recent" && data === "f") {
			this.selectLatestFailure();
			return;
		}
		if (section === "agents" && (matchesKey(data, "return") || matchesKey(data, "space"))) {
			const current = this.selectedItem(section);
			if (!current || Array.isArray(current) || !("sourcePath" in current)) return;
			this.openClarifyFlow({ kind: "agent", agent: current });
			return;
		}
		if (section === "teams" && (matchesKey(data, "return") || matchesKey(data, "space"))) {
			const current = this.selectedItem(section);
			if (!current || !Array.isArray(current)) return;
			this.openClarifyFlow({ kind: "team", teamName: current[0], members: current[1] });
			return;
		}
		if (section === "teams" && data === "r") {
			this.rerunSelectedTeam();
			return;
		}
		if (section === "chains" && (matchesKey(data, "return") || matchesKey(data, "space"))) {
			const current = this.selectedItem(section);
			if (!current || !Array.isArray(current)) return;
			this.openClarifyFlow({ kind: "chain", chainName: current[0], chain: current[1] });
			return;
		}
		if (section === "chains" && data === "r") {
			this.rerunSelectedChain();
			return;
		}
		if (section === "roles" && (matchesKey(data, "return") || matchesKey(data, "space"))) {
			const current = this.selectedItem(section);
			if (!current || Array.isArray(current) || !("role" in current)) return;
			this.openClarifyFlow({ kind: "role", role: current });
			return;
		}
		if ((section === "agents" || section === "teams" || section === "chains" || section === "roles") && data === "l") {
			const current = this.selectedItem(section);
			if (!current) return;
			if (section === "agents" && !Array.isArray(current) && "sourcePath" in current) {
				this.notifyLaunchSnippet(managerLaunchSnippetForAgent(current), `Launch ${current.name}`);
				return;
			}
			if (section === "teams" && Array.isArray(current)) {
				this.notifyLaunchSnippet(managerLaunchSnippetForTeam(current[0]), `Launch team ${current[0]}`);
				return;
			}
			if (section === "chains" && Array.isArray(current)) {
				this.notifyLaunchSnippet(managerLaunchSnippetForChain(current[0]), `Launch chain ${current[0]}`);
				return;
			}
			if (section === "roles" && !Array.isArray(current) && "role" in current) {
				this.notifyLaunchSnippet(managerLaunchSnippetForRole(current), `Launch ${current.label}`);
			}
		}
	}

	render(width: number): string[] {
		const th = this.theme;
		const innerW = Math.max(1, width - 2);
		const pad = (s: string) => {
			const clipped = truncateToWidth(s, innerW, "…", true);
			return clipped + " ".repeat(Math.max(0, innerW - visibleWidth(clipped)));
		};
		const row = (content = "") => th.fg("border", "│") + pad(content) + th.fg("border", "│");
		const block = (lines: string[], text: string, maxLines: number) => {
			for (const line of text.split("\n").slice(0, maxLines)) lines.push(row(` ${truncateToWidth(line, innerW - 1, "…", true)}`));
		};
		const lines: string[] = [];
		const root = this.root();
		const { bundle, agents, teams, chains, roles } = this.manifestSnapshot();
		const recent = managerRecentJobs(12);
		const failures = managerFailureJobs(3);
		const terminalCount = combinedJobs().filter((job) => isTerminalJobStatus(job.status)).length;
		const section = this.currentSection();
		this.clampSelection(section);
		const current = this.selectedItem(section);
		const tabs = [
			{ key: "1", section: "recent" as ManagerSection, label: `Recent ${recent.length}` },
			{ key: "2", section: "agents" as ManagerSection, label: `Agents ${agents.length}` },
			{ key: "3", section: "teams" as ManagerSection, label: `Teams ${teams.length}` },
			{ key: "4", section: "chains" as ManagerSection, label: `Chains ${chains.length}` },
			{ key: "5", section: "roles" as ManagerSection, label: `Roles ${roles.length}` },
		].map((tab) => tab.section === section ? th.fg("accent", `[${tab.key}] ${tab.label}`) : th.fg("dim", `${tab.key}:${tab.label}`)).join("  ");

		lines.push(th.fg("border", `╭${"─".repeat(innerW)}╮`));
		lines.push(row(` ${th.fg("accent", "Subagent Manager")} ${th.fg("dim", "(Tab switch • ↑/↓ move • Enter open • Alt+A/Esc close)")}`));
		lines.push(row(` ${truncateToWidth(tabs, innerW - 1, "…", true)}`));
		if (state.inspectorRefreshPending) {
			lines.push(row(` ${th.fg("muted", "refreshing detached status…")}`));
		} else if (state.inspectorRefreshNote) {
			lines.push(row(` ${th.fg("dim", truncateToWidth(state.inspectorRefreshNote, Math.max(12, innerW - 2), "…", true))}`));
		} else {
			lines.push(row());
		}

		if (section === "recent") {
			if (!current || Array.isArray(current) || !("jobId" in current)) {
				lines.push(row(` ${th.fg("dim", "No detached subagent jobs recorded yet.")}`));
				lines.push(row(` ${th.fg("dim", "Launch with /subagent-explore, /subagent-run, /subagent-chain, or /specialist-run.")}`));
			} else {
				const job = current;
				lines.push(row(` ${th.fg("title", "Recent jobs")}`));
				for (const [index, item] of recent.slice(0, 8).entries()) {
					const marker = index === this.selected.recent ? th.fg("accent", ">") : " ";
					const label = specialistRoleLabel(item) ?? item.agent;
					const attention = needsAttentionStatus(item.status) ? th.fg("warning", " !") : "";
					lines.push(row(` ${marker} ${statusIcon(item.status)} ${shortJobId(item.jobId)} ${truncate(`${label} · ${item.status}`, Math.max(12, innerW - 24)) || ""}${attention}`));
				}
				lines.push(row());
				lines.push(row(` history: ${terminalCount} terminal   attention: ${failures.length}`));
				if (failures.length > 0) {
					lines.push(row(` ${th.fg("title", "Needs attention")}`));
					for (const failed of failures.slice(0, 2)) {
						const label = specialistRoleLabel(failed) ?? failed.agent;
						lines.push(row(` ${statusIcon(failed.status)} ${shortJobId(failed.jobId)} ${truncate(`${label} · ${failed.status}`, Math.max(12, innerW - 18)) || ""}`));
					}
					lines.push(row());
				}
				lines.push(row(` ${th.fg("accent", specialistRoleLabel(job) ?? job.agent)} ${th.fg("dim", `(${job.jobId})`)}`));
				lines.push(row(` status: ${statusIcon(job.status)} ${job.status}   mode: ${job.mode}/${job.executionMode}   when: ${inspectorTime(job)}`));
				lines.push(row(` cwd: ${truncateToWidth(relative(root, job.cwd), Math.max(8, innerW - 7), "…", true)}`));
				if (job.teamName) lines.push(row(` team: ${job.teamName}`));
				if (job.chainName) lines.push(row(` chain: ${job.chainName}`));
				if (job.specialistRole) lines.push(row(` role: ${job.specialistRole}   approval: ${job.writeApproved ? "approved" : "read-first"}`));
				if (typeof job.contextPackUsed === "boolean") lines.push(row(` context: ${job.contextPackUsed ? "mind-v2 attached" : "unavailable"}`));
				if (job.reportPath) lines.push(row(` report: ${truncateToWidth(job.reportPath, Math.max(12, innerW - 10), "…", true)}`));
				if (job.artifactDir) lines.push(row(` artifacts: ${truncateToWidth(job.artifactDir, Math.max(12, innerW - 13), "…", true)}`));
				lines.push(row());
				lines.push(row(` ${th.fg("title", "Task")}`));
				block(lines, String(job.task || "(none)"), 3);
				lines.push(row());
				lines.push(row(` ${th.fg("title", "Result")}`));
				block(lines, inspectorSummary(job), 4);
				lines.push(row());
				lines.push(row(` ${th.fg("dim", "Enter/i inspect • h handoff • r rerun via clarify • f latest failure • c cancel")}`));
			}
		} else if (section === "agents") {
			lines.push(row(` ${th.fg("title", "Available agents")}`));
			for (const [index, agent] of agents.slice(0, 8).entries()) {
				const marker = index === this.selected.agents ? th.fg("accent", ">") : " ";
				lines.push(row(` ${marker} ${truncateToWidth(agent.name, Math.max(12, innerW - 4), "…", true)}`));
			}
			lines.push(row());
			if (!current || Array.isArray(current) || !("sourcePath" in current)) {
				lines.push(row(` ${th.fg("dim", "No available canonical agents found.")}`));
			} else {
				const agent = current;
				const matching = combinedJobs().filter((job) => job.agent === agent.name).slice(0, 3);
				lines.push(row(` ${th.fg("accent", agent.name)}`));
				if (agent.description) block(lines, agent.description, 2);
				lines.push(row(` file: ${truncateToWidth(relative(root, agent.sourcePath), Math.max(12, innerW - 8), "…", true)}`));
				if (agent.model) lines.push(row(` model: ${agent.model}`));
				lines.push(row(` tools: ${agent.tools.join(",") || "(none)"}`));
				if (matching.length > 0) lines.push(row(` recent_jobs: ${matching.map((job) => shortJobId(job.jobId)).join(", ")}`));
				lines.push(row());
				lines.push(row(` launch: ${truncateToWidth(managerLaunchSnippetForAgent(agent), Math.max(12, innerW - 10), "…", true)}`));
				lines.push(row(` ${th.fg("dim", "Enter opens clarify-before-run • l shows the raw command")}`));
			}
		} else if (section === "teams") {
			lines.push(row(` ${th.fg("title", "Available teams")}`));
			for (const [index, entry] of teams.slice(0, 8).entries()) {
				const marker = index === this.selected.teams ? th.fg("accent", ">") : " ";
				lines.push(row(` ${marker} ${entry[0]} · ${entry[1].length} members`));
			}
			lines.push(row());
			if (!current || !Array.isArray(current)) {
				lines.push(row(` ${th.fg("dim", "No available canonical teams found.")}`));
			} else {
				const [name, members] = current;
				const matching = recentJobsForTeam(name, 3);
				lines.push(row(` ${th.fg("accent", name)}`));
				lines.push(row(` members: ${members.length} total`));
				for (const [index, member] of members.slice(0, 4).entries()) {
					lines.push(row(`  ${index + 1}. ${truncateToWidth(member, Math.max(12, innerW - 8), "…", true)}`));
				}
				if (matching.length > 0) {
					lines.push(row(` recent_jobs: ${matching.map((job) => shortJobId(job.jobId)).join(", ")}`));
					const latest = matching[0]!;
					lines.push(row(` latest: ${statusIcon(latest.status)} ${latest.status} · ${inspectorTime(latest)}`));
					lines.push(row(` task: ${truncateToWidth((latest.task || "(none)").replace(/\s+/g, " "), Math.max(12, innerW - 7), "…", true)}`));
				}
				lines.push(row());
				lines.push(row(` launch: ${truncateToWidth(managerLaunchSnippetForTeam(name), Math.max(12, innerW - 10), "…", true)}`));
				lines.push(row(` detail: ${truncateToWidth(`/subagent-team-detail ${name}`, Math.max(12, innerW - 10), "…", true)}`));
				lines.push(row(` ${th.fg("dim", "Enter opens clarify-before-run • r reruns latest team via clarify • l shows the raw command")}`));
			}
		} else if (section === "chains") {
			lines.push(row(` ${th.fg("title", "Available chains")}`));
			for (const [index, entry] of chains.slice(0, 8).entries()) {
				const marker = index === this.selected.chains ? th.fg("accent", ">") : " ";
				lines.push(row(` ${marker} ${entry[0]} · ${entry[1].steps.length} steps`));
			}
			lines.push(row());
			if (!current || !Array.isArray(current)) {
				lines.push(row(` ${th.fg("dim", "No available canonical chains found.")}`));
			} else {
				const [name, chain] = current;
				const matching = recentJobsForChain(name, 3);
				lines.push(row(` ${th.fg("accent", name)}`));
				if (chain.description) block(lines, chain.description, 2);
				lines.push(row(` steps: ${chain.steps.length} total`));
				for (const [index, step] of chain.steps.slice(0, 4).entries()) {
					lines.push(row(`  ${index + 1}. ${truncateToWidth(step.agent, Math.max(12, innerW - 8), "…", true)}`));
					if (step.prompt) lines.push(row(`     prompt: ${truncateToWidth(step.prompt.replace(/\s+/g, " "), Math.max(12, innerW - 14), "…", true)}`));
				}
				if (matching.length > 0) {
					lines.push(row(` recent_jobs: ${matching.map((job) => shortJobId(job.jobId)).join(", ")}`));
					const latest = matching[0]!;
					lines.push(row(` latest: ${statusIcon(latest.status)} ${latest.status} · ${inspectorTime(latest)}`));
					lines.push(row(` task: ${truncateToWidth((latest.task || "(none)").replace(/\s+/g, " "), Math.max(12, innerW - 7), "…", true)}`));
				}
				lines.push(row());
				lines.push(row(` launch: ${truncateToWidth(managerLaunchSnippetForChain(name), Math.max(12, innerW - 10), "…", true)}`));
				lines.push(row(` detail: ${truncateToWidth(`/subagent-chain-detail ${name}`, Math.max(12, innerW - 10), "…", true)}`));
				lines.push(row(` ${th.fg("dim", "Enter opens clarify-before-run • r reruns latest chain via clarify • l shows the raw command")}`));
			}
		} else {
			lines.push(row(` ${th.fg("title", "Specialist roles")}`));
			for (const [index, role] of roles.slice(0, 8).entries()) {
				const marker = index === this.selected.roles ? th.fg("accent", ">") : " ";
				const approval = role.requiresWriteApproval ? "write-approval" : "read-first";
				lines.push(row(` ${marker} ${role.label} · ${approval}`));
			}
			lines.push(row());
			if (!current || Array.isArray(current) || !("role" in current)) {
				lines.push(row(` ${th.fg("dim", "No specialist roles configured.")}`));
			} else {
				const role = current;
				const matching = combinedJobs().filter((job) => job.specialistRole === role.role).slice(0, 3);
				lines.push(row(` ${th.fg("accent", role.label)} (${role.role})`));
				block(lines, role.description, 3);
				lines.push(row(` agent: ${role.agent}`));
				lines.push(row(` trust: ${role.allowedTrustTiers.join("/")}`));
				lines.push(row(` approval: ${role.requiresWriteApproval ? "explicit approve-write required" : "read-first"}`));
				if (matching.length > 0) lines.push(row(` recent_jobs: ${matching.map((job) => shortJobId(job.jobId)).join(", ")}`));
				lines.push(row());
				lines.push(row(` launch: ${truncateToWidth(managerLaunchSnippetForRole(role), Math.max(12, innerW - 10), "…", true)}`));
				lines.push(row(` ${th.fg("dim", "Enter opens clarify-before-run • l shows the raw command • approval stays explicit")}`));
			}
		}

		if (bundle.validationErrors.length > 0) {
			lines.push(row());
			lines.push(row(` ${th.fg("title", "Manifest warnings")}`));
			for (const warning of bundle.validationErrors.slice(0, 2)) block(lines, `- ${warning}`, 1);
		}
		lines.push(th.fg("border", `╰${"─".repeat(innerW)}╯`));
		return lines;
	}

	invalidate(): void {}
	dispose(): void {}
}

function finalizeJob(pi: ExtensionAPI, ctx: ExtensionContext | undefined, jobId: string, patch: Partial<JobRecord>): void {
	const current = state.jobs.get(jobId);
	if (!current) return;
	const previousStatus = current.status;
	const root = ctx?.cwd ?? state.ctx?.cwd ?? process.cwd();
	const updated = persistArtifactBundle(root, { ...current, ...patch });
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
	const currentBeforeSpawn = state.jobs.get(jobId);
	if (currentBeforeSpawn) {
		const enriched = persistArtifactBundle(root, currentBeforeSpawn, { prompt: task, agent });
		state.jobs.set(jobId, enriched);
	}

	const proc = spawn("pi", args, {
		cwd,
		stdio: ["ignore", "pipe", "pipe"],
		shell: false,
		env: {
			...process.env,
			AOC_SUBAGENT_JOB_ID: jobId,
			AOC_SUBAGENT_PARENT_JOB_ID: currentDelegatedParentJobId() ?? "",
			AOC_SUBAGENT_DEPTH: String(currentSubagentNestingDepth() + 1),
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
		const current = state.jobs.get(jobId);
		if (current) {
			const enriched = persistArtifactBundle(root, current, { appendEvent: line });
			state.jobs.set(jobId, enriched);
		}
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
		const text = chunk.toString("utf8");
		stderrBuffer += text;
		const current = state.jobs.get(jobId);
		if (current) {
			const enriched = persistArtifactBundle(root, current, { appendStderr: text });
			state.jobs.set(jobId, enriched);
		}
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
		const updated = state.jobs.get(jobId);
		if (updated) {
			const enriched = persistArtifactBundle(root, updated, { fullOutput: latestAssistantText || undefined });
			state.jobs.set(jobId, enriched);
			persistJob(pi, enriched);
		}
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
	executionMode: ExecutionMode = "background",
): JobRecord {
	const root = ctx.cwd ?? process.cwd();
	assertAgentAvailable(bundle, root, agentName);
	const agent = bundle.agents.find((candidate) => candidate.name === agentName);
	if (!agent) {
		throw new Error(
			`Unknown canonical agent: ${agentName}. Available: ${bundle.agents.map((item) => item.name).join(", ") || "none"}`,
		);
	}
	const toolPolicies = resolveToolPolicies(pi, root, agent.tools);
	assertAllowedToolPolicies(toolPolicies, agent.name);
	const cwd = resolveScopedCwd(root, cwdArg);
	const jobId = makeJobId(agent.name);
	const job = persistArtifactBundle(root, {
		jobId,
		mode: "dispatch",
		executionMode,
		agent: agent.name,
		agentFile: relative(root, agent.sourcePath),
		status: "queued",
		task,
		cwd,
		createdAt: now(),
		model: agent.model,
		tools: agent.tools,
		toolPolicies,
		fallbackUsed: bundle.validationErrors.length > 0,
		manifestErrors: [...bundle.validationErrors],
	}, { prompt: task, agent });

	state.jobs.set(jobId, job);
	persistJob(pi, job);
	updateUi(ctx);
	spawnDetachedStep(pi, ctx, jobId, agent, task, cwd);
	return state.jobs.get(jobId)!;
}

function startDetachedTeam(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	bundle: ManifestBundle,
	teamName: string,
	input: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
): JobRecord {
	const root = ctx.cwd ?? process.cwd();
	assertTeamAvailable(bundle, root, teamName);
	const members = bundle.teams[teamName];
	if (!members || members.length === 0) {
		throw new Error(`Unknown canonical team: ${teamName}. Available: ${Object.keys(bundle.teams).join(", ") || "none"}`);
	}
	const teamAgents = members.map((member) => bundle.agents.find((candidate) => candidate.name === member)).filter(Boolean) as AgentConfig[];
	if (teamAgents.length === 0) {
		throw new Error(`Team ${teamName} has no runnable members.`);
	}
	const cwd = resolveScopedCwd(root, cwdArg);
	const initialToolPolicies = Array.from(new Map(teamAgents.flatMap((agent) => resolveToolPolicies(pi, root, agent.tools)).map((policy) => [policy.name, policy])).values());
	for (const agent of teamAgents) {
		assertAllowedToolPolicies(resolveToolPolicies(pi, root, agent.tools), agent.name);
	}
	const jobId = makeJobId(teamName);
	const job = persistArtifactBundle(root, {
		jobId,
		mode: "parallel",
		executionMode,
		agent: teamName,
		agentFile: relative(root, path.join(root, ".pi", "agents", "teams.yaml")),
		status: "queued",
		task: input,
		cwd,
		createdAt: now(),
		tools: Array.from(new Set(teamAgents.flatMap((agent) => agent.tools))),
		toolPolicies: initialToolPolicies,
		fallbackUsed: bundle.validationErrors.length > 0,
		manifestErrors: [...bundle.validationErrors],
		teamName,
		stepResults: [],
	}, { prompt: input });
	state.jobs.set(jobId, job);
	persistJob(pi, job);
	updateUi(ctx);

	const settled: Array<{ agent: string; status: JobStatus; output?: string; error?: string; stderr?: string }> = [];
	const runMember = (index: number) => {
		if (index >= teamAgents.length) {
			const successCount = settled.filter((entry) => entry.status === "success").length;
			const cancelledCount = settled.filter((entry) => entry.status === "cancelled").length;
			const degradedCount = settled.length - successCount - cancelledCount;
			const status: JobStatus = cancelledCount > 0 ? "cancelled" : (degradedCount > 0 ? "fallback" : "success");
			const summary = `team ${teamName} settled | success=${successCount} fallback=${degradedCount} cancelled=${cancelledCount}`;
			finalizeJob(pi, ctx, jobId, {
				status,
				finishedAt: now(),
				pid: undefined,
				agent: teamName,
				agentFile: relative(root, path.join(root, ".pi", "agents", "teams.yaml")),
				outputExcerpt: truncate(summary),
				error: degradedCount > 0 ? truncate(settled.find((entry) => entry.error)?.error, 320) : undefined,
				stderrExcerpt: truncate(settled.find((entry) => entry.stderr)?.stderr, 320),
				fallbackUsed: bundle.validationErrors.length > 0 || degradedCount > 0,
				teamName,
				stepResults: settled.map((entry) => ({
					agent: entry.agent,
					status: entry.status,
					outputExcerpt: truncate(entry.output),
					stderrExcerpt: truncate(entry.stderr, 320),
					error: truncate(entry.error, 320),
				})),
			});
			return;
		}
		const agent = teamAgents[index]!;
		const stepToolPolicies = resolveToolPolicies(pi, root, agent.tools);
		assertAllowedToolPolicies(stepToolPolicies, agent.name);
		finalizeJob(pi, ctx, jobId, {
			status: "queued",
			finishedAt: undefined,
			exitCode: undefined,
			pid: undefined,
			agent: teamName,
			agentFile: relative(root, path.join(root, ".pi", "agents", "teams.yaml")),
			task: input,
			tools: Array.from(new Set(teamAgents.flatMap((candidate) => candidate.tools))),
			toolPolicies: initialToolPolicies,
			outputExcerpt: truncate(`team ${teamName}: running ${agent.name} (${index + 1}/${teamAgents.length})`),
			teamName,
			stepResults: state.jobs.get(jobId)?.stepResults ?? [],
		});
		spawnDetachedStep(pi, ctx, jobId, agent, input, cwd, (result) => {
			settled.push({ agent: agent.name, status: result.status, output: result.output, error: result.error, stderr: result.stderr });
			const current = state.jobs.get(jobId);
			if (current) {
				const updated = persistArtifactBundle(root, {
					...current,
					agent: teamName,
					agentFile: relative(root, path.join(root, ".pi", "agents", "teams.yaml")),
					task: input,
					tools: Array.from(new Set(teamAgents.flatMap((candidate) => candidate.tools))),
					toolPolicies: initialToolPolicies,
					teamName,
					stepResults: settled.map((entry) => ({
						agent: entry.agent,
						status: entry.status,
						outputExcerpt: truncate(entry.output),
						stderrExcerpt: truncate(entry.stderr, 320),
						error: truncate(entry.error, 320),
					})),
					outputExcerpt: truncate(`team ${teamName}: completed ${agent.name} (${index + 1}/${teamAgents.length})`),
				});
				state.jobs.set(jobId, updated);
				persistJob(pi, updated);
			}
			setTimeout(() => runMember(result.status === "cancelled" ? teamAgents.length : index + 1), 0);
		});
	};

	runMember(0);
	return state.jobs.get(jobId)!;
}

function startDetachedChain(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	bundle: ManifestBundle,
	chainName: string,
	input: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
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
	const initialToolPolicies = resolveToolPolicies(pi, root, firstAgent.tools);
	assertAllowedToolPolicies(initialToolPolicies, firstAgent.name);
	const jobId = makeJobId(chainName);
	const job = persistArtifactBundle(root, {
		jobId,
		mode: "chain",
		executionMode,
		agent: firstAgent.name,
		agentFile: relative(root, firstAgent.sourcePath),
		status: "queued",
		task: input,
		cwd,
		createdAt: now(),
		model: firstAgent.model,
		tools: firstAgent.tools,
		toolPolicies: initialToolPolicies,
		fallbackUsed: bundle.validationErrors.length > 0,
		manifestErrors: [...bundle.validationErrors],
		chainName,
		chainStepIndex: 0,
		chainStepCount: chain.steps.length,
	}, { prompt: input, agent: firstAgent });
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
		const stepToolPolicies = resolveToolPolicies(pi, root, agent.tools);
		assertAllowedToolPolicies(stepToolPolicies, agent.name);
		finalizeJob(pi, ctx, jobId, {
			status: "queued",
			chainStepIndex: index,
			agent: agent.name,
			agentFile: relative(root, agent.sourcePath),
			task: stepTask,
			model: agent.model,
			tools: agent.tools,
			toolPolicies: stepToolPolicies,
			error: undefined,
		});
		const current = state.jobs.get(jobId);
		if (current) {
			const enriched = persistArtifactBundle(root, current, { prompt: stepTask, agent });
			state.jobs.set(jobId, enriched);
			persistJob(pi, enriched);
		}
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
	let job = state.jobs.get(jobId) ?? state.registryJobs.get(jobId);
	if (!job) {
		await refreshRegistryJobs(ctx, jobId, pi);
		job = state.jobs.get(jobId) ?? state.registryJobs.get(jobId);
	}
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
	updateUi(ctx);
}

async function openSubagentManager(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
	await ensureInitialized(pi, ctx);
	if (!state.inspectorOpen) refreshRegistryJobsInBackground(pi, ctx);
	await showSubagentInspector(pi, ctx);
}

function parseLeadingExecutionMode(raw: string): { executionMode: ExecutionMode; rest: string } {
	assertNoUnsupportedSessionModeFlags(raw);
	const trimmed = raw.trim();
	const modeFlag = trimmed.match(/^--mode\s+(background|inline_wait|inline_summary|wait|summary)\s+/i);
	if (modeFlag) {
		return {
			executionMode: normalizeExecutionMode(modeFlag[1]),
			rest: trimmed.slice(modeFlag[0].length).trim(),
		};
	}
	if (/^--wait(\s+|$)/i.test(trimmed)) {
		return { executionMode: "inline_wait", rest: trimmed.replace(/^--wait(\s+|$)/i, "").trim() };
	}
	if (/^--summary(\s+|$)/i.test(trimmed)) {
		return { executionMode: "inline_summary", rest: trimmed.replace(/^--summary(\s+|$)/i, "").trim() };
	}
	if (/^--background(\s+|$)/i.test(trimmed)) {
		return { executionMode: "background", rest: trimmed.replace(/^--background(\s+|$)/i, "").trim() };
	}
	return { executionMode: "background", rest: trimmed };
}

function registerFixedAgentCommand(pi: ExtensionAPI, name: string, agent: string, description: string): void {
	pi.registerCommand(name, {
		description,
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const parsed = parseLeadingExecutionMode(args?.trim() || "");
			const task = parsed.rest;
			if (!task) {
				ctx.ui.notify(`Usage: /${name} [--wait|--summary|--background] <task>`, "warning");
				return;
			}
			const bundle = loadManifestBundle(ctx.cwd ?? process.cwd());
			const job = await launchAgentJob(pi, ctx, agent, task, undefined, parsed.executionMode, bundle);
			const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached ${agent} queued: ${job.jobId}`);
			ctx.ui.notify(feedback.notice, feedback.level);
		},
	});
}

async function dispatchSpecialistRole(
	pi: ExtensionAPI,
	ctx: ExtensionContext,
	roleName: string,
	task: string,
	cwdArg?: string,
	executionMode: ExecutionMode = "background",
	approveWrite?: boolean,
): Promise<{ role: SpecialistRoleConfig; job: JobRecord; contextPackUsed: boolean }> {
	assertDispatchGuardrails();
	const root = ctx.cwd ?? process.cwd();
	const role = getSpecialistRole(roleName);
	enforceRoleApproval(role, task, approveWrite);
	const bundle = loadManifestBundle(root);
	assertAgentAvailable(bundle, root, role.agent);
	const agent = bundle.agents.find((candidate) => candidate.name === role.agent);
	if (!agent) throw new Error(`Role ${role.label} references missing agent ${role.agent}`);
	const toolPolicies = resolveToolPolicies(pi, root, agent.tools);
	assertRoleToolPolicies(toolPolicies, role);
	const contextPack = await fetchMindContextPack(role.role, `specialist dispatch: ${role.role}`);
	const contextPrelude = renderContextPackPrelude(contextPack);
	const preface = [
		`Specialist role: ${role.label}`,
		`Role contract: ${role.description}`,
		`Developer control: explicit invocation only; do not continue into autonomous fan-out.`,
		`Approval mode: ${role.requiresWriteApproval ? (approveWrite ? "approved" : "read-first") : "read-first"}`,
		"Return the role's documented output contract with concrete citations.",
		contextPrelude ? `Mind v2 context pack:\n${contextPrelude}` : undefined,
		task,
	].filter(Boolean).join("\n\n");
	let job = await launchAgentJob(pi, ctx, role.agent, preface, cwdArg, executionMode, bundle);
	job.toolPolicies = toolPolicies;
	job.specialistRole = role.role;
	job.writeApproved = role.requiresWriteApproval ? Boolean(approveWrite) : false;
	job.contextPackUsed = Boolean(contextPrelude);
	const rootWithArtifacts = ctx.cwd ?? process.cwd();
	job = persistArtifactBundle(rootWithArtifacts, job, { prompt: preface, agent });
	if (state.jobs.has(job.jobId)) state.jobs.set(job.jobId, job);
	if (state.registryJobs.has(job.jobId)) state.registryJobs.set(job.jobId, job);
	persistJob(pi, job);
	updateUi(ctx);
	return { role, job, contextPackUsed: Boolean(contextPrelude) };
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
		name: "aoc_specialist_role",
		label: "AOC Specialist Role",
		description: "Explicitly dispatch, inspect, or cancel developer-in-control specialist role runs backed by canonical project-local agents.",
		promptSnippet: "Use this for explicit human-in-command specialist role dispatch such as scout, planner, builder, reviewer, documenter, or red-team runs.",
		promptGuidelines: [
			"Use action=dispatch only when the user explicitly asks to invoke a specialist role.",
			"Use built-in/project-local role agents and respect provenance-aware tool policy from sourceInfo.",
			"Do not use this as autonomous fan-out; developer remains in control.",
			"Builder and red-team write/destructive requests require approveWrite=true.",
			"Use action=status or action=cancel for an existing detached role run instead of guessing.",
		],
		parameters: SpecialistRoleParams,
		renderCall(args, theme, _context) {
			return renderSpecialistRoleCall(args as Record<string, any>, theme);
		},
		renderResult(result, { expanded }, theme, _context) {
			return renderSpecialistRoleResult(result, expanded, theme);
		},
		async execute(_toolCallId, params, signal, _onUpdate, ctx) {
			await ensureInitialized(pi, ctx);
			if (signal?.aborted) throw new Error("aoc_specialist_role aborted before execution");
			switch (params.action) {
				case "list_roles": {
					const text = formatRoleCatalog();
					return { content: [{ type: "text", text }], details: { action: params.action } };
				}
				case "status": {
					await refreshRegistryJobs(ctx, params.jobId, pi);
					const text = formatStatusReport(params.jobId);
					const job = params.jobId ? lookupJob(params.jobId) : undefined;
					return {
						content: [{ type: "text", text }],
						details: { action: params.action, jobId: params.jobId, role: job?.specialistRole, job },
					};
				}
				case "cancel": {
					if (!params.jobId) throw new Error("cancel requires jobId");
					const job = await cancelJob(pi, ctx, params.jobId);
					return {
						content: [{ type: "text", text: `Cancelled ${job.jobId} (${job.agent}) -> ${job.status}` }],
						details: { action: params.action, role: job.specialistRole, job },
					};
				}
				case "dispatch": {
					const role = params.role?.trim();
					const task = params.task?.trim();
					if (!role) throw new Error("dispatch requires role");
					if (!task) throw new Error("dispatch requires task");
					assertSupportedSessionMode(params.sessionMode);
					const executionMode = normalizeExecutionMode(params.executionMode);
					const dispatched = await dispatchSpecialistRole(pi, ctx, role, task, params.cwd, executionMode, params.approveWrite);
					const feedback = await resolveLaunchFeedback(pi, ctx, dispatched.job, `Specialist ${dispatched.role.label} queued: ${dispatched.job.jobId}`);
					const toolSummary = summarizeToolPolicies(feedback.job.toolPolicies);
					const lines = [feedback.notice, `role: ${dispatched.role.role}`, `execution_mode: ${feedback.job.executionMode}`];
					if (toolSummary) lines.push(`tool_provenance: ${toolSummary}`);
					return { content: [{ type: "text", text: lines.join("\n") }], details: { action: params.action, role: dispatched.role.role, job: feedback.job } };
				}
			}
		},
	});

	pi.registerTool({
		name: "aoc_subagent",
		label: "AOC Subagent",
		description: "Dispatch, inspect, and cancel AOC-native detached project subagents defined under .pi/agents.",
		promptSnippet: "Use this to launch or inspect detached AOC subagents backed by canonical .pi/agents manifests.",
		promptGuidelines: [
			"Use action=dispatch to start one detached canonical project agent when the user asks for specialist background analysis.",
			"Use explorer-agent for repo reconnaissance, code-review-agent for bounded review, testing-agent for targeted verification, and scout-web-agent for browser/site investigation when the agent-browser + managed search stack is available.",
			"Use action=dispatch_team with a canonical team name when the user asks for bounded parallel fanout across a predefined specialist set.",
			"Use action=dispatch_chain with a canonical chain name when the user asks for a predefined multi-step handoff flow.",
			"Use action=status to inspect detached subagent job state instead of guessing completion.",
			"Use action=cancel only when the user asks to stop a detached job.",
		],
		parameters: SubagentParams,
		renderCall(args, theme, _context) {
			return renderToolCall(args as Record<string, any>, theme);
		},
		renderResult(result, { expanded }, theme, _context) {
			return renderToolResult(result, expanded, theme);
		},
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
					assertSupportedSessionMode(params.sessionMode);
					const executionMode = normalizeExecutionMode(params.executionMode);
					const job = await launchAgentJob(pi, ctx, agent, task, params.cwd, executionMode, bundle);
					const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached subagent queued: ${job.jobId}`);
					const lines = [feedback.notice, `execution_mode: ${feedback.job.executionMode}`];
					const toolSummary = summarizeToolPolicies(feedback.job.toolPolicies);
					if (toolSummary) lines.push(`tool_provenance: ${toolSummary}`);
					if (bundle.validationErrors.length > 0) {
						lines.push("manifest_warnings:");
						for (const error of bundle.validationErrors) lines.push(`- ${error}`);
					}
					return { content: [{ type: "text", text: lines.join("\n") }], details: { action: params.action, job: feedback.job } };
				}
				case "dispatch_team": {
					const team = params.team?.trim();
					const task = params.task?.trim();
					if (!team) throw new Error("dispatch_team requires team");
					if (!task) throw new Error("dispatch_team requires task");
					assertSupportedSessionMode(params.sessionMode);
					const executionMode = normalizeExecutionMode(params.executionMode);
					const job = await launchTeamJob(pi, ctx, team, task, params.cwd, executionMode, bundle);
					const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached team queued: ${job.jobId}`);
					const lines = [feedback.notice, `execution_mode: ${feedback.job.executionMode}`];
					const toolSummary = summarizeToolPolicies(feedback.job.toolPolicies);
					if (toolSummary) lines.push(`tool_provenance: ${toolSummary}`);
					if (bundle.validationErrors.length > 0) {
						lines.push("manifest_warnings:");
						for (const error of bundle.validationErrors) lines.push(`- ${error}`);
					}
					return { content: [{ type: "text", text: lines.join("\n") }], details: { action: params.action, job: feedback.job } };
				}
				case "dispatch_chain": {
					const chain = params.chain?.trim();
					const task = params.task?.trim();
					if (!chain) throw new Error("dispatch_chain requires chain");
					if (!task) throw new Error("dispatch_chain requires task");
					assertSupportedSessionMode(params.sessionMode);
					const executionMode = normalizeExecutionMode(params.executionMode);
					const job = await launchChainJob(pi, ctx, chain, task, params.cwd, executionMode, bundle);
					const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached chain queued: ${job.jobId}`);
					const lines = [feedback.notice, `execution_mode: ${feedback.job.executionMode}`];
					const toolSummary = summarizeToolPolicies(feedback.job.toolPolicies);
					if (toolSummary) lines.push(`tool_provenance: ${toolSummary}`);
					if (bundle.validationErrors.length > 0) {
						lines.push("manifest_warnings:");
						for (const error of bundle.validationErrors) lines.push(`- ${error}`);
					}
					return { content: [{ type: "text", text: lines.join("\n") }], details: { action: params.action, job: feedback.job } };
				}
			}
		},
	});

	pi.registerCommand("subagent-inspector", {
		description: "Open the detached subagent manager overlay",
		handler: async (_args, ctx) => {
			await openSubagentManager(pi, ctx);
		},
	});

	pi.registerCommand("subagent-manager", {
		description: "Open the manager-lite overlay for agents, teams, chains, roles, and recent jobs",
		handler: async (_args, ctx) => {
			await openSubagentManager(pi, ctx);
		},
	});

	pi.registerShortcut("alt+a", {
		description: "Open detached subagent manager",
		handler: async (ctx) => {
			await openSubagentManager(pi, ctx);
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
		description: "Show recent detached subagent run history. Usage: /subagent-recent [count]",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			await refreshRegistryJobs(ctx, undefined, pi);
			const limit = Math.max(1, Math.min(10, Number.parseInt(args?.trim() || "5", 10) || 5));
			ctx.ui.notify(formatRecentJobs(limit), "info");
		},
	});

	pi.registerCommand("subagent-history", {
		description: "Show recent detached subagent run history. Usage: /subagent-history [count]",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			await refreshRegistryJobs(ctx, undefined, pi);
			const limit = Math.max(1, Math.min(12, Number.parseInt(args?.trim() || "8", 10) || 8));
			ctx.ui.notify(formatRecentJobs(limit), "info");
		},
	});

	pi.registerCommand("subagent-team-detail", {
		description: "Show team detail, members, and recent runs. Usage: /subagent-team-detail <team>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const name = args?.trim();
			if (!name) {
				ctx.ui.notify("Usage: /subagent-team-detail <team>", "warning");
				return;
			}
			await refreshRegistryJobs(ctx, undefined, pi);
			const root = ctx.cwd ?? process.cwd();
			const bundle = loadManifestBundle(root);
			const team = bundle.teams[name];
			if (!team) {
				ctx.ui.notify(`Unknown canonical team: ${name}`, "warning");
				return;
			}
			ctx.ui.notify(formatTeamDetail(root, name, team), "info");
		},
	});

	pi.registerCommand("subagent-chain-detail", {
		description: "Show chain detail, steps, and recent runs. Usage: /subagent-chain-detail <chain>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const name = args?.trim();
			if (!name) {
				ctx.ui.notify("Usage: /subagent-chain-detail <chain>", "warning");
				return;
			}
			await refreshRegistryJobs(ctx, undefined, pi);
			const root = ctx.cwd ?? process.cwd();
			const bundle = loadManifestBundle(root);
			const chain = bundle.chains[name];
			if (!chain) {
				ctx.ui.notify(`Unknown canonical chain: ${name}`, "warning");
				return;
			}
			ctx.ui.notify(formatChainDetail(root, name, chain), "info");
		},
	});

	pi.registerCommand("subagent-failures", {
		description: "Show recent detached subagent failures needing attention. Usage: /subagent-failures [count]",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			await refreshRegistryJobs(ctx, undefined, pi);
			const limit = Math.max(1, Math.min(10, Number.parseInt(args?.trim() || "5", 10) || 5));
			ctx.ui.notify(formatFailureJobs(limit), "info");
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

	pi.registerCommand("subagent-rerun", {
		description: "Rerun a detached subagent job from preserved metadata. Usage: /subagent-rerun [--as-is] <job-id>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const raw = args?.trim() || "";
			const asIs = raw.startsWith("--as-is ") || raw === "--as-is";
			const jobId = (asIs ? raw.slice("--as-is".length) : raw).trim();
			if (!jobId) {
				ctx.ui.notify("Usage: /subagent-rerun [--as-is] <job-id>", "warning");
				return;
			}
			await rerunJob(pi, ctx, jobId, asIs ? "as_is" : "clarify");
		},
	});

	pi.registerCommand("subagent-run", {
		description: "Dispatch one detached project-local subagent. Usage: /subagent-run [--wait|--summary|--background] <agent> :: <task>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const parsed = parseLeadingExecutionMode(args?.trim() || "");
			const raw = parsed.rest;
			const separator = raw.indexOf("::");
			if (separator < 0) {
				ctx.ui.notify("Usage: /subagent-run [--wait|--summary|--background] <agent> :: <task>", "warning");
				return;
			}
			const agent = raw.slice(0, separator).trim() || DEFAULT_AGENT;
			const task = raw.slice(separator + 2).trim();
			if (!task) {
				ctx.ui.notify("Task text is required after ::", "warning");
				return;
			}
			const bundle = loadManifestBundle(ctx.cwd ?? process.cwd());
			const job = await launchAgentJob(pi, ctx, agent, task, undefined, parsed.executionMode, bundle);
			const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached subagent queued: ${job.jobId}`);
			ctx.ui.notify(feedback.notice, feedback.level);
		},
	});

	pi.registerCommand("subagent-team", {
		description: "Dispatch one detached canonical team fanout. Usage: /subagent-team [--wait|--summary|--background] <team> :: <task>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const parsed = parseLeadingExecutionMode(args?.trim() || "");
			const raw = parsed.rest;
			const separator = raw.indexOf("::");
			if (separator < 0) {
				ctx.ui.notify("Usage: /subagent-team [--wait|--summary|--background] <team> :: <task>", "warning");
				return;
			}
			const team = raw.slice(0, separator).trim();
			const task = raw.slice(separator + 2).trim();
			if (!team || !task) {
				ctx.ui.notify("Both team and task are required.", "warning");
				return;
			}
			const bundle = loadManifestBundle(ctx.cwd ?? process.cwd());
			const job = await launchTeamJob(pi, ctx, team, task, undefined, parsed.executionMode, bundle);
			const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached team queued: ${job.jobId}`);
			ctx.ui.notify(feedback.notice, feedback.level);
		},
	});

	pi.registerCommand("subagent-chain", {
		description: "Dispatch one detached canonical chain. Usage: /subagent-chain [--wait|--summary|--background] <chain> :: <task>",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const parsed = parseLeadingExecutionMode(args?.trim() || "");
			const raw = parsed.rest;
			const separator = raw.indexOf("::");
			if (separator < 0) {
				ctx.ui.notify("Usage: /subagent-chain [--wait|--summary|--background] <chain> :: <task>", "warning");
				return;
			}
			const chain = raw.slice(0, separator).trim();
			const task = raw.slice(separator + 2).trim();
			if (!chain || !task) {
				ctx.ui.notify("Both chain and task are required.", "warning");
				return;
			}
			const bundle = loadManifestBundle(ctx.cwd ?? process.cwd());
			const job = await launchChainJob(pi, ctx, chain, task, undefined, parsed.executionMode, bundle);
			const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached chain queued: ${job.jobId}`);
			ctx.ui.notify(feedback.notice, feedback.level);
		},
	});

	pi.registerCommand("specialist-roles", {
		description: "List canonical specialist roles and their backing agents.",
		handler: async (_args, ctx) => {
			await ensureInitialized(pi, ctx);
			ctx.ui.notify(formatRoleCatalog(), "info");
		},
	});

	pi.registerCommand("specialist-run", {
		description: "Dispatch an explicit specialist role. Usage: /specialist-run [--wait|--summary|--background] <role> :: <task> [:: approve-write]",
		handler: async (args, ctx) => {
			await ensureInitialized(pi, ctx);
			const parsed = parseLeadingExecutionMode(args?.trim() || "");
			const raw = parsed.rest;
			const parts = raw.split("::").map((part) => part.trim()).filter(Boolean);
			if (parts.length < 2) {
				ctx.ui.notify("Usage: /specialist-run [--wait|--summary|--background] <role> :: <task> [:: approve-write]", "warning");
				return;
			}
			const [roleName, task, ...rest] = parts;
			const approveWrite = rest.some((part) => /^approve-write$/i.test(part));
			const dispatched = await dispatchSpecialistRole(pi, ctx, roleName, task, undefined, parsed.executionMode, approveWrite);
			const feedback = await resolveLaunchFeedback(pi, ctx, dispatched.job, `Specialist ${dispatched.role.label} queued: ${dispatched.job.jobId}`);
			ctx.ui.notify(feedback.notice, feedback.level);
		},
	});

	registerFixedAgentCommand(pi, "subagent-explore", "explorer-agent", "Dispatch explorer-agent. Usage: /subagent-explore [--wait|--summary|--background] <task>");
	registerFixedAgentCommand(pi, "subagent-review", "code-review-agent", "Dispatch code-review-agent. Usage: /subagent-review [--wait|--summary|--background] <task>");
	registerFixedAgentCommand(pi, "subagent-test", "testing-agent", "Dispatch testing-agent. Usage: /subagent-test [--wait|--summary|--background] <task>");
	registerFixedAgentCommand(pi, "subagent-scout", "scout-web-agent", "Dispatch scout-web-agent. Usage: /subagent-scout [--wait|--summary|--background] <task>");
}
