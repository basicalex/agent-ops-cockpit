#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
manifest_path="$repo_root/vendor/zjstatus-aoc/Cargo.toml"
out_wasm="$repo_root/zellij/plugins/zjstatus-aoc.wasm"
target_triple="${AOC_ZELLIJ_PLUGIN_TARGET:-wasm32-wasip1}"
keep_target_dir=0

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  target_dir="$CARGO_TARGET_DIR"
else
  target_dir="$(mktemp -d "${TMPDIR:-/tmp}/aoc-zjstatus-build.XXXXXX")"
  keep_target_dir=1
fi

cleanup() {
  if [[ "$keep_target_dir" == "1" ]]; then
    rm -rf "$target_dir"
  fi
}
trap cleanup EXIT

if command -v rustup >/dev/null 2>&1; then
  rustup target add "$target_triple" >/dev/null 2>&1 || true
fi

CARGO_TARGET_DIR="$target_dir" cargo build \
  --manifest-path "$manifest_path" \
  --release \
  --target "$target_triple" \
  --bin zjstatus

built_wasm="$target_dir/$target_triple/release/zjstatus.wasm"
[[ -f "$built_wasm" ]]
cp "$built_wasm" "$out_wasm"
chmod 0755 "$out_wasm"

echo "Updated $out_wasm"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$out_wasm"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$out_wasm"
fi
