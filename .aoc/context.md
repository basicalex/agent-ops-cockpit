# Agent Ops Cockpit (AOC) Global Context

## Core Philosophy
This machine uses the **Agent Ops Cockpit (AOC)** system. All agents (Gemini, Claude, OpenCode) running here share a unified set of tools for **Memory** and **Task Management**.

## 1. Project Structure
```
/home/ceii/dev/agent-ops-cockpit
├── AGENTS.md
├── AOC.md
├── bin
│   ├── aoc
│   ├── aoc-agent
│   ├── aoc-agent-run
│   ├── aoc-agent-wrap
│   ├── aoc-align
│   ├── aoc-cc
│   ├── aoc-cleanup
│   ├── aoc-clock
│   ├── aoc-clock-geo
│   ├── aoc-clock-set
│   ├── aoc-codex
│   ├── aoc-codex-tab
│   ├── aoc-doctor
│   ├── aoc-gemini
│   ├── aoc-hub
│   ├── aoc-init
│   ├── aoc-launch
│   ├── aoc-layout
│   ├── aoc-mem
│   ├── aoc-mission-control
│   ├── aoc-mission-control-toggle
│   ├── aoc-new-tab
│   ├── aoc-oc
│   ├── aoc-pane-rename
│   ├── aoc-preview
│   ├── aoc-preview-set
│   ├── aoc-preview-toggle
│   ├── aoc-rlm
│   ├── aoc-sys
│   ├── aoc-taskmaster
│   ├── aoc-tasks
│   ├── aoc-test
│   ├── aoc-uninstall
│   ├── aoc-widget
│   ├── aoc-widget-set
│   ├── claude
│   ├── codex
│   ├── gemini
│   ├── opencode
│   ├── rlm
│   └── tm-editor
├── CHANGELOG.md
├── ClockTemp
│   ├── assets
│   ├── LICENSE
│   ├── README.md
│   ├── requirements.txt
│   └── script
├── cmd
│   ├── aoc-agent-wrap-go
│   ├── aoc-hub
│   └── aoc-taskmaster
├── config
│   ├── btop.conf
│   └── codex-tmux.conf
├── CONTRIBUTING.md
├── crates
│   ├── aoc-agent-wrap-rs
│   ├── aoc-cli
│   ├── aoc-core
│   ├── aoc-hub-rs
│   ├── aoc-mission-control
│   ├── aoc-taskmaster
│   ├── Cargo.lock
│   └── Cargo.toml
├── docs
│   ├── assets
│   ├── feature-upgrade-collection-key.md
│   ├── layouts.md
│   └── mission-control.md
├── install.sh
├── LICENSE
├── plugins
│   └── taskmaster
├── README.md
├── ROADMAP.md
├── scripts
│   ├── build-taskmaster-plugin.sh
│   └── lint.sh
├── yazi
│   ├── init.lua
│   ├── keymap.toml
│   ├── plugins
│   ├── preview.sh
│   ├── theme.toml
│   └── yazi.toml
└── zellij
    ├── aoc.config.kdl
    └── layouts

26 directories, 67 files
```

## 2. Long-Term Memory (`aoc-mem`)
**Purpose:** Persistent storage of architectural decisions.
**Commands:** `aoc-mem read`, `aoc-mem add "fact"`.

## 3. Task Management (`aoc task`)
**Purpose:** Granular tracking of work.
**Commands:** `aoc task list`, `aoc task add "Task"`.

## 4. Operational Rules
- **No Amnesia:** Always check `aoc-mem` first.
- **No Ghost Work:** Track all work in `aoc task` (or `task-master`).

## 5. README Content
# Agent Ops Cockpit (AOC) — Zellij 0.43.1 workspace yeee haw

A lightweight, terminal-first "agent cockpit" layout for coding sessions:

- **Left:** `yazi` file manager (compact view + togglable preview/micro editor)
- **Center top:** agent CLI (default: `codex`)
- **Center bottom:** Taskmaster interactive (fzf-based)
- **Right column:** Calendar/Media widget, Clock, Project terminal
- **Per-tab contract:** one Zellij **tab = one project root** (panes start there)

- [Docs: Custom Layouts / Modes](docs/layouts.md)
- [Docs: Feature Process](docs/feature-upgrade-collection-key.md)

## Requirements

- zellij >= 0.43.1
- yazi (recommended via cargo)
- micro (modern terminal editor, installed automatically via bin/)
- fzf
- python3 (calendar/widget helpers)
- git (recommended for aoc-rlm scan + gitignore)
- tmux (optional, for Codex scrollback in Zellij)
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

Linux source-build deps (Ubuntu/Omakub):
```bash
sudo apt-get update
sudo apt-get install -y pkg-config cmake g++ libharfbuzz-dev libfreetype6-dev libgraphite2-dev
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
- install a `codex` shim into `~/bin` when available so Codex always starts
  through the tmux wrapper

Ensure `~/.local/bin` is in PATH.

Verify dependencies:

```bash
aoc-doctor
```

Setup checklist:
- `zellij --version` is >= 0.43.1
- `yazi` opens and previews images
- Widget pane renders an image after setting a media path

## System Architecture & Philosophy

AOC is not just a layout; it is a **Distributed Cognitive Architecture** for AI-assisted development. It splits context into three distinct, specialized layers to maximize agent performance and consistency across projects.

### The Stack

1.  **Project Context (`.aoc/context.md`)**
    *   **Role:** The "Project Map."
    *   **Content:** Auto-generated snapshot of the file tree and `README`.
    *   **Tool:** `aoc-init` (manual) / `aoc-watcher` (automatic).
    *   **Philosophy:** **Reactive.** Updated in real-time as you edit files, so agents always see the current state.

2.  **Long-Term Memory (`.aoc/memory.md`)**
    *   **Role:** The "Logbook."
    *   **Content:** Persistent architectural decisions, user preferences, and evolution history.
    *   **Tool:** `aoc-mem` (append-only logging).
    *   **Philosophy:** Permanent. Agents read this to understand *why* things are the way they are.

3.  **Task State (`.taskmaster/tasks/tasks.json`)**
    *   **Role:** The "TodoList."
    *   **Content:** Active work items, status, dependencies.
    *   **Tool:** `aoc task` (preferred) / `aoc-taskmaster` (npm optional).
    *   **Philosophy:** Dynamic. High-frequency updates during work.

### Onboarding a Project (`aoc-init`)
The `aoc-init` command is the universal entry point. It "standardizes" any directory by:
1.  Creating the `.aoc/` structure.
2.  Auto-generating the `context.md` context.
3.  Initializing Taskmaster and seeding it with your global preferences (`~/.taskmaster/config.json`).

## Launch

**Step 1: Initialize (Once per project)**
Inside your project directory:

```bash
aoc-init
```

**Step 2: Launch Cockpit**

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
- `m` media
- `g` gallery (renders files from `~/Pictures/Zellij`)
- `p` set media path (mp4/webm/gif/png/jpg/webp/svg)
- In gallery mode, `Enter` toggles a clean view (image only).
  - While in clean view, use arrows or `h/j/k/l` to nudge the image; `0` resets.
  - Press `S` to save the current gallery view as the default on next launch.
Media rendering controls:
- `s` cycle ASCII styles
- `C` cycle color depth
- `D` cycle dither mode
- `w` cycle detail
- `r` edit font ratio (aspect) with h/j/k/l, Enter to apply
- `+/-` adjust render size
You can also set defaults via env vars: `AOC_WIDGET_SYMBOLS`, `AOC_WIDGET_COLORS`, `AOC_WIDGET_DITHER`, `AOC_WIDGET_SCALE`, `AOC_WIDGET_WORK`, `AOC_WIDGET_FONT_RATIO`.

Media is rendered as ASCII via chafa (videos animated).

## Yazi File Manager

The Yazi pane displays command tips in the status bar (similar to Zellij's status bar).

### Yazi Keybindings

| Key | Action |
|-----|--------|
| `Enter` | Open file/directory + expand pane for better visibility |
| `e` | Edit file with `$EDITOR` |
| `q` | Quit Yazi |
| `y` | Set widget media path to selected file |
| `p` | Send selected file to floating preview pane |
| `P` | Toggle floating preview pane |
| `Ctrl+p` | Toggle Yazi's built-in preview split |
| `S` | Star the selected directory (or file's parent) |
| `Esc` | Cancel/escape current action |

### Editing Files

By default, AOC uses **`micro`**, a modern terminal editor that feels like a standard GUI editor (supports mouse selection and common shortcuts).

When you press `Enter` on a file in Yazi, the pane expands for better visibility.

**Micro Shortcuts:**
- **Save:** `Ctrl+s`
- **Quit:** `Ctrl+q`
- **Copy:** `Ctrl+c`
- **Paste:** `Ctrl+v`
- **Undo:** `Ctrl+z`

**Alternative Editors:**
If you prefer a different editor, you can change the `EDITOR` variable in your `~/.bashrc`. However, AOC enforces `micro` in some panes (like Taskmaster and Yazi) via the Zellij layout to ensure a consistent developer experience for beginners.

### Yazi Configuration Files

| File | Purpose |
|------|---------|
| `yazi/yazi.toml` | Main config (compact view, hidden files, sorting) |
| `yazi/keymap.toml` | Custom keybindings for AOC integration |
| `yazi/theme.toml` | Catppuccin-inspired colors for file types |
| `yazi/init.lua` | Status bar command tips |
| `yazi/preview.sh` | Rich file previews (images, PDFs, LaTeX) |
| `yazi/plugins/aoc-open.yazi/` | Open + resize pane plugin |
| `yazi/plugins/aoc-preview-toggle.yazi/` | Toggle preview layout |

## Notes
- The layout expects `codex` (or your selected agent) to be in PATH.
- By default, the agent pane runs through `aoc-agent-run`, which picks the
  selected agent and wraps it in tmux when supported.
- `aoc-codex` still wraps Codex directly; use it if you want the Codex-only
  wrapper.
- The installer drops a `codex` shim in `~/bin` (when it exists) to ensure
  `codex` always uses the tmux wrapper even outside Zellij.
- Taskmaster script expects `task-master` in PATH; otherwise use `aoc task` for mutations.
- Claude plan sync (manual): `aoc task sync --from claude` or `--to claude` (uses `plansDirectory`).

### Agent selection
- Set the default agent with `aoc-agent --set` (or run `aoc-agent` for a menu).
- Open a new tab with a specific agent:
  - `aoc-codex-tab`, `aoc-gemini`, `aoc-cc`, `aoc-oc`
- `AOC_AGENT_ID` overrides the default for a single launch or tab.
- Running the raw `gemini`, `claude`, or `opencode` commands now routes through
  `aoc-agent-wrap` so the TUI gets the tmux-backed scroll history just like
  `codex`; override the real executable via `AOC_GEMINI_BIN`, `AOC_CC_BIN`, or
  `AOC_OC_BIN` if needed.

## Taskmaster Plugin (Default)
The default AOC layout uses the realtime Taskmaster Rust/WASM plugin.

```bash
./scripts/build-taskmaster-plugin.sh
./install.sh
aoc-launch
```

Shortcut: run `aoc-test` to launch the default layout (opens a new tab when already in Zellij).

**Key Controls:**
- `j` / `k` (or arrows/scroll wheel): Move selection up/down
- `x`: Toggle task/subtask status (Done/Pending)
- `Space`: Expand/Collapse subtasks
- `Enter`: Toggle Details pane
- `Tab`: Switch focus between List and Details
- `f`: Cycle Status Filter (All -> Pending -> Done)
- `t`: Cycle Project Tag (Context)
- `?`: Toggle Help panel
- `r`: Refresh tasks manually

**Mouse Support:**
- **Left Click:** Select task
- **Click Selected:** Toggle Details pane
- **Scroll Wheel:** Move selection up/down

**Features:**
- **Realtime Persistence:** Changes (status toggles) are saved immediately to `tasks.json`.
- **Subtasks:** Full support for nested subtask rendering and interaction.
- **Multi-Tag Workflow:** seamless switching between task lists (e.g., `[master]`, `[feature-x]`).
- **Rich UI:** Nerd Fonts, progress bars, and dependency visualization.

### Why the tmux wrapper?
Codex is a full-screen TUI. Zellij can struggle to track scrollback for TUI apps,
so we wrap Codex in tmux with alternate-screen disabled. This makes scrollback
reliable in Zellij panes while keeping Codex behavior the same in other terminals.

## Customization
- **New:** [Create Custom Layouts ("AOC Modes")](docs/layouts.md) that adapt to your project.
- Override commands via env vars: `AOC_AGENT_CMD`, `AOC_CODEX_CMD`, `AOC_TASKMASTER_CMD`, `AOC_FILETREE_CMD`, `AOC_WIDGET_CMD`, `AOC_CLOCK_CMD`, `AOC_SYS_CMD`, `AOC_TERMINAL_CMD`.
- Override the tmux config used by `aoc-codex` with `AOC_CODEX_TMUX_CONF`.
- Override the tmux config for other agent CLIs with `AOC_AGENT_TMUX_CONF`.
- Override agent binaries with `AOC_GEMINI_BIN`, `AOC_CC_BIN`, `AOC_OC_BIN`.
- AOC defaults to `~/.config/zellij/aoc.config.kdl`, which keeps the full UI (top/bottom bars) and starts in normal mode. Set `AOC_ZELLIJ_CONFIG` to use a different config file.
- `Alt ?` cycles swap layouts if you define them in your Zellij config.
- When launching `aoc` outside Zellij, AOC will attempt to fullscreen the current terminal window on X11. Set `AOC_FULLSCREEN=0` to disable.
  - Linux (X11): requires `wmctrl` or `xdotool`; Wayland compositors do not allow this, use your window manager bindings.
  - macOS: use the system fullscreen shortcut (Ctrl+Cmd+F) or configure Alacritty/iTerm to start fullscreen.
  - Windows: use terminal settings or Win+Up to maximize/fullscreen.
- Float preview pane placement can be customized with `AOC_PREVIEW_WIDTH`, `AOC_PREVIEW_HEIGHT`, `AOC_PREVIEW_X`, `AOC_PREVIEW_Y`, `AOC_PREVIEW_PINNED`, and `AOC_PREVIEW_PANE_NAME`.
- To tweak pane sizes, copy the layout:
  `cp ~/.config/zellij/layouts/aoc.kdl ~/.config/zellij/layouts/aoc.local.kdl`
  `aoc-launch` will use `aoc.local` if present.

## Clock Options
- `AOC_CLOCK_INTERVAL=1` controls refresh interval (seconds).
- `AOC_CLOCK_TIME_FORMAT` sets the `date` format for the big time (default: `%H:%M`).
- `AOC_CLOCK_DATE_FORMAT` sets the `date` format for the line below (default: `%A, %B %d`).
- `AOC_CLOCK_FONT` sets the figlet font (default: `small`, requires `figlet` in PATH).
- `AOC_CLOCK_BACKEND` selects the backend: `auto` (default), `clocktemp`, `tty`, or `figlet`.
- In `auto`, AOC prefers ClockTemp (if installed), then `tty-clock`, then the figlet fallback.
- `AOC_CLOCK_CLOCKTEMP_CMD` sets the ClockTemp binary name (default: auto-detects `clocktemp`, `ClockTemp`, or `clock-temp`).
- `AOC_CLOCK_CLOCKTEMP_FLAGS` passes flags directly to ClockTemp (when selected).
- `AOC_CLOCK_TTY_FLAGS` passes flags directly to `tty-clock` (when selected).
- `AOC_CLOCK_AUTO_GEO=1` auto-detects lat/lon for ClockTemp using `ipapi.co` (cached for 24h).
- `AOC_CLOCK_GEO_TTL=86400` controls geo cache TTL in seconds.
- Running `aoc-clock` inside Zellij will respawn a clock pane; set `AOC_CLOCK_SPAWN=0` to disable, `AOC_CLOCK_PANE_NAME` to rename it, and `AOC_CLOCK_PANE_DIRECTION` to control where it splits (default: `up`).
- Run `aoc-clock-geo` to refresh cached location manually.
- Persist clock settings across runs with `aoc-clock-set`.

## Troubleshooting
- Missing previews: install `chafa`, `poppler-utils`, and `librsvg2-bin`.
- Blank task list: ensure `aoc task list` works (or install `task-master` if you want the npm CLI).
- Widget media not rendering: run `aoc-doctor` to confirm `ffmpeg` and `chafa`.
- RLM skill not working: build `aoc-cli` (`cargo build --release -p aoc-cli`) and ensure `aoc-cli` is in PATH.
- TeX preview build errors: install `tectonic` via Cargo using `cargo install --locked tectonic --version 0.14.1`.
- If Cargo builds fail with the `time` crate error, use `cargo +1.78.0 install --locked tectonic --version 0.14.1`
  or `cargo binstall tectonic` for a prebuilt release.
- On Ubuntu, the `bat` binary is named `batcat`; `aoc-doctor` accepts either.

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

## Quick Start for Agents 🤖

If you are an AI agent reading this repo to set it up for a user:

1.  **Install:** Run `./install.sh` to deploy the toolchain (`aoc-init`, `aoc-mem`, etc.) to the user's path.
2.  **Initialize:** Run `aoc-init` in the project root to generate the standard context structure.
3.  **Orient:** Run `aoc-mem read` to ingest the long-term history and architectural decisions of this project.
4.  **Work:** Use `aoc task list` to find pending work, and `aoc task add "..."` to track your plan.

## RLM Skill (Large Codebase Analysis)
Use the Rust-based RLM tool as the default workflow for large repos:

1. **Scan:** `aoc-rlm scan` to measure scale.
2. **Peek:** `aoc-rlm peek "search_term"` for fast snippets.
3. **Slice:** `aoc-rlm chunk --pattern "src/relevant/*.rs"` for chunked processing.

`aoc-rlm` is backed by the Rust `aoc-cli` implementation for speed; build it with
`cargo build --release -p aoc-cli` if you haven't installed binaries yet.

## 6. Active Workstreams (Tags)
```
master (42)
mission-control (10)
```

## 7. RLM Skill (Large Codebase Analysis)
When you need to analyze more files than fit in your context:
1. **Scan:** Run `aoc-rlm scan` (or `rlm scan`) to see the scale of the codebase.
2. **Peek:** Run `aoc-rlm peek "search_term"` (or `rlm peek`) to find relevant snippets and file paths.
3. **Slice:** Run `aoc-rlm chunk --pattern "src/relevant/*.rs"` (or `rlm chunk`) to get JSON chunks.
4. **Process:** Use your available sub-agent tools (like `Task`) to process chunks in parallel.
5. **Reduce:** Synthesize the sub-agent outputs into a final answer.
