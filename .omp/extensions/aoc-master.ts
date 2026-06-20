import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { randomUUID } from "node:crypto";
import { clampInt, clampMaxChars, findProjectRoot, renderCommand, runBoundedCommand } from "./aoc-runtime";
import { runTranscript } from "./aoc-herdr";
import { StringEnum } from "@mariozechner/pi-ai";
import { Type } from "@sinclair/typebox";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import type { CommandResult } from "./aoc-runtime";

const MAX_DEFAULT_CHARS = 12_000;
const MAX_ALLOWED_CHARS = 40_000;
const COMMAND_TIMEOUT_MS = 30_000;
const DEFAULT_MASTER_TTL_MINUTES = 60;
const MIN_MASTER_TTL_MINUTES = 1;
const MAX_MASTER_TTL_MINUTES = 240;
const DEFAULT_WAIT_FOR_IDLE_MS = 0;
const MAX_WAIT_FOR_IDLE_MS = 300_000;
const DEFAULT_START_WAIT_MS = 10_000;
const MAX_START_WAIT_MS = 30_000;
const DEFAULT_RESULT_TIMEOUT_MS = 0;
const MAX_RESULT_TIMEOUT_MS = 300_000;
const REPORT_SUMMARY_MAX_CHARS = 1200;
const REPORT_DECISION_MAX_CHARS = 1200;
const REPORT_EVIDENCE_MAX_CHARS = 4000;
const REPORT_PROMPT_MAX_CHARS = 6000;
const REPORT_INBOX_DEFAULT_LIMIT = 10;
const REPORT_INBOX_MAX_LIMIT = 50;
const FULL_RETARD_MIN_INTERVAL_MS = 30_000;
const FULL_RETARD_BATCH_MAX_REPORTS = 5;

type CommandContext = {
	cwd?: string;
	ui?: { notify?: (message: string, level?: "info" | "warning" | "error") => void | Promise<void> };
};

type AutocompleteItem = { value: string; label?: string; description?: string };

type CommandDefinition = {
	description: string;
	getArgumentCompletions?: (prefix: string) => AutocompleteItem[] | null;
	handler: (args: string | string[] | undefined, ctx: CommandContext) => void | Promise<void>;
};

type OutboundMessage = { customType: string; display: boolean; content: string; details?: Record<string, unknown> };
type SendOptions = { triggerTurn?: boolean };

type CommandExtensionAPI = ExtensionAPI & {
	registerCommand: (name: string, definition: CommandDefinition) => void;
	sendMessage?: (message: OutboundMessage, options?: SendOptions) => void | Promise<void>;
};

type MasterIdentity = { herdrSession: string; workspaceId: string; paneId: string; cwd: string };
type MasterLease = MasterIdentity & { version: 1; enabledAt: string; expiresAt: string };
type AgentInfo = { agent: string; agentStatus: string; cwd: string; paneId: string; terminalId: string; workspaceId: string };

type OrchestrationEventV1 = {
	version: 1;
	timestamp: string;
	action: "assign" | "send";
	masterPaneId: string;
	target: string;
	resolvedTarget: string;
	ok: boolean;
	exitCode: number | null;
	messagePreview: string;
};

type OrchestrationEventV2 = {
	version: 2;
	timestamp: string;
	assignmentId: string;
	action: "assign" | "send";
	deliveryMode: "draft" | "submit";
	masterPaneId: string;
	target: string;
	resolvedTarget: string;
	ok: boolean;
	exitCode: number | null;
	status: "drafted" | "submitted" | "delivery_failed";
	startStatus?: "skipped" | "started" | "start_unconfirmed";
	command: string;
	messagePreview: string;
	expectedMarker: string;
};

type ReportStatus = "started" | "heartbeat" | "blocked" | "done" | "failed";
type ReportDeliveryMode = "queue" | "notify" | "submit" | "full-retard";
type EffectiveReportDeliveryMode = ReportDeliveryMode | "queue_disabled" | "queue_rate_limited" | "queue_self_target" | "delivery_failed";
type MasterReportEventV1 = {
	version: 1;
	reportId: string;
	timestamp: string;
	assignmentId: string;
	reporterPaneId: string;
	reporterWorkspaceId: string;
	reporterSession: string;
	targetMasterPaneId: string;
	status: ReportStatus;
	deliveryMode: ReportDeliveryMode;
	effectiveDeliveryMode: EffectiveReportDeliveryMode;
	summary: string;
	needsDecision?: string;
	evidence?: string;
	unread: boolean;
	messagePreview: string;
	promptedAt?: string;
	promptMode?: "notify" | "submit" | "full-retard";
	warning?: string;
};
type FullRetardStateV1 = {
	version: 1;
	herdrSession: string;
	workspaceId: string;
	masterPaneId: string;
	enabled: boolean;
	updatedAt: string;
	updatedByPaneId: string;
};
type FullRetardRateStateV1 = {
	version: 1;
	herdrSession: string;
	workspaceId: string;
	masterPaneId: string;
	assignmentId: string;
	lastPromptAt: string;
	lastHeartbeatSummary?: string;
};

type OrchestrationEvent = OrchestrationEventV1 | OrchestrationEventV2;

const DeliveryModeSchema = StringEnum(
	["draft", "submit"] as const,
	{ description: "Peer delivery mode for assign/send. draft writes the prompt for operator review without submitting; submit sends it as an OMP turn." },
);

const ReportStatusSchema = StringEnum(["started", "heartbeat", "blocked", "done", "failed"] as const, { description: "Worker report status." });
const ReportDeliveryModeSchema = StringEnum(["queue", "notify", "submit", "full-retard"] as const, { description: "Worker-to-master report delivery mode. queue persists only; notify drafts; submit and full-retard require full-retard master mode." });
const IngestModeSchema = StringEnum(["summary", "full"] as const, { description: "How much report content to return when ingesting." });

const OrchestrateActionSchema = StringEnum(
	["master_on", "master_off", "master_status", "assign", "send", "collect", "full_retard_on", "full_retard_off", "full_retard_status", "inbox", "ingest", "ack"] as const,
	{ description: "Gated Herdr orchestration action. Mutating peer actions require an active /master lease owned by this pane." },
);

const OrchestrateParams = Type.Object({
	action: OrchestrateActionSchema,
	target: Type.Optional(Type.String({ description: "Peer target for assign/send: pane_id, agent name, terminal id, or label." })),
	message: Type.Optional(Type.String({ description: "Message body for action=send." })),
	goal: Type.Optional(Type.String({ description: "Assignment goal for action=assign." })),
	context: Type.Optional(Type.String({ description: "Relevant context for action=assign." })),
	constraints: Type.Optional(Type.String({ description: "Constraints for action=assign." })),
	acceptance: Type.Optional(Type.String({ description: "Acceptance criteria for action=assign." })),
	deliveryMode: Type.Optional(DeliveryModeSchema),
	assignmentId: Type.Optional(Type.String({ description: "Assignment id for collect, inbox, ingest, ack, or report lookup." })),
	status: Type.Optional(ReportStatusSchema),
	unreadOnly: Type.Optional(Type.Boolean({ description: "Filter inbox to unread reports. Defaults to true for inbox." })),
	limit: Type.Optional(Type.Integer({ minimum: 1, maximum: REPORT_INBOX_MAX_LIMIT, description: "Maximum inbox records to return." })),
	reportId: Type.Optional(Type.String({ description: "Report id for ingest or ack." })),
	ingestMode: Type.Optional(IngestModeSchema),
	startWaitMs: Type.Optional(Type.Integer({ minimum: 0, maximum: MAX_START_WAIT_MS, description: "Submit-only start acknowledgement wait. Defaults to 10000ms; 0 skips acknowledgement." })),
	resultTimeoutMs: Type.Optional(Type.Integer({ minimum: 0, maximum: MAX_RESULT_TIMEOUT_MS, description: "Collect-only idle wait timeout. Defaults to 0, which collects current evidence without waiting." })),
	ttlMinutes: Type.Optional(Type.Integer({ minimum: MIN_MASTER_TTL_MINUTES, maximum: MAX_MASTER_TTL_MINUTES, description: "Master lease TTL for action=master_on. Defaults to 60 minutes." })),
	requireIdle: Type.Optional(Type.Boolean({ description: "For assign/send, reject non-idle peers unless false. Defaults to true." })),
	waitForIdleMs: Type.Optional(Type.Integer({ minimum: 0, maximum: MAX_WAIT_FOR_IDLE_MS, description: "For assign/send, wait this long for the peer to become idle before rejecting. Defaults to 0." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});

type OrchestrateParamsType = {
	action: "master_on" | "master_off" | "master_status" | "assign" | "send" | "collect" | "full_retard_on" | "full_retard_off" | "full_retard_status" | "inbox" | "ingest" | "ack";
	target?: string;
	message?: string;
	goal?: string;
	context?: string;
	constraints?: string;
	acceptance?: string;
	deliveryMode?: "draft" | "submit";
	assignmentId?: string;
	status?: "started" | "heartbeat" | "blocked" | "done" | "failed";
	unreadOnly?: boolean;
	limit?: number;
	reportId?: string;
	ingestMode?: "summary" | "full";
	startWaitMs?: number;
	resultTimeoutMs?: number;
	ttlMinutes?: number;
	requireIdle?: boolean;
	waitForIdleMs?: number;
	maxChars?: number;
};

const ReportParams = Type.Object({
	assignmentId: Type.String({ description: "Assignment id from MASTER ASSIGNMENT/MASTER MESSAGE." }),
	status: ReportStatusSchema,
	summary: Type.String({ description: "Bounded progress/result summary." }),
	needsDecision: Type.Optional(Type.String({ description: "Decision needed from master, if any." })),
	evidence: Type.Optional(Type.String({ description: "Bounded evidence, logs, files, or result details." })),
	deliveryMode: Type.Optional(ReportDeliveryModeSchema),
	targetMaster: Type.Optional(Type.String({ description: "Master pane/agent target. Defaults to assignment event masterPaneId." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});
type ReportParamsType = {
	assignmentId: string;
	status: "started" | "heartbeat" | "blocked" | "done" | "failed";
	summary: string;
	needsDecision?: string;
	evidence?: string;
	deliveryMode?: "queue" | "notify" | "submit" | "full-retard";
	targetMaster?: string;
	maxChars?: number;
};

function argsText(args: string | string[] | undefined): string {
	if (Array.isArray(args)) return args.join(" ").trim();
	return (args ?? "").trim();
}

function requireText(value: string | undefined, label: string): string {
	const trimmed = (value ?? "").trim();
	if (!trimmed) throw new Error(`${label} is required`);
	return trimmed;
}

function safeName(value: string): string {
	const safe = value.replace(/[^A-Za-z0-9_.-]/g, "_");
	return safe || "unknown";
}

function stateDir(): string {
	return path.join(os.homedir(), ".omp", "agent", "master");
}

function makeAssignmentId(): string {
	return randomUUID();
}

function leasePath(identity: MasterIdentity): string {
	return path.join(stateDir(), `${safeName(identity.herdrSession)}-${safeName(identity.workspaceId)}.json`);
}

function currentIdentity(ctxCwd: string | undefined): MasterIdentity {
	const herdrSession = (process.env.HERDR_SESSION ?? "").trim();
	const workspaceId = (process.env.HERDR_WORKSPACE_ID ?? "").trim();
	const paneId = (process.env.HERDR_PANE_ID ?? "").trim();
	if (!herdrSession || !workspaceId || !paneId) throw new Error("master mode requires HERDR_SESSION, HERDR_WORKSPACE_ID, and HERDR_PANE_ID");
	return { herdrSession, workspaceId, paneId, cwd: findProjectRoot(ctxCwd) };
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readLease(file: string): MasterLease | null {
	let parsed: unknown;
	try {
		parsed = JSON.parse(fs.readFileSync(file, "utf8"));
	} catch {
		return null;
	}
	if (!isRecord(parsed)) return null;
	if (parsed.version !== 1) return null;
	for (const key of ["paneId", "workspaceId", "herdrSession", "cwd", "enabledAt", "expiresAt"] as const) {
		if (typeof parsed[key] !== "string") return null;
	}
	return parsed as MasterLease;
}

function isExpired(lease: MasterLease, nowMs: number): boolean {
	const expiresAt = Date.parse(lease.expiresAt);
	return !Number.isFinite(expiresAt) || expiresAt <= nowMs;
}

function writeLeaseExclusive(file: string, lease: MasterLease): void {
	fs.mkdirSync(path.dirname(file), { recursive: true });
	const fd = fs.openSync(file, "wx");
	try {
		fs.writeFileSync(fd, JSON.stringify(lease, null, 2));
	} finally {
		fs.closeSync(fd);
	}
}

function replaceLease(file: string, lease: MasterLease): void {
	fs.mkdirSync(path.dirname(file), { recursive: true });
	const tmp = `${file}.tmp-${process.pid}`;
	let fd: number | null = null;
	try {
		fd = fs.openSync(tmp, "w");
		fs.writeFileSync(fd, JSON.stringify(lease, null, 2));
		fs.closeSync(fd);
		fd = null;
		fs.renameSync(tmp, file);
	} catch (error) {
		if (fd !== null) fs.closeSync(fd);
		try {
			fs.unlinkSync(tmp);
		} catch {
			// best-effort cleanup
		}
		throw error;
	}
}

function makeLease(identity: MasterIdentity, ttlMinutes: number): MasterLease {
	const now = new Date();
	const expiresAt = new Date(now.getTime() + ttlMinutes * 60_000);
	return { ...identity, version: 1, enabledAt: now.toISOString(), expiresAt: expiresAt.toISOString() };
}

function conflictFromExisting(file: string, nowMs: number): { ok: false; reason: string; lease: MasterLease | null } | null {
	const lease = readLease(file);
	if (lease && !isExpired(lease, nowMs)) return { ok: false, reason: `master mode already owned by ${lease.paneId} until ${lease.expiresAt}`, lease };
	return null;
}

function acquireLease(identity: MasterIdentity, ttlMinutes: number): { ok: true; lease: MasterLease; refreshed: boolean } | { ok: false; reason: string; lease: MasterLease | null } {
	const file = leasePath(identity);
	const lease = readLease(file);
	const nowMs = Date.now();
	const next = makeLease(identity, ttlMinutes);
	if (!lease) {
		try {
			writeLeaseExclusive(file, next);
			return { ok: true, lease: next, refreshed: false };
		} catch (error) {
			if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
			return conflictFromExisting(file, nowMs) ?? { ok: false, reason: "master mode lease changed; retry status before enabling", lease: readLease(file) };
		}
	}
	if (!isExpired(lease, nowMs)) {
		if (lease.paneId !== identity.paneId) return { ok: false, reason: `master mode already owned by ${lease.paneId} until ${lease.expiresAt}`, lease };
		replaceLease(file, next);
		return { ok: true, lease: next, refreshed: true };
	}
	try {
		fs.unlinkSync(file);
	} catch (error) {
		if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
	}
	try {
		writeLeaseExclusive(file, next);
		return { ok: true, lease: next, refreshed: false };
	} catch (error) {
		if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
		return conflictFromExisting(file, nowMs) ?? { ok: false, reason: "master mode lease changed; retry status before enabling", lease: readLease(file) };
	}
}

function releaseLease(identity: MasterIdentity): { ok: true; reason: string } | { ok: false; reason: string; lease: MasterLease | null } {
	const file = leasePath(identity);
	const lease = readLease(file);
	if (!lease || isExpired(lease, Date.now())) return { ok: true, reason: "master mode already off" };
	if (lease.paneId !== identity.paneId) return { ok: false, reason: `master mode owned by ${lease.paneId}; only the owner can turn it off`, lease };
	try {
		fs.unlinkSync(file);
	} catch (error) {
		if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
	}
	return { ok: true, reason: "master mode disabled" };
}

function requireActiveLease(identity: MasterIdentity): MasterLease {
	const lease = readLease(leasePath(identity));
	if (!lease || isExpired(lease, Date.now())) throw new Error("master mode is off; run /master on before using aoc_orchestrate");
	if (lease.paneId !== identity.paneId) throw new Error(`master mode is owned by ${lease.paneId}; this pane is ${identity.paneId}`);
	return lease;
}

async function runHerdr(args: string[], cwd: string, maxChars: number, timeoutMs: number, signal?: AbortSignal): Promise<CommandResult> {
	return await runBoundedCommand("herdr", args, {
		cwd,
		maxStdoutChars: maxChars,
		maxStderrChars: Math.min(maxChars, 8000),
		timeoutMs,
		missingMessage: "herdr is not installed or not on PATH, or no Herdr server is reachable. aoc_orchestrate only works inside a Herdr-managed session.",
		signal,
	});
}

function parseAgentInfo(stdout: string): AgentInfo {
	let parsed: unknown;
	try {
		parsed = JSON.parse(stdout);
	} catch {
		throw new Error("could not parse herdr agent info");
	}
	if (!isRecord(parsed) || !isRecord(parsed.result) || !isRecord(parsed.result.agent)) throw new Error("could not parse herdr agent info");
	const agent = parsed.result.agent;
	return {
		agent: String(agent.agent || ""),
		agentStatus: String(agent.agent_status || "unknown"),
		cwd: String(agent.cwd || ""),
		paneId: String(agent.pane_id || ""),
		terminalId: String(agent.terminal_id || ""),
		workspaceId: String(agent.workspace_id || ""),
	};
}

async function getAgentInfo(target: string, cwd: string, maxChars: number, signal?: AbortSignal): Promise<{ info: AgentInfo; command: string; result: CommandResult }> {
	const args = ["agent", "get", target];
	const result = await runHerdr(args, cwd, maxChars, COMMAND_TIMEOUT_MS, signal);
	if (!result.ok) throw new Error("could not parse herdr agent info");
	return { info: parseAgentInfo(result.stdout), command: renderCommand("herdr", args), result };
}

async function ensureTargetReady(params: OrchestrateParamsType, identity: MasterIdentity, cwd: string, maxChars: number, signal?: AbortSignal): Promise<AgentInfo> {
	const target = requireText(params.target, "target");
	let { info } = await getAgentInfo(target, cwd, maxChars, signal);
	if (info.paneId === identity.paneId || target === identity.paneId) throw new Error("refusing to orchestrate this pane; target resolves to the current master pane");
	if (params.requireIdle === false) return info;
	if (info.agentStatus === "idle") return info;
	const waitMs = clampInt(params.waitForIdleMs, DEFAULT_WAIT_FOR_IDLE_MS, 0, MAX_WAIT_FOR_IDLE_MS);
	if (waitMs === 0) throw new Error(`target ${info.paneId || target} is ${info.agentStatus}; assign/send require idle by default`);
	await runHerdr(["agent", "wait", target, "--status", "idle", "--timeout", String(waitMs)], cwd, maxChars, waitMs + COMMAND_TIMEOUT_MS, signal);
	info = (await getAgentInfo(target, cwd, maxChars, signal)).info;
	if (info.agentStatus !== "idle") throw new Error(`target ${info.paneId || target} did not become idle within ${waitMs}ms`);
	return info;
}

function appendEvent(event: OrchestrationEvent): void {
	fs.mkdirSync(stateDir(), { recursive: true });
	fs.appendFileSync(path.join(stateDir(), "events.jsonl"), `${JSON.stringify(event)}\n`);
}

function readEvents(): OrchestrationEvent[] {
	try {
		const text = fs.readFileSync(path.join(stateDir(), "events.jsonl"), "utf8");
		const events: OrchestrationEvent[] = [];
		for (const line of text.split(/\r?\n/)) {
			const trimmed = line.trim();
			if (!trimmed) continue;
			try {
				const parsed: unknown = JSON.parse(trimmed);
				if (!isRecord(parsed)) continue;
				if (parsed.version === 1 || parsed.version === 2) events.push(parsed as OrchestrationEvent);
			} catch {
				// Ignore malformed event lines; the log is best-effort evidence.
			}
		}
		return events;
	} catch {
		return [];
	}
}

function findAssignmentEvent(assignmentId: string): OrchestrationEventV2 | undefined {
	const events = readEvents();
	for (let index = events.length - 1; index >= 0; index -= 1) {
		const event = events[index];
		if (event.version === 2 && event.assignmentId === assignmentId) return event;
	}
	return undefined;
}

function inboxDir(): string {
	return path.join(stateDir(), "inbox");
}

function reportDir(assignmentId: string): string {
	return path.join(inboxDir(), safeName(assignmentId));
}

function reportPath(assignmentId: string): string {
	return path.join(reportDir(assignmentId), "reports.jsonl");
}

function fullRetardPath(identity: MasterIdentity): string {
	return path.join(stateDir(), `${safeName(identity.herdrSession)}-${safeName(identity.workspaceId)}-full-retard.json`);
}

function fullRetardRatePath(identity: MasterIdentity, targetMasterPaneId: string, assignmentId: string): string {
	return path.join(stateDir(), "full-retard-rate", safeName(targetMasterPaneId), `${safeName(assignmentId)}.json`);
}

function truncateField(value: string | undefined, max: number): string | undefined {
	const trimmed = (value ?? "").trim();
	if (!trimmed) return undefined;
	return trimmed.length > max ? trimmed.slice(0, max) : trimmed;
}

function makeReportId(): string {
	return randomUUID();
}

function appendReport(report: MasterReportEventV1): void {
	fs.mkdirSync(reportDir(report.assignmentId), { recursive: true });
	fs.appendFileSync(reportPath(report.assignmentId), `${JSON.stringify(report)}\n`);
}

function readReportsForAssignment(assignmentId: string): MasterReportEventV1[] {
	try {
		const text = fs.readFileSync(reportPath(assignmentId), "utf8");
		const reports: MasterReportEventV1[] = [];
		for (const line of text.split(/\r?\n/)) {
			const trimmed = line.trim();
			if (!trimmed) continue;
			try {
				const parsed: unknown = JSON.parse(trimmed);
				if (!isRecord(parsed) || parsed.version !== 1) continue;
				reports.push(parsed as MasterReportEventV1);
			} catch {
				// Ignore malformed report lines; inbox records are best-effort evidence.
			}
		}
		return reports;
	} catch {
		return [];
	}
}

function writeReportsForAssignment(assignmentId: string, reports: MasterReportEventV1[]): void {
	const file = reportPath(assignmentId);
	fs.mkdirSync(path.dirname(file), { recursive: true });
	const tmp = `${file}.tmp-${process.pid}`;
	let fd: number | null = null;
	try {
		fd = fs.openSync(tmp, "w");
		for (const report of reports) fs.writeFileSync(fd, `${JSON.stringify(report)}\n`);
		fs.closeSync(fd);
		fd = null;
		fs.renameSync(tmp, file);
	} catch (error) {
		if (fd !== null) fs.closeSync(fd);
		try {
			fs.unlinkSync(tmp);
		} catch {
			// best-effort cleanup
		}
		throw error;
	}
}

function readAllReports(limit: number, filters: { assignmentId?: string; status?: ReportStatus; unreadOnly?: boolean; targetMasterPaneId?: string }): MasterReportEventV1[] {
	const boundedLimit = clampInt(limit, REPORT_INBOX_DEFAULT_LIMIT, 1, REPORT_INBOX_MAX_LIMIT);
	const assignmentIds: string[] = [];
	if (filters.assignmentId) {
		assignmentIds.push(filters.assignmentId);
	} else {
		try {
			for (const entry of fs.readdirSync(inboxDir(), { withFileTypes: true })) {
				if (entry.isDirectory()) assignmentIds.push(entry.name);
			}
		} catch {
			return [];
		}
	}
	const reports: MasterReportEventV1[] = [];
	for (const assignmentId of assignmentIds) {
		for (const report of readReportsForAssignment(assignmentId)) {
			if (filters.status && report.status !== filters.status) continue;
			if (filters.unreadOnly === true && report.unread !== true) continue;
			if (filters.targetMasterPaneId && report.targetMasterPaneId !== filters.targetMasterPaneId) continue;
			reports.push(report);
		}
	}
	return reports
		.sort((left, right) => Date.parse(right.timestamp) - Date.parse(left.timestamp))
		.slice(0, boundedLimit);
}

function updateReportsById(reportIds: Set<string>, update: (report: MasterReportEventV1) => MasterReportEventV1): void {
	if (reportIds.size === 0) return;
	let entries: fs.Dirent[];
	try {
		entries = fs.readdirSync(inboxDir(), { withFileTypes: true });
	} catch {
		return;
	}
	for (const entry of entries) {
		if (!entry.isDirectory()) continue;
		const assignmentId = entry.name;
		const reports = readReportsForAssignment(assignmentId);
		let changed = false;
		const next = reports.map((report) => {
			if (!reportIds.has(report.reportId)) return report;
			changed = true;
			return update(report);
		});
		if (changed) writeReportsForAssignment(assignmentId, next);
	}
}

function readFullRetardState(identity: MasterIdentity): FullRetardStateV1 | null {
	let parsed: unknown;
	try {
		parsed = JSON.parse(fs.readFileSync(fullRetardPath(identity), "utf8"));
	} catch {
		return null;
	}
	if (!isRecord(parsed) || parsed.version !== 1) return null;
	if (typeof parsed.herdrSession !== "string" || typeof parsed.workspaceId !== "string" || typeof parsed.masterPaneId !== "string" || typeof parsed.enabled !== "boolean" || typeof parsed.updatedAt !== "string" || typeof parsed.updatedByPaneId !== "string") return null;
	return parsed as FullRetardStateV1;
}

function writeFullRetardState(identity: MasterIdentity, enabled: boolean): FullRetardStateV1 {
	const state: FullRetardStateV1 = {
		version: 1,
		herdrSession: identity.herdrSession,
		workspaceId: identity.workspaceId,
		masterPaneId: identity.paneId,
		enabled,
		updatedAt: new Date().toISOString(),
		updatedByPaneId: identity.paneId,
	};
	fs.mkdirSync(stateDir(), { recursive: true });
	fs.writeFileSync(fullRetardPath(identity), JSON.stringify(state, null, 2));
	return state;
}

function removeFullRetardState(identity: MasterIdentity): void {
	try {
		fs.unlinkSync(fullRetardPath(identity));
	} catch (error) {
		if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
	}
}

function fullRetardEnabledForTarget(identity: MasterIdentity, targetMasterPaneId: string): boolean {
	const lease = readLease(leasePath(identity));
	if (!lease || isExpired(lease, Date.now())) return false;
	if (lease.paneId !== targetMasterPaneId) return false;
	const state = readFullRetardState(identity);
	return state?.enabled === true && state.masterPaneId === targetMasterPaneId;
}

function readFullRetardRateState(identity: MasterIdentity, targetMasterPaneId: string, assignmentId: string): FullRetardRateStateV1 | null {
	let parsed: unknown;
	try {
		parsed = JSON.parse(fs.readFileSync(fullRetardRatePath(identity, targetMasterPaneId, assignmentId), "utf8"));
	} catch {
		return null;
	}
	if (!isRecord(parsed) || parsed.version !== 1) return null;
	if (typeof parsed.herdrSession !== "string" || typeof parsed.workspaceId !== "string" || typeof parsed.masterPaneId !== "string" || typeof parsed.assignmentId !== "string" || typeof parsed.lastPromptAt !== "string") return null;
	if (parsed.lastHeartbeatSummary !== undefined && typeof parsed.lastHeartbeatSummary !== "string") return null;
	return parsed as FullRetardRateStateV1;
}

function writeFullRetardRateState(identity: MasterIdentity, targetMasterPaneId: string, assignmentId: string, heartbeatSummary?: string): void {
	const file = fullRetardRatePath(identity, targetMasterPaneId, assignmentId);
	const previous = readFullRetardRateState(identity, targetMasterPaneId, assignmentId);
	const state: FullRetardRateStateV1 = {
		version: 1,
		herdrSession: identity.herdrSession,
		workspaceId: identity.workspaceId,
		masterPaneId: targetMasterPaneId,
		assignmentId,
		lastPromptAt: new Date().toISOString(),
		lastHeartbeatSummary: heartbeatSummary ?? previous?.lastHeartbeatSummary,
	};
	fs.mkdirSync(path.dirname(file), { recursive: true });
	fs.writeFileSync(file, JSON.stringify(state, null, 2));
}

function messagePreview(message: string, max = 500): string {
	return message.length > max ? message.slice(0, max) : message;
}

function firstTextContent(result: { content?: Array<{ type?: string; text?: string }> }): string {
	return result.content?.find((item) => item.type === "text")?.text ?? "";
}

function assistantResultMarkerFound(transcriptText: string, expectedMarker: string): boolean {
	const marker = "last assistant response:";
	const index = transcriptText.indexOf(marker);
	if (index < 0) return false;
	return transcriptText.slice(index + marker.length).trimStart().startsWith(expectedMarker);
}

function outputSummary(result: CommandResult): string {
	const text = [result.stderr.trim(), result.stdout.trim()].filter(Boolean).join("\n");
	return text || `exit ${result.exitCode}`;
}

function renderAssignMessage(identity: MasterIdentity, assignmentId: string, goal: string, context: string | undefined, constraints: string | undefined, acceptance: string | undefined): string {
	const baseAcceptance = (acceptance ?? "").trim() || "Reply with completion status, changed files if any, verification performed, and blockers. Do not mutate outside the stated scope.";
	return `[MASTER ASSIGNMENT ${assignmentId} from ${identity.paneId}]

Goal:
${goal}

Context:
${(context ?? "").trim() || "(none provided)"}

Constraints:
${(constraints ?? "").trim() || "(none provided)"}

Acceptance:
${baseAcceptance}

Progress reports:
If available, use aoc_report with this assignment id for started, heartbeat, blocked, done, or failed updates. Default deliveryMode=queue records the report for master review without prompting the master. Use deliveryMode=full-retard only when the master assignment or master status explicitly says full-retard is on. aoc_report does not replace the final result marker.

Final reply MUST start with: MASTER RESULT ${assignmentId}`;
}

function renderSendMessage(identity: MasterIdentity, assignmentId: string, message: string): string {
	return `[MASTER MESSAGE ${assignmentId} from ${identity.paneId}]

${message}

If available, use aoc_report with this assignment id for progress or blockers. aoc_report does not replace the final result marker.

If you reply, start with: MASTER RESULT ${assignmentId}`;
}

function renderMasterReportPrompt(report: MasterReportEventV1): string {
	const text = `MASTER INBOX REPORT ${report.assignmentId}
report id: ${report.reportId}
reporter: ${report.reporterPaneId}
status: ${report.status}
needs decision: ${report.needsDecision || "none"}
summary:
${report.summary}
evidence:
${report.evidence || "none"}`;
	return truncateField(text, REPORT_PROMPT_MAX_CHARS) ?? "";
}

function renderMasterReportBatchPrompt(reports: MasterReportEventV1[]): string {
	const blocks = reports.slice(0, FULL_RETARD_BATCH_MAX_REPORTS).map((report) => `assignment id: ${report.assignmentId}
report id: ${report.reportId}
reporter: ${report.reporterPaneId}
status: ${report.status}
needs decision: ${report.needsDecision || "none"}
summary:
${report.summary}
evidence:
${report.evidence || "none"}`);
	const text = `MASTER INBOX REPORT BATCH

${blocks.join("\n\n")}`;
	return truncateField(text, REPORT_PROMPT_MAX_CHARS) ?? "";
}

async function runDraftDelivery(target: string, message: string, cwd: string, maxChars: number, signal?: AbortSignal): Promise<{ command: string; result: CommandResult }> {
	const args = ["agent", "send", target, message];
	return {
		command: renderCommand("herdr", ["agent", "send", target, messagePreview(message)]),
		result: await runHerdr(args, cwd, maxChars, COMMAND_TIMEOUT_MS, signal),
	};
}

async function runSubmitDelivery(info: AgentInfo, message: string, cwd: string, maxChars: number, signal?: AbortSignal): Promise<{ command: string; result: CommandResult }> {
	if (info.agent !== "omp") throw new Error("submit delivery requires an OMP peer; use deliveryMode=draft for non-OMP targets");
	if (!info.paneId) throw new Error("submit delivery requires a resolved Herdr pane id");
	const args = ["pane", "run", info.paneId, message];
	return {
		command: renderCommand("herdr", ["pane", "run", info.paneId, messagePreview(message)]),
		result: await runHerdr(args, cwd, maxChars, COMMAND_TIMEOUT_MS, signal),
	};
}

async function reportToMaster(params: ReportParamsType, reporter: MasterIdentity, cwd: string, maxChars: number, signal?: AbortSignal) {
	const boundedMaxChars = clampMaxChars(params.maxChars, maxChars, MAX_ALLOWED_CHARS);
	const assignmentId = requireText(params.assignmentId, "assignmentId");
	const summary = truncateField(requireText(params.summary, "summary"), REPORT_SUMMARY_MAX_CHARS) ?? "";
	const event = findAssignmentEvent(assignmentId);
	if (!event && !params.targetMaster) {
		return { content: [{ type: "text", text: "report not queued\nassignment not found; targetMaster is required for untracked reports" }], details: { action: "report", ok: false, assignmentId, reason: "assignment not found; targetMaster is required for untracked reports" } };
	}
	const requestedDeliveryMode: ReportDeliveryMode = params.deliveryMode ?? "queue";
	const targetMaster = params.targetMaster ?? event?.masterPaneId ?? "";
	const needsDecision = truncateField(params.needsDecision, REPORT_DECISION_MAX_CHARS);
	const evidence = truncateField(params.evidence, REPORT_EVIDENCE_MAX_CHARS);
	let info: AgentInfo | null = null;
	let resolveWarning: string | undefined;
	try {
		info = (await getAgentInfo(targetMaster, cwd, boundedMaxChars, signal)).info;
	} catch (error) {
		resolveWarning = "target master could not be resolved; queued only";
		if (!event) {
			const reason = error instanceof Error ? error.message : String(error);
			return { content: [{ type: "text", text: `report not queued\n${reason}` }], details: { action: "report", ok: false, assignmentId, reason } };
		}
	}
	const targetMasterPaneId = info?.paneId || event?.masterPaneId || targetMaster;
	let effectiveDeliveryMode: EffectiveReportDeliveryMode = requestedDeliveryMode;
	let warning = resolveWarning;
	if (info?.paneId === reporter.paneId || targetMasterPaneId === reporter.paneId) {
		effectiveDeliveryMode = "queue_self_target";
		warning = "target master resolves to current pane; queued only";
	} else if (!info && requestedDeliveryMode !== "queue") {
		effectiveDeliveryMode = "delivery_failed";
	}
	const reportPromptPreviewSeed: MasterReportEventV1 = {
		version: 1,
		reportId: makeReportId(),
		timestamp: new Date().toISOString(),
		assignmentId,
		reporterPaneId: reporter.paneId,
		reporterWorkspaceId: reporter.workspaceId,
		reporterSession: reporter.herdrSession,
		targetMasterPaneId,
		status: params.status,
		deliveryMode: requestedDeliveryMode,
		effectiveDeliveryMode,
		summary,
		needsDecision,
		evidence,
		unread: true,
		messagePreview: "",
		warning,
	};
	const report: MasterReportEventV1 = { ...reportPromptPreviewSeed, messagePreview: messagePreview(renderMasterReportPrompt(reportPromptPreviewSeed)) };
	appendReport(report);

	const details: Record<string, unknown> = { action: "report", ok: true, assignmentId, reportId: report.reportId, status: report.status, deliveryMode: requestedDeliveryMode, effectiveDeliveryMode, targetMasterPaneId, warning };
	const lines = [
		`assignment id: ${assignmentId}`,
		`report id: ${report.reportId}`,
		`status: queued`,
		`delivery mode: ${requestedDeliveryMode}`,
		`effective delivery mode: ${effectiveDeliveryMode}`,
		`target master: ${targetMasterPaneId}`,
	];
	if (warning) lines.push(`warning: ${warning}`);
	const finish = (ok = true) => ({ content: [{ type: "text", text: lines.join("\n") }], details: { ...details, ok } });

	if (requestedDeliveryMode === "queue" || effectiveDeliveryMode === "queue_self_target" || !info) return finish(effectiveDeliveryMode !== "delivery_failed");
	if (requestedDeliveryMode === "notify") {
		const delivered = await runDraftDelivery(targetMasterPaneId, renderMasterReportPrompt(report), cwd, boundedMaxChars, signal);
		details.command = delivered.command;
		details.exitCode = delivered.result.exitCode;
		if (delivered.result.ok) {
			const promptedAt = new Date().toISOString();
			updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, promptedAt, promptMode: "notify", effectiveDeliveryMode: "notify" }));
			lines[4] = "effective delivery mode: notify";
			lines.push("prompt: drafted");
			details.effectiveDeliveryMode = "notify";
			return finish();
		}
		updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, effectiveDeliveryMode: "delivery_failed", warning: outputSummary(delivered.result) }));
		lines[4] = "effective delivery mode: delivery_failed";
		lines.push(`warning: ${outputSummary(delivered.result)}`);
		details.effectiveDeliveryMode = "delivery_failed";
		details.warning = outputSummary(delivered.result);
		return finish(false);
	}
	if (requestedDeliveryMode === "submit") {
		if (!fullRetardEnabledForTarget(reporter, targetMasterPaneId)) {
			updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, effectiveDeliveryMode: "queue_disabled", warning: "full-retard is off for target master" }));
			lines[4] = "effective delivery mode: queue_disabled";
			lines.push("warning: full-retard is off for target master");
			details.effectiveDeliveryMode = "queue_disabled";
			details.warning = "full-retard is off for target master";
			return finish();
		}
		const delivered = await runSubmitDelivery(info, renderMasterReportPrompt(report), cwd, boundedMaxChars, signal);
		details.command = delivered.command;
		details.exitCode = delivered.result.exitCode;
		if (delivered.result.ok) {
			const promptedAt = new Date().toISOString();
			updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, promptedAt, promptMode: "submit", effectiveDeliveryMode: "submit" }));
			lines[4] = "effective delivery mode: submit";
			lines.push("prompt: submitted");
			details.effectiveDeliveryMode = "submit";
			return finish();
		}
		updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, effectiveDeliveryMode: "delivery_failed", warning: outputSummary(delivered.result) }));
		lines[4] = "effective delivery mode: delivery_failed";
		lines.push(`warning: ${outputSummary(delivered.result)}`);
		details.effectiveDeliveryMode = "delivery_failed";
		details.warning = outputSummary(delivered.result);
		return finish(false);
	}
	if (!fullRetardEnabledForTarget(reporter, targetMasterPaneId)) {
		updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, effectiveDeliveryMode: "queue_disabled", warning: "full-retard is off for target master" }));
		lines[4] = "effective delivery mode: queue_disabled";
		lines.push("warning: full-retard is off for target master");
		details.effectiveDeliveryMode = "queue_disabled";
		details.warning = "full-retard is off for target master";
		return finish();
	}
	const rate = readFullRetardRateState(reporter, targetMasterPaneId, assignmentId);
	if (params.status === "heartbeat" && rate?.lastHeartbeatSummary === summary) {
		updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, effectiveDeliveryMode: "queue_rate_limited", warning: "duplicate heartbeat suppressed" }));
		lines[4] = "effective delivery mode: queue_rate_limited";
		lines.push("warning: duplicate heartbeat suppressed");
		details.effectiveDeliveryMode = "queue_rate_limited";
		details.warning = "duplicate heartbeat suppressed";
		return finish();
	}
	if (rate && Date.now() - Date.parse(rate.lastPromptAt) < FULL_RETARD_MIN_INTERVAL_MS) {
		updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, effectiveDeliveryMode: "queue_rate_limited", warning: "full-retard prompt suppressed by rate limit" }));
		lines[4] = "effective delivery mode: queue_rate_limited";
		lines.push("warning: full-retard prompt suppressed by rate limit");
		details.effectiveDeliveryMode = "queue_rate_limited";
		details.warning = "full-retard prompt suppressed by rate limit";
		return finish();
	}
	const selected = readAllReports(REPORT_INBOX_MAX_LIMIT, { unreadOnly: true, targetMasterPaneId })
		.filter((candidate) => !candidate.promptedAt)
		.slice(0, FULL_RETARD_BATCH_MAX_REPORTS)
		.reverse();
	if (!selected.some((candidate) => candidate.reportId === report.reportId)) selected.push(report);
	const reportsToSubmit = selected.slice(-FULL_RETARD_BATCH_MAX_REPORTS);
	const prompt = reportsToSubmit.length === 1 ? renderMasterReportPrompt(reportsToSubmit[0]) : renderMasterReportBatchPrompt(reportsToSubmit);
	const delivered = await runSubmitDelivery(info, prompt, cwd, boundedMaxChars, signal);
	details.command = delivered.command;
	details.exitCode = delivered.result.exitCode;
	if (delivered.result.ok) {
		const promptedAt = new Date().toISOString();
		const selectedIds = new Set(reportsToSubmit.map((selectedReport) => selectedReport.reportId));
		updateReportsById(selectedIds, (stored) => ({ ...stored, promptedAt, promptMode: "full-retard", effectiveDeliveryMode: "full-retard" }));
		writeFullRetardRateState(reporter, targetMasterPaneId, assignmentId, params.status === "heartbeat" ? summary : undefined);
		lines[4] = "effective delivery mode: full-retard";
		lines.push(`prompt: submitted ${reportsToSubmit.length === 1 ? "single report" : `${reportsToSubmit.length} reports`}`);
		details.effectiveDeliveryMode = "full-retard";
		details.submittedReports = reportsToSubmit.map((selectedReport) => selectedReport.reportId);
		return finish();
	}
	updateReportsById(new Set([report.reportId]), (stored) => ({ ...stored, effectiveDeliveryMode: "delivery_failed", warning: outputSummary(delivered.result) }));
	lines[4] = "effective delivery mode: delivery_failed";
	lines.push(`warning: ${outputSummary(delivered.result)}`);
	details.effectiveDeliveryMode = "delivery_failed";
	details.warning = outputSummary(delivered.result);
	return finish(false);
}

async function waitForPeerStart(target: string, cwd: string, maxChars: number, startWaitMs: number, signal?: AbortSignal): Promise<{ status: "skipped" | "started" | "start_unconfirmed"; text: string }> {
	if (startWaitMs === 0) return { status: "skipped", text: "skipped" };
	const result = await runHerdr(["agent", "wait", target, "--status", "working", "--timeout", String(startWaitMs)], cwd, maxChars, startWaitMs + COMMAND_TIMEOUT_MS, signal);
	if (result.ok) return { status: "started", text: "started" };
	const summary = outputSummary(result);
	if (summary.includes("timed out waiting for agent status change")) return { status: "start_unconfirmed", text: `start_unconfirmed after ${startWaitMs}ms` };
	return { status: "start_unconfirmed", text: `start_unconfirmed: ${summary}` };
}

async function sendToPeer(action: "assign" | "send", params: OrchestrateParamsType, identity: MasterIdentity, cwd: string, maxChars: number, signal?: AbortSignal) {
	requireActiveLease(identity);
	const target = requireText(params.target, "target");
	const info = await ensureTargetReady(params, identity, cwd, maxChars, signal);
	const assignmentId = makeAssignmentId();
	const deliveryMode = params.deliveryMode ?? "draft";
	const message = action === "assign"
		? renderAssignMessage(identity, assignmentId, requireText(params.goal, "goal"), params.context, params.constraints, params.acceptance)
		: renderSendMessage(identity, assignmentId, requireText(params.message, "message"));
	let command = "";
	let result: CommandResult;
	try {
		const delivered = deliveryMode === "draft"
			? await runDraftDelivery(target, message, cwd, maxChars, signal)
			: await runSubmitDelivery(info, message, cwd, maxChars, signal);
		command = delivered.command;
		result = delivered.result;
	} catch (error) {
		result = { ok: false, exitCode: null, stdout: "", stderr: error instanceof Error ? error.message : String(error), timedOut: false, truncated: false };
	}
	if (!command) {
		command = deliveryMode === "draft"
			? renderCommand("herdr", ["agent", "send", target, messagePreview(message)])
			: renderCommand("herdr", ["pane", "run", info.paneId || target, messagePreview(message)]);
	}
	let startAck: { status: "skipped" | "started" | "start_unconfirmed"; text: string } | undefined;
	if (result.ok && deliveryMode === "submit") {
		startAck = await waitForPeerStart(info.paneId || target, cwd, maxChars, clampInt(params.startWaitMs, DEFAULT_START_WAIT_MS, 0, MAX_START_WAIT_MS), signal);
	}
	const preview = messagePreview(message);
	const deliveryStatus: "drafted" | "submitted" | "delivery_failed" = result.ok ? (deliveryMode === "draft" ? "drafted" : "submitted") : "delivery_failed";
	const expectedMarker = `MASTER RESULT ${assignmentId}`;
	const details: Record<string, unknown> = { action, ok: result.ok, target, resolvedTarget: info.paneId, assignmentId, deliveryMode, status: deliveryStatus, command, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated, startStatus: startAck?.status };
	const lines = [
		`$ ${command}`,
		`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
		"",
		`action: ${action}`,
		`assignment id: ${assignmentId}`,
		`delivery mode: ${deliveryMode}`,
		`target: ${info.paneId}`,
		`status before send: ${info.agentStatus}`,
		`status: ${deliveryStatus}`,
	];
	if (startAck) lines.push(`start acknowledgement: ${startAck.text}`);
	if (result.ok && deliveryMode === "draft") lines.push("next: review the peer pane and press Enter there to submit, or rerun with deliveryMode=submit for autonomous execution");
	lines.push("message preview:", preview);
	if (!result.ok) lines.push("", `Herdr ${deliveryMode} delivery did not complete successfully. Treat this as unavailable orchestration; use aoc_herdr read/transcript to inspect target state before retrying.`);
	try {
		appendEvent({ version: 2, timestamp: new Date().toISOString(), assignmentId, action, deliveryMode, masterPaneId: identity.paneId, target, resolvedTarget: info.paneId, ok: result.ok, exitCode: result.exitCode, status: deliveryStatus, startStatus: startAck?.status, command, messagePreview: preview, expectedMarker });
	} catch (error) {
		const eventLogError = error instanceof Error ? error.message : String(error);
		details.eventLogError = eventLogError;
		lines.push("", `event log write failed: ${eventLogError}`);
	}
	return { content: [{ type: "text", text: lines.join("\n") }], details };
}

async function waitForPeerIdle(target: string, cwd: string, maxChars: number, resultTimeoutMs: number, signal?: AbortSignal): Promise<string> {
	if (resultTimeoutMs === 0) return "not requested";
	const result = await runHerdr(["agent", "wait", target, "--status", "idle", "--timeout", String(resultTimeoutMs)], cwd, maxChars, resultTimeoutMs + COMMAND_TIMEOUT_MS, signal);
	if (result.ok) return "peer reached idle";
	const summary = outputSummary(result);
	if (summary.includes("timed out waiting for agent status change")) return `peer still not idle after ${resultTimeoutMs}ms`;
	return `idle wait unconfirmed: ${summary}`;
}

async function collectAssignment(params: OrchestrateParamsType, identity: MasterIdentity, cwd: string, maxChars: number, signal?: AbortSignal) {
	requireActiveLease(identity);
	const assignmentId = requireText(params.assignmentId, "assignmentId");
	const event = findAssignmentEvent(assignmentId);
	if (!event) return { content: [{ type: "text", text: `assignment ${assignmentId} was not found in ${stateDir()}/events.jsonl` }], details: { action: "collect", ok: false, assignmentId, reason: "assignment not found" } };
	const resultTimeoutMs = clampInt(params.resultTimeoutMs, DEFAULT_RESULT_TIMEOUT_MS, 0, MAX_RESULT_TIMEOUT_MS);
	const lines = [
		`assignment id: ${assignmentId}`,
		`target: ${event.resolvedTarget}`,
		`delivery mode: ${event.deliveryMode}`,
		`expected marker: ${event.expectedMarker}`,
	];
	if (resultTimeoutMs > 0) lines.push(`idle wait: ${await waitForPeerIdle(event.resolvedTarget, cwd, maxChars, resultTimeoutMs, signal)}`);
	const transcript = await runTranscript({ action: "transcript", target: event.resolvedTarget, session: "latest", scope: "last", maxChars }, cwd, maxChars, signal);
	const transcriptText = firstTextContent(transcript);
	const markerFound = assistantResultMarkerFound(transcriptText, event.expectedMarker);
	lines.push(markerFound ? "result marker found" : "result marker not found", "", "last assistant response:", transcriptText);
	if (!markerFound || transcript.details?.ok === false) {
		const fallback = await runHerdr(["agent", "read", event.resolvedTarget, "--source", "recent", "--lines", "120", "--format", "text"], cwd, maxChars, COMMAND_TIMEOUT_MS, signal);
		lines.push("", "pane fallback:", fallback.ok ? fallback.stdout : outputSummary(fallback));
	}
	return {
		content: [{ type: "text", text: lines.join("\n") }],
		details: { action: "collect", ok: markerFound, assignmentId, target: event.resolvedTarget, deliveryMode: event.deliveryMode, markerFound },
	};
}

function reportSummaryPreview(value: string): string {
	return messagePreview(value.replace(/\s+/g, " "), 220);
}

function formatInboxReportLine(report: MasterReportEventV1): string {
	return `${report.timestamp} ${report.reportId} assignment=${report.assignmentId} reporter=${report.reporterPaneId} status=${report.status} unread=${report.unread} mode=${report.deliveryMode}/${report.effectiveDeliveryMode} summary=${reportSummaryPreview(report.summary)}`;
}

function findReportById(reportId: string, targetMasterPaneId: string): MasterReportEventV1 | undefined {
	try {
		for (const entry of fs.readdirSync(inboxDir(), { withFileTypes: true })) {
			if (!entry.isDirectory()) continue;
			for (const report of readReportsForAssignment(entry.name)) {
				if (report.reportId === reportId && report.targetMasterPaneId === targetMasterPaneId) return report;
			}
		}
	} catch {
		return undefined;
	}
	return undefined;
}

function renderIngestReport(report: MasterReportEventV1, ingestMode: "summary" | "full"): string {
	const evidence = ingestMode === "full" ? (report.evidence || "none") : (messagePreview((report.evidence || "none").replace(/\s+/g, " "), 500));
	return [
		`report id: ${report.reportId}`,
		`assignment id: ${report.assignmentId}`,
		`timestamp: ${report.timestamp}`,
		`reporter: ${report.reporterPaneId}`,
		`status: ${report.status}`,
		`unread: ${report.unread}`,
		`mode: ${report.deliveryMode}/${report.effectiveDeliveryMode}`,
		`needs decision: ${report.needsDecision || "none"}`,
		"summary:",
		ingestMode === "full" ? report.summary : reportSummaryPreview(report.summary),
		"evidence:",
		evidence,
	].join("\n");
}

function ackReports(params: OrchestrateParamsType, targetMasterPaneId: string): number {
	let changed = 0;
	if (params.reportId) {
		try {
			for (const entry of fs.readdirSync(inboxDir(), { withFileTypes: true })) {
				if (!entry.isDirectory()) continue;
				const reports = readReportsForAssignment(entry.name);
				let touched = false;
				const next = reports.map((report) => {
					if (report.reportId !== params.reportId || report.targetMasterPaneId !== targetMasterPaneId || report.unread !== true) return report;
					changed += 1;
					touched = true;
					return { ...report, unread: false };
				});
				if (touched) writeReportsForAssignment(entry.name, next);
			}
		} catch {
			return changed;
		}
		return changed;
	}
	const assignmentId = requireText(params.assignmentId, "assignmentId");
	const reports = readReportsForAssignment(assignmentId);
	let touched = false;
	const next = reports.map((report) => {
		if (report.targetMasterPaneId !== targetMasterPaneId || report.unread !== true) return report;
		if (params.status && report.status !== params.status) return report;
		changed += 1;
		touched = true;
		return { ...report, unread: false };
	});
	if (touched) writeReportsForAssignment(assignmentId, next);
	return changed;
}

function masterPrompt(mode: "on" | "off" | "status" | "full-retard-on" | "full-retard-off" | "full-retard-status", ttlMinutes?: number): string {
	if (mode === "on") return `Enable AOC master mode for this Herdr workspace.

Call aoc_orchestrate with action=master_on and ttlMinutes=${ttlMinutes}. Then report the returned lease owner and expiry. After master mode is enabled, use aoc_herdr for observation and aoc_orchestrate for explicit peer assignments/messages only.`;
	if (mode === "off") return `Disable AOC master mode for this Herdr workspace.

Call aoc_orchestrate with action=master_off. Then report whether this pane released the lease or why it could not.`;
	if (mode === "full-retard-on") return `Enable AOC full-retard reporting for this Herdr workspace.

Call aoc_orchestrate with action=full_retard_on. Then report the returned owner pane, workspace, and timestamp.`;
	if (mode === "full-retard-off") return `Disable AOC full-retard reporting for this Herdr workspace.

Call aoc_orchestrate with action=full_retard_off. Then report whether full-retard is off.`;
	if (mode === "full-retard-status") return `Show AOC full-retard reporting status for this Herdr workspace.

Call aoc_orchestrate with action=full_retard_status. Report whether full-retard is on or off for this master pane.`;
	return `Show AOC master mode status for this Herdr workspace.

Call aoc_orchestrate with action=master_status. Report whether this pane owns the active lease, another pane owns it, or master mode is off.`;
}

export default function aocMasterExtension(pi: CommandExtensionAPI): void {
	pi.registerCommand("master", {
		description: "Usage: /master on [minutes], /master off, /master status, or /master full-retard on|off|status. Enable gated AOC master orchestration for this Herdr workspace.",
		getArgumentCompletions: (_prefix: string): AutocompleteItem[] | null => [
			{ value: "on 30", label: "on 30", description: "Enable master mode for 30 minutes" },
			{ value: "on 60", label: "on 60", description: "Enable master mode for 60 minutes" },
			{ value: "on 240", label: "on 240", description: "Enable master mode for 240 minutes" },
			{ value: "status", label: "status", description: "Show current master-mode lease" },
			{ value: "off", label: "off", description: "Disable master mode for this pane" },
			{ value: "full-retard on", label: "full-retard on", description: "Enable bounded worker-to-master report prompts" },
			{ value: "full-retard off", label: "full-retard off", description: "Disable worker-to-master report prompts" },
			{ value: "full-retard status", label: "full-retard status", description: "Show full-retard report prompt status" },
		],
		handler: async (args, ctx) => {
			const parts = argsText(args).split(/\s+/).filter(Boolean);
			const rawMode = parts[0]?.toLowerCase() || "status";
			let mode: "on" | "off" | "status" | "full-retard-on" | "full-retard-off" | "full-retard-status";
			if (rawMode === "on" || rawMode === "enable") mode = "on";
			else if (rawMode === "off" || rawMode === "disable") mode = "off";
			else if (rawMode === "status") mode = "status";
			else if (rawMode === "full-retard") {
				const submode = parts[1]?.toLowerCase() || "status";
				if (submode === "on" || submode === "enable") mode = "full-retard-on";
				else if (submode === "off" || submode === "disable") mode = "full-retard-off";
				else if (submode === "status") mode = "full-retard-status";
				else {
					const message = `Unknown full-retard mode '${submode}'. Use /master full-retard on, /master full-retard off, or /master full-retard status.`;
					await ctx.ui?.notify?.(message, "error");
					throw new Error(message);
				}
			}
			else {
				const message = `Unknown master mode '${rawMode}'. Use /master on [minutes], /master off, /master status, or /master full-retard status.`;
				await ctx.ui?.notify?.(message, "error");
				throw new Error(message);
			}
			const ttlMinutes = mode === "on" ? clampInt(parts[1] === undefined ? undefined : Number.parseInt(parts[1], 10), DEFAULT_MASTER_TTL_MINUTES, MIN_MASTER_TTL_MINUTES, MAX_MASTER_TTL_MINUTES) : undefined;
			const content = masterPrompt(mode, ttlMinutes);
			if (typeof pi.sendMessage === "function") {
				await pi.sendMessage(
					{ customType: "aoc.master.request", display: true, content, details: { mode, ttlMinutes, cwd: ctx.cwd } },
					{ triggerTurn: true },
				);
				return;
			}
			await ctx.ui?.notify?.(content, "info");
		},
	});

	pi.registerTool({
		name: "aoc_report",
		label: "AOC Report",
		description: "Worker-to-master reporting for AOC assignments. Default deliveryMode=queue appends a bounded report to the master inbox without prompting the master. notify drafts a bounded master prompt. submit and full-retard require full-retard master mode.",
		promptSnippet: "Use aoc_report for progress/blockers/results during MASTER ASSIGNMENT. Final completion still requires MASTER RESULT <assignmentId>. Default queue does not prompt master; full-retard is explicit and bounded.",
		promptGuidelines: [
			"Use aoc_report for started, heartbeat, blocked, done, or failed updates during MASTER ASSIGNMENT.",
			"Final completion still requires the exact MASTER RESULT <assignmentId> marker in your final assistant reply; aoc_report does not replace collect.",
			"Default deliveryMode=queue records the report for master review without prompting the master.",
			"deliveryMode=notify drafts a bounded prompt to the master; deliveryMode=submit and deliveryMode=full-retard require the master-owned full-retard toggle.",
			"full-retard is explicit and bounded; it permits worker-to-master report prompts only, not arbitrary Herdr operations.",
		],
		parameters: ReportParams,
		async execute(_toolCallId, params: ReportParamsType, signal, _onUpdate, ctx) {
			const projectRoot = findProjectRoot(ctx.cwd);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const identity = currentIdentity(ctx.cwd);
			try {
				return await reportToMaster(params, identity, projectRoot, maxChars, signal);
			} catch (error) {
				const reason = error instanceof Error ? error.message : String(error);
				return { content: [{ type: "text", text: reason }], details: { action: "report", ok: false, reason } };
			}
		},
	});

	pi.registerTool({
		name: "aoc_orchestrate",
		label: "AOC Orchestrate",
		description: "Gated Herdr orchestration for master agents: enable/disable/status master mode, full_retard_on/off/status, draft reviewable peer prompts by default, explicitly submit OMP peer turns, collect assignment results, and manage queued reports with inbox/ingest/ack. Mutating actions require /master on and an active lease owned by this pane.",
		promptSnippet: "Use /master on before aoc_orchestrate assign/send/collect/inbox. Default deliveryMode=draft preserves operator review. full-retard is off by default; enable it explicitly before workers may submit report prompts back to the master.",
		promptGuidelines: [
			"Use aoc_orchestrate only after /master on has enabled master mode for this pane; otherwise master_on is the only mutating setup action you may call.",
			"Use aoc_herdr list/get/transcript/read before assigning work so targets, status, and current context are grounded.",
			"assign/send reject busy peers by default. Do not set requireIdle=false unless the user explicitly asks to interrupt or the peer is known to be waiting for master input.",
			"Default assign/send deliveryMode=draft uses Herdr agent send and does not submit the peer turn; report it as awaiting operator submit.",
			"deliveryMode=submit is allowed only for resolved OMP peers; it uses Herdr pane run to submit the prepared assignment as one peer turn. Never expose arbitrary shell commands, keystrokes, focus, close, move, resize, spawn, or broadcast actions.",
			"After submit, use start acknowledgement and collect with the assignment id; after draft, use aoc_herdr read/transcript or operator confirmation before assuming the peer executed anything.",
			"full-retard is a master-owned toggle; it is off by default and disabled by /master off.",
			"Use inbox/ingest/ack for queued aoc_report records; inbox lists bounded summaries, ingest pulls selected content, ack marks read.",
			"Workers remain executors; full-retard only permits bounded worker-to-master report prompts, not arbitrary Herdr operations.",
		],
		parameters: OrchestrateParams,
		async execute(_toolCallId, params: OrchestrateParamsType, signal, _onUpdate, ctx) {
			const projectRoot = findProjectRoot(ctx.cwd);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const identity = currentIdentity(ctx.cwd);
			if (params.action === "master_on") {
				const ttl = clampInt(params.ttlMinutes, DEFAULT_MASTER_TTL_MINUTES, MIN_MASTER_TTL_MINUTES, MAX_MASTER_TTL_MINUTES);
				const result = acquireLease(identity, ttl);
				if (result.ok) {
					return {
						content: [{ type: "text", text: `${result.refreshed ? "master mode refreshed" : "master mode enabled"}\nowner: ${result.lease.paneId}\nworkspace: ${result.lease.workspaceId}\nsession: ${result.lease.herdrSession}\nexpires: ${result.lease.expiresAt}` }],
						details: { action: "master_on", ok: true, lease: result.lease, refreshed: result.refreshed },
					};
				}
				return { content: [{ type: "text", text: `master mode unavailable\n${result.reason}` }], details: { action: "master_on", ok: false, reason: result.reason, lease: result.lease } };
			}
			if (params.action === "master_off") {
				const result = releaseLease(identity);
				let cleanupError: string | undefined;
				if (result.ok) {
					try {
						removeFullRetardState(identity);
					} catch (error) {
						cleanupError = error instanceof Error ? error.message : String(error);
					}
				}
				const cleanupText = cleanupError ? `\nfull-retard cleanup failed: ${cleanupError}` : "";
				return { content: [{ type: "text", text: `${result.reason === "master mode disabled" ? "master mode disabled" : "master mode unchanged"}\n${result.reason}${cleanupText}` }], details: { action: "master_off", ok: result.ok, reason: result.reason, fullRetardCleanupError: cleanupError } };
			}
			if (params.action === "master_status") {
				const lease = readLease(leasePath(identity));
				const active = lease && !isExpired(lease, Date.now()) ? lease : null;
				if (!active) return { content: [{ type: "text", text: "master mode off" }], details: { action: "master_status", ok: true, owner: null, ownedByThisPane: false } };
				const ownedByThisPane = active.paneId === identity.paneId;
				return {
					content: [{ type: "text", text: ownedByThisPane ? `master mode on\nowner: ${active.paneId}\nexpires: ${active.expiresAt}` : `master mode owned by ${active.paneId}\nexpires: ${active.expiresAt}` }],
					details: { action: "master_status", ok: true, owner: active.paneId, ownedByThisPane },
				};
			}
			if (params.action === "full_retard_on") {
				try {
					requireActiveLease(identity);
					const state = writeFullRetardState(identity, true);
					return { content: [{ type: "text", text: `full-retard on\nowner: ${state.masterPaneId}\nworkspace: ${state.workspaceId}\ntimestamp: ${state.updatedAt}` }], details: { action: "full_retard_on", ok: true, state } };
				} catch (error) {
					const reason = error instanceof Error ? error.message : String(error);
					return { content: [{ type: "text", text: reason }], details: { action: "full_retard_on", ok: false, reason } };
				}
			}
			if (params.action === "full_retard_off") {
				try {
					requireActiveLease(identity);
					removeFullRetardState(identity);
					return { content: [{ type: "text", text: "full-retard off" }], details: { action: "full_retard_off", ok: true } };
				} catch (error) {
					const reason = error instanceof Error ? error.message : String(error);
					return { content: [{ type: "text", text: reason }], details: { action: "full_retard_off", ok: false, reason } };
				}
			}
			if (params.action === "full_retard_status") {
				try {
					requireActiveLease(identity);
					const enabled = fullRetardEnabledForTarget(identity, identity.paneId);
					const state = readFullRetardState(identity);
					return { content: [{ type: "text", text: enabled ? `full-retard on\nowner: ${identity.paneId}\nworkspace: ${identity.workspaceId}\ntimestamp: ${state?.updatedAt ?? "unknown"}` : "full-retard off" }], details: { action: "full_retard_status", ok: true, enabled, state } };
				} catch (error) {
					const reason = error instanceof Error ? error.message : String(error);
					return { content: [{ type: "text", text: reason }], details: { action: "full_retard_status", ok: false, reason } };
				}
			}
			if (params.action === "inbox") {
				try {
					requireActiveLease(identity);
					const limit = clampInt(params.limit, REPORT_INBOX_DEFAULT_LIMIT, 1, REPORT_INBOX_MAX_LIMIT);
					const unreadOnly = params.unreadOnly ?? true;
					const reports = readAllReports(limit, { assignmentId: params.assignmentId, status: params.status, unreadOnly, targetMasterPaneId: identity.paneId });
					const text = reports.length ? reports.map(formatInboxReportLine).join("\n") : "inbox empty";
					return { content: [{ type: "text", text }], details: { action: "inbox", ok: true, count: reports.length, unreadOnly, limit } };
				} catch (error) {
					const reason = error instanceof Error ? error.message : String(error);
					return { content: [{ type: "text", text: reason }], details: { action: "inbox", ok: false, reason } };
				}
			}
			if (params.action === "ingest") {
				try {
					requireActiveLease(identity);
					const ingestMode = params.ingestMode ?? "summary";
					let report: MasterReportEventV1 | undefined;
					if (params.reportId) {
						report = findReportById(params.reportId, identity.paneId);
					} else {
						const assignmentId = requireText(params.assignmentId, "assignmentId");
						report = readAllReports(1, { assignmentId, unreadOnly: true, targetMasterPaneId: identity.paneId })[0]
							?? readAllReports(1, { assignmentId, targetMasterPaneId: identity.paneId })[0];
					}
					if (!report) return { content: [{ type: "text", text: "report not found" }], details: { action: "ingest", ok: false, reportId: params.reportId, assignmentId: params.assignmentId } };
					return { content: [{ type: "text", text: renderIngestReport(report, ingestMode) }], details: { action: "ingest", ok: true, reportId: report.reportId, assignmentId: report.assignmentId, ingestMode } };
				} catch (error) {
					const reason = error instanceof Error ? error.message : String(error);
					return { content: [{ type: "text", text: reason }], details: { action: "ingest", ok: false, reason } };
				}
			}
			if (params.action === "ack") {
				try {
					requireActiveLease(identity);
					if (!params.reportId && !params.assignmentId) throw new Error("ack requires reportId or assignmentId");
					const changed = ackReports(params, identity.paneId);
					return { content: [{ type: "text", text: `ack changed: ${changed}` }], details: { action: "ack", ok: true, changed } };
				} catch (error) {
					const reason = error instanceof Error ? error.message : String(error);
					return { content: [{ type: "text", text: reason }], details: { action: "ack", ok: false, reason } };
				}
			}
			if (params.action === "collect") {
				try {
					return await collectAssignment(params, identity, projectRoot, maxChars, signal);
				} catch (error) {
					const reason = error instanceof Error ? error.message : String(error);
					return { content: [{ type: "text", text: reason }], details: { action: params.action, ok: false, reason } };
				}
			}
			if (params.action !== "assign" && params.action !== "send") return { content: [{ type: "text", text: `unsupported action: ${params.action}` }], details: { action: params.action, ok: false, reason: "unsupported action" } };
			try {
				return await sendToPeer(params.action, params, identity, projectRoot, maxChars, signal);
			} catch (error) {
				const reason = error instanceof Error ? error.message : String(error);
				return { content: [{ type: "text", text: reason }], details: { action: params.action, ok: false, reason } };
			}
		},
	});
}
