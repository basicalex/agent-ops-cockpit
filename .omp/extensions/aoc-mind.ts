import { spawn } from "node:child_process";
import * as path from "node:path";
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

interface MindCommandResult {
	ok: boolean;
	exitCode: number | null;
	stdout: string;
	stderr: string;
	timedOut: boolean;
	truncated: boolean;
}

function stripAt(value: string): string {
	return value.startsWith("@") ? value.slice(1) : value;
}

function scopedCwd(root: string, requested?: string): string {
	if (!requested) return root;
	const resolved = path.resolve(root, stripAt(requested));
	if (resolved !== root && !resolved.startsWith(root + path.sep)) {
		throw new Error(`cwd must stay under project root: ${requested}`);
	}
	return resolved;
}

function clampMaxChars(value?: number): number {
	return Math.min(Math.max(value ?? MAX_DEFAULT_CHARS, 1000), MAX_ALLOWED_CHARS);
}

function positiveInt(value: number | undefined, fallback: number, max: number): number {
	return Math.min(Math.max(value ?? fallback, 1), max);
}

function truncateOutput(text: string, maxChars: number): { text: string; truncated: boolean } {
	if (text.length <= maxChars) return { text, truncated: false };
	return { text: text.slice(0, maxChars) + "\n…[truncated]", truncated: true };
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

async function runMind(args: string[], cwd: string, maxChars: number, signal?: AbortSignal): Promise<MindCommandResult> {
	return await new Promise<MindCommandResult>((resolve, reject) => {
		let stdout = "";
		let stderr = "";
		let settled = false;
		let timedOut = false;
		const child = spawn("aoc-mind-service", args, { cwd, stdio: ["ignore", "pipe", "pipe"], shell: false });
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
				reject(new Error("aoc-mind-service is not installed or not on PATH. Run aoc-init after installing AOC."));
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
			const projectRoot = ctx.cwd ?? process.cwd();
			const cwd = scopedCwd(projectRoot, params.cwd);
			const args = buildArgs(params, projectRoot);
			const maxChars = clampMaxChars(params.maxChars);
			const result = await runMind(args, cwd, maxChars, signal);
			const command = `aoc-mind-service ${args.map((arg) => (arg.includes(" ") ? JSON.stringify(arg) : arg)).join(" ")}`;
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
