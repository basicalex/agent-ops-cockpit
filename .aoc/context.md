# Project Context Snapshot

## Repository
- Name: agent-ops-cockpit
- Root: /home/ceii/dev/agent-ops-cockpit
- Git branch: main

## Key Files
- README.md
- package.json
- pnpm-lock.yaml

## Project Structure (tree -L 2)
```
/home/ceii/dev/agent-ops-cockpit
├── AGENTS.md
├── AOC.md
├── bin
│   ├── aoc
│   ├── aoc-agent
│   ├── aoc-agent-install
│   ├── aoc-agent-run
│   ├── aoc-agent-wrap
│   ├── aoc-align
│   ├── aoc-cleanup
│   ├── aoc-cleanup-core.py
│   ├── aoc-clock
│   ├── aoc-clock-set
│   ├── aoc-control
│   ├── aoc-control-toggle
│   ├── aoc-doctor
│   ├── aoc-hub
│   ├── aoc-init
│   ├── aoc-insight
│   ├── aoc-launch
│   ├── aoc-layout
│   ├── aoc-map
│   ├── aoc-mem
│   ├── aoc-mind-toggle
│   ├── aoc-mission-control
│   ├── aoc-mission-control-tab
│   ├── aoc-mission-control-toggle
│   ├── aoc-momo
│   ├── aoc-new-tab
│   ├── aoc-open-explorer
│   ├── aoc-open-file
│   ├── aoc-pane-evidence
│   ├── aoc-pane-rename
│   ├── aoc-pi
│   ├── aoc-pulse-pane
│   ├── aoc-refresh-layout-state
│   ├── aoc-rlm
│   ├── aoc-rtk
│   ├── aoc-rtk-proxy
│   ├── aoc-search
│   ├── aoc-skill
│   ├── aoc-stm
│   ├── aoc-stm-read
│   ├── aoc-subagent-supervision
│   ├── aoc-subagent-supervision-toggle
│   ├── aoc-sys
│   ├── aoc-tab-group
│   ├── aoc-tab-metadata
│   ├── aoc-task
│   ├── aoc-taskmaster
│   ├── aoc-test
│   ├── aoc-theme
│   ├── aoc-tm
│   ├── aoc-uninstall
│   ├── aoc-utils.sh
│   ├── aoc-web-smoke
│   ├── aoc-yazi
│   ├── aoc-yazi-mermaid
│   ├── aoc-yazi-mermaid-open
│   ├── aoc-yazi-preview
│   ├── aoc-zellij-plugin
│   ├── aoc-zellij-resize
│   ├── aoc-zellij.sh
│   ├── rlm
│   ├── tm
│   └── tm-editor
├── CHANGELOG.md
├── CODE_OF_CONDUCT.md
├── config
│   ├── btop.conf
│   ├── codex-tmux.conf
│   └── opencode
├── CONTRIBUTING.md
├── crates
│   ├── aoc-agent-wrap-rs
│   ├── aoc-cli
│   ├── aoc-control
│   ├── aoc-core
│   ├── aoc-hub-rs
│   ├── aoc-installer
│   ├── aoc-mind
│   ├── aoc-mission-control
│   ├── aoc-opencode-adapter
│   ├── aoc-pi-adapter
│   ├── aoc-segment-routing
│   ├── aoc-storage
│   ├── aoc-task-attribution
│   ├── aoc-taskmaster
│   ├── aoc-yazi-mermaid
│   ├── Cargo.lock
│   └── Cargo.toml
├── docs
│   ├── agent-extensibility.md
│   ├── agents.md
│   ├── aoc-map.md
│   ├── ARCHITECTURE.md
│   ├── assets
│   ├── configuration.md
│   ├── control-pane.md
│   ├── deprecations.md
│   ├── feature-upgrade-collection-key.md
│   ├── implementation-status-checklist.md
│   ├── insight-compaction-ingest.md
│   ├── insight-subagent-orchestration.md
│   ├── insight-t3-alignment.md
│   ├── installation.md
│   ├── layouts.md
│   ├── mind-background-reliability-checklist.md
│   ├── mind-runtime-validation.md
│   ├── mind-v2-architecture-cutover-checklist.md
│   ├── mission-control.md
│   ├── mission-control-ops.md
│   ├── moremotion.md
│   ├── omo-regression-checklist.md
│   ├── phase2-module-plan.md
│   ├── pi-only-rollout-checklist.md
│   ├── presets.md
│   ├── pulse-ipc-protocol.md
│   ├── pulse-vnext-rollout.md
│   ├── research
│   ├── security
│   ├── skills.md
│   ├── subagent-runtime.md
│   ├── yazi-mermaid-preview.md
│   └── zellij-top-bar.md
├── install
│   └── bootstrap.sh
├── install.sh
├── lib
│   └── aoc_cleanup
├── LICENSE
├── micro
│   └── bindings.json
├── package.json
├── pnpm-lock.yaml
├── README.md
├── ROADMAP.md
├── scripts
│   ├── lint.sh
│   ├── opencode
│   ├── pi
│   ├── smoke.sh
│   ├── verify-mind-runtime-safety.sh
│   └── zellij
├── SECURITY.md
├── shellcheck-v0.10.0
│   ├── LICENSE.txt
│   ├── README.txt
│   └── shellcheck
├── SUPPORT.md
├── vendor
│   └── zjstatus-aoc
├── walkthrough.md
├── yazi
│   ├── init.lua
│   ├── keymap.toml
│   ├── plugins
│   ├── theme.toml
│   └── yazi.toml
└── zellij
    ├── aoc.config.kdl.template
    ├── layouts
    └── plugins

40 directories, 124 files
```

## README Headings
# AOC - Terminal-First AI Workspace
## ✨ Why AOC?
### The Problem with AI Development Today
### The AOC Solution
## 🚦 Start Here
## 🚀 Quick Start
### One-Line Install
### Verify Installation
### After Install
### Next Steps
## 🎯 Core Workflow
## 🎯 Key Features
### Core Features
### 1. PI-Only Agent Runtime
# Set/select runtime
# Or launch directly
### 2. Native Taskmaster TUI
### 3. Insight CLI - Mind-backed retrieval and provenance
# Retrieve bounded citations/snippets across project canon + session exports
# Inspect provenance / traversal graph for an artifact, task, or file
# Check runtime health
### 4. RLM Skill - Large Codebase Analysis
# Measure repository scale
# Search across codebase
# Process in manageable chunks
### 4. Agent Skills
# Sync PI skills
### 5. Yazi File Manager Integration
## 🕹️ Alt+C Control Pane
### Agent Browser + Search
### Optional Integrations and Advanced Workflows
### 6. Custom Layouts ("AOC Modes")
# See available layout shortcuts in this project
# (type and press Tab for completion)
# Set as default
## 🏗️ Distributed Cognitive Architecture
### The Three Layers
#### 1. Context (`.aoc/context.md`) - The "Project Map"
#### 2. Memory (`.aoc/memory.md`) - The "Logbook"
#### 3. Tasks (`.taskmaster/tasks/tasks.json`) - The "Todo List"

## Current Task Tag
```
env-protec
```

## Active Workstreams (Tags)
```
184 (5)
aoc-presets (1)
aoc/pi_cleanup (9)
deprecation (10)
detached-orchestration (3)
env-protec (7)
master (46)
mermaid (1)
mind (50)
mission-control (17)
omo (10)
pi-compaction-ui (1)
pi-terminal-ops (1)
pulse-hub-spoke (8)
pulse-tab-overview (1)
rtk (5)
safety (9)
session-overseer (0)
sub-agents (6)
subagent-ux (1)
```

## Task PRD Location
- Directory: .taskmaster/docs/prds
- Resolve tag PRD default with: aoc-task tag prd show --tag <tag>
- Resolve task PRD override with: aoc-task prd show <id> --tag <tag>
- Effective precedence: task PRD override -> tag PRD default
