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

type CommandContext = {
	cwd?: string;
	ui?: {
		notify?: (message: string, level?: "info" | "warning" | "error") => void | Promise<void>;
	};
};

type AutocompleteItem = {
	value: string;
	label?: string;
	description?: string;
};

type CommandDefinition = {
	description: string;
	getArgumentCompletions?: (prefix: string) => AutocompleteItem[] | null;
	handler: (args: string | string[] | undefined, ctx: CommandContext) => void | Promise<void>;
};

type OutboundMessage = {
	customType: string;
	display: boolean;
	content: string;
	details?: Record<string, unknown>;
};

type SendOptions = {
	triggerTurn?: boolean;
};

type DoxExtensionAPI = ExtensionAPI & {
	registerCommand?: (name: string, definition: CommandDefinition) => void;
	sendMessage?: (message: OutboundMessage, options?: SendOptions) => void | Promise<void>;
};

const DOX_COMMAND_COMPLETIONS: AutocompleteItem[] = [
	{ value: "full", label: "full", description: "Map, scout, map contracts, critic review, writer dry-run." },
	{ value: "scout", label: "scout", description: "Map metadata and launch dox-scout for target paths." },
	{ value: "map", label: "map", description: "Run aoc_dox map and summarize metadata." },
	{ value: "review", label: "review", description: "Review current .aoc/dox candidate decisions." },
	{ value: "doctor", label: "doctor", description: "Validate DOX metadata and AGENTS chain health." },
	{ value: "dry-run", label: "dry-run", description: "Run safe apply dry-run only." },
];

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

function renderDoxCommandPrompt(args: string | string[] | undefined): { mode: string; target: string; content: string } {
	const text = (Array.isArray(args) ? args.join(" ") : args ?? "").trim();
	const [rawMode = "full", ...rest] = text.split(/\s+/).filter(Boolean);
	const aliases: Record<string, string> = {
		run: "full",
		all: "full",
		full: "full",
		scout: "scout",
		map: "map",
		review: "review",
		doctor: "doctor",
		"dry-run": "dry-run",
		dryrun: "dry-run",
		apply: "dry-run",
	};
	const mode = aliases[rawMode.toLowerCase()];
	if (!mode) {
		throw new Error("Unknown /dox mode. Use one of: full, scout, map, review, doctor, dry-run.");
	}
	const target = rest.join(" ").trim();
	const targetLine = target ? `Target path or focus: \`${target}\`.` : "Target path or focus: repo-wide high-risk and insufficient-coverage paths only.";

	if (mode === "map") {
		return {
			mode,
			target,
			content: `Use the aoc-dox-cartography skill.

Run aoc_dox with action=map and json=true. Then summarize .aoc/dox/map.json, .aoc/dox/candidates.json, .aoc/dox/budgets.json, and .aoc/dox/routes.json. Do not launch subagents and do not run apply --yes.

${targetLine}`,
		};
	}
	if (mode === "review") {
		return {
			mode,
			target,
			content: `Use the aoc-dox-cartography skill.

Run aoc_dox with action=review and json=true. Summarize create/update/reject decisions, budget status, and next safe operator action. Do not write files and do not run apply --yes.

${targetLine}`,
		};
	}
	if (mode === "doctor") {
		return {
			mode,
			target,
			content: `Use the aoc-dox-cartography skill.

Run aoc_dox with action=doctor and json=true. If it fails, report exact invalid metadata, evidence paths, commands, or budget issues and propose the smallest safe metadata fix. Do not run apply --yes.

${targetLine}`,
		};
	}
	if (mode === "dry-run") {
		return {
			mode,
			target,
			content: `Use the aoc-dox-cartography skill.

Run aoc_dox with action=apply-dry-run and json=true. Report target AGENTS.md paths and rendered byte counts. Do not write AGENTS.md files and do not run apply --yes.

${targetLine}`,
		};
	}
	if (mode === "scout") {
		return {
			mode,
			target,
			content: `Use the aoc-dox-cartography skill.

Run aoc_dox with action=map and json=true first. Use .aoc/dox/map.json resolution coverage. Launch dox-scout in parallel for ${target ? `\`${target}\`` : "high-risk or insufficient-coverage paths only"}. Each scout must return DoxCandidate JSON or a fenced JSON array. Do not run dox-mapper, dox-writer, or apply --yes unless the operator asks for /dox full.

${targetLine}`,
		};
	}
	return {
		mode,
		target,
		content: `Use the aoc-dox-cartography skill.

Run the full safe DOX workflow:

1. Run aoc_dox with action=map and json=true.
2. Inspect .aoc/dox/map.json for AGENTS resolution coverage and use ${target ? `\`${target}\`` : "high-risk or insufficient-coverage paths only"} as the scout scope.
3. Launch dox-scout in parallel for the scoped paths.
4. Launch dox-mapper only for scout-approved candidate areas.
5. Launch dox-critic on every create/update proposal; treat reject as success.
6. Use dox-writer only after critic approval. Writer may edit only .aoc/dox/candidates.json and .aoc/dox/report.md.
7. Finish by running aoc_dox action=apply-dry-run, then aoc_dox action=doctor.

Never run aoc dox apply --yes from this command. Do not create or edit AGENTS.md directly.

${targetLine}`,
	};
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
	const commands = pi as DoxExtensionAPI;
	commands.registerCommand?.("dox", {
		description: "Usage: /dox [full|scout|map|review|doctor|dry-run] [path]. Run safe AOC DOX cartography with dox-* agents.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => {
			const query = prefix.trim().toLowerCase();
			if (!query) return DOX_COMMAND_COMPLETIONS;
			return DOX_COMMAND_COMPLETIONS.filter((item) => item.value.startsWith(query) || item.label?.toLowerCase().startsWith(query));
		},
		handler: async (args, ctx) => {
			try {
				const prompt = renderDoxCommandPrompt(args);
				if (typeof commands.sendMessage === "function") {
					await commands.sendMessage(
						{
							customType: "aoc.dox.request",
							display: true,
							content: prompt.content,
							details: { mode: prompt.mode, target: prompt.target, cwd: ctx.cwd },
						},
						{ triggerTurn: true },
					);
					return;
				}
				await ctx.ui?.notify?.(prompt.content, "info");
			} catch (err) {
				const message = err instanceof Error ? err.message : String(err);
				await ctx.ui?.notify?.(message, "error");
				throw err;
			}
		},
	});
}
