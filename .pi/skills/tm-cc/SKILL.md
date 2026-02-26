---
name: tm-cc
description: Run Taskmaster in cross-project mode with explicit tm root targeting and full CRUD flows.
---

## When to use
- You need to inspect or edit tags/tasks in another repository.
- You want Taskmaster source switching on demand (not in handshake defaults).
- You need explicit Create/Read/Update/Delete control against a non-current project.

## Prompt aliases (PI)
- `/tm-cc` (canonical)

## Cross-project targeting
Use either:
- `--tm-root <path>` (or `--tm-source <path>`) on `tm`, `aoc-task`, or `aoc-taskmaster`
- `AOC_TASKMASTER_ROOT=<path>` environment variable

Examples:
- `tm --tm-root ~/dev/proj-a tag list`
- `tm --tm-root ~/dev/proj-b list --tag mind`
- `aoc-task --tm-root ../proj-c add "Draft migration" --tag infra`
- `aoc-taskmaster --tm-root ~/dev/proj-d`

## Full CRUD quick map
- Create: `add`, `sub add`, `tag add`, `prd init`
- Read: `list`, `show`, `tag list`, `tag current`, `prd show`
- Update: `edit`, `status`, `done/reopen`, `move`, `tag rename`, `prd set`
- Delete: `remove`, `sub remove`, `tag remove`, `prd clear`

## Guardrails
- Never edit `.taskmaster/tasks/tasks.json` directly.
- Use explicit `--tm-root` for every cross-project command.
- Before destructive operations, confirm the target root and tag.
- Keep handshake compact; invoke this workflow only when needed.
