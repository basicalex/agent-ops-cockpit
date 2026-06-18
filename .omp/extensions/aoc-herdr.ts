import { clampInt, clampMaxChars, findProjectRoot, renderCommand, runBoundedCommand } from "./aoc-runtime";
import { StringEnum } from "@mariozechner/pi-ai";
import { Type } from "@sinclair/typebox";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const MAX_DEFAULT_CHARS = 12_000;
const MAX_ALLOWED_CHARS = 40_000;
const COMMAND_TIMEOUT_MS = 30_000;
const WAIT_GRACE_MS = 5_000;
const WAIT_DEFAULT_MS = 10_000;
const WAIT_MIN_MS = 500;
const WAIT_MAX_MS = 30_000;
const READ_DEFAULT_LINES = 50;
const READ_MAX_LINES = 1000;

const HerdrActionSchema = StringEnum(
	["list", "get", "read", "explain", "wait"] as const,
	{ description: "Read-only Herdr peer-agent observation action. No send/start/keys/state mutations are exposed." },
);

const HerdrParams = Type.Object({
	action: HerdrActionSchema,
	target: Type.Optional(Type.String({ description: "Peer target for get/read/explain/wait: pane_id (e.g. w<ID>:p<N>), agent name, terminal id, or label." })),
	source: Type.Optional(StringEnum(["visible", "recent", "recent-unwrapped"] as const, { description: "Pane read source for action=read. Defaults to recent." })),
	format: Type.Optional(StringEnum(["text", "ansi"] as const, { description: "Pane read format for action=read. Defaults to text." })),
	lines: Type.Optional(Type.Integer({ minimum: 1, maximum: READ_MAX_LINES, description: "Lines to read for action=read. Defaults to 50." })),
	status: Type.Optional(StringEnum(["idle", "working", "blocked", "unknown"] as const, { description: "Status to wait for, for action=wait. Required when action=wait." })),
	waitTimeoutMs: Type.Optional(Type.Integer({ minimum: WAIT_MIN_MS, maximum: WAIT_MAX_MS, description: "Wait deadline in ms for action=wait. Defaults to 10000." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});

type HerdrParamsType = {
	action: "list" | "get" | "read" | "explain" | "wait";
	target?: string;
	source?: "visible" | "recent" | "recent-unwrapped";
	format?: "text" | "ansi";
	lines?: number;
	status?: "idle" | "working" | "blocked" | "unknown";
	waitTimeoutMs?: number;
	maxChars?: number;
};

function requireText(value: string | undefined, label: string): string {
	const trimmed = (value ?? "").trim();
	if (!trimmed) throw new Error(`${label} is required`);
	return trimmed;
}

function waitMsFrom(params: HerdrParamsType): number {
	return clampInt(params.waitTimeoutMs, WAIT_DEFAULT_MS, WAIT_MIN_MS, WAIT_MAX_MS);
}

function buildArgs(params: HerdrParamsType): string[] {
	const args: string[] = [];
	switch (params.action) {
		case "list":
			args.push("agent", "list");
			break;
		case "get":
			args.push("agent", "get", requireText(params.target, "target"));
			break;
		case "read":
			args.push(
				"agent", "read", requireText(params.target, "target"),
				"--source", params.source ?? "recent",
				"--lines", String(clampInt(params.lines, READ_DEFAULT_LINES, 1, READ_MAX_LINES)),
				"--format", params.format ?? "text",
			);
			break;
		case "explain":
			args.push("agent", "explain", requireText(params.target, "target"), "--json");
			break;
		case "wait": {
			args.push(
				"agent", "wait", requireText(params.target, "target"),
				"--status", requireText(params.status, "status"),
				"--timeout", String(waitMsFrom(params)),
			);
			break;
		}
	}
	return args;
}

async function runHerdr(command: string, args: string[], cwd: string, maxChars: number, timeoutMs: number, signal?: AbortSignal) {
	return await runBoundedCommand(command, args, {
		cwd,
		maxStdoutChars: maxChars,
		maxStderrChars: Math.min(maxChars, 8000),
		timeoutMs,
		missingMessage: "herdr is not installed or not on PATH, or no Herdr server is reachable. aoc_herdr only works inside a Herdr-managed session.",
		signal,
	});
}

export default function aocHerdrExtension(pi: ExtensionAPI): void {
	pi.registerTool({
		name: "aoc_herdr",
		label: "AOC Herdr",
		description: "Read-only observation of peer agents in the Herdr session: list peers and their idle/working/blocked state, read a peer's recent pane for context, explain a peer's detection state, or barrier-wait until a peer reaches a status. Requires a running Herdr server (run inside a Herdr pane).",
		promptSnippet: "Observe peer Herdr agents: list, read recent pane context, explain, or wait on a status.",
		promptGuidelines: [
			"Use aoc_herdr to observe peer agents in the Herdr session: list agents and their idle/working/blocked state, read a peer's recent pane for on-demand context, explain a peer's detection state, or barrier-wait until a peer reaches a status.",
			"aoc_herdr is strictly read-only. It must not send text, keystrokes, prompts, or commands to peers, and must not start, focus, rename, close, split, move, or report state on panes or agents. No such actions are exposed.",
			"Prefer list to discover a target (pane_id, agent name, or terminal id), then pass it to get/read/explain/wait. wait is blocking and bounded; always rely on its timeout and never chain waits that could deadlock on peers waiting on each other.",
			"aoc_herdr only works inside a Herdr-managed session. If it reports herdr missing or no server reachable, do not retry in a loop; continue within your own session and note the limitation.",
		],
		parameters: HerdrParams,
		async execute(_toolCallId, params: HerdrParamsType, signal, _onUpdate, ctx) {
			const cwd = findProjectRoot(ctx.cwd);
			const args = buildArgs(params);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			const commandBin = "herdr";
			const timeoutMs = params.action === "wait" ? waitMsFrom(params) + WAIT_GRACE_MS : COMMAND_TIMEOUT_MS;
			const result = await runHerdr(commandBin, args, cwd, maxChars, timeoutMs, signal);
			const command = renderCommand(commandBin, args);
			// herdr agent wait prints "timed out waiting for agent status change" to stderr and
			// exits 1 on its own timeout (confirmed live); the match case prints JSON to stdout
			// and exits 0. A wait timeout is a legitimate observation, not unavailable evidence.
			const waitExpired = params.action === "wait" && result.stderr.includes("timed out waiting for agent status change");
			const lines = [
				`$ ${command}`,
				`exit: ${result.exitCode}${result.timedOut ? " (timed out)" : ""}${result.truncated ? " (truncated)" : ""}`,
			];
			if (result.stdout.trim()) lines.push("", result.stdout.trimEnd());
			if (result.stderr.trim()) lines.push("", "stderr:", result.stderr.trimEnd());
			if (waitExpired) {
				lines.push("", `wait expired: target did not reach status ${params.status} within ${waitMsFrom(params)}ms`);
			} else if (!result.ok) {
				lines.push("", "Herdr did not complete successfully. Treat this as unavailable evidence; continue within your own session and note the limitation.");
			}
			return {
				content: [{ type: "text", text: lines.join("\n") }],
				details: { action: params.action, command, target: params.target ?? null, ok: result.ok, exitCode: result.exitCode, timedOut: result.timedOut, truncated: result.truncated, ...(params.action === "wait" ? { waitExpired } : {}) },
			};
		},
	});
}
