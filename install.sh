#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"

# Ensure dirs exist
mkdir -p "$BIN_DIR"
mkdir -p "$HOME/.config/zellij/layouts"
mkdir -p "$HOME/.config/zellij"
mkdir -p "$HOME/.config/zellij/plugins"
mkdir -p "$HOME/.config/yazi"
mkdir -p "$HOME/.config/yazi/plugins"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/btop"
mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/aoc"

log() { echo ">> $1"; }

# 1. Install Scripts
log "Installing scripts..."
for f in "$ROOT_DIR/bin/"*; do
  filename=$(basename "$f")
  # Skip micro if it's there (it shouldn't be, but just in case)
  [[ "$filename" == "micro" ]] && continue
  
  install -m 0755 "$f" "$BIN_DIR/$filename"
done

# Ensure codex shim
if [[ -d "$HOME/bin" && -w "$HOME/bin" ]]; then
  install -m 0755 "$ROOT_DIR/bin/codex" "$HOME/bin/codex"
fi

# 2. Rust Build & Install
log "Building Rust components..."
if command -v cargo >/dev/null 2>&1; then
  # Build aoc-cli
  log "Building aoc-cli..."
  cargo install --path "$ROOT_DIR/crates/aoc-cli" --root "$HOME/.local" --force --quiet || {
    # Fallback for older cargos that don't support --root in the same way or if it fails
    # Try direct build
    log "Cargo install failed, trying build --release..."
    (cd "$ROOT_DIR/crates" && cargo build --release -p aoc-cli)
    cp "$ROOT_DIR/crates/target/release/aoc-cli" "$BIN_DIR/aoc-cli"
  }

  # Build aoc-taskmaster (native TUI)
  log "Building aoc-taskmaster..."
  if cargo build --release -p aoc-taskmaster --manifest-path "$ROOT_DIR/crates/Cargo.toml"; then
    if [[ -f "$ROOT_DIR/crates/target/release/aoc-taskmaster" ]]; then
      install -m 0755 "$ROOT_DIR/crates/target/release/aoc-taskmaster" "$BIN_DIR/aoc-taskmaster-native"
    fi
  else
    log "WARNING: Failed to build aoc-taskmaster."
  fi

  # Build aoc-control (native TUI)
  log "Building aoc-control..."
  if cargo build --release -p aoc-control --manifest-path "$ROOT_DIR/crates/Cargo.toml"; then
    if [[ -f "$ROOT_DIR/crates/target/release/aoc-control" ]]; then
      install -m 0755 "$ROOT_DIR/crates/target/release/aoc-control" "$BIN_DIR/aoc-control-native"
    fi
  else
    log "WARNING: Failed to build aoc-control."
  fi
else
  log "WARNING: cargo not found. Skipping Rust builds. You must install aoc-cli manually."
fi

# 3. Dependencies (Micro & ZJStatus)
log "Checking dependencies..."

# Micro
if ! command -v micro >/dev/null 2>&1; then
  if [[ ! -f "$BIN_DIR/micro" ]]; then
    log "Downloading micro..."
    curl https://getmic.ro | bash
    mv micro "$BIN_DIR/micro"
  fi
else
  log "Micro found."
fi

# ZJStatus
ZJSTATUS_PATH="$HOME/.config/zellij/plugins/zjstatus.wasm"
if [[ ! -f "$ZJSTATUS_PATH" ]]; then
  log "Downloading zjstatus.wasm..."
  curl -L -o "$ZJSTATUS_PATH" https://github.com/dj95/zjstatus/releases/latest/download/zjstatus.wasm
fi

# 4. Generate & Install Configs
log "Generating configurations..."

# Zellij Layout
# Replace placeholders in template
PROJECTS_BASE="$HOME/dev"
[[ ! -d "$PROJECTS_BASE" ]] && PROJECTS_BASE="$HOME"

sed \
  -e "s|{{HOME}}|$HOME|g" \
  -e "s|{{PROJECTS_BASE}}|$PROJECTS_BASE|g" \
  "$ROOT_DIR/zellij/layouts/aoc.kdl.template" > "$HOME/.config/zellij/layouts/aoc.kdl"

sed \
  -e "s|{{HOME}}|$HOME|g" \
  "$ROOT_DIR/zellij/layouts/minimal.kdl.template" > "$HOME/.config/zellij/layouts/minimal.kdl"

log "Generated layouts in $HOME/.config/zellij/layouts/"

# Copy other configs
install -m 0644 "$ROOT_DIR/zellij/aoc.config.kdl" "$HOME/.config/zellij/aoc.config.kdl"
install -m 0644 "$ROOT_DIR/yazi/yazi.toml" "$HOME/.config/yazi/yazi.toml"
install -m 0755 "$ROOT_DIR/yazi/preview.sh" "$HOME/.config/yazi/preview.sh"
install -m 0644 "$ROOT_DIR/yazi/keymap.toml" "$HOME/.config/yazi/keymap.toml"
install -m 0644 "$ROOT_DIR/yazi/theme.toml" "$HOME/.config/yazi/theme.toml"
install -m 0644 "$ROOT_DIR/yazi/init.lua" "$HOME/.config/yazi/init.lua"
install -m 0644 "$ROOT_DIR/config/codex-tmux.conf" "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/codex-tmux.conf"
install -m 0644 "$ROOT_DIR/config/btop.conf" "${XDG_CONFIG_HOME:-$HOME/.config}/aoc/btop/btop.conf"

# Yazi Plugins
if [[ -d "$ROOT_DIR/yazi/plugins" ]]; then
  shopt -s nullglob
  for d in "$ROOT_DIR/yazi/plugins/"*.yazi; do
    [[ -d "$d" ]] || continue
    dest="$HOME/.config/yazi/plugins/$(basename "$d")"
    mkdir -p "$dest"
    for f in "$d"/*.lua; do
      [[ -f "$f" ]] || continue
      install -m 0644 "$f" "$dest/$(basename "$f")"
    done
  done
  shopt -u nullglob
fi

log "AOC Installed Successfully!"
log "Run 'aoc' to start."
