#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
aoc_init_bin="$repo_root/bin/aoc-init"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-init-omo-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
mkdir -p "$HOME" "$XDG_CONFIG_HOME"

project_root="$tmp_root/project"
mkdir -p "$project_root/.git"

AOC_INIT_SKIP_BUILD=1 bash "$aoc_init_bin" "$project_root" >/dev/null

policy_file="$project_root/.opencode/oh-my-opencode.jsonc"
if [[ -f "$policy_file" ]]; then
  fail "aoc-init should not seed project OmO policy by default"
fi

AOC_INIT_SKIP_BUILD=1 bash "$aoc_init_bin" "$project_root" >/dev/null

if [[ -f "$policy_file" ]]; then
  fail "aoc-init should remain non-seeding on repeated runs"
fi

echo "aoc-init OmO non-seeding behavior verified."
