import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import * as path from "node:path";

type CommandContext = {
	cwd?: string;
	ui?: {
		notify?: (message: string, level?: "info" | "warning" | "error") => void | Promise<void>;
	};
};

type CommandDefinition = {
	description: string;
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

const MODE_ALIASES: Record<string, string> = {
	strategy: "strategy",
	concepts: "concepts",
	concept: "concepts",
	image: "image",
	images: "image",
	review: "image-review",
	"image-review": "image-review",
	svg: "svg",
	campaign: "campaign",
};

const FALLBACK_MODE_COMPONENTS: Record<string, string[]> = {
	strategy: ["core", "mode-strategy"],
	concepts: ["core", "mode-concepts"],
	image: ["core", "mode-image"],
	"image-review": ["core", "mode-image-review"],
	svg: ["core", "mode-svg"],
	campaign: ["core", "mode-campaign"],
};

function argsText(args: string | string[] | undefined): string {
	if (Array.isArray(args)) return args.join(" ").trim();
	return (args ?? "").trim();
}

function findProjectRoot(start: string | undefined): string {
	let current = path.resolve(start || process.cwd());
	for (;;) {
		if (existsSync(path.join(current, ".aoc")) || existsSync(path.join(current, ".git"))) return current;
		const parent = path.dirname(current);
		if (parent === current) return path.resolve(start || process.cwd());
		current = parent;
	}
}

function normalizeMode(input: string): string {
	const raw = input.trim().split(/\s+/, 1)[0]?.toLowerCase() ?? "";
	const mode = MODE_ALIASES[raw];
	if (!mode) {
		throw new Error(`Unknown brand content mode '${raw || "<missing>"}'. Use one of: strategy, concepts, image, review, svg, campaign.`);
	}
	return mode;
}

async function readTextIfExists(file: string): Promise<string | null> {
	try {
		return await readFile(file, "utf8");
	} catch {
		return null;
	}
}

function parseModeComponents(toml: string): Record<string, string[]> {
	const result: Record<string, string[]> = {};
	let inModes = false;
	for (const rawLine of toml.split(/\r?\n/)) {
		const line = rawLine.trim();
		if (!line || line.startsWith("#")) continue;
		if (line.startsWith("[")) {
			inModes = line === "[components.modes]";
			continue;
		}
		if (!inModes) continue;
		const match = /^([A-Za-z0-9_-]+)\s*=\s*\[(.*)]\s*$/.exec(line);
		if (!match) continue;
		const components: string[] = [];
		for (const componentMatch of match[2].matchAll(/"([^"]+)"/g)) {
			components.push(componentMatch[1]);
		}
		if (components.length > 0) result[match[1]] = components;
	}
	return result;
}

async function loadPromptComponents(projectRoot: string, mode: string): Promise<{ presetPath: string; componentNames: string[]; content: string }> {
	const presetPath = path.join(projectRoot, ".aoc", "presets", "hyperframes", "preset.toml");
	const componentsDir = path.join(projectRoot, ".aoc", "presets", "hyperframes", "components");
	const presetText = await readTextIfExists(presetPath);
	const parsedModes = presetText ? parseModeComponents(presetText) : {};
	const componentNames = parsedModes[mode] ?? FALLBACK_MODE_COMPONENTS[mode];
	if (!componentNames) throw new Error(`No prompt components configured for mode '${mode}'.`);

	const chunks: string[] = [];
	for (const name of componentNames) {
		const file = path.join(componentsDir, `${name}.md`);
		const text = await readTextIfExists(file);
		if (!text) throw new Error(`Missing HyperFrames prompt component: .aoc/presets/hyperframes/components/${name}.md`);
		chunks.push(`<!-- component: ${name} -->\n${text.trim()}`);
	}

	return { presetPath, componentNames, content: chunks.join("\n\n") };
}

function renderModePrompt(mode: string, loaded: { presetPath: string; componentNames: string[]; content: string }, projectRoot: string): string {
	return `Run AOC HyperFrames branded content mode: ${mode}

Loaded prompt components from ${path.relative(projectRoot, loaded.presetPath) || ".aoc/presets/hyperframes/preset.toml"}:
${loaded.componentNames.map((name) => `- ${name}`).join("\n")}

${loaded.content}

Brand-content operating contract:
- Active runtime is OMP. Do not use Pi subagent controls or .pi/extensions/aoc-presets for this workflow.
- Keep the operator approval boundary intact: strategy -> concepts -> image prompts -> image review/regions -> SVG specs -> campaign assembly.
- Use GPT Image 2 as a prompt/artifact workflow: write prompt packs and asset paths; do not claim generation happened unless files exist.
- Use specialist OMP agents only for exact specs/code and target paths unless the operator explicitly approves direct writes.
- Prefer html-video for content-graph/storyboard/template/studio/render flows; use direct HyperFrames HTML/GSAP when custom motion is required.
- Relevant commands: aoc-hyperframes brand init, brand check --no-lint, brand board --write, brand campaign, check --no-lint, catalog --write.

Proceed with the requested ${mode} work using the current project files as source of truth.`;
}

function registerBrandCommand(pi: ExtensionAPI, name: string, description: string): void {
	pi.registerCommand(name, {
		description,
		handler: async (args, ctx) => {
			try {
				const text = argsText(args);
				const mode = normalizeMode(text);
				const projectRoot = findProjectRoot(ctx.cwd);
				const loaded = await loadPromptComponents(projectRoot, mode);
				const content = renderModePrompt(mode, loaded, projectRoot);
				if (typeof pi.sendMessage === "function") {
					await pi.sendMessage(
						{
							customType: "aoc.brand-content.request",
							display: true,
							content,
							details: { mode, command: name, cwd: ctx.cwd, projectRoot, components: loaded.componentNames },
						},
						{ triggerTurn: true },
					);
					return;
				}
				await ctx.ui?.notify?.(content, "info");
			} catch (err) {
				const message = err instanceof Error ? err.message : String(err);
				await ctx.ui?.notify?.(message, "error");
				throw err;
			}
		},
	});
}

export default function aocBrandContentExtension(pi: ExtensionAPI): void {
	registerBrandCommand(pi, "brand-content", "Load an AOC branded-content pipeline mode into the active OMP turn.");
	registerBrandCommand(pi, "hyperframes-director", "Load an AOC HyperFrames director mode into the active OMP turn.");
}
