# Agent Ops Cockpit (AOC) — Zellij 0.43.1 workspace

A lightweight, terminal-first "agent cockpit" layout for coding sessions:

- **Left:** `yazi` file manager (tree + preview)
- **Center top:** `codex` (always visible)
- **Center bottom:** Taskmaster interactive (fzf-based)
- **Right column:** Calendar/Media widget, Sys details, Project terminal
- **Per-tab contract:** one Zellij **tab = one project root** (panes start there)

## Requirements

- zellij >= 0.43.1
- yazi (recommended via cargo)
- fzf
- chafa
- ffmpeg
- poppler-utils (for pdf -> png)
- librsvg2-bin (for svg -> png)
- optional: ripgrep (`rg`), bat, tectonic (for .tex previews)

This setup is terminal-emulator agnostic (Alacritty, Kitty, GNOME Terminal, etc.)
as long as Zellij is installed and in PATH.

Ubuntu/Debian quick install:

```bash
sudo apt-get update
sudo apt-get install -y zellij fzf ffmpeg chafa poppler-utils librsvg2-bin ripgrep bat
# optional tex:
# sudo apt-get install -y tectonic
```

Yazi install:

```bash
cargo install --locked yazi-fm yazi-cli
```

Other distros:
- Fedora: `sudo dnf install -y zellij fzf ffmpeg chafa poppler-utils librsvg2-tools ripgrep bat`
- Arch: `sudo pacman -S zellij fzf ffmpeg chafa poppler ripgrep bat`
- Alpine: `sudo apk add zellij fzf ffmpeg chafa poppler-utils librsvg`

TeX preview (recommended cross-distro):
```bash
cargo install --locked tectonic --version 0.14.1
```

If Cargo builds fail with a `time` crate type inference error, use a pinned
toolchain or a prebuilt binary:
```bash
rustup toolchain install 1.78.0
cargo +1.78.0 install --locked tectonic --version 0.14.1
```
```bash
cargo install cargo-binstall
cargo binstall tectonic
```

## Install

From this repo:

```bash
./install.sh
```

This will:
- copy scripts to `~/.local/bin`
- install Zellij layout to `~/.config/zellij/layouts/aoc.kdl`
- install Yazi config to `~/.config/yazi/` (preview script included)

Ensure `~/.local/bin` is in PATH.

Verify dependencies:

```bash
aoc-doctor
```

Setup checklist:
- `zellij --version` is >= 0.43.1
- `yazi` opens and previews images
- Widget pane renders an image after setting a media path

## Launch

From inside a project directory:

```bash
aoc-launch
```

## New Tabs
Create a new tab and choose layout:

```bash
aoc
```

Or skip the prompt:

```bash
aoc --aoc --name my-project
aoc --default
```

### Pane expansion (minimal + fast)
Zellij doesn't auto-resize panes on focus by default. Use:
- **Fullscreen current pane:** `Ctrl + f`
- **Cycle panes:** default zellij bindings

## Starred root (re-anchor panes)
Set a new "starred root" and broadcast `cd` to your panes:

```bash
aoc-star /path/to/project
```

This is intentionally explicit and includes a confirmation prompt.

## Widget (Calendar ⇄ Media)
In the top-right widget pane:
- `c` calendar
- `m` media
- `p` set media path (mp4/webm/gif/png/jpg/webp/svg)

Media is rendered as ASCII via chafa (videos animated).

## Notes
- The layout expects `codex` to be in PATH.
- Taskmaster script expects `task-master-ai` in PATH; adjust in `bin/aoc-taskmaster` if needed.

## Customization
- Override commands via env vars: `AOC_CODEX_CMD`, `AOC_TASKMASTER_CMD`, `AOC_FILETREE_CMD`, `AOC_WIDGET_CMD`, `AOC_SYS_CMD`, `AOC_TERMINAL_CMD`.
- To tweak pane sizes, copy the layout:
  `cp ~/.config/zellij/layouts/aoc.kdl ~/.config/zellij/layouts/aoc.local.kdl`
  `aoc-launch` will use `aoc.local` if present.

## Sys Details Options
- `AOC_SYS_INTERVAL=2` controls refresh interval (seconds).
- Set `AOC_SYS_CPU=0`, `AOC_SYS_MEM=0`, or `AOC_SYS_DISK=0` to hide sections.

## Troubleshooting
- Missing previews: install `chafa`, `poppler-utils`, and `librsvg2-bin`.
- Blank task list: ensure `task-master-ai` is in PATH.
- Widget media not rendering: run `aoc-doctor` to confirm `ffmpeg` and `chafa`.
- TeX preview build errors: install `tectonic` via Cargo using `cargo install --locked tectonic --version 0.14.1`.
- If Cargo builds fail with the `time` crate error, use `cargo +1.78.0 install --locked tectonic --version 0.14.1`
  or `cargo binstall tectonic` for a prebuilt release.

## Screenshot
- Store the latest layout screenshot at `docs/screenshot.png` and reference it in docs or release notes.

## Lint
Run shellcheck locally:

```bash
./scripts/lint.sh
```

## Uninstall
Remove installed files:

```bash
aoc-uninstall
```

## Releases
- Follow SemVer and update `CHANGELOG.md` for each release.
- Tag releases as `vX.Y.Z`.
