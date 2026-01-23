#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.config/zellij/layouts"
mkdir -p "$HOME/.config/zellij"
mkdir -p "$HOME/.config/zellij/plugins"
mkdir -p "$HOME/.config/yazi"
mkdir -p "$HOME/.config/yazi/plugins"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/btop"
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

# Ensure codex shim is earlier in PATH when ~/bin is prioritized.
if [[ -d "$HOME/bin" && -w "$HOME/bin" ]]; then
  install_file "$ROOT_DIR/bin/codex" "$HOME/bin/codex" 0755
fi

# Install zellij layout
install_file "$ROOT_DIR/zellij/layouts/aoc.kdl" "$HOME/.config/zellij/layouts/aoc.kdl" 0644
install_file "$ROOT_DIR/zellij/aoc.config.kdl" "$HOME/.config/zellij/aoc.config.kdl" 0644

# Install yazi config + preview
install_file "$ROOT_DIR/yazi/yazi.toml" "$HOME/.config/yazi/yazi.toml" 0644
install_file "$ROOT_DIR/yazi/preview.sh" "$HOME/.config/yazi/preview.sh" 0755
install_file "$ROOT_DIR/yazi/keymap.toml" "$HOME/.config/yazi/keymap.toml" 0644
install_file "$ROOT_DIR/yazi/theme.toml" "$HOME/.config/yazi/theme.toml" 0644
install_file "$ROOT_DIR/yazi/init.lua" "$HOME/.config/yazi/init.lua" 0644

# Install yazi plugins
if [[ -d "$ROOT_DIR/yazi/plugins" ]]; then
  shopt -s nullglob
  for d in "$ROOT_DIR/yazi/plugins/"*.yazi; do
    [[ -d "$d" ]] || continue
    dest="$HOME/.config/yazi/plugins/$(basename "$d")"
    mkdir -p "$dest"
    for f in "$d"/*.lua; do
      [[ -f "$f" ]] || continue
      install_file "$f" "$dest/$(basename "$f")" 0644
    done
  done
  shopt -u nullglob
fi

# Install Codex tmux config
install_file "$ROOT_DIR/config/codex-tmux.conf" "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/codex-tmux.conf" 0644
# Install btop config (small-pane friendly)
install_file "$ROOT_DIR/config/btop.conf" "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/btop/btop.conf" 0644

# Install Taskmaster plugin if built
plugin_wasm=""
if [[ -f "$ROOT_DIR/plugins/taskmaster/target/wasm32-wasi/release/aoc-taskmaster-plugin.wasm" ]]; then
  plugin_wasm="$ROOT_DIR/plugins/taskmaster/target/wasm32-wasi/release/aoc-taskmaster-plugin.wasm"
elif [[ -f "$ROOT_DIR/plugins/taskmaster/target/wasm32-wasip1/release/aoc-taskmaster-plugin.wasm" ]]; then
  plugin_wasm="$ROOT_DIR/plugins/taskmaster/target/wasm32-wasip1/release/aoc-taskmaster-plugin.wasm"
fi

if [[ -n "$plugin_wasm" ]]; then
  install_file "$plugin_wasm" "$HOME/.config/zellij/plugins/aoc-taskmaster.wasm" 0644
else
  echo "Taskmaster plugin not built. Run: ./scripts/build-taskmaster-plugin.sh"
fi

echo "Installed AOC."
echo "Launch from a project dir:"
echo "  aoc-launch"
