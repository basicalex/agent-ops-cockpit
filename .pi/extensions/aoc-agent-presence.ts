import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

interface PresenceState {
  schema: number;
  agent_id: string;
  session_id: string;
  project_key: string;
  project_root: string;
  tab_scope: string | null;
  pid: number;
  model: string;
  lifecycle: "starting" | "idle" | "thinking" | "tool" | "error" | "offline";
  current_tool: string | null;
  chat_title: string | null;
  session_file: string | null;
  tool_count: number;
  turn_count: number;
  context_pct: number | null;
  started_at: string;
  last_activity_at: string;
  heartbeat_at: string;
}

const DEFAULT_HEARTBEAT_MS = 30_000;
const MIN_HEARTBEAT_MS = 5_000;
const MAX_HEARTBEAT_MS = 5 * 60_000;
const SCHEMA = 1;

export function resolveHeartbeatMs(value = process.env.AOC_AGENT_PRESENCE_HEARTBEAT_MS): number {
  if (!value?.trim()) return DEFAULT_HEARTBEAT_MS;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) return DEFAULT_HEARTBEAT_MS;
  return Math.min(MAX_HEARTBEAT_MS, Math.max(MIN_HEARTBEAT_MS, parsed));
}

function nowIso(): string {
  return new Date().toISOString();
}

function projectKey(root: string): string {
  return root.replace(/[^A-Za-z0-9_.-]+/g, "_").replace(/^_+|_+$/g, "") || "default";
}

function registryRoot(): string {
  return process.env.AOC_AGENT_REGISTRY_DIR
    || path.join(process.env.XDG_STATE_HOME || path.join(os.homedir(), ".local", "state"), "aoc", "agent-registry");
}

function atomicWriteJson(file: string, value: unknown): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.${process.pid}.${Date.now()}.tmp`;
  fs.writeFileSync(tmp, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
  fs.renameSync(tmp, file);
}

function safeContextPct(ctx: ExtensionContext | undefined): number | null {
  try {
    const pct = ctx?.getContextUsage?.()?.percent;
    return typeof pct === "number" && Number.isFinite(pct) ? Math.round(pct) : null;
  } catch {
    return null;
  }
}

function modelId(ctx: ExtensionContext | undefined, fallback = "unknown"): string {
  return ctx?.model?.id || fallback;
}

function textFromContent(content: unknown): string {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return content
      .map((block) => {
        if (block && typeof block === "object" && (block as any).type === "text") {
          return String((block as any).text || "");
        }
        return "";
      })
      .filter(Boolean)
      .join(" ");
  }
  return "";
}

function compactTitle(text: string): string | null {
  const first = text.replace(/\s+/g, " ").trim();
  if (!first) return null;
  return first.length > 80 ? `${first.slice(0, 77)}…` : first;
}

function titleFromSession(ctx: ExtensionContext | undefined): string | null {
  try {
    const branch = ctx?.sessionManager?.getBranch?.() || [];
    for (const entry of branch) {
      if ((entry as any).type !== "message") continue;
      const msg = (entry as any).message;
      if (msg?.role !== "user") continue;
      const title = compactTitle(textFromContent(msg.content));
      if (title) return title;
    }
  } catch {
    // best-effort only
  }
  return null;
}

export default function aocAgentPresence(pi: ExtensionAPI) {
  let ctxCurrent: ExtensionContext | undefined;
  let timer: NodeJS.Timeout | undefined;
  let filePath: string | undefined;
  let state: PresenceState | undefined;

  function publish(patch: Partial<PresenceState> = {}) {
    if (!state || !filePath) return;
    const now = nowIso();
    state = {
      ...state,
      ...patch,
      model: modelId(ctxCurrent, state.model),
      context_pct: safeContextPct(ctxCurrent),
      heartbeat_at: now,
      last_activity_at: patch.last_activity_at || now,
    };
    try {
      atomicWriteJson(filePath, state);
    } catch {
      // Presence is observability-only; never disrupt the agent.
    }
  }

  pi.on("session_start", async (_event, ctx) => {
    ctxCurrent = ctx;
    const root = ctx.cwd || process.env.AOC_PROJECT_ROOT || process.cwd();
    const key = projectKey(root);
    const sessionId = process.env.ZELLIJ_SESSION_NAME || process.env.AOC_SESSION_ID || key;
    const tabScope = process.env.AOC_TAB_SCOPE || process.env.AOC_AGENT_ID || null;
    const rawAgentId = process.env.AOC_AGENT_ID || "";
    const instanceId = process.env.AOC_AGENT_INSTANCE_ID || process.env.AOC_PANE_INSTANCE_ID || "";
    const agentId = instanceId
      || (rawAgentId && rawAgentId !== "pi" ? rawAgentId : `${sessionId}:${tabScope || "pi"}:${process.pid}`);
    const started = nowIso();
    filePath = path.join(registryRoot(), key, `${agentId.replace(/[^A-Za-z0-9_.-]+/g, "_")}.json`);
    state = {
      schema: SCHEMA,
      agent_id: agentId,
      session_id: sessionId,
      project_key: key,
      project_root: root,
      tab_scope: tabScope,
      pid: process.pid,
      model: modelId(ctx),
      lifecycle: "idle",
      current_tool: null,
      chat_title: process.env.AOC_CHAT_TITLE || titleFromSession(ctx),
      session_file: ctx.sessionManager?.getSessionFile?.() || null,
      tool_count: 0,
      turn_count: 0,
      context_pct: safeContextPct(ctx),
      started_at: started,
      last_activity_at: started,
      heartbeat_at: started,
    };
    publish({ lifecycle: "idle" });
    timer = setInterval(() => publish(), resolveHeartbeatMs());
    timer.unref?.();
  });

  pi.on("message_start", async (event, ctx) => {
    ctxCurrent = ctx;
    const msg = (event as any).message;
    if (msg?.role === "user" && !state?.chat_title) {
      const title = compactTitle(textFromContent(msg.content));
      if (title) publish({ chat_title: title });
    }
  });

  pi.on("agent_start", async (_event, ctx) => {
    ctxCurrent = ctx;
    publish({ lifecycle: "thinking", current_tool: null, chat_title: state?.chat_title || titleFromSession(ctx) });
  });

  pi.on("turn_start", async (_event, ctx) => {
    ctxCurrent = ctx;
    publish({ lifecycle: "thinking", current_tool: null, turn_count: (state?.turn_count || 0) + 1 });
  });

  pi.on("tool_execution_start", async (event, ctx) => {
    ctxCurrent = ctx;
    publish({ lifecycle: "tool", current_tool: event.toolName, tool_count: (state?.tool_count || 0) + 1 });
  });

  pi.on("tool_execution_end", async (_event, ctx) => {
    ctxCurrent = ctx;
    publish({ lifecycle: "thinking", current_tool: null });
  });

  pi.on("agent_end", async (_event, ctx) => {
    ctxCurrent = ctx;
    publish({ lifecycle: "idle", current_tool: null });
  });

  pi.on("session_shutdown", async () => {
    if (timer) clearInterval(timer);
    publish({ lifecycle: "offline", current_tool: null });
  });
}
