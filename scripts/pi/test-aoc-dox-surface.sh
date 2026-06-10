#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
init="$repo_root/bin/aoc-init"
herdr_install="$repo_root/bin/aoc-herdr-install"
dox_extension="$repo_root/.omp/extensions/aoc-dox.ts"
dox_skill="$repo_root/.omp/skills/aoc-dox-cartography/SKILL.md"
dox_docs="$repo_root/docs/dox-cartography.md"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq "$needle" "$file" || fail "Expected $file to contain: $needle"
}

[[ -f "$init" ]] || fail "Missing aoc-init: $init"
[[ -f "$herdr_install" ]] || fail "Missing aoc-herdr-install: $herdr_install"
[[ -f "$dox_extension" ]] || fail "Missing OMP Dox extension: $dox_extension"
[[ -f "$dox_skill" ]] || fail "Missing OMP Dox skill: $dox_skill"
[[ -f "$dox_docs" ]] || fail "Missing Dox docs: $dox_docs"

assert_contains "$dox_extension" 'commands.registerCommand?.("dox"'
assert_contains "$dox_extension" 'Launch dox-scout in parallel'
assert_contains "$dox_extension" 'Never run aoc dox apply --yes from this command'
assert_contains "$dox_skill" 'Launch `dox-scout` in parallel'
assert_contains "$dox_docs" '/dox full'

assert_contains "$init" 'aoc-dox.ts'
assert_contains "$init" 'dox-scout.md'
assert_contains "$init" 'dox-mapper.md'
assert_contains "$init" 'dox-critic.md'
assert_contains "$init" 'dox-writer.md'
assert_contains "$init" 'aoc-dox-cartography'
assert_contains "$init" 'setup_omp_skills'

assert_contains "$herdr_install" 'aoc-dox.ts'
assert_contains "$herdr_install" 'dox-scout.md'
assert_contains "$herdr_install" 'dox-mapper.md'
assert_contains "$herdr_install" 'dox-critic.md'
assert_contains "$herdr_install" 'dox-writer.md'
assert_contains "$herdr_install" 'aoc-dox-cartography'

echo "OK: AOC DOX slash command and seed surface are present"
