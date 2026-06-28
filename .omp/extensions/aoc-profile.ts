import { findProjectRoot, renderCommand, resolveRepoCommand, runBoundedCommand } from "./aoc-runtime";

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

type ExtensionAPI = {
	registerCommand: (name: string, definition: CommandDefinition) => void;
};

const PROFILE_MODES = ["list", "show", "enable", "disable", "set", "explain"] as const;
const PROFILE_COMPLETIONS: AutocompleteItem[] = [
	{ value: "list", label: "list", description: "List available capability profiles." },
	{ value: "show", label: "show", description: "Show active profiles and asset counts." },
	{ value: "enable", label: "enable", description: "Enable a profile for this project." },
	{ value: "disable", label: "disable", description: "Disable a profile for this project." },
	{ value: "set", label: "set", description: "Replace active profile state for this project." },
	{ value: "explain", label: "explain", description: "Explain a profile or asset." },
	{ value: "core", label: "core", description: "Default lean AOC OMP surface." },
	{ value: "operator", label: "operator", description: "Herdr/operator orchestration surface." },
	{ value: "dox", label: "dox", description: "DOX cartography surface." },
	{ value: "hyperframes", label: "hyperframes", description: "HyperFrames content surface." },
	{ value: "research", label: "research", description: "Local web-search surface." },
	{ value: "full", label: "full", description: "Full compatibility surface." },
];

function argsText(args: string | string[] | undefined): string {
	if (Array.isArray(args)) return args.join(" ").trim();
	return (args ?? "").trim();
}

function filteredCompletions(prefix: string): AutocompleteItem[] {
	const query = prefix.trim().toLowerCase();
	if (!query) return PROFILE_COMPLETIONS;
	return PROFILE_COMPLETIONS.filter((item) => item.value.startsWith(query) || item.label?.toLowerCase().startsWith(query));
}

function parseProfileCommand(args: string | string[] | undefined): { mode: string; value?: string } {
	const parts = argsText(args).split(/\s+/).filter(Boolean);
	const mode = (parts[0] ?? "show").toLowerCase();
	if (!PROFILE_MODES.includes(mode as (typeof PROFILE_MODES)[number])) {
		throw new Error("Unknown /profile mode. Use one of: list, show, enable, disable, set, explain.");
	}
	if ((mode === "enable" || mode === "disable" || mode === "set" || mode === "explain") && !parts[1]) {
		throw new Error(`Usage: /profile ${mode} <profile-or-asset>`);
	}
	return { mode, value: parts[1] };
}

export default function aocProfileExtension(pi: ExtensionAPI): void {
	pi.registerCommand("profile", {
		description: "Usage: /profile [list|show|enable|disable|set|explain] [profile-or-asset]. Inspect or update AOC OMP capability profiles.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => filteredCompletions(prefix),
		handler: async (args, ctx) => {
			const { mode, value } = parseProfileCommand(args);
			const projectRoot = findProjectRoot(ctx.cwd);
			const command = resolveRepoCommand(projectRoot, "bin/aoc-profile", "aoc-profile");
			const commandArgs = [mode];
			if (value) commandArgs.push(value);
			commandArgs.push("--root", projectRoot, "--manifest", `${projectRoot}/.omp/manifest.toml`);
			const result = await runBoundedCommand(command, commandArgs, {
				cwd: projectRoot,
				maxStdoutChars: 12000,
				maxStderrChars: 4000,
				timeoutMs: 10000,
				missingMessage: "aoc-profile command not found. Run aoc-init or install AOC bin assets.",
			});
			const lines = [renderCommand(command, commandArgs)];
			if (result.stdout.trim()) lines.push(result.stdout.trim());
			if (result.stderr.trim()) lines.push(result.stderr.trim());
			if (result.timedOut) lines.push("Command timed out.");
			if (result.truncated) lines.push("Output truncated.");
			if (result.ok && (mode === "enable" || mode === "disable" || mode === "set")) {
				lines.push("Run aoc-init or aoc-herdr-install to refresh installed OMP assets; restart OMP if the current process should see the changed extension list.");
			}
			await ctx.ui?.notify?.(lines.join("\n"), result.ok ? "info" : "error");
		},
	});
}
