---
name: prd-align
description: Align tasks and implementation with the PRD.
---

## Steps
1. Identify the target task and active tag.
2. Read PRD context in precedence order:
   - task override: `aoc-task prd show <id> --tag <tag>`
   - tag default: `aoc-task tag prd show --tag <tag>`
3. Review task details for coverage, dependencies, and acceptance criteria.
4. Update task details and `testStrategy` to match the effective PRD.
5. Flag gaps or drift and propose updates.

## Guardrails
- Subtasks must not include PRD links.
- Use task-level PRD only when intentionally overriding tag default.
- Effective PRD precedence is task override -> tag default.
