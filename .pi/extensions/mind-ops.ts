import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import * as fs from "node:fs";
import {
	currentMindSnapshot,
	finalizeMindSession,
	formatMindStatus,
	launchMindUi,
	readStandaloneMindStatus,
	requestManualObserverRun,
} from "./lib/mind.ts";

async function notifyMindStatus(ctx: any): Promise<void> {
	const snapshot = currentMindSnapshot(ctx);
	const standalone = readStandaloneMindStatus(ctx);
	const lines = formatMindStatus(snapshot);
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

export default function (pi: ExtensionAPI) {
	pi.registerCommand("mind", {
		description: "Open or toggle the project-scoped AOC Mind floating UI",
		handler: async (_args, ctx) => {
			const result = launchMindUi(ctx);
			ctx.ui.notify(result.message, result.ok ? "info" : "warning");
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
		description: "Open or toggle the project-scoped AOC Mind floating UI",
		handler: async (ctx) => {
			const result = launchMindUi(ctx);
			ctx.ui.notify(result.message, result.ok ? "info" : "warning");
		},
	});
}
