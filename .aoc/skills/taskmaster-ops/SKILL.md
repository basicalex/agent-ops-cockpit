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
- `tm tag current`
- `tm tag prd show`
- `aoc-task tag list`
- `aoc-task tag current`
- `aoc-task tag prd show --tag <tag>`
- `aoc-task tag prd init --tag <tag>`
- `aoc-task tag prd set <path> --tag <tag>`
- `aoc-task tag prd clear --tag <tag>`
- `aoc-task prd show <id>`
- `aoc-task prd init <id>`
- `aoc-task prd set <id> <path>`
- `aoc-task prd clear <id>`

## TUI usage
- Use the Taskmaster pane to toggle status, expand subtasks, and switch tags.
- Keep tasks small and actionable.

## Guardrail
- Never edit `.taskmaster/tasks/tasks.json` directly.
- Use tag-level PRD defaults with task-level overrides; do not add PRDs to subtasks.
