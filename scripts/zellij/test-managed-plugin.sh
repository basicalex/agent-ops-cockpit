#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
plugin_bin="$repo_root/bin/aoc-zellij-plugin"

authoritative_wasm="$repo_root/zellij/plugins/zjstatus-aoc.wasm"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_exists() {
  [[ -e "$1" ]] || fail "Expected path to exist: $1"
}

assert_same_file() {
  cmp -s "$1" "$2" || fail "Expected files to match: $1 == $2"
}

orig_home="$HOME"
orig_rustup_home="${RUSTUP_HOME:-$orig_home/.rustup}"
orig_cargo_home="${CARGO_HOME:-$orig_home/.cargo}"

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-zellij-plugin-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
export XDG_STATE_HOME="$tmp_root/state"
export ZELLIJ_CONFIG_DIR="$tmp_root/zellij-config"
export RUSTUP_HOME="$orig_rustup_home"
export CARGO_HOME="$orig_cargo_home"
export PATH="$CARGO_HOME/bin:$PATH"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$ZELLIJ_CONFIG_DIR"

# Bundled artifact install path
bash "$plugin_bin" install >/dev/null
installed_wasm="$ZELLIJ_CONFIG_DIR/plugins/zjstatus-aoc.wasm"
cache_wasm="$XDG_CONFIG_HOME/aoc/zellij/plugins/zjstatus-aoc.wasm"
assert_exists "$installed_wasm"
assert_exists "$cache_wasm"
assert_same_file "$authoritative_wasm" "$installed_wasm"
assert_same_file "$authoritative_wasm" "$cache_wasm"

# Build-from-source path
rm -f "$installed_wasm"
rm -f "$cache_wasm"
AOC_ZELLIJ_PLUGIN_BUILD=1 CARGO_TARGET_DIR="$tmp_root/target" bash "$plugin_bin" install >/dev/null
assert_exists "$installed_wasm"
assert_exists "$cache_wasm"
assert_same_file "$authoritative_wasm" "$installed_wasm"
assert_same_file "$authoritative_wasm" "$cache_wasm"

echo "managed AOC Zellij plugin install/build smoke tests passed."
