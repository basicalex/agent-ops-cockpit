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

function renderJjInitPrompt(scope: string): string {
	const target = scope || "the current repository";
	return `Run the AOC/OMP Jujutsu initialization workflow for: ${target}

The user's /jj-init invocation is explicit approval to analyze whether this Git repository should be converted to colocated Jujutsu+Git. It is not approval to push, fetch, import/export, clean, reset, stash, or commit work. The only intended mutation is \`jj git init --colocate\`, and only after the safety checks below pass or the user explicitly confirms the exact dirty-state risk.

Workflow:

1. Inspect read-only state first
- Confirm \`jj\` is installed with \`jj --version\`.
- Run \`aoc-handshake --json\` when available and read the VCS metadata.
- Run \`git rev-parse --show-toplevel\` and confirm the target is the intended Git repository root.
- Run \`jj root\` and \`jj git colocation status\` to check whether Jujutsu is already initialized.
- Run \`git status --short\`, \`git diff --stat\`, and \`git diff --cached --stat\` to understand dirty work before touching repository metadata.

2. Refuse or clarify unsafe states
- If the repo already has Jujutsu, do not re-initialize; verify with \`jj status\` and report the existing mode.
- If this is not a Git repository, stop and report that /jj-init only initializes Jujutsu over an existing Git repo.
- If \`jj\` is missing, stop and tell the operator to install it.
- If dirty/staged/untracked work is present, summarize it and ask one concise clarification before initializing unless the user has already explicitly asked to convert this dirty repo now.
- Never stash, reset, clean, delete files, commit, push, fetch, or run \`jj git import/export\` as part of initialization.

3. Initialize only when safe or explicitly confirmed
- Run: \`jj git init --colocate\`
- If remote bookmark hints are printed, track matching existing remote bookmarks when that is a direct metadata setup step, for example \`jj bookmark track main --remote=origin\`.
- If Jujutsu identity warnings appear, copy existing Git identity into Jujutsu config only when observed from \`git config --get user.name\` and \`git config --get user.email\`; do not invent identity values.

4. Refresh AOC context
- Run \`aoc-init\` or the repo-local AOC init command if available, with existing project conventions for skipping expensive builds when appropriate.
- Do not rely on a stale installed \`aoc-handshake\`; verify the active command reports VCS metadata.

5. Verify and report
- Run \`jj root\`.
- Run \`jj git colocation status\`.
- Run \`jj status\`.
- Run \`aoc-handshake --json\` and confirm \`vcs.kind == "jj"\`, \`preferredTool == "jj"\`, and \`jj.colocatedGit == true\`.
- Report dirty work that remains in the Jujutsu \`@\` working-copy change.

Final response:
- whether initialization happened or was skipped because Jujutsu already existed
- Jujutsu root and colocation status
- AOC handshake VCS result
- current dirty-work summary
- exact next instruction for already-running agents: run \`aoc-handshake --json\`, follow \`preferredTool=jj\`, and stop using Git staging semantics

Safety rules:
- AOC startup and \`aoc-init\` detect/report Jujutsu; they do not auto-initialize it.
- \`/jj-init\` is the explicit operator workflow for initialization.
- Never push through Git or Jujutsu without explicit push approval.
- Never hide, reset, clean, or commit dirty work during initialization.`;
}

export default function aocJjInitExtension(pi: ExtensionAPI): void {
	pi.registerCommand("jj-init", {
		description: "Safely initialize colocated Jujutsu for an existing Git repo",
		async handler(args, ctx) {
			const scope = argsText(args);
			const content = renderJjInitPrompt(scope);
			if (pi.sendMessage) {
				await pi.sendMessage(
					{
						customType: "aoc.jj-init.request",
						display: true,
						content,
						details: { scope: scope || null, cwd: ctx.cwd || null },
					},
					{ triggerTurn: true },
				);
			} else if (ctx.ui?.notify) {
				await ctx.ui.notify(content, "info");
			}
		},
	});
}
