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
│   ├── aoc-clock
│   ├── aoc-clock-set
│   ├── aoc-control
│   ├── aoc-control-toggle
│   ├── aoc-doctor
│   ├── aoc-hub
│   ├── aoc-init
│   ├── aoc-launch
│   ├── aoc-layout
│   ├── aoc-mem
│   ├── aoc-mission-control
│   ├── aoc-mission-control-tab
│   ├── aoc-mission-control-toggle
│   ├── aoc-momo
│   ├── aoc-new-tab
│   ├── aoc-open-explorer
│   ├── aoc-open-file
│   ├── aoc-pane-rename
│   ├── aoc-pi
│   ├── aoc-preview
│   ├── aoc-preview-set
│   ├── aoc-preview-toggle
│   ├── aoc-rlm
│   ├── aoc-rtk
│   ├── aoc-rtk-proxy
│   ├── aoc-skill
│   ├── aoc-stm
│   ├── aoc-stm-read
│   ├── aoc-sys
│   ├── aoc-task
│   ├── aoc-taskmaster
│   ├── aoc-test
│   ├── aoc-theme
│   ├── aoc-tm
│   ├── aoc-uninstall
│   ├── aoc-utils.sh
│   ├── aoc-widget
│   ├── aoc-widget-set
│   ├── aoc-yazi
│   ├── aoc-zellij-resize
│   ├── rlm
│   ├── tm
│   └── tm-editor
├── CHANGELOG.md
├── cmd
│   ├── aoc-agent-wrap-go
│   ├── aoc-hub
│   └── aoc-taskmaster
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
│   ├── Cargo.lock
│   └── Cargo.toml
├── docs
│   ├── agent-extensibility.md
│   ├── agents.md
│   ├── assets
│   ├── configuration.md
│   ├── deprecations.md
│   ├── feature-upgrade-collection-key.md
│   ├── insight-compaction-ingest.md
│   ├── insight-subagent-orchestration.md
│   ├── insight-t3-alignment.md
│   ├── installation.md
│   ├── layouts.md
│   ├── mind-v2-architecture-cutover-checklist.md
│   ├── mission-control.md
│   ├── mission-control-ops.md
│   ├── moremotion.md
│   ├── omo-regression-checklist.md
│   ├── pi-only-rollout-checklist.md
│   ├── pulse-ipc-protocol.md
│   ├── pulse-vnext-rollout.md
│   ├── research
│   └── skills.md
├── install
│   └── bootstrap.sh
├── install.sh
├── lib
│   └── aoc_cleanup
├── LICENSE
├── micro
│   └── bindings.json
├── package.json
├── plugins
├── pnpm-lock.yaml
├── README.md
├── ROADMAP.md
├── scripts
│   ├── lint.sh
│   ├── opencode
│   ├── pi
│   └── smoke.sh
├── SECURITY.md
├── shellcheck-v0.10.0
│   ├── LICENSE.txt
│   ├── README.txt
│   └── shellcheck
├── SUPPORT.md
├── walkthrough.md
├── yazi
│   ├── init.lua
│   ├── keymap.toml
│   ├── plugins
│   ├── preview.sh
│   ├── theme.toml
│   └── yazi.toml
└── zellij
    ├── aoc.config.kdl.template
    └── layouts

39 directories, 100 files
```

## README Headings
# AOC - Terminal-First AI Workspace
## ✨ Why AOC?
### The Problem with AI Development Today
### The AOC Solution
## 🚀 Quick Start
### One-Line Install
### Verify Installation
### Next Steps
## 🎯 Key Features
### 1. PI-Only Agent Runtime
# Set/select runtime
# Or launch directly
### 2. Native Taskmaster TUI
### 3. RLM Skill - Large Codebase Analysis
# Measure repository scale
# Search across codebase
# Process in manageable chunks
### 4. Agent Skills
# Sync PI skills
### 5. Yazi File Manager Integration
### 6. Custom Layouts ("AOC Modes")
# Try the minimal layout
# See available layout shortcuts in this project
# (type and press Tab for completion)
# Set as default
## 🏗️ Distributed Cognitive Architecture
### The Three Layers
#### 1. Context (`.aoc/context.md`) - The "Project Map"
#### 2. Memory (`.aoc/memory.md`) - The "Logbook"
#### 3. Tasks (`.taskmaster/tasks/tasks.json`) - The "Todo List"
#### 4. Task PRDs (`.taskmaster/docs/prds/`) - The "Spec Layer"
#### 5. Short-Term Memory (`.aoc/stm/`) - The "Handoff Buffer"
### Per-Tab Isolation
### Standard Agent Workflow
## 📋 Requirements
## 🎮 Widget Controls
## 📊 Comparison with Alternatives
## 🛠️ Configuration
### Quick Overrides
# Use a different layout

## Current Task Tag
```
mind
```

## Active Workstreams (Tags)
```
aoc/pi_cleanup (9)
deprecation (10)
master (45)
mermaid (1)
mind (41)
mission-control (17)
omo (10)
pulse-hub-spoke (8)
rtk (5)
safety (9)
session-overseer (1)
sub-agents (13)
```

## Task PRD Location
- Directory: .taskmaster/docs/prds
- Resolve tag PRD default with: aoc-task tag prd show --tag <tag>
- Resolve task PRD override with: aoc-task prd show <id> --tag <tag>
- Effective precedence: task PRD override -> tag PRD default
