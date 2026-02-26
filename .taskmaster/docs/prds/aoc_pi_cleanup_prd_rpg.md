# AOC PI-First Cleanup PRD (RPG)

## Problem Statement
AOC currently mixes canonical agent assets between `.aoc/` and `.pi/`.
That split is manageable in multi-agent mode, but for PI-only operation it creates unnecessary projection/sync complexity and drift risk (for example missing `.pi/settings.json` in newly initialized repos).

We need a PI-first architecture where PI runtime assets are canonical in `.pi/`, while `.aoc/` remains the AOC control plane for context, memory, STM, RTK, and workspace orchestration.

## Target Users
- Maintainers operating AOC in PI-only mode.
- Contributors onboarding to AOC repositories where PI is the sole agent runtime.
- Existing AOC repo owners migrating from mixed canonical locations.

## Success Metrics
- 100% of fresh `aoc-init` PI-first repos contain required `.pi` baseline (`settings.json`, prompts, skills) without manual edits.
- 0 ambiguous ownership for PI assets (single canonical location per asset type).
- Existing repos converge after one `aoc-init` run with no destructive overwrites.
- Documentation and behavior remain consistent (no stale non-PI guidance).

---

## Capability Tree

### Capability: PI-First Canonical Ownership
Define and enforce single-source ownership for PI runtime assets.

#### Feature: PI canonical asset map
- **Description**: Establish canonical storage for prompts, skills, settings, and extensions under `.pi/`.
- **Inputs**: Current repo layout, AOC conventions.
- **Outputs**: Explicit ownership contract.
- **Behavior**: Resolve each asset class to one canonical path and remove duplicates.

#### Feature: AOC control-plane boundary
- **Description**: Keep `.aoc/` focused on control/state primitives.
- **Inputs**: Existing `.aoc` responsibilities.
- **Outputs**: Narrowed `.aoc` contract.
- **Behavior**: Retain context/memory/stm/rtk/layouts in `.aoc`, avoid PI runtime duplication there.

### Capability: Deterministic Initialization
Ensure `aoc-init` always creates a valid PI-first baseline.

#### Feature: PI baseline seeding
- **Description**: Seed `.pi/settings.json`, `.pi/prompts/`, `.pi/skills/` deterministically.
- **Inputs**: Repo root, template sources.
- **Outputs**: Idempotent PI-ready tree.
- **Behavior**: Create missing files/dirs, preserve user-modified existing files.

#### Feature: Alias hygiene
- **Description**: Prevent accidental duplicate prompt/skill aliases.
- **Inputs**: Seed manifest/templates.
- **Outputs**: Single intended command/skill surface.
- **Behavior**: Seed only approved names (for this scope: canonical `tm-cc` only).

### Capability: Migration Safety for Existing Repos
Provide convergent upgrades for already-initialized repos.

#### Feature: Non-destructive migration
- **Description**: Repair missing required PI files and remove known stale duplicates.
- **Inputs**: Existing repo state.
- **Outputs**: Converged PI-first layout.
- **Behavior**: Add missing, preserve existing canonical files, cleanup deprecated duplicates with explicit rules.

#### Feature: Compatibility window
- **Description**: Keep limited transition compatibility while docs and scripts converge.
- **Inputs**: Legacy paths/references.
- **Outputs**: Predictable migration timeline.
- **Behavior**: Warn during transition and remove compatibility paths at defined milestone.

### Capability: Validation + Documentation
Make PI-first behavior testable and understandable.

#### Feature: E2E init validation
- **Description**: Test fresh + existing repo flows for PI-first guarantees.
- **Inputs**: test harness, `aoc-init` behavior.
- **Outputs**: reproducible CI/smoke checks.
- **Behavior**: Assert required files and idempotency.

#### Feature: PI-first documentation set
- **Description**: Update docs and release notes to PI-only model.
- **Inputs**: README/docs/AGENTS guidance.
- **Outputs**: Consistent operator guidance.
- **Behavior**: Remove non-PI ambiguity and publish migration checklist.

---

## Repository Structure (Target)

```text
project-root/
├── .aoc/
│   ├── context.md
│   ├── memory.md
│   ├── stm/
│   ├── rtk.toml
│   └── layouts/
├── .pi/
│   ├── settings.json
│   ├── prompts/
│   ├── skills/
│   └── extensions/
└── .taskmaster/
    └── docs/prds/
```

## Module Definitions

### Module: `bin/aoc-init`
- **Maps to capability**: Deterministic Initialization + Migration Safety
- **Responsibility**: Seed/repair PI-first project structure idempotently.
- **Exports/commands**: `aoc-init`

### Module: `.pi` asset contract
- **Maps to capability**: PI-First Canonical Ownership
- **Responsibility**: Canonical runtime surface for PI prompts/skills/settings/extensions.

### Module: docs (`README.md`, `docs/*`, `AGENTS.md`)
- **Maps to capability**: Validation + Documentation
- **Responsibility**: PI-first operator and contributor guidance.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- PI-first ownership contract (what is canonical in `.aoc` vs `.pi`).

### Init Layer (Phase 1)
- `aoc-init` deterministic seeding: depends on [PI-first ownership contract].

### Canonicalization Layer (Phase 2)
- Prompt/skill canonical relocation and alias cleanup: depends on [PI-first ownership contract, Init Layer].

### Migration Layer (Phase 3)
- Existing repo compatibility and auto-migration: depends on [Init Layer, Canonicalization Layer].

### Validation & Release Layer (Phase 4)
- E2E tests + docs/release checklist: depends on [Migration Layer].

---

## Development Phases

### Phase 0: Contract Definition
- Task [121]
- Exit: Approved target ownership map and migration guardrails.

### Phase 1: Init Determinism
- Task [122]
- Exit: Fresh `aoc-init` always creates PI baseline (`.pi/settings.json`, prompts, skills).

### Phase 2: Canonical PI Assets
- Tasks [123], [124], [125]
- Exit: PI assets canonicalized; non-PI scaffolding deprecated/removed from active flow.

### Phase 3: Existing Repo Convergence
- Task [126]
- Exit: Existing repos migrate safely via `aoc-init` with no destructive overwrite.

### Phase 4: Quality and Rollout
- Tasks [127], [128]
- Exit: Automated validation green, docs/release notes published.

---

## Risks and Mitigations
- **Risk**: Breaking existing repos with custom local PI files.
  - **Mitigation**: preserve-if-exists semantics; explicit migration rules; no destructive overwrite.
- **Risk**: Hidden references to legacy agent paths.
  - **Mitigation**: repo-wide audits + compatibility warnings during migration window.
- **Risk**: Docs drift from actual init behavior.
  - **Mitigation**: tie docs updates to validation checklist before release.

## Contract Resolution (Task 121)
- Phase 0 ownership/migration contract approved in: `.taskmaster/docs/prds/aoc_pi_cleanup_contract.md`.
- `.aoc/prompts/pi/*` and `.aoc/skills/**` are explicitly treated as **legacy compatibility sources only** during the compatibility window.
- Canonical PI runtime ownership is `.pi/**`; implementation tasks [122]-[126] execute migration safely, and task [128] closes the compatibility window.
