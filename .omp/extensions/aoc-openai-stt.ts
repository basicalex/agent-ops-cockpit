import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";

type RecordingState = {
	process: ChildProcessWithoutNullStreams;
	filePath: string;
	tool: "ffmpeg" | "arecord" | "sox";
};

const STATUS_KEY = "aoc-openai-stt";
const DEFAULT_MODEL = "gpt-4o-mini-transcribe";
const DEFAULT_LANGUAGE = "en";
const MIN_AUDIO_BYTES = 128;
const MAX_PROMPT_CHARS = 1800;
const LEXICON_RELATIVE_PATH = ".aoc/lexicon.md";

let recording: RecordingState | undefined;
let transcribing = false;

function readDotEnvValue(filePath: string, key: string): string | undefined {
	if (!fs.existsSync(filePath)) return undefined;
	const text = fs.readFileSync(filePath, "utf8");
	for (const line of text.split(/\r?\n/)) {
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith("#")) continue;
		const separator = trimmed.indexOf("=");
		if (separator < 1) continue;
		if (trimmed.slice(0, separator).trim() !== key) continue;
		let value = trimmed.slice(separator + 1).trim();
		if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
			value = value.slice(1, -1);
		}
		return value;
	}
	return undefined;
}

function usableOpenAIKey(value: string | undefined): string | undefined {
	if (!value || !value.trim()) return undefined;
	const trimmed = value.trim();
	if (trimmed === "your_openai_api_key_here") return undefined;
	return trimmed;
}


function resolveOpenAIKey(): string | undefined {
	return (
		usableOpenAIKey(readDotEnvValue(path.join(os.homedir(), ".omp", "agent", ".env"), "OPENAI_API_KEY")) ||
		usableOpenAIKey(process.env.OPENAI_API_KEY)
	);
}

function resolveSttModel(): string {
	return process.env.AOC_OPENAI_STT_MODEL || DEFAULT_MODEL;
}

function extractLexiconTerms(text: string): string[] {
	const terms: string[] = [];
	for (const line of text.split(/\r?\n/)) {
		const heading = line.match(/^###\s+(.+)$/);
		if (heading) {
			terms.push(heading[1].trim());
			continue;
		}
		const aliases = line.match(/^Aliases:\s+(.+)$/i);
		if (aliases) {
			for (const alias of aliases[1].split(",")) {
				const term = alias.trim();
				if (term && term.toLowerCase() !== "none") terms.push(term);
			}
		}
	}
	return Array.from(new Set(terms)).filter(Boolean);
}

function buildTranscriptionPrompt(cwd: string): string | undefined {
	const lexiconPath = path.join(cwd, LEXICON_RELATIVE_PATH);
	if (!fs.existsSync(lexiconPath)) return undefined;
	const terms = extractLexiconTerms(fs.readFileSync(lexiconPath, "utf8"));
	const combinedTerms = ["Oh My Pi", "OMP", "Agent Ops Cockpit", "AOC", ...terms];
	const uniqueTerms = Array.from(new Set(combinedTerms)).filter(Boolean);
	if (uniqueTerms.length === 0) return undefined;
	return `Use these project-specific spellings and phrases when transcribing: ${uniqueTerms.join("; ")}`.slice(0, MAX_PROMPT_CHARS);
}


function commandExists(command: string): boolean {
	const result = Bun.spawnSync(["sh", "-lc", `command -v ${command}`], { stdout: "ignore", stderr: "ignore" });
	return result.exitCode === 0;
}

function unmuteDefaultSource(): boolean {
	if (!commandExists("pactl")) return false;
	const result = Bun.spawnSync(["pactl", "set-source-mute", "@DEFAULT_SOURCE@", "0"], {
		stdout: "ignore",
		stderr: "ignore",
	});
	return result.exitCode === 0;
}


function startRecorder(filePath: string): RecordingState {
	if (commandExists("ffmpeg")) {
		return {
			process: spawn("ffmpeg", ["-hide_banner", "-loglevel", "error", "-f", "pulse", "-i", "default", "-ar", "16000", "-ac", "1", "-sample_fmt", "s16", "-y", filePath]),
			filePath,
			tool: "ffmpeg",
		};
	}
	if (commandExists("arecord")) {
		return {
			process: spawn("arecord", ["-f", "S16_LE", "-r", "16000", "-c", "1", filePath]),
			filePath,
			tool: "arecord",
		};
	}
	if (commandExists("sox")) {
		return {
			process: spawn("sox", ["-d", "-r", "16000", "-c", "1", "-b", "16", "-t", "wav", filePath]),
			filePath,
			tool: "sox",
		};
	}
	throw new Error("No recording tool found. Install ffmpeg, arecord, or sox.");
}

async function waitForRecorderStart(state: RecordingState): Promise<void> {
	const { promise, resolve, reject } = Promise.withResolvers<void>();
	let settled = false;
	const timer = setTimeout(() => {
		if (settled) return;
		settled = true;
		resolve();
	}, 350);
	state.process.once("error", (error) => {
		if (settled) return;
		settled = true;
		clearTimeout(timer);
		reject(error);
	});
	state.process.once("exit", (code) => {
		if (settled) return;
		settled = true;
		clearTimeout(timer);
		reject(new Error(`${state.tool} exited before recording started (${code ?? "signal"}).`));
	});
	return promise;
}

async function stopRecorder(state: RecordingState): Promise<void> {
	const { promise, resolve } = Promise.withResolvers<void>();
	state.process.once("close", () => resolve());
	if (state.tool === "ffmpeg") {
		state.process.stdin.write("q");
		state.process.stdin.end();
	} else {
		state.process.kill("SIGTERM");
	}
	const killTimer = setTimeout(() => state.process.kill("SIGKILL"), 3000);
	await promise;
	clearTimeout(killTimer);
}

async function transcribeWithOpenAI(filePath: string, cwd: string): Promise<string> {
	const apiKey = resolveOpenAIKey();
	if (!apiKey) throw new Error("OPENAI_API_KEY is not set. Add it to ~/.omp/agent/.env or the omp environment.");
	const stat = fs.statSync(filePath);
	if (stat.size < MIN_AUDIO_BYTES) throw new Error("Recording was empty. Check microphone input.");

	const body = new FormData();
	body.set("model", resolveSttModel());
	body.set("language", process.env.AOC_OPENAI_STT_LANGUAGE || DEFAULT_LANGUAGE);
	const prompt = buildTranscriptionPrompt(cwd);
	if (prompt) body.set("prompt", prompt);
	body.set("file", Bun.file(filePath), "speech.wav");

	const response = await fetch("https://api.openai.com/v1/audio/transcriptions", {
		method: "POST",
		headers: { Authorization: `Bearer ${apiKey}` },
		body,
	});
	const text = await response.text();
	if (!response.ok) {
		let message = text.trim();
		try {
			const parsed = JSON.parse(text) as { error?: { message?: string } };
			message = parsed.error?.message || message;
		} catch {
			// Keep raw response text.
		}
		throw new Error(`OpenAI transcription failed (${response.status}): ${message}`);
	}
	const parsed = JSON.parse(text) as { text?: string };
	return (parsed.text || "").trim();
}

async function toggleOpenAIStt(ctx: ExtensionContext): Promise<void> {
	if (transcribing) {
		ctx.ui.notify("OpenAI STT transcription is already running.", "info");
		return;
	}
	if (!recording) {
		const filePath = path.join(os.tmpdir(), `aoc-openai-stt-${Date.now()}.wav`);
		const unmuted = unmuteDefaultSource();
		const state = startRecorder(filePath);
		recording = state;
		await waitForRecorderStart(state);
		ctx.ui.setStatus(STATUS_KEY, "AOC STT recording — press Alt+Space again to transcribe");
		ctx.ui.notify(unmuted ? "AOC OpenAI STT recording started; microphone was unmuted." : "AOC OpenAI STT recording started.", "info");
		return;
	}

	const state = recording;
	recording = undefined;
	transcribing = true;
	ctx.ui.setStatus(STATUS_KEY, "AOC STT transcribing with OpenAI...");
	try {
		await stopRecorder(state);
		const text = await transcribeWithOpenAI(state.filePath, ctx.cwd);
		if (text) {
			ctx.ui.pasteToEditor(text);
			ctx.ui.notify("AOC OpenAI STT inserted transcription.", "info");
		} else {
			ctx.ui.notify("AOC OpenAI STT heard no speech.", "warning");
		}
	} finally {
		transcribing = false;
		ctx.ui.setStatus(STATUS_KEY, undefined);
		fs.rm(state.filePath, { force: true }, () => undefined);
	}
}

export default function aocOpenAISttExtension(pi: ExtensionAPI): void {
	pi.registerShortcut("alt+space", {
		description: "Toggle AOC OpenAI speech-to-text",
		handler: toggleOpenAIStt,
	});
	pi.registerCommand("aoc-stt", {
		description: "Toggle AOC OpenAI speech-to-text recording/transcription.",
		handler: async (_args, ctx) => toggleOpenAIStt(ctx),
	});
}
