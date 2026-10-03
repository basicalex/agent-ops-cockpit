import { mkdtemp, mkdir, copyFile, writeFile, rm, realpath } from "node:fs/promises";
import { tmpdir, homedir } from "node:os";
import { existsSync } from "node:fs";
import { afterEach, expect, test } from "bun:test";
import { resolve } from "node:path";
import { ReadOnlyViolation, type ExecOptions } from "../src/contracts";

import { isAllowed, runReadOnly, setRunner } from "../src/exec";
const journalFixture = resolve(import.meta.dir, "fixtures/w2/aoc-journal");
const checkoutJournal = resolve(import.meta.dir, "../../bin/aoc-journal");
const journal = resolve(process.env.AOC_LIVE_JOURNAL_BIN ?? (existsSync(checkoutJournal) ? checkoutJournal : resolve(homedir(), ".local/bin/aoc-journal")));
afterEach(() => setRunner(null));

const allowed = [
  ["herdr", "api", "snapshot"], ["herdr", "pane", "read", "pane-1", "--source", "recent-unwrapped", "--lines", "20", "--format", "text"],
  ...["status", "log", "diff", "show", "rev-parse", "grep", "ls-files", "rev-list", "for-each-ref"].map(subcommand => ["git", subcommand]),
  ["git", "worktree", "list", "--porcelain"], ["git", "branch", "--show-current"], ["git", "cat-file", "-s", "HEAD"], ["git", "cat-file", "-t", "HEAD"],
  ...["issue", "pr"].flatMap(kind => ["view", "list"].map(action => ["gh", kind, action])),
  [journal, "state", "10", "--json", "--events", "0", "--repo", "owner/repo"],
];
for (const argv of allowed) test(`allows ${argv.join(" ")}`, () => expect(isAllowed(argv)).toBe(true));

const denied = [
  [], ["sh", "-c", "git status"], ["herdr", "pane", "send-text", "pane-1", "x"], ["herdr", "pane", "send-keys", "pane-1", "Enter"],
  ["herdr", "pane", "run", "pane-1"], ["herdr", "agent", "list"], ["herdr", "api", "snapshot", "extra"],
  ["herdr", "pane", "read", "--help"], ["herdr", "pane", "read", "p", "--source", "invalid"], ["herdr", "pane", "read", "p", "--lines", "-1"],
  ...["push", "commit", "checkout", "stash", "config", "fetch"].map(action => ["git", action]),
  ["git", "diff", "--output=x"], ["git", "diff", "--output", "x"], ["git", "grep", "-O"], ["git", "grep", "-Ovim"], ["git", "diff", "--no-index", "a", "b"], ["git", "grep", "--no-index", "x"],
  ["git", "diff", "--out=x"], ["git", "diff", "--o=x"], ["git", "grep", "--open=vim"], ["git", "grep", "-nOvim"],
  ["git", "log", "-o", "x"], ["git", "log", "-ox"], ["git", "-c", "foo=bar", "status"], ["git", "status", "-cfoo=bar"],
  ...["--ext-diff", "--textconv", "--exec=x", "--git-dir=x", "--work-tree=x", "--upload-pack=x", "--config=x", "--open-files-in-pager=vim"].map(flag => ["git", "diff", flag]),
  ["git", "worktree", "add", "/tmp/x"], ["git", "branch", "new"], ["git", "cat-file", "--batch"],
  ...["edit", "comment", "close", "reopen", "create", "delete"].map(action => ["gh", "issue", action]),
  ...["merge", "checkout", "review"].map(action => ["gh", "pr", action]), ["gh", "api", "repos/x/y"],
  ...["--method=POST", "-XPOST", "--field=x=y", "-fx=y", "--raw-field=x=y", "-Fx=y", "--input=x", "--web", "-w"].map(flag => ["gh", "issue", "view", "10", flag]),
  ["aoc-journal", "state", "10"], [journal, "post", "10"], [journal, "follow-up", "10"], [journal, "state", "0"], [journal, "state", "-1"],
  [journal, "state", "10", "--events", "-1"], [journal, "state", "10", "--events"], [journal, "state", "10", "--repo", "../bad/name"],
  [journal, "state", "10", "--json", "--json"], [journal, "state", "10", "--body", "x"], [journal, "state", "10", "extra"],
  ["git", "status", "bad\0argument"],
];
for (const argv of denied) test(`rejects ${argv.join(" ")}`, () => expect(isAllowed(argv)).toBe(false));

test("injected runner cannot bypass the allowlist", async () => {
  let called = false;
  setRunner(async () => { called = true; return { stdout: "", stderr: "", code: 0, truncated: false }; });
  await expect(runReadOnly(["git", "push"])).rejects.toBeInstanceOf(ReadOnlyViolation);
  expect(called).toBe(false);
});

test("git reads receive safety configuration and noninteractive environment", async () => {
  const calls: string[][] = [];
  setRunner(async (argv, opts) => {
    calls.push(argv);
    const prepared = opts as ExecOptions & { env: Record<string, string> };
    expect(prepared.env).toMatchObject({ GIT_OPTIONAL_LOCKS: "0", GIT_TERMINAL_PROMPT: "0", GIT_PAGER: "cat", PAGER: "cat", GH_PAGER: "cat", GH_PROMPT_DISABLED: "1", NO_COLOR: "1" });
    expect(opts.timeoutMs).toBe(10000);
    expect(opts.maxBytes).toBe(200000);
    return { stdout: "safe", stderr: "", code: 0, truncated: false };
  });
  for (const argv of [["git", "diff"], ["git", "show", "HEAD"], ["git", "log", "-p"], ["git", "status"]]) await runReadOnly(argv);
  expect(calls[0]).toEqual(["git", "-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "diff.external=", "-c", "core.pager=cat", "--no-pager", "diff", "--no-ext-diff", "--no-textconv"]);
  expect(calls[1]).toContain("--no-textconv");
  expect(calls[2]).toContain("--no-ext-diff");
  expect(calls[3]).not.toContain("--no-textconv");
});

test("real backend caps bytes and kills a timed-out process", async () => {
  // Isolate the module-load override from the rest of the test suite.
  const code = `import assert from "node:assert/strict";
    const path = ${JSON.stringify(resolve(import.meta.dir, "../src/exec.ts"))};
    const {runReadOnly} = await import(path);
    const journal = ${JSON.stringify(journalFixture)};
    const capped = await runReadOnly([journal, "state", "10", "--json"], {maxBytes: 32});
    assert.equal(Buffer.byteLength(capped.stdout), 32); assert.equal(capped.truncated, true); assert.equal(capped.code, 0);
    const unicode = await runReadOnly([journal, "state", "10", "--events", "998"], {maxBytes: 5});
    assert.equal(unicode.stdout, "\\u20ac"); assert.equal(unicode.truncated, true);
    const timed = await runReadOnly([journal, "state", "10", "--events", "999"], {timeoutMs: 50});
    assert.equal(timed.code, 124); assert.equal(timed.stderr, "Read-only command timed out");
    console.log("backend boundaries passed");`;
  const proc = Bun.spawn([process.execPath, "--eval", code], { env: { ...process.env, AOC_LIVE_JOURNAL_BIN: journalFixture }, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exit] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  expect(stderr).toBe("");
  expect(exit).toBe(0);
  expect(stdout).toBe("backend boundaries passed\n");
});

test("module startup resolves override, checkout, and installed journal paths", async () => {
  const dir = await realpath(await mkdtemp(resolve(tmpdir(), "aoc-live-resolver-")));
  const src = resolve(dir, "share/aoc/live/src");
  await mkdir(src, { recursive: true });
  await copyFile(resolve(import.meta.dir, "../src/exec.ts"), resolve(src, "exec.ts"));
  await copyFile(resolve(import.meta.dir, "../src/contracts.ts"), resolve(src, "contracts.ts"));
  const env: NodeJS.ProcessEnv = { ...process.env, HOME: dir };
  delete env.AOC_LIVE_JOURNAL_BIN;
  const installed = resolve(dir, ".local/bin/aoc-journal");
  const checkout = resolve(dir, "share/aoc/bin/aoc-journal");
  async function check(expected: string, override?: string) {
    const code = `const path = ${JSON.stringify(resolve(src, "exec.ts"))}; const {isAllowed} = await import(path); console.log(JSON.stringify([isAllowed([${JSON.stringify(expected)}, "state", "1", "--json"]), isAllowed(["aoc-journal", "state", "1"])]));`;
    const proc = Bun.spawn([process.execPath, "--eval", code], { env: override ? { ...env, AOC_LIVE_JOURNAL_BIN: override } : env, stdout: "pipe", stderr: "pipe" });
    expect(await new Response(proc.stdout).text()).toBe("[true,false]\n");
    expect(await proc.exited).toBe(0);
  }
  try {
    await check(installed);
    await mkdir(resolve(dir, "share/aoc/bin"));
    await writeFile(checkout, "synthetic fixture");
    await check(checkout);
    await check(journalFixture, journalFixture);
  } finally { await rm(dir, { recursive: true, force: true }); }
});
