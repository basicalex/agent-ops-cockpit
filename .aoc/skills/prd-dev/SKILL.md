---
name: prd-dev
description: Draft and refine Taskmaster PRDs linked at tag/task scope via aocPrd.
---

## Output
Create or update a PRD document under `.taskmaster/docs/prds/` and ensure it is linked at the appropriate scope:
- tag default via `aoc-task tag prd set|init`
- task override via `aoc-task prd set|init`

## Process
1. Identify the target task ID and active tag (`tm tag current` / `aoc-task tag current`).
2. Resolve or create links via:
   - `aoc-task tag prd show|init|set --tag <tag>` for tag defaults
   - `aoc-task prd show|init|set <id> --tag <tag>` for task overrides
3. Ask clarifying questions to resolve gaps.
4. Draft sections: problem, goals, non-goals, user stories, requirements.
5. Include acceptance criteria and dependencies.
6. Cover risks, performance, security, UX, and maintainability.
7. Define a test strategy and success metrics.

## Guardrails
- Subtasks must not include PRD links.
- Use task-level PRD only when it intentionally overrides the tag default.
- Effective PRD precedence is task override -> tag default.

## Format
Plain text with clear headings and bullet lists.
