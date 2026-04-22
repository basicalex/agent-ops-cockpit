import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { loadPresetRegistry } from "./manifest.ts";
import { registerPresetCommands } from "./commands.ts";
import { renderPresetPrompt } from "./renderer.ts";
import { applyPresetSkillFilters } from "./skill-filters.ts";
import { applyStatus, copyState, materializeState, normalizeMode, persistState, restoreState, type PresetRuntimeState } from "./state.ts";

const PRESET_WIDGET_ID = "aoc-preset-runtime";

export default function (pi: ExtensionAPI) {
  const registry = loadPresetRegistry(process.cwd());
  let activeState: PresetRuntimeState = {};

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
    const lines = activeState.preset ? [
      `Preset ${activeState.preset}${activeState.mode ? `/${activeState.mode}` : ""}${activeState.submode ? `/${activeState.submode}` : ""}`,
      `Active skills: ${activeState.activeSkills?.join(", ") || "none"}`,
      `Recommended skills: ${activeState.recommendedSkills?.join(", ") || "none"}`,
      activeState.handoff?.summary ? `Handoff: ${activeState.handoff.summary}` : "",
      activeState.transitionHistory?.length ? `Recent switch: ${activeState.transitionHistory[activeState.transitionHistory.length - 1]}` : "",
    ].filter(Boolean) : [
      "Preset off",
      "Primary flow: start with aoc, then switch presets live.",
      "Convenience bootstrap: aoc.design starts AOC with design preselected.",
    ];
    ctx.ui?.setWidget?.(PRESET_WIDGET_ID, lines, { placement: "belowEditor" });
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
      ctx.ui?.notify?.(`Preset skill filters drifted. Queuing reload for ${label}: ${loaded}`, "info");
      pi.sendUserMessage("/preset-runtime-reload", { deliverAs: "followUp" });
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
}
