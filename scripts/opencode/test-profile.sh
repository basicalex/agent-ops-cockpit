#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
profile_bin="$repo_root/bin/aoc-opencode-profile"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local message="$3"
  if [[ "$expected" != "$actual" ]]; then
    fail "$message (expected '$expected', got '$actual')"
  fi
}

assert_exists() {
  local path="$1"
  local message="$2"
  [[ -e "$path" ]] || fail "$message (missing: $path)"
}

assert_file_contains() {
  local path="$1"
  local token="$2"
  local message="$3"
  if [[ ! -f "$path" ]]; then
    fail "$message (missing file: $path)"
  fi
  if ! grep -Fq "$token" "$path"; then
    fail "$message (missing token '$token' in $path)"
  fi
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-opencode-profile-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
export XDG_STATE_HOME="$tmp_root/state"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME"

export AOC_OPENCODE_PROFILE_YES=1

main_path="$XDG_CONFIG_HOME/opencode"
sandbox_path="$XDG_CONFIG_HOME/aoc/opencode/profiles/sandbox"

assert_eq "$main_path" "$("$profile_bin" resolve main)" "resolve main profile path"
assert_eq "$sandbox_path" "$("$profile_bin" resolve sandbox)" "resolve sandbox profile path"

assert_eq "$main_path" "$("$profile_bin" resolve)" "resolve active path defaults to main"
export OPENCODE_CONFIG_DIR="$tmp_root/external-opencode"
assert_eq "$tmp_root/external-opencode" "$("$profile_bin" resolve)" "resolve active path honors OPENCODE_CONFIG_DIR"
unset OPENCODE_CONFIG_DIR

assert_eq "$sandbox_path" "$("$profile_bin" init sandbox)" "init returns sandbox path"
assert_exists "$sandbox_path/opencode.json" "init creates opencode.json"

printf '{"keep":true}\n' > "$sandbox_path/opencode.json"
"$profile_bin" init sandbox >/dev/null
assert_file_contains "$sandbox_path/opencode.json" '"keep":true' "init is non-destructive for existing opencode.json"

custom_path="$("$profile_bin" resolve qa-profile)"
mkdir -p "$custom_path"
printf 'keep-me\n' > "$custom_path/preserved.txt"
"$profile_bin" init qa-profile >/dev/null
assert_exists "$custom_path/preserved.txt" "init preserves existing custom profile files"

"$profile_bin" init main >/dev/null
printf '{"before":true}\n' > "$main_path/opencode.json"
printf '{"after":true}\n' > "$sandbox_path/opencode.json"

"$profile_bin" promote sandbox main --yes >/dev/null
assert_file_contains "$main_path/opencode.json" '"after":true' "promote copies sandbox into main"

backup_root="$XDG_STATE_HOME/aoc/opencode-profile-backups/main"
assert_exists "$backup_root" "promote creates backup root"

first_snapshot=""
if ! read -r first_snapshot < <("$profile_bin" list-backups main); then
  fail "list-backups should return at least one snapshot"
fi

printf '{"mutated":true}\n' > "$main_path/opencode.json"
"$profile_bin" rollback main "$first_snapshot" --yes >/dev/null
assert_file_contains "$main_path/opencode.json" '"before":true' "rollback restores selected snapshot"

export AOC_OPENCODE_PROFILE_YES=0
if "$profile_bin" promote sandbox main </dev/null >/dev/null 2>&1; then
  fail "promote main without --yes should fail in non-interactive mode"
fi
assert_file_contains "$main_path/opencode.json" '"before":true' "failed promote does not mutate main"

echo "All aoc-opencode-profile tests passed."
