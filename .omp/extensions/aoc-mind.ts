import { clampInt, clampMaxChars, findProjectRoot, renderCommand, resolveRepoCommand, runBoundedCommand, scopedCwd } from "./aoc-runtime";
import { StringEnum } from "@mariozechner/pi-ai";
import { Type } from "@sinclair/typebox";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const MAX_DEFAULT_CHARS = 16_000;
const MAX_ALLOWED_CHARS = 48_000;
const COMMAND_TIMEOUT_MS = 30_000;

const MindActionSchema = StringEnum(
	["status", "evidence", "provenance", "mnemopi_candidates"] as const,
	{ description: "Read-only AOC Mind action to run." },
);

const MindParams = Type.Object({
	action: MindActionSchema,
	reason: Type.Optional(Type.String({ description: "Required explicit reason for evidence and mnemopi_candidates." })),
	mode: Type.Optional(StringEnum(["focused", "resume", "decision", "debug"] as const, { description: "Evidence retrieval mode." })),
	activeTag: Type.Optional(Type.String({ description: "Optional AOC active tag filter." })),
	conversationId: Type.Optional(Type.String({ description: "Optional provenance conversation id." })),
	sessionId: Type.Optional(Type.String({ description: "Optional provenance session id." })),
	artifactId: Type.Optional(Type.String({ description: "Optional provenance artifact id." })),
	checkpointId: Type.Optional(Type.String({ description: "Optional provenance checkpoint id." })),
	canonEntryId: Type.Optional(Type.String({ description: "Optional provenance canon entry id." })),
	taskId: Type.Optional(Type.String({ description: "Optional provenance task id." })),
	filePath: Type.Optional(Type.String({ description: "Optional provenance file path." })),
	cwd: Type.Optional(Type.String({ description: "Optional working directory scoped under the current project root." })),
	maxItems: Type.Optional(Type.Integer({ minimum: 1, maximum: 32, description: "Maximum evidence/candidate items." })),
	maxNodes: Type.Optional(Type.Integer({ minimum: 1, maximum: 256, description: "Maximum provenance nodes." })),
	maxEdges: Type.Optional(Type.Integer({ minimum: 1, maximum: 512, description: "Maximum provenance edges." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});

type MindAction = "status" | "evidence" | "provenance" | "mnemopi_candidates";
type MindMode = "focused" | "resume" | "decision" | "debug";

interface MindParamsType {
	action: MindAction;
	reason?: string;
	mode?: MindMode;
	activeTag?: string;
	conversationId?: string;
	sessionId?: string;
	artifactId?: string;
	checkpointId?: string;
	canonEntryId?: string;
	taskId?: string;
	filePath?: string;
	cwd?: string;
	maxItems?: number;
	maxNodes?: number;
	maxEdges?: number;
	maxChars?: number;
}


function positiveInt(value: number | undefined, fallback: number, max: number): number {
	return clampInt(value, fallback, 1, max);
}


function requiredReason(params: MindParamsType): string {
	const reason = params.reason?.trim();
	if (!reason) throw new Error(`${params.action} requires a non-empty reason`);
	return reason;
}

function addOptional(args: string[], flag: string, value?: string): void {
	const trimmed = value?.trim();
	if (trimmed) args.push(flag, trimmed);
}

function buildArgs(params: MindParamsType, projectRoot: string): string[] {
	switch (params.action) {
		case "status":
			return ["status", "--project-root", projectRoot, "--json"];
		case "evidence": {
			const args = ["evidence-pack", "--project-root", projectRoot, "--reason", requiredReason(params), "--json", "--max-items", String(positiveInt(params.maxItems, 12, 32))];
			addOptional(args, "--mode", params.mode);
			addOptional(args, "--active-tag", params.activeTag);
			return args;
		}
		case "mnemopi_candidates": {
			const args = ["mnemopi-candidates", "--project-root", projectRoot, "--reason", requiredReason(params), "--json", "--max-items", String(positiveInt(params.maxItems, 12, 32))];
			addOptional(args, "--mode", params.mode);
			addOptional(args, "--active-tag", params.activeTag);
			return args;
		}
		case "provenance": {
			const args = ["provenance-query", "--project-root", projectRoot, "--json", "--max-nodes", String(positiveInt(params.maxNodes, 64, 256)), "--max-edges", String(positiveInt(params.maxEdges, 128, 512))];
			addOptional(args, "--session-id", params.sessionId);
			addOptional(args, "--conversation-id", params.conversationId);
			addOptional(args, "--artifact-id", params.artifactId);
			addOptional(args, "--checkpoint-id", params.checkpointId);
			addOptional(args, "--canon-entry-id", params.canonEntryId);
			addOptional(args, "--task-id", params.taskId);
			addOptional(args, "--file-path", params.filePath);
			addOptional(args, "--active-tag", params.activeTag);
			return args;
		}
	}
}

async function runMind(command: string, args: string[], cwd: string, maxChars: number, signal?: AbortSignal): Promise<import("./aoc-runtime").CommandResult> {
	return await runBoundedCommand(command, args, {
		cwd,
		maxStdoutChars: maxChars,
		maxStderrChars: Math.min(maxChars, 8000),
		timeoutMs: COMMAND_TIMEOUT_MS,
		missingMessage: "aoc-mind-service is not installed or not on PATH. Run aoc-init after installing AOC.",
		signal,
	});
}

export default function aocMindExtension(pi: ExtensionAPI): void {
	pi.registerTool({
		name: "aoc_mind",
		label: "AOC Mind",
		description: "Query AOC Mind historical/provenance intelligence through read-only status, evidence, provenance, and Mnemopi-candidate commands.",
		promptSnippet: "Use AOC Mind for cited prior decisions, provenance, debugging history, and dry-run Mnemopi candidate memories.",
		promptGuidelines: [
			"Use aoc_mind when the user asks what happened before, why a decision was made, or what evidence supports project memory.",
			"Use action=evidence with an explicit reason before proposing derived long-term memories.",
			"Use action=mnemopi_candidates only as dry-run candidate synthesis; it does not write to Mnemopi.",
			"Treat AOC Mind output as cited historical evidence, not automatic prompt memory or a replacement for Mnemopi recall.",
		],
		parameters: MindParams,
		async execute(_toolCallId, params: MindParamsType, signal, _onUpdate, ctx) {
			const projectRoot = findProjectRoot(ctx.cwd);
			const cwd = scopedCwd(projectRoot, params.cwd);
			const args = buildArgs(params, projectRoot);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const commandBin = resolveRepoCommand(projectRoot, "bin/aoc-mind-service", "aoc-mind-service");
			const result = await runMind(commandBin, args, cwd, maxChars, signal);
			const command = renderCommand(commandBin, args);
			const lines = [
				`$ ${command}`,
				`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
			];
			if (result.stdout.trim()) lines.push("", result.stdout.trimEnd());
			if (result.stderr.trim()) lines.push("", "stderr:", result.stderr.trimEnd());
			if (!result.ok) lines.push("", "AOC Mind did not complete successfully. Treat this as unavailable evidence and fall back to targeted repo inspection.");
			return {
				content: [{ type: "text", text: lines.join("\n") }],
				details: { action: params.action, command, cwd, ok: result.ok, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated },
			};
		},
	});
}
