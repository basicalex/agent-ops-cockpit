import { createReadStream } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import { isAbsolute, relative, resolve, dirname } from "node:path";
import { LIMITS, ReadOnlyViolation, type CodeHit, type DiffRequest, type DiffResult, type FileChange, type FileRead, type GitState } from "./contracts";
import { runReadOnly } from "./exec";
import { isSecretPath, redact } from "./redact";

function limit(value: number | undefined, fallback: number): number {
  const n = value ?? fallback;
  if (!Number.isSafeInteger(n) || n < 0) throw new Error("Invalid result limit");
  return n;
}
function refName(ref: string): string {
  if (ref.startsWith("-") || !/^[A-Za-z0-9_.\/~^@{}-]+(\.\.\.?[A-Za-z0-9_.\/~^@{}-]+)?$/.test(ref)) throw new ReadOnlyViolation("Invalid Git ref");
  return ref;
}
function inside(root: string, path: string): boolean {
  const rel = relative(root, path);
  return !isAbsolute(rel) && rel !== ".." && !rel.startsWith("../");
}
async function repoPath(root: string, path: string, missing: boolean, allowSecret: boolean): Promise<string> {
  if (!path || isAbsolute(path) || path.includes("\0") || path.replaceAll("\\", "/").split("/").includes("..")) throw new ReadOnlyViolation("Path must stay inside the repository");
  const base = await realpath(root);
  const target = resolve(base, path);
  if (!inside(base, target)) throw new ReadOnlyViolation("Path must stay inside the repository");
  let probe = target;
  for (;;) {
    try {
      const actual = await realpath(probe);
      if (!inside(base, actual)) throw new ReadOnlyViolation("Symlink leaves the repository");
      if (!allowSecret && (isSecretPath(path) || isSecretPath(relative(base, actual)))) throw new ReadOnlyViolation("Secret paths cannot be read");
      break;
    } catch (error) {
      if (error instanceof ReadOnlyViolation) throw error;
      if (!missing || !["ENOENT", "ENOTDIR"].includes((error as NodeJS.ErrnoException).code ?? "") || probe === base) throw error;
      probe = dirname(probe);
    }
  }
  return relative(base, target).replaceAll("\\", "/");
}
async function git(root: string, args: string[]): Promise<string> {
  const result = await runReadOnly(["git", ...args], { cwd: root });
  if (result.code !== 0) throw new Error(redact(result.stderr.trim()) || "Git read failed");
  if (result.truncated) throw new Error("Git metadata exceeds the output limit");
  return result.stdout;
}

type Stat = { change: FileChange; oldPath?: string };
function numstat(text: string): Stat[] {
  const records = text.split("\0");
  const result: Stat[] = [];
  for (let i = 0; i < records.length; i++) {
    const match = /^(\d+|-)\t(\d+|-)\t([\s\S]*)$/.exec(records[i]!);
    if (!match) continue;
    let path = match[3]!;
    let oldPath: string | undefined;
    if (!path) { oldPath = records[++i]; path = records[++i] ?? ""; }
    result.push({ change: { path, status: oldPath ? "R" : "M", added: match[1] === "-" ? null : Number(match[1]), removed: match[2] === "-" ? null : Number(match[2]) }, oldPath });
  }
  return result;
}

async function patchOmission(root: string, path: string, ref: string | null): Promise<"private-key" | "inspection-limit" | null> {
  const marker = "PRIVATE KEY-----";
  if (ref === null) {
    // A hunk can omit the PEM delimiters; scan the entire working-tree file.
    let tail = "";
    try {
      for await (const chunk of createReadStream(resolve(root, path))) {
        const text = tail + chunk.toString("utf8");
        if (text.includes(marker)) return "private-key";
        tail = text.slice(-(marker.length - 1));
      }
    } catch (error) {
      // Deleted files and submodule directories have no working-tree text blob.
      if (!["ENOENT", "EISDIR"].includes((error as NodeJS.ErrnoException).code ?? "")) throw error;
    }
    return null;
  }
  const object = `${ref}:${path}`;
  const size = await runReadOnly(["git", "cat-file", "-s", object], { cwd: root });
  // Missing stage zero can also mean a conflict; inspect all conflict blobs.
  if (size.code === 128) {
    if (ref !== "") return null;
    const stages = await Promise.all([":1", ":2", ":3"].map(stage => patchOmission(root, path, stage)));
    return stages.includes("private-key") ? "private-key" : stages.includes("inspection-limit") ? "inspection-limit" : null;
  }
  if (size.code !== 0 || size.truncated) throw new Error(redact(size.stderr) || "Cannot inspect diff baseline");
  if (Number(size.stdout.trim()) > 200_000) return "inspection-limit";
  const blob = await runReadOnly(["git", "show", object], { cwd: root });
  if (blob.code !== 0) throw new Error(redact(blob.stderr) || "Cannot inspect diff blob");
  if (blob.truncated) return "inspection-limit";
  return blob.stdout.includes(marker) ? "private-key" : null;
}

export async function getGitState(root: string, opts: { commits?: number } = {}): Promise<GitState> {
  const count = limit(opts.commits, LIMITS.commits);
  const [branch, head, status, stagedStats, unstagedStats, worktreeText] = await Promise.all([
    git(root, ["branch", "--show-current"]), runReadOnly(["git", "rev-parse", "--verify", "HEAD"], { cwd: root }),
    git(root, ["status", "--porcelain=v2", "-z", "--untracked-files=all"]),
    git(root, ["diff", "--cached", "--numstat", "-z", "--find-renames"]), git(root, ["diff", "--numstat", "-z", "--find-renames"]),
    git(root, ["worktree", "list", "--porcelain"]),
  ]);
  const stagedCounts = Object.fromEntries(numstat(stagedStats).map(item => [item.change.path, item.change]));
  const unstagedCounts = Object.fromEntries(numstat(unstagedStats).map(item => [item.change.path, item.change]));
  const staged: FileChange[] = [], unstaged: FileChange[] = [], untracked: string[] = [];
  const records = status.split("\0");
  for (let i = 0; i < records.length; i++) {
    const record = records[i]!;
    if (record.startsWith("? ")) { untracked.push(redact(record.slice(2))); continue; }
    const fields = record.split(" ");
    const kind = fields[0];
    if (!["1", "2", "u"].includes(kind ?? "")) continue;
    const xy = fields[1]!;
    const path = fields.slice(kind === "2" ? 9 : kind === "u" ? 10 : 8).join(" ");
    if (kind === "2") i++;
    if (xy[0] !== ".") staged.push({ path: redact(path), status: xy[0]!, added: stagedCounts[path]?.added ?? null, removed: stagedCounts[path]?.removed ?? null });
    if (xy[1] !== ".") unstaged.push({ path: redact(path), status: xy[1]!, added: unstagedCounts[path]?.added ?? null, removed: unstagedCounts[path]?.removed ?? null });
  }
  const upstreamResult = await runReadOnly(["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"], { cwd: root });
  const upstream = upstreamResult.code === 0 ? upstreamResult.stdout.trim() : null;
  let ahead = 0, behind = 0;
  if (upstream) [ahead, behind] = (await git(root, ["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])).trim().split(/\s+/).map(Number) as [number, number];
  const hasHead = head.code === 0;
  const recent = hasHead && count ? await git(root, ["log", `-${count}`, "--format=%H%x1f%aI%x1f%an%x1f%s"]) : "";
  const recentCommits = recent.trimEnd().split("\n").filter(Boolean).map(line => {
    const [sha, date, author, ...subject] = line.split("\x1f");
    return { sha: sha!, date: date!, author: redact(author ?? ""), subject: redact(subject.join("\x1f")) };
  });
  const worktrees = worktreeText.trimEnd().split("\n\n").filter(Boolean).map(block => {
    const lines = block.split("\n");
    const branch = lines.find(line => line.startsWith("branch "))?.slice(7).replace(/^refs\/heads\//, "") ?? null;
    return { path: redact(lines.find(line => line.startsWith("worktree "))?.slice(9) ?? ""), branch: branch === null ? null : redact(branch) };
  });
  return { root: redact(root), branch: branch.trim() ? redact(branch.trim()) : null, head: hasHead ? head.stdout.trim() : "", headSubject: hasHead ? redact((await git(root, ["log", "-1", "--format=%s"])).trimEnd()) : "", upstream: upstream === null ? null : redact(upstream), ahead, behind, dirty: staged.length + unstaged.length + untracked.length > 0, staged, unstaged, untracked: untracked.slice(0, LIMITS.untracked), untrackedTotal: untracked.length, recentCommits, worktrees };
}

export async function getDiff(req: DiffRequest): Promise<DiffResult> {
  const maxBytes = limit(req.maxBytes, LIMITS.diffBytes);
  const target = req.target ?? "worktree";
  const revision = target === "worktree" ? [] : target === "staged" ? ["--cached"] : [refName(target)];
  const paths = await Promise.all((req.paths ?? []).map(path => repoPath(req.root, path, true, true)));
  const stats = numstat(await git(req.root, ["diff", ...revision, "--numstat", "-z", "--find-renames", "--", ...paths.map(path => `:(literal)${path}`)]));
  let beforeRefs = [target === "worktree" ? "" : target === "staged" ? "HEAD" : target];
  let afterRef: string | null = target === "staged" ? "" : null;
  if (target !== "worktree" && target !== "staged" && target.includes("..")) {
    const [before, separator, after] = target.split(/(\.\.\.?)/);
    afterRef = after || "HEAD";
    beforeRefs = separator === "..."
      ? (await git(req.root, ["rev-parse", "--revs-only", target])).trim().split("\n").filter(ref => ref.startsWith("^")).map(ref => ref.slice(1))
      : [before || "HEAD"];
    if (!beforeRefs.length) throw new Error("Cannot inspect diff merge base");
  }
  let inspectionTruncated = false;
  const readable: string[] = [], notes: string[] = [];
  for (const stat of stats) {
    let secret = isSecretPath(stat.change.path) || (stat.oldPath !== undefined && isSecretPath(stat.oldPath));
    if (!secret) {
      try { await repoPath(req.root, stat.change.path, true, false); }
      catch (error) { if (!(error instanceof ReadOnlyViolation)) throw error; secret = true; }
    }
    let omission: "private-key" | "inspection-limit" | null = null;
    if (!secret) {
      const sides = await Promise.all([
        ...beforeRefs.map(ref => patchOmission(req.root, stat.oldPath ?? stat.change.path, ref)),
        patchOmission(req.root, stat.change.path, afterRef),
      ]);
      omission = sides.includes("private-key") ? "private-key" : sides.includes("inspection-limit") ? "inspection-limit" : null;
    }
    if (secret || omission === "private-key") notes.push(`[Secret file omitted: ${redact(stat.change.path)}]\n`);
    else if (omission === "inspection-limit") {
      notes.push(`[File patch omitted: ${redact(stat.change.path)} (inspection exceeds output limit)]\n`);
      inspectionTruncated = true;
    }
    else {
      readable.push(stat.change.path);
      if (stat.oldPath !== undefined) readable.push(stat.oldPath);
    }
  }
  let patch = "", truncated = inspectionTruncated;
  if (readable.length) {
    const output = await runReadOnly(["git", "diff", ...revision, "--find-renames", "--submodule=short", "--", ...readable.map(path => `:(literal)${path}`)], { cwd: req.root, maxBytes: Math.max(200_000, maxBytes + 4096) });
    if (output.code !== 0) throw new Error(redact(output.stderr) || "Git diff failed");
    patch = redact(output.stdout);
    truncated ||= output.truncated;
  }
  patch += notes.join("");
  const bytes = Buffer.from(patch);
  if (bytes.length > maxBytes) { patch = new TextDecoder().decode(bytes.subarray(0, maxBytes), { stream: true }); truncated = true; }
  return { summary: stats.map(({ change }) => ({ ...change, path: redact(change.path) })), patch, truncated };
}

export async function readRepoFile(root: string, relPath: string, opts: { startLine?: number; endLine?: number; ref?: string } = {}): Promise<FileRead> {
  const path = await repoPath(root, relPath, opts.ref !== undefined, false);
  const startLine = opts.startLine ?? 1;
  const requestedEnd = opts.endLine ?? startLine + LIMITS.fileLines - 1;
  if (!Number.isSafeInteger(startLine) || startLine < 1 || !Number.isSafeInteger(requestedEnd) || requestedEnd < startLine) throw new Error("Invalid line window");
  let bytes: Buffer;
  if (opts.ref !== undefined) {
    const output = await runReadOnly(["git", "show", `${refName(opts.ref)}:${path}`], { cwd: root, maxBytes: Number.MAX_SAFE_INTEGER });
    if (output.code !== 0) throw new Error(redact(output.stderr) || "Git file read failed");
    bytes = Buffer.from(output.stdout);
  } else {
    const tracked = await runReadOnly(["git", "ls-files", "--error-unmatch", "--", `:(literal)${path}`], { cwd: root });
    if (tracked.code !== 0) throw new ReadOnlyViolation("Only tracked files can be read");
    bytes = await readFile(resolve(root, path));
  }
  if (bytes.includes(0)) return { path: redact(path), startLine, endLine: startLine, totalLines: 0, text: "[Binary file omitted]", truncated: false };
  // Redact before slicing so a line window cannot expose part of a PEM block.
  const source = bytes.toString("utf8");
  const original = source.split("\n");
  if (original.at(-1) === "") original.pop();
  const totalLines = original.length;
  const endLine = Math.min(requestedEnd, startLine + LIMITS.fileLines - 1, totalLines);
  const lineSafe = source.replace(/-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----[\s\S]*?(?:-----END (?:[A-Z]+ )?PRIVATE KEY-----|$)/g, block => block.split("\n").map(() => "[REDACTED]").join("\n"));
  const text = redact(lineSafe).split("\n").slice(startLine - 1, endLine).join("\n");
  return { path: redact(path), startLine, endLine: Math.max(startLine - 1, endLine), totalLines, text, truncated: requestedEnd > startLine + LIMITS.fileLines - 1 || endLine < totalLines };
}

export async function searchCode(root: string, query: string, opts: { pathGlob?: string; regex?: boolean; maxHits?: number } = {}): Promise<{ hits: CodeHit[]; truncated: boolean }> {
  const maxHits = limit(opts.maxHits, LIMITS.searchHits);
  if (opts.pathGlob !== undefined && (isAbsolute(opts.pathGlob) || opts.pathGlob.includes("\0") || opts.pathGlob.split("/").includes(".."))) throw new ReadOnlyViolation("Search path must stay inside the repository");
  const args = ["git", "grep", "-n", "-I", "--full-name", "-z", opts.regex ? "-E" : "-F", "-e", query, "--", ...(opts.pathGlob === undefined ? [] : [opts.pathGlob])];
  const output = await runReadOnly(args, { cwd: root });
  if (output.code !== 0 && output.code !== 1) throw new Error(redact(output.stderr) || "Git search failed");
  const hits: CodeHit[] = [];
  let truncated = output.truncated;
  const pattern = /([^\0]+)\0(\d+)\0([^\n]*)(?:\n|$)/g;
  for (const match of output.stdout.matchAll(pattern)) {
    const path = match[1]!;
    if (isSecretPath(path)) continue;
    try { await repoPath(root, path, false, false); } catch (error) { if (error instanceof ReadOnlyViolation) continue; throw error; }
    if (hits.length === maxHits) { truncated = true; break; }
    const line = Number(match[2]);
    const file = await readRepoFile(root, path, { startLine: line, endLine: line });
    hits.push({ path: redact(path), line, text: file.text.slice(0, 300) });
  }
  return { hits, truncated };
}

export async function repoRootFor(dir: string): Promise<string | null> {
  try {
    const result = await runReadOnly(["git", "rev-parse", "--show-toplevel"], { cwd: dir });
    return result.code === 0 && !result.truncated ? result.stdout.trim() : null;
  } catch { return null; }
}
