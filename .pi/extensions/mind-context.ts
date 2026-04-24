import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import {
	fetchMindContextPack,
	renderContextPackPrelude,
} from "./lib/mind.ts";

async function showPack(ctx: any, mode: string, detail: boolean, role = "operator", reason = "pi /mind-pack"): Promise<void> {
	const pack = await fetchMindContextPack(ctx, mode, detail, role, reason);
	const rendered = renderContextPackPrelude(pack);
	if (!rendered) {
		ctx.ui.notify("Mind context pack unavailable", "warning");
		return;
	}
	ctx.ui.notify(rendered, "info");
}

export default function (pi: ExtensionAPI) {
	pi.registerCommand("mind-pack", {
		description: "Render a focused/intent-bound Mind context pack; do not use as startup priming",
		handler: async (args, ctx) => {
			const mode = args?.[0] || "handoff";
			const reason = args?.slice(1).join(" ").trim() || `pi /mind-pack ${String(mode)}`;
			await showPack(ctx, String(mode), false, "operator", reason);
		},
	});

	pi.registerCommand("mind-pack-expanded", {
		description: "Render expanded Mind context only for explicit resume/audit/debug intent",
		handler: async (args, ctx) => {
			const mode = args?.[0] || "handoff";
			const reason = args?.slice(1).join(" ").trim() || `pi /mind-pack-expanded ${String(mode)}`;
			await showPack(ctx, String(mode), true, "operator", reason);
		},
	});

	pi.registerCommand("mind-resume", {
		description: "Load compact resume context after explicit continuation/resume intent",
		handler: async (_args, ctx) => {
			await showPack(ctx, "resume", false, "operator", "pi /mind-resume");
		},
	});
}
