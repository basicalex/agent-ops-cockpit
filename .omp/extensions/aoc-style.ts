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
type PonytailCommand = PonytailMode | "status" | "review" | "audit" | "debt" | "help";
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
	{ value: "review", label: "review", description: "Run the diff-focused over-engineering review workflow." },
	{ value: "audit", label: "audit", description: "Run the repo or target-wide over-engineering audit workflow." },
	{ value: "debt", label: "debt", description: "Run the ponytail debt-ledger workflow." },
	{ value: "help", label: "help", description: "Show Ponytail commands and modes." },
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

const PONYTAIL_WORKFLOW_PURPOSE: Record<"review" | "audit" | "debt", string> = {
	review: "Run Ponytail's diff-focused over-engineering review workflow. List only concrete delete, stdlib, native, yagni, or shrink findings; apply no edits.",
	audit: "Run Ponytail's repo or target-wide over-engineering audit workflow. Rank biggest cuts first; apply no edits.",
	debt: "Run Ponytail's debt-ledger workflow. Search for ponytail: markers, report ceiling and upgrade triggers, and apply no edits unless the user explicitly asks to persist a ledger.",
};

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

function parsePonytailMode(args: string | string[] | undefined): PonytailCommand {
	const text = argsText(args);
	const [rawMode = "status"] = text.split(/\s+/).filter(Boolean);
	const mode = rawMode.toLowerCase();
	if (mode === "off" || mode === "lite" || mode === "full" || mode === "ultra" || mode === "status" || mode === "review" || mode === "audit" || mode === "debt" || mode === "help") return mode;
	throw new Error("Unknown /ponytail mode. Use one of: off, lite, full, ultra, status, review, audit, debt, help.");
}

function ponytailTarget(args: string | string[] | undefined): string {
	const parts = argsText(args).split(/\s+/).filter(Boolean);
	return parts.slice(1).join(" ").trim();
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

function ponytailHelpCard(): string {
	return `Ponytail commands:
/ponytail status — report active Ponytail/Caveman hook state.
/ponytail off|lite|full|ultra — set persistent Ponytail host-hook mode.
/ponytail review [target] — run diff-focused over-engineering review through ponytail-workflows.
/ponytail audit [target] — run repo or target-wide over-engineering audit through ponytail-workflows.
/ponytail debt [target] — run ponytail: marker debt ledger through ponytail-workflows.
/ponytail help — show this card.`;
}

export default function aocStyleExtension(pi: ExtensionAPI): void {
	void STYLE_STATUS_CUSTOM_TYPE;
	pi.registerCommand("ponytail", {
		description: "Usage: /ponytail [off|lite|full|ultra|status|review|audit|debt|help] [target]. Set Ponytail host hook state or run Ponytail workflows.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => filteredCompletions(prefix, PONYTAIL_MODE_COMPLETIONS),
		handler: async (args, ctx) => {
			const mode = parsePonytailMode(args);
			if (mode === "status") {
				await notify(ctx, statusMessage(readStyleState()));
				return;
			}
			if (mode === "help") {
				await notify(ctx, ponytailHelpCard());
				return;
			}
			if (mode === "review" || mode === "audit" || mode === "debt") {
				const target = ponytailTarget(args);
				await send(pi, ctx, renderNamedPrompt("ponytail-workflows", PONYTAIL_WORKFLOW_PURPOSE[mode], target), {
					command: "ponytail",
					mode,
					skill: "ponytail-workflows",
					cwd: ctx.cwd,
					target: target || null,
				});
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

	pi.on?.("before_agent_start", async (event) => {
		const state = readStyleState();
		if (state.ponytail === "off" && state.caveman === "off") return;
		return { systemPrompt: `${event.systemPrompt}\n\n${renderStyleHookPrompt(state)}` };
	});
}
