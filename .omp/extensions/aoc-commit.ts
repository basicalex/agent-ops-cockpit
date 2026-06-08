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

The user's /commit invocation is approval to run the full VCS-aware commit flow directly: inspect, select a safe atomic change set, commit with the detected repository workflow, and report the result. Never push unless explicitly requested.

Workflow:

1. Detect VCS mode, then inspect read-only state
- Prefer startup context VCS metadata. If it is unavailable or stale, run \`aoc-handshake --json\`.
- For Jujutsu repositories, run narrow summaries:
  - jj status
  - jj diff --summary
  - jj diff --stat
  - targeted jj diff -- <filesets> when needed
- For Git-only repositories, run narrow summaries:
  - git status --short
  - git diff --stat
  - git diff --cached --stat
  - targeted diffs for candidate files only
- Identify unrelated/pre-existing changes and exclude or split them before committing.

2. Resolve provenance
- Identify relevant task/subtask/spec from recent implementation context, Taskmaster, or explicit user instructions.
- Use tm/aoc-task only when it materially improves commit provenance.
- Do not use broad Mind/STM recall unless a focused missing-provenance question requires it.

3. Plan atomic commit(s)
- Group by intent, not by timestamp.
- Prefer one commit for one coherent implementation slice.
- Git uses explicit staging; never stage broad paths like .
- Jujutsu has no Git staging area: the working copy is the current mutable @ change, and jj commit without filesets selects all current changes.
- If Jujutsu @ is mixed, split unrelated work first with jj split / jj commit <filesets> / jj squash -i as appropriate; if the intended split is unclear, ask one concise clarification before mutating.

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
- Git-only: stage only explicit paths with git add -- path ..., commit, and report the observed SHA.
- Jujutsu: verify @ contains only the intended atomic work, then use jj commit -m <message> or jj describe -m <message> plus the workflow-appropriate new-change step.
- Jujutsu selected filesets: when the intended fileset is clear but @ is mixed, use jj commit -m <message> <filesets> or jj split <filesets> according to the desired split direction.
- If no safe atomic set can be inferred, ask one concise clarification before staging or mutating.
- Never push unless explicitly requested.

Final response after commit:
- Git commit SHA or Jujutsu change/commit identity
- subject
- files committed
- tests noted
- remaining unrelated changes, if any

Safety:
- Never commit secrets/tokens/private logs.
- Never stage broad Git paths or include unrelated/pre-existing changes.
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
