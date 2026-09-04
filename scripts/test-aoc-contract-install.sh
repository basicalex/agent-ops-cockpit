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
  HOME="$1" XDG_CONFIG_HOME="$1/.config" AOC_SOURCE_ROOT="$source_root" AOC_INSTALL_JCODE=1 bash "$script" "${@:2}"
}

contract_path() {
  printf '%s/.config/aoc/communication-contract.md' "$1"
}

jcode_path() {
  printf '%s/.jcode/prompt-overlay.md' "$1"
}

sha_path() {
  printf '%s/.config/aoc/communication-contract.jcode.sha' "$1"
}

# Jcode is disabled by default, so a normal install creates only the canonical contract.
disabled_home="$tmp/disabled-home"
mkdir -p "$disabled_home"
assert_eq "$(HOME="$disabled_home" XDG_CONFIG_HOME="$disabled_home/.config" AOC_SOURCE_ROOT="$source_root" bash "$script" --status)" $'canonical missing\njcode disabled'
HOME="$disabled_home" XDG_CONFIG_HOME="$disabled_home/.config" AOC_SOURCE_ROOT="$source_root" bash "$script" >/dev/null
[[ -f "$(contract_path "$disabled_home")" && ! -e "$disabled_home/.jcode" ]]

# Empty homes report missing targets before installation.
missing_home="$tmp/missing-home"
mkdir -p "$missing_home"
assert_eq "$(run_installer "$missing_home" --status)" $'canonical missing\njcode missing'

# Fresh install writes both targets and records ownership of the jcode overlay.
home="$tmp/home"
mkdir -p "$home"
run_installer "$home" >/dev/null
canonical="$(contract_path "$home")"
jcode="$(jcode_path "$home")"
sha_record="$(sha_path "$home")"
[[ -f "$canonical" && -f "$jcode" && -f "$sha_record" ]]
cmp -s "$source_contract" "$canonical"
cmp -s "$source_contract" "$jcode"
assert_eq "$(run_installer "$home" --status)" $'canonical installed\njcode installed'

# A repeated install preserves contents and mtimes.
canonical_before="$tmp/canonical-before"
jcode_before="$tmp/jcode-before"
cp "$canonical" "$canonical_before"
cp "$jcode" "$jcode_before"
canonical_mtime="$(stat -f '%m' "$canonical")"
jcode_mtime="$(stat -f '%m' "$jcode")"
run_installer "$home" >/dev/null
cmp -s "$canonical_before" "$canonical"
cmp -s "$jcode_before" "$jcode"
assert_eq "$(stat -f '%m' "$canonical")" "$canonical_mtime"
assert_eq "$(stat -f '%m' "$jcode")" "$jcode_mtime"

# Foreign jcode overlays are never replaced without a matching ownership record.
foreign_home="$tmp/foreign-home"
foreign_jcode="$(jcode_path "$foreign_home")"
mkdir -p "$(dirname "$foreign_jcode")"
printf 'foreign overlay\n' > "$foreign_jcode"
run_installer "$foreign_home" >"$tmp/foreign-output" 2>"$tmp/foreign-error"
assert_eq "$(<"$foreign_jcode")" 'foreign overlay'
assert_eq "$(run_installer "$foreign_home" --status)" $'canonical installed\njcode user-owned'
[[ "$(<"$tmp/foreign-error")" == *'keeping user-owned'* ]]

# A changed source updates both managed targets and refreshes the ownership hash.
printf '\nUpdated test contract.\n' >> "$source_contract"
run_installer "$home" >/dev/null
cmp -s "$source_contract" "$canonical"
cmp -s "$source_contract" "$jcode"
assert_eq "$(shasum -a 256 "$jcode" | awk '{print $1}')" "$(<"$sha_record")"

# Prime's composed rail keeps exactly one communication-contract section.
prime_home="$tmp/prime-home"
mkdir -p "$prime_home"
HOME="$prime_home" XDG_CONFIG_HOME="$prime_home/.config" AOC_SOURCE_ROOT="$root" \
  bash "$root/bin/aoc-prime-memory-install" >/dev/null
HOME="$prime_home" XDG_CONFIG_HOME="$prime_home/.config" AOC_SOURCE_ROOT="$root" \
  bash "$root/bin/aoc-prime-memory-install" >/dev/null
append_system="$prime_home/.prime/agent/APPEND_SYSTEM.md"
assert_eq "$(grep -c '<!-- aoc communication contract -->' "$append_system")" '1'

bash -n "$script"
bash -n "$root/bin/aoc-omp"
bash -n "$root/bin/aoc-prime-memory-install"
bash -n "$root/install.sh"

printf 'AOC communication contract installer checks passed\n'
