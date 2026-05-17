import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { matchesKey, truncateToWidth, visibleWidth, wrapTextWithAnsi } from "@mariozechner/pi-tui";
import * as fs from "node:fs";
import {
	currentMindSnapshot,
	finalizeMindSession,
	formatMindStatus,
	fetchMindContextPack,
	readStandaloneMindStatus,
	renderContextPackPrelude,
	requestManualObserverRun,
} from "./lib/mind.ts";

function formatMindRuntimeMode(standalone: any): string {
	const serviceStatus = standalone?.service_status;
	const lease = standalone?.service_lease;
	if (serviceStatus?.heartbeat_fresh || serviceStatus?.state === "warm" || lease?.owner_id) return "managed/warm";
	if (serviceStatus?.state === "cold") return "on-demand/cold (expected outside managed AOC session)";
	return "on-demand";
}

async function notifyMindStatus(ctx: any): Promise<void> {
	const snapshot = currentMindSnapshot(ctx);
	const standalone = readStandaloneMindStatus(ctx);
	const lines = [
		"mind_runtime: " + formatMindRuntimeMode(standalone),
		"startup_context: metadata-only",
		"default_context: none/lazy",
		...formatMindStatus(snapshot),
	];
	if (typeof standalone?.store_exists === "boolean") lines.push(`mind_store_exists: ${standalone.store_exists ? "yes" : "no"}`);
	if (typeof standalone?.latest_pi_session_file === "string" && standalone.latest_pi_session_file.trim()) {
		lines.push(`latest_pi_session_file: ${standalone.latest_pi_session_file}`);
	}
	const serviceStatus = standalone?.service_status;
	if (serviceStatus?.state) lines.push(`service_state: ${serviceStatus.state}`);
	if (typeof serviceStatus?.stale === "boolean") lines.push(`service_stale: ${serviceStatus.stale ? "yes" : "no"}`);
	if (serviceStatus?.blocker) lines.push(`service_blocker: ${serviceStatus.blocker}`);
	const lease = standalone?.service_lease;
	if (lease?.owner_id) lines.push(`service_lease_owner: ${lease.owner_id}`);
	const health = standalone?.health_snapshot;
	if (health?.lifecycle) lines.push(`service_lifecycle: ${health.lifecycle}`);
	if (typeof health?.queue_depth === "number") lines.push(`queue_depth: ${health.queue_depth}`);
	if (typeof health?.t3_queue_depth === "number") lines.push(`t3_queue_depth: ${health.t3_queue_depth}`);
	if (health?.last_error) lines.push(`service_last_error: ${health.last_error}`);
	ctx.ui.notify(lines.join("\n"), "info");
}

async function notifyMindDebug(ctx: any): Promise<void> {
	const snapshot = currentMindSnapshot(ctx);
	const lines = formatMindStatus(snapshot);
	lines.push(`mind_store_exists: ${fs.existsSync(snapshot.storePath) ? "yes" : "no"}`);
	ctx.ui.notify(lines.join("\n"), "info");
}

type MindMenuAction = "status" | "focused-pack" | "resume-pack" | "observer" | "finalize" | "store" | "debug";

type MindMenuItem = {
	label: string;
	action: MindMenuAction;
	description: string;
};

const MIND_MENU_ITEMS: MindMenuItem[] = [
	{ label: "Status", action: "status", description: "Show compact standalone Mind runtime, store, session, queue, and last command status." },
	{ label: "Focused context", action: "focused-pack", description: "Render a focused project Mind context pack inside Pi. No floating pane or external TUI." },
	{ label: "Resume context", action: "resume-pack", description: "Render a compact resume context pack inside Pi for continuation work." },
	{ label: "Observer run", action: "observer", description: "Queue a project-scoped Mind observer run for the current Pi session/pane." },
	{ label: "Finalize session", action: "finalize", description: "Finalize the current Mind session slice and enqueue export/T3 processing." },
	{ label: "Store path", action: "store", description: "Show the active project Mind SQLite store path." },
	{ label: "Debug ingest", action: "debug", description: "Show detailed local Mind ingest transport/debug state." },
];

function padRight(text: string, width: number): string {
	const current = visibleWidth(text);
	return current >= width ? truncateToWidth(text, width) : `${text}${" ".repeat(width - current)}`;
}

async function showContextPack(ctx: any, mode: string, reason: string): Promise<void> {
	const pack = await fetchMindContextPack(ctx, mode, false, "operator", reason);
	const rendered = renderContextPackPrelude(pack);
	ctx.ui.notify(rendered || `Mind ${mode} context pack unavailable`, rendered ? "info" : "warning");
}

async function runMindMenuAction(action: MindMenuAction, ctx: any): Promise<void> {
	if (action === "status") return notifyMindStatus(ctx);
	if (action === "focused-pack") return showContextPack(ctx, "focused", "pi Alt+M focused context");
	if (action === "resume-pack") return showContextPack(ctx, "resume", "pi Alt+M resume context");
	if (action === "observer") {
		const result = await requestManualObserverRun(ctx);
		ctx.ui.notify(result.message, result.ok ? "success" : "warning");
		return;
	}
	if (action === "finalize") {
		const result = await finalizeMindSession(ctx, "pi Alt+M");
		ctx.ui.notify(result.message, result.ok ? "success" : "warning");
		return;
	}
	if (action === "store") {
		const snapshot = currentMindSnapshot(ctx);
		ctx.ui.notify(`Mind store path\n${snapshot.storePath}`, "info");
		return;
	}
	if (action === "debug") return notifyMindDebug(ctx);
}

async function openMindMenu(ctx: any): Promise<MindMenuAction | undefined> {
	return ctx.ui.custom<MindMenuAction | undefined>((_tui: any, theme: any, _keybindings: any, done: (value?: MindMenuAction) => void) => {
		let index = 0;
		const snapshot = currentMindSnapshot(ctx);
		const standalone = readStandaloneMindStatus(ctx);
		const statusBits = [
			formatMindRuntimeMode(standalone),
			`status ${snapshot.mindStatus}`,
			standalone?.store_exists ? "store yes" : undefined,
			snapshot.lastError ? "error" : undefined,
		].filter(Boolean).join(" · ");

		function move(delta: number): void {
			index = (index + delta + MIND_MENU_ITEMS.length) % MIND_MENU_ITEMS.length;
		}

		function renderPanel(width: number): string[] {
			const panelWidth = Math.max(58, Math.min(width, 96));
			const innerWidth = panelWidth - 2;
			const navWidth = Math.max(20, Math.floor(innerWidth * 0.34));
			const gap = 2;
			const detailWidth = Math.max(24, innerWidth - navWidth - gap * 2 - 1);
			const selected = MIND_MENU_ITEMS[index]!;
			const accent = (text: string) => theme.fg("accent", text);
			const dim = (text: string) => theme.fg("dim", text);
			const border = (text: string) => theme.fg("borderMuted", text);
			const strongBorder = (text: string) => theme.fg("borderAccent", text);
			const row = (left: string, right = "") => {
				const spacer = " ".repeat(Math.max(1, innerWidth - visibleWidth(left) - visibleWidth(right)));
				return `│${left}${spacer}${right}│`;
			};

			const lines: string[] = [];
			lines.push(strongBorder(`╭${"─".repeat(innerWidth)}╮`));
			lines.push(row(accent(theme.bold("AOC Mind")), accent(theme.bold("Alt+M"))));
			lines.push(row(dim(statusBits || "Mind status unavailable"), dim("instant overlay")));
			lines.push(border(`├${"─".repeat(innerWidth)}┤`));

			const bodyHeight = 9;
			const navLines = MIND_MENU_ITEMS.map((item, i) => i === index ? accent(`▸ ${item.label}`) : `  ${item.label}`);
			while (navLines.length < bodyHeight) navLines.push("");
			const detailLines = [
				accent(theme.bold(selected.label)),
				...wrapTextWithAnsi(selected.description, detailWidth),
				"",
				dim("Enter runs selected action."),
				dim("All actions run inside Pi; no external floating UI."),
			];
			while (detailLines.length < bodyHeight) detailLines.push("");

			for (let i = 0; i < bodyHeight; i++) {
				const left = padRight(navLines[i] || "", navWidth);
				const right = padRight(detailLines[i] || "", detailWidth);
				lines.push(`│ ${left}${" ".repeat(gap)}${border("│")}${" ".repeat(gap)}${right} │`);
			}

			lines.push(border(`├${"─".repeat(innerWidth)}┤`));
			lines.push(row(dim("[j/k] move  [enter] run  [esc/q] close"), dim("Memory / Mind")));
			lines.push(strongBorder(`╰${"─".repeat(innerWidth)}╯`));
			return lines;
		}

		return {
			render(width: number) {
				const blank = theme.bg("customMessageBg", " ".repeat(width));
				return [
					blank,
					...renderPanel(width - 2).map((line) => theme.bg("customMessageBg", padRight(` ${line}`, width))),
					blank,
				];
			},
			invalidate() {},
			handleInput(data: string) {
				if (data === "j" || matchesKey(data, "down")) return move(1);
				if (data === "k" || matchesKey(data, "up")) return move(-1);
				if (matchesKey(data, "return")) return done(MIND_MENU_ITEMS[index]?.action);
				if (data === "q" || matchesKey(data, "escape") || matchesKey(data, "alt+m")) return done(undefined);
			},
		};
	}, {
		overlay: true,
		overlayOptions: {
			width: "58%",
			minWidth: 60,
			maxHeight: "65%",
			anchor: "center",
			margin: 1,
		},
	});
}

export default function (pi: ExtensionAPI) {
	pi.registerCommand("mind", {
		description: "Open the AOC Mind command overlay",
		handler: async (_args, ctx) => {
			const action = await openMindMenu(ctx);
			if (!action) return;
			await runMindMenuAction(action, ctx);
		},
	});

	pi.registerCommand("mind-observer-run", {
		description: "Manually queue an AOC Mind observer run",
		handler: async (_args, ctx) => {
			const result = await requestManualObserverRun(ctx);
			ctx.ui.notify(result.message, result.ok ? "info" : "warning");
		},
	});

	pi.registerCommand("mind-finalize", {
		description: "Finalize the current Mind session slice and enqueue export/T3 processing",
		handler: async (_args, ctx) => {
			const result = await finalizeMindSession(ctx, "pi /mind-finalize");
			ctx.ui.notify(result.message, result.ok ? "info" : "warning");
		},
	});

	pi.registerCommand("mind-status", {
		description: "Show standalone Mind ingest/runtime health for the current project",
		handler: async (_args, ctx) => {
			await notifyMindStatus(ctx);
		},
	});

	pi.registerCommand("aoc-status", {
		description: "Show managed AOC launch/runtime health for the current Pi session",
		handler: async (_args, ctx) => {
			await notifyMindStatus(ctx);
		},
	});

	pi.registerCommand("mind-store-path", {
		description: "Show the active Mind store path for this project/session",
		handler: async (_args, ctx) => {
			const snapshot = currentMindSnapshot(ctx);
			ctx.ui.notify(`Mind store path\n${snapshot.storePath}`, "info");
		},
	});

	pi.registerCommand("mind-debug-ingest", {
		description: "Show detailed Mind ingest transport/debug state",
		handler: async (_args, ctx) => {
			await notifyMindDebug(ctx);
		},
	});

	pi.registerShortcut("alt+m", {
		description: "Open the AOC Mind command overlay",
		handler: async (ctx) => {
			const action = await openMindMenu(ctx);
			if (!action) return;
			await runMindMenuAction(action, ctx);
		},
	});
}
