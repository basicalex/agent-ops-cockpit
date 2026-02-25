# AOC Architecture & Agent Guidelines

This file defines the always-on rules for agents in this repo. Procedural playbooks live in AOC skills.

## Always-on rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- **DO NOT manually read these files** - use the Bash tool to run CLI commands instead (see below).
- Run AOC commands via Bash tool - do NOT use Read tool for `.aoc/memory.md`, `.aoc/stm/current.md`, or `.taskmaster/tasks/tasks.json`.
- RTK routing is default-on for new AOC projects (`.aoc/rtk.toml` mode=`on`); existing explicit mode=`off` is preserved.
- RTK exists to improve context health: allowlisted noisy commands are condensed for better signal density, with fail-open native fallback.

## Low-Token Default Mode
- Keep responses concise by default; do not print full files or raw logs unless explicitly requested.
- Start with the smallest viable step; use narrow, path-scoped searches before broad scans.
- Read files in bounded chunks and avoid rereading unchanged large files.
- Summarize command/tool output with actionable lines only (key errors, next actions).
- Run targeted checks/tests first; run full-suite commands only when required.
- If targeted inspection fails, escalate scope gradually and state why.
- Use fresh sessions after major milestones or context drift to reduce replay overhead.
- For narrow diagnostics/Q&A, use at most 3 tool calls before first answer; ask before broader escalation.
- Do not open/read image binaries unless the user explicitly asks to view/open one now.
- Use one narrow diagnostic path first; avoid retry spray with variant commands unless first attempt fails.

## AOC CLI Commands (run via Bash tool - NOT Read tool)
These commands are in PATH and work without loading any skill:

**Memory:**
- `aoc-mem read` - read persistent memory
- `aoc-mem add "decision"` - record architectural decision

**Short-Term Memory (STM):**
- `aoc-stm` - print current draft (shortcut for `aoc-stm read-current`)
- `aoc-stm handoff` - archive current draft and print handoff snapshot
- `aoc-stm resume` - print archived handoff snapshot (latest by default)
- `aoc-stm read` - read latest archived snapshot
- `aoc-stm archive` - archive current draft
- `aoc-stm add "note"` - add to current draft
- `aoc-stm edit` - edit current draft in editor

**Tasks:**
- `tm list` - list tasks (alias for `aoc-task`)
- `tm add "Task name"` - add new task
- `tm tag current` - print effective active tag
- `tm tag prd show` - show PRD linked to active tag
- `aoc-task tag prd show --tag <tag>` - show PRD linked to a specific tag
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
- `.taskmaster/docs/prds/`: PRD documents linked to tags and tasks.
- Tag PRD defaults are linked via tag `aocPrd`; resolve with `aoc-task tag prd show --tag <tag>`.
- Task PRD overrides are linked via task `aocPrd`; resolve with `aoc-task prd show <id> --tag <tag>`.
- Effective precedence is task PRD override -> tag PRD default.
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
