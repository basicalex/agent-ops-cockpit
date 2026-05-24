# Spec: PRD-to-Spec Naming Refactor

## Metadata
- Task ID: 207
- Tag: env-protec
- Kind: implementation/refactor spec
- Legacy compatibility: keep `aocPrd`, `prd` commands, and `.taskmaster/docs/prds/` valid.

## Problem
AOC uses "PRD" as the generic linked planning document name, but linked documents now include architecture, implementation, recovery, rollout, and operational specs. Public language should use the broader term "Spec" without breaking existing projects.

## Goals
- Prefer `spec` in user-facing CLI help, docs, generated AGENTS guidance, and TUI labels.
- Add `spec` command surfaces while preserving `prd` as aliases.
- Default newly initialized linked documents to `.taskmaster/docs/specs/`.
- Keep existing `aocPrd` metadata and `.taskmaster/docs/prds/` links working.
- Provide `spec-rpg-authoring` while retaining `prd-rpg-authoring` as a legacy alias skill.

## Non-Goals
- No schema migration from `aocPrd` to `aocSpec` in this phase.
- No bulk move of existing documents from `docs/prds` to `docs/specs`.
- No removal of PRD commands or legacy paths.

## Functional Decomposition
1. CLI aliases: expose `task spec ...` and `task tag spec ...`, with legacy `prd` aliases.
2. Storage defaults: create `.taskmaster/docs/specs/` and use it for generated tag/task specs.
3. UI/docs language: replace generic PRD labels with linked spec wording.
4. Skills: add `spec-rpg-authoring`; keep `prd-rpg-authoring` legacy-compatible.
5. Verification: compile and smoke-test both `spec` and legacy `prd` paths.

## Acceptance Criteria
- [x] `aoc-cli task --help` shows `spec`.
- [x] `aoc-cli task tag --help` shows `spec`.
- [x] `task tag spec init` writes under `.taskmaster/docs/specs/`.
- [x] `task tag prd show` still resolves the same link.
- [x] TUI labels say Spec instead of PRD.
- [x] Generated AGENTS/aoc-init guidance prefers spec and documents legacy compatibility.

## Test Strategy
- `cargo check -p aoc-cli -p aoc-taskmaster --manifest-path crates/Cargo.toml`
- `cargo run -q --manifest-path crates/Cargo.toml -p aoc-cli -- task --help`
- `cargo run -q --manifest-path crates/Cargo.toml -p aoc-cli -- task tag --help`
- Temp-project smoke test for `task tag spec init` and legacy `task tag prd show`.
