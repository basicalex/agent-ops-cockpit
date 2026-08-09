import { describe, expect, it } from "bun:test";
import goalCompact, {
	pruneGoalContextMessages,
	recomputeGoalCutPoint,
	shouldCancelFrozenCompaction,
} from "./aoc-goal-compact.ts";

function naiveMessageOnlyCut(entries: readonly Record<string, unknown>[], keepRecentTokens: number): string | undefined {
	let accumulatedTokens = 0;
	for (let index = entries.length - 1; index >= 0; index--) {
		const entry = entries[index];
		if (entry.type !== "message") continue;
		const message = entry.message;
		if (typeof message !== "object" || message === null || !("content" in message)) continue;
		let characters = 0;
		if (typeof message.content === "string") {
			characters = message.content.length;
		} else if (Array.isArray(message.content)) {
			for (const block of message.content) {
				if (block && typeof block === "object" && "text" in block && typeof block.text === "string") {
					characters += block.text.length;
				}
			}
		}
		accumulatedTokens += Math.ceil(characters / 4);
		if (accumulatedTokens >= keepRecentTokens) return typeof entry.id === "string" ? entry.id : undefined;
	}
	return typeof entries[0]?.id === "string" ? entries[0].id : undefined;
}

describe("pruneGoalContextMessages", () => {
	it("keeps only the newest goal context while preserving every other message and order", () => {
		const system = { role: "system", content: "system" };
		const oldGoal = { role: "custom", customType: "goal_context", content: "old" };
		const user = { role: "user", content: "work" };
		const bookkeeping = { role: "custom", customType: "other", content: "keep" };
		const newestGoal = { role: "custom", customType: "goal_context", content: "new" };
		const assistant = { role: "assistant", content: "done" };

		const result = pruneGoalContextMessages([
			system,
			oldGoal,
			user,
			bookkeeping,
			newestGoal,
			assistant,
		]);

		expect(result).toEqual([system, user, bookkeeping, newestGoal, assistant]);
		expect(result[0]).toBe(system);
		expect(result[3]).toBe(newestGoal);
	});
});

describe("recomputeGoalCutPoint", () => {
	it("counts custom messages that the prime-agent 0.7.1 cut-point search skips", () => {
		const entries = [
			{
				type: "message",
				id: "old-user",
				parentId: null,
				timestamp: "2026-01-01T00:00:00.000Z",
				message: { role: "user", content: "old" },
			},
			{
				type: "message",
				id: "old-assistant",
				parentId: "old-user",
				timestamp: "2026-01-01T00:00:01.000Z",
				message: { role: "assistant", content: [{ type: "text", text: "small" }] },
			},
			{
				type: "custom_message",
				id: "large-goal-context",
				parentId: "old-assistant",
				timestamp: "2026-01-01T00:00:02.000Z",
				customType: "goal_context",
				content: "x".repeat(800),
				display: false,
			},
			{
				type: "message",
				id: "recent-assistant",
				parentId: "large-goal-context",
				timestamp: "2026-01-01T00:00:03.000Z",
				message: { role: "assistant", content: [{ type: "text", text: "recent" }] },
			},
		];
		const preparation = {
			firstKeptEntryId: "old-user",
			tokensBefore: 250,
			settings: { keepRecentTokens: 100, reserveTokens: 16_384 },
		};

		const corrected = recomputeGoalCutPoint(entries as never, preparation);
		const naiveFirstKeptEntryId = naiveMessageOnlyCut(entries, 100);

		expect(naiveFirstKeptEntryId).toBe("old-user");
		expect(corrected.firstKeptEntryId).toBe("large-goal-context");
		expect(corrected.firstKeptEntryId).not.toBe(naiveFirstKeptEntryId);
	});
});

describe("frozen compaction guard", () => {
	it("cancels when the corrected and prepared keep point repeats the previous compaction", async () => {
		const entries = [
			{
				type: "message",
				id: "kept",
				parentId: null,
				timestamp: "2026-01-01T00:00:00.000Z",
				message: { role: "user", content: "old" },
			},
			{
				type: "compaction",
				id: "previous-compaction",
				parentId: "kept",
				timestamp: "2026-01-01T00:00:01.000Z",
				summary: "prior summary",
				firstKeptEntryId: "kept",
				tokensBefore: 200_000,
			},
			{
				type: "message",
				id: "recent",
				parentId: "previous-compaction",
				timestamp: "2026-01-01T00:00:02.000Z",
				message: { role: "assistant", content: [{ type: "text", text: "recent" }] },
			},
		];
		const preparation = {
			firstKeptEntryId: "kept",
			tokensBefore: 210_000,
			previousSummary: "prior summary",
			settings: { keepRecentTokens: 20_000, reserveTokens: 16_384 },
		};

		expect(shouldCancelFrozenCompaction(entries as never, "kept")).toBe(true);

		type Handler = (event: unknown, context: unknown) => unknown;
		const handlers = new Map<string, Handler>();
		goalCompact({
			on(event: string, handler: Handler) {
				handlers.set(event, handler);
			},
		} as never);
		const beforeCompact = handlers.get("session_before_compact");
		expect(beforeCompact).toBeDefined();
		const result = await beforeCompact?.(
			{
				preparation,
				branchEntries: entries,
				signal: new AbortController().signal,
			},
			{},
		);
		expect(result).toEqual({ cancel: true });
	});
});

describe("turn-end compaction trigger", () => {
	it("uses the API's 0–100 percentage scale and blocks re-entry", () => {
		type Handler = (event: unknown, context: unknown) => unknown;
		const handlers = new Map<string, Handler>();
		goalCompact({
			on(event: string, handler: Handler) {
				handlers.set(event, handler);
			},
		} as never);
		const turnEnd = handlers.get("turn_end");
		const sessionCompact = handlers.get("session_compact");
		const agentEnd = handlers.get("agent_end");
		let percent = 50;
		let compactions = 0;
		const context = {
			getContextUsage: () => ({ tokens: 50, contextWindow: 100, percent }),
			compact: () => {
				compactions++;
			},
		};

		turnEnd?.({}, context);
		expect(compactions).toBe(0);
		percent = 76;
		turnEnd?.({}, context);
		turnEnd?.({}, context);
		expect(compactions).toBe(1);
		sessionCompact?.({}, context);
		turnEnd?.({}, context);
		expect(compactions).toBe(2);
		agentEnd?.({}, context);
	});
});
