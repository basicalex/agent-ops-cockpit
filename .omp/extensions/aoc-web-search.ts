import { clampInt, clampMaxChars, findProjectRoot, renderCommand, resolveRepoCommand, runBoundedCommand } from "./aoc-runtime";
import { Type } from "@sinclair/typebox";
import { StringEnum } from "@mariozechner/pi-ai";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const MAX_DEFAULT_CHARS = 16_000;
const MAX_ALLOWED_CHARS = 40_000;
const COMMAND_TIMEOUT_MS = 45_000;

const SearchModeSchema = StringEnum(["general", "docs", "error", "package", "github"] as const, {
	description: "Search mode. Use docs for official documentation, error for issue/error lookups, package for package registries, github for GitHub repositories.",
});

const WebSearchParams = Type.Object({
	query: Type.String({ minLength: 1, description: "Search query." }),
	mode: Type.Optional(SearchModeSchema),
	limit: Type.Optional(Type.Integer({ minimum: 1, maximum: 20, description: "Maximum normalized results to return." })),
	direct: Type.Optional(Type.Boolean({ description: "For package mode, query package registries directly instead of local SearXNG." })),
	noAutoStart: Type.Optional(Type.Boolean({ description: "Do not auto-start the managed local search service." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});

type SearchMode = "general" | "docs" | "error" | "package" | "github";

type WebSearchParamsType = {
	query: string;
	mode?: SearchMode;
	limit?: number;
	direct?: boolean;
	noAutoStart?: boolean;
	maxChars?: number;
};


function positiveInt(value: number | undefined, fallback: number, max: number): number {
	return clampInt(value, fallback, 1, max);
}


function buildArgs(params: WebSearchParamsType): string[] {
	const mode = params.mode ?? "general";
	const args = ["query", "--json", "--mode", mode, "--limit", String(positiveInt(params.limit, 5, 20))];
	if (params.direct === true) args.push("--direct");
	if (params.noAutoStart === true) args.push("--no-auto-start");
	args.push(params.query.trim());
	return args;
}

async function runCommand(command: string, args: string[], cwd: string, maxChars: number, signal?: AbortSignal): Promise<import("./aoc-runtime").CommandResult> {
	return await runBoundedCommand(command, args, {
		cwd,
		maxStdoutChars: maxChars,
		maxStderrChars: Math.min(maxChars, 8000),
		timeoutMs: COMMAND_TIMEOUT_MS,
		missingMessage: "aoc-search is not installed or not on PATH. Run aoc-init or install AOC before using aoc_web_search.",
		signal,
	});
}

export default function aocWebSearchExtension(pi: ExtensionAPI): void {
	pi.registerTool({
		name: "aoc_web_search",
		label: "AOC Web Search",
		description: "Search the web through AOC's local aoc-search/SearXNG stack and direct package/GitHub lookup modes. Use this when built-in web search providers are unavailable, out of credits, unauthorized, or timing out.",
		promptSnippet: "Use aoc_web_search for web research via the project-local AOC search stack instead of paid built-in web-search providers.",
		promptGuidelines: [
			"Use aoc_web_search when external web research is needed and built-in web search fails with provider, credit, authorization, or timeout errors.",
			"Prefer mode=docs for official documentation, mode=error for errors/issues, mode=package with direct=true for npm/PyPI/crates lookups, and mode=github for repository discovery.",
			"If aoc_web_search reports local search is unconfigured or unhealthy, explain the operational fix from the error instead of retrying paid providers repeatedly.",
		],
		parameters: WebSearchParams,
		async execute(_toolCallId, params: WebSearchParamsType, signal, _onUpdate, ctx) {
			const projectRoot = findProjectRoot(ctx.cwd);
			const command = resolveRepoCommand(projectRoot, "bin/aoc-search", "aoc-search");
			const args = buildArgs(params);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const result = await runCommand(command, args, projectRoot, maxChars, signal);
			const renderedCommand = renderCommand(command, args);
			const lines = [
				`$ ${renderedCommand}`,
				`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
			];
			if (result.stdout.trim()) lines.push("", result.stdout.trimEnd());
			if (result.stderr.trim()) lines.push("", "stderr:", result.stderr.trimEnd());
			if (!result.ok) lines.push("", "AOC web search did not complete successfully. Treat this as unavailable evidence and report the operational error.");
			return {
				content: [{ type: "text", text: lines.join("\n") }],
				details: { command: renderedCommand, mode: params.mode ?? "general", ok: result.ok, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated },
			};
		},
	});
}
