# AOC Architecture & Agent Guidelines

This file defines the always-on rules for agents in this repo. Procedural playbooks live in AOC skills.

## Always-on rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- `.aoc/memory.md` is append-only; use `aoc-mem` to read/search/add. Do not edit the file directly.
- `.taskmaster/tasks/tasks.json` is task state; use the Taskmaster TUI or `aoc-task` commands. Do not edit the file directly.
- Task PRDs are linked per task (not subtask) via `aocPrd`; keep PRD documents in `.taskmaster/docs/prds/` and resolve via `aoc-task prd` commands.
- Record major decisions and constraints in memory (`aoc-mem add "..."`).

## Core files
- `.aoc/context.md`: auto-generated project snapshot.
- `.aoc/memory.md`: persistent decision log.
- `.taskmaster/tasks/tasks.json`: dynamic task queue.
- `.taskmaster/docs/prds/`: task-level PRD documents linked from tasks.

## Skills (load when needed)
- `aoc-workflow`: standard project workflow.
- `aoc-init-ops`: initialize or repair AOC files.
- `memory-ops`: read/search/add to memory.
- `taskmaster-ops`: manage tasks and tags.
- `rlm-analysis`: large codebase analysis flow.
- `prd-dev`: draft the Taskmaster PRD.
- `prd-align`: align tasks with the PRD.
- `tag-align`: normalize task tags and dependencies.
- `task-breakdown`: expand tasks into subtasks.
- `task-checker`: verify implementation vs. testStrategy.
- `release-notes`: draft changelog and release notes.
- `skill-creator`: create or update AOC skills.
