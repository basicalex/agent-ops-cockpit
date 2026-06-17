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

const DOX_COMMAND_COMPLETIONS: AutocompleteItem[] = [
	{ value: "full", label: "full", description: "Map, scout, map contracts, critic review, writer dry-run." },
	{ value: "scout", label: "scout", description: "Map metadata and launch dox-scout for target paths." },
	{ value: "map", label: "map", description: "Run aoc_dox map and summarize metadata." },
	{ value: "review", label: "review", description: "Review current .aoc/dox candidate decisions." },
	{ value: "packet", label: "packet", description: "Render/write the DOX review packet for editor review." },
	{ value: "doctor", label: "doctor", description: "Validate DOX metadata and AGENTS chain health." },
	{ value: "dry-run", label: "dry-run", description: "Run safe apply dry-run only." },
];

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
		packet: "packet",
		"review-packet": "packet",
		doctor: "doctor",
		"dry-run": "dry-run",
		dryrun: "dry-run",
		apply: "dry-run",
	};
	const mode = aliases[rawMode.toLowerCase()];
	if (!mode) {
		throw new Error("Unknown /dox mode. Use one of: full, scout, map, review, packet, doctor, dry-run.");
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

Run aoc_dox with action=review and json=true, then run aoc_dox with action=review-packet and writePacket=true when create/update candidates exist. Summarize create/update/reject decisions, budget status, .aoc/dox/review.md when written, and next safe operator action. Do not write AGENTS.md files and do not run apply --yes.

${targetLine}`,
		};
	}
	if (mode === "packet") {
		return {
			mode,
			target,
			content: `Use the aoc-dox-cartography skill.

Run aoc_dox with action=review-packet, json=false, and writePacket=true. Summarize the proposed AGENTS.md routes, rejected routes, .aoc/dox/review.md path, and the exact manual apply command. Do not write AGENTS.md files and do not run apply --yes.

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

Run aoc_dox with action=apply-dry-run and json=true. Report target paths/bytes and point to \`.aoc/dox/review.md\` if present; if missing, run review-packet first. Do not write AGENTS.md files and do not run apply --yes.

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
7. Run aoc_dox action=review-packet with writePacket=true and report .aoc/dox/review.md plus a concise route summary.
8. Finish by running aoc_dox action=apply-dry-run, then aoc_dox action=doctor.

Never run aoc dox apply --yes from this command. Do not create or edit AGENTS.md directly.

${targetLine}`,
	};
}

export default function aocDoxCommandExtension(pi: ExtensionAPI): void {
	pi.registerCommand("dox", {
		description: "Usage: /dox [full|scout|map|review|packet|doctor|dry-run] [path]. Run safe AOC DOX cartography with dox-* agents.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => {
			const query = prefix.trim().toLowerCase();
			if (!query) return DOX_COMMAND_COMPLETIONS;
			return DOX_COMMAND_COMPLETIONS.filter((item) => item.value.startsWith(query) || item.label?.toLowerCase().startsWith(query));
		},
		handler: async (args, ctx) => {
			try {
				const prompt = renderDoxCommandPrompt(args);
				if (typeof pi.sendMessage === "function") {
					await pi.sendMessage(
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
