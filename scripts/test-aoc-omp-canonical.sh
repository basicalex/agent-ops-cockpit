#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

project="$tmp/project"
omp_runtime="$tmp/omp-agent"
mkdir -p "$project" "$omp_runtime"

AOC_INIT_SKIP_BUILD=1 AOC_OMP_AGENT_DIR="$omp_runtime" bash "$root/bin/aoc-init" "$project"

for required in \
  .aoc/context.md \
  .omp/extensions \
  .omp/agents \
  .omp/skills; do
  if [[ ! -e "$project/$required" ]]; then
    echo "ERROR: missing initialized project asset: $required" >&2
    exit 1
  fi
done

if [[ -e "$project/.pi" ]]; then
  echo "ERROR: aoc-init created legacy .pi directory" >&2
  exit 1
fi

for required in \
  aoc-codegraph.ts \
  aoc-mind.ts \
  aoc-commit.ts \
  aoc-state.ts \
  aoc-dox.ts \
  aoc-dox-command.ts \
  aoc-jj-init.ts \
  aoc-brand-content.ts \
  aoc-web-search.ts; do
  [[ -f "$omp_runtime/extensions/$required" ]] || { echo "ERROR: missing OMP extension: $required" >&2; exit 1; }
done

for required in \
  brand-strategy.md \
  brand-concept.md \
  svg-asset.md \
  hyperframes-content.md \
  dox-scout.md \
  dox-mapper.md \
  dox-critic.md \
  dox-writer.md; do
  [[ -f "$omp_runtime/agents/$required" ]] || { echo "ERROR: missing OMP agent: $required" >&2; exit 1; }
done

for required in \
  animejs-core-api \
  animejs-performance-a11y \
  animejs-react-integration \
  animejs-reviewer \
  animejs-scene-planner \
  animejs-scroll-interaction \
  animejs-svg-motion \
  animejs-text-splitting \
  animejs-timelines \
  aoc-dox-cartography \
  aoc-gaps \
  aoc-hyperframes \
  aoc-init-ops \
  aoc-lexicon \
  aoc-map \
  aoc-stm \
  aoc-understand \
  aoc-update \
  architecture-design \
  design-diff \
  design-director \
  design-handoff \
  design-premium-ui \
  design-redesign \
  design-review \
  design-spec \
  design-tokens \
  enforce-dashboard-ux-guardrails \
  frontend-design \
  funnel-design \
  gsap \
  hyperframes \
  hyperframes-cli \
  motion-director \
  omarchy-theme-ops \
  rlm-analysis \
  safe-gamification \
  spec-rpg-authoring \
  tm-cc \
  website-to-hyperframes; do
  [[ -f "$omp_runtime/skills/$required/SKILL.md" ]] || { echo "ERROR: missing OMP skill: $required" >&2; exit 1; }
done

doctor_log="$tmp/doctor.log"
(cd "$project" && bash "$root/bin/aoc-doctor") >"$doctor_log"
for forbidden in '.pi/settings.json' 'PI runtime' 'pi-multi-auth'; do
  if grep -Fq "$forbidden" "$doctor_log"; then
    echo "ERROR: aoc-doctor mentioned retired surface: $forbidden" >&2
    exit 1
  fi
done

echo "PASS: AOC OMP canonical init"
