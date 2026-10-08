import { createHash, randomBytes, randomUUID } from "node:crypto";
import { appendFile, chmod, mkdir, open, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { z } from "zod";
import { resolveWorkspace } from "./herdr";
import { redact } from "./redact";
import { answerKeys, isControlAllowed, liveBranch, runControl, shellQuote, validRef, type ControlScope } from "./control";

const identity = z.string().min(1).refine(value => !value.startsWith("-") && !value.includes("\0"));
const recordSchema = z.object({
  id: z.string().regex(/^[a-z0-9]{6}$/), workspaceId: identity, workspaceLabel: z.string(),
  repoRoot: z.string().refine(isAbsolute), worktree: z.string().refine(isAbsolute), branch: z.string().regex(liveBranch),
  harness: z.enum(["claude", "omp"]), tabId: identity, paneId: identity,
  createdAt: z.string(), stoppedAt: z.string().optional(), prUrl: z.string().optional(),
}).strict();
export type AgentRecord = z.infer<typeof recordSchema>;
export type AnswerInput = { keys: string[]; text?: never } | { text: string; keys?: never };
export type AgentStart = { id: string; branch: string; worktree: string; tabLabel: string; status: string };
export type AgentSummary = { branch: string; base: string; commitsAhead: number; changedFiles: number };
export type AgentListing = AgentRecord & { status: string; reportPresent: boolean };
export interface Agents {
  start(workspaceRef: string, task: string, harness?: "claude" | "omp"): Promise<AgentStart>;
  list(): Promise<AgentListing[]>;
  read(id: string, lines?: number): Promise<{ id: string; text: string; worktree: AgentSummary; report: string | null }>;
  send(id: string, text: string): Promise<{ id: string; sent: boolean }>;
  answer(id: string, input: AnswerInput): Promise<{ id: string; answered: boolean }>;
  stop(id: string): Promise<{ id: string; status: string }>;
  openPr(id: string, title: string, body: string): Promise<{ id: string; prUrl: string }>;
}
export function agentStateDir(env = process.env): string {
  return resolve(env.AOC_LIVE_STATE_DIR || join(env.XDG_STATE_HOME || join(homedir(), ".local/state"), "aoc/live"));
}

export function launchScript(agent: Pick<AgentRecord, "id" | "worktree" | "harness">, stateDir: string, sessionId: string): string {
  const dir = join(stateDir, "agents", agent.id);
  const prompt = `Read ${join(dir, "task.md")} and execute the task exactly as written.`;
  let script = "#!/usr/bin/env bash\nset -euo pipefail\numask 077\nunset GH_TOKEN GITHUB_TOKEN GH_ENTERPRISE_TOKEN GITHUB_ENTERPRISE_TOKEN\n";
  script += "for key in ${!GIT_CONFIG_@}; do unset \"$key\"; done\nunset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR\n";
  script += `export GH_CONFIG_DIR=${shellQuote(join(dir, "gh-config"))}\nexport GIT_TERMINAL_PROMPT=0\nexport GIT_CONFIG_COUNT=3\n`;
  for (const [i, url] of ["https://github.com/", "git@github.com:", "ssh://git@github.com/"].entries()) {
    script += `export GIT_CONFIG_KEY_${i}='url.aoc-push-blocked:///.pushInsteadOf'\nexport GIT_CONFIG_VALUE_${i}=${shellQuote(url)}\n`;
  }
  script += `cd ${shellQuote(agent.worktree)}\n`;
  if (agent.harness === "claude") {
    script += 'set --\nif [[ -f "$HOME/.config/aoc/communication-contract.md" ]]; then\n  set -- --append-system-prompt "$(cat "$HOME/.config/aoc/communication-contract.md")"\nfi\n';
    script += `exec claude --model opus --session-id ${shellQuote(sessionId)} --permission-mode bypassPermissions --disallowedTools 'Bash(git push:*)' 'Bash(gh:*)' "$@" ${shellQuote(prompt)}\n`;
  } else script += `exec aoc-omp --model openai-codex/gpt-6.1-sol --thinking high ${shellQuote(prompt)}\n`;
  return script;
}

type Dependencies = {
  stateDir: string; run: typeof runControl; resolveWorkspace: typeof resolveWorkspace;
  schedule: (job: () => Promise<void>) => void; sleep: (ms: number) => Promise<void>;
};
const busy = new Set<string>();
export function createAgents(overrides: Partial<Dependencies> = {}): Agents {
  const deps: Dependencies = {
    stateDir: agentStateDir(), run: runControl, resolveWorkspace,
    schedule: job => { setTimeout(() => { void job().catch(() => {}); }, 0).unref(); },
    sleep: ms => Bun.sleep(ms), ...overrides,
  };
  const stateDir = resolve(deps.stateDir), registryFile = join(stateDir, "agents.json");
  async function privateDir(path: string) {
    await mkdir(path, { recursive: true, mode: 0o700 });
    await chmod(path, 0o700);
  }
  async function enabled() {
    if (await stat(join(stateDir, "agents.disabled")).then(() => true, error => {
      if (error.code === "ENOENT") return false;
      throw error;
    })) throw new Error("agent control is disabled");
  }
  async function registry(): Promise<AgentRecord[]> {
    let source: string;
    try { source = await readFile(registryFile, "utf8"); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return []; throw error; }
    const parsed = z.array(recordSchema).safeParse(JSON.parse(source));
    if (!parsed.success || new Set(parsed.data.map(a => a.id)).size !== parsed.data.length
      || parsed.data.some(a => a.worktree !== join(stateDir, "worktrees", a.id))) throw new Error("Invalid agent registry");
    return parsed.data;
  }
  async function save(records: AgentRecord[]) {
    await privateDir(stateDir);
    const temp = `${registryFile}.${randomUUID()}.tmp`;
    try {
      await writeFile(temp, JSON.stringify(records) + "\n", { mode: 0o600, flag: "wx" });
      await rename(temp, registryFile);
    } finally { await rm(temp, { force: true }); }
  }
  async function audit(tool: string, id: string | null, workspace: string | null, text = "") {
    await privateDir(stateDir);
    const file = join(stateDir, "audit.jsonl");
    await appendFile(file, JSON.stringify({ at: new Date().toISOString(), tool, id, workspace, textLength: text.length, textSha256: createHash("sha256").update(text).digest("hex") }) + "\n", { mode: 0o600 });
    await chmod(file, 0o600);
  }
  async function locked<T>(operation: () => Promise<T>): Promise<T> {
    if (busy.has(stateDir)) throw new Error("Agent control is busy; try again");
    busy.add(stateDir);
    try { return await operation(); } finally { busy.delete(stateDir); }
  }
  function scope(agent: AgentRecord): ControlScope {
    return { stateDir, repoRoots: [agent.repoRoot, agent.worktree], workspaceIds: [agent.workspaceId], paneIds: [agent.paneId], tabIds: [agent.tabId], branches: [agent.branch], launchScripts: [join(stateDir, "agents", agent.id, "launch.sh")] };
  }
  async function command(argv: string[], allow: ControlScope, deadline?: number) {
    await enabled();
    if (!isControlAllowed(argv, allow)) throw new Error("Command is not on the agent control allowlist");
    const remaining = deadline === undefined ? 10_000 : Math.min(10_000, deadline - Date.now());
    if (remaining <= 0) throw new Error("Agent start timed out");
    const result = await deps.run(argv, { timeoutMs: remaining, maxBytes: 200_000 }, allow);
    if (result.code !== 0) {
      if (result.stderr.includes("agent_blocked") || result.stdout.includes("agent_blocked")) throw new Error("Agent is blocked; use agent_answer");
      throw new Error(`Agent control command failed (${result.code})`);
    }
    if (result.truncated) throw new Error("Agent control output was truncated");
    return result.stdout;
  }
  async function find(id: string, tool: string, text = "") {
    await enabled();
    const records = await registry(), agent = records.find(a => a.id === id);
    if (!agent) throw new Error("Agent id is not in the registry");
    await audit(tool, id, agent.workspaceId, text);
    return { records, agent };
  }
  async function panes(workspaceId: string, allow: ControlScope, ignoreDisabled = false) {
    const argv = ["herdr", "pane", "list", "--workspace", workspaceId];
    const output = ignoreDisabled ? await deps.run(argv, { timeoutMs: 10_000, maxBytes: 200_000 }, allow) : null;
    if (output && (output.code !== 0 || output.truncated)) throw new Error("Agent status unavailable");
    const value = JSON.parse(output ? output.stdout : await command(argv, allow));
    return z.object({ result: z.object({ panes: z.array(z.object({ pane_id: identity, tab_id: identity, agent_status: z.string().optional() })) }) }).parse(value).result.panes;
  }
  async function defaultBranch(agent: AgentRecord, deadline?: number) {
    try {
      const ref = (await command(["git", "-C", agent.repoRoot, "symbolic-ref", "refs/remotes/origin/HEAD"], scope(agent), deadline)).trim();
      const branch = ref.replace(/^refs\/remotes\/origin\//, "");
      if (!ref.startsWith("refs/remotes/origin/") || !validRef(branch)) throw new Error("Invalid origin default branch");
      return branch;
    } catch (error) {
      // A missing origin/HEAD is common in older clones.
      if (error instanceof Error && /^Agent control command failed \((?:1|128)\)$/.test(error.message)) return "main";
      throw error;
    }
  }
  async function summary(agent: AgentRecord) {
    const allow = scope(agent), base = await defaultBranch(agent);
    const branch = (await command(["git", "-C", agent.worktree, "symbolic-ref", "--short", "HEAD"], allow)).trim();
    const commits = (await command(["git", "-C", agent.worktree, "log", "--oneline", `origin/${base}..HEAD`], allow)).trim();
    const changes = (await command(["git", "-C", agent.worktree, "status", "--porcelain"], allow)).trim();
    return { branch, base, commitsAhead: commits ? commits.split("\n").length : 0, changedFiles: changes ? changes.split("\n").length : 0 };
  }
  async function answerTrust(id: string) {
    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline) {
      await enabled();
      const agent = (await registry()).find(a => a.id === id);
      if (!agent || agent.stoppedAt) return;
      const allow = scope(agent);
      const text = await command(["herdr", "pane", "read", agent.paneId, "--source", "recent", "--lines", "40", "--format", "text"], allow);
      if (text.includes("Yes, I trust this folder")) {
        await locked(async () => {
          const current = (await registry()).find(a => a.id === id);
          if (!current || current.stoppedAt) return;
          await command(["herdr", "pane", "send-keys", current.paneId, "Down"], scope(current));
          await command(["herdr", "pane", "send-keys", current.paneId, "Enter"], scope(current));
        });
        return;
      }
      const status = (await panes(agent.workspaceId, allow)).find(p => p.pane_id === agent.paneId && p.tab_id === agent.tabId)?.agent_status;
      if (status === "working" || status === "done") return;
      await deps.sleep(500);
    }
  }
  return {
    async start(workspaceRef: string, task: string, harness: "claude" | "omp" = "omp") {
      if (!task.trim() || task.includes("\0") || !["claude", "omp"].includes(harness)) throw new Error("Invalid agent task or harness");
      const deadline = Date.now() + 18_000;
      return locked(async () => {
        await enabled();
        const records = await registry();
        if (records.filter(a => !a.stoppedAt).length >= 6) throw new Error("At most 6 running agents are allowed");
        let timer: ReturnType<typeof setTimeout> | undefined;
        const workspace = await Promise.race([
          deps.resolveWorkspace(workspaceRef),
          new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("Agent start timed out")), Math.max(1, deadline - Date.now())); }),
        ]).finally(() => clearTimeout(timer));
        if (!workspace.repoRoot || !isAbsolute(workspace.repoRoot) || !identity.safeParse(workspace.workspaceId).success) throw new Error("Workspace has no available git repository");
        const id = randomBytes(3).toString("hex");
        if (records.some(a => a.id === id)) throw new Error("Agent id collision; try again");
        const slug = task.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 30).replace(/-$/, "") || "task";
        const agent: AgentRecord = { id, workspaceId: workspace.workspaceId, workspaceLabel: workspace.label, repoRoot: workspace.repoRoot,
          worktree: join(stateDir, "worktrees", id), branch: `aoc/live-${slug}-${id}`, harness, tabId: "pending", paneId: "pending", createdAt: new Date().toISOString() };
        await audit("agent_start", id, workspace.workspaceId, task);
        const base = await defaultBranch(agent, deadline), allow = scope(agent);
        await command(["git", "-C", agent.repoRoot, "fetch", "origin"], allow, deadline);
        await privateDir(stateDir);
        await privateDir(join(stateDir, "worktrees"));
        await command(["git", "-C", agent.repoRoot, "worktree", "add", "-b", agent.branch, agent.worktree, `origin/${base}`], allow, deadline);
        await privateDir(join(stateDir, "agents"));
        const dir = join(stateDir, "agents", id);
        await mkdir(dir, { mode: 0o700 });
        await privateDir(join(dir, "gh-config"));
        const report = join(dir, "report.md"), taskFile = join(dir, "task.md"), launchFile = join(dir, "launch.sh");
        const packet = `Task from Alex via voice\n\n${task}\n\nRules:\nYou own this task end to end in this worktree.\nDo it yourself. Do not spawn panes, tabs or other agents. Do not use herdr-orchestrate.\nCommit on ${agent.branch} with plain messages and no co-author lines.\nYou cannot push or open PRs; aoc-live does that.\nRun the project's tests.\nWhen done, write a short report to ${report}: what changed, tests run and results, and open questions. Then stop.\nIf you need a decision, ask in the terminal and wait.\n`;
        await writeFile(taskFile, packet, { mode: 0o600, flag: "wx" });
        await writeFile(launchFile, launchScript(agent, stateDir, randomUUID()), { mode: 0o600, flag: "wx" });
        const created = JSON.parse(await command(["herdr", "tab", "create", "--workspace", agent.workspaceId, "--cwd", agent.worktree, "--label", `live-${id}`, "--no-focus"], allow, deadline)).result;
        agent.tabId = created?.tab_id ?? created?.tab?.tab_id;
        agent.paneId = created?.root_pane?.pane_id;
        if (!recordSchema.safeParse(agent).success) throw new Error("herdr tab create returned no valid tab or pane id");
        // Record ownership before typing into the new pane, including failed launches.
        records.push(agent);
        await save(records);
        await command(["herdr", "pane", "run", agent.paneId, "bash " + shellQuote(launchFile)], scope(agent), deadline);
        if (harness === "claude") deps.schedule(() => answerTrust(id));
        return { id, branch: agent.branch, worktree: agent.worktree, tabLabel: `live-${id}`, status: "starting" };
      });
    },
    async list() {
      const records = await registry();
      await audit("agent_list", null, null);
      const statuses = new Map<string, string>();
      for (const workspaceId of new Set(records.filter(a => !a.stoppedAt).map(a => a.workspaceId))) {
        try {
          for (const pane of await panes(workspaceId, { stateDir, repoRoots: [], workspaceIds: [workspaceId] }, true)) statuses.set(`${workspaceId}:${pane.tab_id}:${pane.pane_id}`, pane.agent_status ?? "unknown");
        } catch { /* The registry remains readable when herdr is unavailable. */ }
      }
      return Promise.all(records.map(async a => ({ ...a, status: a.stoppedAt ? "stopped" : statuses.get(`${a.workspaceId}:${a.tabId}:${a.paneId}`) ?? "unknown", reportPresent: await stat(join(stateDir, "agents", a.id, "report.md")).then(() => true, () => false) })));
    },
    async read(id: string, lines = 80) {
      const { agent } = await find(id, "agent_read");
      if (!Number.isInteger(lines) || lines < 1 || lines > 200) throw new Error("lines must be 1–200");
      const text = agent.stoppedAt ? "" : await command(["herdr", "pane", "read", agent.paneId, "--source", "recent", "--lines", String(lines), "--format", "text"], scope(agent));
      let report: string | null = null;
      try {
        const handle = await open(join(stateDir, "agents", id, "report.md"), "r");
        try {
          const buffer = Buffer.alloc(8192), { bytesRead } = await handle.read(buffer, 0, buffer.length, 0);
          report = redact(new TextDecoder().decode(buffer.subarray(0, bytesRead), { stream: true }));
        } finally { await handle.close(); }
      } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
      return { id, text: redact(text), worktree: await summary(agent), report };
    },
    async send(id: string, text: string) {
      return locked(async () => {
        const { agent } = await find(id, "agent_send", text);
        if (agent.stoppedAt) throw new Error("Agent is stopped");
        if (!text.trim() || text.length > 4000) throw new Error("text must be 1–4000 characters");
        await command(["herdr", "agent", "prompt", agent.paneId, text], scope(agent));
        return { id, sent: true };
      });
    },
    async answer(id: string, input: AnswerInput) {
      return locked(async () => {
        const { agent } = await find(id, "agent_answer", "text" in input ? input.text : JSON.stringify(input.keys));
        if (agent.stoppedAt) throw new Error("Agent is stopped");
        const parsed = z.union([
          z.object({ keys: z.array(z.enum(answerKeys)).min(1).max(10) }).strict(),
          z.object({ text: z.string().min(1).max(500) }).strict(),
        ]).safeParse(input);
        if (!parsed.success) throw new Error("Use 1–10 allowed answer keys or text of 1–500 characters");
        const allow = scope(agent), status = (await panes(agent.workspaceId, allow)).find(p => p.pane_id === agent.paneId && p.tab_id === agent.tabId)?.agent_status;
        // omp panes often report "unknown" because herdr cannot detect their state.
        if (status === "working") throw new Error("Agent is working; wait or use agent_send");
        if ("keys" in parsed.data) await command(["herdr", "pane", "send-keys", agent.paneId, ...parsed.data.keys.map(key => key === "Escape" ? "esc" : key)], allow);
        else {
          await command(["herdr", "pane", "send-text", agent.paneId, parsed.data.text], allow);
          await command(["herdr", "pane", "send-keys", agent.paneId, "Enter"], allow);
        }
        return { id, answered: true };
      });
    },
    async stop(id: string) {
      return locked(async () => {
        const { records, agent } = await find(id, "agent_stop");
        if (!agent.stoppedAt) {
          await command(["herdr", "tab", "close", agent.tabId], scope(agent));
          agent.stoppedAt = new Date().toISOString();
          await save(records);
        }
        return { id, status: "stopped" };
      });
    },
    async openPr(id: string, title: string, body: string) {
      return locked(async () => {
        const { records, agent } = await find(id, "agent_open_pr", JSON.stringify({ title, body }));
        if (!title.trim() || title.length > 120 || body.length > 4000 || title.includes("\0") || body.includes("\0")) throw new Error("Invalid PR title or body");
        const state = await summary(agent);
        if (state.branch !== agent.branch || !liveBranch.test(agent.branch)) throw new Error("Agent must be on its registered aoc/live-* branch");
        if (state.commitsAhead < 1) throw new Error("Agent branch has no commits ahead of origin");
        if (state.changedFiles) throw new Error("Agent worktree must be clean before opening a PR");
        const commonDir = (await command(["git", "-C", agent.repoRoot, "rev-parse", "--git-common-dir"], scope(agent))).trim();
        const config = await readFile(join(resolve(agent.repoRoot, commonDir), "config"), "utf8");
        let inOrigin = false, origin = "";
        for (const line of config.split("\n")) {
          if (/^\s*\[/.test(line)) { inOrigin = /^\s*\[remote\s+"origin"\]\s*(?:[#;].*)?$/.test(line); continue; }
          const match = inOrigin && /^\s*url\s*=\s*(.*?)\s*$/.exec(line);
          if (match) origin = match[1]!.replace(/^"(.*)"$/, "$1");
        }
        const match = /^(?:https:\/\/github\.com\/|git@github\.com:|ssh:\/\/git@github\.com\/)([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+?)(?:\.git)?\/?$/.exec(origin);
        if (!match || [match[1], match[2]].some(part => part === "." || part === "..")) throw new Error("Origin must be a GitHub HTTPS or SSH repository URL");
        const repo = `${match[1]}/${match[2]}`, allow = { ...scope(agent), githubRepo: repo };
        await command(["git", "-C", agent.worktree, "push", `git@github.com:${repo}.git`, agent.branch], allow);
        const url = (await command(["gh", "pr", "create", "-R", repo, "--head", agent.branch, "--base", state.base, `--title=${title}`, `--body=${body}`], allow)).trim();
        if (!/^https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\/pull\/\d+$/.test(url)) throw new Error("gh returned no PR URL");
        agent.prUrl = url;
        await save(records);
        return { id, prUrl: url };
      });
    },
  };
}
