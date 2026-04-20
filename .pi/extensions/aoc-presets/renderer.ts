import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { PresetRecord } from "./manifest.ts";
import type { PresetRuntimeState } from "./state.ts";

function unique(items: string[]): string[] {
  return items.filter((item, index) => item && items.indexOf(item) === index);
}

export function resolveComponentNames(record: PresetRecord, state: PresetRuntimeState): string[] {
  const modes = record.manifest.components?.modes ?? {};
  const fallback = record.manifest.components?.default ?? [];
  const modeNames = state.mode ? modes[state.mode] : undefined;
  return unique(modeNames ?? fallback);
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
