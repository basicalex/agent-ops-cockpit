import { spawn } from "node:child_process";
import * as path from "node:path";
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

function stripAt(value: string): string {
	return value.startsWith("@") ? value.slice(1) : value;
}

function scopedCwd(root: string, requested?: string): string {
	if (!requested || requested.trim().length === 0) return root;
	const resolved = path.resolve(root, stripAt(requested));
	const rel = path.relative(root, resolved);
	if (rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel))) return resolved;
	throw new Error(`cwd escapes project root: ${requested}`);
}

function clampMaxChars(value?: number): number {
	if (!Number.isFinite(value ?? NaN)) return MAX_DEFAULT_CHARS;
	return Math.max(1000, Math.min(MAX_ALLOWED_CHARS, Math.floor(value as number)));
}

function truncateOutput(text: string, maxChars: number): { text: string; truncated: boolean } {
	if (text.length <= maxChars) return { text, truncated: false };
	return {
		text: `${text.slice(0, maxChars)}\n\n[truncated ${text.length - maxChars} chars; rerun with a narrower query or higher maxChars]`,
		truncated: true,
	};
}

function requireText(value: string | undefined, label: string): string {
	const trimmed = (value ?? "").trim();
	if (!trimmed) throw new Error(`${label} is required`);
	return trimmed;
}

function positiveInt(value: number | undefined, fallback: number, max: number): number {
	if (!Number.isFinite(value ?? NaN)) return fallback;
	return Math.max(1, Math.min(max, Math.floor(value as number)));
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

async function runCodeGraph(args: string[], cwd: string, maxChars: number, signal?: AbortSignal) {
	return await new Promise<{ ok: boolean; exitCode: number | null; stdout: string; stderr: string; timedOut: boolean; truncated: boolean }>((resolve, reject) => {
		let stdout = "";
		let stderr = "";
		let settled = false;
		let timedOut = false;
		const child = spawn("codegraph", args, { cwd, stdio: ["ignore", "pipe", "pipe"], shell: false });
		const timer = setTimeout(() => {
			timedOut = true;
			child.kill("SIGTERM");
		}, COMMAND_TIMEOUT_MS);

		const abort = () => child.kill("SIGTERM");
		signal?.addEventListener("abort", abort, { once: true });

		child.stdout.on("data", (chunk) => {
			stdout += String(chunk);
		});
		child.stderr.on("data", (chunk) => {
			stderr += String(chunk);
		});
		child.on("error", (error: NodeJS.ErrnoException) => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			signal?.removeEventListener("abort", abort);
			if (error.code === "ENOENT") {
				reject(new Error("codegraph CLI is not installed or not on PATH. Install CodeGraph, then run codegraph init -i for this project."));
				return;
			}
			reject(error);
		});
		child.on("close", (exitCode) => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			signal?.removeEventListener("abort", abort);
			const out = truncateOutput(stdout, maxChars);
			const err = truncateOutput(stderr, Math.min(maxChars, 8000));
			resolve({ ok: exitCode === 0 && !timedOut, exitCode, stdout: out.text, stderr: err.text, timedOut, truncated: out.truncated || err.truncated });
		});
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
			const projectRoot = ctx.cwd ?? process.cwd();
			const cwd = scopedCwd(projectRoot, params.cwd);
			const args = buildArgs(params, projectRoot);
			const maxChars = clampMaxChars(params.maxChars);
			const result = await runCodeGraph(args, cwd, maxChars, signal);
			const command = `codegraph ${args.map((arg) => (arg.includes(" ") ? JSON.stringify(arg) : arg)).join(" ")}`;
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
