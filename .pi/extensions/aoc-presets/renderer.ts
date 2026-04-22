import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { PresetRecord } from "./manifest.ts";
import type { PresetRuntimeState } from "./state.ts";

function unique(items: string[]): string[] {
  return items.filter((item, index) => item && items.indexOf(item) === index);
}

export function resolveComponentNames(record: PresetRecord, state: PresetRuntimeState): string[] {
  const core = record.manifest.components?.core ?? [];
  const modes = record.manifest.components?.modes ?? {};
  const fallback = record.manifest.components?.default ?? [];
  const modeNames = state.mode ? modes[state.mode] : undefined;
  return unique([...(core ?? []), ...((modeNames ?? fallback) ?? [])]);
}

export function renderPresetPrompt(record: PresetRecord, state: PresetRuntimeState): { text: string; warnings: string[] } {
  const warnings: string[] = [];
  const names = resolveComponentNames(record, state);
  const parts: string[] = [];

  for (const name of names) {
    const path = join(record.componentsDir, `${name}.md`);
    if (!existsSync(path)) {
      warnings.push(`missing component '${name}'`);
      continue;
    }
    const text = readFileSync(path, "utf8").trim();
    if (text) parts.push(text);
  }

  parts.push([
    "## Preset runtime contract",
    `Active preset: ${record.id}${state.mode ? `/${state.mode}` : ""}${state.submode ? `/${state.submode}` : ""}.`,
    state.activeSkills?.length ? `Treat these skills as active now: ${state.activeSkills.join(", ")}.` : "No preset-specific skills are active.",
    state.recommendedSkills?.length ? `Only recommend or route toward these preset skills when the task actually matches: ${state.recommendedSkills.join(", ")}.` : "Do not recommend preset-specific skills unless the user explicitly asks for them.",
    "Keep installed-but-inactive skills dormant; do not bias toward them when the preset is off or when another preset/mode is active.",
  ].filter(Boolean).join("\n"));

  if (state.handoff?.summary) {
    parts.push([
      "## Preset handoff",
      state.handoff.summary,
      state.handoff.activeSkills?.length ? `Prior active skills: ${state.handoff.activeSkills.join(", ")}.` : "",
      state.handoff.recommendedSkills?.length ? `Prior recommended skills: ${state.handoff.recommendedSkills.join(", ")}.` : "",
    ].filter(Boolean).join("\n"));
  }

  if (state.mode === "motion") {
    const submode = state.submode?.trim();
    if (submode) {
      parts.push([
        "## Active motion submode",
        `Use motion submode: ${submode}.`,
        "Keep the design preset active while biasing the response toward this motion domain.",
      ].join("\n"));
    }
  }

  return {
    text: parts.join("\n\n").trim(),
    warnings,
  };
}
