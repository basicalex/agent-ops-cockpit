#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
manifest_path="$repo_root/vendor/zjstatus-aoc/Cargo.toml"
tracked_wasm="$repo_root/zellij/plugins/zjstatus-aoc.wasm"
target_triple="${AOC_ZELLIJ_PLUGIN_TARGET:-wasm32-wasip1}"
target_dir="$(mktemp -d "${TMPDIR:-/tmp}/aoc-zjstatus-verify.XXXXXX")"
trap 'rm -rf "$target_dir"' EXIT

if command -v rustup >/dev/null 2>&1; then
  rustup target add "$target_triple" >/dev/null 2>&1 || true
fi

CARGO_TARGET_DIR="$target_dir" cargo build \
  --manifest-path "$manifest_path" \
  --release \
  --target "$target_triple" \
  --bin zjstatus >/dev/null

built_wasm="$target_dir/$target_triple/release/zjstatus.wasm"
[[ -f "$built_wasm" ]]
[[ -f "$tracked_wasm" ]]

if ! cmp -s "$built_wasm" "$tracked_wasm"; then
  echo "Managed zjstatus wasm is out of sync with vendor/zjstatus-aoc." >&2
  echo "Run: bash scripts/zellij/rebuild-managed-plugin.sh" >&2
  exit 1
fi

echo "Managed zjstatus wasm matches vendored source."
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$tracked_wasm"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$tracked_wasm"
fi
