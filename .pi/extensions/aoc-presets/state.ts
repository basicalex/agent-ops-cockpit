import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import type { PresetRecord } from "./manifest.ts";

export const PRESET_ENTRY_TYPE = "aoc-preset-state-v1";

export interface PresetRuntimeState {
  preset?: string;
  mode?: string;
  submode?: string;
  source?: string;
  updatedAt?: number;
}

export interface PresetStore {
  active: PresetRuntimeState;
}

export function emptyState(): PresetRuntimeState {
  return { preset: undefined, mode: undefined, submode: undefined, source: undefined, updatedAt: undefined };
}

export function copyState(state?: PresetRuntimeState | null): PresetRuntimeState {
  return {
    preset: state?.preset,
    mode: state?.mode,
    submode: state?.submode,
    source: state?.source,
    updatedAt: state?.updatedAt,
  };
}

export function normalizeMode(record: PresetRecord, mode?: string, submode?: string): PresetRuntimeState {
  const modes = record.manifest.components?.modes ?? {};
  const defaultMode = record.manifest.defaultMode || Object.keys(modes)[0] || "default";
  let resolvedMode = (mode || "").trim() || defaultMode;
  if (!modes[resolvedMode]) resolvedMode = defaultMode;
  return {
    preset: record.id,
    mode: resolvedMode,
    submode: submode?.trim() || undefined,
  };
}

export function restoreState(ctx: ExtensionContext): PresetRuntimeState {
  const entries = ctx.sessionManager.getEntries?.() ?? [];
  let latest = emptyState();
  for (const entry of entries as any[]) {
    if (entry?.type !== "custom" || entry?.customType !== PRESET_ENTRY_TYPE || !entry?.data) continue;
    latest = copyState(entry.data as PresetRuntimeState);
  }
  return latest;
}

export function persistState(pi: ExtensionAPI, state: PresetRuntimeState): void {
  pi.appendEntry<PresetRuntimeState>(PRESET_ENTRY_TYPE, copyState(state));
}

export function applyStatus(ctx: ExtensionContext, registry: Map<string, PresetRecord>, state: PresetRuntimeState): void {
  const preset = state.preset ? registry.get(state.preset) : undefined;
  const badge = preset?.manifest.runtime?.statusBadge || state.preset?.toUpperCase() || "preset";
  const text = state.preset ? `${badge}:${state.mode || "default"}${state.submode ? `/${state.submode}` : ""}` : "preset:off";
  ctx.ui?.setStatus?.("preset", text);
}

export function describeState(registry: Map<string, PresetRecord>, state: PresetRuntimeState): string {
  if (!state.preset) return "preset: off";
  const preset = registry.get(state.preset);
  const label = preset?.manifest.label || state.preset;
  const warnings = preset?.warnings?.length ? `\nwarnings: ${preset.warnings.join("; ")}` : "";
  return [
    `preset: ${label} (${state.preset})`,
    `mode: ${state.mode || preset?.manifest.defaultMode || "default"}`,
    `submode: ${state.submode || "none"}`,
    `source: ${state.source || "unknown"}`,
    warnings.trim(),
  ].filter(Boolean).join("\n");
}
