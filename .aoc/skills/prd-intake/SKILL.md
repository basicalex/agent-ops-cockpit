---
name: prd-intake
description: Orchestrate project PRD into tasks via sub-agents and aoc-task.
---

## Goal
Turn a project PRD (usually `.taskmaster/docs/prd.md`) into a high-quality task graph using agent refinement and `aoc-task add/edit`.

## Workflow
1. Verify PRD exists and has actionable sections (goals, stories, requirements, acceptance criteria).
2. Analyze the PRD directly for coverage, duplicates, and scope boundaries.
3. In OpenCode, fan out sub-agents by domain/epic for refinement (parallel where possible).
4. Resolve active tag when not explicitly provided: `aoc-task tag current` (or `tm tag current`).
5. Apply task changes with `aoc-task` primitives:
   - Link tag PRD default: `aoc-task tag prd set <path> --tag <tag>`
   - Create: `aoc-task add "<title>" --desc "..." --details "..." --test-strategy "..." --priority <...> --tag <tag>`
   - Update: `aoc-task edit <id> --title "..." --desc "..." --details "..." --test-strategy "..." --tag <tag>`
   - Link task PRD override when needed: `aoc-task prd set <id> <path> --tag <tag>`
   - Manage dependencies/status/subtasks with `aoc-task edit --depends`, `aoc-task status`, `aoc-task sub ...`
6. For explicit replace requests, remove target-tag tasks first using `aoc-task rm <id> --tag <tag>` then recreate.
7. Run follow-up alignment:
   - `task-breakdown` for large tasks
   - `tag-align` for tags/status/dependencies
   - `prd-align` to ensure details and testStrategy match the PRD

## Guardrails
- Never edit `.taskmaster/tasks/tasks.json` directly.
- Use a review checkpoint before destructive replacement.
- Use tag-level PRD defaults with optional task-level overrides; subtasks must not include PRD links.
- Keep generated tasks actionable and testable.

## Notes
- Default parse source is `.taskmaster/docs/prd.md` (fallback `.taskmaster/docs/prd.txt`).
- Final persistence should go through `aoc-task add/edit` in this workflow.
