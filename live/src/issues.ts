import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";
import { z } from "zod";
import type { IssueComment, IssueState, IssueSummary } from "./contracts";
import { runReadOnly } from "./exec";
import { redact } from "./redact";

const checkoutJournal = resolve(import.meta.dir, "../../bin/aoc-journal");
const journalPath = resolve(process.env.AOC_LIVE_JOURNAL_BIN ?? (existsSync(checkoutJournal) ? checkoutJournal : resolve(homedir(), ".local/bin/aoc-journal")));
const nullableText = z.string().nullable();
const snapshotSchema = z.object({
  schema: z.literal("aoc.issue.state/v1"), issue: z.number().int().positive(), url: z.string(), title: z.string(),
  objective: z.string(), state: z.string(), labels: z.array(z.string()), closedAt: nullableText,
  blockers: z.array(z.string()), commits: z.array(z.string()), pull_requests: z.array(z.string()),
  completion: z.object({ complete: z.boolean(), url: nullableText, at: nullableText }),
  events: z.array(z.object({
    source: z.enum(["dispatch", "journal"]), event: z.string(), kind: nullableText, run_id: nullableText,
    seq: z.number().int().nullable(), at: nullableText, url: nullableText, author: nullableText,
    state: nullableText, change: nullableText, evidence: nullableText, next: nullableText, blockers: nullableText, summary: nullableText,
  })),
});
const commentsSchema = z.object({ comments: z.array(z.object({
  author: z.object({ login: z.string() }).nullable(), createdAt: z.string(), body: z.string(), url: z.string(),
})) });
const summariesSchema = z.array(z.object({
  number: z.number().int().positive(), title: z.string(), state: z.string(), labels: z.array(z.object({ name: z.string() })),
  updatedAt: z.string(), url: z.string(),
}));

function count(value: number | undefined, fallback: number): number {
  const result = value ?? fallback;
  if (!Number.isSafeInteger(result) || result < 0) throw new Error("Invalid issue result limit");
  return result;
}
async function json(root: string, argv: string[]): Promise<unknown> {
  const result = await runReadOnly(argv, { cwd: root });
  if (result.code !== 0) throw new Error(redact(result.stderr) || "Issue read failed");
  if (result.truncated) throw new Error("Issue JSON exceeds the output limit");
  return JSON.parse(result.stdout);
}
function redactStrings<T>(value: T): T;
function redactStrings(value: unknown): unknown {
  if (typeof value === "string") return redact(value);
  if (Array.isArray(value)) return value.map(redactStrings);
  if (value !== null && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, field]) => [key, redactStrings(field)]));
  return value;
}

export async function listIssues(root: string, opts: { state?: "open" | "closed" | "all"; label?: string; limit?: number } = {}): Promise<IssueSummary[]> {
  const limit = count(opts.limit, 20);
  const state = opts.state ?? "open";
  if (!["open", "closed", "all"].includes(state)) throw new Error("Invalid issue state");
  if (!limit) return [];
  const argv = ["gh", "issue", "list", "--json", "number,title,state,labels,updatedAt,url", "--limit", String(limit), "--state", state];
  if (opts.label !== undefined) argv.push("--label", opts.label);
  const rows = summariesSchema.parse(await json(root, argv));
  return rows.slice(0, limit).map(row => redactStrings({ number: row.number, title: row.title, state: row.state, labels: row.labels.map(label => label.name), updatedAt: row.updatedAt, url: row.url }));
}

export async function getIssueState(root: string, number: number, opts: { events?: number; comments?: number } = {}): Promise<IssueState> {
  if (!Number.isSafeInteger(number) || number < 1) throw new Error("Issue number must be a positive integer");
  const events = count(opts.events, 10), comments = count(opts.comments, 3);
  const [snapshot, response] = await Promise.all([
    json(root, [journalPath, "state", String(number), "--json", "--events", String(events)]),
    json(root, ["gh", "issue", "view", String(number), "--json", "comments"]),
  ]);
  const state = snapshotSchema.parse(snapshot);
  if (state.issue !== number) throw new Error("Unexpected issue snapshot number");
  const allComments = commentsSchema.parse(response).comments;
  const plain = allComments.filter(comment => !/^<!-- aoc-(?:dispatch|journal) .* -->\r?$/.test(comment.body.split("\n", 1)[0]!))
    .sort((a, b) => a.createdAt.localeCompare(b.createdAt));
  const recentComments: IssueComment[] = (comments ? plain.slice(-comments) : []).map(comment => ({
    author: redact(comment.author?.login ?? ""), createdAt: redact(comment.createdAt), body: redact(comment.body).slice(0, 1500), url: redact(comment.url),
  }));
  return { ...redactStrings(state), events: (events ? state.events.slice(-events) : []).map(event => redactStrings(event)), recentComments, commentTotal: allComments.length };
}
