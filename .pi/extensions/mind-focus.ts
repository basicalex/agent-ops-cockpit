import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { blocksToText, inferFocusSnapshot } from "./lib/mind.ts";

function recentMessage(ctx: any): any | undefined {
	const branch = ctx.sessionManager?.getBranch?.() ?? [];
	for (let index = branch.length - 1; index >= 0; index--) {
		const entry = branch[index];
		if ((entry as any)?.type === "message") return (entry as any).message;
	}
	return undefined;
}

export default function (pi: ExtensionAPI) {
	pi.registerCommand("mind-focus", {
		description: "Show the current inferred Mind focus/task/file hints for this Pi session",
		handler: async (_args, ctx) => {
			const message = recentMessage(ctx);
			const focus = inferFocusSnapshot(message, ctx);
			const preview = message ? blocksToText(message.content).split("\n").slice(0, 3).join(" ").trim() : "none";
			const lines = [
				`project_root: ${focus.projectRoot}`,
				`store_path: ${focus.storePath}`,
				`focus: ${focus.focusLabel || "none"}`,
				`task_ids: ${focus.taskIds.length > 0 ? focus.taskIds.join(", ") : "none"}`,
				`file_paths: ${focus.filePaths.length > 0 ? focus.filePaths.join(", ") : "none"}`,
				`command_hint: ${focus.commandHint || "none"}`,
				`preview: ${preview || "none"}`,
			];
			ctx.ui.notify(lines.join("\n"), "info");
		},
	});
}
