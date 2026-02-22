# AOC Architecture & Agent Guidelines

This file defines the always-on rules for agents in this repo. Procedural playbooks live in AOC skills.

## Always-on rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- **DO NOT manually read these files** - use the Bash tool to run CLI commands instead (see below).
- Run AOC commands via Bash tool - do NOT use Read tool for `.aoc/memory.md`, `.aoc/stm/current.md`, or `.taskmaster/tasks/tasks.json`.
- RTK routing is default-on for new AOC projects (`.aoc/rtk.toml` mode=`on`); existing explicit mode=`off` is preserved.
- RTK exists to improve context health: allowlisted noisy commands are condensed for better signal density, with fail-open native fallback.

## AOC CLI Commands (run via Bash tool - NOT Read tool)
These commands are in PATH and work without loading any skill:

**Memory:**
- `aoc-mem read` - read persistent memory
- `aoc-mem add "decision"` - record architectural decision

**Short-Term Memory (STM):**
- `aoc-stm` - print current draft (shortcut for `aoc-stm read-current`)
- `aoc-stm read` - read latest archived snapshot
- `aoc-stm archive` - archive current draft
- `aoc-stm add "note"` - add to current draft
- `aoc-stm edit` - edit current draft in editor

**Tasks:**
- `tm list` - list tasks (alias for `aoc-task`)
- `tm add "Task name"` - add new task
- `tm` - open Taskmaster TUI

**Other:**
- `aoc-init` - initialize/repair AOC files
- `aoc-mem search "query"` - search memory
- `aoc-rtk status` - check RTK routing status
- `aoc-rtk enable|disable` - toggle RTK routing mode
- `aoc-rtk doctor` - run RTK diagnostics
- `aoc-rtk install --auto` - auto-fetch and install pinned RTK binary

## Core files
- `.aoc/context.md`: auto-generated project snapshot.
- `.aoc/rtk.toml`: project-local RTK routing policy and install contract.
- `.aoc/layouts/`: project-shared Zellij layouts for AOC (`*.kdl`).
- `.taskmaster/docs/prds/`: task-level PRD documents linked from tasks.
- Task PRDs are linked per task via `aocPrd`; resolve via `aoc-task prd show <id>`.
- Keep task PRDs in git: `.taskmaster/docs/prds/**` should always be tracked.
- Keep AOC/task state in git: `.aoc/**` and `.taskmaster/**` should not be ignored.

## Task Management
- `.taskmaster/tasks/tasks.json` is task state; use the Taskmaster TUI, `aoc-task`, or `tm` (alias for `aoc-task`). Do not edit the file directly.
- Record major decisions and constraints in memory (`aoc-mem add "..."`).

## Skills (load when needed)
- `aoc-workflow`: standard project workflow.
- `teach-workflow`: guided teach-mode scans, dives, and local insight logging.
- `rlm-analysis`: large codebase analysis flow.
- `prd-dev`: draft the Taskmaster PRD.
- `prd-intake`: parse a project PRD into initial task sets.
- `prd-align`: align tasks with the PRD.
- `tag-align`: normalize task tags and dependencies.
- `task-breakdown`: expand a task into clear subtasks.
- `task-checker`: verify implementation vs. testStrategy.
- `release-notes`: draft changelog and release notes.
- `skill-creator`: create or update AOC skills.
- `zellij-theme-ops`: create and manage global Zellij themes.

Note: `aoc-mem`, `aoc-stm`, and `tm` are basic CLI commands (see above) - no skill needed.
