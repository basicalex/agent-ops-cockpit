---
name: prd-dev
description: Draft and refine a task-level Taskmaster PRD linked via aocPrd.
---

## Output
Create or update a task-level PRD document under `.taskmaster/docs/prds/` and ensure it is linked on the task with `aoc-task prd set` (or created with `aoc-task prd init`).

## Process
1. Identify the target task ID and active tag.
2. Resolve or create the linked PRD path via `aoc-task prd show/init/set`.
3. Ask clarifying questions to resolve gaps.
4. Draft sections: problem, goals, non-goals, user stories, requirements.
5. Include acceptance criteria and dependencies.
6. Cover risks, performance, security, UX, and maintainability.
7. Define a test strategy and success metrics.

## Guardrails
- PRD links are task-level only; subtasks must not include PRD links.

## Format
Plain text with clear headings and bullet lists.
