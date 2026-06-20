import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
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
const TRANSCRIPT_DEFAULT_TAIL = 20;

const HerdrActionSchema = StringEnum(
	["list", "get", "read", "explain", "wait", "transcript"] as const,
	{ description: "Read-only Herdr peer-agent observation action. No send/start/keys/state mutations are exposed." },
);

const HerdrParams = Type.Object({
	action: HerdrActionSchema,
	target: Type.Optional(Type.String({ description: "Peer target for get/read/explain/wait/transcript: pane_id (e.g. w<ID>:p<N>), agent name, terminal id, or label." })),
	source: Type.Optional(StringEnum(["visible", "recent", "recent-unwrapped"] as const, { description: "Pane read source for action=read. Defaults to recent." })),
	format: Type.Optional(StringEnum(["text", "ansi"] as const, { description: "Pane read format for action=read. Defaults to text." })),
	lines: Type.Optional(Type.Integer({ minimum: 1, maximum: READ_MAX_LINES, description: "Lines to read for action=read. Defaults to 50." })),
	status: Type.Optional(StringEnum(["idle", "working", "blocked", "unknown"] as const, { description: "Status to wait for, for action=wait. Required when action=wait." })),
	waitTimeoutMs: Type.Optional(Type.Integer({ minimum: WAIT_MIN_MS, maximum: WAIT_MAX_MS, description: "Wait deadline in ms for action=wait. Defaults to 10000." })),
	session: Type.Optional(Type.String({ description: "For action=transcript: session selector. 'latest' (default, most recently modified), a case-insensitive title substring, or a session id prefix." })),
	scope: Type.Optional(StringEnum(["last", "summary", "tail"] as const, { description: "For action=transcript: 'last' (default) = peer's last assistant response; 'summary' = title + first user ask + last assistant response + message counts; 'tail' = last N messages." })),
	maxChars: Type.Optional(Type.Integer({ minimum: 1000, maximum: MAX_ALLOWED_CHARS, description: "Maximum characters returned to the model." })),
});

export type HerdrParamsType = {
	action: "list" | "get" | "read" | "explain" | "wait" | "transcript";
	target?: string;
	source?: "visible" | "recent" | "recent-unwrapped";
	format?: "text" | "ansi";
	lines?: number;
	status?: "idle" | "working" | "blocked" | "unknown";
	waitTimeoutMs?: number;
	session?: string;
	scope?: "last" | "summary" | "tail";
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

type SessionEntry = { file: string; id: string; title: string; mtimeMs: number; startedAt: string };
type TranscriptMsg = { role: string; text: string; tools: string[] };

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}

function asText(value: unknown): string {
	return typeof value === "string" ? value : "";
}

function ompSessionsDir(): string {
	return path.join(os.homedir(), ".omp", "agent", "sessions");
}

function slugFromCwd(cwd: string): string {
	const home = os.homedir();
	const rel = cwd.startsWith(home + "/") ? cwd.slice(home.length) : cwd;
	return rel.split("/").join("-");
}

function readFirstLine(file: string): string {
	const fd = fs.openSync(file, "r");
	try {
		const chunks: Buffer[] = [];
		const buffer = Buffer.alloc(512);
		for (;;) {
			const bytes = fs.readSync(fd, buffer, 0, buffer.length, null);
			if (bytes === 0) break;
			const newline = buffer.subarray(0, bytes).indexOf(10);
			if (newline >= 0) {
				chunks.push(Buffer.from(buffer.subarray(0, newline)));
				break;
			}
			chunks.push(Buffer.from(buffer.subarray(0, bytes)));
			if (chunks.reduce((total, chunk) => total + chunk.length, 0) > 64 * 1024) break;
		}
		return Buffer.concat(chunks).toString("utf8");
	} finally {
		fs.closeSync(fd);
	}
}

function listSessions(dir: string): SessionEntry[] {
	if (!fs.existsSync(dir)) return [];
	return fs.readdirSync(dir)
		.filter((name) => name.endsWith(".jsonl"))
		.flatMap((name) => {
			const file = path.join(dir, name);
			try {
				const first = JSON.parse(readFirstLine(file)) as unknown;
				if (!isRecord(first) || first.type !== "session") return [];
				const stat = fs.statSync(file);
				return [{
					file,
					id: asText(first.id),
					title: asText(first.title) || "(untitled)",
					mtimeMs: stat.mtimeMs,
					startedAt: asText(first.timestamp),
				}];
			} catch {
				return [];
			}
		})
		.sort((left, right) => right.mtimeMs - left.mtimeMs);
}

function readTranscript(file: string): { title: string; id: string; cwd: string; startedAt: string; msgs: TranscriptMsg[] } {
	let title = "(untitled)";
	let id = "";
	let cwd = "";
	let startedAt = "";
	const msgs: TranscriptMsg[] = [];
	for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
		if (!line.trim()) continue;
		let parsed: unknown;
		try {
			parsed = JSON.parse(line);
		} catch {
			continue;
		}
		if (!isRecord(parsed)) continue;
		if (parsed.type === "session") {
			title = asText(parsed.title) || title;
			id = asText(parsed.id) || id;
			cwd = asText(parsed.cwd) || cwd;
			startedAt = asText(parsed.timestamp) || startedAt;
			continue;
		}
		if (parsed.type !== "message" || !isRecord(parsed.message)) continue;
		const role = asText(parsed.message.role);
		const content = parsed.message.content;
		const tools: string[] = [];
		const textParts: string[] = [];
		if (typeof content === "string") {
			textParts.push(content);
		} else if (Array.isArray(content)) {
			for (const block of content) {
				if (!isRecord(block)) continue;
				if (block.type === "text" || block.type === "output_text") {
					const text = asText(block.text);
					if (text) textParts.push(text);
				} else if (block.type === "tool_use" || block.type === "toolCall") {
					const name = asText(block.name);
					if (name) tools.push(name);
				}
			}
		}
		msgs.push({ role, text: textParts.join("\n"), tools });
	}
	return { title, id, cwd, startedAt, msgs };
}

function pickSession(entries: SessionEntry[], selector: string): SessionEntry | undefined {
	const trimmed = selector.trim();
	if (!trimmed || trimmed === "latest") return entries[0];
	const direct = entries.find((entry) => entry.id.startsWith(trimmed) || entry.file.includes(trimmed));
	if (direct) return direct;
	const lower = trimmed.toLowerCase();
	return entries.find((entry) => entry.title.toLowerCase().includes(lower));
}

function truncateText(text: string, maxLength: number): string {
	if (text.length <= maxLength) return text;
	return `${text.slice(0, Math.max(0, maxLength - 15)).trimEnd()}\n...(truncated)`;
}

function truncateWithHeader(header: string, body: string, maxChars: number): { text: string; truncated: boolean } {
	const prefix = `${header}\n\n`;
	const full = `${prefix}${body}`;
	if (full.length <= maxChars) return { text: full, truncated: false };
	return { text: `${prefix}${body.slice(0, Math.max(0, maxChars - prefix.length - 15)).trimEnd()}\n...(truncated)`, truncated: true };
}

function messageCounts(msgs: TranscriptMsg[]): { user: number; assistant: number; toolResult: number } {
	return {
		user: msgs.filter((msg) => msg.role === "user").length,
		assistant: msgs.filter((msg) => msg.role === "assistant").length,
		toolResult: msgs.filter((msg) => msg.role === "toolResult" || msg.role === "tool" || msg.role === "tool_result").length,
	};
}

function reasonResult(target: string, reason: string) {
	return {
		content: [{ type: "text", text: reason }],
		details: { action: "transcript", target, ok: false, reason },
	};
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
		case "transcript":
			args.push("agent", "get", requireText(params.target, "target"));
			break;
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

export async function runTranscript(params: HerdrParamsType, cwd: string, maxChars: number, signal?: AbortSignal) {
	const target = requireText(params.target, "target");
	const commandBin = "herdr";
	const args = ["agent", "get", target];
	const result = await runHerdr(commandBin, args, cwd, maxChars, COMMAND_TIMEOUT_MS, signal);
	const failReason = "could not resolve peer (herdr get failed); transcript needs a reachable OMP peer";
	let parsed: unknown;
	try {
		parsed = JSON.parse(result.stdout);
	} catch {
		return reasonResult(target, failReason);
	}
	if (!result.ok || !isRecord(parsed) || !isRecord(parsed.result) || !isRecord(parsed.result.agent)) {
		return reasonResult(target, failReason);
	}
	const agent = parsed.result.agent;
	const agentKind = asText(agent.agent);
	const agentCwd = asText(agent.cwd);
	if (agentKind !== "omp") {
		return reasonResult(target, `peer is '${agentKind}', not omp; transcript reading supports OMP peers — use action=read for pane scrollback`);
	}
	const dir = path.join(ompSessionsDir(), slugFromCwd(agentCwd));
	const entries = listSessions(dir);
	if (entries.length === 0) {
		return reasonResult(target, `no OMP sessions found for peer cwd ${agentCwd}; the peer may not be OMP or has no recorded sessions — use action=read for pane scrollback`);
	}
	const selector = (params.session ?? "latest").trim();
	const selected = pickSession(entries, selector);
	if (!selected) {
		const recent = entries.slice(0, 12).map((entry) => `- ${entry.title} (${entry.startedAt || "unknown date"})`).join("\n");
		const reason = "no session match";
		return {
			content: [{ type: "text", text: `no session matched '${params.session}'. Recent sessions:\n${recent}` }],
			details: { action: "transcript", target, ok: false, reason },
		};
	}
	const transcript = readTranscript(selected.file);
	const counts = messageCounts(transcript.msgs);
	const assistantMsgs = transcript.msgs.filter((msg) => msg.role === "assistant" && msg.text.trim());
	const scope = params.scope ?? "last";
	const header = [
		`$ aoc_herdr transcript target=${target} session=${params.session ?? "latest"} scope=${scope}`,
		"",
		`session: ${transcript.title} (${transcript.startedAt})`,
		`peer cwd: ${agentCwd}`,
		`messages: ${transcript.msgs.length} (${counts.user} user, ${counts.assistant} assistant, ${counts.toolResult} toolResult)`,
	].join("\n");
	const bodyLines: string[] = [];
	if (scope === "summary") {
		const firstUser = transcript.msgs.find((msg) => msg.role === "user" && msg.text.trim());
		bodyLines.push(
			"first user ask:",
			firstUser ? truncateText(firstUser.text.trim(), 600) : "(none)",
			"",
			"last assistant response:",
			assistantMsgs.length ? truncateText(assistantMsgs[assistantMsgs.length - 1].text.trim(), 1500) : "(none)",
		);
	} else if (scope === "tail") {
		for (const msg of transcript.msgs.slice(-TRANSCRIPT_DEFAULT_TAIL)) {
			const text = msg.text.trim();
			if (!text && msg.tools.length === 0) continue;
			const suffix = msg.tools.length ? ` [tools: ${msg.tools.join(", ")}]` : "";
			bodyLines.push(`${msg.role}: ${text}${suffix}`);
		}
	} else if (assistantMsgs.length) {
		bodyLines.push("last assistant response:", assistantMsgs[assistantMsgs.length - 1].text.trim());
	} else {
		bodyLines.push("last assistant response:", "(no assistant text in session)");
	}
	const truncated = truncateWithHeader(header, bodyLines.join("\n"), maxChars);
	return {
		content: [{ type: "text", text: truncated.text }],
		details: {
			action: "transcript",
			target,
			ok: true,
			peer: { agent: agentKind, cwd: agentCwd },
			session: { title: transcript.title, id: transcript.id, startedAt: transcript.startedAt },
			scope,
			selector: params.session ?? "latest",
			messageCount: transcript.msgs.length,
			truncated: truncated.truncated,
		},
	};
}

export default function aocHerdrExtension(pi: ExtensionAPI): void {
	pi.registerTool({
		name: "aoc_herdr",
		label: "AOC Herdr",
		description: "Read-only observation of peer agents in the Herdr session: list peers and their idle/working/blocked state, read pane scrollback, read a peer OMP session transcript for real context, explain detection state, or barrier-wait until a peer reaches a status. Requires a running Herdr server (run inside a Herdr pane).",
		promptSnippet: "Observe peer Herdr agents: list, read pane context, read transcript context, explain, or wait on a status.",
		promptGuidelines: [
			"Use aoc_herdr to observe peer agents in the Herdr session: list agents and their idle/working/blocked state, read a peer's recent pane for on-demand context, read a peer OMP transcript for last response, summary, or recent tail, explain a peer's detection state, or barrier-wait until a peer reaches a status.",
			"aoc_herdr is strictly read-only. It must not send text, keystrokes, prompts, or commands to peers, and must not start, focus, rename, close, split, move, or report state on panes or agents. No such actions are exposed.",
			"Prefer list to discover a target (pane_id, agent name, or terminal id), then pass it to get/read/transcript/explain/wait. wait is blocking and bounded; always rely on its timeout and never chain waits that could deadlock on peers waiting on each other.",
			"For a peer's actual work (its last response or what it planned/did), prefer action=transcript over action=read — read only shows pane scrollback, which is empty when the peer is idle. transcript reads the peer's saved OMP session; it falls back gracefully if the peer is not OMP or has no sessions.",
			"aoc_herdr only works inside a Herdr-managed session. If it reports herdr missing or no server reachable, do not retry in a loop; continue within your own session and note the limitation.",
		],
		parameters: HerdrParams,
		async execute(_toolCallId, params: HerdrParamsType, signal, _onUpdate, ctx) {
			const cwd = findProjectRoot(ctx.cwd);
			const maxChars = clampMaxChars(params.maxChars, MAX_DEFAULT_CHARS, MAX_ALLOWED_CHARS);
			if (params.action === "transcript") return await runTranscript(params, cwd, maxChars, signal);
			const args = buildArgs(params);
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
