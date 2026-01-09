#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLUGIN_DIR="$ROOT_DIR/plugins/taskmaster"
TARGET=""
if rustc --print target-list | grep -q '^wasm32-wasi$'; then
  TARGET="wasm32-wasi"
elif rustc --print target-list | grep -q '^wasm32-wasip1$'; then
  TARGET="wasm32-wasip1"
fi

if [[ -z "$TARGET" ]]; then
  echo "No wasm32-wasi target found. Install one with:" >&2
  echo "  rustup target add wasm32-wasip1" >&2
  echo "  # or: rustup target add wasm32-wasi" >&2
  exit 1
fi

OUTPUT="$PLUGIN_DIR/target/$TARGET/release/aoc-taskmaster-plugin.wasm"
DEST="$HOME/.config/zellij/plugins/aoc-taskmaster.wasm"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found; install Rust to build the plugin." >&2
  exit 1
fi

cargo build --manifest-path "$PLUGIN_DIR/Cargo.toml" --target "$TARGET" --release

if [[ ! -f "$OUTPUT" ]]; then
  echo "Build output not found at $OUTPUT" >&2
  exit 1
fi

mkdir -p "$(dirname "$DEST")"
install -m 0644 "$OUTPUT" "$DEST"

echo "Installed plugin to $DEST"
