import { clampInt, clampMaxChars, findProjectRoot, renderCommand, resolveRepoCommand, runBoundedCommand, scopedCwd, stripAt } from "./aoc-runtime";
import { StringEnum } from "@mariozechner/pi-ai";
import { Type } from "@sinclair/typebox";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const MAX_DEFAULT_CHARS = 12_000;
const MAX_ALLOWED_CHARS = 40_000;
const COMMAND_TIMEOUT_MS = 30_000;

const CodeGraphActionSchema = StringEnum(
	["status", "files", "search", "context", "callers", "callees", "impact", "affected"] as const,
	{ description: "Read-only CodeGraph action to run." },
);

const CodeGraphParams = Type.Object({
	action: CodeGraphActionSchema,
	query: Type.Optional(Type.String({ description: "Search/context text for search and context actions." })),
	symbol: Type.Optional(Type.String({ description: "Symbol name or id for callers/callees/impact actions." })),
	files: Type.Optional(Type.Array(Type.String(), { description: "Changed/source files for affected-test analysis." })),
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
	limit: Type.Optional(Type.Integer({ minimum: 1, maximum: 100, description: "Result limit for search/call graph actions." })),
	depth: Type.Optional(Type.Integer({ minimum: 1, maximum: 10, description: "Traversal depth for impact/affected actions." })),
	maxDepth: Type.Optional(Type.Integer({ minimum: 1, maximum: 12, description: "Maximum file tree depth for files action." })),
	filter: Type.Optional(Type.String({ description: "Optional file/test glob filter for files or affected actions." })),
	pattern: Type.Optional(Type.String({ description: "Optional glob pattern for files action." })),
	json: Type.Optional(Type.Boolean({ description: "Ask CodeGraph for JSON output where the CLI supports it." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});

type CodeGraphParamsType = {
	action: "status" | "files" | "search" | "context" | "callers" | "callees" | "impact" | "affected";
	query?: string;
	symbol?: string;
	files?: string[];
	cwd?: string;
	limit?: number;
	depth?: number;
	maxDepth?: number;
	filter?: string;
	pattern?: string;
	json?: boolean;
	maxChars?: number;
};


function requireText(value: string | undefined, label: string): string {
	const trimmed = (value ?? "").trim();
	if (!trimmed) throw new Error(`${label} is required`);
	return trimmed;
}

function positiveInt(value: number | undefined, fallback: number, max: number): number {
	return clampInt(value, fallback, 1, max);
}

function buildArgs(params: CodeGraphParamsType, projectRoot: string): string[] {
	const args: string[] = [];
	const json = params.json === true;

	switch (params.action) {
		case "status":
			args.push("status", projectRoot);
			if (json) args.push("--json");
			break;
		case "files":
			args.push("files", "--path", projectRoot);
			if (params.filter) args.push("--filter", params.filter);
			if (params.pattern) args.push("--pattern", params.pattern);
			if (params.maxDepth) args.push("--max-depth", String(positiveInt(params.maxDepth, 4, 12)));
			if (json) args.push("--json");
			break;
		case "search":
			args.push("query", requireText(params.query, "query"), "--path", projectRoot, "--limit", String(positiveInt(params.limit, 10, 100)));
			if (json) args.push("--json");
			break;
		case "context":
			args.push("context", requireText(params.query, "query"), "--path", projectRoot, "--max-nodes", String(positiveInt(params.limit, 20, 100)), "--format", json ? "json" : "markdown");
			break;
		case "callers":
			args.push("callers", requireText(params.symbol, "symbol"), "--path", projectRoot, "--limit", String(positiveInt(params.limit, 20, 100)));
			if (json) args.push("--json");
			break;
		case "callees":
			args.push("callees", requireText(params.symbol, "symbol"), "--path", projectRoot, "--limit", String(positiveInt(params.limit, 20, 100)));
			if (json) args.push("--json");
			break;
		case "impact":
			args.push("impact", requireText(params.symbol, "symbol"), "--path", projectRoot, "--depth", String(positiveInt(params.depth, 2, 10)));
			if (json) args.push("--json");
			break;
		case "affected": {
			const files = (params.files ?? []).map(stripAt).filter((file) => file.trim().length > 0);
			if (files.length === 0) throw new Error("files is required for affected");
			args.push("affected", ...files, "--path", projectRoot);
			if (params.depth) args.push("--depth", String(positiveInt(params.depth, 5, 10)));
			if (params.filter) args.push("--filter", params.filter);
			if (json) args.push("--json");
			break;
		}
	}

	return args;
}

async function runCodeGraph(command: string, args: string[], cwd: string, maxChars: number, signal?: AbortSignal) {
	return await runBoundedCommand(command, args, {
		cwd,
		maxStdoutChars: maxChars,
		maxStderrChars: Math.min(maxChars, 8000),
		timeoutMs: COMMAND_TIMEOUT_MS,
		missingMessage: "codegraph CLI is not installed or not on PATH. Install CodeGraph, then run codegraph init -i for this project.",
		signal,
	});
}

export default function aocCodeGraphExtension(pi: ExtensionAPI): void {
	pi.registerTool({
		name: "aoc_codegraph",
		label: "AOC CodeGraph",
		description: "Query a local CodeGraph index for read-only agent code intelligence. Requires an existing CodeGraph install and project index.",
		promptSnippet: "Query a local CodeGraph code index for symbols, context, impact, files, and affected tests.",
		promptGuidelines: [
			"Use aoc_codegraph before broad grep/read scans when .codegraph exists and the task is code discovery, architecture tracing, impact analysis, or affected-test selection.",
			"Use aoc_codegraph as read-only discovery; do not use it to install, initialize, index, or sync projects.",
			"If aoc_codegraph reports CodeGraph is missing, stale, or uninitialized, fall back to targeted repo inspection and report the limitation.",
		],
		parameters: CodeGraphParams,
		async execute(_toolCallId, params: CodeGraphParamsType, signal, _onUpdate, ctx) {
			const projectRoot = findProjectRoot(ctx.cwd);
			const cwd = scopedCwd(projectRoot, params.cwd);
			const args = buildArgs(params, projectRoot);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const commandBin = resolveRepoCommand(projectRoot, "bin/codegraph", "codegraph");
			const result = await runCodeGraph(commandBin, args, cwd, maxChars, signal);
			const command = renderCommand(commandBin, args);
			const lines = [
				`$ ${command}`,
				`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
			];
			if (result.stdout.trim()) lines.push("", result.stdout.trimEnd());
			if (result.stderr.trim()) lines.push("", "stderr:", result.stderr.trimEnd());
			if (!result.ok) lines.push("", "CodeGraph did not complete successfully. Treat this as unavailable evidence and fall back to targeted repo inspection.");
			return {
				content: [{ type: "text", text: lines.join("\n") }],
				details: { action: params.action, command, cwd, ok: result.ok, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated },
			};
		},
	});
}
