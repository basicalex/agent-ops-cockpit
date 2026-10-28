#!/usr/bin/env bash
set -euo pipefail

# aoc-init adds its .gitignore rules once. A second run leaves the file
# byte-identical, and the backup folders it creates stay out of git.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" XDG_CONFIG_HOME="$tmp/config"
export AOC_OMP_AGENT_DIR="$tmp/omp-agent" AOC_CLAUDE_INSTALL_OFFLINE=1
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$AOC_OMP_AGENT_DIR"

fail() { echo "ERROR: $*" >&2; exit 1; }
run_init() {
  local log="$1"
  AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$project" > "$log" 2>&1 || { cat "$log" >&2; fail "aoc-init failed"; }
}

project="$tmp/project"
mkdir -p "$project"
git -C "$project" init -q
# No trailing newline: aoc-init adds exactly one before its rules.
printf 'node_modules/' > "$project/.gitignore"

run_init "$tmp/first.log"
ignore="$project/.gitignore"
[[ "$(head -n 2 "$ignore")" == $'node_modules/\n# AOC tracking policy (managed by aoc-init)' ]] ||
  fail 'first run did not add one newline before its rules'
[[ "$(grep -Fxc '.aoc-backups/' "$ignore")" == 1 ]] || fail 'backup ignore rule missing or repeated'
grep -q '^$' "$ignore" && fail 'first run wrote a blank line'
cp "$ignore" "$tmp/after-first"

run_init "$tmp/second.log"
cmp -s "$tmp/after-first" "$ignore" || { diff "$tmp/after-first" "$ignore" >&2 || true; fail 'second run changed .gitignore'; }
run_init "$tmp/third.log"
cmp -s "$tmp/after-first" "$ignore" || fail 'third run changed .gitignore'

# The rule must win over the !/.aoc/** and !/.omp/... re-includes.
for rel in .omp/.aoc-backups/skills.aoc-dirty-backup.1/SKILL.md \
  .aoc/presets/.aoc-backups/x.aoc-unmanaged-backup.1 .pi/skills/.aoc-backups/y; do
  mkdir -p "$(dirname "$project/$rel")"
  printf 'old\n' > "$project/$rel"
  git -C "$project" check-ignore -q "$rel" || fail "backup path not ignored: $rel"
done
for rel in .aoc/context.md AGENTS.md; do
  git -C "$project" check-ignore -q "$rel" && fail "tracked AOC file ignored: $rel"
done

# An older repo that already has the tracking rules gets only the new line.
old="$tmp/old"
mkdir -p "$old"
sed '/^\.aoc-backups\/$/d' "$tmp/after-first" > "$old/.gitignore"
project="$old"
run_init "$tmp/old.log"
diff <(sed '/^\.aoc-backups\/$/d' "$old/.gitignore") <(sed '/^\.aoc-backups\/$/d' "$tmp/after-first") >/dev/null ||
  fail 'older repo .gitignore changed beyond the backup rule'
[[ "$(tail -n 1 "$old/.gitignore")" == '.aoc-backups/' ]] || fail 'backup rule not appended to older repo'

printf 'PASS: .gitignore stable across runs; AOC backup folders ignored\n'
