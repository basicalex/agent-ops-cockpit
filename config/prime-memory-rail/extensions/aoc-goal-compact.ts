/**
 * aoc-goal-compact — goal-mode context and compaction guardrail.
 *
 * context: sends only the newest goal_context message to the provider.
 * session_before_compact: corrects prime-agent 0.7.1's cut-point accounting for
 * custom_message entries and cancels repeated no-op compactions.
 * turn_end: starts compaction above 75% context use, with a re-entry guard.
 *
 * Managed by agent-ops-cockpit (config/prime-memory-rail); installed by
 * bin/aoc-prime-memory-install. Edit in the repo, not in ~/.prime/agent/.
 */
import type { ExtensionAPI, ExtensionContext, SessionEntry } from "@earendil-works/pi-coding-agent";

const DEFAULT_KEEP_RECENT_TOKENS = 20_000;
const DEFAULT_RESERVE_TOKENS = 16_384;
const COMPACT_THRESHOLD_PERCENT = 75;
const FETCH_SUMMARY_APIS = new Set([
	"anthropic-messages",
	"google-generative-ai",
	"openai-completions",
	"openai-responses",
	"azure-openai-responses",
	"openai-codex-responses",
]);

type ContextMessage = { customType?: string } & Record<string, unknown>;

type PreparationShape = {
	firstKeptEntryId: string;
	tokensBefore: number;
	previousSummary?: string;
	settings?: {
		keepRecentTokens?: number;
		reserveTokens?: number;
	};
};

export interface CorrectedCutPoint {
	boundaryStart: number;
	cutIndex: number;
	firstKeptEntryId: string | undefined;
}

/** Return the original message objects in order, minus stale goal contexts. */
export function pruneGoalContextMessages<T extends ContextMessage>(messages: readonly T[]): T[] {
	let newestGoalContext = -1;
	for (let index = messages.length - 1; index >= 0; index--) {
		if (messages[index]?.customType === "goal_context") {
			newestGoalContext = index;
			break;
		}
	}
	if (newestGoalContext < 0) return [...messages];
	return messages.filter(
		(message, index) => message.customType !== "goal_context" || index === newestGoalContext,
	);
}

function contentCharacters(content: unknown): number {
	if (typeof content === "string") return content.length;
	if (!Array.isArray(content)) return 0;
	let characters = 0;
	for (const block of content) {
		if (!block || typeof block !== "object") continue;
		if ("type" in block && block.type === "text" && "text" in block && typeof block.text === "string") {
			characters += block.text.length;
		} else if ("type" in block && block.type === "image") {
			characters += 4_800;
		}
	}
	return characters;
}

/** Mirrors prime-agent 0.7.1 estimateTokens without adding a runtime package dependency. */
function estimateMessageTokens(message: Record<string, unknown>): number {
	let characters = 0;
	switch (message.role) {
		case "user":
		case "custom":
		case "toolResult":
			characters = contentCharacters(message.content);
			break;
		case "assistant":
			if (Array.isArray(message.content)) {
				for (const block of message.content) {
					if (!block || typeof block !== "object" || !("type" in block)) continue;
					if (block.type === "text" && "text" in block && typeof block.text === "string") {
						characters += block.text.length;
					} else if (block.type === "thinking" && "thinking" in block && typeof block.thinking === "string") {
						characters += block.thinking.length;
					} else if (block.type === "toolCall") {
						const name = "name" in block && typeof block.name === "string" ? block.name : "";
						const args = "arguments" in block ? block.arguments : undefined;
						characters += name.length + JSON.stringify(args).length;
					}
				}
			}
			break;
		case "bashExecution":
			characters =
				(typeof message.command === "string" ? message.command.length : 0) +
				(typeof message.output === "string" ? message.output.length : 0);
			break;
		case "branchSummary":
		case "compactionSummary":
			characters = typeof message.summary === "string" ? message.summary.length : 0;
			break;
	}
	return Math.ceil(characters / 4);
}

function entryTokens(entry: SessionEntry): number {
	if (entry.type === "message") {
		return estimateMessageTokens(entry.message as unknown as Record<string, unknown>);
	}
	if (entry.type === "custom_message") {
		return Math.ceil(contentCharacters(entry.content) / 4);
	}
	return 0;
}

function isValidCutPoint(entry: SessionEntry): boolean {
	if (entry.type === "custom_message" || entry.type === "branch_summary") return true;
	if (entry.type !== "message") return false;
	return ["bashExecution", "custom", "branchSummary", "compactionSummary", "user", "assistant"].includes(
		entry.message.role,
	);
}

function previousCompactionIndex(entries: readonly SessionEntry[], preparation: PreparationShape): number {
	for (let index = entries.length - 1; index >= 0; index--) {
		const entry = entries[index];
		if (entry.type !== "compaction") continue;
		if (preparation.previousSummary === undefined || entry.summary === preparation.previousSummary) return index;
	}
	return -1;
}

function compactionBoundary(entries: readonly SessionEntry[], preparation: PreparationShape): number {
	const compactionIndex = previousCompactionIndex(entries, preparation);
	if (compactionIndex < 0) return 0;
	const previous = entries[compactionIndex];
	if (previous.type !== "compaction") return 0;
	const previousKeepIndex = entries.findIndex((entry) => entry.id === previous.firstKeptEntryId);
	return previousKeepIndex >= 0 ? previousKeepIndex : compactionIndex + 1;
}

/** Recompute prime-agent's keep point while counting context-bearing custom messages. */
export function recomputeGoalCutPoint(
	entries: readonly SessionEntry[],
	preparation: PreparationShape,
): CorrectedCutPoint {
	const boundaryStart = compactionBoundary(entries, preparation);
	const cutPoints: number[] = [];
	for (let index = boundaryStart; index < entries.length; index++) {
		if (isValidCutPoint(entries[index])) cutPoints.push(index);
	}
	if (cutPoints.length === 0) {
		return {
			boundaryStart,
			cutIndex: boundaryStart,
			firstKeptEntryId: entries[boundaryStart]?.id,
		};
	}

	const keepRecentTokens = preparation.settings?.keepRecentTokens ?? DEFAULT_KEEP_RECENT_TOKENS;
	let accumulatedTokens = 0;
	let cutIndex = cutPoints[0];
	for (let index = entries.length - 1; index >= boundaryStart; index--) {
		const entry = entries[index];
		if (entry.type !== "message" && entry.type !== "custom_message") continue;
		accumulatedTokens += entryTokens(entry);
		if (accumulatedTokens < keepRecentTokens) continue;
		cutIndex = cutPoints.find((candidate) => candidate >= index) ?? cutIndex;
		break;
	}

	while (cutIndex > boundaryStart) {
		const previous = entries[cutIndex - 1];
		if (previous.type === "compaction" || previous.type === "message") break;
		cutIndex--;
	}

	return {
		boundaryStart,
		cutIndex,
		firstKeptEntryId: entries[cutIndex]?.id,
	};
}

/** Detect the frozen keep point that makes repeated compactions free no context. */
export function shouldCancelFrozenCompaction(
	entries: readonly SessionEntry[],
	firstKeptEntryId: string,
): boolean {
	for (let index = entries.length - 1; index >= 0; index--) {
		const entry = entries[index];
		if (entry.type === "compaction") return entry.firstKeptEntryId === firstKeptEntryId;
	}
	return false;
}

function messagesForSummary(entries: readonly SessionEntry[], start: number, end: number): Record<string, unknown>[] {
	const messages: Record<string, unknown>[] = [];
	for (let index = start; index < end; index++) {
		const entry = entries[index];
		if (entry.type === "message") {
			messages.push(entry.message as unknown as Record<string, unknown>);
		} else if (entry.type === "custom_message") {
			messages.push({
				role: "custom",
				customType: entry.customType,
				content: entry.content,
				display: entry.display,
				details: entry.details,
				timestamp: new Date(entry.timestamp).getTime(),
			});
		} else if (entry.type === "branch_summary") {
			messages.push({
				role: "branchSummary",
				summary: entry.summary,
				fromId: entry.fromId,
				timestamp: new Date(entry.timestamp).getTime(),
			});
		}
	}
	return messages;
}

function extractResponseText(payload: unknown): string {
	if (typeof payload !== "object" || payload === null) return "";
	if ("output_text" in payload && typeof payload.output_text === "string") return payload.output_text;

	const choices = "choices" in payload && Array.isArray(payload.choices) ? payload.choices : [];
	const choice = choices.find((value) => typeof value === "object" && value !== null && "message" in value);
	if (
		choice &&
		typeof choice === "object" &&
		choice !== null &&
		"message" in choice &&
		typeof choice.message === "object" &&
		choice.message !== null &&
		"content" in choice.message
	) {
		const content = choice.message.content;
		if (typeof content === "string") return content;
		if (Array.isArray(content)) {
			return content
				.map((part) =>
					typeof part === "object" && part !== null && "text" in part && typeof part.text === "string"
						? part.text
						: "",
				)
				.join("");
		}
	}

	const content = "content" in payload && Array.isArray(payload.content) ? payload.content : [];
	const anthropicText = content
		.map((part) => {
			if (typeof part !== "object" || part === null || !("type" in part) || !("text" in part)) return "";
			return part.type === "text" && typeof part.text === "string" ? part.text : "";
		})
		.join("");
	if (anthropicText) return anthropicText;

	const output = "output" in payload && Array.isArray(payload.output) ? payload.output : [];
	const responseText = output
		.flatMap((item) => {
			if (typeof item !== "object" || item === null || !("content" in item)) return [];
			return Array.isArray(item.content) ? item.content : [];
		})
		.map((part) =>
			typeof part === "object" && part !== null && "text" in part && typeof part.text === "string"
				? part.text
				: "",
		)
		.join("");
	if (responseText) return responseText;

	const candidates = "candidates" in payload && Array.isArray(payload.candidates) ? payload.candidates : [];
	return candidates
		.flatMap((candidate) => {
			if (typeof candidate !== "object" || candidate === null || !("content" in candidate)) return [];
			const candidateContent = candidate.content;
			if (
				typeof candidateContent !== "object" ||
				candidateContent === null ||
				!("parts" in candidateContent)
			) {
				return [];
			}
			return Array.isArray(candidateContent.parts) ? candidateContent.parts : [];
		})
		.map((part) =>
			typeof part === "object" && part !== null && "text" in part && typeof part.text === "string"
				? part.text
				: "",
		)
		.join("");
}

function parseEventStream(body: string): string {
	let streamedText = "";
	let completedText = "";
	for (const chunk of body.split(/\r?\n\r?\n/)) {
		const data = chunk
			.split(/\r?\n/)
			.filter((line) => line.startsWith("data:"))
			.map((line) => line.slice(5).trim())
			.join("\n");
		if (!data || data === "[DONE]") continue;
		try {
			const event: unknown = JSON.parse(data);
			if (typeof event !== "object" || event === null || !("type" in event)) continue;
			if (
				event.type === "response.output_text.delta" &&
				"delta" in event &&
				typeof event.delta === "string"
			) {
				streamedText += event.delta;
			}
			if (
				(event.type === "response.completed" || event.type === "response.done") &&
				"response" in event &&
				typeof event.response === "object" &&
				event.response !== null
			) {
				completedText = extractResponseText(event.response);
			}
		} catch {
			continue;
		}
	}
	return completedText || streamedText;
}

async function completeSummaryWithFetch(
	model: NonNullable<ExtensionContext["model"]>,
	apiKey: string,
	authHeaders: Record<string, string> | undefined,
	prompt: string,
	maxTokens: number,
	signal: AbortSignal,
): Promise<string> {
	const headers = new Headers(model.headers);
	for (const [name, value] of Object.entries(authHeaders ?? {})) headers.set(name, value);
	headers.set("content-type", "application/json");

	const baseUrl = model.baseUrl.replace(/\/+$/, "");
	let url: string;
	let body: Record<string, unknown>;
	if (model.api === "anthropic-messages") {
		url = baseUrl.endsWith("/messages") ? baseUrl : `${baseUrl}/messages`;
		if (!headers.has("x-api-key") && !headers.has("authorization")) headers.set("x-api-key", apiKey);
		if (!headers.has("anthropic-version")) headers.set("anthropic-version", "2023-06-01");
		body = {
			model: model.id,
			max_tokens: maxTokens,
			messages: [{ role: "user", content: prompt }],
		};
	} else if (model.api === "google-generative-ai") {
		url = `${baseUrl}/models/${encodeURIComponent(model.id)}:generateContent`;
		const separator = url.includes("?") ? "&" : "?";
		url += `${separator}key=${encodeURIComponent(apiKey)}`;
		body = {
			contents: [{ role: "user", parts: [{ text: prompt }] }],
			generationConfig: { maxOutputTokens: maxTokens },
		};
	} else if (
		model.api === "openai-responses" ||
		model.api === "azure-openai-responses" ||
		model.api === "openai-codex-responses"
	) {
		const isCodex = model.api === "openai-codex-responses";
		if (isCodex) {
			url = baseUrl.endsWith("/codex/responses")
				? baseUrl
				: baseUrl.endsWith("/codex")
					? `${baseUrl}/responses`
					: `${baseUrl}/codex/responses`;
		} else {
			url = baseUrl.endsWith("/responses") ? baseUrl : `${baseUrl}/responses`;
		}
		if (!headers.has("authorization") && !headers.has("api-key")) {
			headers.set("authorization", `Bearer ${apiKey}`);
		}
		if (isCodex) {
			const tokenPayload = apiKey.split(".")[1];
			if (tokenPayload) {
				try {
					const claims: unknown = JSON.parse(
						Buffer.from(tokenPayload, "base64url").toString("utf8"),
					);
					if (
						typeof claims === "object" &&
						claims !== null &&
						"https://api.openai.com/auth" in claims
					) {
						const authClaim = claims["https://api.openai.com/auth"];
						if (
							typeof authClaim === "object" &&
							authClaim !== null &&
							"chatgpt_account_id" in authClaim &&
							typeof authClaim.chatgpt_account_id === "string"
						) {
							headers.set("chatgpt-account-id", authClaim.chatgpt_account_id);
						}
					}
				} catch {
					// The API will return an actionable auth error if the token is malformed.
				}
			}
			headers.set("openai-beta", "responses=experimental");
			headers.set("accept", "text/event-stream");
			headers.set("originator", "pi");
		}
		body = {
			model: model.id,
			store: false,
			stream: isCodex,
			instructions: "Summarize the supplied conversation. Return only the summary.",
			input: [{ role: "user", content: [{ type: "input_text", text: prompt }] }],
			...(isCodex ? { text: { verbosity: "low" } } : { max_output_tokens: maxTokens }),
		};
	} else if (model.api === "openai-completions") {
		url = baseUrl.endsWith("/chat/completions") ? baseUrl : `${baseUrl}/chat/completions`;
		if (!headers.has("authorization") && !headers.has("api-key")) {
			headers.set("authorization", `Bearer ${apiKey}`);
		}
		body = {
			model: model.id,
			messages: [{ role: "user", content: prompt }],
			max_tokens: maxTokens,
		};
	} else {
		throw new Error(`unsupported summary API: ${model.api}`);
	}

	const response = await fetch(url, {
		method: "POST",
		headers,
		body: JSON.stringify(body),
		signal,
	});
	const responseBody = await response.text();
	if (!response.ok) {
		throw new Error(`summary request failed (${response.status}): ${responseBody.slice(0, 500)}`);
	}
	const contentType = response.headers.get("content-type") ?? "";
	if (contentType.includes("text/event-stream")) return parseEventStream(responseBody);
	return extractResponseText(JSON.parse(responseBody));
}

async function summarizeCorrectedRange(
	entries: readonly SessionEntry[],
	start: number,
	end: number,
	preparation: PreparationShape,
	customInstructions: string | undefined,
	signal: AbortSignal,
	ctx: ExtensionContext,
): Promise<string | undefined> {
	const currentModel = ctx.model;
	const model =
		currentModel && FETCH_SUMMARY_APIS.has(currentModel.api)
			? currentModel
			: ctx.modelRegistry.getAvailable().find((candidate) => FETCH_SUMMARY_APIS.has(candidate.api));
	if (!model) return undefined;
	const auth = await ctx.modelRegistry.getApiKeyAndHeaders(model);
	if (!auth.ok || !auth.apiKey) return undefined;

	const messages = messagesForSummary(entries, start, end);
	if (messages.length === 0 && !preparation.previousSummary) return undefined;

	const conversation = JSON.stringify(messages);
	const previousContext = preparation.previousSummary
		? `\n\nPrevious session summary for context:\n${preparation.previousSummary}`
		: "";
	const customFocus = customInstructions ? `\n\nCompaction focus:\n${customInstructions}` : "";
	const prompt = `You are a conversation summarizer. Create a structured, concise summary that preserves goals, constraints, decisions, code changes, current work, blockers, and next steps.${previousContext}${customFocus}\n\n<conversation>\n${conversation}\n</conversation>`;
	const summary = await completeSummaryWithFetch(
		model,
		auth.apiKey,
		auth.headers,
		prompt,
		Math.floor(0.8 * (preparation.settings?.reserveTokens ?? DEFAULT_RESERVE_TOKENS)),
		signal,
	);
	return summary.trim() ? summary : undefined;
}

let compactionInFlight = false;

export default function (pi: ExtensionAPI) {
	pi.on("context", (event) => {
		try {
			return { messages: pruneGoalContextMessages(event.messages) };
		} catch {
			return undefined;
		}
	});

	pi.on("session_before_compact", async (event, ctx) => {
		const preparation = event.preparation as PreparationShape;
		const corrected = recomputeGoalCutPoint(event.branchEntries, preparation);
		if (!corrected.firstKeptEntryId) return undefined;

		if (corrected.firstKeptEntryId === preparation.firstKeptEntryId) {
			return shouldCancelFrozenCompaction(event.branchEntries, preparation.firstKeptEntryId)
				? { cancel: true }
				: undefined;
		}

		try {
			const summary = await summarizeCorrectedRange(
				event.branchEntries,
				corrected.boundaryStart,
				corrected.cutIndex,
				preparation,
				event.customInstructions,
				event.signal,
				ctx,
			);
			if (!summary) return undefined;
			return {
				compaction: {
					summary,
					firstKeptEntryId: corrected.firstKeptEntryId,
					tokensBefore: preparation.tokensBefore,
				},
			};
		} catch (error) {
			if (ctx.hasUI && !event.signal.aborted) {
				ctx.ui.notify(`aoc-goal-compact: corrected compaction failed: ${String(error)}`, "warning");
			}
			return undefined;
		}
	});

	pi.on("turn_end", (_event, ctx) => {
		try {
			const percent = ctx.getContextUsage()?.percent;
			if (compactionInFlight || percent === null || percent === undefined || percent <= COMPACT_THRESHOLD_PERCENT) {
				return;
			}
			compactionInFlight = true;
			ctx.compact({
				onError: () => {
					compactionInFlight = false;
				},
			});
		} catch {
			compactionInFlight = false;
		}
	});

	pi.on("session_compact", () => {
		compactionInFlight = false;
	});
	pi.on("agent_end", () => {
		compactionInFlight = false;
	});
}
