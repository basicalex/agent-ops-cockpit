import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import type { PresetRecord } from "./manifest.ts";

export const PRESET_ENTRY_TYPE = "aoc-preset-state-v2";
export const LEGACY_PRESET_ENTRY_TYPE = "aoc-preset-state-v1";

export interface PresetHandoff {
  from?: string;
  to?: string;
  summary?: string;
  activeSkills?: string[];
  recommendedSkills?: string[];
  updatedAt?: number;
}

export interface PresetRuntimeState {
  preset?: string;
  mode?: string;
  submode?: string;
  source?: string;
  updatedAt?: number;
  activeSkills?: string[];
  recommendedSkills?: string[];
  handoff?: PresetHandoff;
  transitionHistory?: string[];
}

export interface PresetStore {
  active: PresetRuntimeState;
}

function unique(items: string[]): string[] {
  return items.filter((item, index) => item && items.indexOf(item) === index);
}

function copyHandoff(handoff?: PresetHandoff | null): PresetHandoff | undefined {
  if (!handoff) return undefined;
  return {
    from: handoff.from,
    to: handoff.to,
    summary: handoff.summary,
    activeSkills: [...(handoff.activeSkills ?? [])],
    recommendedSkills: [...(handoff.recommendedSkills ?? [])],
    updatedAt: handoff.updatedAt,
  };
}

function labelForState(state?: PresetRuntimeState | null): string | undefined {
  if (!state?.preset) return undefined;
  return `${state.preset}:${state.mode || "default"}${state.submode ? `/${state.submode}` : ""}`;
}

function lookupNote(record: PresetRecord | undefined, state: PresetRuntimeState): string | undefined {
  if (!record || !state.mode) return undefined;
  const modeNote = record.manifest.handoff?.modeNotes?.[state.mode]?.trim();
  const submodeNote = state.submode ? record.manifest.handoff?.submodeNotes?.[state.submode]?.trim() : undefined;
  return [modeNote, submodeNote].filter(Boolean).join(" ").trim() || undefined;
}

function buildHandoff(prev: PresetRuntimeState | undefined, next: PresetRuntimeState, prevRecord?: PresetRecord, nextRecord?: PresetRecord): PresetHandoff | undefined {
  if (!prev?.preset) return copyHandoff(prev?.handoff ?? next.handoff);
  const from = labelForState(prev);
  const to = labelForState(next) || "preset:off";
  const fromNote = lookupNote(prevRecord, prev);
  const toNote = next.preset ? lookupNote(nextRecord, next) : undefined;
  const summary = [
    `Transition ${from} -> ${to}.`,
    fromNote ? `Carry forward: ${fromNote}` : "",
    toNote ? `Focus now: ${toNote}` : "",
  ].filter(Boolean).join(" ").trim();

  return {
    from,
    to,
    summary: summary || undefined,
    activeSkills: [...(prev.activeSkills ?? [])],
    recommendedSkills: [...(prev.recommendedSkills ?? [])],
    updatedAt: Date.now(),
  };
}

export function emptyState(): PresetRuntimeState {
  return {
    preset: undefined,
    mode: undefined,
    submode: undefined,
    source: undefined,
    updatedAt: undefined,
    activeSkills: [],
    recommendedSkills: [],
    handoff: undefined,
    transitionHistory: [],
  };
}

export function copyState(state?: PresetRuntimeState | null): PresetRuntimeState {
  return {
    preset: state?.preset,
    mode: state?.mode,
    submode: state?.submode,
    source: state?.source,
    updatedAt: state?.updatedAt,
    activeSkills: [...(state?.activeSkills ?? [])],
    recommendedSkills: [...(state?.recommendedSkills ?? [])],
    handoff: copyHandoff(state?.handoff),
    transitionHistory: [...(state?.transitionHistory ?? [])],
  };
}

export function resolveSkillState(record: PresetRecord, state: PresetRuntimeState): Pick<PresetRuntimeState, "activeSkills" | "recommendedSkills"> {
  const skills = record.manifest.skills ?? {};
  const mode = state.mode || record.manifest.defaultMode;
  const submode = state.submode;
  const activeSkills = unique([
    ...(skills.active ?? []),
    ...(mode ? skills.activeByMode?.[mode] ?? [] : []),
    ...(submode ? skills.activeBySubmode?.[submode] ?? [] : []),
  ]);
  const recommendedSkills = unique([
    ...(skills.recommended ?? []),
    ...(mode ? skills.recommendedByMode?.[mode] ?? [] : []),
    ...(submode ? skills.recommendedBySubmode?.[submode] ?? [] : []),
  ]);
  return { activeSkills, recommendedSkills };
}

export function normalizeMode(record: PresetRecord, mode?: string, submode?: string): PresetRuntimeState {
  const modes = record.manifest.components?.modes ?? {};
  const defaultMode = record.manifest.defaultMode || Object.keys(modes)[0] || "default";
  let resolvedMode = (mode || "").trim() || defaultMode;
  if (!modes[resolvedMode]) resolvedMode = defaultMode;
  const next: PresetRuntimeState = {
    preset: record.id,
    mode: resolvedMode,
    submode: submode?.trim() || undefined,
  };
  return { ...next, ...resolveSkillState(record, next) };
}

function buildHistory(prev: PresetRuntimeState | undefined, next: PresetRuntimeState): string[] {
  const prior = [...(prev?.transitionHistory ?? [])];
  const from = labelForState(prev) || "preset:off";
  const to = labelForState(next) || "preset:off";
  if (from === to) return prior.slice(-5);
  const stamp = new Date(next.updatedAt || Date.now()).toISOString().replace("T", " ").slice(0, 16);
  return [...prior, `${stamp} ${from} -> ${to}`].slice(-5);
}

export function materializeState(registry: Map<string, PresetRecord>, prev: PresetRuntimeState | undefined, next: PresetRuntimeState): PresetRuntimeState {
  const base = copyState(next);
  if (base.preset) {
    const record = registry.get(base.preset);
    if (record) Object.assign(base, resolveSkillState(record, base));
  } else {
    base.activeSkills = [];
    base.recommendedSkills = [];
  }
  const prevRecord = prev?.preset ? registry.get(prev.preset) : undefined;
  const nextRecord = base.preset ? registry.get(base.preset) : undefined;
  base.handoff = buildHandoff(prev, base, prevRecord, nextRecord);
  base.transitionHistory = buildHistory(prev, base);
  return base;
}

export function restoreState(ctx: ExtensionContext): PresetRuntimeState {
  const entries = ctx.sessionManager.getEntries?.() ?? [];
  let latest = emptyState();
  for (const entry of entries as any[]) {
    if (entry?.type !== "custom" || !entry?.data) continue;
    if (entry?.customType !== PRESET_ENTRY_TYPE && entry?.customType !== LEGACY_PRESET_ENTRY_TYPE) continue;
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
  const suffix = state.activeSkills?.length ? `+${state.activeSkills.length}` : "";
  const text = state.preset ? `${badge}:${state.mode || "default"}${state.submode ? `/${state.submode}` : ""}${suffix}` : "preset:off";
  ctx.ui?.setStatus?.("preset", text);
}

export function formatHistory(state: PresetRuntimeState): string {
  if (!state.transitionHistory?.length) return "history: none";
  return ["preset history:", ...state.transitionHistory].join("\n");
}

export function formatHandoff(state: PresetRuntimeState): string {
  const handoff = state.handoff;
  if (!handoff?.summary) return "handoff: none";
  return [
    `handoff: ${handoff.from || "unknown"} -> ${handoff.to || "unknown"}`,
    handoff.summary,
    handoff.activeSkills?.length ? `prior active skills: ${handoff.activeSkills.join(", ")}` : "",
    handoff.recommendedSkills?.length ? `prior recommended skills: ${handoff.recommendedSkills.join(", ")}` : "",
  ].filter(Boolean).join("\n");
}

export function describeState(registry: Map<string, PresetRecord>, state: PresetRuntimeState): string {
  if (!state.preset) {
    return ["preset: off", state.handoff?.summary ? formatHandoff(state) : ""].filter(Boolean).join("\n");
  }
  const preset = registry.get(state.preset);
  const label = preset?.manifest.label || state.preset;
  const warnings = preset?.warnings?.length ? `warnings: ${preset.warnings.join("; ")}` : "";
  return [
    `preset: ${label} (${state.preset})`,
    `mode: ${state.mode || preset?.manifest.defaultMode || "default"}`,
    `submode: ${state.submode || "none"}`,
    `source: ${state.source || "unknown"}`,
    `active skills: ${state.activeSkills?.join(", ") || "none"}`,
    `recommended skills: ${state.recommendedSkills?.join(", ") || "none"}`,
    warnings,
    state.handoff?.summary ? formatHandoff(state) : "",
    state.transitionHistory?.length ? formatHistory(state) : "",
  ].filter(Boolean).join("\n");
}
