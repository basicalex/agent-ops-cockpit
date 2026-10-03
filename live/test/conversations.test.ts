import { afterAll, beforeAll, expect, test } from "bun:test";
import { copyFile, mkdir, mkdtemp, rm, utimes, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { LIMITS, type Pane } from "../src/contracts";
import { conversationForPane, listConversations, searchConversations, getSlice, summarize, transcriptDirectory, setConversationRedactor } from "../src/conversations";

const fixtureDir = new URL("fixtures/w1/", import.meta.url).pathname;
const root = "/fixture/project";
const oldClaude = process.env.AOC_LIVE_CLAUDE_DIR;
const oldOmp = process.env.AOC_LIVE_OMP_DIR;
let runtime: string;
let claudePath: string;
const pane: Pane = { paneId: "p1", tabId: "t1", workspaceId: "w1", agent: "claude", agentStatus: "working", sessionId: "session-one", cwd: root, title: "", focused: false };

beforeAll(async () => {
  runtime = await mkdtemp(join(fixtureDir, "runtime-conversations-"));
  process.env.AOC_LIVE_CLAUDE_DIR = join(runtime, "claude");
  process.env.AOC_LIVE_OMP_DIR = join(runtime, "omp");
  setConversationRedactor(text => text.replaceAll("synthetic-secret", "[REDACTED]"));
  const claudeDir = transcriptDirectory(root, "claude");
  const ompDir = transcriptDirectory(root, "omp");
  await mkdir(claudeDir, { recursive: true }); await mkdir(ompDir, { recursive: true });
  claudePath = join(claudeDir, "session-one.jsonl");
  await copyFile(join(fixtureDir, "claude.jsonl"), claudePath);
  const ompPath = join(ompDir, "2026-10-01T11-00-00Z_omp-one.jsonl");
  await copyFile(join(fixtureDir, "omp.jsonl"), ompPath);
  await utimes(claudePath, new Date(), new Date(Date.now() - 1000));
  await utimes(ompPath, new Date(), new Date());
});
afterAll(async () => {
  if (oldClaude === undefined) delete process.env.AOC_LIVE_CLAUDE_DIR; else process.env.AOC_LIVE_CLAUDE_DIR = oldClaude;
  if (oldOmp === undefined) delete process.env.AOC_LIVE_OMP_DIR; else process.env.AOC_LIVE_OMP_DIR = oldOmp;
  setConversationRedactor(null);
  await rm(runtime, { recursive: true, force: true });
});

test("Claude messages exclude tool output, reasoning, sidechains, metadata and injected context", async () => {
  const ref = await conversationForPane(pane);
  expect(ref!.id).toBe("claude:session-one");
  expect(ref!.messageCount).toBe(9);
  const slice = await getSlice("claude:session-one#1", { before: 1, after: 2 });
  expect(slice.messages.map(m => m.text)).toEqual(["Find the Release plan", "Release plan starts with scope.", "What is next?", "Next: verify the Release. [REDACTED]"]);
  expect(slice.messages[1]!.tools).toEqual(["read_file"]);
  expect(slice.messages.map(m => m.index)).toEqual([0, 1, 2, 3]);
  expect(slice.nextCursor).toBe("claude:session-one#4");
  expect(slice.prevCursor).toBeNull();
  expect(JSON.stringify(slice)).not.toMatch(/hidden|thinking|sidechain|tool input|meta text/);
  expect(await conversationForPane({ ...pane, agent: "omp" })).toBeNull();
  expect(await conversationForPane({ ...pane, sessionId: "missing" })).toBeNull();
  expect(await conversationForPane({ ...pane, sessionId: "../escape" })).toBeNull();
});

test("omp message shapes, titles and tool names are supported; newest transcripts first", async () => {
  const refs = await listConversations(root);
  expect(refs.map(ref => ref.id)).toEqual(["omp:omp-one", "claude:session-one"]);
  expect(refs[0]!.title).toBe("Synthetic Release");
  expect(refs[0]!.messageCount).toBe(3);
  const slice = await getSlice("omp:omp-one");
  expect(slice.messages.map(m => m.text)).toEqual(["Review release changes", "Release changes reviewed.", "Ready for review."]);
  expect(slice.messages[1]!.tools).toEqual(["read"]);
  expect((await summarize(refs[0]!)).firstUserMessage).toBe("Review release changes");
});

test("search is case-insensitive, newest first, excludes old files and stops at the cap", async () => {
  const oldPath = join(transcriptDirectory(root, "claude"), "old.jsonl");
  await writeFile(oldPath, JSON.stringify({ type: "user", message: { role: "user", content: "Release old" } }) + "\n");
  const old = new Date(Date.now() - 31 * 86400_000); await utimes(oldPath, old, old);
  const result = await searchConversations("rElEaSe", { repoRoots: [root], limit: 3 });
  expect(result.hits.map(m => m.cursor)).toEqual(["omp:omp-one#0", "omp:omp-one#1", "claude:session-one#0"]);
  expect(result.truncated).toBe(true);
  const full = await searchConversations("release", { repoRoots: [root] });
  expect(full.hits.map(m => m.conversation)).not.toContain("claude:old");
  expect((await searchConversations("hidden", { repoRoots: [root] })).hits).toEqual([]);
  expect((await getSlice(result.hits[0]!.cursor)).messages[0]!.text).toBe("Review release changes");
});

test("id slices return last six, cursors paginate and each message is capped", async () => {
  const last = await getSlice("claude:session-one");
  expect(last.messages.map(m => m.index)).toEqual([3, 4, 5, 6, 7, 8]);
  expect(last.prevCursor).toBe("claude:session-one#2");
  expect(last.nextCursor).toBeNull();
  await expect(getSlice("claude:session-one#90")).rejects.toThrow("out of range");
  const longPath = join(transcriptDirectory(root, "claude"), "long.jsonl");
  await writeFile(longPath, Array.from({ length: 25 }, (_, i) => JSON.stringify({ type: "user", message: { role: "user", content: `Message ${i}: ` + "x".repeat(2000) } })).join("\n"));
  await listConversations(root);
  const capped = await getSlice("claude:long#12", { before: 19, after: 19 });
  expect(capped.messages.map(m => m.index)).toEqual(Array.from({ length: 20 }, (_, i) => i));
  expect(capped.messages.every(m => m.text.length === LIMITS.messageChars)).toBe(true);
});
