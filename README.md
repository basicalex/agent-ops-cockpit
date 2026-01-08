# Agent Ops Cockpit (AOC) — Zellij 0.43.1 workspace

A lightweight, terminal-first "agent cockpit" layout for coding sessions:

- **Left:** `yazi` file manager (compact view + togglable preview)
- **Center top:** `codex` (always visible)
- **Center bottom:** Taskmaster interactive (fzf-based)
- **Right column:** Calendar/Media widget, Clock, Project terminal
- **Per-tab contract:** one Zellij **tab = one project root** (panes start there)

## Requirements

- zellij >= 0.43.1
- yazi (recommended via cargo)
- fzf
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
- `+/-` adjust render size
You can also set defaults via env vars: `AOC_WIDGET_SYMBOLS`, `AOC_WIDGET_COLORS`, `AOC_WIDGET_DITHER`, `AOC_WIDGET_SCALE`, `AOC_WIDGET_WORK`.

Media is rendered as ASCII via chafa (videos animated).

In Yazi:
- `y` set the widget media path to the selected file.
- `p` send selected file to the floating preview pane.
- `P` toggle the floating preview pane.
- `Ctrl+p` toggle Yazi's built-in preview split.
- `S` star the selected directory (or file's parent).

## Notes
- The layout expects `codex` to be in PATH.
- By default, the Codex pane runs through `aoc-codex`, which wraps Codex in tmux
  (with alternate-screen disabled) so you get scrollback inside Zellij.
- The installer drops a `codex` shim in `~/bin` (when it exists) to ensure
  `codex` always uses the tmux wrapper even outside Zellij.
- Taskmaster script expects `task-master-ai` in PATH; adjust in `bin/aoc-taskmaster` if needed.

### Why the tmux wrapper?
Codex is a full-screen TUI. Zellij can struggle to track scrollback for TUI apps,
so we wrap Codex in tmux with alternate-screen disabled. This makes scrollback
reliable in Zellij panes while keeping Codex behavior the same in other terminals.

## Customization
- Override commands via env vars: `AOC_CODEX_CMD`, `AOC_TASKMASTER_CMD`, `AOC_FILETREE_CMD`, `AOC_WIDGET_CMD`, `AOC_CLOCK_CMD`, `AOC_SYS_CMD`, `AOC_TERMINAL_CMD`.
- Override the tmux config used by `aoc-codex` with `AOC_CODEX_TMUX_CONF`.
- AOC defaults to `~/.config/zellij/aoc.config.kdl`, which keeps the full UI (top/bottom bars) and starts in normal mode. Set `AOC_ZELLIJ_CONFIG` to use a different config file.
- `Alt ?` cycles swap layouts if you define them in your Zellij config.
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
- Run `aoc-clock-geo` to refresh cached location manually.
- Persist clock settings across runs with `aoc-clock-set`.

## Troubleshooting
- Missing previews: install `chafa`, `poppler-utils`, and `librsvg2-bin`.
- Blank task list: ensure `task-master-ai` is in PATH.
- Widget media not rendering: run `aoc-doctor` to confirm `ffmpeg` and `chafa`.
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
