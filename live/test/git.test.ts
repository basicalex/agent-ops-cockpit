import { afterEach, beforeEach, expect, test } from "bun:test";
import { mkdtemp, mkdir, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { LIMITS, ReadOnlyViolation } from "../src/contracts";

import { setRunner } from "../src/exec";
import { getGitState, getDiff, readRepoFile, searchCode, repoRootFor } from "../src/git";
let sandbox: string, root: string;
async function setupGit(...args: string[]) {
  const proc = Bun.spawn(["git", "-c", "core.hooksPath=/dev/null", "-c", "user.name=Synthetic Worker", "-c", "user.email=synthetic@example.invalid", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  if (code !== 0) throw new Error(stderr);
  return stdout;
}
beforeEach(async () => {
  setRunner(null);
  sandbox = await mkdtemp(resolve(tmpdir(), "aoc-live-w2-"));
  root = resolve(sandbox, "repo");
  await mkdir(root);
  root = await realpath(root);
  await setupGit("init", "-b", "main");
  await writeFile(resolve(root, "tracked.txt"), "hello world\nneedle plain\napi_key=synthetic-tracked-key\n");
  await writeFile(resolve(root, "staged.txt"), "base\n");
  await writeFile(resolve(root, ".env"), "needle PRIVATE_SYNTHETIC_ENV_CONTENT\n");
  await writeFile(resolve(root, "binary.dat"), Buffer.from([65, 0, 66]));
  await setupGit("add", "tracked.txt", "staged.txt", ".env", "binary.dat");
  await setupGit("commit", "-m", "Initial fixture");
});
afterEach(async () => { setRunner(null); await rm(sandbox, { recursive: true, force: true }); });

test("state reports upstream divergence, staged and unstaged counts, and capped untracked files", async () => {
  await setupGit("branch", "upstream");
  await setupGit("branch", "--set-upstream-to=upstream", "main");
  await writeFile(resolve(root, "staged.txt"), "base\ncommitted\n");
  await setupGit("add", "staged.txt");
  await setupGit("commit", "-m", "Second fixture");
  await writeFile(resolve(root, "staged.txt"), "base\ncommitted\nstaged\n");
  await setupGit("add", "staged.txt");
  await writeFile(resolve(root, "staged.txt"), "base\ncommitted\nstaged\nunstaged\n");
  for (let i = 0; i < 25; i++) await writeFile(resolve(root, `untracked-${i}.txt`), "fixture");
  const state = await getGitState(root);
  expect(state.branch).toBe("main");
  expect(state.head).toMatch(/^[0-9a-f]{40}$/);
  expect(state.headSubject).toBe("Second fixture");
  expect(state.upstream).toBe("upstream");
  expect([state.ahead, state.behind]).toEqual([1, 0]);
  expect(state.dirty).toBe(true);
  expect(state.staged).toContainEqual({ path: "staged.txt", status: "M", added: 1, removed: 0 });
  expect(state.unstaged).toContainEqual({ path: "staged.txt", status: "M", added: 1, removed: 0 });
  expect(state.untrackedTotal).toBe(25);
  expect(state.untracked).toHaveLength(LIMITS.untracked);
  expect(state.recentCommits.map(commit => commit.subject)).toEqual(["Second fixture", "Initial fixture"]);
  expect(state.worktrees).toEqual([{ path: root, branch: "main" }]);
  expect(await repoRootFor(root)).toBe(root);
  expect(await repoRootFor(sandbox)).toBeNull();
});

test("detached and unborn repositories have defined state", async () => {
  await setupGit("checkout", "--detach");
  expect((await getGitState(root, { commits: 0 })).branch).toBeNull();
  const empty = resolve(sandbox, "empty");
  await mkdir(empty);
  const saved = root;
  root = empty;
  await setupGit("init", "-b", "main");
  const state = await getGitState(empty);
  root = saved;
  expect(state.head).toBe("");
  expect(state.recentCommits).toEqual([]);
  expect(state.dirty).toBe(false);
});

test("diff redacts patches and never reads secret hunks", async () => {
  await writeFile(resolve(root, "tracked.txt"), "hello changed\nneedle plain\napi_key=synthetic-new-key\n");
  await writeFile(resolve(root, ".env"), "PRIVATE_SYNTHETIC_ENV_CHANGED\n");
  const diff = await getDiff({ root });
  expect(diff.summary.map(change => change.path).sort()).toEqual([".env", "tracked.txt"]);
  expect(diff.patch).toContain("hello changed");
  expect(diff.patch).toContain("api_key=[REDACTED]");
  expect(diff.patch).toContain("[Secret file omitted: .env]");
  expect(diff.patch).not.toContain("PRIVATE_SYNTHETIC_ENV");
  expect(diff.patch).not.toContain("synthetic-new-key");
  const capped = await getDiff({ root, maxBytes: 30 });
  expect(Buffer.byteLength(capped.patch)).toBeLessThanOrEqual(30);
  expect(capped.truncated).toBe(true);
});

test("diff supports staged changes, refs, and safe path filters", async () => {
  await writeFile(resolve(root, "staged.txt"), "base\nstaged change\n");
  await setupGit("add", "staged.txt");
  await writeFile(resolve(root, "staged.txt"), "base\nstaged change\nunstaged change\n");
  const staged = await getDiff({ root, target: "staged", paths: ["staged.txt"] });
  expect(staged.patch).toContain("+staged change");
  expect(staged.patch).not.toContain("+unstaged change");
  expect((await getDiff({ root, target: "HEAD" })).patch).toContain("+unstaged change");
  expect((await getDiff({ root, target: "HEAD..HEAD" })).summary).toEqual([]);
  await expect(getDiff({ root, target: "--output=x" })).rejects.toBeInstanceOf(ReadOnlyViolation);
  await expect(getDiff({ root, paths: ["../escape"] })).rejects.toBeInstanceOf(ReadOnlyViolation);
});

test("tracked files and historical refs are redacted with correct line windows", async () => {
  expect((await readRepoFile(root, "tracked.txt", { startLine: 2, endLine: 3 })).text).toBe("needle plain\napi_key=[REDACTED]");
  await writeFile(resolve(root, "tracked.txt"), "new worktree content\n");
  expect((await readRepoFile(root, "tracked.txt", { ref: "HEAD" })).text).toBe("hello world\nneedle plain\napi_key=[REDACTED]");
  expect((await readRepoFile(root, "binary.dat")).text).toBe("[Binary file omitted]");
  await writeFile(resolve(root, "large.txt"), Array.from({ length: 250 }, (_, i) => `line ${i + 1}`).join("\n") + "\n");
  await setupGit("add", "large.txt");
  const file = await readRepoFile(root, "large.txt", { endLine: 250 });
  expect(file.totalLines).toBe(250);
  expect(file.endLine).toBe(200);
  expect(file.text.split("\n")).toHaveLength(200);
  expect(file.truncated).toBe(true);
});

test("file reads reject untracked, secret, traversal, and symlink escape paths", async () => {
  await writeFile(resolve(root, "untracked.txt"), "synthetic");
  await writeFile(resolve(sandbox, "outside.txt"), "outside synthetic");
  await symlink(resolve(sandbox, "outside.txt"), resolve(root, "escape.txt"));
  await symlink(".env", resolve(root, "alias.txt"));
  await setupGit("add", "escape.txt", "alias.txt");
  for (const path of ["untracked.txt", ".env", "../outside.txt", "escape.txt", "alias.txt"]) await expect(readRepoFile(root, path)).rejects.toBeInstanceOf(ReadOnlyViolation);
  await writeFile(resolve(root, "tracked*.txt"), "untracked wildcard filename");
  await expect(readRepoFile(root, "tracked*.txt")).rejects.toBeInstanceOf(ReadOnlyViolation);
  await expect(readRepoFile(root, "tracked.txt", { ref: "--output=x" })).rejects.toBeInstanceOf(ReadOnlyViolation);
});

test("PEM redaction protects windows inside a block without moving later lines", async () => {
  await writeFile(resolve(root, "document.txt"), "before\n-----BEGIN " + "PRIVATE KEY-----\nSYNTHETIC_PRIVATE_KEY_DATA\n-----END PRIVATE KEY-----\nafter\n");
  await setupGit("add", "document.txt");
  expect((await readRepoFile(root, "document.txt", { startLine: 3, endLine: 3 })).text).toBe("[REDACTED]");
  expect((await searchCode(root, "SYNTHETIC_PRIVATE_KEY_DATA")).hits[0]?.text).toBe("[REDACTED]");
  expect((await readRepoFile(root, "document.txt", { startLine: 5, endLine: 5 })).text).toBe("after");
});

test("search excludes secret and binary files, handles literal and regex queries, and caps hits", async () => {
  await writeFile(resolve(root, "tracked.txt"), "needle first\nneedle second\napi_key=synthetic-search-key\n-F literal\n" + "x".repeat(400) + "\n");
  const found = await searchCode(root, "needle", { maxHits: 1 });
  expect(found.hits).toEqual([{ path: "tracked.txt", line: 1, text: "needle first" }]);
  expect(found.truncated).toBe(true);
  expect((await searchCode(root, "api_key")).hits[0]?.text).toBe("api_key=[REDACTED]");
  expect((await searchCode(root, "needle (first|second)", { regex: true, pathGlob: "*.txt" })).hits.map(hit => hit.line)).toEqual([1, 2]);
  expect((await searchCode(root, "-F")).hits[0]?.text).toBe("-F literal");
  expect((await searchCode(root, "x")).hits[0]?.text.length).toBe(300);
  expect((await searchCode(root, "not present")).hits).toEqual([]);
  expect((await searchCode(root, "needle", { maxHits: 0 })).truncated).toBe(true);
  expect((await searchCode(root, "needle", { pathGlob: ".env" })).hits).toEqual([]);
});

test("rename status preserves spaced paths and numstat counts", async () => {
  await setupGit("mv", "tracked.txt", "renamed file.txt");
  const state = await getGitState(root);
  expect(state.staged).toContainEqual({ path: "renamed file.txt", status: "R", added: 0, removed: 0 });
  const diff = await getDiff({ root, target: "staged" });
  expect(diff.summary[0]?.path).toBe("renamed file.txt");
  expect(diff.patch).toContain("rename to renamed file.txt");
});

test("renaming a secret never exposes its former contents", async () => {
  await setupGit("mv", ".env", "settings.txt");
  const diff = await getDiff({ root, target: "staged" });
  expect(diff.summary.map(change => change.path)).toEqual(["settings.txt"]);
  expect(diff.patch).toBe("[Secret file omitted: settings.txt]\n");
});

test("interior PEM hunks are omitted for worktree, staged, and committed diffs", async () => {
  const lines = ["-----BEGIN " + "PRIVATE KEY-----", ...Array.from({ length: 100 }, (_, i) => `SYNTHETIC_KEY_MATERIAL_${i}`), "-----END PRIVATE KEY-----", ""];
  await writeFile(resolve(root, "document.txt"), lines.join("\n"));
  await setupGit("add", "document.txt");
  await setupGit("commit", "-m", "Synthetic PEM fixture");
  lines[50] = "SYNTHETIC_CHANGED_KEY_MATERIAL";
  await writeFile(resolve(root, "document.txt"), lines.join("\n"));
  const rawHunk = await setupGit("diff", "--no-ext-diff", "--no-textconv", "--", "document.txt");
  expect(rawHunk).toContain("SYNTHETIC_CHANGED_KEY_MATERIAL");
  expect(rawHunk).not.toContain("BEGIN PRIVATE KEY");
  expect(rawHunk).not.toContain("END PRIVATE KEY");
  const worktree = await getDiff({ root });
  expect(worktree.summary).toContainEqual({ path: "document.txt", status: "M", added: 1, removed: 1 });
  expect(worktree.patch).toBe("[Secret file omitted: document.txt]\n");
  await setupGit("add", "document.txt");
  expect((await getDiff({ root, target: "staged" })).patch).toBe("[Secret file omitted: document.txt]\n");
  await setupGit("commit", "-m", "Synthetic interior change");
  expect((await getDiff({ root, target: "HEAD~1..HEAD" })).patch).toBe("[Secret file omitted: document.txt]\n");
});

test("deleted PEM files are omitted using index and historical baseline contents", async () => {
  await writeFile(resolve(root, "document.txt"), "-----BEGIN " + "RSA PRIVATE KEY-----\nSYNTHETIC_BASELINE_KEY_DATA\n-----END RSA PRIVATE KEY-----\n");
  await setupGit("add", "document.txt");
  await setupGit("commit", "-m", "Synthetic baseline PEM");
  await rm(resolve(root, "document.txt"));
  expect((await getDiff({ root })).patch).toBe("[Secret file omitted: document.txt]\n");
  await setupGit("add", "document.txt");
  expect((await getDiff({ root, target: "staged" })).patch).toBe("[Secret file omitted: document.txt]\n");
  await setupGit("commit", "-m", "Remove synthetic PEM");
  expect((await getDiff({ root, target: "HEAD~1..HEAD" })).patch).toBe("[Secret file omitted: document.txt]\n");
});

test("PEM inspection uses the three-dot merge base, not the endpoint files", async () => {
  await writeFile(resolve(root, "document.txt"), "-----BEGIN " + "PRIVATE KEY-----\nSYNTHETIC_MERGE_BASE_KEY\n-----END PRIVATE KEY-----\n");
  await setupGit("add", "document.txt");
  await setupGit("commit", "-m", "Synthetic PEM merge base");
  const base = (await setupGit("rev-parse", "HEAD")).trim();
  await setupGit("checkout", "-b", "left");
  await writeFile(resolve(root, "document.txt"), "left safe endpoint\n");
  await setupGit("add", "document.txt");
  await setupGit("commit", "-m", "Left removes synthetic PEM");
  await setupGit("checkout", "-b", "right", base);
  await writeFile(resolve(root, "document.txt"), "right safe endpoint\n");
  await setupGit("add", "document.txt");
  await setupGit("commit", "-m", "Right removes synthetic PEM");
  expect((await getDiff({ root, target: "left..right" })).patch).toContain("+right safe endpoint");
  expect((await getDiff({ root, target: "left...right" })).patch).toBe("[Secret file omitted: document.txt]\n");
});

test("oversized baseline inspection fails closed without suppressing safe patches", async () => {
  await writeFile(resolve(root, "large.txt"), "synthetic padding\n".repeat(13_000));
  await setupGit("add", "large.txt");
  await setupGit("commit", "-m", "Large synthetic baseline");
  await writeFile(resolve(root, "large.txt"), "safe replacement\n");
  await writeFile(resolve(root, "tracked.txt"), "safe sibling change\n");
  const diff = await getDiff({ root });
  expect(diff.patch).toContain("+safe sibling change");
  expect(diff.patch).toContain("[File patch omitted: large.txt (inspection exceeds output limit)]");
  expect(diff.patch).not.toContain("synthetic padding");
  expect(diff.truncated).toBe(true);
});

test("PEM introduced only in a range's new blob is omitted", async () => {
  await writeFile(resolve(root, "document.txt"), "safe baseline\n");
  await setupGit("add", "document.txt");
  await setupGit("commit", "-m", "Safe baseline");
  await writeFile(resolve(root, "document.txt"), "-----BEGIN " + "PRIVATE KEY-----\nSYNTHETIC_NEW_BLOB_KEY\n-----END PRIVATE KEY-----\n");
  await setupGit("add", "document.txt");
  await setupGit("commit", "-m", "Synthetic new-side PEM");
  expect((await getDiff({ root, target: "HEAD~1..HEAD" })).patch).toBe("[Secret file omitted: document.txt]\n");
});
