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

type ExtensionAPI = {
	registerCommand: (name: string, definition: CommandDefinition) => void;
	sendMessage?: (message: OutboundMessage, options?: SendOptions) => void | Promise<void>;
};

type PonytailMode = "lite" | "full" | "ultra" | "off" | "status" | "default";

type PonytailPrompt = {
	mode: PonytailMode;
	content: string;
};

const PONYTAIL_MODE_COMPLETIONS: AutocompleteItem[] = [
	{ value: "lite", label: "lite", description: "Use the ponytail skill lightly for concise code-review discipline." },
	{ value: "full", label: "full", description: "Use the ponytail skill as the active review posture for this thread." },
	{ value: "ultra", label: "ultra", description: "Use the ponytail skill with strict, high-scrutiny review posture." },
	{ value: "off", label: "off", description: "Stop using the ponytail skill for this thread." },
	{ value: "status", label: "status", description: "Report whether ponytail guidance is currently being used in this thread." },
	{ value: "default", label: "default", description: "Explain OMP default-mode limits and apply ponytail to this session." },
];

function argsText(args: string | string[] | undefined): string {
	if (Array.isArray(args)) return args.join(" ").trim();
	return (args ?? "").trim();
}

function parsePonytailMode(args: string | string[] | undefined): PonytailMode {
	const text = argsText(args);
	const [rawMode = "status"] = text.split(/\s+/).filter(Boolean);
	const mode = rawMode.toLowerCase();
	if (mode === "lite" || mode === "full" || mode === "ultra" || mode === "off" || mode === "status" || mode === "default") {
		return mode;
	}
	throw new Error("Unknown /ponytail mode. Use one of: lite, full, ultra, off, status, default.");
}

function renderPonytailPrompt(args: string | string[] | undefined): PonytailPrompt {
	const mode = parsePonytailMode(args);
	if (mode === "off") {
		return {
			mode,
			content: `Stop using the ponytail skill for the current thread.

Confirm briefly that ponytail guidance is disabled for this thread. Do not mutate files, configuration, memory, or project state because of this slash command.`,
		};
	}
	if (mode === "status") {
		return {
			mode,
			content: `Report the current ponytail posture for this thread.

If there is no prior explicit /ponytail instruction in this thread, say that no ponytail mode is active yet. Do not mutate files, configuration, memory, or project state because of this slash command.`,
		};
	}
	if (mode === "default") {
		return {
			mode,
			content: `Use the ponytail skill for the current session.

OMP cannot persist a global ponytail default unless the host adds storage or a system-prompt lifecycle hook. Treat this as a current-session instruction only, and explain that future threads need another /ponytail command unless the host implements persistence. Do not mutate files, configuration, memory, or project state because of this slash command.`,
		};
	}
	return {
		mode,
		content: `Use the ponytail skill in ${mode} mode for the current thread.

Treat this as an instruction prompt only: apply the ponytail review posture during agent reasoning and responses in this thread. Do not mutate files, configuration, memory, or project state because of this slash command.`,
	};
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

export default function ponytailExtension(pi: ExtensionAPI): void {
	pi.registerCommand("ponytail", {
		description: "Usage: /ponytail [lite|full|ultra|off|status|default]. Set ponytail posture for the current OMP thread.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => {
			const query = prefix.trim().toLowerCase();
			if (!query) return PONYTAIL_MODE_COMPLETIONS;
			return PONYTAIL_MODE_COMPLETIONS.filter((item) => item.value.startsWith(query) || item.label?.toLowerCase().startsWith(query));
		},
		handler: async (args, ctx) => {
			const prompt = renderPonytailPrompt(args);
			await send(pi, ctx, prompt.content, { command: "ponytail", mode: prompt.mode, cwd: ctx.cwd });
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
				renderNamedPrompt("ponytail-help", "Explain the available Ponytail slash commands and when to use lite, full, ultra, off, status, default, review, audit, and debt modes.", target),
				{ command: "ponytail-help", skill: "ponytail-help", cwd: ctx.cwd, target: target || null },
			);
		},
	});
}
