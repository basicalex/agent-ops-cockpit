import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { clampInt, clampMaxChars, findProjectRoot, renderCommand, runBoundedCommand } from "./aoc-runtime";
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

type OrchestrationEvent = {
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

const OrchestrateActionSchema = StringEnum(
	["master_on", "master_off", "master_status", "assign", "send"] as const,
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
	ttlMinutes: Type.Optional(Type.Integer({ minimum: MIN_MASTER_TTL_MINUTES, maximum: MAX_MASTER_TTL_MINUTES, description: "Master lease TTL for action=master_on. Defaults to 60 minutes." })),
	requireIdle: Type.Optional(Type.Boolean({ description: "For assign/send, reject non-idle peers unless false. Defaults to true." })),
	waitForIdleMs: Type.Optional(Type.Integer({ minimum: 0, maximum: MAX_WAIT_FOR_IDLE_MS, description: "For assign/send, wait this long for the peer to become idle before rejecting. Defaults to 0." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});

type OrchestrateParamsType = {
	action: "master_on" | "master_off" | "master_status" | "assign" | "send";
	target?: string;
	message?: string;
	goal?: string;
	context?: string;
	constraints?: string;
	acceptance?: string;
	ttlMinutes?: number;
	requireIdle?: boolean;
	waitForIdleMs?: number;
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

function messagePreview(message: string, max = 500): string {
	return message.length > max ? message.slice(0, max) : message;
}

function renderAssignMessage(identity: MasterIdentity, goal: string, context: string | undefined, constraints: string | undefined, acceptance: string | undefined): string {
	return `[MASTER ASSIGNMENT from ${identity.paneId}]

Goal:
${goal}

Context:
${(context ?? "").trim() || "(none provided)"}

Constraints:
${(constraints ?? "").trim() || "(none provided)"}

Acceptance:
${(acceptance ?? "").trim() || "Reply with completion status, changed files if any, verification performed, and blockers. Do not mutate outside the stated scope."}`;
}

function renderSendMessage(identity: MasterIdentity, message: string): string {
	return `[MASTER MESSAGE from ${identity.paneId}]

${message}`;
}

async function sendToPeer(action: "assign" | "send", params: OrchestrateParamsType, identity: MasterIdentity, cwd: string, maxChars: number, signal?: AbortSignal) {
	requireActiveLease(identity);
	const target = requireText(params.target, "target");
	const info = await ensureTargetReady(params, identity, cwd, maxChars, signal);
	const message = action === "assign"
		? renderAssignMessage(identity, requireText(params.goal, "goal"), params.context, params.constraints, params.acceptance)
		: renderSendMessage(identity, requireText(params.message, "message"));
	const args = ["agent", "send", target, message];
	let result: CommandResult;
	try {
		result = await runHerdr(args, cwd, maxChars, COMMAND_TIMEOUT_MS, signal);
	} catch (error) {
		result = { ok: false, exitCode: null, stdout: "", stderr: error instanceof Error ? error.message : String(error), timedOut: false, truncated: false };
	}
	const command = renderCommand("herdr", ["agent", "send", target, messagePreview(message)]);
	const preview = messagePreview(message);
	const details: Record<string, unknown> = { action, ok: result.ok, target, resolvedTarget: info.paneId, command, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated };
	const lines = [
		`$ ${command}`,
		`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
		"",
		`action: ${action}`,
		`target: ${info.paneId}`,
		`status before send: ${info.agentStatus}`,
		"message preview:",
		preview,
	];
	if (!result.ok) lines.push("", "Herdr send did not complete successfully. Treat this as unavailable orchestration; use aoc_herdr read/transcript to inspect target state before retrying.");
	try {
		appendEvent({ version: 1, timestamp: new Date().toISOString(), action, masterPaneId: identity.paneId, target, resolvedTarget: info.paneId, ok: result.ok, exitCode: result.exitCode, messagePreview: preview });
	} catch (error) {
		const eventLogError = error instanceof Error ? error.message : String(error);
		details.eventLogError = eventLogError;
		lines.push("", `event log write failed: ${eventLogError}`);
	}
	return { content: [{ type: "text", text: lines.join("\n") }], details };
}

function masterPrompt(mode: "on" | "off" | "status", ttlMinutes?: number): string {
	if (mode === "on") return `Enable AOC master mode for this Herdr workspace.

Call aoc_orchestrate with action=master_on and ttlMinutes=${ttlMinutes}. Then report the returned lease owner and expiry. After master mode is enabled, use aoc_herdr for observation and aoc_orchestrate for explicit peer assignments/messages only.`;
	if (mode === "off") return `Disable AOC master mode for this Herdr workspace.

Call aoc_orchestrate with action=master_off. Then report whether this pane released the lease or why it could not.`;
	return `Show AOC master mode status for this Herdr workspace.

Call aoc_orchestrate with action=master_status. Report whether this pane owns the active lease, another pane owns it, or master mode is off.`;
}

export default function aocMasterExtension(pi: CommandExtensionAPI): void {
	pi.registerCommand("master", {
		description: "Usage: /master on [minutes], /master off, or /master status. Enable gated AOC master orchestration for this Herdr workspace.",
		getArgumentCompletions: (_prefix: string): AutocompleteItem[] | null => [
			{ value: "on 30", label: "on 30", description: "Enable master mode for 30 minutes" },
			{ value: "on 60", label: "on 60", description: "Enable master mode for 60 minutes" },
			{ value: "on 240", label: "on 240", description: "Enable master mode for 240 minutes" },
			{ value: "status", label: "status", description: "Show current master-mode lease" },
			{ value: "off", label: "off", description: "Disable master mode for this pane" },
		],
		handler: async (args, ctx) => {
			const parts = argsText(args).split(/\s+/).filter(Boolean);
			const rawMode = parts[0]?.toLowerCase() || "status";
			let mode: "on" | "off" | "status";
			if (rawMode === "on" || rawMode === "enable") mode = "on";
			else if (rawMode === "off" || rawMode === "disable") mode = "off";
			else if (rawMode === "status") mode = "status";
			else {
				const message = `Unknown master mode '${rawMode}'. Use /master on [minutes], /master off, or /master status.`;
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
		name: "aoc_orchestrate",
		label: "AOC Orchestrate",
		description: "Gated Herdr orchestration for master agents: enable/disable/status master mode, then send structured assignments or short messages to peer agents. Mutating actions require /master on and an active lease owned by this pane.",
		promptSnippet: "Use /master on before aoc_orchestrate assign/send. Observe peers with aoc_herdr, then send explicit bounded assignments/messages through aoc_orchestrate.",
		promptGuidelines: [
			"Use aoc_orchestrate only after /master on has enabled master mode for this pane; otherwise master_on is the only mutating setup action you may call.",
			"Use aoc_herdr list/get/transcript/read before assigning work so targets, status, and current context are grounded.",
			"assign/send reject busy peers by default. Do not set requireIdle=false unless the user explicitly asks to interrupt or the peer is known to be waiting for master input.",
			"Never use aoc_orchestrate for shell commands, keystrokes, focus, close, move, resize, or pane run. This tool only supports Herdr agent send with bounded text.",
			"After sending an assignment, use aoc_herdr wait/transcript/read to collect results instead of repeatedly sending prompts.",
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
				return { content: [{ type: "text", text: `${result.reason === "master mode disabled" ? "master mode disabled" : "master mode unchanged"}\n${result.reason}` }], details: { action: "master_off", ok: result.ok, reason: result.reason } };
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
			try {
				return await sendToPeer(params.action, params, identity, projectRoot, maxChars, signal);
			} catch (error) {
				const reason = error instanceof Error ? error.message : String(error);
				return { content: [{ type: "text", text: reason }], details: { action: params.action, ok: false, reason } };
			}
		},
	});
}
