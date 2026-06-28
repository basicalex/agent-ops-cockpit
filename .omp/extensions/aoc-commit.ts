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

function argsText(args: string | string[] | undefined): string {
	if (Array.isArray(args)) return args.join(" ").trim();
	return (args ?? "").trim();
}

function renderCommitPrompt(scope: string): string {
	const target = scope || "the current completed work";
	return `Run the AOC/OMP commit workflow for this prompt-first intent: ${target}

The user's /commit invocation is approval to run the Git commit flow directly: inspect, select a safe atomic change set, commit, and report the result. Treat the text after /commit as the primary commit intent/scope; use recent session edits and related dirty work only when they support that intent. Never push unless explicitly requested.

Workflow:

1. Inspect read-only Git state
- Run \`git status --short\`.
- Run \`git diff --stat\`.
- Run \`git diff --cached --stat\`.
- Inspect targeted diffs for candidate files only.
- Identify unrelated/pre-existing changes and exclude them before committing, even if they were edited in the same working tree.

2. Resolve provenance
- Identify relevant task/subtask/spec from recent implementation context, Taskmaster, or explicit user instructions.
- Use tm/aoc-task only when it materially improves commit provenance.
- Do not use broad Mind/STM recall unless a focused missing-provenance question requires it.

3. Plan atomic commit(s)
- Start prompt-first: infer the intended commit slice from the /commit arguments; if they are empty, use the current completed work from the session.
- Include only files that are part of that coherent intent: directly edited session files plus related dirty work needed for correctness.
- Group by intent, not by timestamp.
- Prefer one commit for one coherent implementation slice.
- Stage only explicit paths with \`git add -- <paths>\`; never stage broad paths like \`.\`.
- If the intended slice is unclear after inspecting the prompt, session context, and targeted diffs, ask one concise clarification before mutating.

4. Draft commit message
Use:

<type>(<scope>): <imperative summary>

Include a concise body plus trailers when known:

AOC-Task: <id>
AOC-Subtask: <id.n>
AOC-PRD: <path>
AOC-Intent: <durable intent>
Tests: <commands run/results>
Risk: low|medium|high; <reason>

5. Validate, commit directly, then refresh CodeGraph cache
- Run targeted validation appropriate to the selected files when practical.
- Stage only explicit paths with \`git add -- path ...\`, commit, and report the observed SHA.
- If no safe atomic set can be inferred from the prompt, session context, and targeted diffs, ask one concise clarification before staging or mutating.
- Never push unless explicitly requested.
- After a successful commit only, if \`.codegraph/\` exists and \`codegraph\` is on PATH, run \`codegraph sync <repo-root>\` as best-effort cache maintenance.
- Never run CodeGraph sync before the commit, never include \`.codegraph/**\` in the commit, and never let sync failure change, undo, block, or invalidate the committed VCS result.
- If sync fails or is unavailable, report it as advisory cache status and continue the final commit report.

Final response after commit:
- Git commit SHA
- subject
- files committed
- tests noted
- CodeGraph cache status: synced | skipped (no index/CLI) | failed (advisory; reason)
- remaining unrelated changes, if any

Safety:
- Treat /commit as approval to commit only the safe atomic change set inferred from the prompt, recent session context, and targeted diffs.
- Never commit secrets/tokens/private logs.
- Never push without explicit push approval.
- Do not include raw chain-of-thought or huge diffs in commit messages.`;
}

export default function aocCommitExtension(pi: ExtensionAPI): void {
	pi.registerCommand("commit", {
		description: "Usage: /commit [intent]. Commit only the prompt-selected atomic slice using Git.",
		getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => {
			const examples = [
				"commit only the prompt-first /commit workflow updates",
				"commit the docs update, leave implementation changes uncommitted",
				"commit the bug fix and related test only",
			];
			const trimmed = prefix.trim().toLowerCase();
			const matches = examples
				.filter(example => !trimmed || example.toLowerCase().includes(trimmed))
				.map(example => ({ value: example, label: example }));
			return matches.length > 0 ? matches : null;
		},
		handler: async (args, ctx) => {
			const scope = argsText(args);
			const content = renderCommitPrompt(scope);
			if (typeof pi.sendMessage === "function") {
				await pi.sendMessage(
					{
						customType: "aoc.commit.request",
						display: true,
						content,
						details: { scope, cwd: ctx.cwd },
					},
					{ triggerTurn: true },
				);
				return;
			}
			await ctx.ui?.notify?.(content, "info");
		},
	});
}
