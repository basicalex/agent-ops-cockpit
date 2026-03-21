# Installation Guide

Detailed installation instructions for Agent Ops Cockpit (AOC) on various platforms.

## Recommended user path

For most users:

1. run the bootstrap installer
2. run `aoc-doctor`
3. run `aoc` inside a project
4. press `Alt+C`
5. use **Settings -> Tools** for PI runtime setup and optional integrations

If you want web research, the short path is:

- `Alt+C -> Settings -> Tools -> Agent Browser + Search`
- install Agent Browser
- seed PI browser + web research skills
- enable managed local search
- start/verify search
- run `Verify web research stack`

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
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

This entrypoint will:

1. Resolve the latest release tag (or use your pinned `--ref`)
2. Download the portable `aoc-installer` Rust binary when available
3. Fall back to source archive install if no matching binary exists
4. Install AOC to `~/.local/bin` and user config paths
5. Auto-install the PI agent CLI (required by `aoc-doctor`) unless disabled

### Bootstrap Options

```bash
# pin a specific release tag
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --ref v0.2.0

# non-interactive install for automation
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --yes

# skip the post-install doctor check
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --skip-doctor

# install from a fork or mirror
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo your-org/agent-ops-cockpit
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

# Skip automatic PI agent install (enabled by default)
AOC_INSTALL_PI_AGENT=0 ./install.sh

# Allow install to continue if PI install fails
AOC_INSTALL_PI_REQUIRED=0 ./install.sh

# Skip Rust toolchain bootstrap if cargo is missing
AOC_INSTALL_RUST=0 ./install.sh
```

### Post-install setup contract (`aoc-init`)

After install (or manual `aoc-init`), a PI-first project should include:

- `.pi/settings.json`
- `.pi/prompts/tm-cc.md`
- `.pi/skills/aoc-init-ops/SKILL.md` (plus other seeded skills)
- `.pi/extensions/minimal.ts`
- `.pi/extensions/themeMap.ts`
- `.aoc/context.md`
- `.aoc/rtk.toml`

Quick verification:

```bash
test -f .pi/extensions/minimal.ts
test -f .pi/extensions/themeMap.ts
test -f .pi/prompts/tm-cc.md
test -f .pi/settings.json
```

If anything is missing, run:

```bash
AOC_INIT_SKIP_BUILD=1 aoc-init
```

### Agent CLI Installers and Tool Setup from Alt+C

After install, open `Alt+C` -> **Settings -> Tools**.

Common actions:

- **PI agent installer** — check runtime status and run install/update actions for `pi`
- **Agent Browser + Search** — browser tool sync, web-research skill seeding, managed SearXNG enable/start, end-to-end verification
- **Vercel CLI** — tool + PI skill sync + verify
- **MoreMotion** — `aoc-momo` host/local-source flows

See also: [Control Pane Guide](control-pane.md).

Non-PI agent harnesses are removed from AOC (see [Deprecations and removals](deprecations.md)).

- AOC only runs installer commands (no third-party binaries are bundled).
- You can override installer commands with `AOC_PI_INSTALL_CMD` and `AOC_PI_UPDATE_CMD` (see [Configuration](configuration.md)).
- `pi` uses npm by default: `pnpm add -g @mariozechner/pi-coding-agent`.

## Platform-Specific Instructions

### Ubuntu/Debian

```bash
sudo apt-get update
sudo apt-get install -y zellij fzf ffmpeg chafa poppler-utils ripgrep bat file

# Optional: for .tex file previews
sudo apt-get install -y tectonic
```

**Install Yazi (recommended via cargo):**

```bash
cargo install --locked --force yazi-build
```

**Install `resvg` on Ubuntu/Debian:**

```bash
cargo install --locked resvg
```

**Yazi image backend on Ubuntu/Debian:**
- `ueberzugpp` is often not available in default apt repos
- install `ueberzugpp` from upstream, or use Kitty/kitten as the image backend

**Note:** On Ubuntu, the `bat` binary is named `batcat`. AOC accepts either.

### Fedora

```bash
sudo dnf install -y zellij fzf ffmpeg chafa poppler-utils ripgrep bat file resvg
```

### Arch Linux

```bash
sudo pacman -S zellij fzf ffmpeg chafa poppler ripgrep bat file resvg ueberzugpp
```

### Alpine

```bash
sudo apk add zellij fzf ffmpeg chafa poppler-utils ripgrep bat file resvg ueberzugpp
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
brew install zellij yazi fzf ffmpeg chafa poppler ripgrep bat file-formula resvg
```

## Optional Dependencies

### Managed Local Search (optional)

Managed local search is opt-in and currently uses Docker + Docker Compose to run a local SearXNG instance bound to `127.0.0.1:8888`.

Requirements:

- `docker`
- `docker compose` (or legacy `docker-compose`)

Enable it from:

- `Alt+C -> Settings -> Tools -> Agent Browser + Search`

This flow can:

- generate `.aoc/search.toml`
- generate `.aoc/services/searxng/docker-compose.yml`
- generate `.aoc/services/searxng/settings.yml`
- start/verify the local search service
- sync `.pi/skills/agent-browser/SKILL.md`
- seed `.pi/skills/web-research/SKILL.md`

Enabling managed local search also ensures both PI skills are present so agents can use the intended search-first, browse-second workflow right away.

CLI verification after enabling:

```bash
aoc-search status
aoc-search start --wait
aoc-search health
aoc-search query --limit 3 "rust clap subcommands"
bin/aoc-web-smoke
```

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
- All optional components

## Setup Checklist

- [ ] `zellij --version` is >= 0.43.1
- [ ] `yazi` opens and previews images
- [ ] `aoc-doctor` reports all green

## Troubleshooting

### Managed Local Search Not Starting

Check runtime status first:

```bash
aoc-search status
aoc-search start --wait
aoc-search health
bin/aoc-web-smoke
```

If `aoc-search` is healthy but `bin/aoc-web-smoke` fails, the likely issue is browser integration rather than search itself.

Common fixes:

- confirm `docker` and `docker compose` are installed
- confirm `.aoc/search.toml` exists
- confirm `.aoc/services/searxng/docker-compose.yml` exists
- confirm `.aoc/services/searxng/settings.yml` includes JSON output formats
- inspect container logs:

```bash
docker compose -f .aoc/services/searxng/docker-compose.yml ps
docker compose -f .aoc/services/searxng/docker-compose.yml logs --tail=200 searxng
```

If search is not configured yet, enable it from:

- `Alt+C -> Settings -> Tools -> Agent Browser + Search`

### Web Research Troubleshooting

#### `aoc-search` is healthy but `bin/aoc-web-smoke` fails

That usually means:
- managed search is working
- `agent-browser` setup or browser-side runtime is still failing

Check:

```bash
agent-browser --version
bin/aoc-web-smoke
```

Then retry from:
- `Alt+C -> Settings -> Tools -> Agent Browser + Search`
- `Install/update Agent Browser tool`
- `Verify web research stack`

#### SearXNG logs show errors but search still works

This can happen when some upstream public engines reject or rate-limit requests.
What matters most for AOC is:
- `aoc-search health` passes
- `aoc-search query ...` returns results
- `bin/aoc-web-smoke` passes

The local search service can be healthy even if some upstream engines are noisy.

#### Search returns empty or weak results

Check:

```bash
aoc-search query --limit 5 "your query"
docker compose -f .aoc/services/searxng/docker-compose.yml logs --tail=200 searxng
```

If needed:
- restart managed search
- try a more concrete query
- inspect `.aoc/services/searxng/settings.yml`

#### Generated search files have odd ownership/permissions

If a mounted settings file becomes hard to edit after container activity, reset it locally and restart search:

```bash
chmod 644 .aoc/services/searxng/settings.yml
bin/aoc-search stop
bin/aoc-search start --wait
```

### Missing Previews

Install native Yazi preview dependencies:

```bash
# Ubuntu/Debian
sudo apt-get install file
cargo install --locked resvg

# Verify
aoc-doctor
```

On Ubuntu/Debian, `ueberzugpp` is often not available in the default apt repos.
Install it from upstream, or use Kitty/kitten as the native Yazi image backend.
On Linux generally, `ueberzugpp` is the recommended backend when available.

### Blank Task List

Ensure taskmaster is working:

```bash
tm list
aoc-task list
```

Or install `task-master` npm package if you prefer the CLI version.

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
