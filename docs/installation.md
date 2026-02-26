# Installation Guide

Detailed installation instructions for Agent Ops Cockpit (AOC) on various platforms.

## Table of Contents

- [Quick Install](#quick-install)
- [Platform-Specific Instructions](#platform-specific-instructions)
  - [Ubuntu/Debian](#ubuntudebian)
  - [Fedora](#fedora)
  - [Arch Linux](#arch-linux)
  - [Alpine](#alpine)
- [WSL (Windows)](#wsl-windows)
- [macOS](#macos)
- [Optional Dependencies](#optional-dependencies)
  - [TeX Preview Support](#tex-preview-support)
- [Verification](#verification)
- [Troubleshooting](#troubleshooting)

## Quick Install

Run the online bootstrap installer:

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo basicalex/agent-ops-cockpit
```

This entrypoint will:

1. Resolve the latest release tag (or use your pinned `--ref`)
2. Download the portable `aoc-installer` Rust binary when available
3. Fall back to source archive install if no matching binary exists
4. Install AOC to `~/.local/bin` and user config paths

### Bootstrap Options

```bash
# pin a specific release tag
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo basicalex/agent-ops-cockpit --ref v0.2.0

# non-interactive install for automation
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo basicalex/agent-ops-cockpit --yes

# skip the post-install doctor check
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo basicalex/agent-ops-cockpit --skip-doctor
```

If you already cloned the repo, you can still run `./install.sh` directly.

By default, `install.sh` now runs `aoc-init` for your current working directory (`$PWD`) after installation so AOC files (including `.aoc/rtk.toml`) are seeded immediately.

RTK routing defaults to `mode = "on"` for newly initialized projects. If a project already has `.aoc/rtk.toml` with `mode = "off"`, `aoc-init` preserves that explicit disable and logs it.

Install-time overrides:

```bash
# Skip automatic project initialization
AOC_INSTALL_AUTO_INIT=0 ./install.sh

# Initialize a specific project path after install
AOC_INIT_TARGET=~/dev/my-project ./install.sh
```

### Agent CLI Installers from Alt+C

After install, open `Alt+C` -> **Settings -> Agent installers** to check runtime status and run install/update actions for `pi`.

Non-PI agent harnesses are removed from AOC.

- AOC only runs installer commands (no third-party binaries are bundled).
- You can override installer commands with `AOC_PI_INSTALL_CMD` and `AOC_PI_UPDATE_CMD` (see [Configuration](configuration.md)).
- `pi` uses npm by default: `pnpm add -g @mariozechner/pi-coding-agent`.

## Platform-Specific Instructions

### Ubuntu/Debian

```bash
sudo apt-get update
sudo apt-get install -y zellij fzf ffmpeg chafa poppler-utils librsvg2-bin ripgrep bat

# Optional: for .tex file previews
sudo apt-get install -y tectonic
```

**Install Yazi (recommended via cargo):**

```bash
cargo install --locked --force yazi-build
```

**Note:** On Ubuntu, the `bat` binary is named `batcat`. AOC accepts either.

### Fedora

```bash
sudo dnf install -y zellij fzf ffmpeg chafa poppler-utils librsvg2-tools ripgrep bat
```

### Arch Linux

```bash
sudo pacman -S zellij fzf ffmpeg chafa poppler ripgrep bat
```

### Alpine

```bash
sudo apk add zellij fzf ffmpeg chafa poppler-utils librsvg
```

## WSL (Windows)

**Requirements:**
- WSL2 is required (WSL1 is not supported)
- Use the Linux package lists above inside your distro

**Limitations:**
- Fullscreen helpers (`wmctrl`/`xdotool`) do not work in WSL/WSLg
- Set `AOC_FULLSCREEN=0` or use your terminal's fullscreen/maximize

## macOS

macOS is fully supported. Install dependencies via Homebrew:

```bash
brew install zellij yazi fzf ffmpeg chafa poppler librsvg ripgrep bat
```

## Optional Dependencies

### TeX Preview Support

For previewing `.tex` files, install Tectonic:

**Cross-distro (recommended):**

```bash
cargo install --locked tectonic --version 0.14.1
```

**If Cargo builds fail with a `time` crate type inference error:**

```bash
rustup toolchain install 1.78.0
cargo +1.78.0 install --locked tectonic --version 0.14.1
```

**Alternative (prebuilt binary):**

```bash
cargo install cargo-binstall
cargo binstall tectonic
```

**Linux source-build dependencies (Ubuntu/Omakub):**

```bash
sudo apt-get update
sudo apt-get install -y pkg-config cmake g++ libharfbuzz-dev libfreetype6-dev libgraphite2-dev
```

## Verification

After installation, verify all dependencies are correctly installed:

```bash
aoc-doctor
```

This will check for:
- Zellij version (>= 0.43.1)
- Yazi functionality
- Widget rendering capabilities
- All optional components

## Setup Checklist

- [ ] `zellij --version` is >= 0.43.1
- [ ] `yazi` opens and previews images
- [ ] Widget pane renders an image after setting a media path
- [ ] `aoc-doctor` reports all green

## Troubleshooting

### Missing Previews

Install required media tools:

```bash
# Ubuntu/Debian
sudo apt-get install chafa poppler-utils librsvg2-bin

# Verify
aoc-doctor
```

### Blank Task List

Ensure taskmaster is working:

```bash
tm list
aoc-task list
```

Or install `task-master` npm package if you prefer the CLI version.

### Widget Media Not Rendering

Run diagnostics:

```bash
aoc-doctor
```

Confirm `ffmpeg` and `chafa` are installed.

### TeX Preview Build Errors

If you see errors building TeX previews:

1. Install `tectonic` via Cargo (see [TeX Preview Support](#tex-preview-support))
2. If the `time` crate error occurs, use the pinned toolchain method
3. Or use `cargo binstall tectonic` for a prebuilt release

### RLM Skill Not Working

Build the Rust CLI:

```bash
cargo build --release -p aoc-cli
```

Ensure `aoc-cli` is in your PATH.

### Bat Binary Name

On Ubuntu, the binary is named `batcat` instead of `bat`. AOC's `aoc-doctor` accepts either name.

---

**Next Steps:**
- Return to [Main README](../README.md)
- Read about [Configuration](configuration.md)
- Learn about [Custom Layouts](layouts.md)
