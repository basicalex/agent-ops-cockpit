import { afterEach, expect, test } from "bun:test";
import { getSnapshot, getOverviewSnapshot, resolveWorkspace, readPane, workspaceRepoRoots, setHerdrDependencies } from "../src/herdr";
import type { Runner } from "../src/contracts";

const fixture = await Bun.file(new URL("fixtures/w1/snapshot.json", import.meta.url)).json();
const redact = (text: string) => text.replaceAll("synthetic-secret", "[REDACTED]");
afterEach(() => setHerdrDependencies(null));

function inject(raw = fixture) {
  const calls: string[][] = [];
  const roots: string[] = [];
  const run: Runner = async argv => {
    calls.push(argv);
    return { stdout: argv[1] === "api" ? JSON.stringify(raw) : "screen synthetic-secret", stderr: "", code: 0, truncated: false };
  };
  setHerdrDependencies({ run, redact, repoRootFor: async cwd => {
    roots.push(cwd);
    return cwd.startsWith("/fixture/project-worktree") ? "/fixture/project-worktree" : cwd.startsWith("/fixture/project") ? "/fixture/project" : "/fixture/other";
  } });
  return { calls, roots };
}

test("normalizes panes, chooses majority repo root and retains worktrees", async () => {
  const { calls, roots } = inject();
  const snapshot = await getSnapshot();
  expect(snapshot.workspaces[0]!.repoRoot).toBe("/fixture/project");
  expect(snapshot.workspaces[0]!.tabs[0]!.panes[0]).toMatchObject({ sessionId: "session-one", agent: "claude", title: "Agent" });
  expect(snapshot.workspaces[0]!.tabs[1]!.panes[0]!.cwd).toBe("/fixture/project-worktree/subdir");
  expect(roots.filter(root => root === "/fixture/project")).toHaveLength(1);
  expect(calls).toEqual([["herdr", "api", "snapshot"]]);
  expect(await workspaceRepoRoots(snapshot.workspaces[0]!)).toEqual(["/fixture/project", "/fixture/project-worktree"]);
});

test("overview never queries git and workspace lookup rejects ambiguous basenames", async () => {
  const raw = structuredClone(fixture);
  raw.result.snapshot.workspaces[1].label = "Alternate";
  raw.result.snapshot.panes[3].cwd = "/elsewhere/project";
  const { roots } = inject(raw);
  await getOverviewSnapshot();
  expect(roots).toEqual([]);
  setHerdrDependencies({ run: async () => ({ stdout: JSON.stringify(raw), stderr: "", code: 0, truncated: false }), redact, repoRootFor: async cwd => cwd === "/elsewhere/project" ? cwd : "/fixture/project" });
  expect((await resolveWorkspace("w1")).workspaceId).toBe("w1");
  expect((await resolveWorkspace("ALTERNATE")).workspaceId).toBe("w2");
  // Label and repo aliases both match, so a duplicate basename must not pick arbitrarily.
  await expect(resolveWorkspace("project")).rejects.toThrow("Ambiguous workspace");
  await expect(resolveWorkspace("missing")).rejects.toThrow("Candidates: Project (w1), Alternate (w2)");
});

test("pane reads are bounded and redacted", async () => {
  const { calls } = inject();
  expect(await readPane("p1", { lines: 40 })).toBe("screen [REDACTED]");
  expect(calls[0]).toEqual(["herdr", "pane", "read", "p1", "--source", "recent", "--lines", "40", "--format", "text"]);
  await expect(readPane("p1", { lines: 201 })).rejects.toThrow("lines must be");
});
