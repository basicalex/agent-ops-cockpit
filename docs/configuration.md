# Configuration Guide

Advanced configuration options for Agent Ops Cockpit (AOC).

## Table of Contents

- [Environment Variables](#environment-variables)
  - [Command Overrides](#command-overrides)
  - [Widget Configuration](#widget-configuration)
  - [Clock Configuration](#clock-configuration)
  - [Layout and Display](#layout-and-display)
  - [Agent Configuration](#agent-configuration)
- [Custom Layouts](#custom-layouts)
- [Per-Project Configuration](#per-project-configuration)

## Environment Variables

### Command Overrides

Override default commands used in AOC layouts:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_AGENT_CMD` | Command to run in agent pane | Auto-detected |
| `AOC_CODEX_CMD` | Codex-specific command | `codex` |
| `AOC_TASKMASTER_CMD` | Taskmaster TUI command | `aoc-taskmaster` |
| `AOC_FILETREE_CMD` | File manager command | `yazi` |
| `AOC_WIDGET_CMD` | Widget pane command | `aoc-widget` |
| `AOC_CLOCK_CMD` | Clock command | Auto-detected |
| `AOC_SYS_CMD` | System stats command | `aoc-sys` |
| `AOC_TERMINAL_CMD` | Terminal shell | `$SHELL` |

### Widget Configuration

Control the media/calendar widget rendering:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_WIDGET_SYMBOLS` | ASCII style preset | Auto |
| `AOC_WIDGET_COLORS` | Color depth (1/8/16/256/truecolor) | Auto |
| `AOC_WIDGET_DITHER` | Dithering mode | Auto |
| `AOC_WIDGET_SCALE` | Render size | Auto |
| `AOC_WIDGET_WORK` | Detail level | Auto |
| `AOC_WIDGET_FONT_RATIO` | Aspect ratio for rendering | Auto |

**Widget Controls (when widget is focused):**

- `m` - Switch to media mode
- `g` - Switch to gallery mode (renders from `~/Pictures/Zellij`)
- `p` - Set media path (mp4/webm/gif/png/jpg/webp/svg)
- `Enter` (in gallery) - Toggle clean view
- `s` - Cycle ASCII styles
- `C` - Cycle color depth
- `D` - Cycle dither mode
- `w` - Cycle detail level
- `r` - Edit font ratio with h/j/k/l
- `+/-` - Adjust render size

### Clock Configuration

Fine-tune the clock widget appearance:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_CLOCK_INTERVAL` | Refresh interval in seconds | `1` |
| `AOC_CLOCK_TIME_FORMAT` | Time format (date format string) | `%H:%M` |
| `AOC_CLOCK_DATE_FORMAT` | Date format (date format string) | `%A, %B %d` |
| `AOC_CLOCK_FONT` | Figlet font name | `small` |
| `AOC_CLOCK_BACKEND` | Backend selection | `auto` |
| `AOC_CLOCK_CLOCKTEMP_CMD` | ClockTemp binary name | Auto-detect |
| `AOC_CLOCK_CLOCKTEMP_FLAGS` | Flags for ClockTemp | None |
| `AOC_CLOCK_TTY_FLAGS` | Flags for tty-clock | None |
| `AOC_CLOCK_AUTO_GEO` | Auto-detect location | `1` (enabled) |
| `AOC_CLOCK_GEO_TTL` | Geo cache TTL in seconds | `86400` (24h) |
| `AOC_CLOCK_SPAWN` | Spawn new pane when running in Zellij | `1` |
| `AOC_CLOCK_PANE_NAME` | Name for clock pane | `Clock` |
| `AOC_CLOCK_PANE_DIRECTION` | Split direction for clock pane | `up` |

**Backend Priority (when `AOC_CLOCK_BACKEND=auto`):**
1. ClockTemp (if installed)
2. tty-clock (if installed)
3. Figlet fallback

**Manual Location Refresh:**

```bash
aoc-clock-geo
```

**Persist Clock Settings:**

```bash
aoc-clock-set
```

### Layout and Display

Control layout behavior and appearance:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_ZELLIJ_CONFIG` | Custom Zellij config file | `~/.config/zellij/aoc.config.kdl` |
| `AOC_FULLSCREEN` | Auto-fullscreen on launch | `1` (Linux X11 only) |
| `AOC_CONTROL_FLOATING` | Open aoc-control as floating pane | `1` |
| `AOC_CLEANUP` | Run cleanup on launch | `1` |
| `AOC_CLEANUP_SESSIONS` | Limit cleanup to sessions (`current` or comma list) | All sessions |
| `AOC_CLEANUP_PANE_STRICT` | Allow cleanup within sessions based on pane layout | `0` |
| `AOC_CLEANUP_INTERACTIVE` | Prompt for cleanup mode when interactive | `1` |

### Pulse and Mission Control

Control Pulse vNext and the Mission Control Pulse Overview mode:

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_PULSE_VNEXT_ENABLED` | Enable Pulse UDS hub/subscriber paths | `1` |
| `AOC_PULSE_OVERVIEW_ENABLED` | Enable Pulse Overview pane mode and related polling/display paths | `0` |

Notes:

- With `AOC_PULSE_OVERVIEW_ENABLED=0` (default), Mission Control starts in Work mode and cycles Work/Diff/Health.
- Set `AOC_PULSE_OVERVIEW_ENABLED=1` to re-enable Overview for experimentation.
- Sidecar/overview code remains in-repo for a later phase; this flag is the rollout gate.

**Preview Pane Placement:**

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_PREVIEW_WIDTH` | Floating preview width | Percentage |
| `AOC_PREVIEW_HEIGHT` | Floating preview height | Percentage |
| `AOC_PREVIEW_X` | X position | Percentage |
| `AOC_PREVIEW_Y` | Y position | Percentage |
| `AOC_PREVIEW_PINED` | Keep pinned | Boolean |
| `AOC_PREVIEW_PANE_NAME` | Pane name | `Preview` |

### Zellij Shortcuts (AOC Defaults)

AOC ships a custom Zellij keybind layer in `~/.config/zellij/aoc.config.kdl` (or `AOC_ZELLIJ_CONFIG`). These are the most used Alt bindings; Zellij defaults still apply.

| Key | Action |
|----------|-------------|
| `Alt c` | Toggle AOC control (floating) |
| `Alt s` | Next swap layout |
| `Alt f` | Toggle floating panes |
| `Alt n` | New pane |
| `Alt i` | Previous tab |
| `Alt o` | Next tab |
| `Alt u` | Move tab left |
| `Alt p` | Move tab right |
| `Alt [` | Toggle pane grouping |
| `Alt ]` | Next tab (alias) |
| `Alt h/j/k/l` | Move focus |
| `Alt =/-` | Resize |

### Agent Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `AOC_AGENT_ID` | Override default agent for session | From `aoc-agent` |
| `AOC_GEMINI_BIN` | Gemini binary path | Auto-detect |
| `AOC_CC_BIN` | Claude Code binary path | Auto-detect |
| `AOC_OC_BIN` | OpenCode binary path | Auto-detect |
| `AOC_AGENT_PATTERN` | Additional agent names for cleanup | None |
| `AOC_CODEX_TMUX_CONF` | Custom tmux config for Codex | Default |
| `AOC_AGENT_TMUX_CONF` | Custom tmux config for other agents | Default |

## Custom Layouts

AOC supports custom "AOC Modes" - see [Custom Layouts Guide](layouts.md) for details.

**Quick Reference:**

```bash
# Use minimal layout
aoc-new-tab --layout minimal

# Set default layout
aoc-layout --set minimal

# Create custom layout in ~/.config/zellij/layouts/
```

**Layout Placeholders:**

When creating custom layouts, AOC automatically replaces these tokens:

- `__AOC_TAB_NAME__` → Tab name
- `__AOC_PROJECT_ROOT__` → Absolute project path
- `__AOC_AGENT_ID__` → Unique agent/project ID

## Per-Project Configuration

AOC uses a **Distributed Cognitive Architecture** with three layers:

### 1. Project Context (`.aoc/context.md`)

- **Purpose:** Auto-generated project map
- **Content:** File tree, README snapshot
- **Refresh:** `aoc-init` (manual) or `aoc-watcher` (auto)

### 2. Long-Term Memory (`.aoc/memory.md`)

- **Purpose:** Persistent architectural decisions
- **Access:** `aoc-mem read` (start of task), `aoc-mem add` (decisions)

### 3. Task State (`.taskmaster/tasks/tasks.json`)

- **Purpose:** Active work queue
- **Management:** `aoc-task` commands

### Global Configuration

User defaults are stored in:

- `~/.config/aoc/config.toml` - AOC settings
- `~/.taskmaster/config.json` - Taskmaster preferences

---

**See Also:**
- [Installation Guide](installation.md)
- [Custom Layouts](layouts.md)
- [Main README](../README.md)
