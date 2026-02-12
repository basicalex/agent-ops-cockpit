---
name: aoc-workflow
description: Standard AOC workflow using context, memory, and tasks.
---

## When to use
Use this when you start a new task or need to re-orient inside a project.

## Steps
1. If AOC files are missing or stale, run `aoc-init` from the project root.
2. Read memory: `aoc-mem read` and `aoc-mem search "<topic>"` as needed.
3. Review tasks: `aoc-task list` or the Taskmaster TUI.
4. For the active task, check PRD linkage with `aoc-task prd show <id>`; if missing, create/link via `aoc-task prd init <id>` or `aoc-task prd set <id> <path>`.
5. Plan: add or refine tasks with `aoc-task add "<task>"` and set status.
6. Execute changes and run tests.
7. If context gets tight, capture state (`aoc-stm add/edit`; in OpenCode you can run `/stm`), archive it (`aoc-stm archive`), and run `aoc-stm` to load latest diary context into the transcript.
8. Update tasks and record decisions: `aoc-task status <id> done`, `aoc-mem add "<decision>"`.

## Guardrails
- Do not edit `.aoc/memory.md` directly.
- Do not keep long-term decisions in `.aoc/stm/current.md`; promote durable decisions to `aoc-mem`.
- Do not edit `.taskmaster/tasks/tasks.json` directly.
- Do not add PRD links to subtasks; PRDs are task-level only (`aocPrd`).
