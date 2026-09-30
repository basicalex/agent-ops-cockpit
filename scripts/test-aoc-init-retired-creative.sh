#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" XDG_CONFIG_HOME="$tmp/config"
export AOC_OMP_AGENT_DIR="$tmp/omp-agent" AOC_CLAUDE_INSTALL_OFFLINE=1
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$AOC_OMP_AGENT_DIR"

project="$tmp/old-project"
mkdir -p "$project/.aoc" "$project/.omp/skills" "$project/.pi/skills"
cat > "$project/AGENTS.md" <<'EOF'
# User instructions

## Agent Ops Cockpit (AOC)
- Use root `DESIGN.md` before UI, docs-site, marketing, HyperFrames, or other product-facing work.

## User instructions
Keep this section.
EOF

retired=(
  .aoc/presets/hyperframes
  .aoc/presets/design
  .omp/skills/aoc-hyperframes .omp/skills/hyperframes .omp/skills/hyperframes-cli
  .omp/skills/website-to-hyperframes
  .omp/skills/animejs-core-api .omp/skills/design-review
  .omp/skills/motion-director .omp/skills/ponytail-review
  .omp/skills/frontend-design .omp/skills/aoc-stm
  .omp/agents/brand-concept.md .omp/agents/brand-strategy.md
  .omp/agents/hyperframes-content.md .omp/agents/svg-asset.md
  .omp/extensions/aoc-brand-content.ts
  .pi/skills/aoc-hyperframes .pi/skills/hyperframes .pi/skills/hyperframes-cli
  .pi/skills/website-to-hyperframes .pi/skills/gsap
  .pi/prompts/hyperframes.md
  .aoc/open-design .aoc/skills-optional .aoc/prompts-optional
)
for rel in "${retired[@]}"; do
  if [[ "$rel" == *.md || "$rel" == *.ts ]]; then
    mkdir -p "$(dirname "$project/$rel")"
    printf 'old asset\n' > "$project/$rel"
    printf 'aoc-managed: true\n' > "$project/$rel.aoc-managed"
  else
    mkdir -p "$project/$rel"
    printf 'old asset\n' > "$project/$rel/old.txt"
    printf 'aoc-managed: true\n' > "$project/$rel/.aoc-managed"
  fi
done
# Manifest-only ownership (no marker); directory and file cases.
rm "$project/.pi/skills/gsap/.aoc-managed" "$project/.omp/agents/svg-asset.md.aoc-managed" \
  "$project/.omp/skills/design-review/.aoc-managed"
cat > "$project/.aoc/managed-assets.json" <<'EOF'
{"schemaVersion":1,"assets":{".pi/skills/gsap":{"asset":"skill/gsap"},".omp/agents/svg-asset.md":{"asset":"agent/svg-asset"},".omp/skills/design-review":{"asset":"skill/design-review"},".aoc/open-design/old.txt":{"asset":"old"}}}
EOF
mkdir -p "$project/.omp/skills/gsap" "$project/.omp/skills/gsap-custom"
printf 'user owned\n' > "$project/.omp/skills/gsap/SKILL.md"
printf 'user owned\n' > "$project/.omp/skills/gsap-custom/SKILL.md"
# A managed parent was already replaced wholesale before this change. Dirty
# contents are backed up by that existing refresh, not kept in the live tree.
printf 'aoc-managed: true\n' > "$project/.omp/skills/.aoc-managed"

log="$tmp/init.log"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$project" > "$log" 2>&1 || { cat "$log" >&2; exit 1; }
for rel in "${retired[@]}"; do
  [[ ! -e "$project/$rel" && ! -e "$project/$rel.aoc-managed" ]] || { echo "ERROR: retired asset remains: $rel" >&2; exit 1; }
done
backup=("$project/.omp/.aoc-backups/skills.aoc-dirty-backup."*)
[[ ${#backup[@]} -eq 1 && -d "${backup[0]}" ]] || { echo 'ERROR: managed skills backup missing' >&2; exit 1; }
for rel in gsap gsap-custom; do
  [[ -f "${backup[0]}/$rel/SKILL.md" ]] || { echo "ERROR: user asset missing from managed-tree backup: $rel" >&2; exit 1; }
done
grep -Fq 'Preserving unmarked retired creative asset: .omp/skills/gsap' "$log" || { echo 'ERROR: missing unmarked asset warning' >&2; exit 1; }
grep -Fq 'marketing, or other product-facing work' "$project/AGENTS.md" || { echo 'ERROR: managed AOC contract not refreshed' >&2; exit 1; }
python3 - "$project/.aoc/managed-assets.json" <<'PY'
import json
import sys
assets = json.load(open(sys.argv[1], encoding="utf-8"))["assets"]
for name in (".pi/skills/gsap", ".omp/agents/svg-asset.md", ".omp/skills/design-review", ".aoc/open-design/old.txt"):
    if name in assets:
        raise SystemExit(f"ERROR: retired manifest entry remains: {name}")
PY

fresh="$tmp/fresh-project"
mkdir -p "$fresh"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$fresh" > "$tmp/fresh.log" 2>&1 || { cat "$tmp/fresh.log" >&2; exit 1; }
[[ ! -e "$fresh/.aoc/presets/design" ]] || { echo 'ERROR: retired design preset reseeded into fresh project' >&2; exit 1; }
for rel in AGENTS.md DESIGN.md .aoc/effective-agent-contract.md; do
  if [[ -f "$fresh/$rel" ]] && grep -Eiq 'HyperFrames|open-design|brand-content' "$fresh/$rel"; then
    echo "ERROR: retired wording in fresh $rel" >&2
    exit 1
  fi
done
printf 'PASS: retired assets removed; unmarked assets warned and backed up by existing reseeding; fresh contracts clean\n'
