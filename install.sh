#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.config/zellij/layouts"
mkdir -p "$HOME/.config/yazi"
mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/aoc"

install_file() {
  local src="$1"
  local dest="$2"
  local mode="$3"

  if [[ -f "$dest" ]] && cmp -s "$src" "$dest"; then
    echo "Up to date: $dest"
    return
  fi

  install -m "$mode" "$src" "$dest"
  echo "Installed: $dest"
}

# Install scripts
for f in "$ROOT_DIR/bin/"*; do
  install_file "$f" "$HOME/.local/bin/$(basename "$f")" 0755
done

# Install zellij layout
install_file "$ROOT_DIR/zellij/layouts/aoc.kdl" "$HOME/.config/zellij/layouts/aoc.kdl" 0644

# Install yazi config + preview
install_file "$ROOT_DIR/yazi/yazi.toml" "$HOME/.config/yazi/yazi.toml" 0644
install_file "$ROOT_DIR/yazi/preview.sh" "$HOME/.config/yazi/preview.sh" 0755

echo "Installed AOC."
echo "Launch from a project dir:"
echo "  aoc-launch"
