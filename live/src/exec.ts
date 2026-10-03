import { resolve } from "node:path";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { ReadOnlyViolation, type ExecOptions, type ExecResult, type Runner } from "./contracts";

const safetyPrefix = ["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "diff.external=", "-c", "core.pager=cat", "--no-pager"];
const safeEnv = {
  GIT_OPTIONAL_LOCKS: "0", GIT_TERMINAL_PROMPT: "0", GIT_PAGER: "cat", PAGER: "cat",
  GH_PAGER: "cat", GH_PROMPT_DISABLED: "1", NO_COLOR: "1",
};
type PreparedOptions = ExecOptions & { env: Record<string, string | undefined> };
let runner: Runner | null = null;
const checkoutJournal = resolve(import.meta.dir, "../../bin/aoc-journal");
const journalPath = resolve(process.env.AOC_LIVE_JOURNAL_BIN ?? (existsSync(checkoutJournal) ? checkoutJournal : resolve(homedir(), ".local/bin/aoc-journal")));

export function isAllowed(argv: string[]): boolean {
  if (!argv.length || argv.some(arg => typeof arg !== "string" || arg.includes("\0"))) return false;
  const [command, ...args] = argv;
  if (args.some(arg => /^(?:--output(?:=|$)|--open-files-in-pager|--ext-diff(?:=|$)|--textconv(?:=|$)|--exec|--git-dir|--work-tree|--upload-pack|--config)/.test(arg) || arg === "-c" || arg.startsWith("-c="))) return false;
  if (command === journalPath) {
    if (args[0] !== "state" || !/^[1-9]\d*$/.test(args[1] ?? "")) return false;
    const seen = new Set<string>();
    for (let i = 2; i < args.length; i++) {
      const flag = args[i]!;
      if (seen.has(flag)) return false;
      seen.add(flag);
      if (flag === "--json") continue;
      const value = args[++i] ?? "";
      if (flag === "--events" ? !/^\d+$/.test(value)
        : flag === "--repo" ? !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value) : true) return false;
    }
    return true;
  }
  if (command === "herdr") {
    if (args.join("\0") === "api\0snapshot") return true;
    if (args[0] !== "pane" || args[1] !== "read" || !args[2] || args[2].startsWith("-")) return false;
    const seen = new Set<string>();
    for (let i = 3; i < args.length; i += 2) {
      const flag = args[i]!;
      const value = args[i + 1];
      if (seen.has(flag)) return false;
      seen.add(flag);
      if (flag === "--source" ? !["recent", "visible", "recent-unwrapped"].includes(value ?? "")
        : flag === "--lines" ? !/^[1-9]\d*$/.test(value ?? "")
        : flag === "--format" ? value !== "text" : true) return false;
    }
    return true;
  }
  if (command === "git") {
    // Git accepts abbreviated long options and bundled short options.
    if (args.some(arg => {
      const flag = arg.split("=", 1)[0]!;
      return (flag.startsWith("--") && flag.length > 2 && ["--output", "--open-files-in-pager", "--ext-diff", "--textconv", "--exec", "--git-dir", "--work-tree", "--upload-pack", "--config", "--no-index"].some(unsafe => unsafe.startsWith(flag)))
        || /^-[^-].*O/.test(arg);
    })) return false;
    if (args.some(arg => /^-o/.test(arg) || arg.startsWith("-O") || /^-c./.test(arg))) return false;
    const [subcommand, ...rest] = args;
    if (subcommand === "worktree") return rest[0] === "list" && rest.slice(1).every(arg => ["--porcelain", "-z", "--verbose", "-v", "--expire=now"].includes(arg));
    if (subcommand === "branch") return rest.length === 1 && rest[0] === "--show-current";
    if (subcommand === "cat-file") return ["-s", "-t"].includes(rest[0] ?? "") && rest.length === 2 && !rest[1]!.startsWith("-");
    return ["status", "log", "diff", "show", "rev-parse", "grep", "ls-files", "rev-list", "for-each-ref"].includes(subcommand ?? "");
  }
  if (command === "gh") {
    if (args.some(arg => /^(?:--method(?:=|$)|--field(?:=|$)|--raw-field(?:=|$)|--input(?:=|$)|--web(?:=|$))/.test(arg) || /^-(?:X|f|F|w)/.test(arg))) return false;
    return ["issue", "pr"].includes(args[0] ?? "") && ["view", "list"].includes(args[1] ?? "");
  }
  return false;
}

async function spawn(argv: string[], opts: PreparedOptions): Promise<ExecResult> {
  const proc = Bun.spawn(argv, { cwd: opts.cwd, env: opts.env, stdin: "ignore", stdout: "pipe", stderr: "pipe" });
  let timedOut = false;
  const readers = new Set<ReadableStreamDefaultReader<Uint8Array>>();
  const timer = setTimeout(() => {
    timedOut = true;
    proc.kill("SIGKILL");
    for (const reader of readers) void reader.cancel().catch(() => {});
  }, opts.timeoutMs!);
  async function collect(stream: ReadableStream<Uint8Array>) {
    const reader = stream.getReader();
    readers.add(reader);
    const chunks: Uint8Array[] = [];
    let size = 0;
    let truncated = false;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      const available = Math.max(0, opts.maxBytes! - size);
      if (value.length > available) truncated = true;
      if (available) { const chunk = value.slice(0, available); chunks.push(chunk); size += chunk.length; }
    }
    readers.delete(reader);
    return { text: new TextDecoder().decode(Buffer.concat(chunks, size), { stream: truncated }), truncated };
  }
  try {
    const [stdout, stderr, code] = await Promise.all([collect(proc.stdout), collect(proc.stderr), proc.exited]);
    return { stdout: stdout.text, stderr: timedOut ? "Read-only command timed out" : stderr.text, code: timedOut ? 124 : code, truncated: stdout.truncated };
  } finally { clearTimeout(timer); }
}

export async function runReadOnly(argv: string[], opts: ExecOptions = {}): Promise<ExecResult> {
  if (!isAllowed(argv)) throw new ReadOnlyViolation("Command is not on the read-only allowlist");
  const env: Record<string, string | undefined> = { ...process.env, ...safeEnv };
  // Inherited Git overrides can bypass cwd and the explicit safety configuration.
  for (const key of Object.keys(env)) if (key.startsWith("GIT_CONFIG_") || ["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_EXTERNAL_DIFF"].includes(key)) delete env[key];
  const prepared: PreparedOptions = { ...opts, timeoutMs: opts.timeoutMs ?? 10_000, maxBytes: opts.maxBytes ?? 200_000, env };
  if (!Number.isFinite(prepared.timeoutMs) || prepared.timeoutMs! <= 0 || !Number.isSafeInteger(prepared.maxBytes) || prepared.maxBytes! < 0) throw new Error("Invalid execution limits");
  let command = [...argv];
  if (argv[0] === "git") {
    const flags = ["diff", "show", "log"].includes(argv[1]!) ? ["--no-ext-diff", "--no-textconv"] : [];
    command = ["git", ...safetyPrefix, argv[1]!, ...flags, ...argv.slice(2)];
  }
  return (runner ?? ((command, options) => spawn(command, options as PreparedOptions)))(command, prepared);
}

export function setRunner(fn: Runner | null): void { runner = fn; }
