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

function argsText(args: string | string[] | undefined): string {
	if (Array.isArray(args)) return args.join(" ").trim();
	return (args ?? "").trim();
}

function renderCommitPrompt(scope: string): string {
	const target = scope || "the current completed work";
	return `Run the AOC/OMP commit workflow for: ${target}

The user's /commit invocation is approval to run the full commit flow directly: inspect, select a safe atomic file set, stage exact paths, commit, and report the result. Never push unless explicitly requested.

Workflow:

1. Inspect read-only state
- Run narrow Git summaries:
  - git status --short
  - git diff --stat
  - git diff --cached --stat
- Inspect targeted diffs for candidate files only.
- Identify unrelated/pre-existing changes and exclude them from the plan.

2. Resolve provenance
- Identify relevant task/subtask/spec from recent implementation context, Taskmaster, or explicit user instructions.
- Use tm/aoc-task only when it materially improves commit provenance.
- Do not use broad Mind/STM recall unless a focused missing-provenance question requires it.

3. Plan atomic commit(s)
- Group by intent, not by timestamp.
- Prefer one commit for one coherent implementation slice.
- If changes are mixed, commit only the safe inferred atomic set or ask for the missing split decision.
- Never stage broad paths like .

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

5. Validate and commit directly
- Run targeted validation appropriate to the selected files when practical.
- Stage only explicit paths with git add -- path ...
- Commit with the drafted provenance-rich message.
- If no safe atomic file set can be inferred, ask one concise clarification before staging.
- Never push unless explicitly requested.

Final response after commit:
- commit SHA
- subject
- files committed
- tests noted
- remaining unrelated changes, if any

Safety:
- Never commit secrets/tokens/private logs.
- Never stage broad paths or unrelated/pre-existing changes.
- Never push without explicit push approval.
- Do not include raw chain-of-thought or huge diffs in commit messages.`;
}

export default function aocCommitExtension(pi: ExtensionAPI): void {
	pi.registerCommand("commit", {
		description: "Run the AOC/OMP safe atomic commit workflow for the current work.",
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
