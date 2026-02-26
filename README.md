# AOC - Terminal-First AI Workspace

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](./CHANGELOG.md)
[![Zellij](https://img.shields.io/badge/zellij-%E2%89%A50.43.1-green.svg)](https://zellij.dev)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)
[![Build](https://github.com/basicalex/agent-ops-cockpit/actions/workflows/ci.yml/badge.svg)](https://github.com/basicalex/agent-ops-cockpit/actions/workflows/ci.yml)

> **The Distributed Cognitive Architecture for AI-Assisted Development**

AOC (Agent Ops Cockpit) is a terminal-native workspace that brings **context-aware AI agents**, **integrated task management**, and **project memory** together in a unified Zellij layout.

[📸 Screenshot](./docs/assets/aoc1.png) | [📖 Quick Start](#quick-start) | [🔧 Installation](#installation) | [📚 Documentation](#documentation)

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
│   📁 Yazi        │   🤖 Agent       │   📅 Widget               │
│   File Manager   │   (pi)           │   Calendar/Media          │
│                  │   PI-only mode   │   Clock                  │
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

## 🚀 Quick Start

### One-Line Install

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo basicalex/agent-ops-cockpit
```

The bootstrap entrypoint downloads the latest release installer (portable Rust binary when available), falls back to source install when needed, and installs AOC to user-local paths.

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

### Next Steps

Choose your path:

| 🚀 **Start Coding** | 🤖 **Configure Agents** | 🔧 **Customize** |
|---------------------|------------------------|------------------|
| `aoc` in any project dir | `aoc-agent --set` | `aoc.minimal` |
| Open files in Yazi | Choose PI Agent (npm, recommended) | Create your own "AOC Modes" |
| Press `Enter` to edit with `micro` | PI-first core + optional BYO wrappers | [Custom Layouts Guide](./docs/layouts.md) |

---

## 🎯 Key Features

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

### 3. RLM Skill - Large Codebase Analysis

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
| `Enter` | Smart open (dir enter, text edit, media default app) |
| `e` | Edit with `$EDITOR` (micro) |
| `g s` | Edit short-term memory |
| `g S` | Jump to `.aoc/stm` |
| `W` | Set widget media path |
| `p` | Send to floating preview |

**Preview support:** Images, PDFs, SVGs, LaTeX, code with syntax highlighting

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
- `zellij` >= 0.43.1
- `yazi` (file manager)
- `fzf` (fuzzy finder)
- `micro` (editor - auto-installed)

**Optional but Recommended:**
- `tmux` (for agent scrollback)
- `chafa` + `ffmpeg` (for media widgets)
- `git` (for RLM and git integration)

**Platform Support:**
- ✅ Linux (X11/Wayland)
- ✅ macOS
- ✅ WSL2 (Windows)

**See [Installation Guide](./docs/installation.md) for distro-specific commands.**

---

## 🎮 Widget Controls

The top-right widget pane supports media, calendar, and clock:

**Media & Gallery:**
- `m` - Media mode
- `g` - Gallery mode (from `~/Pictures/Zellij`)
- `p` - Set media path
- `Enter` - Toggle clean view (media/gallery)
- `h/j/k/l` or arrows - Offset in clean view (`0` reset)
- `G` - Save current settings as global defaults
- `R` - Reset settings (media -> clear project asset + global defaults, gallery -> built-in defaults)

Media path + media render settings are stored per project. Gallery settings are global and used when no project media is set.

**Rendering Controls:**
- `s` - Cycle ASCII styles
- `C` - Cycle color depth
- `D` - Cycle dither mode
- `w` - Cycle detail
- `r` - Edit font ratio
- `+/-` - Adjust render size

**Configure defaults via environment variables:**
`AOC_WIDGET_SYMBOLS`, `AOC_WIDGET_COLORS`, `AOC_WIDGET_DITHER`, `AOC_WIDGET_SCALE`

**See [Configuration Guide](./docs/configuration.md) for all options.**

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

From `Alt+C` (`aoc-control`), open **Settings -> Agent installers** to view install status and run install/update actions for supported CLIs.

### Environment Variables

AOC supports extensive customization via environment variables:

**RTK Routing:** `AOC_RTK_BYPASS`, `AOC_RTK_MODE`, `AOC_RTK_CONFIG`, `AOC_RTK_BINARY`, `AOC_RTK_ULTRA_COMPACT`, `AOC_RTK_ROUTE_NON_TTY_STDIN` (new `aoc-init` projects default RTK mode to `on`; existing explicit `off` is preserved)

RTK keeps agent context healthier by condensing noisy command output while preserving safety via fail-open fallback to native command execution.

**Command Overrides:** `AOC_AGENT_CMD`, `AOC_TASKMASTER_CMD`, `AOC_TASKMASTER_ROOT`, `AOC_FILETREE_CMD`

**Agent Installer Overrides:** `AOC_PI_INSTALL_CMD`, `AOC_PI_UPDATE_CMD`

**PI Runtime Tuning:** `AOC_PI_BIN`, `AOC_PI_LOW_TOKEN_MODE`, `AOC_PI_LOW_TOKEN_PROMPT`, `AOC_PI_APPEND_SYSTEM_PROMPT`, `AOC_PI_HANDSHAKE_MODE`

**Widget:** `AOC_WIDGET_SYMBOLS`, `AOC_WIDGET_COLORS`, `AOC_WIDGET_DITHER`, `AOC_WIDGET_SCALE`

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

| Document | Description |
|----------|-------------|
| [Installation Guide](./docs/installation.md) | Platform-specific setup instructions |
| [Configuration Guide](./docs/configuration.md) | Environment variables and customization |
| [Agent Skills](./docs/skills.md) | Skill format and sync workflow |
| [Agents](./docs/agents.md) | PI prompts and PI-only runtime reference |
| [Agent Extensibility](./docs/agent-extensibility.md) | Bring-your-own agent CLI wrappers with PI-first core |
| [MoreMotion](./docs/moremotion.md) | Optional Remotion integration |
| [Custom Layouts](./docs/layouts.md) | Creating "AOC Modes" |
| [Mission Control](./docs/mission-control.md) | Architecture and event schema |
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
```

**Common issues:**

| Issue | Solution |
|-------|----------|
| Missing previews | Install `chafa`, `poppler-utils`, `librsvg2-bin` |
| Blank task list | Run `aoc-task init` then `tm list` |
| Widget not rendering | Run `aoc-doctor`, check `ffmpeg` and `chafa` |
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
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash -s -- --repo basicalex/agent-ops-cockpit
```

[⬆️ Back to Top](#aoc---terminal-first-ai-workspace)
