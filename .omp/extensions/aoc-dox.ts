import { clampMaxChars, findProjectRoot, renderCommand, resolveRepoCommand, runBoundedCommand, scopedCwd } from "./aoc-runtime";
import { StringEnum } from "@mariozechner/pi-ai";
import { Type } from "@sinclair/typebox";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const MAX_DEFAULT_CHARS = 12_000;
const MAX_ALLOWED_CHARS = 40_000;
const COMMAND_TIMEOUT_MS = 30_000;

const DoxActionSchema = StringEnum(["review", "review-packet", "doctor", "apply-dry-run"] as const, {
	description: "Read-only AOC DOX action to run. This tool never writes metadata or applies changes.",
});

const DoxWriterActionSchema = StringEnum(["map", "eval", "review-packet"] as const, {
	description: "Metadata-writing AOC DOX action for writer workflows only.",
});

const DoxParams = Type.Object({
	action: DoxActionSchema,
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
	json: Type.Optional(Type.Boolean({ description: "Request JSON output where supported." })),
	noCodegraph: Type.Optional(Type.Boolean({ description: "Disable CodeGraph usage where supported." })),
	minScore: Type.Optional(Type.Integer({ minimum: 1, maximum: 20, description: "Minimum candidate score where supported." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: 40000, description: "Maximum characters returned to the model." })),
});

const DoxWriterParams = Type.Object({
	action: DoxWriterActionSchema,
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
	json: Type.Optional(Type.Boolean({ description: "Request JSON output where supported." })),
	noCodegraph: Type.Optional(Type.Boolean({ description: "Disable CodeGraph usage for map." })),
	minScore: Type.Optional(Type.Integer({ minimum: 1, maximum: 20, description: "Minimum candidate score for map." })),
	writePacket: Type.Optional(Type.Boolean({ description: "For action=review-packet, write .aoc/dox/review.md." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: 40000, description: "Maximum characters returned to the model." })),
});

type DoxParamsType = {
	action: "review" | "review-packet" | "doctor" | "apply-dry-run";
	cwd?: string;
	json?: boolean;
	noCodegraph?: boolean;
	minScore?: number;
	maxChars?: number;
};

type DoxWriterParamsType = {
	action: "map" | "eval" | "review-packet";
	cwd?: string;
	json?: boolean;
	noCodegraph?: boolean;
	minScore?: number;
	writePacket?: boolean;
	maxChars?: number;
};



function buildReadOnlyArgs(params: DoxParamsType): string[] {
	const args = ["dox"];
	const json = params.json === true;

	switch (params.action) {
		case "review":
			args.push("review");
			if (json) args.push("--json");
			break;
		case "review-packet":
			args.push("review", "--packet");
			if (json) args.push("--json");
			break;
		case "doctor":
			args.push("doctor");
			if (json) args.push("--json");
			break;
		case "apply-dry-run":
			args.push("apply", "--dry-run");
			if (json) args.push("--json");
			break;
	}

	return args;
}

function buildWriterArgs(params: DoxWriterParamsType): string[] {
	const args = ["dox"];
	const json = params.json === true;

	switch (params.action) {
		case "map":
			args.push("map");
			if (json) args.push("--json");
			if (params.noCodegraph) args.push("--no-codegraph");
			if (Number.isFinite(params.minScore ?? NaN)) args.push("--min-score", String(Math.floor(params.minScore as number)));
			break;
		case "eval":
			args.push("eval");
			if (json) args.push("--json");
			break;
		case "review-packet":
			args.push("review", "--packet");
			if (params.writePacket === true) args.push("--write-packet");
			if (json) args.push("--json");
			break;
	}

	return args;
}

async function runAoc(command: string, args: string[], cwd: string, maxChars: number, signal?: AbortSignal) {
	return await runBoundedCommand(command, args, {
		cwd,
		maxStdoutChars: maxChars,
		maxStderrChars: Math.min(maxChars, 8000),
		timeoutMs: COMMAND_TIMEOUT_MS,
		missingMessage: "aoc CLI is not installed or not on PATH; install AOC or run from the agent-ops-cockpit development shell.",
		signal,
	});
}


export default function aocDoxExtension(pi: ExtensionAPI): void {
	pi.registerTool({
		name: "aoc_dox",
		label: "AOC DOX",
		description: "Run read-only AOC DOX cartography actions for AGENTS.md resolution review packets, doctor checks, and dry-run apply output.",
		promptSnippet: "Use AOC DOX to inspect AGENTS resolution coverage, candidate review packets, doctor checks, and apply dry-run output without metadata writes.",
		promptGuidelines: [
			"Use aoc_dox for read-only AOC DOX review packets, doctor checks, and apply dry-run output.",
			"Do not use aoc_dox for metadata-writing map or eval workflows; writer workflows must use aoc_dox_writer.",
			"This tool is read-only plus apply dry-run by construction: it cannot write .aoc/dox/review.md and cannot run apply --yes.",
			"Do not use aoc_dox to replace ordinary project documentation; AGENTS.md output is sparse operational context only.",
		],
		parameters: DoxParams,
		async execute(_toolCallId, params: DoxParamsType, signal, _onUpdate, ctx) {
			const projectRoot = findProjectRoot(ctx.cwd);
			const cwd = scopedCwd(projectRoot, params.cwd);
			const args = buildReadOnlyArgs(params);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const commandBin = resolveRepoCommand(projectRoot, "bin/aoc", "aoc");
			const result = await runAoc(commandBin, args, cwd, maxChars, signal);
			const command = renderCommand(commandBin, args);
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
	pi.registerTool({
		name: "aoc_dox_writer",
		label: "AOC DOX Writer",
		description: "Run metadata-writing AOC DOX actions for writer workflows only. This surface may refresh .aoc/dox metadata but still cannot apply changes.",
		promptSnippet: "Use AOC DOX writer actions only from dox-writer or intentionally metadata-refreshing main-agent workflows.",
		promptGuidelines: [
			"Use aoc_dox_writer only for DOX writer workflows that are allowed to refresh .aoc/dox metadata.",
			"Use action=map or action=eval only when metadata refresh is intentional and expected by the workflow.",
			"For action=review-packet, writePacket=true writes .aoc/dox/review.md before dry-run review; do not use it from read-only scout, mapper, or critic agents.",
			"This tool does not expose apply --yes; use aoc_dox action=apply-dry-run for dry-run application review.",
		],
		parameters: DoxWriterParams,
		async execute(_toolCallId, params: DoxWriterParamsType, signal, _onUpdate, ctx) {
			const projectRoot = findProjectRoot(ctx.cwd);
			const cwd = scopedCwd(projectRoot, params.cwd);
			const args = buildWriterArgs(params);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const commandBin = resolveRepoCommand(projectRoot, "bin/aoc", "aoc");
			const result = await runAoc(commandBin, args, cwd, maxChars, signal);
			const command = renderCommand(commandBin, args);
			const lines = [
				`$ ${command}`,
				`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
			];
			if (result.stdout.trim()) lines.push("", result.stdout.trimEnd());
			if (result.stderr.trim()) lines.push("", "stderr:", result.stderr.trimEnd());
			if (!result.ok) lines.push("", "AOC DOX writer action did not complete successfully. Treat this as unavailable evidence and fall back to targeted repo inspection.");
			return {
				content: [{ type: "text", text: lines.join("\n") }],
				details: { action: params.action, command, cwd, ok: result.ok, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated },
			};
		},
	});
}
