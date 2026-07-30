#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

home="$tmp_dir/home"
fake_bin="$tmp_dir/bin"
mkdir -p "$home" "$fake_bin"

link_tool() {
  local name="$1"
  local path
  path="$(command -v "$name")"
  ln -s "$path" "$fake_bin/$name"
}

for tool in bash cat cp date dirname env mkdir mktemp python3 rm; do
  link_tool "$tool"
done

output="$tmp_dir/output.txt"
if ! HOME="$home" XDG_CONFIG_HOME="$home/.config" PATH="$fake_bin" "$root/bin/aoc-herdr-install" >"$output" 2>&1; then
  echo "ERROR: installer failed without herdr" >&2
  cat "$output" >&2
  exit 1
fi

if [[ ! -f "$home/.config/herdr/config.toml" ]]; then
  echo "ERROR: missing installed Herdr config" >&2
  exit 1
fi

if [[ ! -f "$home/.omp/agent/extensions/aoc-profile.ts" ]]; then
  echo "ERROR: missing synced AOC OMP extension" >&2
  exit 1
fi

if [[ ! -d "$home/.omp/agent/skills/aoc-understand" ]]; then
  echo "ERROR: missing synced AOC OMP skill" >&2
  exit 1
fi

if [[ "$( <"$output" )" != *"herdr integration install omp"* ]]; then
  echo "ERROR: missing retry command for Herdr OMP integration" >&2
  cat "$output" >&2
  exit 1
fi

printf 'aoc-herdr-install missing-herdr fixture passed\n'
