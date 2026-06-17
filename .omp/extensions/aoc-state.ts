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

function renderStatusPrompt(scope: string): string {
	const target = scope || "this AOC project";
	return `Run the read-only AOC project-state audit for: ${target}

Workflow:
- Run \`aoc state status\`.
- If the router is unavailable, run \`aoc-state status\`.
- Do not commit, push, clean, reset, or delete anything.
- Report tracked state roots, ignored runtime roots, unsafe candidates, unexpectedly ignored files, Jujutsu/Git state summaries, and whether unrelated current @ work must be split before state commit.`;
}

function renderCommitPrompt(scope: string): string {
	const target = scope || "AOC project state";
	return `Run the explicit AOC project-state commit workflow for: ${target}

The user's /state-commit invocation is approval to inspect and commit only safe repo-owned AOC project-state files. It is not approval to push.

Workflow:
1. Detect VCS
- Run \`aoc-handshake --json\`.
- If it reports \`vcs.kind = "jj"\`, use Jujutsu only. Never use \`git add\`, \`git commit\`, or Git staging semantics in this repo.
- If it reports Git-only, use explicit Git path staging only.

2. Audit state first
- Run \`aoc state status\`.
- Stop before committing if unsafe candidates, unexpectedly ignored project-state files, or ambiguous filesets remain.

3. Inspect candidate state filesets
Default AOC state filesets:
- .aoc/ excluding logs, locks, .aoc/mind/, .aoc/tools/, and backups
- .taskmaster/ excluding logs and locks
- .omp/extensions/
- .omp/agents/
- .omp/skills/
- AGENTS.md, DESIGN.md, relevant AOC docs/tests for this state workflow

For Jujutsu:
- Run \`jj status\`.
- Run \`jj diff --summary -- <state filesets>\`.
- Run \`jj diff --stat -- <state filesets>\`.
- If current \`@\` is mixed, commit selected state filesets with \`jj commit -m <message> <filesets>\` or split first with \`jj split <filesets>\`. Ask one concise clarification if the split direction is unclear.
- Never run \`jj git push\` during /state-commit.

For Git-only:
- Run \`git status --short -- <state paths>\` and \`git diff --stat -- <state paths>\`.
- Stage only explicit approved state paths. Never stage broad paths like \`.\`.

4. Commit message
Use:

chore(state): track AOC project state

Include trailers:
AOC-Intent: keep AOC project state portable through Jujutsu/Git remotes
Tests: <state audit and init tests>
Risk: low|medium; <reason>

5. Report
- Jujutsu change/commit identity or Git SHA
- files committed
- tests run
- remaining unrelated changes
- reminder that push requires separate /state-push invocation.`;
}

function renderPushPrompt(scope: string): string {
	const target = scope || "AOC project state";
	return `Run the explicit AOC project-state push workflow for: ${target}

The user's /state-push invocation is explicit push intent, but you must still verify the exact Jujutsu bookmark and remote before mutating. Never use raw \`git push\` in a Jujutsu repo.

Workflow:
1. Verify VCS and safety
- Run \`aoc-handshake --json\` and require \`vcs.kind = "jj"\` for this workflow in the AOC repo.
- Run \`aoc state status\`.
- Stop if unsafe candidates or unexpectedly ignored project-state files are reported.
- Run \`jj status\` and verify the intended project-state commit is already committed; do not push mixed uncommitted \`@\` work.

2. Verify target
- Run \`jj bookmark list\`.
- Run \`jj git remote list\`.
- Use remote \`origin\` only when it exists.
- Use bookmark \`main\` only when local \`main\` is current/advanceable and the operator intent is clear.
- If no bookmark is known, or local bookmark is behind/diverged, stop and ask one concise clarification before pushing.

3. Push
- Prefer \`jj git push --bookmark <bookmark> --remote <remote>\`.
- Never run \`git push\` in a Jujutsu repo.
- Report the exact command run and observed result.`;
}

async function send(pi: ExtensionAPI, ctx: CommandContext, customType: string, content: string, details: Record<string, unknown>): Promise<void> {
	if (typeof pi.sendMessage === "function") {
		await pi.sendMessage({ customType, display: true, content, details }, { triggerTurn: true });
		return;
	}
	await ctx.ui?.notify?.(content, "info");
}

export default function aocStateExtension(pi: ExtensionAPI): void {
	pi.registerCommand("state-status", {
		description: "Run the read-only AOC project-state audit.",
		handler: async (args, ctx) => {
			const scope = argsText(args);
			await send(pi, ctx, "aoc.state.status.request", renderStatusPrompt(scope), { scope, cwd: ctx.cwd });
		},
	});

	pi.registerCommand("state-commit", {
		description: "Commit only safe AOC project-state files through the detected VCS workflow.",
		handler: async (args, ctx) => {
			const scope = argsText(args);
			await send(pi, ctx, "aoc.state.commit.request", renderCommitPrompt(scope), { scope, cwd: ctx.cwd });
		},
	});

	pi.registerCommand("state-push", {
		description: "Push committed AOC project state through explicit Jujutsu bookmark/remote workflow.",
		handler: async (args, ctx) => {
			const scope = argsText(args);
			await send(pi, ctx, "aoc.state.push.request", renderPushPrompt(scope), { scope, cwd: ctx.cwd });
		},
	});
}
