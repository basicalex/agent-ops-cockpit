import { basename } from "node:path";
import { z } from "zod";
import type { Pane, Runner, Workspace } from "./contracts";
import { runReadOnly } from "./exec";
import { repoRootFor } from "./git";
import { redact } from "./redact";

type Dependencies = { run: Runner; repoRootFor: typeof repoRootFor; redact: typeof redact };
const defaults: Dependencies = { run: runReadOnly, repoRootFor, redact };
let deps = defaults;
export function setHerdrDependencies(overrides: Partial<Dependencies> | null): void {
  deps = overrides ? { ...defaults, ...overrides } : defaults;
}
const identity = { workspace_id: z.string(), agent_status: z.string().optional(), focused: z.boolean().optional() };
const snapshotSchema = z.object({ result: z.object({ snapshot: z.object({
  workspaces: z.array(z.object({ ...identity, label: z.string(), number: z.number() })),
  tabs: z.array(z.object({ ...identity, tab_id: z.string(), label: z.string(), number: z.number() })),
  panes: z.array(z.object({
    ...identity, pane_id: z.string(), tab_id: z.string(), cwd: z.string().optional(),
    foreground_cwd: z.string().optional(), agent: z.string().nullable().optional(),
    terminal_title: z.string().optional(), terminal_title_stripped: z.string().optional(),
    agent_session: z.object({ kind: z.string(), value: z.string(), agent: z.string().optional() }).nullable().optional(),
  })),
}) }) });
const text = (value: unknown): string => typeof value === "string" ? value : "";

// This path deliberately never queries git: workspace_overview stays inexpensive.
export async function getOverviewSnapshot(): Promise<{ workspaces: Workspace[] }> {
  const result = await deps.run(["herdr", "api", "snapshot"], {});
  if (result.code !== 0 || result.truncated) throw new Error("herdr snapshot unavailable or truncated");
  const parsed = snapshotSchema.safeParse(JSON.parse(result.stdout));
  if (!parsed.success) throw new Error("Invalid herdr snapshot");
  const raw = parsed.data.result.snapshot;
  const panes: Pane[] = raw.panes.map(p => ({
    paneId: text(p.pane_id), tabId: text(p.tab_id), workspaceId: text(p.workspace_id),
    agent: text(p.agent ?? p.agent_session?.agent) || null, agentStatus: text(p.agent_status) || "unknown",
    sessionId: p.agent_session?.kind === "id" ? text(p.agent_session.value) || null : null,
    cwd: text(p.foreground_cwd) || text(p.cwd), title: text(p.terminal_title_stripped ?? p.terminal_title), focused: p.focused === true,
  }));
  return { workspaces: raw.workspaces.map(w => ({
    workspaceId: text(w.workspace_id), label: text(w.label), number: Number(w.number),
    agentStatus: text(w.agent_status) || "unknown", focused: w.focused === true, repoRoot: null,
    tabs: raw.tabs.filter(t => t.workspace_id === w.workspace_id).map(t => ({
      tabId: text(t.tab_id), workspaceId: text(t.workspace_id), label: text(t.label), number: Number(t.number),
      agentStatus: text(t.agent_status) || "unknown", focused: t.focused === true,
      panes: panes.filter(p => p.tabId === t.tab_id && p.workspaceId === w.workspace_id),
    })),
  })) };
}

async function paneRoots(workspace: Workspace, cache = new Map<string, Promise<string | null>>()): Promise<string[]> {
  return (await Promise.all(workspace.tabs.flatMap(t => t.panes).map(p => {
    if (!p.cwd) return null;
    if (!cache.has(p.cwd)) cache.set(p.cwd, deps.repoRootFor(p.cwd).catch(() => null));
    return cache.get(p.cwd)!;
  }))).filter((root): root is string => root !== null);
}
export async function workspaceRepoRoots(workspace: Workspace): Promise<string[]> {
  return [...new Set(await paneRoots(workspace))];
}
export async function getSnapshot(): Promise<{ workspaces: Workspace[] }> {
  const snapshot = await getOverviewSnapshot();
  const cache = new Map<string, Promise<string | null>>();
  await Promise.all(snapshot.workspaces.map(async w => {
    const counts = new Map<string, number>();
    for (const root of await paneRoots(w, cache)) counts.set(root, (counts.get(root) ?? 0) + 1);
    w.repoRoot = [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))[0]?.[0] ?? null;
  }));
  return snapshot;
}
export async function resolveWorkspace(ref: string): Promise<Workspace> {
  const { workspaces } = await getSnapshot();
  const exact = workspaces.find(w => w.workspaceId === ref);
  if (exact) return exact;
  const needle = ref.toLowerCase();
  const matches = workspaces.filter(w => w.label.toLowerCase() === needle || (w.repoRoot && basename(w.repoRoot).toLowerCase() === needle));
  if (matches.length === 1) return matches[0]!;
  const candidates = (matches.length ? matches : workspaces).map(w => `${w.label} (${w.workspaceId})`).join(", ");
  throw new Error(`${matches.length ? "Ambiguous workspace" : "Workspace not found"}: ${ref}. Candidates: ${candidates || "none"}`);
}
export async function readPane(paneId: string, { lines = 40 }: { lines?: number } = {}): Promise<string> {
  if (!paneId || paneId.startsWith("-")) throw new Error("Invalid pane id");
  if (!Number.isInteger(lines) || lines < 1 || lines > 200) throw new Error("lines must be 1–200");
  const result = await deps.run(["herdr", "pane", "read", paneId, "--source", "recent", "--lines", String(lines), "--format", "text"], {});
  if (result.code !== 0) throw new Error("Pane read unavailable");
  return deps.redact(result.stdout) + (result.truncated ? "\n[truncated]" : "");
}
