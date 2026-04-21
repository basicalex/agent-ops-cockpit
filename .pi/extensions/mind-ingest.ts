import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import {
	ingestMindMessage,
	rememberCompactionPreparation,
	sendMindCompactionCheckpoint,
} from "./lib/mind.ts";

function warnIngestUnavailable(ctx: any, reason: string): void {
	ctx?.ui?.setStatus?.("mind", `sync pending (${reason})`);
	ctx?.ui?.notify?.(`Mind sync failed: ${reason}`, "warning");
}

export default function (pi: ExtensionAPI) {
	pi.on("message_end", async (event, ctx) => {
		const result = await ingestMindMessage((event as any)?.message, ctx);
		if (!result.ok) warnIngestUnavailable(ctx, result.error ?? "mind sync failed");
	});

	pi.on("session_before_compact", async (event, _ctx) => {
		const preparation = (event as any)?.preparation;
		rememberCompactionPreparation({
			capturedAtMs: Date.now(),
			firstKeptEntryId: typeof preparation?.firstKeptEntryId === "string" ? preparation.firstKeptEntryId : undefined,
			tokensBefore: typeof preparation?.tokensBefore === "number" ? preparation.tokensBefore : undefined,
		});
	});

	pi.on("session_compact", async (event, ctx) => {
		const result = await sendMindCompactionCheckpoint(event as any, ctx);
		if (!result.ok) warnIngestUnavailable(ctx, result.error ?? "mind sync failed");
	});

}
