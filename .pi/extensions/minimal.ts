/**
 * Minimal — Model + Mind observer HUD in compact footer
 *
 * Shows:
 * - active model
 * - AOC Mind observer state (queued/running/success/fallback/error)
 * - T1 pre-filter load bar (authoritative feed when available, deterministic local fallback)
 * - session context usage bar
 *
 * Shortcut:
 * - Ctrl+Shift+O: request manual observer run (Pulse command: run_observer)
 */

import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { applyExtensionDefaults } from "./themeMap.ts";
import { truncateToWidth, visibleWidth } from "@mariozechner/pi-tui";
import { createConnection, type Socket } from "node:net";
import { join } from "node:path";

type MindStatus = "idle" | "queued" | "running" | "success" | "fallback" | "error";

type Sample = { at: number; tokens: number };

type MindFeedProgress = {
	t0_estimated_tokens: number;
	t1_target_tokens: number;
	t1_hard_cap_tokens: number;
	tokens_until_next_run: number;
};

type ExtensionState = {
	ctx?: ExtensionContext;
	initialized: boolean;
	filteredTokens: number;
	samples: Sample[];
	lastEstimateAtMs?: number;
	lastTokenRecomputeAtMs?: number;
	sessionStartAnimationStartedAtMs?: number;
	mindStatus: MindStatus;
	mindReason?: string;
	mindUpdatedAtMs?: number;
	mindProgress?: MindFeedProgress;
	pulseConnected: boolean;
	pulseSocket?: Socket;
	pulseBuffer: string;
	reconnectTimer?: NodeJS.Timeout;
	refreshTimer?: NodeJS.Timeout;
	lastPulseRequestId?: string;
};

const T1_TARGET_TOKENS = 28_000;
const SAMPLE_WINDOW_MS = 10 * 60 * 1000;
const REFRESH_INTERVAL_MS = 117;
const TOKEN_RECOMPUTE_INTERVAL_MS = 2_000;
const RUNNING_ANIMATION_STEP_MS = 117;
const MIND_BAR_WIDTH = 10;
const SESSION_START_ANIMATION_STEPS = Math.max(1, (MIND_BAR_WIDTH - 1) * 2);
const SESSION_START_ANIMATION_MS = (SESSION_START_ANIMATION_STEPS + 1) * RUNNING_ANIMATION_STEP_MS;

const state: ExtensionState = {
	initialized: false,
	filteredTokens: 0,
	samples: [],
	mindStatus: "idle",
	pulseConnected: false,
	pulseBuffer: "",
};

function stableHashHex(input: string): string {
	let hash = 2166136261;
	for (let i = 0; i < input.length; i++) {
		hash ^= input.charCodeAt(i);
		hash = Math.imul(hash, 16777619) >>> 0;
	}
	return hash.toString(16).padStart(8, "0");
}

function sessionSlug(sessionId: string): string {
	let slug = "";
	for (const ch of sessionId) {
		slug += /[A-Za-z0-9._-]/.test(ch) ? ch : "-";
	}
	while (slug.includes("--")) slug = slug.replaceAll("--", "-");
	slug = slug.replace(/^-+|-+$/g, "");
	const base = slug.length > 0 ? slug : "session";
	const short = base.length > 48 ? base.slice(0, 48) : base;
	return `${short}-${stableHashHex(sessionId)}`;
}

function pulseSocketPath(sessionId: string): string {
	const envPath = process.env.AOC_PULSE_SOCK?.trim();
	if (envPath) return envPath;

	const runtimeDir = process.env.XDG_RUNTIME_DIR?.trim()
		|| (typeof process.getuid === "function" ? `/run/user/${process.getuid()}` : "")
		|| "/tmp";

	return join(runtimeDir, "aoc", sessionSlug(sessionId), "pulse.sock");
}

function nowIso(): string {
	return new Date().toISOString();
}

function estimateTokens(text: string): number {
	if (!text) return 0;
	return Math.ceil(text.length / 4);
}

function blocksToText(content: unknown): string {
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

function toolMetaLine(message: any): string {
	const name = String(message?.toolName || "tool");
	const ok = message?.isError ? "error" : "ok";
	const details = message?.details ?? {};
	const latency = details?.latencyMs ?? details?.latency_ms ?? details?.durationMs ?? details?.duration_ms;
	const exitCode = details?.exitCode ?? details?.exit_code;
	const outBytes = typeof details?.outputBytes === "number"
		? details.outputBytes
		: (typeof details?.stdoutBytes === "number" ? details.stdoutBytes : undefined);

	let line = `${name} ${ok}`;
	if (typeof latency === "number") line += ` latency=${latency}ms`;
	if (typeof exitCode === "number") line += ` exit=${exitCode}`;
	if (typeof outBytes === "number") line += ` bytes=${outBytes}`;
	return line;
}

function recomputeFilteredTokens(ctx: ExtensionContext): void {
	const branch = ctx.sessionManager.getBranch?.() ?? [];
	let tokens = 0;

	for (const entry of branch) {
		if (!entry || typeof entry !== "object") continue;
		if ((entry as any).type !== "message") continue;
		const message = (entry as any).message;
		if (!message || typeof message !== "object") continue;

		switch (message.role) {
			case "user":
			case "assistant":
			case "system": {
				tokens += estimateTokens(blocksToText(message.content));
				break;
			}
			case "toolResult": {
				tokens += estimateTokens(toolMetaLine(message));
				break;
			}
			case "bashExecution": {
				const cmd = typeof message.command === "string" ? message.command : "bash";
				const code = typeof message.exitCode === "number" ? ` exit=${message.exitCode}` : "";
				tokens += estimateTokens(`bash ${cmd}${code}`);
				break;
			}
			default:
				break;
		}
	}

	state.filteredTokens = tokens;
	state.lastEstimateAtMs = Date.now();
	state.lastTokenRecomputeAtMs = state.lastEstimateAtMs;
	state.samples.push({ at: state.lastEstimateAtMs, tokens });
	const cutoff = state.lastEstimateAtMs - SAMPLE_WINDOW_MS;
	state.samples = state.samples.filter((s) => s.at >= cutoff);
}

function bar(pct: number, width = 10): string {
	const clamped = Math.max(0, Math.min(1, pct));
	const filled = Math.round(clamped * width);
	return "#".repeat(filled) + "-".repeat(Math.max(0, width - filled));
}

function barAtPosition(pos: number, width = MIND_BAR_WIDTH): string {
	const safeWidth = Math.max(1, width);
	const clampedPos = Math.max(0, Math.min(safeWidth - 1, Math.floor(pos)));
	const chars = Array.from({ length: safeWidth }, () => "-");
	chars[clampedPos] = "#";
	return chars.join("");
}

function pingPongPosition(step: number, maxIndex: number): number {
	if (maxIndex <= 0) return 0;
	const cycle = maxIndex * 2;
	const stepInCycle = ((step % cycle) + cycle) % cycle;
	return stepInCycle <= maxIndex ? stepInCycle : cycle - stepInCycle;
}

function runningBar(width = MIND_BAR_WIDTH): string {
	const safeWidth = Math.max(1, width);
	const maxIndex = Math.max(0, safeWidth - 1);
	const step = Math.floor(Date.now() / RUNNING_ANIMATION_STEP_MS);
	return barAtPosition(pingPongPosition(step, maxIndex), safeWidth);
}

function isSessionStartAnimating(now = Date.now()): boolean {
	return Boolean(
		state.sessionStartAnimationStartedAtMs
		&& now - state.sessionStartAnimationStartedAtMs < SESSION_START_ANIMATION_MS,
	);
}

function sessionStartBar(width = MIND_BAR_WIDTH, now = Date.now()): string {
	const safeWidth = Math.max(1, width);
	const startedAt = state.sessionStartAnimationStartedAtMs;
	if (!startedAt) return barAtPosition(0, safeWidth);

	const elapsed = Math.max(0, now - startedAt);
	const step = Math.floor(elapsed / RUNNING_ANIMATION_STEP_MS);
	const maxIndex = Math.max(0, safeWidth - 1);
	const pathLen = Math.max(1, maxIndex * 2);
	const boundedStep = Math.min(pathLen, step);
	const pos = boundedStep <= maxIndex ? boundedStep : pathLen - boundedStep;
	return barAtPosition(pos, safeWidth);
}

function mindBar(pct: number, status: MindStatus, width = MIND_BAR_WIDTH): string {
	const now = Date.now();
	if (status === "running") return runningBar(width);
	if (isSessionStartAnimating(now)) return sessionStartBar(width, now);
	return bar(pct, width);
}

function parseMindProgress(input: any): MindFeedProgress | undefined {
	if (!input || typeof input !== "object") return undefined;
	const t0 = Number(input.t0_estimated_tokens);
	const target = Number(input.t1_target_tokens);
	const hardCap = Number(input.t1_hard_cap_tokens);
	const until = Number(input.tokens_until_next_run);
	if (!Number.isFinite(t0) || !Number.isFinite(target) || !Number.isFinite(hardCap) || !Number.isFinite(until)) {
		return undefined;
	}
	if (target <= 0 || hardCap < target) return undefined;
	return {
		t0_estimated_tokens: Math.max(0, Math.round(t0)),
		t1_target_tokens: Math.max(1, Math.round(target)),
		t1_hard_cap_tokens: Math.max(Math.round(target), Math.round(hardCap)),
		tokens_until_next_run: Math.max(0, Math.round(until)),
	};
}

function composeCenteredFooterLine(left: string, center: string, right: string, width: number): string {
	const leftWidth = visibleWidth(left);
	const centerWidth = visibleWidth(center);
	const rightWidth = visibleWidth(right);

	if (leftWidth + centerWidth + rightWidth + 2 > width) {
		return truncateToWidth(`${left} ${center} ${right}`, width);
	}

	const rightStart = Math.max(0, width - rightWidth);
	let centerStart = Math.floor((width - centerWidth) / 2);
	centerStart = Math.max(centerStart, leftWidth + 1);
	centerStart = Math.min(centerStart, rightStart - centerWidth - 1);

	if (centerStart < leftWidth + 1 || centerStart + centerWidth >= rightStart) {
		return truncateToWidth(`${left} ${center} ${right}`, width);
	}

	const gapLeft = " ".repeat(Math.max(1, centerStart - leftWidth));
	const gapRight = " ".repeat(Math.max(1, rightStart - (centerStart + centerWidth)));
	return truncateToWidth(`${left}${gapLeft}${center}${gapRight}${right}`, width);
}

function renderFooter(width: number, _theme: any): string {
	const ctx = state.ctx as any;
	const model = ctx?.model?.id || "no-model";
	const usage = ctx?.getContextUsage?.();
	const ctxPct = usage && usage.percent !== null ? Number(usage.percent) / 100 : 0;

	const t0Tokens = state.mindProgress?.t0_estimated_tokens ?? state.filteredTokens;
	const t1Target = state.mindProgress?.t1_target_tokens ?? T1_TARGET_TOKENS;
	const mindLoadPct = Math.min(1, t0Tokens / Math.max(1, t1Target));
	const mindPart = `✦ [${mindBar(mindLoadPct, state.mindStatus, MIND_BAR_WIDTH)}]✦`;
	const ctxPart = `[${bar(ctxPct)}] ${Math.round(ctxPct * 100)}%`;

	return composeCenteredFooterLine(` ${model}`, mindPart, `${ctxPart} `, width);
}

function applyFooter(ctx: ExtensionContext): void {
	state.ctx = ctx;
	ctx.ui.setFooter((_tui: unknown, theme: any, _footerData: unknown) => ({
		dispose: () => {},
		invalidate() {},
		render(width: number): string[] {
			return [renderFooter(width, theme)];
		},
	}));
}

function applyMindPayload(payload: any): void {
	const events = Array.isArray(payload?.events) ? payload.events : [];
	const latest = events.length > 0 ? events[0] : undefined;
	const statusRaw = typeof latest?.status === "string" ? latest.status : "";

	switch (statusRaw) {
		case "queued":
		case "running":
		case "success":
		case "fallback":
		case "error":
			state.mindStatus = statusRaw;
			break;
		default:
			break;
	}

	state.mindReason = typeof latest?.reason === "string" ? latest.reason : undefined;
	const progress = parseMindProgress(latest?.progress) ?? parseMindProgress(payload?.progress);
	if (progress) {
		state.mindProgress = progress;
	}
	if (typeof payload?.updated_at_ms === "number") {
		state.mindUpdatedAtMs = payload.updated_at_ms;
	}
}

function parseEnvelope(line: string): any | undefined {
	if (!line.trim()) return undefined;
	try {
		return JSON.parse(line);
	} catch {
		return undefined;
	}
}

function writeEnvelope(socket: Socket | undefined, envelope: any): void {
	if (!socket || socket.destroyed) return;
	socket.write(JSON.stringify(envelope) + "\n");
}

function pulseIdentity(ctx: ExtensionContext): { sessionId: string; paneId: string; senderId: string; agentId: string } | undefined {
	const sessionId = process.env.AOC_SESSION_ID?.trim();
	const paneId = process.env.AOC_PANE_ID?.trim();
	if (!sessionId || !paneId) return undefined;
	const senderId = `pi-minimal-${process.pid}`;
	const agentId = `${sessionId}::${paneId}`;
	return { sessionId, paneId, senderId, agentId };
}

function sendPulseCommand(ctx: ExtensionContext, command: string, args: Record<string, unknown>): boolean {
	const identity = pulseIdentity(ctx);
	if (!identity) return false;
	const socket = state.pulseSocket;
	if (!socket || socket.destroyed || !state.pulseConnected) return false;
	writeEnvelope(socket, {
		version: "1",
		type: "command",
		session_id: identity.sessionId,
		sender_id: identity.senderId,
		request_id: `${command}-${Date.now()}`,
		timestamp: nowIso(),
		payload: {
			command,
			target_agent_id: identity.agentId,
			args,
		},
	});
	return true;
}

function startPulse(ctx: ExtensionContext): void {
	const identity = pulseIdentity(ctx);
	if (!identity) return;
	const { sessionId, paneId, senderId, agentId } = identity;

	const socketPath = pulseSocketPath(sessionId);

	const connect = () => {
		if (state.pulseSocket && !state.pulseSocket.destroyed) return;

		const socket = createConnection(socketPath);
		state.pulseSocket = socket;
		state.pulseBuffer = "";

		socket.on("connect", () => {
			state.pulseConnected = true;

			writeEnvelope(socket, {
				version: "1",
				type: "hello",
				session_id: sessionId,
				sender_id: senderId,
				timestamp: nowIso(),
				payload: {
					client_id: senderId,
					role: "subscriber",
					capabilities: ["mind_observer"],
					agent_id: agentId,
					pane_id: paneId,
				},
			});

			writeEnvelope(socket, {
				version: "1",
				type: "subscribe",
				session_id: sessionId,
				sender_id: senderId,
				timestamp: nowIso(),
				payload: {
					topics: ["agent_state", "health"],
				},
			});
		});

		socket.on("data", (chunk: Buffer) => {
			state.pulseBuffer += chunk.toString("utf8");
			for (;;) {
				const idx = state.pulseBuffer.indexOf("\n");
				if (idx < 0) break;
				const line = state.pulseBuffer.slice(0, idx);
				state.pulseBuffer = state.pulseBuffer.slice(idx + 1);
				const env = parseEnvelope(line);
				if (!env || env.session_id !== sessionId) continue;

				if (env.type === "snapshot") {
					const states = Array.isArray(env.payload?.states) ? env.payload.states : [];
					const mine = states.find((s: any) => s?.agent_id === agentId);
					if (mine?.source?.mind_observer) applyMindPayload(mine.source.mind_observer);
				}

				if (env.type === "delta") {
					const changes = Array.isArray(env.payload?.changes) ? env.payload.changes : [];
					for (const change of changes) {
						if (change?.agent_id !== agentId) continue;
						if (change?.op === "remove") {
							state.mindStatus = "idle";
							state.mindProgress = undefined;
							continue;
						}
						if (change?.state?.source?.mind_observer) {
							applyMindPayload(change.state.source.mind_observer);
						}
					}
				}

				if (env.type === "command_result" && env.request_id && env.request_id === state.lastPulseRequestId) {
					if (env.payload?.command === "run_observer" && env.payload?.status === "accepted") {
						state.mindStatus = "queued";
					}
				}
			}
		});

		const scheduleReconnect = () => {
			state.pulseConnected = false;
			if (state.pulseSocket && !state.pulseSocket.destroyed) state.pulseSocket.destroy();
			state.pulseSocket = undefined;
			if (state.reconnectTimer) clearTimeout(state.reconnectTimer);
			state.reconnectTimer = setTimeout(connect, 2000);
		};

		socket.on("error", () => scheduleReconnect());
		socket.on("close", () => scheduleReconnect());
	};

	connect();
}

function extractCommandHintFromToolResult(message: any): string | undefined {
	const details = message?.details;
	if (details && typeof details === "object") {
		const command = (details as any).command;
		if (typeof command === "string" && command.trim().length > 0) {
			return command.trim();
		}
	}
	const text = blocksToText(message?.content);
	const cmdPrefix = "Command:";
	const idx = text.indexOf(cmdPrefix);
	if (idx >= 0) {
		const line = text.slice(idx + cmdPrefix.length).split("\n")[0]?.trim();
		if (line) return line;
	}
	return undefined;
}

function buildMindIngestPayload(message: any, ctx: ExtensionContext): Record<string, unknown> | undefined {
	const conversationId = ctx.sessionManager.getSessionId?.();
	if (!conversationId || typeof conversationId !== "string") return undefined;
	const timestampMs = typeof message?.timestamp === "number" ? Math.round(message.timestamp) : Date.now();
	const role = typeof message?.role === "string" ? message.role : "";
	const eventIdSeed = JSON.stringify({
		role,
		timestampMs,
		text: blocksToText(message?.content),
		tool: message?.toolName,
		details: message?.details,
	});
	const eventId = `pi:${conversationId}:${stableHashHex(eventIdSeed)}`;

	if (role === "user" || role === "assistant" || role === "system") {
		return {
			conversation_id: conversationId,
			event_id: eventId,
			timestamp_ms: timestampMs,
			body: {
				kind: "message",
				role,
				text: blocksToText(message?.content),
			},
		};
	}

	if (role === "toolResult") {
		const toolName = typeof message?.toolName === "string" ? message.toolName : "tool";
		const details = message?.details ?? {};
		const outputText = blocksToText(message?.content);
		return {
			conversation_id: conversationId,
			event_id: eventId,
			timestamp_ms: timestampMs,
			body: {
				kind: "tool_result",
				tool_name: toolName,
				is_error: Boolean(message?.isError),
				latency_ms: typeof details?.latencyMs === "number"
					? details.latencyMs
					: (typeof details?.latency_ms === "number" ? details.latency_ms : undefined),
				exit_code: typeof details?.exitCode === "number"
					? details.exitCode
					: (typeof details?.exit_code === "number" ? details.exit_code : undefined),
				output: outputText || undefined,
				redacted: Boolean(details?.redacted),
			},
		};
	}

	return undefined;
}

function maybeIngestMindEvent(message: any, ctx: ExtensionContext): void {
	const payload = buildMindIngestPayload(message, ctx);
	if (!payload) return;
	sendPulseCommand(ctx, "mind_ingest_event", payload);

	if (message?.role === "toolResult") {
		const toolName = typeof message?.toolName === "string" ? message.toolName.toLowerCase() : "";
		if (toolName === "bash") {
			const commandHint = extractCommandHintFromToolResult(message) ?? "";
			if (/\baoc-stm\s+handoff\b/i.test(commandHint)) {
				sendPulseCommand(ctx, "mind_handoff", {
					conversation_id: ctx.sessionManager.getSessionId?.(),
					reason: "stm handoff",
				});
			}
		}
	}
}

function requestManualObserverRun(ctx: ExtensionContext): boolean {
	const identity = pulseIdentity(ctx);
	if (!identity) return false;
	const socket = state.pulseSocket;
	if (!socket || socket.destroyed || !state.pulseConnected) return false;

	const requestId = `mind-run-${Date.now()}`;
	state.lastPulseRequestId = requestId;

	writeEnvelope(socket, {
		version: "1",
		type: "command",
		session_id: identity.sessionId,
		sender_id: identity.senderId,
		request_id: requestId,
		timestamp: nowIso(),
		payload: {
			command: "run_observer",
			target_agent_id: identity.agentId,
			args: {
				reason: "pi shortcut",
				conversation_id: ctx.sessionManager.getSessionId?.(),
			},
		},
	});
	state.mindStatus = "queued";
	return true;
}

function startRefreshLoop(ctx: ExtensionContext): void {
	if (state.refreshTimer) clearInterval(state.refreshTimer);
	state.refreshTimer = setInterval(() => {
		const now = Date.now();
		if (!state.lastTokenRecomputeAtMs || now - state.lastTokenRecomputeAtMs >= TOKEN_RECOMPUTE_INTERVAL_MS) {
			recomputeFilteredTokens(ctx);
		}
		applyFooter(ctx);
	}, REFRESH_INTERVAL_MS);
}

function bootstrap(ctx: ExtensionContext, options?: { animateOnStart?: boolean }): void {
	if (options?.animateOnStart) {
		state.sessionStartAnimationStartedAtMs = Date.now();
	} else {
		state.sessionStartAnimationStartedAtMs = undefined;
	}
	applyExtensionDefaults(import.meta.url, ctx);
	recomputeFilteredTokens(ctx);
	applyFooter(ctx);
	startRefreshLoop(ctx);
	startPulse(ctx);
	state.initialized = true;
}

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event, ctx) => {
		if (!ctx?.ui) return;
		bootstrap(ctx, { animateOnStart: true });
	});

	pi.on("session_switch", async (_event, ctx) => {
		if (!ctx?.ui) return;
		bootstrap(ctx);
	});

	pi.on("message_end", async (event, ctx) => {
		maybeIngestMindEvent((event as any)?.message, ctx);
		recomputeFilteredTokens(ctx);
		applyFooter(ctx);
	});

	pi.on("turn_end", async (_event, ctx) => {
		recomputeFilteredTokens(ctx);
		applyFooter(ctx);
	});

	pi.registerShortcut("ctrl+shift+o", {
		description: "Trigger AOC Mind observer run",
		handler: async (ctx) => {
			const ok = requestManualObserverRun(ctx);
			ctx.ui.notify(ok ? "Observer run queued" : "Observer run unavailable (Pulse disconnected)", ok ? "info" : "warning");
		},
	});

	pi.on("session_shutdown", async () => {
		if (state.refreshTimer) clearInterval(state.refreshTimer);
		if (state.reconnectTimer) clearTimeout(state.reconnectTimer);
		if (state.pulseSocket && !state.pulseSocket.destroyed) state.pulseSocket.destroy();
		state.pulseSocket = undefined;
		state.pulseConnected = false;
	});
}
