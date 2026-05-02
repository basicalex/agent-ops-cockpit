#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

if ! command -v bun >/dev/null 2>&1; then
  echo "ERROR: bun is required for preset extension smoke tests" >&2
  exit 1
fi

BUN_SCRIPT="$(mktemp "$repo_root/.aoc/preset-smoke.XXXXXX.ts")"
trap 'rm -f "$BUN_SCRIPT"' EXIT

cat >"$BUN_SCRIPT" <<'TS'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { mkdtempSync } from "node:fs";
import { loadPresetRegistry } from "../.pi/extensions/aoc-presets/manifest.ts";
import { materializeState, normalizeMode } from "../.pi/extensions/aoc-presets/state.ts";
import { resolveComponentNames } from "../.pi/extensions/aoc-presets/renderer.ts";
import { applyPresetSkillFilters } from "../.pi/extensions/aoc-presets/skill-filters.ts";

function fail(message: string): never {
  console.error(`FAIL: ${message}`);
  process.exit(1);
}

function assert(condition: unknown, message: string): void {
  if (!condition) fail(message);
}

const root = process.cwd();
const registry = loadPresetRegistry(root);
assert(registry.size > 0, "no presets loaded");
assert(registry.has("design"), "design preset missing");
assert(registry.has("hyperframes"), "hyperframes preset missing");

for (const [id, record] of registry.entries()) {
  assert(record.manifest.id === id, `${id}: manifest id '${record.manifest.id}' does not match directory`);
  assert(record.warnings.length === 0, `${id}: warnings: ${record.warnings.join("; ")}`);
  assert(record.manifest.defaultMode, `${id}: missing defaultMode`);

  const modes = record.manifest.components?.modes ?? {};
  assert(Object.keys(modes).length > 0, `${id}: no component modes`);
  assert(modes[record.manifest.defaultMode], `${id}: defaultMode has no component mode`);

  for (const mode of Object.keys(modes)) {
    const normalized = normalizeMode(record, mode);
    const state = materializeState(registry, {}, normalized);
    assert(state.preset === id, `${id}/${mode}: materialized wrong preset`);
    assert(state.mode === mode, `${id}/${mode}: materialized wrong mode '${state.mode}'`);
    const components = resolveComponentNames(record, state);
    assert(components.length > 0, `${id}/${mode}: no resolved components`);
    for (const component of components) {
      const path = join(record.componentsDir, `${component}.md`);
      assert(existsSync(path), `${id}/${mode}: missing component ${path}`);
    }
  }
}

const tmpRoot = mkdtempSync(join(tmpdir(), "aoc-preset-smoke-"));
mkdirSync(join(tmpRoot, ".aoc"), { recursive: true });
mkdirSync(join(tmpRoot, ".pi"), { recursive: true });
writeFileSync(join(tmpRoot, ".pi", "settings.json"), JSON.stringify({ skills: ["custom-skill", "+skills/aoc-hyperframes", "!skills/hyperframes"] }, null, 2) + "\n");
process.env.AOC_PROJECT_ROOT = tmpRoot;

const result = applyPresetSkillFilters("/", {
  preset: "hyperframes",
  mode: "compose",
  activeSkills: ["aoc-hyperframes"],
  recommendedSkills: [],
});
assert(result.path === join(tmpRoot, ".pi", "settings.json"), "skill filters did not use AOC_PROJECT_ROOT");
assert(result.visibleManagedSkills.join(",") === "aoc-hyperframes", "wrong visible managed skills");
const settings = JSON.parse(readFileSync(result.path, "utf8"));
assert(settings.skills.includes("custom-skill"), "preserved custom skill missing");
assert(settings.skills.includes("+skills/aoc-hyperframes"), "aoc-hyperframes skill not enabled");
assert(settings.skills.includes("!skills/hyperframes"), "hyperframes helper exclude missing");
assert(settings.skills.includes("!skills/design-*"), "design exclude missing");

console.log(`OK: ${registry.size} presets validated; skill filters project-root safe`);
TS

bun "$BUN_SCRIPT"
