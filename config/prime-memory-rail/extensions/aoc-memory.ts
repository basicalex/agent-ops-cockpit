/**
 * aoc-memory — shared agent-memory rail (aoc Level 2).
 *
 * before_agent_start: appends the repo's shared memory index
 * (~/.aoc/memory/<repo>/MEMORY.md) to the system prompt so the index is in
 * context every session, without relying on the model obeying a "read this
 * file first" instruction.
 *
 * refine_complete: validates the shared store after each refine (auto or
 * manual) and mechanically regenerates missing index lines from file
 * frontmatter. Discrepancies it cannot repair are surfaced as notifications.
 *
 * Runtime dependencies are node builtins only, so the validator can also be
 * imported directly for unit-style testing:
 *   bun -e 'import("./aoc-memory.ts").then(m => console.log(m.validateAndRepairStore("<dir>")))'
 *
 * Managed by agent-ops-cockpit (config/prime-memory-rail); installed by
 * bin/aoc-prime-memory-install. Edit in the repo, not in ~/.prime/agent/.
 */
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

const MEMORY_ROOT = join(homedir(), ".aoc", "memory");
const HEADING = "# Shared memory index";

function gitRoot(cwd: string): string | null {
	try {
		return execFileSync("git", ["-C", cwd, "rev-parse", "--show-toplevel"], {
			stdio: ["ignore", "pipe", "ignore"],
			encoding: "utf8",
		}).trim();
	} catch {
		return null;
	}
}

export function storeDirFor(cwd: string): { repo: string; dir: string } {
	const repo = basename(gitRoot(cwd) ?? cwd);
	return { repo, dir: join(MEMORY_ROOT, repo) };
}

interface Frontmatter {
	name?: string;
	description?: string;
	author?: string;
}

/** Minimal frontmatter reader for the memory-file contract (no YAML dep). */
export function parseFrontmatter(content: string): Frontmatter | null {
	const match = content.match(/^---\r?\n([\s\S]*?)\r?\n---(\r?\n|$)/);
	if (!match) return null;
	const body = match[1];
	const field = (key: string): string | undefined =>
		body.match(new RegExp(`^\\s*${key}:\\s*(.+?)\\s*$`, "m"))?.[1];
	return { name: field("name"), description: field("description"), author: field("author") };
}

export interface StoreReport {
	problems: string[];
	repaired: string[];
}

/**
 * Validate ~/.aoc/memory/<repo>/ against its MEMORY.md index:
 * - every memory file has an index line (missing lines are regenerated from
 *   frontmatter name/description, with a "(prime)" suffix for prime-authored files)
 * - every index line points at an existing file
 * - frontmatter parses and carries name + description
 * - "(prime)" index annotations and `author: prime` frontmatter agree
 */
export function validateAndRepairStore(dir: string): StoreReport {
	const problems: string[] = [];
	const repaired: string[] = [];
	const indexPath = join(dir, "MEMORY.md");
	const files = readdirSync(dir).filter((f) => f.endsWith(".md") && f !== "MEMORY.md");
	let indexText = existsSync(indexPath) ? readFileSync(indexPath, "utf8") : "";

	const indexedFiles = new Map<string, string>(); // filename -> index line
	for (const line of indexText.split("\n")) {
		const m = line.match(/^\s*-\s*\[[^\]]*\]\(([^)]+\.md)\)/);
		if (m) indexedFiles.set(m[1], line);
	}

	for (const [file] of indexedFiles) {
		if (!existsSync(join(dir, file))) {
			problems.push(`index line references missing file: ${file}`);
		}
	}

	const additions: string[] = [];
	for (const file of files) {
		const fm = parseFrontmatter(readFileSync(join(dir, file), "utf8"));
		if (!fm || !fm.name || !fm.description) {
			problems.push(`frontmatter missing or incomplete (need name + description): ${file}`);
			continue;
		}
		const isPrime = fm.author === "prime";
		const line = indexedFiles.get(file);
		if (!line) {
			additions.push(`- [${fm.name}](${file}) — ${fm.description}${isPrime ? " (prime)" : ""}`);
			repaired.push(`regenerated index line for ${file}`);
			continue;
		}
		const flagged = /\(prime\)\s*$/.test(line);
		if (isPrime && !flagged) {
			problems.push(`${file} has author: prime but its index line lacks the (prime) suffix`);
		} else if (!isPrime && flagged) {
			problems.push(`${file} index line says (prime) but the file has no author: prime`);
		}
	}

	if (additions.length > 0) {
		if (indexText.length > 0 && !indexText.endsWith("\n")) indexText += "\n";
		if (indexText.length === 0) indexText = "# Memory index\n\n";
		writeFileSync(indexPath, indexText + additions.join("\n") + "\n");
	}

	return { problems, repaired };
}

export default function (pi: ExtensionAPI) {
	pi.on("before_agent_start", async (event) => {
		try {
			const { repo, dir } = storeDirFor(event.systemPromptOptions?.cwd ?? process.cwd());
			const indexPath = join(dir, "MEMORY.md");
			let block: string;
			if (existsSync(indexPath)) {
				block =
					`${HEADING}\n\n` +
					`Shared store: ~/.aoc/memory/${repo}/ — read a memory file only when its index line is relevant to the task.\n\n` +
					readFileSync(indexPath, "utf8").trim();
			} else {
				block =
					`${HEADING}\n\n` +
					`No shared memories for this repo yet (store: ~/.aoc/memory/${repo}/); create the store on your first durable write per the shared memory contract.`;
			}
			return { systemPrompt: `${event.systemPrompt}\n\n${block}` };
		} catch (error) {
			// Never block session start on a rail problem; surface and continue.
			return undefined;
		}
	});

	pi.on("refine_complete", async (_event, ctx) => {
		try {
			const { repo, dir } = storeDirFor(process.cwd());
			if (!existsSync(dir)) return;
			const report = validateAndRepairStore(dir);
			if (report.repaired.length > 0) {
				ctx.ui.notify(`aoc-memory: ${report.repaired.join("; ")} (${repo})`, "info");
			}
			if (report.problems.length > 0) {
				ctx.ui.notify(`aoc-memory: store issues in ~/.aoc/memory/${repo}/: ${report.problems.join("; ")}`, "warning");
			}
		} catch (error) {
			ctx.ui.notify(`aoc-memory: store validation failed: ${String(error)}`, "warning");
		}
	});
}
