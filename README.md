# AOC - Terminal-First AI Workspace

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](./CHANGELOG.md)
[![Zellij](https://img.shields.io/badge/zellij-%E2%89%A50.44.0%20recommended-green.svg)](https://zellij.dev)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)
[![Build](https://github.com/basicalex/agent-ops-cockpit/actions/workflows/ci.yml/badge.svg)](https://github.com/basicalex/agent-ops-cockpit/actions/workflows/ci.yml)

> **The Distributed Cognitive Architecture for AI-Assisted Development**

AOC (Agent Ops Cockpit) is a terminal-first AI workspace built around **Zellij**, a persistent **PI coding agent**, **project memory**, **Taskmaster**, and an **Alt+C control pane** for installing and operating optional integrations.

Use it to:
- run a persistent coding agent inside a project-aware terminal workspace
- keep project context and architectural memory in-repo
- manage work through Taskmaster without leaving the terminal
- enable optional web research with **Agent Browser + managed local search**

[📸 Screenshot](./docs/assets/aoc1.png) | [📖 Quick Start](#quick-start) | [🕹️ Alt+C Control](#-altc-control-pane) | [🔧 Installation](#installation) | [📚 Documentation](#documentation)

---

<img width="3840" height="2096" alt="image" src="https://github.com/user-attachments/assets/fad6e520-c409-49c0-a024-2b29cc236a64" />


## ✨ Why AOC?

### The Problem with AI Development Today

Traditional workflows fragment your AI context across browser tabs, terminal windows, and scattered notes:

- **Lost Context** - Every new chat starts from zero
- **Manual Copy-Pasting** - Code, tasks, and decisions live in different places
- **No Project Memory** - AI can't remember previous decisions or constraints
- **Fragmented Workflow** - Switching between file manager, editor, terminal, and AI interface

### The AOC Solution

AOC implements a **Distributed Cognitive Architecture** that separates concerns into three persistent layers:

```
┌─────────────────────────────────────────────────────────────────┐
│                    AOC Workspace Layout                         │
├──────────────────┬──────────────────┬───────────────────────────┤
│   📁 Yazi        │   🤖 Agent       │   📡 Pulse                │
│   File Manager   │   (pi)           │   Session telemetry       │
│                  │   PI-only mode   │   + runtime status        │
├──────────────────┼──────────────────┴───────────────────────────┤
│   Project Files  │   📋 Taskmaster TUI                          │
│                  │   Interactive task & subtask management      │
└──────────────────┴──────────────────────────────────────────────┘
         │                    │                    │
         └────────────────────┼────────────────────┘
                              ▼
        ┌──────────────────────────────────────────────────┐
        │         DISTRIBUTED COGNITIVE ARCHITECTURE       │
        ├──────────────────────────────────────────────────┤
        │                                                  │
        │  🗺️ Context        🧠 Memory          ✅ Tasks    │
        │  (Reactive)       (Persistent)       (Dynamic)   │
        │                                                  │
        │  .aoc/context.md  .aoc/memory.md     tasks.json  │
        │  Auto-updated     Append-only        Real-time   │
        │  Project facts +  Architectural      Status &    │
        │  structure map    decisions          priorities  │
        │                                                  │
        └──────────────────────────────────────────────────┘
```

**Result:** Your AI agents maintain context across sessions, remember your preferences, and track work items—all automatically.

---

## 🚦 Start Here

Most users only need this flow:

1. Install AOC
2. Run `aoc` inside a project
3. Press `Alt+C` to open the control pane
4. Use **Settings -> Tools** to install/configure optional integrations
5. Run `aoc-doctor` if something looks off

If you want optional web research, go to:

- `Alt+C -> Settings -> Tools -> Agent Browser + Search`

and run:

1. `Install/update Agent Browser tool`
2. `Install/update PI skill`
3. `Install/update PI web research skill`
4. `Enable managed local search (SearXNG)`
5. `Start/verify local search`
6. `Verify web research stack`

## 🚀 Quick Start

### One-Line Install

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

The bootstrap entrypoint downloads the latest release installer (portable Rust binary when available), falls back to source install when needed, and installs AOC to user-local paths.

For forks or custom mirrors, pass `--repo <owner/name>`.

For a local clone workflow, you can still run:

```bash
./install.sh
```

That's it. AOC will:
1. Install all scripts and configurations
2. Initialize your project's cognitive architecture
3. Launch the workspace

### Verify Installation

```bash
aoc-doctor
```

### After Install

Do this first:

```bash
aoc-doctor
cd ~/your-project
aoc
```

Then inside AOC:
- use Yazi to move around the repo
- use the PI pane for coding work
- use Taskmaster for active tasks
- press `Alt+C` for operator/configuration actions

### Next Steps

Choose your path:

| 🚀 **Start Coding** | 🤖 **Configure Agents** | 🔧 **Customize** |
|---------------------|------------------------|------------------|
| `aoc` in any project dir | `aoc-agent --set` | `aoc.minimal` |
| Open files in Yazi | Choose PI Agent (npm, recommended) | Create your own "AOC Modes" |
| Press `Enter` to edit with `micro` | PI-first core + optional BYO wrappers | [Custom Layouts Guide](./docs/layouts.md) |

---

## 🎯 Core Workflow

AOC is easiest to understand as four core pieces:

1. **PI runtime** for coding and terminal collaboration
2. **Taskmaster** for active work tracking
3. **AOC memory/context** for project continuity
4. **Alt+C** for operator controls and integrations

Optional integrations like web research, Vercel, and MoreMotion layer on top of that core.

## 🎯 Key Features

### Core Features

### 1. PI-Only Agent Runtime

AOC now runs in PI-only mode with a single runtime:

```bash
# Set/select runtime
aoc-agent --set pi

# Or launch directly
aoc-pi           # Open tab with PI Agent (npm)
```

**PI runtime gets:**
- Persistent project memory (`.aoc/memory.md`)
- Real-time context updates (`.aoc/context.md`)
- Task integration (Taskmaster TUI)
- tmux-backed scrollback for reliability
- stable per-project Zellij sessions for better attach/resurrection DX

Need another agent CLI? AOC keeps core support PI-only, but you can plug in alternatives via wrappers: [Agent Extensibility](./docs/agent-extensibility.md).

### 2. Native Taskmaster TUI

Rust-based task management with rich interactions:

| Feature | Key |
|---------|-----|
| Toggle task status | `x` |
| Expand/collapse subtasks | `Space` |
| Cycle project tags | `t` |
| Filter by status | `f` |
| Toggle details | `Enter` |
| Mouse support | Click & scroll |

**Features:**
- ✅ Nested subtasks with expand/collapse
- ✅ Multiple project contexts (tags)
- ✅ Real-time persistence to `tasks.json`
- ✅ Status filtering (All/Pending/Done)
- ✅ Progress bars and dependency visualization

### 3. Insight CLI - Mind-backed retrieval and provenance

Use `aoc insight` (or `bin/aoc-insight`) from inside an AOC pane to query the live Mind/Insight runtime:

```bash
# Retrieve bounded citations/snippets across project canon + session exports
aoc insight retrieve --scope auto --mode brief --active-tag mind "planner drift"

# Inspect provenance / traversal graph for an artifact, task, or file
aoc insight provenance --artifact-id obs:1
aoc insight provenance --task-id 132
aoc insight provenance --file-path docs/configuration.md

# Check runtime health
aoc insight status
```

### 4. RLM Skill - Large Codebase Analysis

Built-in tooling for analyzing large repositories without context overflow:

```bash
# Measure repository scale
aoc-rlm scan

# Search across codebase
aoc-rlm peek "search_term"

# Process in manageable chunks
aoc-rlm chunk --pattern "src/relevant/*.rs"
```

### 4. Agent Skills

Reusable workflow playbooks stored in `.pi/skills/` (PI-first canonical):

```bash
# Sync PI skills
aoc-skill sync --agent pi
```

**Included skills:** `aoc-workflow`, `teach-workflow`, `memory-ops`, `taskmaster-ops`, `tm-cc`, `rlm-analysis`, `prd-dev`, `prd-intake`, `prd-align`, `tag-align`, `task-breakdown`, `task-checker`, `release-notes`, `skill-creator`, `zellij-theme-ops`.

`aoc-init` seeds default PI skills in `.pi/skills` and syncs PI skills only. PI is the only supported runtime direction in AOC.

PI smoke checks:

```bash
bash scripts/pi/test-aoc-init-pi-first.sh
bash scripts/pi/test-pi-only-agent-surface.sh
```

**Teach mode (PI):**

PI (project prompt templates seeded by `aoc-init`):

```bash
/teach
/teach-full
/teach-dive ingestion
/teach-ask how are you useful?
/tm-cc
```

Teach mode is read-first by default, explains implementation with file references, and stores optional local continuity notes under `.aoc/insight/`.

**Optional MoreMotion (React projects):**

```bash
aoc-momo init
```

Then use:

```text
PI: /momo
```

### 5. Yazi File Manager Integration

Keyboard-driven file management with rich previews:

| Key | Action |
|-----|--------|
| `Enter` / `o` | Smart open (dir enter, text edit, media default app) |
| `e` | Toggle Full Yazi mode (expand pane + configurable 3-col/2-col view) |
| `g s` | Edit short-term memory |
| `g S` | Jump to `.aoc/stm` |

Optional tuning via env vars:
- `AOC_YAZI_PANE_EXPANDED_VIEW=2col|3col` (default `2col`)
- `AOC_YAZI_PANE_COLLAPSE_RIGHT_COLUMN=1|0` (default `1`) + `AOC_YAZI_RIGHT_COLUMN_COLLAPSE_STEPS=6` to temporarily shrink the right-side Pulse/Terminal column while Yazi is expanded

**Preview support:** Native Yazi previews for files and images (including SVGs when native Yazi dependencies/backend are available)

## 🕹️ Alt+C Control Pane

`Alt+C` opens `aoc-control`, the operator surface for AOC.

Use it for:
- checking runtime/tool status
- PI installer actions
- RTK routing controls
- PI compaction presets
- Agent Browser + Search setup
- Vercel CLI setup
- MoreMotion setup flows

The right-hand detail pane explains the currently selected action, and long-running setup flows now run asynchronously where appropriate.

### Agent Browser + Search

AOC supports a **search-first, browse-second** web research workflow:

- `aoc-search` queries managed local SearXNG
- `agent-browser` opens and inspects real web pages
- PI skills teach agents to search first, browse second

This stack is:
- **opt-in**
- **per-repo**
- backed by **Docker + Docker Compose** for managed search

Recommended setup flow:

1. `Alt+C -> Settings -> Tools -> Agent Browser + Search`
2. Install/update Agent Browser
3. Install/update PI browser skill
4. Install/update PI web research skill
5. Enable managed local search
6. Start/verify local search
7. Verify web research stack

CLI equivalents:

```bash
aoc-search status
aoc-search start --wait
aoc-search health
aoc-search query --limit 5 "rust clap subcommands"
bin/aoc-web-smoke
```

If `aoc-search` is healthy but `bin/aoc-web-smoke` fails, search is up but browser integration still needs attention.

### Optional Integrations and Advanced Workflows

| Integration | What it does | Where to enable/manage it | Requirements |
|-------------|--------------|----------------------------|--------------|
| Agent Browser | Opens and automates real web pages | `Alt+C -> Settings -> Tools -> Agent Browser + Search` | `agent-browser` install flow |
| Managed Local Search | Local SearXNG for search-first web research | `Alt+C -> Settings -> Tools -> Agent Browser + Search` | Docker + Docker Compose |
| Vercel CLI | Deploy/inspect Vercel projects from AOC workflows | `Alt+C -> Settings -> Tools -> Vercel CLI` | `vercel` CLI |
| MoreMotion | Optional Remotion workflow helpers for React/video projects | `Alt+C -> Settings -> Tools -> MoreMotion` | local repo or supported source flow |

See also: [Installation Guide](./docs/installation.md), [Configuration Guide](./docs/configuration.md), and [Control Pane Guide](./docs/control-pane.md).

### 6. Custom Layouts ("AOC Modes")

Create specialized layouts for different workflows:

```bash
# Try the minimal layout
aoc.minimal

# See available layout shortcuts in this project
# (type and press Tab for completion)
aoc.

# Set as default
aoc-layout --set minimal
```

`aoc` still opens your regular default tab/session. `aoc.<layout>` opens a tab/session with that layout (for example `aoc.hybrid`).

**Included layouts:**
- `aoc` (default) - Full cockpit with all features
- `minimal` - Streamlined for focused work
- `unstat` - AOC layout without zjstatus status bars (temporary fallback for Zellij 0.44 plugin issues)

**Related launchers:**
- `aoc.unstat` - launch AOC with the `unstat` layout
- `aoc.zlj` - launch AOC with the regular `aoc` layout (top/bottom `zjstatus` bars)

**Create your own** with context injection placeholders (`__AOC_PROJECT_ROOT__`, `__AOC_TAB_NAME__`, `__AOC_AGENT_ID__`).

- Team-shared layouts: `.aoc/layouts/` (commit to git)
- Personal layouts: `~/.config/zellij/layouts/`

Named layout resolution prefers project layouts first, then global layouts. See [Custom Layouts Guide](./docs/layouts.md).

---

## 🏗️ Distributed Cognitive Architecture

AOC's architecture solves the fundamental problem of **context management in AI-assisted development**:

### The Three Layers

#### 1. Context (`.aoc/context.md`) - The "Project Map"
- **Role:** Reactive, auto-generated snapshot
- **Content:** Project-specific facts, key files, structure tree, README headings, and workstream tags
- **Update:** Automatic via `aoc-watcher` or manual via `aoc-init`
- **Agent Usage:** Read at task start to understand current codebase state

#### 2. Memory (`.aoc/memory.md`) - The "Logbook"
- **Role:** Persistent, append-only record
- **Content:** Architectural decisions, user preferences, evolution history
- **Update:** Manual via `aoc-mem add "..."`
- **Agent Usage:** Read to understand *why* things are the way they are

#### 3. Tasks (`.taskmaster/tasks/tasks.json`) - The "Todo List"
- **Role:** Dynamic work queue
- **Content:** Active tasks, subtasks, dependencies, priorities
- **Update:** Via Taskmaster TUI, `tm`, or `aoc-task` CLI
- **Agent Usage:** Track work, update status, create new items

#### 4. Task PRDs (`.taskmaster/docs/prds/`) - The "Spec Layer"
- **Role:** Task-level implementation specification
- **Content:** Problem framing, requirements, acceptance criteria, validation plan
- **Linking:** Stored on each task as `aocPrd` (task-level only; no subtask PRDs)
- **Update:** Via `aoc-task prd show|init|set|clear|parse`

#### 5. Short-Term Memory (`.aoc/stm/`) - The "Handoff Buffer"
- **Role:** Session diary + handoff context for transparent agent continuity
- **Content:** working draft (`current.md`) and archived snapshots (`archive/*.md`)
- **Update:** Via `aoc-stm add|edit|archive|history|read|handoff|resume` (`aoc-stm` defaults to current draft)
- **Lifecycle:** Keep STM entries as project diary artifacts; promote durable architecture decisions to `aoc-mem`
- **Behavior:** `aoc-stm` prints current draft; `aoc-stm handoff` archives current draft and prints the handoff snapshot; `aoc-stm resume` (or `aoc-stm read`) loads archived resume context

### Per-Tab Isolation

Each Zellij tab = One isolated project context:

- **Root Anchoring:** All panes start in the project root
- **Context Injection:** Layouts automatically receive `__AOC_PROJECT_ROOT__`, `__AOC_AGENT_ID__`
- **Memory Boundaries:** Each project has its own `.aoc/` directory

**Agent Pane Names Include Root Tag:**
```
Agent [my-project]     # Shows which project context is active
```

### Standard Agent Workflow

When you start working in AOC:

1. **Orient:** `aoc-mem read` - Ingest past decisions and preferences
2. **Context:** `.aoc/context.md` - Automatically provides current project map
3. **Plan:** `tm add "..."` (alias: `aoc-task add`) - Track your work plan
4. **Intake (optional):** Use Taskmaster commands (`tm add/edit`, alias: `aoc-task add/edit`) to capture/shape implementation tasks from your PRD.
5. **Spec:** `aoc-task prd show <id>` - Read linked PRD before implementation
6. **Execute:** Edit files, run commands, collaborate with AI agent
7. **Handoff Prep:** Use `aoc-stm add` / `aoc-stm edit` to write a concise `.aoc/stm/current.md` draft
8. **Load STM Context:** `aoc-stm resume` - Load archived handoff context into terminal/agent transcript (`aoc-stm` remains draft-only)
9. **Update:** Mark tasks done in Taskmaster TUI
10. **Record:** `aoc-mem add "..."` - Document significant decisions

---

## 📋 Requirements

**Core Dependencies:**
- `zellij` >= 0.43.1 (`>= 0.44.0` recommended for native pane/tab JSON inventory and explicit floating-pane control)
- `yazi` (file manager)
- `fzf` (fuzzy finder)
- `micro` (editor - auto-installed)

**Optional but Recommended:**
- `tmux` (for agent scrollback)
- `git` (for RLM and git integration)

**Platform Support:**
- ✅ Linux (X11/Wayland)
- ✅ macOS
- ✅ WSL2 (Windows)

**See [Installation Guide](./docs/installation.md) for distro-specific commands.**

---

## 📊 Comparison with Alternatives

| Feature | AOC | tmux+vim | Standard IDE |
|---------|-----|----------|--------------|
| **Per-project AI context** | ✅ Auto | ❌ Manual | ❌ None |
| **Alternative agent CLIs** | ✅ BYO wrappers (`AOC_AGENT_CMD`) | ⚠️ Complex | ❌ None |
| **Terminal-native** | ✅ Yes | ✅ Yes | ❌ No |
| **Task integration** | ✅ Built-in | ❌ None | ⚠️ Plugin |
| **File manager** | ✅ Yazi | ⚠️ Optional | ✅ Yes |
| **Context persistence** | ✅ 3-layer | ❌ None | ⚠️ Limited |
| **Scrollback reliability** | ✅ tmux-backed | ✅ Yes | ✅ Yes |

---

## 🛠️ Configuration

### Quick Overrides

```bash
# Use a different layout
AOC_ZELLIJ_CONFIG=~/.config/zellij/my-layout.kdl aoc

# Disable auto-fullscreen
AOC_FULLSCREEN=0 aoc

# Override agent for one session
AOC_AGENT_ID=pi aoc
```

### Theme Quickstart

```bash
# Open interactive TUI selector
aoc-theme tui

# Create a custom global theme template
aoc-theme init --name review-mode

# Install curated preset packs
aoc-theme presets install --all

# Apply immediately in active Zellij session
aoc-theme apply --name review-mode

# Persist as default theme for future launches
aoc-theme set-default --name review-mode

# Re-sync AOC-wide theme artifacts if needed
aoc-theme sync
```

From `Alt+C` (`aoc-control`), open **Settings -> Tools** for nested controls (RTK, PI installer, PI compaction presets with selectable context-window math, Agent Browser + Search, Vercel CLI tool/skill sync, MoreMotion flows). See [Installation Guide](./docs/installation.md) for setup steps and [Configuration Guide](./docs/configuration.md) for path/env details.

### Environment Variables

AOC supports extensive customization via environment variables:

**RTK Routing:** `AOC_RTK_BYPASS`, `AOC_RTK_MODE`, `AOC_RTK_CONFIG`, `AOC_RTK_BINARY`, `AOC_RTK_ULTRA_COMPACT`, `AOC_RTK_ROUTE_NON_TTY_STDIN` (new `aoc-init` projects default RTK mode to `on`; existing explicit `off` is preserved)

RTK keeps agent context healthier by condensing noisy command output while preserving safety via fail-open fallback to native command execution.

**Command Overrides:** `AOC_AGENT_CMD`, `AOC_TASKMASTER_CMD`, `AOC_TASKMASTER_ROOT`, `AOC_FILETREE_CMD`

**Agent Installer Overrides:** `AOC_PI_INSTALL_CMD`, `AOC_PI_UPDATE_CMD`

**PI Runtime Tuning:** `AOC_PI_BIN`, `AOC_PI_LOW_TOKEN_MODE`, `AOC_PI_LOW_TOKEN_PROMPT`, `AOC_PI_APPEND_SYSTEM_PROMPT`, `AOC_PI_HANDSHAKE_MODE`

**Clock:** `AOC_CLOCK_INTERVAL`, `AOC_CLOCK_TIME_FORMAT`, `AOC_CLOCK_BACKEND`, `AOC_CLOCK_FONT`

**See [Configuration Guide](./docs/configuration.md) for complete reference.**

---

## 🤝 Contributing

We welcome contributions! See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

**Quick setup for contributors:**

```bash
# 1. Install dependencies (see docs/installation.md)
# 2. Clone and install
git clone <repo>
cd agent-ops-cockpit
./install.sh

# 3. Build Rust components
cargo build --workspace

# 4. Run tests
./scripts/lint.sh
```

**Areas where help is welcome:**
- Multi-shell support (fish, zsh, PowerShell)
- Custom layout contributions
- Documentation improvements
- Windows native support (when Zellij supports it)

**See [ROADMAP.md](./ROADMAP.md) for future direction.**

---

## 📚 Documentation

**Recommended reading order:**
1. [Installation Guide](./docs/installation.md) — install, Alt+C setup, optional web research
2. [Configuration Guide](./docs/configuration.md) — env vars, paths, Alt+C-managed config surfaces
3. [Agents](./docs/agents.md) — canonical `.pi/**` runtime contract and migration checklist
4. [Deprecations and removals](./docs/deprecations.md) — what was removed/simplified and current support boundary

| Document | Description |
|----------|-------------|
| [Installation Guide](./docs/installation.md) | Platform-specific setup instructions + post-install contract |
| [Control Pane Guide](./docs/control-pane.md) | Alt+C operator flows, background jobs, logs, and tool setup |
| [Agents](./docs/agents.md) | PI-first runtime contract (`.pi/**`), prompts, extensions, migration checks |
| [Configuration Guide](./docs/configuration.md) | Environment variables and customization |
| [Deprecations and removals](./docs/deprecations.md) | PI-only transition summary and legacy-path behavior |
| [Agent Skills](./docs/skills.md) | Skill format and sync workflow |
| [Agent Extensibility](./docs/agent-extensibility.md) | Bring-your-own agent CLI wrappers with PI-first core |
| [MoreMotion](./docs/moremotion.md) | Optional Remotion integration |
| [Custom Layouts](./docs/layouts.md) | Creating "AOC Modes" |
| [Mission Control](./docs/mission-control.md) | Architecture and event schema |
| [Implementation Status Checklist](./docs/implementation-status-checklist.md) | Current shipped/partial/deferred overview across AOC surfaces |
| [Mind Runtime Validation](./docs/mind-runtime-validation.md) | Live smoke check + one-command hardening suite for Mind rollout confidence |
| [Yazi Mermaid Preview](./docs/yazi-mermaid-preview.md) | Mermaid preview architecture, cache behavior, fallback UX, and current limitations |
| [PI-only Rollout Checklist](./docs/pi-only-rollout-checklist.md) | Release closeout + post-release verification |
| [CHANGELOG.md](./CHANGELOG.md) | Release history |
| [ROADMAP.md](./ROADMAP.md) | Future development plans |

---

## 🆘 Troubleshooting

**Quick diagnostics:**

```bash
aoc-doctor          # Check all dependencies
tm list             # Verify task controls work
tm --tm-root ~/dev/other-project tag list  # Cross-project Taskmaster targeting
aoc-task list       # Canonical command
aoc-mem read        # Check memory system
aoc-rtk status      # Check RTK routing state
aoc-rtk git status  # Manual RTK routing smoke check
bash scripts/pi/validate-mind-runtime-live.sh       # Fast Mind runtime smoke check
bash scripts/pi/validate-mind-runtime-hardening.sh  # Broader Mind rollout/hardening suite
```

**Common issues:**

| Issue | Solution |
|-------|----------|
| Missing previews | Run `aoc-doctor`; install `file`, `resvg` (use Cargo on Ubuntu/Debian), and a supported Yazi image backend (`ueberzugpp` when available, or Kitty/kitten) |
| Blank task list | Run `aoc-task init` then `tm list` |
| RLM not working | Build with `cargo build --release -p aoc-cli` |
| TeX preview errors | Install `tectonic` via Cargo |

**See [Installation Guide - Troubleshooting](./docs/installation.md#troubleshooting) for detailed solutions.**

---

## 🔍 Keywords

terminal workspace, AI agent IDE, zellij layout, terminal multiplexer, ai-assisted development, pi agent, rust tui, yazi file manager, task management, context isolation, distributed cognitive architecture

---

## 🙏 Acknowledgments

AOC builds on excellent open-source tools:
- [Zellij](https://zellij.dev) - Terminal workspace
- [Yazi](https://yazi-rs.github.io) - File manager
- [micro](https://micro-editor.github.io) - Modern terminal editor
- [tmux](https://github.com/tmux/tmux) - Terminal multiplexer (agent scrollback)
- [fzf](https://github.com/junegunn/fzf) - Fuzzy finder
- [chafa](https://hpjansson.org/chafa) - Image-to-text converter

---

## 📄 License

Apache License 2.0 - see [LICENSE](./LICENSE) file for details.

---

**Ready to transform your AI-assisted development workflow?**

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

[⬆️ Back to Top](#aoc---terminal-first-ai-workspace)
