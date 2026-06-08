#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local needle="$1"
  local file="$2"
  grep -Fq -- "$needle" "$file" || fail "Expected '$needle' in $file"
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-voxtype-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
mkdir -p "$HOME" "$XDG_CONFIG_HOME"

"$repo_root/bin/aoc-voxtype-setup" --quiet
filter="$HOME/.local/bin/voxtype-aoc-lexicon-filter"
system_lexicon="$XDG_CONFIG_HOME/aoc/voxtype-lexicon.md"
config="$XDG_CONFIG_HOME/voxtype/config.toml"

[[ -x "$filter" ]] || fail "Expected executable filter at $filter"
[[ -f "$system_lexicon" ]] || fail "Expected system lexicon at $system_lexicon"
[[ -f "$config" ]] || fail "Expected VoxType config at $config"
assert_contains 'command = "'"$filter"'"' "$config"
assert_contains '### VoxType' "$system_lexicon"

output="$(printf '%s' 'a o c uses voice type and task master' | "$filter" --no-active-project)"
[[ "$output" == 'AOC uses VoxType and Taskmaster' ]] || fail "Unexpected normalization: $output"

cp "$config" "$tmp_root/config.once"
"$repo_root/bin/aoc-voxtype-setup" --quiet
cmp -s "$config" "$tmp_root/config.once" || fail "Expected setup to be idempotent for config"

echo "PASS: aoc-voxtype-setup"
