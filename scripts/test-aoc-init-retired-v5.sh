#!/usr/bin/env bash
set -euo pipefail

# Migration v5 removes state left by retired tools and keeps Pi, presets and
# the lexicon. Stale user-level OMP assets are pruned too.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" XDG_CONFIG_HOME="$tmp/config"
export AOC_OMP_AGENT_DIR="$tmp/omp-agent" AOC_CLAUDE_INSTALL_OFFLINE=1
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$AOC_OMP_AGENT_DIR"

fail() { echo "ERROR: $*" >&2; exit 1; }

project="$tmp/old-project"
mkdir -p "$project/.aoc" "$project/.omp/extensions"
printf '{"schemaVersion":1,"projectAocVersion":4}\n' > "$project/.aoc/init-state.json"

retired_files=(
  .aoc/rtk.toml .aoc/memory.md .aoc/memory.md.lock .aoc/effective-agent-contract.md .aoc/search.toml
  .omp/extensions/aoc-mind.ts .omp/extensions/aoc-herdr.ts .omp/extensions/aoc-jj-init.ts
  .omp/extensions/ponytail.ts .omp/extensions/aoc-codegraph.ts .omp/extensions/aoc-web-search.ts
)
retired_dirs=(.aoc/mind .aoc/layouts .aoc/stm .aoc/services/searxng)
for rel in "${retired_files[@]}"; do
  printf 'old\n' > "$project/$rel"
done
for rel in "${retired_dirs[@]}"; do
  mkdir -p "$project/$rel"
  printf 'old\n' > "$project/$rel/old.txt"
done
ln -s .aoc/context.md "$project/.cursorrules"

# Kept: Pi, presets, lexicon, prompts (with the dead tm line rewritten).
mkdir -p "$project/.pi/skills/mine" "$project/.aoc/presets/mine" "$project/.aoc/prompts"
printf 'pi\n' > "$project/.pi/skills/mine/SKILL.md"
printf 'preset\n' > "$project/.aoc/presets/mine/README.md"
printf 'lexicon\n' > "$project/.aoc/lexicon.md"
printf -- '- Avoid broad scans by default. Do NOT run `tm list`, `git status`, or repo-wide `rg` unless the user asks or a targeted check fails.\n' \
  > "$project/.aoc/prompts/omp-low-token.md"
kept=(.pi/skills/mine/SKILL.md .aoc/presets/mine/README.md .aoc/lexicon.md .aoc/prompts/omp-low-token.md)

# User-level OMP dir: retired names, a name recorded by an older aoc-init, and
# a user-owned extension that AOC never installed.
ext_dir="$AOC_OMP_AGENT_DIR/extensions"
mkdir -p "$ext_dir"
for name in aoc-mind.ts aoc-codegraph.ts aoc-web-search.ts old-seeded.ts user-own.ts; do
  printf 'old\n' > "$ext_dir/$name"
done
printf 'old-seeded.ts\n' > "$ext_dir/.aoc-seeded"

log="$tmp/init.log"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$project" > "$log" 2>&1 || { cat "$log" >&2; exit 1; }

for rel in "${retired_files[@]}" "${retired_dirs[@]}" .cursorrules .aoc/services; do
  [[ ! -e "$project/$rel" && ! -L "$project/$rel" ]] || fail "retired path remains: $rel"
done
for rel in "${kept[@]}"; do
  [[ -f "$project/$rel" ]] || fail "kept path removed: $rel"
done
grep -Fq 'tm list' "$project/.aoc/prompts/omp-low-token.md" && fail 'dead tm line kept in omp prompt'
grep -Fq 'Do NOT run `git status` or repo-wide `rg`' "$project/.aoc/prompts/omp-low-token.md" || fail 'omp prompt line not rewritten'
grep -Fq 'Applying AOC project migration v5' "$log" || fail 'v5 migration did not run'
python3 - "$project/.aoc/init-state.json" <<'PY'
import json
import sys
state = json.load(open(sys.argv[1], encoding="utf-8"))
if state.get("projectAocVersion") != 5:
    raise SystemExit(f"ERROR: project version is {state.get('projectAocVersion')}, not 5")
if "v5-retired-tool-cleanup" not in state.get("migrationsApplied", []):
    raise SystemExit("ERROR: v5 migration not recorded")
PY
for name in aoc-mind.ts aoc-codegraph.ts aoc-web-search.ts old-seeded.ts; do
  [[ ! -e "$ext_dir/$name" ]] || fail "stale user OMP extension remains: $name"
done
[[ -f "$ext_dir/user-own.ts" ]] || fail 'user-owned OMP extension removed'
grep -Fxq 'aoc-profile.ts' "$ext_dir/.aoc-seeded" || fail 'seeded extension record not written'
for name in aoc-mind.ts aoc-codegraph.ts aoc-web-search.ts; do
  [[ ! -e "$project/.omp/extensions/$name" ]] || fail "retired extension reseeded: $name"
done

# A real .cursorrules file is not ours to delete.
other="$tmp/real-cursorrules"
mkdir -p "$other/.aoc"
printf '{"schemaVersion":1,"projectAocVersion":4}\n' > "$other/.aoc/init-state.json"
printf 'my rules\n' > "$other/.cursorrules"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$other" > "$tmp/other.log" 2>&1 || { cat "$tmp/other.log" >&2; exit 1; }
[[ -f "$other/.cursorrules" && ! -L "$other/.cursorrules" ]] || fail 'real .cursorrules removed'

# Fresh projects get no search or retired state.
fresh="$tmp/fresh"
mkdir -p "$fresh"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$fresh" > "$tmp/fresh.log" 2>&1 || { cat "$tmp/fresh.log" >&2; exit 1; }
for rel in .aoc/search.toml .aoc/services .aoc/rtk.toml .aoc/mind .omp/extensions/aoc-codegraph.ts .omp/extensions/aoc-web-search.ts; do
  [[ ! -e "$fresh/$rel" ]] || fail "fresh project got retired path: $rel"
done
[[ ! -e "$XDG_CONFIG_HOME/aoc/search" && ! -e "$XDG_CONFIG_HOME/aoc/services" ]] || fail 'search seed written to the config dir'

printf 'PASS: v5 removed retired state; Pi, presets, lexicon and user files kept\n'
