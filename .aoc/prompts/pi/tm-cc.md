---
description: Activate cross-project Taskmaster control mode (tm-cc) with explicit root targeting.
---
Load and follow the `tm-cc` skill.

Session rules:
1. Confirm the target Taskmaster root first (`--tm-root <path>` or `AOC_TASKMASTER_ROOT`).
2. If root is missing, ask for it before running cross-project task commands.
3. Use `tm`, `aoc-task`, or `aoc-taskmaster` with explicit root targeting.
4. Support full CRUD when requested (create/read/update/delete).
5. Keep handshake behavior unchanged; this mode is opt-in per task.

Guardrails:
- Never edit `.taskmaster/tasks/tasks.json` directly.
- Before destructive operations, restate target root + tag and ask for confirmation.
