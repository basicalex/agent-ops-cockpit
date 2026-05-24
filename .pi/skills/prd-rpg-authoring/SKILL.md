---
name: prd-rpg-authoring
description: Legacy alias for spec-rpg-authoring. Author RPG-format specs/PRDs and link them via Taskmaster's legacy-compatible aocPrd metadata.
---

## Goal
Legacy alias for `spec-rpg-authoring`. Produce a complete Spec/PRD in RPG format using the canonical template at:
- `.taskmaster/templates/example_prd_rpg.txt`

Prefer new specs under:
- `.taskmaster/docs/specs/*_spec_rpg.md`

Legacy PRDs under `.taskmaster/docs/prds/*_prd_rpg.md` remain valid.

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
4. Draft the spec with full RPG sections (problem, functional/structural decomposition, dependency graph, phases, test strategy, architecture, risks).
5. Save to `.taskmaster/docs/specs/<name>_spec_rpg.md` for new documents unless intentionally updating a legacy PRD.
6. Link spec:
   - Tag default: `aoc-task tag spec set <path> --tag <tag>`
   - Task override (only when intentional): `aoc-task spec set <id> <path> --tag <tag>`
   - Legacy aliases remain supported: `tag prd`, `prd`
7. Validate before finishing:
   - No circular dependencies in module/task graph
   - Foundation layer has no dependencies
   - Each phase has entry/exit criteria and test strategy
   - Subtasks do not carry spec links

## Guardrails
- Use the template file as the source of truth; do not maintain a duplicate embedded template.
- Never edit `.taskmaster/tasks/tasks.json` directly.
- Use tag-level spec defaults unless a task-level override is explicitly required.
- Treat PRD as a legacy/product-specific subtype of Spec.
- Keep dependencies explicit and topologically buildable.
