import fs from "node:fs";
import os from "node:os";
import path from "node:path";

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

type BeforeAgentStartEvent = {
	systemPrompt: string;
};

type ExtensionAPI = {
	registerCommand: (name: string, definition: CommandDefinition) => void;
	sendMessage?: (message: OutboundMessage, options?: SendOptions) => void | Promise<void>;
	on?: (event: "before_agent_start", handler: (event: BeforeAgentStartEvent) => Promise<{ systemPrompt: string } | void> | { systemPrompt: string } | void) => void;
};

export type PonytailMode = "off" | "lite" | "full" | "ultra";
export type CavemanMode = "off" | "lite" | "full" | "ultra" | "wenyan-lite" | "wenyan-full" | "wenyan-ultra";
export type StyleState = { version: 1; ponytail: PonytailMode; caveman: CavemanMode; updatedAt: string };

const STYLE_STATUS_CUSTOM_TYPE = "aoc.style.status";
const DEFAULT_STYLE_STATE: StyleState = { version: 1, ponytail: "off", caveman: "off", updatedAt: "" };

const PONYTAIL_MODE_COMPLETIONS: AutocompleteItem[] = [
	{ value: "off", label: "off", description: "Disable the Ponytail host hook." },
	{ value: "lite", label: "lite", description: "Enable the Ponytail host hook in lite mode." },
	{ value: "full", label: "full", description: "Enable the Ponytail host hook in full mode." },
	{ value: "ultra", label: "ultra", description: "Enable the Ponytail host hook in ultra mode." },
	{ value: "status", label: "status", description: "Report the active style host hook state." },
];

const CAVEMAN_MODE_COMPLETIONS: AutocompleteItem[] = [
	{ value: "off", label: "off", description: "Disable the Caveman host hook." },
	{ value: "lite", label: "lite", description: "Enable the Caveman host hook in lite mode." },
	{ value: "full", label: "full", description: "Enable the Caveman host hook in full mode." },
	{ value: "ultra", label: "ultra", description: "Enable the Caveman host hook in ultra mode." },
	{ value: "wenyan-lite", label: "wenyan-lite", description: "Enable the Caveman host hook in wenyan-lite mode." },
	{ value: "wenyan-full", label: "wenyan-full", description: "Enable the Caveman host hook in wenyan-full mode." },
	{ value: "wenyan-ultra", label: "wenyan-ultra", description: "Enable the Caveman host hook in wenyan-ultra mode." },
	{ value: "status", label: "status", description: "Report the active style host hook state." },
];

function argsText(args: string | string[] | undefined): string {
	if (Array.isArray(args)) return args.join(" ").trim();
	return (args ?? "").trim();
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isPonytailMode(value: unknown): value is PonytailMode {
	return value === "off" || value === "lite" || value === "full" || value === "ultra";
}

function isCavemanMode(value: unknown): value is CavemanMode {
	return value === "off" || value === "lite" || value === "full" || value === "ultra" || value === "wenyan-lite" || value === "wenyan-full" || value === "wenyan-ultra";
}

export function styleStatePath(): string {
	const configured = process.env.AOC_STYLE_STATE_FILE?.trim();
	if (configured) return configured;
	return path.join(os.homedir(), ".omp", "agent", "style-hooks.json");
}

export function readStyleState(): StyleState {
	let parsed: unknown;
	try {
		parsed = JSON.parse(fs.readFileSync(styleStatePath(), "utf8"));
	} catch {
		return DEFAULT_STYLE_STATE;
	}
	if (!isRecord(parsed)) return DEFAULT_STYLE_STATE;
	if (parsed.version !== 1) return DEFAULT_STYLE_STATE;
	if (!isPonytailMode(parsed.ponytail)) return DEFAULT_STYLE_STATE;
	if (!isCavemanMode(parsed.caveman)) return DEFAULT_STYLE_STATE;
	if (typeof parsed.updatedAt !== "string") return DEFAULT_STYLE_STATE;
	return { version: 1, ponytail: parsed.ponytail, caveman: parsed.caveman, updatedAt: parsed.updatedAt };
}

export function writeStyleState(next: StyleState): void {
	const file = styleStatePath();
	fs.mkdirSync(path.dirname(file), { recursive: true });
	const tmp = `${file}.tmp-${process.pid}`;
	let fd: number | null = null;
	try {
		fd = fs.openSync(tmp, "w");
		fs.writeFileSync(fd, JSON.stringify(next, null, 2));
		fs.closeSync(fd);
		fd = null;
		fs.renameSync(tmp, file);
	} catch (error) {
		if (fd !== null) fs.closeSync(fd);
		try {
			fs.unlinkSync(tmp);
		} catch {
			// best-effort cleanup
		}
		throw error;
	}
}

export function renderStyleHookPrompt(state: StyleState): string {
	const lines = ["# AOC Host Style Hooks"];
	if (state.ponytail !== "off") {
		lines.push(
			`- Ponytail engineering mode: ${state.ponytail}.`,
			"- Correctness first; choose boring minimal implementation; avoid new abstractions unless existing code proves need.",
			"- Delete obsolete code instead of adding compatibility shims; prefer source fixes over warning suppression.",
		);
	}
	if (state.caveman !== "off") {
		lines.push(
			`- Caveman output mode: ${state.caveman}.`,
			"- Compress prose; drop filler, pleasantries, and hedging; preserve user's language.",
			"- Preserve code symbols, paths, API names, CLI commands, commit keywords, and exact error strings verbatim.",
			"- Use clear normal prose for security warnings, irreversible/destructive actions, and ordered steps where compression could create ambiguity.",
			"- Never announce caveman style unless user asks what mode is active.",
		);
		if (state.caveman === "ultra") lines.push("- Ultra: use arrows and common prose abbreviations only when they cannot alter technical meaning.");
		if (state.caveman.startsWith("wenyan-")) lines.push("- Wenyan modes: use concise classical Chinese style only when the user's dominant language is Chinese; otherwise keep the user's language and apply equivalent compression.");
	}
	return lines.join("\n");
}

function filteredCompletions(prefix: string, items: AutocompleteItem[]): AutocompleteItem[] {
	const query = prefix.trim().toLowerCase();
	if (!query) return items;
	return items.filter((item) => item.value.startsWith(query) || item.label?.toLowerCase().startsWith(query));
}

function parsePonytailMode(args: string | string[] | undefined): PonytailMode | "status" {
	const text = argsText(args);
	const [rawMode = "status"] = text.split(/\s+/).filter(Boolean);
	const mode = rawMode.toLowerCase();
	if (mode === "off" || mode === "lite" || mode === "full" || mode === "ultra" || mode === "status") return mode;
	throw new Error("Unknown /ponytail mode. Use one of: off, lite, full, ultra, status.");
}

function parseCavemanMode(args: string | string[] | undefined): CavemanMode | "status" {
	const text = argsText(args);
	const [rawMode = "status"] = text.split(/\s+/).filter(Boolean);
	const mode = rawMode.toLowerCase();
	if (mode === "off" || mode === "lite" || mode === "full" || mode === "ultra" || mode === "wenyan-lite" || mode === "wenyan-full" || mode === "wenyan-ultra" || mode === "status") return mode;
	throw new Error("Unknown /caveman mode. Use one of: off, lite, full, ultra, wenyan-lite, wenyan-full, wenyan-ultra, status.");
}

function statusMessage(state: StyleState): string {
	return `Ponytail: ${state.ponytail}; Caveman: ${state.caveman}. State: ${styleStatePath()}.`;
}

async function notify(ctx: CommandContext, message: string): Promise<void> {
	await ctx.ui?.notify?.(message, "info");
}

function renderNamedPrompt(skillName: string, purpose: string, target: string): string {
	const suffix = target ? `\n\nTarget supplied by user: ${target}` : "";
	return `Use the ${skillName} skill.

${purpose}${suffix}

This slash command only hands the request to the agent. Do not mutate files, configuration, memory, or project state because of the command itself; only make changes if the user's actual task asks for them.`;
}

async function send(pi: ExtensionAPI, ctx: CommandContext, content: string, details: Record<string, unknown>): Promise<void> {
	if (typeof pi.sendMessage === "function") {
		await pi.sendMessage({ customType: "ponytail", display: true, content, details }, { triggerTurn: true });
		return;
	}
	await ctx.ui?.notify?.(content, "info");
}

export default function aocStyleExtension(pi: ExtensionAPI): void {
	pi.registerCommand("ponytail", {
		description: "Usage: /ponytail [off|lite|full|ultra|status]. Set Ponytail host hook state.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => filteredCompletions(prefix, PONYTAIL_MODE_COMPLETIONS),
		handler: async (args, ctx) => {
			const mode = parsePonytailMode(args);
			if (mode === "status") {
				await notify(ctx, statusMessage(readStyleState()));
				return;
			}
			const current = readStyleState();
			writeStyleState({ ...current, ponytail: mode, updatedAt: new Date().toISOString() });
			await notify(ctx, `Ponytail host hook set to ${mode}.`);
		},
	});

	pi.registerCommand("caveman", {
		description: "Usage: /caveman [off|lite|full|ultra|wenyan-lite|wenyan-full|wenyan-ultra|status]. Set Caveman host hook state.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => filteredCompletions(prefix, CAVEMAN_MODE_COMPLETIONS),
		handler: async (args, ctx) => {
			const mode = parseCavemanMode(args);
			if (mode === "status") {
				await notify(ctx, statusMessage(readStyleState()));
				return;
			}
			const current = readStyleState();
			writeStyleState({ ...current, caveman: mode, updatedAt: new Date().toISOString() });
			await notify(ctx, `Caveman host hook set to ${mode}.`);
		},
	});

	pi.registerCommand("ponytail-review", {
		description: "Ask the agent to use the ponytail-review skill for review-focused work.",
		handler: async (args, ctx) => {
			const target = argsText(args);
			await send(
				pi,
				ctx,
				renderNamedPrompt("ponytail-review", "Review the current task or supplied target with Ponytail's practical code-review posture. Prefer concise findings, concrete risk, and maintainable fixes.", target),
				{ command: "ponytail-review", skill: "ponytail-review", cwd: ctx.cwd, target: target || null },
			);
		},
	});

	pi.registerCommand("ponytail-audit", {
		description: "Ask the agent to use the ponytail-audit skill for broader audit work.",
		handler: async (args, ctx) => {
			const target = argsText(args);
			await send(
				pi,
				ctx,
				renderNamedPrompt("ponytail-audit", "Audit the current task or supplied target with Ponytail's engineering-risk posture. Focus on correctness, hidden coupling, edge cases, and operational hazards.", target),
				{ command: "ponytail-audit", skill: "ponytail-audit", cwd: ctx.cwd, target: target || null },
			);
		},
	});

	pi.registerCommand("ponytail-debt", {
		description: "Ask the agent to use the ponytail-debt skill for technical-debt analysis.",
		handler: async (args, ctx) => {
			const target = argsText(args);
			await send(
				pi,
				ctx,
				renderNamedPrompt("ponytail-debt", "Analyze technical debt in the current task or supplied target with Ponytail's bias for boring, removable complexity and source-level fixes.", target),
				{ command: "ponytail-debt", skill: "ponytail-debt", cwd: ctx.cwd, target: target || null },
			);
		},
	});

	pi.registerCommand("ponytail-help", {
		description: "Ask the agent to use the ponytail-help skill for Ponytail command guidance.",
		handler: async (args, ctx) => {
			const target = argsText(args);
			await send(
				pi,
				ctx,
				renderNamedPrompt("ponytail-help", "Explain the available Ponytail slash commands and when to use host-hook modes, workflow review, audit, and debt commands.", target),
				{ command: "ponytail-help", skill: "ponytail-help", cwd: ctx.cwd, target: target || null },
			);
		},
	});

	pi.on?.("before_agent_start", async (event) => {
		const state = readStyleState();
		if (state.ponytail === "off" && state.caveman === "off") return;
		return { systemPrompt: `${event.systemPrompt}\n\n${renderStyleHookPrompt(state)}` };
	});
}
