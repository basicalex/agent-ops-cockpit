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
4. Plan: add or refine tasks with `aoc-task add "<task>"` and set status.
5. Execute changes and run tests.
6. Update tasks and record decisions: `aoc-task status <id> done`, `aoc-mem add "<decision>"`.

## Guardrails
- Do not edit `.aoc/memory.md` directly.
- Do not edit `.taskmaster/tasks/tasks.json` directly.
