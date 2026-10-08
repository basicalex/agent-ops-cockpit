import { afterEach, expect, test } from "bun:test";
import { mkdtemp, mkdir, readFile, realpath, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import type { ExecResult, Workspace } from "../src/contracts";
import { createAgents, type AgentRecord } from "../src/agents";
import { isControlAllowed, runControl, setControlRunner, shellQuote, type ControlScope } from "../src/control";
import { createTools } from "../src/tools";
import { z } from "zod";

const dirs: string[] = [];
afterEach(async () => {
  setControlRunner(null);
  await Promise.all(dirs.splice(0).map(dir => rm(dir, { recursive: true, force: true })));
});
const ok = (stdout = ""): ExecResult => ({ stdout, stderr: "", code: 0, truncated: false });
async function fixture() {
  const dir = await mkdtemp(join(tmpdir(), "aoc-live-agents-")); dirs.push(dir);
  const stateDir = join(dir, "state"), repoRoot = join(dir, "repo");
  await mkdir(join(repoRoot, ".git"), { recursive: true });
  await writeFile(join(repoRoot, ".git/config"), '[remote "origin"]\n\turl = https://github.com/alex/project.git\n');
  const workspace: Workspace = { workspaceId: "w1", label: "Voice", number: 1, agentStatus: "idle", focused: false, repoRoot, tabs: [] };
  const calls: string[][] = [], jobs: (() => Promise<void>)[] = [], timeouts: number[] = [];
  const branches = new Map<string, string>(), panes: { pane_id: string; tab_id: string; agent_status: string }[] = [];
  const state = { commits: "abc123 Fix task\n", dirty: "", screen: "agent screen", blocked: false, base: "main", missingDefault: false, branchOverride: "", failLaunch: false };
  const agents = createAgents({ stateDir, resolveWorkspace: async () => workspace, schedule: job => { jobs.push(job); }, sleep: async () => {},
    run: async (argv, options, scope) => {
      expect(isControlAllowed(argv, scope)).toBe(true);
      calls.push([...argv]); timeouts.push(options.timeoutMs!);
      if (argv[0] === "git") {
        const [root, sub, ...args] = argv.slice(2);
        if (sub === "symbolic-ref") return args[0] === "--short" ? ok(state.branchOverride || branches.get(root!)!)
          : state.missingDefault ? { ...ok(), code: 128 } : ok(`refs/remotes/origin/${state.base}\n`);
        if (sub === "worktree") { branches.set(args[3]!, args[2]!); await mkdir(args[3]!, { recursive: true }); return ok(); }
        if (sub === "status") return ok(state.dirty);
        if (sub === "log") return ok(state.commits);
        if (sub === "rev-parse") return ok(".git\n");
        return ok();
      }
      if (argv[0] === "gh") return ok("https://github.com/alex/project/pull/42\n");
      if (argv[1] === "tab" && argv[2] === "create") {
        const n = panes.length + 1;
        panes.push({ pane_id: `p${n}`, tab_id: `t${n}`, agent_status: "idle" });
        return ok(JSON.stringify({ result: { root_pane: { pane_id: `p${n}` }, tab: { tab_id: `t${n}` } } }));
      }
      if (argv[2] === "list") return ok(JSON.stringify({ result: { panes } }));
      if (argv[2] === "read") return ok(state.screen);
      if (argv[2] === "prompt" && state.blocked) return { ...ok(), code: 1, stderr: "agent_blocked" };
      if (argv[2] === "run" && state.failLaunch) return { ...ok(), code: 1 };
      return ok();
    },
  });
  return { dir, stateDir, repoRoot, workspace, calls, jobs, timeouts, panes, state, agents };
}

test("unknown ids never reach a control command", async () => {
  const f = await fixture();
  for (const operation of [() => f.agents.read("other"), () => f.agents.send("other", "task"), () => f.agents.answer("other", { keys: ["Enter"] }), () => f.agents.stop("other"), () => f.agents.openPr("other", "Title", "Body")]) {
    await expect(operation()).rejects.toThrow("not in the registry");
  }
  expect(f.calls).toEqual([]);
});

test("kill switch refuses every control tool except list, including background trust", async () => {
  const f = await fixture(), a = await f.agents.start("w1", "Fix a task", "claude");
  await writeFile(join(f.stateDir, "agents.disabled"), "");
  const before = f.calls.length;
  for (const operation of [() => f.agents.start("w1", "More work", "omp"), () => f.agents.read(a.id), () => f.agents.send(a.id, "Continue"), () => f.agents.answer(a.id, { text: "yes" }), () => f.agents.stop(a.id), () => f.agents.openPr(a.id, "Fix", "Details"), () => f.jobs[0]!()]) {
    await expect(operation()).rejects.toThrow("agent control is disabled");
  }
  expect(f.calls.length).toBe(before);
  expect((await f.agents.list())[0]!.status).toBe("idle");
});

test("six running agents is the limit; stopping keeps the worktree and frees a slot", async () => {
  const f = await fixture();
  const started = [];
  for (let i = 0; i < 6; i++) started.push(await f.agents.start("w1", `Task ${i}`, "omp"));
  const before = f.calls.length;
  await expect(f.agents.start("w1", "Seventh", "omp")).rejects.toThrow("At most 6");
  expect(f.calls.length).toBe(before);
  await f.agents.stop(started[0]!.id);
  expect((await stat(started[0]!.worktree)).isDirectory()).toBe(true);
  await f.agents.start("w1", "Replacement", "omp");
  const listed = await f.agents.list();
  expect(listed.filter(a => a.status === "stopped").map(a => a.id)).toEqual([started[0]!.id]);
  expect(listed.filter(a => !a.stoppedAt).length).toBe(6);
});

test("concurrent starts cannot exceed the limit or lose registry entries", async () => {
  const f = await fixture();
  const attempts = await Promise.allSettled([f.agents.start("w1", "First", "omp"), f.agents.start("w1", "Second", "omp")]);
  expect(attempts.map(a => a.status)).toEqual(["fulfilled", "rejected"]);
  expect((attempts[1] as PromiseRejectedResult).reason.message).toContain("busy");
  expect((await f.agents.list()).length).toBe(1);
});

test("start isolates the task and persists ownership before launch; trust runs later", async () => {
  const f = await fixture(), task = "Fix login; $(touch should-not-run) 'quoted'";
  const a = await f.agents.start("Voice", task, "claude");
  expect(a.id).toMatch(/^[a-z0-9]{6}$/);
  expect(a.branch).toMatch(/^aoc\/live-[a-z0-9-]{1,30}-[a-z0-9]{6}$/);
  expect(a.worktree).toBe(join(f.stateDir, "worktrees", a.id));
  expect(a.tabLabel).toBe(`live-${a.id}`);
  expect(a.status).toBe("starting");
  expect(f.calls).toEqual([
    ["git", "-C", f.repoRoot, "symbolic-ref", "refs/remotes/origin/HEAD"],
    ["git", "-C", f.repoRoot, "fetch", "origin"],
    ["git", "-C", f.repoRoot, "worktree", "add", "-b", a.branch, a.worktree, "origin/main"],
    ["herdr", "tab", "create", "--workspace", "w1", "--cwd", a.worktree, "--label", a.tabLabel, "--no-focus"],
    ["herdr", "pane", "run", "p1", "bash " + shellQuote(join(f.stateDir, "agents", a.id, "launch.sh"))],
  ]);
  expect(f.timeouts.every(ms => ms > 0 && ms <= 10_000)).toBe(true);
  const dir = join(f.stateDir, "agents", a.id), taskFile = join(dir, "task.md"), launchFile = join(dir, "launch.sh");
  expect(await readFile(taskFile, "utf8")).toContain(task);
  for (const file of [taskFile, launchFile, join(f.stateDir, "agents.json"), join(f.stateDir, "audit.jsonl")]) expect((await stat(file)).mode & 0o777).toBe(0o600);
  for (const path of [f.stateDir, join(f.stateDir, "agents"), dir, join(dir, "gh-config")]) expect((await stat(path)).mode & 0o777).toBe(0o700);
  const stored: AgentRecord[] = JSON.parse(await readFile(join(f.stateDir, "agents.json"), "utf8"));
  expect(stored[0]).toMatchObject({ id: a.id, tabId: "t1", paneId: "p1", branch: a.branch });
  f.state.screen = "Yes, I trust this folder";
  await f.jobs[0]!();
  expect(f.calls.slice(-2)).toEqual([["herdr", "pane", "send-keys", "p1", "Down"], ["herdr", "pane", "send-keys", "p1", "Enter"]]);
});

test("origin/HEAD absence falls back to main", async () => {
  const f = await fixture(); f.state.missingDefault = true;
  await f.agents.start("w1", "Fix the code", "omp");
  expect(f.calls[2]!.at(-1)).toBe("origin/main");
  expect(f.jobs).toEqual([]);
});

test("failed pane launch still leaves an owned tab that stop can close", async () => {
  const f = await fixture(); f.state.failLaunch = true;
  await expect(f.agents.start("w1", "Fail launch", "omp")).rejects.toThrow("command failed");
  const [a] = await f.agents.list();
  await f.agents.stop(a!.id);
  expect(f.calls.at(-1)).toEqual(["herdr", "tab", "close", "t1"]);
});

test("send reports blocked questions and audit stores only lengths and hashes", async () => {
  const f = await fixture(), text = "Please fix the private task details";
  const a = await f.agents.start("w1", text, "omp");
  f.state.blocked = true;
  await expect(f.agents.send(a.id, text)).rejects.toThrow("use agent_answer");
  const source = await readFile(join(f.stateDir, "audit.jsonl"), "utf8");
  expect(source).not.toContain(text);
  const rows = source.trim().split("\n").map(line => JSON.parse(line));
  for (const row of rows) expect(row).toMatchObject({ id: a.id, workspace: "w1", textLength: text.length, textSha256: createHash("sha256").update(text).digest("hex") });
  expect(rows.map(row => row.tool)).toEqual(["agent_start", "agent_send"]);
});

test("answers validate keys, refuse working agents and map Escape to herdr esc", async () => {
  const f = await fixture(), a = await f.agents.start("w1", "Answer a question", "omp");
  for (const keys of [[], ["ctrl+c"], ["0"], Array(11).fill("Enter")]) await expect(f.agents.answer(a.id, { keys })).rejects.toThrow("allowed answer keys");
  await expect(f.agents.answer(a.id, { text: "x".repeat(501) })).rejects.toThrow("1–500");
  f.panes[0]!.agent_status = "working";
  await expect(f.agents.answer(a.id, { keys: ["Enter"] })).rejects.toThrow("Agent is working");
  f.panes[0]!.agent_status = "blocked";
  await f.agents.answer(a.id, { keys: ["Escape", "Down", "2", "Enter"] });
  expect(f.calls.at(-1)).toEqual(["herdr", "pane", "send-keys", "p1", "esc", "Down", "2", "Enter"]);
  await f.agents.answer(a.id, { text: "Use this choice" });
  expect(f.calls.slice(-2)).toEqual([["herdr", "pane", "send-text", "p1", "Use this choice"], ["herdr", "pane", "send-keys", "p1", "Enter"]]);
});

test("read masks secrets, caps reports, reports progress and reads stopped worktrees", async () => {
  const f = await fixture(), a = await f.agents.start("w1", "Read progress", "omp");
  const secret = "ghp_" + "A".repeat(36);
  f.state.screen = `Working with ${secret}`;
  f.state.dirty = " M src/file.ts\n?? new.ts\n";
  await writeFile(join(f.stateDir, "agents", a.id, "report.md"), `Report ${secret}\n` + "z".repeat(9000));
  const data = await f.agents.read(a.id, 200);
  expect(data.text).not.toContain(secret);
  expect(data.report).not.toContain(secret);
  expect(Buffer.byteLength(data.report!)).toBeLessThanOrEqual(8192);
  expect(data.worktree).toEqual({ branch: a.branch, base: "main", commitsAhead: 1, changedFiles: 2 });
  expect((await f.agents.list())[0]!.reportPresent).toBe(true);
  await expect(f.agents.read(a.id, 201)).rejects.toThrow("1–200");
  await f.agents.stop(a.id);
  expect((await f.agents.read(a.id)).text).toBe("");
  await expect(f.agents.send(a.id, "Continue")).rejects.toThrow("stopped");
});

test("openPr rejects empty, dirty and switched branches before pushing", async () => {
  const f = await fixture(), a = await f.agents.start("w1", "PR checks", "omp");
  f.state.commits = "";
  await expect(f.agents.openPr(a.id, "Fix", "Details")).rejects.toThrow("no commits ahead");
  f.state.commits = "abc Fix\n"; f.state.dirty = " M file.ts\n";
  await expect(f.agents.openPr(a.id, "Fix", "Details")).rejects.toThrow("must be clean");
  f.state.dirty = ""; f.state.branchOverride = "main";
  await expect(f.agents.openPr(a.id, "Fix", "Details")).rejects.toThrow("registered aoc/live-*");
  expect(f.calls.some(c => c[3] === "push" || c[0] === "gh")).toBe(false);
});

test("openPr pushes only the owned branch over SSH and records the PR URL", async () => {
  const f = await fixture(), a = await f.agents.start("w1", "PR task", "omp");
  f.state.base = "trunk";
  expect(await f.agents.openPr(a.id, "Fix task", "Tests passed")).toEqual({ id: a.id, prUrl: "https://github.com/alex/project/pull/42" });
  expect(f.calls.slice(-2)).toEqual([
    ["git", "-C", a.worktree, "push", "git@github.com:alex/project.git", a.branch],
    ["gh", "pr", "create", "-R", "alex/project", "--head", a.branch, "--base", "trunk", "--title=Fix task", "--body=Tests passed"],
  ]);
  expect((await f.agents.list())[0]!.prUrl).toBe("https://github.com/alex/project/pull/42");
  const audit = await readFile(join(f.stateDir, "audit.jsonl"), "utf8");
  expect(audit).not.toContain("Fix task"); expect(audit).not.toContain("Tests passed");
});

test("control allowlist rejects commands, repos, panes, branches and destinations outside its scope", async () => {
  const dir = "/tmp/agent-control-fixture", branch = "aoc/live-task-abc123", worktree = join(dir, "worktrees/abc123");
  const scope: ControlScope = { stateDir: dir, repoRoots: ["/fixture/repo", worktree], workspaceIds: ["w1"], paneIds: ["p1"], tabIds: ["t1"], branches: [branch], githubRepo: "alex/project", launchScripts: [join(dir, "agents/abc123/launch.sh")] };
  const rejected = [
    ["bash", "-c", "echo unsafe"], ["herdr", "agent", "send", "p1", "text"], ["herdr", "tab", "close", "user-tab"],
    ["herdr", "pane", "send-keys", "p2", "Enter"], ["herdr", "pane", "send-keys", "p1", "ctrl+c"],
    ["herdr", "pane", "run", "p1", "rm -rf /"], ["git", "-C", "/outside", "fetch", "origin"],
    ["git", "-C", "/fixture/repo", "config", "--get", "remote.origin.url"], ["git", "-C", "/fixture/repo", "push", "git@github.com:alex/project.git", "main"],
    ["git", "-C", worktree, "push", "https://github.com/alex/project", branch], ["git", "-C", worktree, "push", "git@github.com:other/project.git", branch],
    ["git", "-C", worktree, "push", "git@github.com:alex/project.git", branch, "--force"],
    ["git", "-C", "/fixture/repo", "worktree", "add", "-b", branch, "/tmp/outside", "origin/main"],
    ["git", "-C", worktree, "log", "--oneline", "--output=/tmp/file"],
    ["gh", "pr", "create", "-R", "alex/project", "--head", "main", "--base", "main", "--title=Fix", "--body=Body"],
  ];
  let reachedRunner = false;
  setControlRunner(async () => { reachedRunner = true; return ok(); });
  for (const argv of rejected) {
    expect(isControlAllowed(argv, scope)).toBe(false);
    await expect(runControl(argv, {}, scope)).rejects.toThrow("allowlist");
  }
  expect(reachedRunner).toBe(false);
  await runControl(["git", "-C", worktree, "push", "git@github.com:alex/project.git", branch], {}, scope);
  expect(reachedRunner).toBe(true);
});

test("agent tools expose lifecycle errors without leaking pane secrets", async () => {
  const f = await fixture(), tools = createTools({ agents: f.agents });
  const call = (name: string, input: unknown) => tools.find(t => t.name === name)!.execute(input);
  const started = await call("agent_start", { workspace: "w1", task: "MCP task", harness: "omp" });
  const content = z.object({ type: z.literal("text"), text: z.string() });
  const a = JSON.parse(content.parse(started.content[0]).text);
  f.state.screen = "ghp_" + "B".repeat(36);
  const read = await call("agent_read", { id: a.id });
  expect(JSON.parse(content.parse(read.content[0]).text).text).not.toContain(f.state.screen);
  expect((await call("agent_stop", { id: "unowned" })).isError).toBe(true);
  expect((await call("agent_answer", { id: a.id, input: { keys: ["ctrl+c"] } })).isError).toBe(true);
});

test("launch scripts remove GitHub credentials and replace inherited push settings for both harnesses", async () => {
  const f = await fixture(), bin = join(f.dir, "bin"), home = join(f.dir, "home");
  await mkdir(bin); await mkdir(home);
  const stub = '#!/usr/bin/env bun\nconst env=Object.fromEntries(Object.entries(process.env).filter(([key])=>key.startsWith("GIT_CONFIG_")||["GH_TOKEN","GITHUB_TOKEN","GH_ENTERPRISE_TOKEN","GITHUB_ENTERPRISE_TOKEN","GH_CONFIG_DIR","GIT_TERMINAL_PROMPT"].includes(key)));console.log(JSON.stringify({cwd:process.cwd(),args:process.argv.slice(2),env}));\n';
  for (const command of ["claude", "aoc-omp"]) await writeFile(join(bin, command), stub, { mode: 0o700 });
  for (const harness of ["claude", "omp"] as const) {
    const a = await f.agents.start("w1", "Launch safely", harness);
    const child = Bun.spawn(["bash", join(f.stateDir, "agents", a.id, "launch.sh")], {
      env: { PATH: `${bin}:${process.env.PATH}`, HOME: home, GH_TOKEN: "fake-token", GITHUB_TOKEN: "fake-token", GH_ENTERPRISE_TOKEN: "fake-token", GITHUB_ENTERPRISE_TOKEN: "fake-token", GIT_CONFIG_COUNT: "1", GIT_CONFIG_KEY_0: "url.unsafe.pushInsteadOf", GIT_CONFIG_VALUE_0: "https://github.com/", GIT_CONFIG_PARAMETERS: "'url.unsafe.pushInsteadOf=https://github.com/'" },
      stdout: "pipe", stderr: "pipe",
    });
    const [code, output, error] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    expect(code).toBe(0); expect(error).toBe("");
    const observed = z.object({ cwd: z.string(), args: z.array(z.string()), env: z.record(z.string()) }).parse(JSON.parse(output));
    expect(observed.cwd).toBe(await realpath(a.worktree));
    const env = observed.env;
    for (const key of ["GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN", "GIT_CONFIG_PARAMETERS"]) expect(env[key]).toBeUndefined();
    expect(env.GH_CONFIG_DIR).toBe(join(f.stateDir, "agents", a.id, "gh-config"));
    expect(env.GIT_TERMINAL_PROMPT).toBe("0"); expect(env.GIT_CONFIG_COUNT).toBe("3");
    for (const [i, url] of ["https://github.com/", "git@github.com:", "ssh://git@github.com/"].entries()) {
      expect(env[`GIT_CONFIG_KEY_${i}`]).toBe("url.aoc-push-blocked:///.pushInsteadOf");
      expect(env[`GIT_CONFIG_VALUE_${i}`]).toBe(url);
    }
    expect(observed.args.at(-1)).toBe(`Read ${join(f.stateDir, "agents", a.id, "task.md")} and execute the task exactly as written.`);
    if (harness === "claude") {
      expect(observed.args.slice(0, 3)).toEqual(["--model", "opus", "--session-id"]);
      expect(observed.args).toContain("Bash(git push:*)"); expect(observed.args).toContain("Bash(gh:*)");
    } else expect(observed.args.slice(0, 4)).toEqual(["--model", "openai-codex/gpt-6.1-sol", "--thinking", "high"]);
  }
});

test("control subprocesses cap output and time out with a fake read-only herdr", async () => {
  const f = await fixture(), bin = join(f.dir, "bin");
  await mkdir(bin);
  // This checks the OS subprocess deadline; fake timers cannot stop a real child.
  await writeFile(join(bin, "herdr"), `#!/usr/bin/env bun\nif(process.argv.includes("slow"))await Bun.sleep(2000);else process.stdout.write("x".repeat(1000));\n`, { mode: 0o700 });
  const oldPath = process.env.PATH;
  process.env.PATH = `${bin}:${oldPath}`;
  const allow: ControlScope = { stateDir: f.stateDir, repoRoots: [], workspaceIds: ["fast", "slow"] };
  try {
    const capped = await runControl(["herdr", "pane", "list", "--workspace", "fast"], { maxBytes: 16 }, allow);
    expect(capped).toMatchObject({ code: 0, stdout: "x".repeat(16), truncated: true });
    const timed = await runControl(["herdr", "pane", "list", "--workspace", "slow"], { timeoutMs: 50 }, allow);
    expect(timed).toMatchObject({ code: 124, stderr: "Agent control command timed out" });
  } finally { if (oldPath === undefined) delete process.env.PATH; else process.env.PATH = oldPath; }
});
