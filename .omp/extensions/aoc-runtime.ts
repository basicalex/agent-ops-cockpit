import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";

export type CommandResult = { ok: boolean; exitCode: number | null; stdout: string; stderr: string; timedOut: boolean; truncated: boolean };

export function stripAt(value: string): string {
	return value.startsWith("@") ? value.slice(1) : value;
}

export function findProjectRoot(start: string | undefined): string {
	let current = path.resolve(start || process.cwd());
	while (true) {
		if (fs.existsSync(path.join(current, ".aoc")) || fs.existsSync(path.join(current, ".git"))) return current;
		const parent = path.dirname(current);
		if (parent === current) return path.resolve(start || process.cwd());
		current = parent;
	}
}

export function scopedCwd(projectRoot: string, requested?: string): string {
	if (!requested || requested.trim().length === 0) return projectRoot;
	const resolved = path.resolve(projectRoot, stripAt(requested));
	const rel = path.relative(projectRoot, resolved);
	if (rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel))) return resolved;
	throw new Error(`cwd escapes project root: ${requested}`);
}

export function clampInt(value: number | undefined, fallback: number, min: number, max: number): number {
	if (!Number.isFinite(value ?? NaN)) return fallback;
	return Math.max(min, Math.min(max, Math.floor(value as number)));
}

export function clampMaxChars(value: number | undefined, fallback: number, max: number): number {
	return clampInt(value, fallback, 1000, max);
}

export function resolveRepoCommand(projectRoot: string, relativeBin: string, fallback: string): string {
	const local = path.join(projectRoot, relativeBin);
	return fs.existsSync(local) ? local : fallback;
}

export function renderCommand(command: string, args: string[]): string {
	return `${path.basename(command)} ${args.map((arg) => (arg.includes(" ") ? JSON.stringify(arg) : arg)).join(" ")}`;
}

function appendBounded(current: string, chunk: unknown, maxChars: number): { text: string; truncated: boolean } {
	if (current.length >= maxChars) return { text: current, truncated: true };
	const value = String(chunk);
	const remaining = maxChars - current.length;
	if (value.length <= remaining) return { text: current + value, truncated: false };
	return { text: current + value.slice(0, remaining), truncated: true };
}

export async function runBoundedCommand(
	command: string,
	args: string[],
	options: { cwd: string; maxStdoutChars: number; maxStderrChars?: number; timeoutMs: number; missingMessage: string; signal?: AbortSignal },
): Promise<CommandResult> {
	const { promise, resolve, reject } = Promise.withResolvers<CommandResult>();
	let stdout = "";
	let stderr = "";
	let settled = false;
	let timedOut = false;
	let truncated = false;
	const maxStderrChars = options.maxStderrChars ?? Math.min(options.maxStdoutChars, 8000);
	const child = spawn(command, args, { cwd: options.cwd, stdio: ["ignore", "pipe", "pipe"], shell: false });
	const timer = setTimeout(() => {
		timedOut = true;
		child.kill("SIGTERM");
	}, options.timeoutMs);
	const abort = () => child.kill("SIGTERM");
	options.signal?.addEventListener("abort", abort, { once: true });

	child.stdout.on("data", (chunk) => {
		const next = appendBounded(stdout, chunk, options.maxStdoutChars);
		stdout = next.text;
		truncated = truncated || next.truncated;
	});
	child.stderr.on("data", (chunk) => {
		const next = appendBounded(stderr, chunk, maxStderrChars);
		stderr = next.text;
		truncated = truncated || next.truncated;
	});
	child.on("error", (error: NodeJS.ErrnoException) => {
		if (settled) return;
		settled = true;
		clearTimeout(timer);
		options.signal?.removeEventListener("abort", abort);
		if (error.code === "ENOENT") {
			reject(new Error(options.missingMessage));
			return;
		}
		reject(error);
	});
	child.on("close", (exitCode) => {
		if (settled) return;
		settled = true;
		clearTimeout(timer);
		options.signal?.removeEventListener("abort", abort);
		resolve({ ok: exitCode === 0 && !timedOut, exitCode, stdout, stderr, timedOut, truncated });
	});
	return await promise;
}
