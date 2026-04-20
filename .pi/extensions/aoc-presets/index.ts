import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { loadPresetRegistry } from "./manifest.ts";
import { registerPresetCommands } from "./commands.ts";
import { renderPresetPrompt } from "./renderer.ts";
import { applyStatus, copyState, normalizeMode, persistState, restoreState, type PresetRuntimeState } from "./state.ts";

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
    const next = { ...normalizeMode(record, envMode, envSubmode), source: String(process.env.AOC_PRESET_SOURCE || "layout"), updatedAt: Date.now() };
    setState(next);
    persistState(pi, next);
  }

  function hydrate(ctx: any): void {
    setState(restoreState(ctx));
    syncFromEnv(ctx);
    applyStatus(ctx, registry, activeState);
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
  });

  pi.on("before_agent_start", async (event: any, ctx) => {
    if (!activeState.preset || !event?.systemPrompt) return;
    const record = registry.get(activeState.preset);
    if (!record) return;
    const rendered = renderPresetPrompt(record, activeState);
    if (!rendered.text) return;
    applyStatus(ctx, registry, activeState);
    return {
      systemPrompt: `${event.systemPrompt}\n\n[AOC PRESET: ${activeState.preset}${activeState.mode ? `/${activeState.mode}` : ""}${activeState.submode ? `/${activeState.submode}` : ""}]\n${rendered.text}`,
    };
  });

  pi.on("message_end", async (_event, ctx) => {
    applyStatus(ctx, registry, activeState);
  });
}
