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

## Launch

From inside a project directory:

```bash
ZELLIJ_PROJECT_ROOT="$PWD" zellij --layout aoc
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
