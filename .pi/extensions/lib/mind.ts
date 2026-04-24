import type { ExtensionContext } from "@mariozechner/pi-coding-agent";
import * as fs from "node:fs";
import { spawnSync } from "node:child_process";
import { homedir } from "node:os";
import { basename, join } from "node:path";

export type MindStatus = "idle" | "queued" | "running" | "success" | "fallback" | "error";

export type MindFeedProgress = {
	t0_estimated_tokens: number;
	t1_target_tokens: number;
	t1_hard_cap_tokens: number;
	tokens_until_next_run: number;
};

export type PendingCompactionPreparation = {
	capturedAtMs: number;
	firstKeptEntryId?: string;
	tokensBefore?: number;
};

type MindCommandPayload = {
	command?: string;
	status?: string;
	message?: string;
	error?: { code?: string; message?: string };
};

type MindSharedState = {
	mindStatus: MindStatus;
	mindReason?: string;
	mindUpdatedAtMs?: number;
	mindProgress?: MindFeedProgress;
	lastError?: string;
	lastSendFailedAtMs?: number;
	lastCommand?: {
		command: string;
		status?: string;
		message?: string;
		atMs: number;
	};
	pendingCompactionPreparation?: PendingCompactionPreparation;
};

const STANDALONE_SYNC_TIMEOUT_MS = 10_000;
const STANDALONE_SYNC_CARGO_TIMEOUT_MS = 60_000;

type MindCommandOutcome = {
	ok: boolean;
	payload?: MindCommandPayload;
	error?: string;
	fallbackUsed?: boolean;
	fallbackMode?: "standalone_sync_pi";
};

const GLOBAL_KEY = "__AOC_MIND_EXTENSION_SHARED_STATE__";

function globalState(): MindSharedState {
	const root = globalThis as Record<string, unknown>;
	const existing = root[GLOBAL_KEY] as MindSharedState | undefined;
	if (existing) return existing;
	const created: MindSharedState = {
		mindStatus: "idle",
	};
	root[GLOBAL_KEY] = created;
	return created;
}

export function stableHashHex(input: string): string {
	let hash = 2166136261;
	for (let i = 0; i < input.length; i++) {
		hash ^= input.charCodeAt(i);
		hash = Math.imul(hash, 16777619) >>> 0;
	}
	return hash.toString(16).padStart(8, "0");
}


function parseMindProgress(payload: any): MindFeedProgress | undefined {
	if (!payload || typeof payload !== "object") return undefined;
	const t0 = Number((payload as any).t0_estimated_tokens);
	const target = Number((payload as any).t1_target_tokens);
	const hard = Number((payload as any).t1_hard_cap_tokens);
	const until = Number((payload as any).tokens_until_next_run);
	if (![t0, target, hard, until].every((value) => Number.isFinite(value))) return undefined;
	return {
		t0_estimated_tokens: Math.max(0, Math.round(t0)),
		t1_target_tokens: Math.max(0, Math.round(target)),
		t1_hard_cap_tokens: Math.max(0, Math.round(hard)),
		tokens_until_next_run: Math.max(0, Math.round(until)),
	};
}


function markError(message: string): void {
	const state = globalState();
	state.lastError = message;
	state.lastSendFailedAtMs = Date.now();
}

function clearError(): void {
	const state = globalState();
	state.lastError = undefined;
}

export function blocksToText(content: unknown): string {
	if (!content) return "";
	if (typeof content === "string") return content;
	if (!Array.isArray(content)) return "";
	const parts: string[] = [];
	for (const block of content) {
		if (!block || typeof block !== "object") continue;
		const rec = block as Record<string, unknown>;
		if (rec.type === "text" && typeof rec.text === "string") parts.push(rec.text);
		if (rec.type === "thinking" && typeof rec.thinking === "string") parts.push(rec.thinking);
	}
	return parts.join("\n");
}

export function resolveProjectRoot(ctx?: ExtensionContext): string {
	const sessionManager = ctx?.sessionManager as any;
	return String(sessionManager?.getProjectRoot?.() || process.env.AOC_PROJECT_ROOT || process.cwd());
}

export function resolveMindStorePath(ctx?: ExtensionContext): string {
	const explicit = process.env.AOC_MIND_STORE_PATH?.trim();
	if (explicit) return explicit;
	return join(resolveProjectRoot(ctx), ".aoc", "mind", "project.sqlite");
}

function envFlag(name: string): boolean | undefined {
	const value = process.env[name]?.trim().toLowerCase();
	if (!value) return undefined;
	if (["1", "true", "yes", "on"].includes(value)) return true;
	if (["0", "false", "no", "off"].includes(value)) return false;
	return undefined;
}

export function currentMindSnapshot(ctx?: ExtensionContext): {
	mindStatus: MindStatus;
	mindReason?: string;
	mindUpdatedAtMs?: number;
	mindProgress?: MindFeedProgress;
	transportMode: "standalone";
	lastError?: string;
	lastSendFailedAtMs?: number;
	lastCommand?: MindSharedState["lastCommand"];
	projectRoot: string;
	storePath: string;
	sessionId?: string;
	paneId?: string;
	launchMode?: string;
	wrapMode?: string;
	tmuxActive?: boolean;
	bootloaderActive?: boolean;
} {
	const state = globalState();
	const identity = currentStandaloneSessionIdentity();
	return {
		mindStatus: state.mindStatus,
		mindReason: state.mindReason,
		mindUpdatedAtMs: state.mindUpdatedAtMs,
		mindProgress: state.mindProgress,
		transportMode: "standalone",
		lastError: state.lastError,
		lastSendFailedAtMs: state.lastSendFailedAtMs,
		lastCommand: state.lastCommand,
		projectRoot: resolveProjectRoot(ctx),
		storePath: resolveMindStorePath(ctx),
		sessionId: identity?.sessionId,
		paneId: identity?.paneId,
		launchMode: process.env.AOC_AGENT_LAUNCH_MODE?.trim() || undefined,
		wrapMode: process.env.AOC_AGENT_WRAP_MODE?.trim() || undefined,
		tmuxActive: envFlag("AOC_AGENT_TMUX_ACTIVE"),
		bootloaderActive: envFlag("AOC_AGENT_BOOTLOADER_ACTIVE"),
	};
}

export function rememberCompactionPreparation(preparation: PendingCompactionPreparation | undefined): void {
	globalState().pendingCompactionPreparation = preparation;
}

export function consumeCompactionPreparation(): PendingCompactionPreparation | undefined {
	const state = globalState();
	const value = state.pendingCompactionPreparation;
	state.pendingCompactionPreparation = undefined;
	return value;
}

function standaloneMindAgentId(): string {
	return `pi-extension-${process.pid}`;
}

function applyStandaloneSyncSuccess(command: string, note: string, progress?: MindFeedProgress): void {
	const state = globalState();
	state.mindStatus = "success";
	state.mindReason = note;
	state.mindUpdatedAtMs = Date.now();
	if (progress) state.mindProgress = progress;
	state.lastCommand = {
		command,
		status: "ok",
		message: note,
		atMs: Date.now(),
	};
	clearError();
}

type MindServiceLaunchSpec = {
	serviceBin?: string;
	cargoManifest?: string;
	sourceRoot?: string;
};

function readMindServiceLaunchConfig(path: string): MindServiceLaunchSpec | undefined {
	if (!path || !fs.existsSync(path)) return undefined;
	try {
		const parsed = JSON.parse(fs.readFileSync(path, "utf8")) as Record<string, unknown>;
		const serviceBin = typeof parsed.serviceBin === "string" ? parsed.serviceBin.trim() : "";
		const cargoManifest = typeof parsed.cargoManifest === "string" ? parsed.cargoManifest.trim() : "";
		const sourceRoot = typeof parsed.sourceRoot === "string" ? parsed.sourceRoot.trim() : "";
		return { serviceBin: serviceBin || undefined, cargoManifest: cargoManifest || undefined, sourceRoot: sourceRoot || undefined };
	} catch {
		return undefined;
	}
}

function isRunnableCommand(candidate: string | undefined): boolean {
	if (!candidate) return false;
	if (candidate.includes("/") || candidate.includes("\\")) return fs.existsSync(candidate);
	return true;
}

function resolveMindServiceLaunchSpec(projectRoot: string): MindServiceLaunchSpec {
	const projectConfig = readMindServiceLaunchConfig(join(projectRoot, ".aoc", "mind-service.json"));
	const globalConfig = readMindServiceLaunchConfig(join(homedir(), ".config", "aoc", "mind-service.json"));
	const envServiceBin = process.env.AOC_MIND_SERVICE_BIN?.trim() || undefined;
	const envCargoManifest = process.env.AOC_MIND_CARGO_MANIFEST?.trim() || undefined;
	const envSourceRoot = process.env.AOC_MIND_SOURCE_ROOT?.trim() || undefined;
	const cargoManifest = [
		envCargoManifest,
		envSourceRoot ? join(envSourceRoot, "crates", "Cargo.toml") : undefined,
		projectConfig?.cargoManifest,
		projectConfig?.sourceRoot ? join(projectConfig.sourceRoot, "crates", "Cargo.toml") : undefined,
		globalConfig?.cargoManifest,
		globalConfig?.sourceRoot ? join(globalConfig.sourceRoot, "crates", "Cargo.toml") : undefined,
		join(projectRoot, "crates", "Cargo.toml"),
	].find((candidate) => !!candidate && fs.existsSync(candidate));
	const serviceBin = [envServiceBin, projectConfig?.serviceBin, globalConfig?.serviceBin].find((candidate) => isRunnableCommand(candidate));
	return { serviceBin, cargoManifest, sourceRoot: envSourceRoot || projectConfig?.sourceRoot || globalConfig?.sourceRoot };
}

function runMindServiceJson(
	ctx: ExtensionContext,
	args: string[],
	timeoutMs: number,
): { ok: boolean; parsed?: any; error?: string } {
	const projectRoot = resolveProjectRoot(ctx);
	const launchSpec = resolveMindServiceLaunchSpec(projectRoot);
	const cargoManifest = launchSpec.cargoManifest;
	const tryInstalled = spawnSync(launchSpec.serviceBin || "aoc-mind-service", args, {
		encoding: "utf8",
		env: process.env,
		timeout: timeoutMs,
	});
	let result = tryInstalled;
	if (tryInstalled.error && cargoManifest) {
		result = spawnSync("cargo", [
			"run",
			"--quiet",
			"--manifest-path",
			cargoManifest,
			"-p",
			"aoc-mind",
			"--bin",
			"aoc-mind-service",
			"--",
			...args,
		], {
			encoding: "utf8",
			env: process.env,
			timeout: Math.max(timeoutMs, STANDALONE_SYNC_CARGO_TIMEOUT_MS),
		});
	}
	if (result.error) {
		if ((result.error as NodeJS.ErrnoException).code === "ENOENT" && !cargoManifest) {
			return { ok: false, error: "mind service unavailable: install aoc-mind-service or set AOC_MIND_SERVICE_BIN / AOC_MIND_CARGO_MANIFEST" };
		}
		return { ok: false, error: `mind service failed: ${result.error.message}` };
	}
	const stdout = String(result.stdout || "").trim();
	const stderr = String(result.stderr || "").trim();
	let parsed: any = undefined;
	if (stdout) {
		try {
			parsed = JSON.parse(stdout);
		} catch {
			parsed = undefined;
		}
	}
	if (result.status !== 0) {
		return { ok: false, error: String(parsed?.error || stderr || stdout || `mind service exited with status ${result.status ?? "unknown"}`) };
	}
	return { ok: true, parsed };
}

export function readStandaloneMindStatus(ctx: ExtensionContext): any | undefined {
	const projectRoot = resolveProjectRoot(ctx);
	const result = runMindServiceJson(ctx, ["status", "--project-root", projectRoot, "--json"], STANDALONE_SYNC_TIMEOUT_MS);
	return result.ok ? result.parsed : undefined;
}

function currentStandaloneSessionIdentity(): { sessionId: string; paneId: string } | undefined {
	const sessionId = process.env.AOC_SESSION_ID?.trim();
	const paneId = process.env.AOC_PANE_ID?.trim();
	if (!sessionId || !paneId) return undefined;
	return { sessionId, paneId };
}

function runStandaloneMindCommand(ctx: ExtensionContext, args: string[], timeoutMs = STANDALONE_SYNC_TIMEOUT_MS): MindCommandOutcome {
	const service = runMindServiceJson(ctx, args, timeoutMs);
	if (!service.ok) {
		const error = service.error || "mind service command failed";
		markError(error);
		return { ok: false, error };
	}
	const parsed = service.parsed;
	if (parsed?.ok === false) {
		const error = String(parsed?.error || "mind service command failed");
		markError(error);
		return { ok: false, error };
	}
	const command = String(args[0] || "mind-service");
	const message = typeof parsed?.message === "string" ? parsed.message : "ok";
	clearError();
	globalState().lastCommand = {
		command,
		status: "ok",
		message,
		atMs: Date.now(),
	};
	return {
		ok: true,
		payload: { command, status: "ok", message },
		fallbackUsed: true,
		fallbackMode: "standalone_sync_pi",
	};
}

function runStandaloneSyncPi(ctx: ExtensionContext, command: string): MindCommandOutcome {
	const projectRoot = resolveProjectRoot(ctx);
	const sessionFile = process.env.AOC_PI_SESSION_FILE?.trim();
	const args = [
		"sync-pi",
		"--project-root",
		projectRoot,
		"--agent-id",
		standaloneMindAgentId(),
		"--json",
	];
	if (sessionFile) {
		args.push("--session-file", sessionFile);
	}
	const service = runMindServiceJson(ctx, args, STANDALONE_SYNC_TIMEOUT_MS);
	if (!service.ok) {
		const error = service.error || "standalone sync failed";
		markError(error);
		return { ok: false, error };
	}
	const parsed = service.parsed;
	if (parsed?.ok === false) {
		const error = String(parsed?.error || "standalone sync failed");
		markError(error);
		return { ok: false, error };
	}
	const note = typeof parsed?.message === "string"
		? parsed.message
		: typeof parsed?.session_file === "string" && parsed.session_file.length > 0
			? `standalone sync ingested ${basename(parsed.session_file)}`
			: "standalone sync completed";
	const progress = parseMindProgress(parsed?.progress);
	applyStandaloneSyncSuccess(command, note, progress);
	return {
		ok: true,
		payload: { command, status: "ok", message: note },
		fallbackUsed: true,
		fallbackMode: "standalone_sync_pi",
	};
}


function extractCommandHintFromToolResult(message: any): string | undefined {
	const details = message?.details;
	if (details && typeof details === "object") {
		const command = (details as any).command;
		if (typeof command === "string" && command.trim().length > 0) return command.trim();
	}
	const text = blocksToText(message?.content);
	const prefix = "Command:";
	const idx = text.indexOf(prefix);
	if (idx >= 0) {
		const line = text.slice(idx + prefix.length).split("\n")[0]?.trim();
		if (line) return line;
	}
	return undefined;
}

function inferFilePathsFromText(text: string): string[] {
	const matches = text.match(/(?:\.?\.?\/)?[A-Za-z0-9_./-]+\.[A-Za-z0-9_-]{1,12}/g) ?? [];
	const values = matches
		.map((value) => value.trim())
		.filter((value) => value.length > 1 && !value.startsWith("http://") && !value.startsWith("https://"));
	return Array.from(new Set(values)).slice(0, 8);
}

function inferTaskIdsFromText(text: string): string[] {
	const ids = new Set<string>();
	for (const match of text.matchAll(/\btask\s+#?(\d+)\b/gi)) ids.add(match[1]);
	for (const match of text.matchAll(/\b#(\d{2,})\b/g)) ids.add(match[1]);
	return Array.from(ids).slice(0, 6);
}

export function inferFocusSnapshot(message: any, ctx: ExtensionContext): {
	projectRoot: string;
	storePath: string;
	focusLabel?: string;
	filePaths: string[];
	taskIds: string[];
	commandHint?: string;
} {
	const sessionManager = ctx.sessionManager as any;
	const role = typeof message?.role === "string" ? message.role : "";
	const text = blocksToText(message?.content);
	const commandHint = role === "toolResult" ? extractCommandHintFromToolResult(message) : undefined;
	const combined = [text, commandHint, process.env.AOC_TAB_SCOPE, basename(resolveProjectRoot(ctx))].filter(Boolean).join("\n");
	const filePaths = inferFilePathsFromText(combined);
	const taskIds = inferTaskIdsFromText(combined);
	const focusLabel = [
		process.env.AOC_TAB_SCOPE,
		taskIds.length > 0 ? `task:${taskIds[0]}` : undefined,
		filePaths.length > 0 ? `file:${filePaths[0]}` : undefined,
		sessionManager?.getProjectRoot?.() ? basename(String(sessionManager.getProjectRoot())) : undefined,
	].filter(Boolean).join(" · ") || undefined;
	return {
		projectRoot: resolveProjectRoot(ctx),
		storePath: resolveMindStorePath(ctx),
		focusLabel,
		filePaths,
		taskIds,
		commandHint,
	};
}

export function buildMindIngestPayload(message: any, ctx: ExtensionContext): Record<string, unknown> | undefined {
	const conversationId = ctx.sessionManager.getSessionId?.();
	if (!conversationId || typeof conversationId !== "string") return undefined;
	const timestampMs = typeof message?.timestamp === "number" ? Math.round(message.timestamp) : Date.now();
	const role = typeof message?.role === "string" ? message.role : "";
	const text = blocksToText(message?.content);
	const eventIdSeed = JSON.stringify({ role, timestampMs, text, tool: message?.toolName, details: message?.details });
	const eventId = `pi:${conversationId}:${stableHashHex(eventIdSeed)}`;
	const focus = inferFocusSnapshot(message, ctx);
	const attrs: Record<string, unknown> = {
		project_root: focus.projectRoot,
		pane_id: process.env.AOC_PANE_ID || undefined,
		source_extension: "mind-ingest",
		file_paths: focus.filePaths,
		task_ids: focus.taskIds,
		focus_label: focus.focusLabel,
		tab_scope: process.env.AOC_TAB_SCOPE || undefined,
	};
	if (role === "user" || role === "assistant" || role === "system") {
		return {
			conversation_id: conversationId,
			event_id: eventId,
			timestamp_ms: timestampMs,
			attrs,
			body: {
				kind: "message",
				role,
				text,
			},
		};
	}
	if (role === "toolResult") {
		const toolName = typeof message?.toolName === "string" ? message.toolName : "tool";
		const details = message?.details ?? {};
		const commandHint = extractCommandHintFromToolResult(message);
		attrs.tool_name = toolName;
		attrs.command_hint = commandHint;
		if (commandHint) attrs.file_paths = Array.from(new Set([...(attrs.file_paths as string[]), ...inferFilePathsFromText(commandHint)]));
		return {
			conversation_id: conversationId,
			event_id: eventId,
			timestamp_ms: timestampMs,
			attrs,
			body: {
				kind: "tool_result",
				tool_name: toolName,
				is_error: Boolean(message?.isError),
				latency_ms: typeof details?.latencyMs === "number" ? details.latencyMs : (typeof details?.latency_ms === "number" ? details.latency_ms : undefined),
				exit_code: typeof details?.exitCode === "number" ? details.exitCode : (typeof details?.exit_code === "number" ? details.exit_code : undefined),
				output: text || undefined,
				redacted: Boolean(details?.redacted),
			},
		};
	}
	return undefined;
}

export async function ingestMindMessage(message: any, ctx: ExtensionContext): Promise<{ ok: boolean; error?: string; fallbackUsed?: boolean }> {
	const payload = buildMindIngestPayload(message, ctx);
	if (!payload) return { ok: true };
	const result = runStandaloneSyncPi(ctx, "mind_ingest_event");
	return { ok: result.ok, error: result.error, fallbackUsed: true };
}

export async function sendMindCompactionCheckpoint(event: any, ctx: ExtensionContext): Promise<{ ok: boolean; error?: string; fallbackUsed?: boolean }> {
	consumeCompactionPreparation();
	const result = runStandaloneSyncPi(ctx, "mind_compaction_checkpoint");
	return { ok: result.ok, error: result.error, fallbackUsed: true };
}

export async function requestManualObserverRun(ctx: ExtensionContext): Promise<{ ok: boolean; message: string }> {
	const identity = currentStandaloneSessionIdentity();
	if (!identity) return { ok: false, message: "missing AOC session/pane context" };
	const result = runStandaloneMindCommand(ctx, [
		"observer-run",
		"--project-root", resolveProjectRoot(ctx),
		"--session-id", identity.sessionId,
		"--pane-id", identity.paneId,
		"--conversation-id", String(ctx.sessionManager.getSessionId?.() || ""),
		"--agent-id", standaloneMindAgentId(),
		"--reason", "pi shortcut",
		"--json",
	]);
	if (!result.ok) return { ok: false, message: result.error ?? result.payload?.message ?? "Observer run unavailable" };
	globalState().mindStatus = "queued";
	return { ok: true, message: result.payload?.message || "Observer run queued" };
}

export async function finalizeMindSession(ctx: ExtensionContext, reason = "pi command"): Promise<{ ok: boolean; message: string }> {
	const identity = currentStandaloneSessionIdentity();
	if (!identity) return { ok: false, message: "missing AOC session/pane context" };
	const result = runStandaloneMindCommand(ctx, [
		"finalize-session",
		"--project-root", resolveProjectRoot(ctx),
		"--session-id", identity.sessionId,
		"--pane-id", identity.paneId,
		"--conversation-id", String(ctx.sessionManager.getSessionId?.() || ""),
		"--reason", reason,
		"--json",
	], 20_000);
	if (!result.ok) return { ok: false, message: result.error ?? result.payload?.message ?? "Mind finalize unavailable" };
	return { ok: true, message: result.payload?.message || "Mind finalize queued" };
}

export async function fetchMindContextPack(ctx: ExtensionContext, mode: string, detail = false, role = "operator", reason = "pi context"): Promise<any | undefined> {
	const service = runMindServiceJson(ctx, [
		"context-pack",
		"--project-root", resolveProjectRoot(ctx),
		"--mode", mode,
		"--role", role,
		"--reason", reason,
		...(detail ? ["--detail"] : []),
		"--json",
	], STANDALONE_SYNC_TIMEOUT_MS);
	if (!service.ok || service.parsed?.ok === false) return undefined;
	return service.parsed?.pack;
}

export function renderContextPackPrelude(pack: any | undefined): string | undefined {
	const lines = Array.isArray(pack?.rendered_lines) ? pack.rendered_lines.filter((line: unknown) => typeof line === "string" && line.trim()) : [];
	if (lines.length === 0) return undefined;
	const citations = Array.isArray(pack?.citations)
		? pack.citations
			.slice(0, 6)
			.map((citation: any) => citation?.reference || citation?.label || citation?.source_id)
			.filter((value: unknown): value is string => typeof value === "string" && value.trim().length > 0)
		: [];
	const suffix = citations.length > 0 ? `\n\nContext citations: ${citations.join(", ")}` : "";
	return `${lines.join("\n")}${suffix}`;
}

function resolveMindToggleCommand(root: string): string {
	const local = join(root, "bin", "aoc-mind-toggle");
	return fs.existsSync(local) ? local : "aoc-mind-toggle";
}

export function launchMindUi(ctx: ExtensionContext): { ok: boolean; message: string } {
	const root = resolveProjectRoot(ctx);
	const command = resolveMindToggleCommand(root);
	try {
		const result = spawnSync(command, [], {
			cwd: root,
			stdio: "ignore",
			shell: false,
			timeout: 8_000,
			env: process.env,
		});
		if (result.error) return { ok: false, message: `Mind UI unavailable: ${result.error.message}` };
		if ((result.status ?? 1) !== 0) return { ok: false, message: `Mind UI launcher exited with status ${result.status ?? 1}` };
		return { ok: true, message: "Project Mind toggled" };
	} catch (error) {
		return { ok: false, message: `Mind UI unavailable: ${error instanceof Error ? error.message : String(error)}` };
	}
}

export function formatMindStatus(snapshot: ReturnType<typeof currentMindSnapshot>): string[] {
	const lines = [
		`mind_status: ${snapshot.mindStatus}`,
		`transport_mode: ${snapshot.transportMode}`,
		`project_root: ${snapshot.projectRoot}`,
		`store_path: ${snapshot.storePath}`,
	];
	if (snapshot.sessionId) lines.push(`session_id: ${snapshot.sessionId}`);
	if (snapshot.paneId) lines.push(`pane_id: ${snapshot.paneId}`);
	if (snapshot.launchMode) lines.push(`launch_mode: ${snapshot.launchMode}`);
	if (snapshot.wrapMode) lines.push(`wrap_mode: ${snapshot.wrapMode}`);
	if (typeof snapshot.tmuxActive === "boolean") lines.push(`tmux_active: ${snapshot.tmuxActive ? "yes" : "no"}`);
	if (typeof snapshot.bootloaderActive === "boolean") lines.push(`bootloader_active: ${snapshot.bootloaderActive ? "yes" : "no"}`);
	if (snapshot.mindReason) lines.push(`reason: ${snapshot.mindReason}`);
	if (snapshot.mindUpdatedAtMs) lines.push(`updated_at: ${new Date(snapshot.mindUpdatedAtMs).toISOString()}`);
	if (snapshot.mindProgress) {
		lines.push(`t0_estimated_tokens: ${snapshot.mindProgress.t0_estimated_tokens}`);
		lines.push(`t1_target_tokens: ${snapshot.mindProgress.t1_target_tokens}`);
		lines.push(`tokens_until_next_run: ${snapshot.mindProgress.tokens_until_next_run}`);
	}
	if (snapshot.lastCommand) lines.push(`last_command: ${snapshot.lastCommand.command} ${snapshot.lastCommand.status ?? "unknown"}`);
	if (snapshot.lastError) lines.push(`last_error: ${snapshot.lastError}`);
	if (snapshot.lastSendFailedAtMs) lines.push(`last_send_failed_at: ${new Date(snapshot.lastSendFailedAtMs).toISOString()}`);
	return lines;
}
