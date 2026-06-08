import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
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

type CommandResult = {
	ok: boolean;
	exitCode: number | null;
	stdout: string;
	stderr: string;
	timedOut: boolean;
	truncated: boolean;
};

function positiveInt(value: number | undefined, fallback: number, max: number): number {
	if (!Number.isFinite(value ?? NaN)) return fallback;
	return Math.max(1, Math.min(max, Math.floor(value as number)));
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

function resolveSearchCommand(projectRoot: string): string {
	const local = path.join(projectRoot, "bin", "aoc-search");
	return fs.existsSync(local) ? local : "aoc-search";
}

function buildArgs(params: WebSearchParamsType): string[] {
	const mode = params.mode ?? "general";
	const args = ["query", "--json", "--mode", mode, "--limit", String(positiveInt(params.limit, 5, 20))];
	if (params.direct === true) args.push("--direct");
	if (params.noAutoStart === true) args.push("--no-auto-start");
	args.push(params.query.trim());
	return args;
}

async function runCommand(command: string, args: string[], cwd: string, maxChars: number, signal?: AbortSignal): Promise<CommandResult> {
	return await new Promise<CommandResult>((resolve, reject) => {
		let stdout = "";
		let stderr = "";
		let settled = false;
		let timedOut = false;
		const child = spawn(command, args, { cwd, stdio: ["ignore", "pipe", "pipe"], shell: false });
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
				reject(new Error("aoc-search is not installed or not on PATH. Run aoc-init or install AOC before using aoc_web_search."));
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
			const projectRoot = ctx.cwd ?? process.cwd();
			const command = resolveSearchCommand(projectRoot);
			const args = buildArgs(params);
			const maxChars = clampMaxChars(params.maxChars);
			const result = await runCommand(command, args, projectRoot, maxChars, signal);
			const renderedCommand = `${path.basename(command)} ${args.map((arg) => (arg.includes(" ") ? JSON.stringify(arg) : arg)).join(" ")}`;
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
