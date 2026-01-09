#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLUGIN_DIR="$ROOT_DIR/plugins/taskmaster"
OUTPUT="$PLUGIN_DIR/target/wasm32-wasi/release/aoc-taskmaster-plugin.wasm"
DEST="$HOME/.config/zellij/plugins/aoc-taskmaster.wasm"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found; install Rust to build the plugin." >&2
  exit 1
fi

cargo build --manifest-path "$PLUGIN_DIR/Cargo.toml" --target wasm32-wasi --release

if [[ ! -f "$OUTPUT" ]]; then
  echo "Build output not found at $OUTPUT" >&2
  exit 1
fi

mkdir -p "$(dirname "$DEST")"
install -m 0644 "$OUTPUT" "$DEST"

echo "Installed plugin to $DEST"
