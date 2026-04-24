import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { PresetRuntimeState } from "./state.ts";

const MANAGED_SKILL_NAMES = [
  "design-diff",
  "design-director",
  "design-handoff",
  "design-premium-ui",
  "design-redesign",
  "design-review",
  "design-spec",
  "design-tokens",
  "motion-director",
  "animejs-core-api",
  "animejs-performance-a11y",
  "animejs-react-integration",
  "animejs-reviewer",
  "animejs-scene-planner",
  "animejs-scroll-interaction",
  "animejs-svg-motion",
  "animejs-text-splitting",
  "animejs-timelines",
] as const;

const MANAGED_SKILL_SET = new Set<string>(MANAGED_SKILL_NAMES);
const BASE_EXCLUDES = [
  "!skills/design-*",
  "!skills/motion-director",
  "!skills/animejs-*",
] as const;
const MANAGED_PLUS_ENTRIES = new Set(MANAGED_SKILL_NAMES.map((name) => `+skills/${name}`));
const MANAGED_ENTRIES = new Set<string>([
  ...BASE_EXCLUDES,
  ...MANAGED_PLUS_ENTRIES,
]);

interface SettingsShape {
  skills?: unknown;
  [key: string]: unknown;
}

export interface SkillFilterApplyResult {
  path: string;
  changed: boolean;
  visibleManagedSkills: string[];
  entries: string[];
}

function unique(items: string[]): string[] {
  return items.filter((item, index) => item && items.indexOf(item) === index);
}

function managedSkillsForState(state: PresetRuntimeState): string[] {
  return unique([
    ...(state.activeSkills ?? []),
    ...(state.recommendedSkills ?? []),
  ]).filter((name) => MANAGED_SKILL_SET.has(name));
}

function buildManagedEntries(state: PresetRuntimeState): string[] {
  const visibleManagedSkills = managedSkillsForState(state).sort((a, b) => a.localeCompare(b));
  return [
    ...BASE_EXCLUDES,
    ...visibleManagedSkills.map((name) => `+skills/${name}`),
  ];
}

function parseSettings(path: string): SettingsShape {
  if (!existsSync(path)) return {};
  const raw = readFileSync(path, "utf8").trim();
  if (!raw) return {};
  return JSON.parse(raw) as SettingsShape;
}

export function applyPresetSkillFilters(cwd: string, state: PresetRuntimeState): SkillFilterApplyResult {
  const path = join(cwd, ".pi", "settings.json");
  const settings = parseSettings(path);
  const currentSkills = Array.isArray(settings.skills)
    ? settings.skills.filter((value): value is string => typeof value === "string")
    : [];
  const preservedSkills = currentSkills.filter((entry) => !MANAGED_ENTRIES.has(entry));
  const entries = buildManagedEntries(state);
  const nextSkills = [...preservedSkills, ...entries];
  const changed = JSON.stringify(currentSkills) !== JSON.stringify(nextSkills);
  const nextSettings: SettingsShape = { ...settings, skills: nextSkills };

  if (changed) writeFileSync(path, `${JSON.stringify(nextSettings, null, 2)}\n`, "utf8");

  return {
    path,
    changed,
    visibleManagedSkills: managedSkillsForState(state).sort((a, b) => a.localeCompare(b)),
    entries,
  };
}
