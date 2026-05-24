import type { AgentMessage } from "@earendil-works/pi-agent-core";
import { complete, type UserMessage } from "@earendil-works/pi-ai";
import type { ExtensionAPI, SessionEntry } from "@earendil-works/pi-coding-agent";
import { convertToLlm, serializeConversation } from "@earendil-works/pi-coding-agent";

const MAX_COMMAND_BYTES = 12_000;
const MAX_METADATA_BYTES = 20_000;
const MAX_RECENT_CONTEXT_BYTES = 30_000;
const COMMAND_TIMEOUT_MS = 4_000;
const DEBUG_MARKER = "<!-- aoc-compaction-extension:v2 current-state-rollup -->";

interface CommandSnapshot {
  command: string;
  ok: boolean;
  stdout: string;
  stderr: string;
}

interface AocCompactionDetails {
  kind: "aoc-compaction";
  version: 1;
  activeTag: string | null;
  projectRoot: string;
  commands: CommandSnapshot[];
  readFiles: string[];
  modifiedFiles: string[];
  warnings: string[];
  messageCounts: {
    summarized: number;
    turnPrefix: number;
    recentKept: number;
  };
}

function truncateText(value: string, maxBytes = MAX_COMMAND_BYTES): string {
  const bytes = Buffer.byteLength(value, "utf8");
  if (bytes <= maxBytes) return value;
  const sliced = Buffer.from(value, "utf8").subarray(0, maxBytes).toString("utf8");
  return `${sliced}\n[truncated ${bytes - maxBytes} bytes]`;
}

function textFromResponse(response: any): string {
  return (response?.content || [])
    .filter((block: any) => block?.type === "text" && typeof block.text === "string")
    .map((block: any) => block.text)
    .join("\n")
    .trim();
}

function stringSet(value: unknown): string[] {
  if (value instanceof Set) return [...value].filter((v): v is string => typeof v === "string").sort();
  if (Array.isArray(value)) return value.filter((v): v is string => typeof v === "string").sort();
  return [];
}

function fileOpsFromPreparation(preparation: any): { readFiles: string[]; modifiedFiles: string[] } {
  const ops = preparation?.fileOps || {};
  const readFiles = stringSet(ops.read);
  const modified = new Set<string>();
  for (const file of stringSet(ops.edited)) modified.add(file);
  for (const file of stringSet(ops.written)) modified.add(file);
  return { readFiles, modifiedFiles: [...modified].sort() };
}

function parseActiveTag(handshake: string, tmTag: string): string | null {
  const direct = tmTag.trim().split(/\s+/)[0];
  if (direct) return direct;
  try {
    const parsed = JSON.parse(handshake);
    const tag = parsed?.taskmaster?.activeTag;
    return typeof tag === "string" && tag.trim() ? tag.trim() : null;
  } catch {
    return null;
  }
}

function entryToMessage(entry: SessionEntry): AgentMessage | undefined {
  if (entry.type === "message") return entry.message;
  if (entry.type === "compaction") {
    return {
      role: "compactionSummary",
      summary: entry.summary,
      tokensBefore: entry.tokensBefore,
      timestamp: new Date(entry.timestamp).getTime(),
    } as AgentMessage;
  }
  return undefined;
}

function recentKeptMessages(branchEntries: SessionEntry[], firstKeptEntryId: string): AgentMessage[] {
  const firstKeptIndex = branchEntries.findIndex((entry) => entry.id === firstKeptEntryId);
  if (firstKeptIndex < 0) return [];
  return branchEntries.slice(firstKeptIndex).map(entryToMessage).filter((message): message is AgentMessage => message !== undefined);
}

function detectTaskIds(...texts: string[]): string[] {
  const ids = new Set<string>();
  for (const text of texts) {
    for (const match of text.matchAll(/(?:task\s+|#)(\d{1,5})(?:\.\d+)?\b/gi)) {
      ids.add(match[1]);
      if (ids.size >= 5) return [...ids].sort((a, b) => Number(a) - Number(b));
    }
  }
  return [...ids].sort((a, b) => Number(a) - Number(b));
}

function renderTaskStatus(commands: CommandSnapshot[]): string {
  const taskCommands = commands.filter((item) => /^tm show \d+/.test(item.command));
  if (!taskCommands.length) return "(no task ids detected for bounded task status lookup)";
  return taskCommands
    .map((item) => `### $ ${item.command}\nstatus: ${item.ok ? "ok" : "failed"}\n\`\`\`\n${truncateText(item.stdout.trim() || item.stderr.trim() || "(no output)", 6_000)}\n\`\`\``)
    .join("\n\n");
}

function renderOperationalCapsule(meta: {
  projectRoot: string;
  activeTag: string | null;
  hasDesign: boolean;
  commands: CommandSnapshot[];
  warnings: string[];
}): string {
  const commandBlock = meta.commands
    .map((item) => {
      const out = item.stdout.trim() || item.stderr.trim() || "(no output)";
      return `### $ ${item.command}\nstatus: ${item.ok ? "ok" : "failed"}\n\`\`\`\n${truncateText(out, 4_000)}\n\`\`\``;
    })
    .join("\n\n");

  return truncateText(`## AOC Operational Context Capsule
- Project root: ${meta.projectRoot}
- AOC is active for this repo. Preserve this operating context after compaction.
- Orientation: use \`.aoc/context.md\` when needed; run \`aoc-init\` if AOC context is missing/stale.
- Startup metadata: \`aoc-handshake --json\`.
- Active Taskmaster tag at compaction: ${meta.activeTag || "unknown"}.
- Tasks/specs: use \`tm tag current\`, \`tm list\`, \`tm show <id>\`, \`aoc-task tag spec show --tag <tag>\`, and \`aoc-task spec show <id> --tag <tag>\`.
- Memory: use \`aoc-mem read\`, \`aoc-mem search\`, and \`aoc-mem add\`; do not read \`.aoc/memory.md\` directly.
- STM: use \`aoc-stm status\`, \`/handoff <focus>\`, and \`/rresume [archive]\`; STM is directed handoff-only, not durable memory or a mailbox.
- Protected direct reads: do not read \`.aoc/stm/current.md\`, \`.aoc/memory.md\`, or \`.taskmaster/tasks/tasks.json\` directly.
- Product/UI/design-facing work: ${meta.hasDesign ? "read root `DESIGN.md` before changing UI, docs-site, marketing, HyperFrames, or other product-facing surfaces." : "no root `DESIGN.md` detected at compaction time."}
- Native Pi compaction remains \`/compact [focus]\`; this AOC extension only makes the compaction summary preserve AOC operating knowledge.
- Do not inject broad memory, latest STM, full specs, or raw large diffs during compaction unless explicitly requested later.
${meta.warnings.length ? `\nWarnings:\n${meta.warnings.map((w) => `- ${w}`).join("\n")}` : ""}

## Bounded Metadata Snapshots
${commandBlock || "(metadata unavailable)"}`, MAX_METADATA_BYTES);
}

function buildPrompt(params: {
  conversationText: string;
  previousSummary: string;
  recentKeptContext: string;
  taskStatusContext: string;
  operationalCapsule: string;
  customInstructions?: string;
  readFiles: string[];
  modifiedFiles: string[];
}): string {
  return `You are generating a full AOC-first Pi compaction summary.

Goal: summarize the conversation while preserving the AOC operational contract so the next post-compaction agent still knows the local tools, safety rules, and continuation workflows. Do not turn compaction into broad memory ingestion. Treat AOC metadata as environment/tooling context, not verified task truth.

Evidence precedence is mandatory:
1. User custom instructions and latest/recent kept context are newest and win.
2. Bounded live task status/git metadata may correct stale task progress.
3. Conversation-to-compact is older discarded history.
4. Previous-summary is oldest iterative context and must not preserve stale In Progress or Next Actions when newer evidence conflicts.

${params.operationalCapsule}

<previous-summary oldest-may-be-stale>
${params.previousSummary || "(none)"}
</previous-summary>

<conversation-to-compact older-discarded-history>
${params.conversationText || "(none)"}
</conversation-to-compact>

<recent-kept-context newest-wins-on-conflict>
${params.recentKeptContext || "(none)"}
</recent-kept-context>

<bounded-live-task-status may-correct-stale-progress>
${params.taskStatusContext || "(none)"}
</bounded-live-task-status>

<custom-compact-instructions newest-user-focus>
${params.customInstructions || "(none)"}
</custom-compact-instructions>

Known file operations extracted by Pi:
<read-files>
${params.readFiles.join("\n") || "(none)"}
</read-files>
<modified-files>
${params.modifiedFiles.join("\n") || "(none)"}
</modified-files>

Produce exactly this markdown structure:

## Goal
[User goal(s), concise.]

## Constraints & Preferences
- [User constraints/preferences]
- [Relevant AOC/project constraints]

## Progress
### Done
- [x] [Completed work]

### In Progress
- [ ] [Current incomplete work]

### Blocked
- [Blockers, or "(none)"]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Current Working Set
### Files read/touched
- [Files or areas]

### Modified/staged files
- [Files or areas]

### Validation
- [Commands/results, or what was not run]

## AOC Operational Context
- [Preserve the AOC operating context capsule in concise bullets: tools, active tag, safe commands, protected files, STM rules, design contract rule.]

## Next Safe Actions
1. [Next step]

## Critical Context
- [Only facts needed to continue safely]

<read-files>
[one path per line, or empty]
</read-files>
<modified-files>
[one path per line, or empty]
</modified-files>

Start the output with this exact marker on its own line: ${DEBUG_MARKER}

Keep it concise but do not omit AOC Operational Context. When listing current progress and next actions, prefer recent-kept-context and bounded-live-task-status over previous-summary.`;
}

export default function aocCompaction(pi: ExtensionAPI) {
  pi.on("session_before_compact", async (event, ctx) => {
    if (process.env.AOC_PI_COMPACTION === "0") return;

    const warnings: string[] = [];

    async function run(command: string, args: string[]): Promise<CommandSnapshot> {
      const label = [command, ...args].join(" ");
      try {
        const result = await pi.exec(command, args, { timeout: COMMAND_TIMEOUT_MS, signal: event.signal });
        return {
          command: label,
          ok: result.code === 0,
          stdout: truncateText(String(result.stdout || "")),
          stderr: truncateText(String(result.stderr || "")),
        };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        warnings.push(`${label} failed: ${message}`);
        return { command: label, ok: false, stdout: "", stderr: truncateText(message) };
      }
    }

    try {
      if (!ctx.model) {
        warnings.push("No active model available for AOC compaction; using native compaction.");
        return;
      }

      const auth = await ctx.modelRegistry.getApiKeyAndHeaders(ctx.model);
      if (!auth.ok || !auth.apiKey) {
        warnings.push(auth.ok ? `No API key for ${ctx.model.provider}; using native compaction.` : auth.error);
        return;
      }

      const [handshake, activeTagResult, gitStatus, gitDiffNames, gitCachedNames] = await Promise.all([
        run("aoc-handshake", ["--json"]),
        run("tm", ["tag", "current"]),
        run("git", ["status", "--short"]),
        run("git", ["diff", "--name-only"]),
        run("git", ["diff", "--cached", "--name-only"]),
      ]);

      const activeTag = parseActiveTag(handshake.stdout, activeTagResult.stdout);
      const hasDesign = await run("test", ["-f", "DESIGN.md"]);
      const commands = [handshake, activeTagResult, gitStatus, gitDiffNames, gitCachedNames];
      const { readFiles, modifiedFiles } = fileOpsFromPreparation(event.preparation);

      const messages = [
        ...(event.preparation.messagesToSummarize || []),
        ...(event.preparation.turnPrefixMessages || []),
      ];
      const recentMessages = recentKeptMessages(event.branchEntries || [], event.preparation.firstKeptEntryId);
      const conversationText = serializeConversation(convertToLlm(messages));
      const recentKeptContext = truncateText(
        serializeConversation(convertToLlm(recentMessages)),
        MAX_RECENT_CONTEXT_BYTES,
      );
      const taskIds = detectTaskIds(
        event.preparation.previousSummary || "",
        conversationText,
        recentKeptContext,
        event.customInstructions || "",
      );
      const taskStatusCommands = await Promise.all(taskIds.map((taskId) => run("tm", ["show", taskId, "--tag", activeTag || "env-protec"])));
      commands.push(...taskStatusCommands);
      const taskStatusContext = renderTaskStatus(taskStatusCommands);
      const operationalCapsule = renderOperationalCapsule({
        projectRoot: ctx.cwd,
        activeTag,
        hasDesign: hasDesign.ok,
        commands,
        warnings,
      });

      const userMessage: UserMessage = {
        role: "user",
        content: [{
          type: "text",
          text: buildPrompt({
            conversationText,
            previousSummary: event.preparation.previousSummary || "",
            recentKeptContext,
            taskStatusContext,
            operationalCapsule,
            customInstructions: event.customInstructions,
            readFiles,
            modifiedFiles,
          }),
        }],
        timestamp: Date.now(),
      };

      ctx.ui.notify("AOC compaction: generating AOC-aware operational summary...", "info");
      const response = await complete(
        ctx.model,
        {
          systemPrompt: "You are an expert AOC/Pi compaction summarizer. Preserve operational context, safety rules, provenance, and next actions without inventing facts.",
          messages: [userMessage],
        },
        { apiKey: auth.apiKey, headers: auth.headers, signal: event.signal, maxTokens: 4096 },
      );

      if (response.stopReason === "aborted") return { cancel: true };
      if (response.stopReason === "error") {
        warnings.push(response.errorMessage || "AOC compaction model call failed; using native compaction.");
        return;
      }

      const summary = textFromResponse(response);
      if (!summary) {
        warnings.push("AOC compaction summary was empty; using native compaction.");
        return;
      }

      const details: AocCompactionDetails = {
        kind: "aoc-compaction",
        version: 1,
        activeTag,
        projectRoot: ctx.cwd,
        commands,
        readFiles,
        modifiedFiles,
        warnings,
        messageCounts: {
          summarized: event.preparation.messagesToSummarize?.length || 0,
          turnPrefix: event.preparation.turnPrefixMessages?.length || 0,
          recentKept: recentMessages.length,
        },
      };

      return {
        compaction: {
          summary,
          firstKeptEntryId: event.preparation.firstKeptEntryId,
          tokensBefore: event.preparation.tokensBefore,
          details,
        },
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      ctx.ui.notify(`AOC compaction fell back to native compaction: ${message}`, "warning");
      return;
    }
  });
}
