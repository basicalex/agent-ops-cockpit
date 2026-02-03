# AOC - Terminal-First AI Workspace

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](./CHANGELOG.md)
[![Zellij](https://img.shields.io/badge/zellij-%E2%89%A50.43.1-green.svg)](https://zellij.dev)
[![License](https://img.shields.io/badge/license-MIT-yellow.svg)](./LICENSE)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

> **The Distributed Cognitive Architecture for AI-Assisted Development**

AOC (Agent Ops Cockpit) is a terminal-native workspace that brings **context-aware AI agents**, **integrated task management**, and **project memory** together in a unified Zellij layout.

[📸 Screenshot](./docs/assets/aoc1.png) | [📖 Quick Start](#quick-start) | [🔧 Installation](#installation) | [📚 Documentation](#documentation)

---

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
│   File Manager   │   (codex/gemini/ │   Calendar/Media          │
│                  │   claude/opencode)│   Clock                   │
├──────────────────┼──────────────────┴───────────────────────────┤
│   Project Files  │   📋 Taskmaster TUI                         │
│                  │   Interactive task & subtask management     │
└──────────────────┴─────────────────────────────────────────────┘
         │                    │                    │
         └────────────────────┼────────────────────┘
                              ▼
        ┌──────────────────────────────────────────────────┐
        │         DISTRIBUTED COGNITIVE ARCHITECTURE       │
        ├──────────────────────────────────────────────────┤
        │                                                   │
        │  🗺️ Context        🧠 Memory          ✅ Tasks    │
        │  (Reactive)       (Persistent)       (Dynamic)   │
        │                                                   │
        │  .aoc/context.md  .aoc/memory.md     tasks.json  │
        │  Auto-updated     Append-only        Real-time   │
        │  File tree +      Architectural      Status &    │
        │  README snapshot  decisions          priorities  │
        │                                                   │
        └──────────────────────────────────────────────────┘
```

**Result:** Your AI agents maintain context across sessions, remember your preferences, and track work items—all automatically.

---

## 🚀 Quick Start

### One-Line Install

```bash
./install.sh && aoc-init && aoc
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
| `aoc` in any project dir | `aoc-agent --set` | `aoc-layout --set minimal` |
| Open files in Yazi | Switch between Codex, Gemini, Claude, OpenCode | Create your own "AOC Modes" |
| Press `Enter` to edit with `micro` | Each agent gets isolated context | [Custom Layouts Guide](./docs/layouts.md) |

---

## 🎯 Key Features

### 1. Multi-Agent Support

Seamlessly work with multiple AI agents, each maintaining isolated project context:

```bash
# Switch agents interactively
aoc-agent --set

# Or launch specific agents directly
aoc-codex-tab    # Open tab with Codex
aoc-gemini       # Open tab with Gemini
aoc-cc           # Open tab with Claude Code
aoc-oc           # Open tab with OpenCode
```

**All agents get:**
- Persistent project memory (`.aoc/memory.md`)
- Real-time context updates (`.aoc/context.md`)
- Task integration (Taskmaster TUI)
- tmux-backed scrollback for reliability

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

### 4. Yazi File Manager Integration

Keyboard-driven file management with rich previews:

| Key | Action |
|-----|--------|
| `Enter` | Open file + auto-expand pane |
| `e` | Edit with `$EDITOR` (micro) |
| `y` | Set widget media path |
| `p` | Send to floating preview |
| `S` | Star directory (re-anchor panes) |

**Preview support:** Images, PDFs, SVGs, LaTeX, code with syntax highlighting

### 5. Custom Layouts ("AOC Modes")

Create specialized layouts for different workflows:

```bash
# Try the minimal layout
aoc-new-tab --layout minimal

# Set as default
aoc-layout --set minimal
```

**Included layouts:**
- `aoc` (default) - Full cockpit with all features
- `minimal` - Streamlined for focused work

**Create your own** with context injection placeholders (`__AOC_PROJECT_ROOT__`, `__AOC_TAB_NAME__`, `__AOC_AGENT_ID__`). See [Custom Layouts Guide](./docs/layouts.md).

---

## 🏗️ Distributed Cognitive Architecture

AOC's architecture solves the fundamental problem of **context management in AI-assisted development**:

### The Three Layers

#### 1. Context (`.aoc/context.md`) - The "Project Map"
- **Role:** Reactive, auto-generated snapshot
- **Content:** File tree, README summary, project structure
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
- **Update:** Via Taskmaster TUI or `aoc task` CLI
- **Agent Usage:** Track work, update status, create new items

### Per-Tab Isolation

Each Zellij tab = One isolated project context:

- **Root Anchoring:** All panes start in the project root
- **Star Command:** `aoc-star /path/to/project` to re-anchor all panes
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
3. **Plan:** `aoc task add "..."` - Track your work plan
4. **Execute:** Edit files, run commands, collaborate with AI agent
5. **Update:** Mark tasks done in Taskmaster TUI
6. **Record:** `aoc-mem add "..."` - Document significant decisions

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
- `Enter` - Toggle clean view (in gallery)

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
| **Multi-agent support** | ✅ Native | ⚠️ Complex | ❌ None |
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
AOC_AGENT_ID=gemini aoc
```

### Environment Variables

AOC supports extensive customization via environment variables:

**Command Overrides:** `AOC_AGENT_CMD`, `AOC_CODEX_CMD`, `AOC_TASKMASTER_CMD`, `AOC_FILETREE_CMD`

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
| [Custom Layouts](./docs/layouts.md) | Creating "AOC Modes" |
| [Mission Control](./docs/mission-control.md) | Architecture and event schema |
| [CHANGELOG.md](./CHANGELOG.md) | Release history |
| [ROADMAP.md](./ROADMAP.md) | Future development plans |

---

## 🆘 Troubleshooting

**Quick diagnostics:**

```bash
aoc-doctor          # Check all dependencies
aoc task list       # Verify taskmaster works
aoc-mem read        # Check memory system
```

**Common issues:**

| Issue | Solution |
|-------|----------|
| Missing previews | Install `chafa`, `poppler-utils`, `librsvg2-bin` |
| Blank task list | Run `aoc task list` or install `task-master` npm CLI |
| Widget not rendering | Run `aoc-doctor`, check `ffmpeg` and `chafa` |
| RLM not working | Build with `cargo build --release -p aoc-cli` |
| TeX preview errors | Install `tectonic` via Cargo |

**See [Installation Guide - Troubleshooting](./docs/installation.md#troubleshooting) for detailed solutions.**

---

## 🔍 Keywords

terminal workspace, AI agent IDE, zellij layout, terminal multiplexer, ai-assisted development, codex cli, gemini cli, claude cli, opencode, rust tui, yazi file manager, task management, context isolation, distributed cognitive architecture

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

MIT License - see [LICENSE](./LICENSE) file for details.

---

**Ready to transform your AI-assisted development workflow?**

```bash
./install.sh && aoc-init && aoc
```

[⬆️ Back to Top](#aoc---terminal-first-ai-workspace)
