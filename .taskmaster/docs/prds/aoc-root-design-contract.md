# PRD: AOC root DESIGN.md design contract

## Summary
Make `DESIGN.md` a first-class AOC project contract seeded at the project root by `aoc-init`. The file acts as the canonical visual/product/design architecture source for agents, while subsystem-specific design files such as `hyperframes/docs/DESIGN.md` extend or specialize it.

## Problem
AOC currently has strong operational contracts (`AGENTS.md`, `.aoc/context.md`, Taskmaster PRDs, STM/memory), but design guidance is scattered across skills and subsystem docs. Agents can implement technically correct UI/media changes that drift visually because there is no canonical project design source.

## Goals
- Seed root `DESIGN.md` non-destructively in AOC projects.
- Treat root `DESIGN.md` as the authoritative visual/product design contract for product-facing changes.
- Update agent guidance so UI, docs-site, marketing, and media work consults `DESIGN.md` first.
- Keep subsystem `DESIGN.md` files as extensions, not replacements.
- Preserve existing project-authored `DESIGN.md` files.

## Non-goals
- Do not copy third-party templates verbatim without license review.
- Do not overwrite existing design docs.
- Do not require full token extraction or visual audits in this task.

## Requirements
1. `aoc-init` creates a root `DESIGN.md` only when it is missing.
2. The template is AOC-owned, agent-operational, and compatible with emerging DESIGN.md conventions.
3. `AGENTS.md` guidance tells agents to inspect root `DESIGN.md` before product-facing UI/media/design changes.
4. HyperFrames `docs/DESIGN.md` explains that it extends the root `DESIGN.md` when present.
5. Validation proves idempotency and preservation of an existing root `DESIGN.md`.

## Acceptance Criteria
- Fresh `aoc-init` creates `DESIGN.md` at the project root.
- Rerunning `aoc-init` does not alter existing `DESIGN.md`.
- HyperFrames bootstrap continues to create `hyperframes/docs/DESIGN.md` and references the root design contract.
- Shell syntax and targeted smoke tests pass.
