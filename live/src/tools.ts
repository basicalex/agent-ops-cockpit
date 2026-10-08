import { appendFile, mkdir, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { basename, dirname, join } from "node:path";
import { z } from "zod";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";
import { LIMITS, type IssueState, type Workspace, type Tab } from "./contracts";
import * as herdr from "./herdr";
import * as git from "./git";
import * as issues from "./issues";
import * as conversations from "./conversations";
import { redact } from "./redact";
import { createAgents, type Agents } from "./agents";
import { answerKeys } from "./control";

export const readOnlyAnnotations = { readOnlyHint: true, openWorldHint: false, destructiveHint: false } as const;
export const writeAnnotations = { readOnlyHint: false, openWorldHint: false, destructiveHint: false } as const;
export const stopAnnotations = { readOnlyHint: false, openWorldHint: false, destructiveHint: true } as const;
export const prAnnotations = { readOnlyHint: false, openWorldHint: true, destructiveHint: false } as const;
const NOTES_MAX_BYTES = 1_000_000;
function defaultNotesFile(env = process.env): string {
  const dir = env.AOC_LIVE_STATE_DIR || join(env.XDG_STATE_HOME || join(homedir(), ".local/state"), "aoc/live");
  return join(dir, "notes.jsonl");
}
type Dependencies = {
  herdr: typeof herdr; git: typeof git;
  issues: { listIssues: typeof issues.listIssues; getIssueState: (root: string, number: number, opts?: { events?: number; comments?: number }) => Promise<IssueState> };
  conversations: typeof conversations; redact: typeof redact;
  notesFile: string; agents: Agents;
};
export type LiveTool = {
  name: string; description: string; inputSchema: z.AnyZodObject;
  annotations: { readOnlyHint: boolean; openWorldHint: boolean; destructiveHint: boolean };
  execute(args?: unknown): Promise<CallToolResult>;
};
const nonempty = z.string().trim().min(1);
const workspaceInput = { workspace: nonempty };
const rootInput = { ...workspaceInput, root: nonempty.optional() };

export function createTools(overrides: Partial<Dependencies> = {}): LiveTool[] {
  const deps: Dependencies = { herdr, git, issues, conversations, redact, notesFile: defaultNotesFile(), agents: createAgents(), ...overrides };
  function safe(value: unknown, roots: string[]): unknown {
    if (typeof value === "string") {
      return deps.redact(value).replace(new RegExp(`${homedir().replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[^\\s\"'<>]*`, "g"), path => roots.some(root => path === root || path.startsWith(root + "/")) ? path : "[home path]");
    }
    if (Array.isArray(value)) return value.map(v => safe(v, roots));
    if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, safe(v, roots)]));
    return value;
  }
  function jsonResult(value: unknown, roots: string[], isError = false): CallToolResult {
    let text = JSON.stringify(safe(value, roots));
    if (Buffer.byteLength(text) > LIMITS.toolOutputBytes) {
      const original = text;
      let low = 0, high = Math.min(original.length, LIMITS.toolOutputBytes);
      while (low < high) {
        const mid = Math.ceil((low + high) / 2);
        if (Buffer.byteLength(JSON.stringify({ truncated: true, preview: original.slice(0, mid) })) <= LIMITS.toolOutputBytes) low = mid;
        else high = mid - 1;
      }
      text = JSON.stringify({ truncated: true, preview: original.slice(0, low) });
    }
    return { content: [{ type: "text", text }], ...(isError ? { isError: true } : {}) };
  }
  async function scoped(ref: string, requestedRoot?: string): Promise<{ workspace: Workspace; root: string; roots: string[] }> {
    const workspace = await deps.herdr.resolveWorkspace(ref);
    const roots = await deps.herdr.workspaceRepoRoots(workspace);
    if (requestedRoot && !roots.includes(requestedRoot)) throw new Error("root must equal a git toplevel of this live workspace");
    const root = requestedRoot ?? workspace.repoRoot;
    if (!root || !roots.includes(root)) throw new Error("Workspace has no available git repository");
    return { workspace, root, roots };
  }
  async function soft<T>(fn: () => Promise<T>): Promise<T | string> {
    try { return await fn(); }
    catch (error) { return `unavailable: ${error instanceof Error ? error.message.split("\n")[0]!.slice(0, 240) : "source failed"}`; }
  }
  function tool<A extends z.ZodRawShape>(name: string, description: string, shape: A, read: (args: z.infer<z.ZodObject<A>>, allow: (roots: string[]) => void) => Promise<unknown>): LiveTool {
    const inputSchema = z.object(shape).strict();
    return { name, description, inputSchema, annotations: readOnlyAnnotations,
      async execute(input = {}) {
        let roots: string[] = [];
        try { return jsonResult(await read(inputSchema.parse(input), value => { roots = value; }), roots); }
        catch (error) { return jsonResult({ error: error instanceof z.ZodError ? "Invalid tool arguments" : error instanceof Error ? error.message.split("\n")[0]!.slice(0, 300) : "Read unavailable" }, roots, true); }
      },
    };
  }
  async function tabContext(tab: Tab, includeScreen: boolean): Promise<unknown> {
    return { ...tab, panes: await Promise.all(tab.panes.map(async pane => {
      const context = await soft(async () => {
        const ref = await deps.conversations.conversationForPane(pane);
        if (ref) return { conversation: await deps.conversations.summarize(ref) };
        return includeScreen ? { screen: await deps.herdr.readPane(pane.paneId, { lines: 40 }) } : {};
      });
      return { ...pane, context };
    })) };
  }
  return [
    tool("workspace_overview", "START HERE. One line per live workspace: identity, agents, tabs, working and blocked counts.", {}, async () => {
      const snapshot = await deps.herdr.getOverviewSnapshot();
      return { workspaces: snapshot.workspaces.map(w => {
        const panes = w.tabs.flatMap(t => t.panes);
        const counts = new Map<string, number>();
        for (const pane of panes) if (pane.cwd) { const name = basename(pane.cwd); counts.set(name, (counts.get(name) ?? 0) + 1); }
        const repo = [...counts].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "unknown";
        return `${w.label} (${w.workspaceId}) repo=${repo} status=${w.agentStatus} tabs=${w.tabs.length} working=${panes.filter(p => p.agent && p.agentStatus === "working").length} blocked=${panes.filter(p => p.agent && ["blocked", "failed", "needs-decision"].includes(p.agentStatus)).length}`;
      }) };
    }),
    tool("get_workspace_state", "Read compact repo, agent, issue-journal and blocker context. Call after workspace_overview.", workspaceInput, async ({ workspace: ref }, allow) => {
      const workspace = await deps.herdr.resolveWorkspace(ref);
      const roots = await deps.herdr.workspaceRepoRoots(workspace);
      allow(roots);
      const root = workspace.repoRoot;
      const requireRoot = () => { if (!root || !roots.includes(root)) throw new Error("No available repository"); return root; };
      const [state, diff, openIssues, labelled] = await Promise.all([
        soft(() => deps.git.getGitState(requireRoot(), { commits: 5 })),
        soft(() => deps.git.getDiff({ root: requireRoot(), maxBytes: LIMITS.diffBytes })),
        soft(() => deps.issues.listIssues(requireRoot(), { state: "open", limit: 10 })),
        Promise.all(["needs-alex", "agent-failed", "risk-review"].map(label => soft(() => deps.issues.listIssues(requireRoot(), { state: "open", label, limit: 100 })))),
      ]);
      const issueCache = new Map<string, Promise<IssueState | string>>();
      const journalBlockers: string[] = [];
      const tabs = await Promise.all(workspace.tabs.map(async tab => {
        const panes = await Promise.all(tab.panes.map(async pane => {
          const lastAssistant = await soft(async () => {
            const ref = await deps.conversations.conversationForPane(pane);
            if (!ref) return null;
            return (await deps.conversations.summarize(ref)).lastMessages.filter(m => m.role === "assistant").at(-1)?.text.slice(0, 300) ?? null;
          });
          return { paneId: pane.paneId, agent: pane.agent, status: pane.agentStatus, lastAssistant };
        }));
        const tabRoot = await soft(async () => {
          const agentPane = tab.panes.find(p => p.agent) ?? tab.panes[0];
          return agentPane ? await deps.git.repoRootFor(agentPane.cwd) : root;
        });
        const branch = tabRoot && !tabRoot.startsWith("unavailable:") && roots.includes(tabRoot)
          ? await soft(async () => tabRoot === root && typeof state !== "string" ? state.branch : (await deps.git.getGitState(tabRoot, { commits: 0 })).branch) : null;
        const match = /issue-?(\d+)/i.exec(tab.label) ?? (typeof branch === "string" ? /issue-?(\d+)/i.exec(branch) : null);
        const issueNumber = match ? Number(match[1]) : null;
        let journal: unknown = null;
        if (issueNumber && panes.some(p => p.agent)) {
          const issueRoot = typeof tabRoot === "string" && roots.includes(tabRoot) ? tabRoot : root;
          const key = `${issueRoot}:${issueNumber}`;
          if (!issueCache.has(key)) issueCache.set(key, soft(() => {
            if (!issueRoot || !roots.includes(issueRoot)) throw new Error("No available repository");
            return deps.issues.getIssueState(issueRoot, issueNumber, { events: 1, comments: 0 });
          }));
          const snapshot = await issueCache.get(key)!;
          if (typeof snapshot === "string") journal = snapshot;
          else { journal = { state: snapshot.state, change: snapshot.events.at(-1)?.change ?? null }; journalBlockers.push(...snapshot.blockers.filter(Boolean)); }
        }
        return { tabId: tab.tabId, label: tab.label, number: tab.number, status: tab.agentStatus, issueNumber, journal, panes };
      }));
      const blockerIssues = [
        ...new Map(labelled.flatMap(group => typeof group === "string" ? [] : group).map(issue => [issue.number, issue])).values(),
        ...labelled.filter((group): group is string => typeof group === "string"),
      ];
      return {
        workspace: { id: workspace.workspaceId, label: workspace.label, root, roots },
        git: typeof state === "string" ? state : { branch: state.branch, head: state.head, headSubject: state.headSubject, dirty: state.dirty, staged: state.staged.length, unstaged: state.unstaged.length, untracked: state.untrackedTotal },
        diff: typeof diff === "string" ? diff : diff.summary.slice(0, 10),
        recentCommits: typeof state === "string" ? state : state.recentCommits.slice(0, 5),
        tabs, openIssues, blockers: { issues: blockerIssues, journal: [...new Set(journalBlockers)] },
      };
    }),
    tool("list_tabs", "List a workspace's tabs, panes and agent statuses.", workspaceInput, async ({ workspace: ref }, allow) => {
      const workspace = await deps.herdr.resolveWorkspace(ref);
      allow(await deps.herdr.workspaceRepoRoots(workspace));
      return workspace.tabs;
    }),
    tool("get_tab_context", "Read one tab by label, number or id. Messages only; screen tail when no transcript exists.", { ...workspaceInput, tab: z.union([nonempty, z.number().int()]) }, async ({ workspace: ref, tab: selector }, allow) => {
      const workspace = await deps.herdr.resolveWorkspace(ref); allow(await deps.herdr.workspaceRepoRoots(workspace));
      const exact = workspace.tabs.find(t => t.tabId === String(selector));
      const matches = exact ? [exact] : workspace.tabs.filter(t => t.label.toLowerCase() === String(selector).toLowerCase() || t.number === Number(selector));
      if (matches.length !== 1) throw new Error(matches.length ? "Ambiguous tab; use its id" : "Tab not found in workspace");
      return tabContext(matches[0]!, true);
    }),
    tool("search_conversations", "Find message text in live workspace transcripts. Returns cursors for focused slices.", { query: nonempty, workspace: nonempty.optional() }, async ({ query, workspace }, allow) => {
      const selected = workspace ? [await deps.herdr.resolveWorkspace(workspace)] : (await deps.herdr.getSnapshot()).workspaces;
      const roots = [...new Set((await Promise.all(selected.map(w => deps.herdr.workspaceRepoRoots(w)))).flat())]; allow(roots);
      return deps.conversations.searchConversations(query, { repoRoots: roots });
    }),
    tool("get_conversation_slice", "Read messages around a cursor, or the last six by conversation id. No tool outputs or thinking.", { cursor: nonempty.optional(), conversation: nonempty.optional(), before: z.number().int().min(0).max(19).optional(), after: z.number().int().min(0).max(19).optional() }, async ({ cursor, conversation, before, after }, allow) => {
      if (Boolean(cursor) === Boolean(conversation)) throw new Error("Provide exactly one of cursor or conversation");
      const id = (cursor ?? conversation)!.split("#")[0]!;
      const selected = (await deps.herdr.getSnapshot()).workspaces;
      const roots = [...new Set((await Promise.all(selected.map(w => deps.herdr.workspaceRepoRoots(w)))).flat())]; allow(roots);
      const paneRefs = await Promise.all(selected.flatMap(w => w.tabs.flatMap(t => t.panes))
        .filter(pane => pane.agent?.toLowerCase() === "claude" && `claude:${pane.sessionId}` === id)
        .map(async pane => {
          try {
            const root = await deps.git.repoRootFor(pane.cwd);
            return root && roots.includes(root) ? await deps.conversations.conversationForPane(pane) : null;
          } catch { return null; }
        }));
      if (!paneRefs.some(ref => ref?.id === id)) {
        const refs = (await Promise.all(roots.map(root => deps.conversations.listConversations(root)))).flat();
        if (!refs.some(ref => ref.id === id)) throw new Error("Conversation does not belong to a live workspace repository");
      }
      return deps.conversations.getSlice((cursor ?? conversation)!, { before, after });
    }),
    tool("list_issues", "List workspace issues; open by default. Filter by state or label.", { ...workspaceInput, state: z.enum(["open", "closed", "all"]).optional(), label: nonempty.optional() }, async ({ workspace, state, label }, allow) => {
      const scope = await scoped(workspace); allow(scope.roots); return deps.issues.listIssues(scope.root, { state, label });
    }),
    tool("get_issue_state", "Read the issue's semantic journal snapshot: objective, state, blockers, events and completion.", { ...workspaceInput, number: z.number().int().positive() }, async ({ workspace, number }, allow) => {
      const scope = await scoped(workspace); allow(scope.roots); return deps.issues.getIssueState(scope.root, number);
    }),
    tool("get_git_state", "Read branch, HEAD, change counts, commits and worktrees. root must belong to this workspace.", rootInput, async ({ workspace, root }, allow) => {
      const scope = await scoped(workspace, root); allow(scope.roots); return deps.git.getGitState(scope.root);
    }),
    tool("get_diff", "Read a bounded diff: worktree, staged, or commit/range. Optionally select paths.", { ...rootInput, target: nonempty.optional(), paths: z.array(nonempty).max(50).optional() }, async ({ workspace, root, target, paths }, allow) => {
      const scope = await scoped(workspace, root); allow(scope.roots); return deps.git.getDiff({ root: scope.root, target, paths, maxBytes: LIMITS.diffBytes });
    }),
    tool("read_file", "Read bounded repository file lines, optionally at a git ref. Secret files are refused.", { ...rootInput, path: nonempty, start_line: z.number().int().positive().optional(), end_line: z.number().int().positive().optional(), ref: nonempty.optional() }, async ({ workspace, root, path, start_line, end_line, ref }, allow) => {
      const scope = await scoped(workspace, root); allow(scope.roots); return deps.git.readRepoFile(scope.root, path, { startLine: start_line, endLine: end_line, ref });
    }),
    tool("search_code", "Search repository code by text or regex, optionally constrained by path glob.", { ...rootInput, query: nonempty, path_glob: nonempty.optional(), regex: z.boolean().optional() }, async ({ workspace, root, query, path_glob, regex }, allow) => {
      const scope = await scoped(workspace, root); allow(scope.roots); return deps.git.searchCode(scope.root, query, { pathGlob: path_glob, regex, maxHits: LIMITS.searchHits });
    }),
    tool("read_pane", "Read a redacted screen tail from a pane belonging to the selected workspace.", { ...workspaceInput, pane_id: nonempty, lines: z.number().int().min(1).max(200).optional() }, async ({ workspace: ref, pane_id, lines }, allow) => {
      const workspace = await deps.herdr.resolveWorkspace(ref); allow(await deps.herdr.workspaceRepoRoots(workspace));
      if (!workspace.tabs.some(t => t.panes.some(p => p.paneId === pane_id))) throw new Error("Pane does not belong to workspace");
      return { paneId: pane_id, text: await deps.herdr.readPane(pane_id, { lines }) };
    }),
    { ...tool("save_note", "Append a short note from Alex to the local AOC inbox on the Mac. Nothing is sent anywhere else. Use when Alex asks to note, remember or jot something down.", { text: nonempty.max(2000), workspace: nonempty.optional() }, async ({ text, workspace }) => {
      const size = await stat(deps.notesFile).then(s => s.size, () => 0);
      if (size > NOTES_MAX_BYTES) throw new Error("Note inbox is full");
      await mkdir(dirname(deps.notesFile), { recursive: true, mode: 0o700 });
      const at = new Date().toISOString();
      await appendFile(deps.notesFile, JSON.stringify({ at, text, workspace: workspace ?? null }) + "\n", { mode: 0o600 });
      return { saved: true, at };
    }), annotations: writeAnnotations },
    tool("agent_list", "List coding agents started here, their status and whether a report is ready.", {}, async (_args, allow) => {
      const agents = await deps.agents.list();
      allow(agents.flatMap(agent => [agent.repoRoot, agent.worktree]));
      return agents;
    }),
    tool("agent_read", "Read an agent's screen, branch progress and report. Use its id from agent_list.", { id: nonempty, lines: z.number().int().min(1).max(200).optional() }, async ({ id, lines }) => deps.agents.read(id, lines)),
    { ...tool("agent_start", "Start a coding agent in a workspace's own worktree and tab. Use for a task Alex wants done. harness defaults to omp; use claude only when Alex asks for Claude.", { ...workspaceInput, task: nonempty, harness: z.enum(["claude", "omp"]).optional() }, async ({ workspace, task, harness }, allow) => {
      const started = await deps.agents.start(workspace, task, harness);
      allow([started.worktree]);
      return started;
    }), annotations: writeAnnotations },
    { ...tool("agent_send", "Send a task or follow-up to an agent started here. Use agent_answer for a blocked question.", { id: nonempty, text: nonempty.max(4000) }, async ({ id, text }) => deps.agents.send(id, text)), annotations: writeAnnotations },
    { ...tool("agent_answer", "Answer an agent's terminal question with keys or text. Not while the agent is working.", { id: nonempty, input: z.union([z.object({ keys: z.array(z.enum(answerKeys)).min(1).max(10) }).strict(), z.object({ text: z.string().min(1).max(500) }).strict()]) }, async ({ id, input }) => deps.agents.answer(id, input)), annotations: writeAnnotations },
    { ...tool("agent_stop", "Close an agent's tab. Keep its branch and worktree for review or a PR.", { id: nonempty }, async ({ id }) => deps.agents.stop(id)), annotations: stopAnnotations },
    { ...tool("agent_open_pr", "Push an agent's clean branch and open a GitHub PR. Use after reviewing its report and commits.", { id: nonempty, title: nonempty.max(120), body: z.string().max(4000) }, async ({ id, title, body }) => deps.agents.openPr(id, title, body)), annotations: prAnnotations },
  ];
}
