#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
aoc_init_bin="$repo_root/bin/aoc-init"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_exists() {
  local path="$1"
  [[ -e "$path" ]] || fail "Expected path to exist: $path"
}

assert_not_exists() {
  local path="$1"
  [[ ! -e "$path" ]] || fail "Expected path to be absent: $path"
}

assert_contains() {
  local needle="$1"
  local file="$2"
  grep -Fq "$needle" "$file" || fail "Expected '$needle' in $file"
}

run_init() {
  local project_root="$1"
  local log_file="$2"
  AOC_INIT_SKIP_BUILD=1 bash "$aoc_init_bin" "$project_root" >"$log_file" 2>&1
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-init-pi-first-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
mkdir -p "$HOME" "$XDG_CONFIG_HOME"

# --- Fresh repo flow ---
project_fresh="$tmp_root/fresh"
mkdir -p "$project_fresh/.git"

fresh_log_1="$tmp_root/fresh-init-1.log"
fresh_log_2="$tmp_root/fresh-init-2.log"
run_init "$project_fresh" "$fresh_log_1"

assert_exists "$project_fresh/.aoc/context.md"
assert_exists "$project_fresh/.aoc/memory.md"
assert_exists "$project_fresh/.aoc/stm/current.md"
assert_exists "$project_fresh/.pi/settings.json"
assert_exists "$project_fresh/.pi/prompts/tm-cc.md"
assert_exists "$project_fresh/.pi/skills/aoc-init-ops/SKILL.md"

assert_not_exists "$project_fresh/.aoc/skills"
assert_not_exists "$project_fresh/.codex/skills"
assert_not_exists "$project_fresh/.claude/skills"
assert_not_exists "$project_fresh/.opencode/skills"
assert_not_exists "$project_fresh/.agents/skills"

printf 'custom teach marker\n' > "$project_fresh/.pi/prompts/teach.md"
run_init "$project_fresh" "$fresh_log_2"
assert_contains "custom teach marker" "$project_fresh/.pi/prompts/teach.md"

# --- Existing repo migration flow ---
project_migration="$tmp_root/migration"
mkdir -p "$project_migration/.git"
mkdir -p "$project_migration/.aoc/prompts/pi" "$project_migration/.aoc/skills/custom" "$project_migration/.pi/prompts"

printf 'legacy tmcc prompt\n' > "$project_migration/.aoc/prompts/pi/tmcc.md"
cat > "$project_migration/.aoc/skills/custom/SKILL.md" <<'EOF'
---
name: custom
description: custom migration skill
---
EOF

# Duplicate alias case: both files exist with identical content -> alias should be removed.
printf 'canonical tm-cc\n' > "$project_migration/.pi/prompts/tm-cc.md"
printf 'canonical tm-cc\n' > "$project_migration/.pi/prompts/tmcc.md"

migration_log="$tmp_root/migration-init.log"
run_init "$project_migration" "$migration_log"

assert_exists "$project_migration/.pi/prompts/tm-cc.md"
assert_not_exists "$project_migration/.pi/prompts/tmcc.md"
assert_exists "$project_migration/.pi/skills/custom/SKILL.md"

# Non-destructive migration keeps legacy source content in place.
assert_exists "$project_migration/.aoc/prompts/pi/tmcc.md"
assert_exists "$project_migration/.aoc/skills/custom/SKILL.md"

assert_contains "Removed legacy PI prompt alias duplicate: .pi/prompts/tmcc.md" "$migration_log"
assert_contains "Migrated legacy PI skill: .aoc/skills/custom -> .pi/skills/custom" "$migration_log"

echo "aoc-init PI-first fresh + migration smoke tests passed."
