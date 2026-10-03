import { createReadStream } from "node:fs";
import { lstat, readdir } from "node:fs/promises";
import { homedir } from "node:os";
import { join, relative, resolve } from "node:path";
import { createInterface } from "node:readline";
import { z } from "zod";
import { loadConfig } from "./config";
import { LIMITS, type ConversationRef, type Message, type Pane } from "./contracts";
import { redact } from "./redact";

let redactText = redact;
export function setConversationRedactor(fn: typeof redact | null): void { redactText = fn ?? redact; }
const refs = new Map<string, ConversationRef>();
const timestampSchema = z.union([z.string(), z.number()]).optional();
const recordSchema = z.object({
  type: z.string(), id: z.string().optional(), cwd: z.string().optional(),
  title: z.string().optional(), aiTitle: z.string().optional(), timestamp: timestampSchema,
  isSidechain: z.boolean().optional(), isMeta: z.boolean().optional(), isApiErrorMessage: z.boolean().optional(),
  message: z.object({
    role: z.string().optional(), timestamp: timestampSchema,
    content: z.union([z.string(), z.array(z.object({
      type: z.string(), text: z.string().optional(), name: z.string().optional(),
    }))]).optional(),
  }).optional(),
});

export function transcriptDirectory(root: string, agent: "claude" | "omp"): string {
  const config = loadConfig();
  const slug = agent === "claude" ? resolve(root).replace(/[/.]/g, "-") : "-" + relative(homedir(), resolve(root)).replace(/\//g, "-");
  return join(agent === "claude" ? config.claudeDir : config.ompDir, slug);
}

function cleanText(text: string): string {
  // Only injected context wrappers, not ordinary user-authored XML.
  const tags = "system-reminder|system-notice|system-instruction|system|developer|environment_context|project-context|repo-rules|user_instructions|instructions|memories|context";
  let safe = text.replace(new RegExp(`<(${tags})(?:\\s[^>]*)?>[\\s\\S]*?<\\/\\1\\s*>`, "gi"), "");
  safe = safe.replace(new RegExp(`<(?:${tags})(?:\\s[^>]*)?>[\\s\\S]*$`, "gi"), "");
  return redactText(safe.trim());
}

async function* records(path: string): AsyncGenerator<z.infer<typeof recordSchema>> {
  const stream = createReadStream(path, { encoding: "utf8" });
  const lines = createInterface({ input: stream, crlfDelay: Infinity });
  try {
    for await (const line of lines) {
      try { const parsed = recordSchema.safeParse(JSON.parse(line)); if (parsed.success) yield parsed.data; }
      catch { /* An interrupted final write or malformed record is not a message. */ }
    }
  } finally { lines.close(); stream.destroy(); }
}

async function* messages(ref: ConversationRef): AsyncGenerator<Message> {
  let index = 0;
  for await (const row of records(ref.path)) {
    if (row.isSidechain || row.isMeta || row.isApiErrorMessage) continue;
    if (ref.agent === "claude" ? !["user", "assistant"].includes(row.type) : row.type !== "message") continue;
    const message = row.message;
    const role = message?.role ?? row.type;
    if (role !== "user" && role !== "assistant") continue;
    const tools: string[] = [];
    let text = "";
    if (typeof message?.content === "string") text = message.content;
    else if (Array.isArray(message?.content)) {
      const parts: string[] = [];
      for (const block of message.content) {
        if (block.type === "text" && typeof block.text === "string") parts.push(block.text);
        if (role === "assistant" && ["tool_use", "toolCall"].includes(block.type) && typeof block.name === "string") tools.push(redactText(block.name));
      }
      text = parts.join("\n");
    }
    text = cleanText(text);
    if (!text && !tools.length) continue;
    const at = row.timestamp ?? message?.timestamp;
    yield { cursor: `${ref.id}#${index}`, index: index++, role, at: typeof at === "string" ? at : typeof at === "number" ? new Date(at).toISOString() : null, text, tools: [...new Set(tools)] };
  }
}

async function describe(path: string, agent: "claude" | "omp", cwd: string | null, headerOnly = false): Promise<ConversationRef> {
  const info = await lstat(path);
  if (!info.isFile()) throw new Error("Transcript is not a regular file");
  let id = path.split("/").at(-1)!.replace(/\.jsonl$/, "");
  let title: string | null = null;
  for await (const row of records(path)) {
    if (agent === "omp" && row.type === "session") { if (typeof row.id === "string") id = row.id; cwd = typeof row.cwd === "string" ? row.cwd : cwd; }
    if (typeof row.cwd === "string" && !cwd) cwd = row.cwd;
    if (typeof row.title === "string" && ["title", "session", "title_change"].includes(row.type)) title = cleanText(row.title);
    if (typeof row.aiTitle === "string" && row.type === "ai-title") title = cleanText(row.aiTitle);
    if (headerOnly && (agent === "claude" || row.type === "session")) break;
  }
  const ref: ConversationRef = { id: `${agent}:${id}`, agent, path, cwd, title, updatedAt: info.mtime.toISOString(), messageCount: 0 };
  if (!headerOnly) for await (const _message of messages(ref)) ref.messageCount++;
  refs.set(ref.id, ref);
  return ref;
}

type Candidate = { path: string; agent: "claude" | "omp"; root: string; modified: number };
async function candidates(repoRoots: string[]): Promise<Candidate[]> {
  const result: Candidate[] = [];
  const seen = new Set<string>();
  for (const root of repoRoots) for (const agent of ["claude", "omp"] as const) {
    const dir = transcriptDirectory(root, agent);
    let entries;
    try { entries = await readdir(dir, { withFileTypes: true }); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") continue; throw error; }
    for (const entry of entries) {
      if (!entry.isFile() || !entry.name.endsWith(".jsonl")) continue;
      const path = join(dir, entry.name);
      if (seen.has(path)) continue;
      seen.add(path);
      result.push({ path, agent, root, modified: (await lstat(path)).mtimeMs });
    }
  }
  return result.sort((a, b) => b.modified - a.modified || a.path.localeCompare(b.path));
}

export async function conversationForPane(pane: Pane): Promise<ConversationRef | null> {
  if (pane.agent?.toLowerCase() !== "claude" || !pane.sessionId || !/^[a-zA-Z0-9_-]+$/.test(pane.sessionId)) return null;
  try { return await describe(join(transcriptDirectory(pane.cwd, "claude"), `${pane.sessionId}.jsonl`), "claude", pane.cwd); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return null; throw error; }
}
export async function listConversations(repoRoot: string, { limit = Number.MAX_SAFE_INTEGER }: { limit?: number } = {}): Promise<ConversationRef[]> {
  const result: ConversationRef[] = [];
  for (const item of (await candidates([repoRoot])).slice(0, Math.max(0, limit))) result.push(await describe(item.path, item.agent, repoRoot));
  return result;
}
export async function summarize(ref: ConversationRef): Promise<{ ref: ConversationRef; firstUserMessage: string; lastMessages: Message[] }> {
  let firstUserMessage = "";
  const lastMessages: Message[] = [];
  for await (const message of messages(ref)) {
    if (message.role === "user" && !firstUserMessage) firstUserMessage = message.text.slice(0, 300);
    lastMessages.push({ ...message, text: message.text.slice(0, 300) });
    if (lastMessages.length > 6) lastMessages.shift();
  }
  return { ref, firstUserMessage, lastMessages };
}
export async function searchConversations(query: string, { repoRoots, limit = LIMITS.searchHits }: { repoRoots: string[]; limit?: number }): Promise<{ hits: (Message & { conversation: string })[]; truncated: boolean }> {
  if (!query.trim()) throw new Error("Search query must not be empty");
  limit = Math.min(LIMITS.searchHits, Math.max(1, limit));
  const hits: (Message & { conversation: string })[] = [];
  const cutoff = Date.now() - 30 * 86400_000;
  for (const item of await candidates(repoRoots)) {
    if (item.modified < cutoff) continue;
    const ref = await describe(item.path, item.agent, item.root, true);
    for await (const message of messages(ref)) {
      ref.messageCount = message.index + 1;
      if (!message.text.toLowerCase().includes(query.toLowerCase())) continue;
      hits.push({ ...message, text: message.text.slice(0, LIMITS.messageChars), conversation: ref.id });
      if (hits.length >= limit) return { hits, truncated: true };
    }
  }
  return { hits, truncated: false };
}
export async function getSlice(cursorOrId: string, { before = 3, after = 3 }: { before?: number; after?: number } = {}): Promise<{ conversation: ConversationRef; messages: Message[]; prevCursor: string | null; nextCursor: string | null }> {
  const match = /^(claude|omp):([a-zA-Z0-9_-]+)(?:#(\d+))?$/.exec(cursorOrId);
  if (!match) throw new Error("Invalid conversation id or cursor");
  const id = `${match[1]}:${match[2]}`;
  let ref = refs.get(id);
  if (!ref) {
    const config = loadConfig();
    const base = match[1] === "claude" ? config.claudeDir : config.ompDir;
    for (const dir of await readdir(base, { withFileTypes: true }).catch(() => [])) {
      if (!dir.isDirectory()) continue;
      const files = await readdir(join(base, dir.name), { withFileTypes: true });
      const file = files.find(f => f.isFile() && (match[1] === "claude" ? f.name === `${match[2]}.jsonl` : f.name.endsWith(`_${match[2]}.jsonl`)));
      if (file) { ref = await describe(join(base, dir.name, file.name), match[1] as "claude" | "omp", null); break; }
    }
  }
  if (!ref) throw new Error("Conversation not found");
  ref = await describe(ref.path, ref.agent, ref.cwd);
  if (![before, after].every(n => Number.isInteger(n) && n >= 0)) throw new Error("Slice bounds must be nonnegative integers");
  const index = match[3] === undefined ? Math.max(0, ref.messageCount - 6) : Number(match[3]);
  const start = match[3] === undefined ? index : Math.max(0, index - Math.min(before, 19));
  const end = match[3] === undefined ? ref.messageCount : Math.min(ref.messageCount, index + Math.min(after, 19) + 1, start + 20);
  if (index >= ref.messageCount && ref.messageCount) throw new Error("Cursor index out of range");
  const selected: Message[] = [];
  for await (const message of messages(ref)) {
    if (message.index >= end) break;
    if (message.index >= start) selected.push({ ...message, text: message.text.slice(0, LIMITS.messageChars) });
  }
  return { conversation: ref, messages: selected, prevCursor: start > 0 ? `${id}#${start - 1}` : null, nextCursor: end < ref.messageCount ? `${id}#${end}` : null };
}
