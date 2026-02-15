---
name: taskmaster-ops
description: Manage tasks with aoc-task, tm alias, and the Taskmaster TUI.
---

## CLI basics
- `tm list`
- `aoc-task list`
- `tm add "<task>"`
- `aoc-task add "<task>"`
- `tm status <id> <status>`
- `aoc-task status <id> <status>`
- `tm tag list`
- `aoc-task tag list`
- `aoc-task prd show <id>`
- `aoc-task prd init <id>`
- `aoc-task prd set <id> <path>`
- `aoc-task prd clear <id>`

## TUI usage
- Use the Taskmaster pane to toggle status, expand subtasks, and switch tags.
- Keep tasks small and actionable.

## Guardrail
- Never edit `.taskmaster/tasks/tasks.json` directly.
- PRD links are task-level only; do not add PRDs to subtasks.
