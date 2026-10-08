import { afterAll, beforeAll, expect, test } from "bun:test";
import { z } from "zod";
import type { Server } from "node:http";
import type { AddressInfo } from "node:net";
import type { GitState, IssueState, Workspace } from "../src/contracts";
import { LIMITS } from "../src/contracts";
import { loadConfig } from "../src/config";
import * as herdr from "../src/herdr";
import * as git from "../src/git";
import * as conversations from "../src/conversations";
import { createTools } from "../src/tools";
import { start } from "../src/server";

const workspace: Workspace = {
  workspaceId: "w1", label: "Synthetic", number: 1, agentStatus: "working", focused: true, repoRoot: "/fixture/project",
  tabs: [{ tabId: "t1", workspaceId: "w1", label: "issue-10", number: 1, agentStatus: "working", focused: true, panes: [{ paneId: "p1", tabId: "t1", workspaceId: "w1", cwd: "/fixture/project", agent: "claude", agentStatus: "working", sessionId: null, title: "", focused: true }] }],
};
const state: GitState = { root: "/fixture/project", branch: "issue-10", head: "abc", headSubject: "Synthetic commit", upstream: null, ahead: 0, behind: 0, dirty: true, staged: [], unstaged: [{ path: "src/file.ts", status: "M", added: 1, removed: 0 }], untracked: [], untrackedTotal: 0, recentCommits: [], worktrees: [] };
const issue: IssueState = { schema: "aoc.issue.state/v1", issue: 10, url: "https://example.test/10", title: "Synthetic", objective: "Build context", state: "needs-decision", labels: [], closedAt: null, blockers: ["Approve scope"], commits: [], pull_requests: [], completion: { complete: false, url: null, at: null }, events: [{ source: "journal", event: "progress", kind: null, run_id: null, seq: 1, at: null, url: null, author: null, state: "needs-decision", change: "Adapter implemented", evidence: null, next: null, blockers: "Approve scope", summary: null }], recentComments: [], commentTotal: 0 };
const issueCalls: unknown[] = [];
let gitCalls = 0;
const tools = createTools({
  herdr: { ...herdr, getOverviewSnapshot: async () => ({ workspaces: [workspace] }), getSnapshot: async () => ({ workspaces: [workspace] }), resolveWorkspace: async () => workspace, workspaceRepoRoots: async () => ["/fixture/project", "/fixture/worktree"], readPane: async () => "synthetic screen" },
  git: { ...git, repoRootFor: async () => "/fixture/project", getGitState: async () => { gitCalls++; return state; }, getDiff: async () => { throw new Error("diff failed"); }, readRepoFile: async () => ({ path: "file", startLine: 1, endLine: 1, totalLines: 1, text: "界\"".repeat(12000), truncated: false }) },
  issues: { listIssues: async (_root, opts) => {
    if (opts?.label === "agent-failed") throw new Error("label source failed");
    return opts?.label === "needs-alex" ? [{ number: 11, title: "Needs Alex", state: "OPEN", labels: ["needs-alex"], updatedAt: "", url: "" }] : [];
  }, getIssueState: async (_root, _number, opts) => { issueCalls.push(opts); return issue; } },
  conversations: { ...conversations, conversationForPane: async () => null },
  redact: text => text,
});
let server: Server;
let disabled: Server;
const listeners: Server[] = [];
let url: string;
const headers = { "Content-Type": "application/json", Accept: "application/json, text/event-stream" };
beforeAll(async () => {
  const config = loadConfig({ XDG_CONFIG_HOME: `/tmp/aoc-live-no-config-${process.pid}`, AOC_LIVE_TOKEN: "x".repeat(64), AOC_LIVE_PORT: "0", AOC_LIVE_PUBLIC_PORT: "0" });
  const active = await start({ config, tools });
  server = active.local;
  listeners.push(active.local, active.public);
  url = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  const inactive = await start({ config: { ...config, pathToken: undefined }, tools });
  disabled = inactive.local;
  listeners.push(inactive.local, inactive.public);
});
afterAll(async () => {
  await Promise.all(listeners.map(s => new Promise<void>((resolve, reject) => s.close(error => error ? reject(error) : resolve()))));
});
const rpcResponseSchema = z.object({ result: z.object({
  serverInfo: z.object({ name: z.string() }).default({ name: "" }),
  instructions: z.string().default(""),
  tools: z.array(z.object({ name: z.string(), annotations: z.object({
    readOnlyHint: z.boolean(), destructiveHint: z.boolean(), openWorldHint: z.boolean(),
  }) })).default([]),
  content: z.array(z.object({ type: z.literal("text"), text: z.string() })).default([]),
  isError: z.boolean().optional(),
}) });
async function rpc(method: string, params?: unknown) {
  const response = await fetch(`${url}/mcp/${"x".repeat(64)}`, { method: "POST", headers, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }) });
  expect(response.status).toBe(200);
  return rpcResponseSchema.parse(await response.json());
}

test("stateless initialize lists agent tools with their control annotations", async () => {
  const init = await rpc("initialize", { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "synthetic", version: "1" } });
  expect(init.result.serverInfo.name).toBe("aoc-live");
  const listing = await rpc("tools/list");
  expect(listing.result.tools.map(t => t.name)).toEqual(["workspace_overview", "get_workspace_state", "list_tabs", "get_tab_context", "search_conversations", "get_conversation_slice", "list_issues", "get_issue_state", "get_git_state", "get_diff", "read_file", "search_code", "read_pane", "save_note", "agent_list", "agent_read", "agent_start", "agent_send", "agent_answer", "agent_stop", "agent_open_pr"]);
  const writes = ["save_note", "agent_start", "agent_send", "agent_answer", "agent_stop", "agent_open_pr"];
  for (const tool of listing.result.tools) expect(tool.annotations).toEqual({ readOnlyHint: !writes.includes(tool.name), destructiveHint: tool.name === "agent_stop", openWorldHint: tool.name === "agent_open_pr" });
});

test("wrong and disabled paths return 404; stateless GET and DELETE return 405", async () => {
  expect((await fetch(`${url}/mcp/wrong`)).status).toBe(404);
  expect((await fetch(`${url}/`)).status).toBe(404);
  const off = `http://127.0.0.1:${(disabled.address() as AddressInfo).port}`;
  expect((await fetch(`${off}/mcp/${"x".repeat(64)}`)).status).toBe(404);
  for (const method of ["GET", "DELETE"]) expect((await fetch(`${url}/mcp/${"x".repeat(64)}`, { method, headers })).status).toBe(405);
});

test("tool smoke returns overview without git; snapshot preserves journal blockers despite diff failure", async () => {
  const before = gitCalls;
  const overview = await rpc("tools/call", { name: "workspace_overview", arguments: {} });
  expect(JSON.parse(overview.result.content[0].text).workspaces[0]).toContain("working=1");
  expect(gitCalls).toBe(before);
  const response = await rpc("tools/call", { name: "get_workspace_state", arguments: { workspace: "w1" } });
  const snapshot = JSON.parse(response.result.content[0].text);
  expect(snapshot.git).toMatchObject({ branch: "issue-10", dirty: true, unstaged: 1 });
  expect(snapshot.diff).toBe("unavailable: diff failed");
  expect(snapshot.tabs[0].journal).toEqual({ state: "needs-decision", change: "Adapter implemented" });
  expect(snapshot.blockers.journal).toEqual(["Approve scope"]);
  expect(snapshot.blockers.issues[0].number).toBe(11);
  expect(snapshot.blockers.issues).toContain("unavailable: label source failed");
  expect(issueCalls.at(-1)).toEqual({ events: 1, comments: 0 });
  const issueResponse = await rpc("tools/call", { name: "get_issue_state", arguments: { workspace: "w1", number: 10 } });
  expect(JSON.parse(issueResponse.result.content[0].text)).toEqual(issue);
});

test("out-of-scope roots and panes return short errors; oversized output remains bounded valid JSON", async () => {
  const outside = await rpc("tools/call", { name: "get_git_state", arguments: { workspace: "w1", root: "/fixture/outside" } });
  expect(outside.result.isError).toBe(true);
  expect(JSON.parse(outside.result.content[0].text).error).toContain("git toplevel");
  const pane = await rpc("tools/call", { name: "read_pane", arguments: { workspace: "w1", pane_id: "other" } });
  expect(pane.result.isError).toBe(true);
  const file = await rpc("tools/call", { name: "read_file", arguments: { workspace: "w1", path: "file" } });
  const text = file.result.content[0].text;
  expect(Buffer.byteLength(text)).toBeLessThanOrEqual(LIMITS.toolOutputBytes);
  expect(JSON.parse(text).truncated).toBe(true);
});

test("Host and Origin guards reject browser and DNS-rebinding requests with empty 403 responses", async () => {
  const port = (server.address() as AddressInfo).port;
  const endpoint = `${url}/mcp/${"x".repeat(64)}`;
  const forbidden: Record<string, string>[] = [
    { Origin: "https://evil.example" }, { Origin: "null" },
    { Origin: "http://127.0.0.1:1" }, { Host: `evil.example:${port}` },
    { Host: "127.0.0.1:1" },
  ];
  for (const extra of forbidden) {
    const response = await fetch(endpoint, { method: "POST", headers: { ...headers, ...extra }, body: "{}" });
    expect(response.status).toBe(403);
    expect(await response.text()).toBe("");
  }
  for (const host of [`127.0.0.1:${port}`, `localhost:${port}`]) {
    const response = await fetch(endpoint, { method: "POST", headers: { ...headers, Host: host, Origin: `http://${host}` }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }) });
    expect(response.status).toBe(200);
    expect(rpcResponseSchema.parse(await response.json()).result.tools.map(t => t.name)).toContain("workspace_overview");
  }
  for (const origin of ["https://chatgpt.com", "https://chat.openai.com"]) {
    const response = await fetch(endpoint, { method: "POST", headers: { ...headers, Origin: origin }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }) });
    expect(response.status).toBe(200);
    expect(rpcResponseSchema.parse(await response.json()).result.tools.map(t => t.name)).toContain("workspace_overview");
  }
});

test("slices accept a live pane transcript from a repo subdirectory, but refuse unrelated ids", async () => {
  const nested = structuredClone(workspace);
  nested.tabs[0]!.panes[0]!.cwd = "/fixture/project/subdir";
  nested.tabs[0]!.panes[0]!.sessionId = "nested";
  const ref = { id: "claude:nested", agent: "claude" as const, path: "/fixture/transcripts/nested.jsonl", cwd: "/fixture/project/subdir", title: null, updatedAt: "", messageCount: 1 };
  const scopedTools = createTools({
    herdr: { ...herdr, getSnapshot: async () => ({ workspaces: [nested] }), workspaceRepoRoots: async () => ["/fixture/project"] },
    git: { ...git, repoRootFor: async () => "/fixture/project" },
    conversations: { ...conversations, listConversations: async () => [], conversationForPane: async () => ref, getSlice: async () => ({ conversation: ref, messages: [{ cursor: "claude:nested#0", index: 0, role: "assistant", at: null, text: "Nested repo context", tools: [] }], prevCursor: null, nextCursor: null }) },
    redact: text => text,
  });
  const slice = scopedTools.find(tool => tool.name === "get_conversation_slice")!;
  expect((await slice.execute({ cursor: "claude:nested#0" })).isError).not.toBe(true);
  expect((await slice.execute({ cursor: "claude:unrelated#0" })).isError).toBe(true);
});
