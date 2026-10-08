import { isAbsolute, resolve, join } from "node:path";
import type { ExecOptions, ExecResult, Runner } from "./contracts";

export type ControlScope = {
  stateDir: string;
  repoRoots: string[];
  workspaceIds?: string[];
  paneIds?: string[];
  tabIds?: string[];
  branches?: string[];
  launchScripts?: string[];
  githubRepo?: string;
};
export const answerKeys = ["Enter", "Up", "Down", "Tab", "Escape", "1", "2", "3", "4", "5", "6", "7", "8", "9"] as const;
export const liveBranch = /^aoc\/live-[a-z0-9-]{1,30}-[a-z0-9]{6}$/;
export function validRef(value: string): boolean {
  return !!value && !value.startsWith("-") && !/[\s~^:?*\[\\\x00-\x1f\x7f]/.test(value)
    && !value.includes("..") && !value.includes("@{") && !value.includes("//")
    && value.split("/").every(part => !!part && !part.startsWith(".") && !part.endsWith(".") && !part.endsWith(".lock"));
}
export function shellQuote(value: string): string { return "'" + value.replaceAll("'", "'\\''") + "'"; }

export function isControlAllowed(argv: string[], scope: ControlScope): boolean {
  if (!argv.length || argv.some(arg => typeof arg !== "string" || arg.includes("\0"))) return false;
  const [command, ...a] = argv;
  if (command === "herdr") {
    if (a[0] === "tab" && a[1] === "create") {
      return a.length === 9 && a[2] === "--workspace" && !!scope.workspaceIds?.includes(a[3]!)
        && a[4] === "--cwd" && !!scope.repoRoots.includes(a[5]!) && a[5]!.startsWith(join(resolve(scope.stateDir), "worktrees") + "/")
        && a[6] === "--label" && /^live-[a-z0-9]{6}$/.test(a[7]!) && a[8] === "--no-focus";
    }
    if (a[0] === "tab" && a[1] === "close") return a.length === 3 && !!scope.tabIds?.includes(a[2]!);
    if (a[0] === "pane" && a[1] === "list") return a.length === 4 && a[2] === "--workspace" && !!scope.workspaceIds?.includes(a[3]!);
    if (!["pane", "agent"].includes(a[0]!) || !scope.paneIds?.includes(a[2]!)) return false;
    if (a[0] === "agent") return a[1] === "prompt" && a.length === 4 && a[3]!.length > 0 && a[3]!.length <= 4000;
    if (a[1] === "run") return a.length === 4 && !!scope.launchScripts?.some(path => a[3] === "bash " + shellQuote(path));
    if (a[1] === "send-text") return a.length === 4 && a[3]!.length > 0 && a[3]!.length <= 500;
    if (a[1] === "send-keys") return a.length >= 4 && a.length <= 13 && a.slice(3).every(key => [...answerKeys.filter(k => k !== "Escape"), "esc"].includes(key));
    if (a[1] === "read") return a.length === 9 && a[3] === "--source" && a[4] === "recent" && a[5] === "--lines"
      && /^[1-9]\d*$/.test(a[6]!) && Number(a[6]) <= 200 && a[7] === "--format" && a[8] === "text";
    return false;
  }
  if (command === "git") {
    if (a[0] !== "-C" || !isAbsolute(a[1] ?? "") || !scope.repoRoots.includes(a[1]!)) return false;
    const rest = a.slice(2);
    if (rest.join("\0") === "fetch\0origin") return true;
    if (rest[0] === "worktree") return rest.length === 6 && rest[1] === "add" && rest[2] === "-b"
      && liveBranch.test(rest[3]!) && !!scope.branches?.includes(rest[3]!)
      && resolve(rest[4]!) === join(resolve(scope.stateDir), "worktrees", rest[3]!.slice(-6))
      && rest[5]!.startsWith("origin/") && validRef(rest[5]!);
    if (rest[0] === "rev-parse") return rest.length === 2 && ["--git-common-dir", "HEAD"].includes(rest[1]!);
    if (rest[0] === "symbolic-ref") return rest.join("\0") === "symbolic-ref\0refs/remotes/origin/HEAD" || rest.join("\0") === "symbolic-ref\0--short\0HEAD";
    if (rest[0] === "status") return rest.join("\0") === "status\0--porcelain";
    if (rest[0] === "log") return rest.length === 3 && rest[1] === "--oneline" && /^origin\/.+\.\.HEAD$/.test(rest[2]!) && validRef(rest[2]!.slice(0, -6));
    if (rest[0] === "push") return rest.length === 3 && !!scope.githubRepo && rest[1] === `git@github.com:${scope.githubRepo}.git`
      && liveBranch.test(rest[2]!) && !!scope.branches?.includes(rest[2]!);
    return false;
  }
  if (command === "gh") return a.length === 10 && a[0] === "pr" && a[1] === "create" && a[2] === "-R"
    && /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(a[3]!) && a[3] === scope.githubRepo
    && a[4] === "--head" && liveBranch.test(a[5]!) && !!scope.branches?.includes(a[5]!)
    && a[6] === "--base" && validRef(a[7]!) && a[8]!.startsWith("--title=") && a[8]!.length > 8 && a[8]!.length <= 128
    && a[9]!.startsWith("--body=") && a[9]!.length <= 4007;
  return false;
}

let runner: Runner | null = null;
export function setControlRunner(fn: Runner | null): void { runner = fn; }
async function spawn(argv: string[], opts: ExecOptions & { env: Record<string, string | undefined> }): Promise<ExecResult> {
  const proc = Bun.spawn(argv, { cwd: opts.cwd, env: opts.env, stdin: "ignore", stdout: "pipe", stderr: "pipe" });
  let timedOut = false;
  const readers = new Set<ReadableStreamDefaultReader<Uint8Array>>();
  const timer = setTimeout(() => {
    timedOut = true; proc.kill("SIGKILL");
    for (const reader of readers) void reader.cancel().catch(() => {});
  }, opts.timeoutMs!);
  async function collect(stream: ReadableStream<Uint8Array>) {
    const reader = stream.getReader(); readers.add(reader);
    const chunks: Uint8Array[] = [];
    let size = 0, truncated = false;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      const available = Math.max(0, opts.maxBytes! - size);
      if (value.length > available) truncated = true;
      if (available) { chunks.push(value.slice(0, available)); size += Math.min(value.length, available); }
    }
    readers.delete(reader);
    return { text: new TextDecoder().decode(Buffer.concat(chunks, size), { stream: truncated }), truncated };
  }
  try {
    const [stdout, stderr, code] = await Promise.all([collect(proc.stdout), collect(proc.stderr), proc.exited]);
    return { stdout: stdout.text, stderr: timedOut ? "Agent control command timed out" : stderr.text, code: timedOut ? 124 : code, truncated: stdout.truncated };
  } finally { clearTimeout(timer); }
}
export async function runControl(argv: string[], opts: ExecOptions, scope: ControlScope): Promise<ExecResult> {
  if (!isControlAllowed(argv, scope)) throw new Error("Command is not on the agent control allowlist");
  const timeoutMs = opts.timeoutMs ?? 10_000, maxBytes = opts.maxBytes ?? 200_000;
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0 || !Number.isSafeInteger(maxBytes) || maxBytes < 0) throw new Error("Invalid execution limits");
  const env: Record<string, string | undefined> = { ...process.env, GIT_TERMINAL_PROMPT: "0", GH_PROMPT_DISABLED: "1", GH_PAGER: "cat", GIT_PAGER: "cat", PAGER: "cat", NO_COLOR: "1" };
  for (const key of Object.keys(env)) if (key.startsWith("GIT_CONFIG_") || ["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_SSH_COMMAND", "GIT_SSH", "GIT_EXTERNAL_DIFF"].includes(key)) delete env[key];
  env.GIT_SSH_COMMAND = "ssh -oBatchMode=yes -oStrictHostKeyChecking=yes";
  const command = argv[0] === "git"
    ? ["git", "-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "core.pager=cat", "--no-pager", ...argv.slice(1, 3), ...argv.slice(3, 4), ...(argv[3] === "log" ? ["--no-ext-diff", "--no-textconv"] : []), ...argv.slice(4)] : [...argv];
  const prepared = { ...opts, timeoutMs, maxBytes, env };
  return (runner ?? ((args, options) => spawn(args, options as typeof prepared)))(command, prepared);
}
