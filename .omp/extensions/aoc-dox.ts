import { spawn } from "node:child_process";
import * as path from "node:path";
import { StringEnum } from "@mariozechner/pi-ai";
import { Type } from "@sinclair/typebox";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const MAX_DEFAULT_CHARS = 12_000;
const MAX_ALLOWED_CHARS = 40_000;
const COMMAND_TIMEOUT_MS = 30_000;

const DoxActionSchema = StringEnum(["map", "review", "doctor", "eval", "apply-dry-run"] as const, {
	description: "Safe AOC DOX action to run. This tool never applies writes.",
});

const DoxParams = Type.Object({
	action: DoxActionSchema,
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
	json: Type.Optional(Type.Boolean({ description: "Request JSON output where supported." })),
	noCodegraph: Type.Optional(Type.Boolean({ description: "Disable CodeGraph usage for map." })),
	minScore: Type.Optional(Type.Integer({ minimum: 1, maximum: 20, description: "Minimum candidate score for map." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: 40000, description: "Maximum characters returned to the model." })),
});

type DoxParamsType = {
	action: "map" | "review" | "doctor" | "eval" | "apply-dry-run";
	cwd?: string;
	json?: boolean;
	noCodegraph?: boolean;
	minScore?: number;
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

function buildArgs(params: DoxParamsType): string[] {
	const args = ["dox"];
	const json = params.json === true;

	switch (params.action) {
		case "map":
			args.push("map");
			if (json) args.push("--json");
			if (params.noCodegraph) args.push("--no-codegraph");
			if (Number.isFinite(params.minScore ?? NaN)) args.push("--min-score", String(Math.floor(params.minScore as number)));
			break;
		case "review":
			args.push("review");
			if (json) args.push("--json");
			break;
		case "doctor":
			args.push("doctor");
			if (json) args.push("--json");
			break;
		case "eval":
			args.push("eval");
			if (json) args.push("--json");
			break;
		case "apply-dry-run":
			args.push("apply", "--dry-run");
			if (json) args.push("--json");
			break;
	}

	return args;
}

async function runAoc(args: string[], cwd: string, maxChars: number, signal?: AbortSignal) {
	return await new Promise<{ ok: boolean; exitCode: number | null; stdout: string; stderr: string; timedOut: boolean; truncated: boolean }>((resolve, reject) => {
		let stdout = "";
		let stderr = "";
		let settled = false;
		let timedOut = false;
		const child = spawn("aoc", args, { cwd, stdio: ["ignore", "pipe", "pipe"], shell: false });
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
				reject(new Error("aoc CLI is not installed or not on PATH; install AOC or run from the agent-ops-cockpit development shell."));
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


export default function aocDoxExtension(pi: ExtensionAPI): void {
	pi.registerTool({
		name: "aoc_dox",
		label: "AOC DOX",
		description: "Run safe AOC DOX cartography actions for AGENTS.md resolution coverage, candidate review, doctor checks, and dry-run apply output.",
		promptSnippet: "Use AOC DOX to map sparse AGENTS.md context contracts and inspect .aoc/dox metadata without applying writes.",
		promptGuidelines: [
			"Use aoc_dox for AOC DOX metadata, AGENTS resolution coverage, candidate review, and doctor checks.",
			"Use action=map before launching DOX scout/mapper agents so they consume .aoc/dox evidence instead of rediscovering the whole repo.",
			"This tool is safe by construction: it can run apply-dry-run but cannot run apply --yes.",
			"Do not use aoc_dox to replace ordinary project documentation; AGENTS.md output is sparse operational context only.",
		],
		parameters: DoxParams,
		async execute(_toolCallId, params: DoxParamsType, signal, _onUpdate, ctx) {
			const projectRoot = ctx.cwd ?? process.cwd();
			const cwd = scopedCwd(projectRoot, params.cwd);
			const args = buildArgs(params);
			const maxChars = clampMaxChars(params.maxChars);
			const result = await runAoc(args, cwd, maxChars, signal);
			const command = `aoc ${args.map((arg) => (arg.includes(" ") ? JSON.stringify(arg) : arg)).join(" ")}`;
			const lines = [
				`$ ${command}`,
				`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
			];
			if (result.stdout.trim()) lines.push("", result.stdout.trimEnd());
			if (result.stderr.trim()) lines.push("", "stderr:", result.stderr.trimEnd());
			if (!result.ok) lines.push("", "AOC DOX did not complete successfully. Treat this as unavailable evidence and fall back to targeted repo inspection.");
			return {
				content: [{ type: "text", text: lines.join("\n") }],
				details: { action: params.action, command, cwd, ok: result.ok, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated },
			};
		},
	});
}
