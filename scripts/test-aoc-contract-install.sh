#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

script="$root/bin/aoc-contract-install"
source_root="$tmp/source"
source_contract="$source_root/config/communication-contract/CONTRACT.md"
mkdir -p "$(dirname "$source_contract")"
cp "$root/config/communication-contract/CONTRACT.md" "$source_contract"

assert_eq() {
  local actual="$1"
  local expected="$2"
  if [[ "$actual" != "$expected" ]]; then
    printf 'expected:\n%s\nactual:\n%s\n' "$expected" "$actual" >&2
    exit 1
  fi
}

run_installer() {
  HOME="$1" XDG_CONFIG_HOME="$1/.config" AOC_SOURCE_ROOT="$source_root" bash "$script" "${@:2}"
}

contract_path() {
  printf '%s/.config/aoc/communication-contract.md' "$1"
}

# Empty homes report the missing target before installation.
home="$tmp/home"
mkdir -p "$home"
assert_eq "$(run_installer "$home" --status)" 'canonical missing'

# A fresh install writes only the canonical contract.
run_installer "$home" >/dev/null
canonical="$(contract_path "$home")"
[[ -f "$canonical" ]]
cmp -s "$source_contract" "$canonical"
assert_eq "$(run_installer "$home" --status)" 'canonical installed'
[[ "$(find "$home" -type f | wc -l | tr -d ' ')" == "1" ]]

# A repeated install preserves contents and mtime.
canonical_before="$tmp/canonical-before"
cp "$canonical" "$canonical_before"
canonical_mtime="$(stat -f '%m' "$canonical" 2>/dev/null || stat -c '%Y' "$canonical")"
run_installer "$home" >/dev/null
cmp -s "$canonical_before" "$canonical"
assert_eq "$(stat -f '%m' "$canonical" 2>/dev/null || stat -c '%Y' "$canonical")" "$canonical_mtime"

# A changed source updates the target and keeps a backup of the old copy.
printf '\nUpdated test contract.\n' >> "$source_contract"
run_installer "$home" >/dev/null
cmp -s "$source_contract" "$canonical"
compgen -G "$canonical.bak.*" >/dev/null

bash -n "$script"
bash -n "$root/bin/aoc-omp"
bash -n "$root/install.sh"

printf 'AOC communication contract installer checks passed\n'
