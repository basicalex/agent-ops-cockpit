# Agent Ops Cockpit (AOC)

## Project Overview

**Agent Ops Cockpit (AOC)** is a highly specialized, terminal-first workspace configuration designed to optimize coding sessions with AI agents. It leverages **Zellij** as the terminal multiplexer to create a "cockpit" layout that integrates:

*   **File Management:** `yazi` (left pane)
*   **AI Agent Interaction:** A dedicated pane for agents like `codex`, `gemini`, `claude`, and `opencode` (center top).
*   **Task Management:** A custom "Taskmaster" tool (center bottom).
*   **Widgets:** Calendar, clock, media, and system stats (right column).
*   **Project Terminal:** A standard shell pane rooted in the project directory (right bottom).

The project is polyglot, consisting primarily of:
*   **Shell Scripts (`bin/`):** The core logic for launching the environment, managing agents, and wrapping tools.
*   **Zellij Layouts (`zellij/`):** KDL files defining the pane structure and plugins.
*   **Rust (`plugins/taskmaster`):** A custom Zellij WASM plugin for task management.
*   **Python (`ClockTemp/`):** Scripts for the clock and weather widget.

## Key Components

### 1. The Core Scripts (`bin/`)
These scripts are installed to `~/.local/bin` and drive the entire experience.
*   `aoc`: The main entry point. Launches a new AOC tab or session.
*   `aoc-launch`: Boots the full Zellij layout.
*   `aoc-agent`: Manages which AI agent is active (Codex, Gemini, Claude, etc.).
*   `aoc-agent-run`: The "runner" script that executes the selected agent in the center pane.
*   `aoc-agent-wrap`: Wraps agent CLIs (often in `tmux`) to provide scrollback and better integration.
*   `aoc-star`: "Stars" a directory, re-anchoring all panes to that path.
*   `aoc-watcher`: A Rust-based background daemon that monitors the project filesystem and automatically regenerates `context.md` when files change.
*   `aoc-align`: Automatically re-aligns the current terminal pane to the project root (used when switching tabs or re-anchoring).

### 2. Zellij Configuration (`zellij/`)
*   `layouts/aoc.kdl`: The primary layout definition. It uses `bash` commands to launch the specific `aoc-*` scripts in each pane.
*   `aoc.config.kdl`: The base Zellij configuration used by AOC.

### 3. Taskmaster Plugin (`plugins/taskmaster`)
A Rust-based WASM plugin for Zellij that provides an interactive task list.
*   **Source:** `src/main.rs`
*   **Build:** Compiles to `wasm32-wasi` or `wasm32-wasip1`.

### 4. ClockTemp (`ClockTemp/`)
A Python-based utility for rendering the clock and weather information.
*   **Scripts:** `script/*.py` handle the logic.

## System Architecture: Per-Tab Isolation

AOC uses a **Distributed Cognitive Architecture** with strict per-tab isolation.

### 1. Layout Injection
To solve Zellij's environment variable limitations, AOC uses "Layout Injection."
*   **Placeholders:** Layout templates use tokens like `__AOC_PROJECT_ROOT__` and `__AOC_ROOT_TAG__`.
*   **Just-In-Time Generation:** `aoc-launch` and `aoc-new-tab` replace these tokens with absolute paths and unique IDs, creating a temporary KDL file for each tab.
*   **Anchoring:** Every tab's panes are named `aoc:<root_tag>`, allowing tools to discover the tab's project root via `zellij action dump-layout`.

### 2. Reactive Context (`aoc-watcher`)
The `aoc-watcher` service provides "Live Context":
*   **Discovery:** It scans the active Zellij session for `aoc:<root_tag>` panes to identify all active project roots.
*   **Monitoring:** It spawns efficient `notify` (inotify) watchers for each root.
*   **Atomic Updates:** When a file is saved, it regenerates `.aoc/context.md` atomically, ensuring AI agents always have an up-to-date map of the project.

### 3. Memory System (`.aoc/memory.md`)
A lightweight, markdown-based long-term memory for agents, stored directly in the project.
*   **Location:** `.aoc/memory.md` (project-local).
*   **Tool:** `bin/aoc-mem` manages this file.
*   **Agent Instruction:**
    *   **READ** this memory at the start of a task to understand past decisions and user preferences.
    *   **WRITE** to this memory (via `aoc-mem add`) when making significant architectural choices or learning a new user preference.
*   **Commands:**
    *   `aoc-mem add "fact"`: Record a new decision/fact.
    *   `aoc-mem read`: Dump full context.
    *   `aoc-mem search "query"`: Find specific info.

## Building and Installation

This project does not have a single "build" step in the traditional sense, but rather an **installation** process.

### Installation
Run the installer to deploy scripts and configs to your user directories (`~/.local/bin`, `~/.config/zellij`, etc.).

```bash
./install.sh
```

### Building the Taskmaster Plugin
To build the Rust plugin:

```bash
./scripts/build-taskmaster-plugin.sh
```

### Dependencies
The system relies on several external tools being present in your `$PATH`:
*   `zellij` (>= 0.43.1)
*   `yazi`
*   `fzf`
*   `tmux` (optional, for some agents)
*   `chafa`, `ffmpeg` (for media widgets)

Run the doctor script to verify:
```bash
aoc-doctor
```

## Development Conventions

*   **Shell Scripts:** All scripts in `bin/` should use `#!/usr/bin/env bash` and `set -euo pipefail` for safety.
*   **Naming:** Scripts are prefixed with `aoc-` to avoid namespace collisions.
*   **Wrappers:** Agents are typically wrapped (via `aoc-agent-wrap`) to ensure consistent behavior (scrollback, signal handling) within the Zellij panes.
*   **State:** The system uses `~/.local/state/aoc` to persist state like the current project root or the active default agent.
