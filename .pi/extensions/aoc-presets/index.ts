import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { truncateToWidth, visibleWidth } from "@mariozechner/pi-tui";
import { CAVEMAN_EVENT_SET_LEVEL } from "../lib/caveman.ts";
import { loadPresetRegistry } from "./manifest.ts";
import { registerPresetCommands } from "./commands.ts";
import { renderPresetPrompt } from "./renderer.ts";
import { applyPresetSkillFilters } from "./skill-filters.ts";
import { applyStatus, copyState, materializeState, normalizeMode, persistState, restoreState, type PresetRuntimeState } from "./state.ts";

const PRESET_WIDGET_ID = "aoc-preset-runtime";

function readCurrentCavemanLevel(ctx: any): "off" | "lite" | "full" | "ultra" {
  for (const entry of [...(ctx.sessionManager?.getBranch?.() ?? [])].reverse()) {
    if ((entry as any).type !== "custom" || (entry as any).customType !== "caveman-level-v1") continue;
    const data = (entry as any).data;
    const restored = data?.cavemanLevel ?? data?.level;
    if (restored === "off" || restored === "lite" || restored === "full" || restored === "ultra") return restored;
  }
  return "off";
}

function formatCavemanBadge(level: "off" | "lite" | "full" | "ultra"): string {
  const glyph = level === "off" ? "·" : level === "lite" ? "◇" : level === "full" ? "◈" : "◆";
  return `🪨${glyph}${level !== "off" ? level : ""}`;
}

export default function (pi: ExtensionAPI) {
  const registry = loadPresetRegistry(process.cwd());
  let activeState: PresetRuntimeState = {};
  let lastCtx: any;

  function setState(next: PresetRuntimeState): void {
    activeState = copyState(next);
  }

  function syncFromEnv(ctx: any): void {
    const envPreset = String(process.env.AOC_PRESET || "").trim();
    if (!envPreset) return;
    if (activeState.preset) return;
    const record = registry.get(envPreset);
    if (!record) {
      ctx.ui?.notify?.(`Unknown AOC preset from env: ${envPreset}`, "warning");
      return;
    }
    const envMode = String(process.env.AOC_PRESET_MODE || "").trim() || record.manifest.defaultMode;
    const envSubmode = String(process.env.AOC_PRESET_SUBMODE || "").trim() || undefined;
    const normalized = { ...normalizeMode(record, envMode, envSubmode), source: String(process.env.AOC_PRESET_SOURCE || "layout"), updatedAt: Date.now() };
    const next = materializeState(registry, activeState, normalized);
    setState(next);
    persistState(pi, next);
  }

  function renderWidget(ctx: any): void {
    lastCtx = ctx;
    const caveman = formatCavemanBadge(readCurrentCavemanLevel(ctx));
    const primary = activeState.preset
      ? `Preset ${activeState.preset}${activeState.mode ? `/${activeState.mode}` : ""}${activeState.submode ? `/${activeState.submode}` : ""}`
      : "Preset off";
    const detailLines = activeState.preset ? [
      `Active skills: ${activeState.activeSkills?.join(", ") || "none"}`,
      `Recommended skills: ${activeState.recommendedSkills?.join(", ") || "none"}`,
      activeState.handoff?.summary ? `Handoff: ${activeState.handoff.summary}` : "",
      activeState.transitionHistory?.length ? `Recent switch: ${activeState.transitionHistory[activeState.transitionHistory.length - 1]}` : "",
    ].filter(Boolean) : [];
    ctx.ui?.setWidget?.(PRESET_WIDGET_ID, (_tui: any, _theme: any) => ({
      invalidate() {},
      render(width: number): string[] {
        const left = truncateToWidth(primary, Math.max(1, width - visibleWidth(caveman) - 2), "");
        const spacer = " ".repeat(Math.max(2, width - visibleWidth(left) - visibleWidth(caveman)));
        return [
          truncateToWidth(`${left}${spacer}${caveman}`, width),
          ...detailLines.map((line) => truncateToWidth(line, width)),
        ];
      },
    }), { placement: "belowEditor" });
  }

  function hydrate(ctx: any): void {
    setState(restoreState(ctx));
    syncFromEnv(ctx);
    applyStatus(ctx, registry, activeState);
    renderWidget(ctx);
    const warnings = activeState.preset ? registry.get(activeState.preset)?.warnings ?? [] : [];
    if (warnings.length > 0) ctx.ui?.notify?.(`preset warnings: ${warnings.join("; ")}`, "warning");
  }

  registerPresetCommands(pi, {
    registry,
    getState: () => copyState(activeState),
    setState,
  });

  pi.on("session_start", async (_event, ctx) => {
    hydrate(ctx);
    const filterSync = applyPresetSkillFilters(process.cwd(), activeState);
    if (filterSync.changed) {
      const label = activeState.preset
        ? `${activeState.preset}/${activeState.mode || "default"}${activeState.submode ? `/${activeState.submode}` : ""}`
        : "preset off";
      const loaded = filterSync.visibleManagedSkills.length ? filterSync.visibleManagedSkills.join(", ") : "base AOC only";
      ctx.ui?.notify?.(`Preset skill filters drifted for ${label}: ${loaded}. Run /reload to refresh visible skills.`, "info");
      return;
    }
  });

  pi.on("before_agent_start", async (event: any, ctx) => {
    if (!activeState.preset || !event?.systemPrompt) return;
    const record = registry.get(activeState.preset);
    if (!record) return;
    const rendered = renderPresetPrompt(record, activeState);
    if (!rendered.text) return;
    applyStatus(ctx, registry, activeState);
    renderWidget(ctx);
    return {
      systemPrompt: `${event.systemPrompt}\n\n[AOC PRESET: ${activeState.preset}${activeState.mode ? `/${activeState.mode}` : ""}${activeState.submode ? `/${activeState.submode}` : ""}]\n${rendered.text}`,
    };
  });

  pi.on("message_end", async (_event, ctx) => {
    applyStatus(ctx, registry, activeState);
    renderWidget(ctx);
  });

  pi.events.on(CAVEMAN_EVENT_SET_LEVEL, () => {
    if (!lastCtx) return;
    applyStatus(lastCtx, registry, activeState);
    renderWidget(lastCtx);
  });
}
