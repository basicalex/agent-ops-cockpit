#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.config/zellij/layouts"
mkdir -p "$HOME/.config/yazi"
mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/aoc"

# Install scripts
for f in "$ROOT_DIR/bin/"*; do
  install -m 0755 "$f" "$HOME/.local/bin/$(basename "$f")"
done

# Install zellij layout
install -m 0644 "$ROOT_DIR/zellij/layouts/aoc.kdl" "$HOME/.config/zellij/layouts/aoc.kdl"

# Install yazi config + preview
install -m 0644 "$ROOT_DIR/yazi/yazi.toml" "$HOME/.config/yazi/yazi.toml"
install -m 0755 "$ROOT_DIR/yazi/preview.sh" "$HOME/.config/yazi/preview.sh"

echo "Installed AOC."
echo "Launch from a project dir:"
echo "  ZELLIJ_PROJECT_ROOT=\"$PWD\" zellij --layout aoc"
