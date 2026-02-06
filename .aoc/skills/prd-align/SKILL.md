---
name: prd-align
description: Align tasks and implementation with the PRD.
---

## Steps
1. Identify the target task and read its linked PRD (`aoc-task prd show <id>`).
2. Review task details for coverage, dependencies, and acceptance criteria.
3. Update task details and `testStrategy` to match the linked PRD.
4. Flag gaps or drift and propose updates.

## Guardrails
- PRD links are task-level only; subtasks must not include PRD links.
