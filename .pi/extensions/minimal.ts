/**
 * Minimal — Model + Mind observer HUD in compact footer
 *
 * Shows:
 * - active model
 * - AOC Mind observer state (queued/running/success/fallback/error)
 * - T1 pre-filter load bar (authoritative feed when available, deterministic local fallback)
 * - session context usage bar
 *
 * Mind transport / commands now live in the native Mind extension stack:
 * - `mind-ingest.ts`
 * - `mind-ops.ts`
 * - `mind-context.ts`
 * - `mind-focus.ts`
 */

import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { applyExtensionDefaults } from "./themeMap.ts";
import type { AutocompleteItem } from "@mariozechner/pi-tui";
import { truncateToWidth, visibleWidth } from "@mariozechner/pi-tui";
import { currentMindSnapshot } from "./lib/mind.ts";
import { CAVEMAN_EVENT_SET_LEVEL, CAVEMAN_LEVELS, type CavemanLevel } from "./lib/caveman.ts";

type MindStatus = "idle" | "queued" | "running" | "success" | "fallback" | "error";

type Sample = { at: number; tokens: number };

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

type ExtensionState = {
	ctx?: ExtensionContext;
	initialized: boolean;
	filteredTokens: number;
	samples: Sample[];
	lastEstimateAtMs?: number;
	lastTokenRecomputeAtMs?: number;
	sessionStartAnimationStartedAtMs?: number;
	refreshTimer?: NodeJS.Timeout;
	cavemanLevel: CavemanLevel;
	lastModelId?: string;
	lastContextUsagePct: number;
};

const T1_TARGET_TOKENS = 28_000;
const SAMPLE_WINDOW_MS = 10 * 60 * 1000;
const REFRESH_INTERVAL_MS = 117;
const TOKEN_RECOMPUTE_INTERVAL_MS = 2_000;
const RUNNING_ANIMATION_STEP_MS = 117;
const MIND_BAR_WIDTH = 10;
const SESSION_START_ANIMATION_STEPS = Math.max(1, (MIND_BAR_WIDTH - 1) * 2);
const SESSION_START_ANIMATION_MS = (SESSION_START_ANIMATION_STEPS + 1) * RUNNING_ANIMATION_STEP_MS;
const CAVEMAN_LABELS: Record<CavemanLevel, string> = { off: "off", lite: "lite", full: "full", ultra: "ultra" };

const state: ExtensionState = {
	initialized: false,
	filteredTokens: 0,
	samples: [],
	cavemanLevel: "off",
	lastModelId: undefined,
	lastContextUsagePct: 0,
};

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

function composeFooterWithBridges(
	left: string,
	bridgeLeft: string,
	center: string,
	bridgeRight: string,
	right: string,
	width: number,
): string {
	const leftWidth = visibleWidth(left);
	const bridgeLeftWidth = visibleWidth(bridgeLeft);
	const centerWidth = visibleWidth(center);
	const bridgeRightWidth = visibleWidth(bridgeRight);
	const rightWidth = visibleWidth(right);
	const minimum = leftWidth + centerWidth + rightWidth + 4;
	if (minimum > width) {
		return truncateToWidth(`${left} ${bridgeLeft} ${center} ${bridgeRight} ${right}`, width);
	}

	const rightStart = Math.max(0, width - rightWidth);
	let centerStart = Math.floor((width - centerWidth) / 2);
	centerStart = Math.max(centerStart, leftWidth + 2);
	centerStart = Math.min(centerStart, rightStart - centerWidth - 2);
	if (centerStart < leftWidth + 2 || centerStart + centerWidth >= rightStart - 1) {
		return truncateToWidth(`${left} ${bridgeLeft} ${center} ${bridgeRight} ${right}`, width);
	}

	const leftGapStart = leftWidth;
	const leftGapEnd = centerStart;
	const leftGapWidth = leftGapEnd - leftGapStart;
	const rightGapStart = centerStart + centerWidth;
	const rightGapEnd = rightStart;
	const rightGapWidth = rightGapEnd - rightGapStart;
	if (leftGapWidth < bridgeLeftWidth + 2 || rightGapWidth < bridgeRightWidth + 2) {
		return truncateToWidth(`${left} ${bridgeLeft} ${center} ${bridgeRight} ${right}`, width);
	}

	const leftBridgeStart = leftGapStart + Math.floor((leftGapWidth - bridgeLeftWidth) / 2);
	const rightBridgeStart = rightGapStart + Math.floor((rightGapWidth - bridgeRightWidth) / 2);
	const leftPad = " ".repeat(Math.max(0, leftBridgeStart - leftWidth));
	const betweenLeftAndCenter = " ".repeat(Math.max(1, centerStart - (leftBridgeStart + bridgeLeftWidth)));
	const betweenCenterAndRightBridge = " ".repeat(Math.max(1, rightBridgeStart - (rightGapStart)));
	const tailPad = " ".repeat(Math.max(1, rightStart - (rightBridgeStart + bridgeRightWidth)));

	return truncateToWidth(
		`${left}${leftPad}${bridgeLeft}${betweenLeftAndCenter}${center}${betweenCenterAndRightBridge}${bridgeRight}${tailPad}${right}`,
		width,
	);
}

function captureFooterSnapshot(ctx?: ExtensionContext): void {
	if (!ctx) return;
	try {
		state.lastModelId = ctx.model?.id || state.lastModelId || "no-model";
		const usage = ctx.getContextUsage?.();
		state.lastContextUsagePct = usage && usage.percent !== null ? Number(usage.percent) / 100 : 0;
	} catch {
		// Session replacement invalidates old ctx objects. Keep last good snapshot.
	}
}

function renderFooter(width: number, _theme: any): string[] {
	captureFooterSnapshot(state.ctx);
	const model = state.lastModelId || "no-model";
	const ctxPct = state.lastContextUsagePct;
	const mind = currentMindSnapshot();

	const t0Tokens = mind.mindProgress?.t0_estimated_tokens ?? state.filteredTokens;
	const t1Target = mind.mindProgress?.t1_target_tokens ?? T1_TARGET_TOKENS;
	const mindLoadPct = Math.min(1, t0Tokens / Math.max(1, t1Target));
	const mindPart = `✦ [${mindBar(mindLoadPct, mind.mindStatus, MIND_BAR_WIDTH)}]✦`;
	const leftPart = ` ${model}`;
	const ctxPart = `[${bar(ctxPct)}] ${Math.round(ctxPct * 100)}% `;
	return [composeCenteredFooterLine(leftPart, mindPart, ctxPart, width)];
}

function applyCavemanLevel(pi: ExtensionAPI, next: CavemanLevel, options?: { silent?: boolean }): void {
	state.cavemanLevel = next;
	pi.appendEntry("caveman-level-v1", { cavemanLevel: next, level: next, at: Date.now() });
	if (state.ctx) applyFooter(state.ctx);
	if (!options?.silent) state.ctx?.ui.notify(`caveman: ${CAVEMAN_LABELS[next]}`, next === "off" ? "info" : "success");
}

function applyFooter(ctx: ExtensionContext): void {
	state.ctx = ctx;
	captureFooterSnapshot(ctx);
	ctx.ui.setFooter((_tui: unknown, theme: any, _footerData: unknown) => ({
		dispose: () => {},
		invalidate() {},
		render(width: number): string[] {
			return renderFooter(width, theme);
		},
	}));
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
	state.initialized = true;
}

function restoreCavemanLevel(ctx: ExtensionContext): void {
	for (const entry of [...ctx.sessionManager.getBranch()].reverse()) {
		if ((entry as any).type !== "custom" || (entry as any).customType !== "caveman-level-v1") continue;
		const data = (entry as any).data;
		const restored = data?.cavemanLevel ?? data?.level;
		if (restored && CAVEMAN_LEVELS.includes(restored)) {
			state.cavemanLevel = restored;
			return;
		}
	}
}

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event, ctx) => {
		if (!ctx?.ui) return;
		restoreCavemanLevel(ctx);
		bootstrap(ctx, { animateOnStart: true });
	});

	pi.on("message_end", async (_event, ctx) => {
		recomputeFilteredTokens(ctx);
		applyFooter(ctx);
	});

	pi.on("turn_end", async (_event, ctx) => {
		recomputeFilteredTokens(ctx);
		applyFooter(ctx);
	});

	pi.on("session_shutdown", async () => {
		if (state.refreshTimer) clearInterval(state.refreshTimer);
		state.refreshTimer = undefined;
		state.ctx = undefined;
	});

	pi.events.on(CAVEMAN_EVENT_SET_LEVEL, (data: unknown) => {
		const next = typeof data === "object" && data && "level" in (data as Record<string, unknown>)
			? (data as Record<string, unknown>).level
			: undefined;
		if (typeof next !== "string" || !CAVEMAN_LEVELS.includes(next as CavemanLevel)) return;
		applyCavemanLevel(pi, next as CavemanLevel);
	});

	// --- Caveman command -------------------------------------------
	pi.registerCommand("caveman", {
		description: "Toggle caveman: off → lite → full → ultra",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => {
			const items = (["off", "lite", "full", "ultra"] as const)
				.filter(l => l.startsWith(prefix))
				.map(l => ({ value: l, label: `caveman ${l}` }));
			return items.length > 0 ? items : null;
		},
		handler: async (args, _ctx) => {
			const arg = args?.trim().toLowerCase();
			let next: CavemanLevel;
			if (arg && CAVEMAN_LEVELS.includes(arg as CavemanLevel)) {
				next = arg as CavemanLevel;
			} else {
				const idx = CAVEMAN_LEVELS.indexOf(state.cavemanLevel);
				next = CAVEMAN_LEVELS[(idx + 1) % CAVEMAN_LEVELS.length];
			}
			applyCavemanLevel(pi, next);
		},
	});

	// --- Caveman prompt injection ----------------------------------
	pi.on("before_agent_start", async (event: any) => {
		const level = state.cavemanLevel;
		if (level === "off" || !event.systemPrompt) return;
		const rules: Record<string, string> = {
			lite: "Drop filler/hedging/pleasantries. Keep articles + full sentences. Professional but tight. Technical terms exact. Code unchanged.",
			full: "Drop articles/filler/hedging/pleasantries. Fragments OK. Short synonyms. Technical terms exact. Code unchanged. Pattern: [thing] [action] [reason]. [next step].",
			ultra: "Drop articles/filler/hedging/pleasantries/conjunctions. Abbreviate (DB/auth/req/res/fn). Arrows for causality (X→Y). One word when one word enough. Technical terms exact. Code unchanged.",
		};
		return {
			systemPrompt: `${event.systemPrompt}\n\n[CAVEMAN MODE: ${level.toUpperCase()}] ${rules[level]} Auto-clarity: use full English for security warnings, irreversible actions, and when user repeats a question. Resume caveman after.`,
		};
	});
}
