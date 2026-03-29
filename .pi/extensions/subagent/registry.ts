import * as fs from "node:fs";
import * as net from "node:net";
import * as path from "node:path";
import { withArtifactRefs } from "./artifacts.ts";
import {
	PULSE_COMMAND_TIMEOUT_MS,
	now,
	randomId,
	relative,
	truncate,
	type JobMode,
	type JobRecord,
	type JobStatus,
} from "./shared.ts";

export type PulseCommandResultPayload = {
	command: string;
	status: string;
	message?: string;
	error?: { code?: string; message?: string };
};

export type PulseEnvelope = {
	version?: string | number;
	type?: string;
	session_id?: string;
	sender_id?: string;
	timestamp?: string;
	request_id?: string;
	payload?: any;
};

export type DurableDetachedJob = {
	job_id: string;
	parent_job_id?: string | null;
	owner_plane?: string;
	worker_kind?: string | null;
	mode?: string;
	status?: string;
	agent?: string | null;
	team?: string | null;
	chain?: string | null;
	created_at_ms?: number;
	started_at_ms?: number | null;
	finished_at_ms?: number | null;
	current_step_index?: number | null;
	step_count?: number | null;
	output_excerpt?: string | null;
	stdout_excerpt?: string | null;
	stderr_excerpt?: string | null;
	error?: string | null;
	fallback_used?: boolean;
};

export type DurableDetachedStatusResult = {
	status?: string;
	jobs?: DurableDetachedJob[];
	active_jobs?: number;
	fallback_used?: boolean;
};

export type DurableDetachedCancelResult = {
	job_id: string;
	status?: string;
	summary?: string;
	cancelled?: boolean;
	fallback_used?: boolean;
};

export type DurableDetachedDispatchResult = {
	status?: string;
	summary?: string;
	accepted?: boolean;
	fallback_used?: boolean;
	job?: DurableDetachedJob;
};

export type MindContextPackPayload = {
	schema_version?: string;
	mode?: string;
	profile?: string;
	role?: string;
	active_tag?: string | null;
	reason?: string | null;
	truncated?: boolean;
	rendered_lines?: string[];
	citations?: Array<{ title?: string; reference?: string; source_id?: string }>;
};

export function currentSessionId(): string | undefined {
	const value = process.env.AOC_SESSION_ID?.trim();
	return value ? value : undefined;
}

export function currentPaneId(): string | undefined {
	const value = process.env.AOC_PANE_ID?.trim() || process.env.ZELLIJ_PANE_ID?.trim();
	return value ? value : undefined;
}

export function currentAgentKey(): string | undefined {
	const sessionId = currentSessionId();
	const paneId = currentPaneId();
	if (!sessionId || !paneId) return undefined;
	return `${sessionId}::${paneId}`;
}

export function sessionSlug(sessionId: string): string {
	let slug = sessionId.replace(/[^A-Za-z0-9._-]/g, "-");
	while (slug.includes("--")) slug = slug.replace(/--/g, "-");
	return slug.replace(/^-|-$/g, "") || "session";
}

export function resolvePulseSocketPath(): string | undefined {
	const explicit = process.env.AOC_PULSE_SOCK?.trim();
	if (explicit) return explicit;
	const sessionId = currentSessionId();
	if (!sessionId) return undefined;
	const runtimeDir = process.env.XDG_RUNTIME_DIR?.trim()
		|| (process.env.UID?.trim() ? `/run/user/${process.env.UID.trim()}` : "/tmp");
	return path.join(runtimeDir, "aoc", sessionSlug(sessionId), "pulse.sock");
}

export function pulseClientId(): string {
	return `pi-subagent-${process.pid}-${randomId()}`;
}

export function detachedRegistryAvailability(): { available: boolean; note?: string } {
	const socketPath = resolvePulseSocketPath();
	if (!currentSessionId() || !currentAgentKey()) {
		return { available: false, note: "detached registry unavailable; showing cached jobs" };
	}
	if (!socketPath) {
		return { available: false, note: "detached registry socket unavailable; showing cached jobs" };
	}
	if (!fs.existsSync(socketPath)) {
		return { available: false, note: "pulse socket unavailable; showing cached jobs" };
	}
	return { available: true };
}

function isTerminalCommandStatus(status: string | undefined): boolean {
	if (!status) return false;
	return status !== "accepted" && status !== "queued" && status !== "running";
}

export function modeFromDurable(mode?: string): JobMode {
	switch (mode) {
		case "chain":
			return "chain";
		case "parallel":
			return "parallel";
		case "dispatch":
		default:
			return "dispatch";
	}
}

export function statusFromDurable(status?: string): JobStatus {
	switch (status) {
		case "queued":
		case "running":
		case "success":
		case "fallback":
		case "error":
		case "cancelled":
		case "stale":
			return status;
		default:
			return "error";
	}
}

export function mapDurableJob(job: DurableDetachedJob, root: string): JobRecord {
	const agent = job.agent || job.chain || job.team || "detached-job";
	return withArtifactRefs(root, {
		jobId: job.job_id,
		mode: modeFromDurable(job.mode),
		executionMode: "background",
		agent,
		agentFile: job.agent ? relative(root, path.join(root, ".pi", "agents", `${job.agent}.md`)) : "durable-registry",
		status: statusFromDurable(job.status),
		task: "",
		cwd: root,
		createdAt: job.created_at_ms ?? now(),
		startedAt: job.started_at_ms ?? undefined,
		finishedAt: job.finished_at_ms ?? undefined,
		model: undefined,
		tools: [],
		outputExcerpt: truncate(job.output_excerpt ?? job.stdout_excerpt ?? undefined),
		stderrExcerpt: truncate(job.stderr_excerpt ?? undefined, 320),
		error: truncate(job.error ?? undefined, 320),
		fallbackUsed: Boolean(job.fallback_used),
		manifestErrors: [],
		teamName: job.team ?? undefined,
		chainName: job.chain ?? undefined,
		chainStepIndex: job.current_step_index ?? undefined,
		chainStepCount: job.step_count ?? undefined,
	});
}

export async function sendPulseCommand(command: string, args: Record<string, unknown>): Promise<PulseCommandResultPayload> {
	const sessionId = currentSessionId();
	const targetAgentId = currentAgentKey();
	const socketPath = resolvePulseSocketPath();
	if (!sessionId || !targetAgentId || !socketPath) {
		throw new Error("detached registry unavailable: missing AOC session/pane/socket context");
	}

	const requestId = `subagent-${now()}-${randomId()}`;
	const senderId = pulseClientId();
	const writeEnvelope = (socket: net.Socket, type: string, payload: any, request?: string) => {
		const envelope = {
			version: "1",
			type,
			session_id: sessionId,
			sender_id: senderId,
			timestamp: new Date().toISOString(),
			request_id: request,
			payload,
		};
		socket.write(`${JSON.stringify(envelope)}\n`);
	};

	return await new Promise<PulseCommandResultPayload>((resolve, reject) => {
		const socket = net.createConnection(socketPath);
		let settled = false;
		let buffer = "";
		const finish = (error?: Error, result?: PulseCommandResultPayload) => {
			if (settled) return;
			settled = true;
			clearTimeout(timeout);
			socket.destroy();
			if (error) reject(error);
			else resolve(result!);
		};
		const timeout = setTimeout(() => finish(new Error(`pulse command timed out after ${PULSE_COMMAND_TIMEOUT_MS}ms`)), PULSE_COMMAND_TIMEOUT_MS);

		socket.on("connect", () => {
			writeEnvelope(socket, "hello", {
				client_id: senderId,
				role: "subscriber",
				capabilities: ["snapshot", "delta", "command_result"],
			});
			writeEnvelope(socket, "subscribe", { topics: ["command_result"] });
			writeEnvelope(
				socket,
				"command",
				{ command, target_agent_id: targetAgentId, args },
				requestId,
			);
		});

		socket.on("data", (chunk: Buffer) => {
			buffer += chunk.toString("utf8");
			const lines = buffer.split("\n");
			buffer = lines.pop() ?? "";
			for (const line of lines) {
				if (!line.trim()) continue;
				let envelope: PulseEnvelope;
				try {
					envelope = JSON.parse(line);
				} catch {
					continue;
				}
				if (envelope.session_id !== sessionId) continue;
				if (envelope.request_id !== requestId) continue;
				if (envelope.type !== "command_result") continue;
				const payload = envelope.payload as PulseCommandResultPayload | undefined;
				if (!payload) continue;
				if (!isTerminalCommandStatus(payload.status)) continue;
				finish(undefined, payload);
				return;
			}
		});

		socket.on("error", (error) => finish(error instanceof Error ? error : new Error(String(error))));
		socket.on("close", () => {
			if (!settled) finish(new Error("pulse socket closed before detached registry response arrived"));
		});
	});
}

export async function fetchMindContextPack(role: string, reason: string): Promise<MindContextPackPayload | undefined> {
	try {
		const result = await sendPulseCommand("mind_context_pack", {
			mode: "dispatch",
			role,
			reason,
			detail: false,
		});
		if (result.status !== "ok" || !result.message) return undefined;
		return JSON.parse(result.message) as MindContextPackPayload;
	} catch {
		return undefined;
	}
}

export function renderContextPackPrelude(pack: MindContextPackPayload | undefined): string | undefined {
	const lines = pack?.rendered_lines?.filter((line) => typeof line === "string" && line.trim());
	if (!lines || lines.length === 0) return undefined;
	const citations = (pack?.citations ?? [])
		.slice(0, 6)
		.map((citation) => citation.reference || citation.title || citation.source_id)
		.filter((value): value is string => Boolean(value));
	const suffix = citations.length > 0 ? `\n\nContext citations: ${citations.join(", ")}` : "";
	return `${lines.join("\n")}${suffix}`;
}
