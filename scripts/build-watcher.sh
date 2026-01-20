#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WATCHER_DIR="$ROOT_DIR/plugins/aoc-watcher"

echo "Building aoc-watcher..."
cd "$WATCHER_DIR"
cargo build --release

echo "Installing aoc-watcher to bin/..."
cp target/release/aoc-watcher "$ROOT_DIR/bin/aoc-watcher"

echo "Done."
