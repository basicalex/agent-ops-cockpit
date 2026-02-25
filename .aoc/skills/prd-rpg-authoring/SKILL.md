---
name: prd-rpg-authoring
description: Author canonical RPG-format PRDs from the shared template and link them via Taskmaster aocPrd.
---

## Goal
Produce a complete PRD in the RPG format using the canonical template at:
- `.taskmaster/templates/example_prd_rpg.txt`

Output PRDs under:
- `.taskmaster/docs/prds/*_prd_rpg.md`

## Workflow
1. Resolve task context:
   - Active tag: `tm tag current` (or `aoc-task tag current`)
   - Optional task scope: `aoc-task show <id> --tag <tag>`
2. Read and follow the canonical RPG template exactly:
   - `.taskmaster/templates/example_prd_rpg.txt`
3. Gather project context before drafting:
   - `.aoc/context.md`
   - `aoc-mem read` and targeted `aoc-mem search "<topic>"`
   - Relevant code/docs for the target scope
4. Draft the PRD with full RPG sections (problem, functional/structural decomposition, dependency graph, phases, test strategy, architecture, risks).
5. Save to `.taskmaster/docs/prds/<name>_prd_rpg.md`.
6. Link PRD:
   - Tag default: `aoc-task tag prd set <path> --tag <tag>`
   - Task override (only when intentional): `aoc-task prd set <id> <path> --tag <tag>`
7. Validate before finishing:
   - No circular dependencies in module/task graph
   - Foundation layer has no dependencies
   - Each phase has entry/exit criteria and test strategy
   - Subtasks do not carry PRD links

## Guardrails
- Use the template file as the source of truth; do not maintain a duplicate embedded template.
- Never edit `.taskmaster/tasks/tasks.json` directly.
- Use tag-level PRD defaults unless a task-level override is explicitly required.
- Keep dependencies explicit and topologically buildable.
