// Shared module contracts for aoc-live. Every module implements exactly the
// signatures declared here; tools.ts composes them. Do not change a signature
// without the orchestrator's sign-off.

// ---- exec.ts (W2): the only module allowed to start a subprocess ----
export type ExecResult = { stdout: string; stderr: string; code: number; truncated: boolean };
export type ExecOptions = { cwd?: string; timeoutMs?: number; maxBytes?: number };
export type Runner = (argv: string[], opts: ExecOptions) => Promise<ExecResult>;
// runReadOnly(argv, opts): throws ReadOnlyViolation unless isAllowed(argv).
// isAllowed(argv): boolean. setRunner(fn) swaps the spawn backend in tests;
// the allowlist check still runs before the injected runner.

// ---- redact.ts (W2) ----
// redact(text): string               masks secrets in any text we return
// isSecretPath(path): boolean        true for paths that must never be read

// ---- git.ts (W2): every function takes an absolute repo root ----
export type GitState = {
  root: string;
  branch: string | null;          // null when detached
  head: string;                   // full SHA
  headSubject: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  dirty: boolean;
  staged: FileChange[];
  unstaged: FileChange[];
  untracked: string[];            // capped, see limits
  untrackedTotal: number;
  recentCommits: Commit[];
  worktrees: { path: string; branch: string | null }[];
};
export type FileChange = { path: string; status: string; added: number | null; removed: number | null };
export type Commit = { sha: string; date: string; author: string; subject: string };
export type DiffRequest = { root: string; target?: "worktree" | "staged" | string; paths?: string[]; maxBytes?: number };
export type DiffResult = { summary: FileChange[]; patch: string; truncated: boolean };
export type FileRead = { path: string; startLine: number; endLine: number; totalLines: number; text: string; truncated: boolean };
export type CodeHit = { path: string; line: number; text: string };
// getGitState(root, { commits?: number }): Promise<GitState>
// getDiff(req: DiffRequest): Promise<DiffResult>
// readRepoFile(root, relPath, { startLine?, endLine?, ref? }): Promise<FileRead>
// searchCode(root, query, { pathGlob?, regex?, maxHits? }): Promise<{ hits: CodeHit[]; truncated: boolean }>
// repoRootFor(dir): Promise<string | null>    git toplevel of a directory

// ---- issues.ts (W2) ----
// Issue state comes from issue #9's journal: `bin/aoc-journal state <n> --json
// --events <k>` (schema aoc.issue.state/v1), run from the repo root. That
// snapshot is the default semantic task context; do not re-parse journal
// comments here. Free-form (non-journal) comments come from one gh call.
export type JournalEvent = {
  source: "dispatch" | "journal"; event: string; kind: string | null; run_id: string | null;
  seq: number | null; at: string | null; url: string | null; author: string | null;
  state: string | null; change: string | null; evidence: string | null; next: string | null;
  blockers: string | null; summary: string | null;
};
export type IssueComment = { author: string; createdAt: string; body: string; url: string };
export type IssueSummary = { number: number; title: string; state: string; labels: string[]; updatedAt: string; url: string };
export type IssueState = {
  schema: "aoc.issue.state/v1";
  issue: number; url: string; title: string;
  objective: string;
  state: string;                  // open|ready|running|needs-decision|review|failed|closed
  labels: string[]; closedAt: string | null;
  blockers: string[]; commits: string[]; pull_requests: string[];
  completion: { complete: boolean; url: string | null; at: string | null };
  events: JournalEvent[];         // newest last, capped by --events
  recentComments: IssueComment[]; // non-journal comments only, newest last, capped
  commentTotal: number;
};
// listIssues(root, { state?: "open" | "closed" | "all", label?, limit? }): Promise<IssueSummary[]>
// getIssueState(root, number, { events?, comments? }): Promise<IssueState>

// ---- herdr.ts (W1) ----
export type Pane = {
  paneId: string; tabId: string; workspaceId: string;
  agent: string | null; agentStatus: string; sessionId: string | null;
  cwd: string; title: string; focused: boolean;
};
export type Tab = { tabId: string; workspaceId: string; label: string; number: number; agentStatus: string; focused: boolean; panes: Pane[] };
export type Workspace = { workspaceId: string; label: string; number: number; agentStatus: string; focused: boolean; repoRoot: string | null; tabs: Tab[] };
// getSnapshot(): Promise<{ workspaces: Workspace[] }>   one herdr api snapshot call, normalized
// resolveWorkspace(ref): Promise<Workspace>             by id, label, or repo basename; throws on ambiguity
// readPane(paneId, { lines? }): Promise<string>         herdr pane read, redacted

// ---- conversations.ts (W1) ----
export type ConversationRef = { id: string; agent: "claude" | "omp"; path: string; cwd: string | null; title: string | null; updatedAt: string; messageCount: number };
export type Message = { cursor: string; index: number; role: "user" | "assistant"; at: string | null; text: string; tools: string[] };
// id format: "claude:<sessionId>" | "omp:<sessionId>"; cursor format: "<id>#<index>"
// conversationForPane(pane): Promise<ConversationRef | null>
// listConversations(repoRoot, { limit? }): Promise<ConversationRef[]>
// summarize(ref): Promise<{ ref: ConversationRef; firstUserMessage: string; lastMessages: Message[] }>
// searchConversations(query, { repoRoots, limit? }): Promise<{ hits: (Message & { conversation: string })[]; truncated: boolean }>
// getSlice(cursorOrId, { before?, after? }): Promise<{ conversation: ConversationRef; messages: Message[]; prevCursor: string | null; nextCursor: string | null }>

export const LIMITS = {
  toolOutputBytes: 12_000,        // hard cap on any single tool result
  diffBytes: 10_000,
  fileLines: 200,
  searchHits: 30,
  messageChars: 1_500,            // per message in slices; summaries use 300
  untracked: 20,
  commits: 8,
} as const;

export class ReadOnlyViolation extends Error {}
