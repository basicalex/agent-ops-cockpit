---
name: taskmaster-ops
description: Manage tasks with aoc-task and the Taskmaster TUI.
---

## CLI basics
- `aoc-task list`
- `aoc-task add "<task>"`
- `aoc-task status <id> <status>`
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
