#!/usr/bin/env bash
set -euo pipefail

# aoc-init owns only the marked block in AGENTS.md. Project lines in an old
# whole-file contract move under "## Project rules" once and stay there.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" XDG_CONFIG_HOME="$tmp/config"
export AOC_OMP_AGENT_DIR="$tmp/omp-agent" AOC_CLAUDE_INSTALL_OFFLINE=1
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$AOC_OMP_AGENT_DIR"

fail() { echo "ERROR: $*" >&2; exit 1; }
start='<!-- aoc:managed:start -->'
end='<!-- aoc:managed:end -->'
line_a='- Project rule A: run `make check` before a commit.'
line_b='- Project rule B: `docs/specs/` holds the specs.'
line_c='- Project rule C: never edit `vendor/`.'

project="$tmp/project"
mkdir -p "$project"
cat > "$project/AGENTS.md" <<EOF
# AOC Architecture & Agent Guidelines

This file defines the always-on rules for agents in this repo. Procedural playbooks live in AOC skills.

## Always-on rules
- Use \`.aoc/context.md\` for orientation; run \`aoc-init\` if it is missing or stale.
- RTK routing is default-on for new AOC projects (\`.aoc/rtk.toml\` mode=\`on\`); existing explicit mode=\`off\` is preserved.
$line_a

## Low-Token Default Mode
- Keep responses concise by default; do not print full files or raw logs unless explicitly requested.
- When \`.codegraph/\` exists and the OMP \`aoc_codegraph\` tool is available, use it before broad text/file scans for code discovery, call-flow mapping, impact probes, and affected-test selection.

## Core files
- \`.aoc/context.md\`: auto-generated project snapshot.
$line_b
- \`.aoc/rtk.toml\`: project-local RTK routing policy and install contract.

## Skills (load when needed)
Load a skill only when its description matches the user request. Keep always-on guidance here minimal; procedural playbooks belong in skill files.

$line_c
EOF

run_init() {
  local log="$1"
  AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$project" > "$log" 2>&1 || { cat "$log" >&2; fail "aoc-init failed"; }
}

run_init "$tmp/first.log"
for line in "$line_a" "$line_b" "$line_c"; do
  grep -Fq "kept project line: $line" "$tmp/first.log" || fail "first run did not report kept line: $line"
done
cp "$project/AGENTS.md" "$tmp/after-first.md"

run_init "$tmp/second.log"
grep -Fq 'AGENTS.md managed block already up to date.' "$tmp/second.log" || fail 'second run rewrote the managed block'
cmp -s "$tmp/after-first.md" "$project/AGENTS.md" || fail 'second run changed AGENTS.md'

agents="$project/AGENTS.md"
[[ "$(head -n 1 "$agents")" == "$start" ]] || fail 'AGENTS.md does not start with the managed block'
[[ "$(grep -Fxc "$start" "$agents")" == 1 && "$(grep -Fxc "$end" "$agents")" == 1 ]] || fail 'managed markers are not unique'
[[ "$(grep -Fxc '## Project rules' "$agents")" == 1 ]] || fail 'missing a single Project rules section'
for line in "$line_a" "$line_b" "$line_c"; do
  [[ "$(grep -Fxc -- "$line" "$agents")" == 1 ]] || fail "project line not kept exactly once: $line"
done
if grep -Eq 'codegraph|aoc-rtk|rtk\.toml|RTK' "$agents"; then
  fail 'retired template lines survived the migration'
fi

# Project lines sit after the block, in their original order.
python3 - "$agents" "$end" "$line_a" "$line_b" "$line_c" <<'PY'
import sys
lines = open(sys.argv[1], encoding="utf-8").read().splitlines()
end = lines.index(sys.argv[2])
positions = [lines.index(line) for line in sys.argv[3:]]
if not all(pos > end for pos in positions):
    raise SystemExit("ERROR: project lines are inside the managed block")
if positions != sorted(positions):
    raise SystemExit("ERROR: project lines changed order")
PY

# The block matches what a fresh project gets.
fresh="$tmp/fresh"
mkdir -p "$fresh"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$fresh" > "$tmp/fresh.log" 2>&1 || { cat "$tmp/fresh.log" >&2; fail 'fresh aoc-init failed'; }
block() { sed -n "\\|^$start\$|,\\|^$end\$|p" "$1"; }
[[ "$(block "$agents")" == "$(block "$fresh/AGENTS.md")" ]] || fail 'migrated block differs from the fresh block'
cmp -s "$fresh/AGENTS.md" <(block "$fresh/AGENTS.md") || fail 'fresh AGENTS.md holds more than the managed block'

# A stale block is refreshed and nothing outside it changes.
cat > "$agents" <<EOF
# My project
Intro kept as is.

$start
# AOC Architecture & Agent Guidelines
- When \`.codegraph/\` exists, use it.
$end

## Notes
$line_c
EOF
run_init "$tmp/third.log"
grep -Fq 'Updated the AOC-managed block in AGENTS.md.' "$tmp/third.log" || fail 'stale block was not refreshed'
grep -Fq 'codegraph' "$agents" && fail 'stale block content survived'
[[ "$(sed -n '1,3p' "$agents")" == $'# My project\nIntro kept as is.' ]] || fail 'text before the block changed'
[[ "$(tail -n 2 "$agents")" == "## Notes"$'\n'"$line_c" ]] || fail 'text after the block changed'
[[ "$(block "$agents")" == "$(block "$fresh/AGENTS.md")" ]] || fail 'refreshed block differs from the fresh block'

printf 'PASS: AGENTS.md project lines kept once; managed block refreshed in place\n'
