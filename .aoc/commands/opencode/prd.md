---
description: Orchestrate PRD intake into task generation with sub-agents
agent: build
---
Orchestrate project PRD intake and task generation.

Execution flow:
1. Resolve PRD source path:
   - Use an explicit path from the `/prd` arguments when provided.
   - Otherwise default to `.taskmaster/docs/prd.md` (fallback `.taskmaster/docs/prd.txt`).
2. Read and analyze the PRD directly.
3. Split the PRD into 2-4 domain slices and dispatch sub-agents in parallel to propose task specs.
4. Merge refinements and normalize into final task specs:
   - clear titles
   - actionable descriptions
   - dependencies and test strategy hints
   - dedupe by intent
5. Create/update tasks through `aoc-task` primitives:
   - Resolve active tag first with `aoc-task tag current` (or `tm tag current`) when tag input is omitted.
   - Create new tasks with `aoc-task add "<title>" --desc "..." --details "..." --test-strategy "..." --priority <high|medium|low> --tag <tag>`
   - Update existing tasks with `aoc-task edit <id> --title "..." --desc "..." --details "..." --test-strategy "..." --tag <tag>`
   - Link each created/updated task to the project PRD with `aoc-task prd set <id> <prd-path> --tag <tag>`
   - Manage status/dependencies with `aoc-task status`, `aoc-task edit --depends`, and `aoc-task sub ...`
6. For explicit "replace" requests: remove target-tag tasks with `aoc-task rm <id> --tag <tag>` first, then recreate via `add/edit`.
7. Verify results with `aoc-task list --tag <tag>` and spot-check key tasks with `aoc-task show <id> --tag <tag>`.

Rules:
- Do not edit `.taskmaster/tasks/tasks.json` directly.
- Keep PRD links task-level only.
- For replace mode, require explicit tag targeting.
- Report assumptions and what was inferred from the PRD.

When complete, provide:
- PRD source used
- mode used (`append` or `replace`)
- tag targeted
- tasks created/skipped
- any recommended follow-up (`task-breakdown`, `prd-align`, `tag-align`)
