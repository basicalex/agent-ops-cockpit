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
		description: "Render a compact Mind context pack for the current project/session",
		handler: async (args, ctx) => {
			const mode = args?.[0] || "handoff";
			await showPack(ctx, String(mode), false, "operator", `pi /mind-pack ${String(mode)}`);
		},
	});

	pi.registerCommand("mind-pack-expanded", {
		description: "Render an expanded Mind context pack for the current project/session",
		handler: async (args, ctx) => {
			const mode = args?.[0] || "handoff";
			await showPack(ctx, String(mode), true, "operator", `pi /mind-pack-expanded ${String(mode)}`);
		},
	});
}
