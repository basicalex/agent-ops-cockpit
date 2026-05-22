# Spec: AOC Understand Integration and Teach Deprecation

## Problem Statement
AOC currently has two partial repository-understanding surfaces: `teach-workflow`, which is prompt-only and writes local Markdown under `.aoc/insight/`, and `aoc-map`, which is a curated graph microsite but does not analyze code. Understand-Anything provides the missing structured knowledge graph, dashboard, guided tours, explain/chat/onboard/diff/domain flows, and Pi support. AOC needs a first-class, safe wrapper around Understand-Anything and a clear deprecation path for teach without disrupting `aoc-map`.

## Target Users
- Operators onboarding to large AOC projects who need a searchable architecture graph.
- Agents that need a structured graph/query surface instead of broad ad-hoc repo scans.
- Maintainers who need AOC-controlled install/status/doctor/test behavior before deeper map/Open Design convergence.

## Success Metrics
- `aoc-understand --help`, `status`, and `doctor` work without network or installation.
- `aoc-understand install` is explicit and clones/updates Understand-Anything only when requested.
- `aoc-understand analyze/dashboard/chat/explain/onboard/domain/diff` route to UA skills or scripts with project-root safety.
- `teach-workflow` is marked deprecated/hidden and no longer positioned as the canonical learning flow.
- `aoc-map` remains intact and gains documented future hooks for UA/OD convergence.

## Capability Tree

### Capability: Understand-Anything lifecycle
- **Description**: Manage a global UA checkout for all AOC projects.
- **Inputs**: repo URL/ref/home/source options, local prerequisites.
- **Outputs**: install/status/doctor output and installed plugin source.
- **Behavior**: explicit clone/update/build; never run network install implicitly.

### Capability: Project graph operations
- **Description**: Run UA graph commands from a resolved AOC/git project root.
- **Inputs**: project path, command arguments.
- **Outputs**: `.understand-anything/*` graph files, dashboard URL, answers/explanations.
- **Behavior**: prefer installed UA skills/command scripts; preserve project-root boundaries.

### Capability: Agent skill guidance
- **Description**: Teach agents when to use `aoc-understand` rather than legacy teach.
- **Inputs**: Pi skill metadata and guidance.
- **Outputs**: `.pi/skills/aoc-understand/SKILL.md` and docs.
- **Behavior**: skill is visible as a production capability; teach remains hidden/deprecated.

### Capability: AOC Map/OD future convergence
- **Description**: Reserve integration hooks without implementing advanced convergence now.
- **Inputs**: UA graph files, OD artifact indexes, AOC Map workspace.
- **Outputs**: documented future model.
- **Behavior**: `aoc-map` stays curated/offline; future pages may link/import UA/OD artifacts.

## Repository Structure

```text
agent-ops-cockpit/
├── bin/aoc-understand                  # AOC wrapper CLI
├── .pi/skills/aoc-understand/SKILL.md  # Agent-facing guidance
├── docs/understand.md                  # Human docs
├── docs/aoc-map.md                     # Future integration note
├── docs/skills.md                      # Teach deprecation / new skill listing
├── bin/aoc-init                        # Seed/sync skill and filters
└── scripts/pi/test-aoc-understand.sh   # Smoke tests
```

## Module Definitions

### Module: `bin/aoc-understand`
- **Maps to capability**: Understand-Anything lifecycle + project graph operations
- **Responsibility**: Safe wrapper for install/status/doctor and UA operation dispatch.
- **Exports**: shell command subcommands: `install`, `status`, `doctor`, `analyze`, `dashboard`, `chat`, `explain`, `onboard`, `domain`, `diff`, `open`, `map-sync`.

### Module: `.pi/skills/aoc-understand/SKILL.md`
- **Maps to capability**: Agent skill guidance
- **Responsibility**: Tell Pi agents to use AOC wrapper first and avoid teach.

### Module: docs/tests/init updates
- **Maps to capability**: rollout and verification
- **Responsibility**: Make the integration discoverable, seeded, and smoke-tested.

## Dependency Graph

Foundation layer:
- `aoc-understand-cli`: no dependencies beyond shell, git/node/pnpm diagnostics.
- `aoc-understand-skill`: depends on CLI command surface.

Rollout layer:
- `docs`: depends on CLI and skill decisions.
- `aoc-init-seeding`: depends on skill path and production visibility policy.
- `tests`: depends on CLI, docs assumptions, and init behavior.

Future layer:
- `aoc-map-ua-sync`: depends on stable `.understand-anything/knowledge-graph.json` schema and existing `aoc-map` workspace.
- `aoc-map-od-convergence`: depends on UA sync and existing `aoc-od` artifact metadata.

## Implementation Phases

### Phase 1: Spec/task alignment
- Create this spec and a Taskmaster parent task with concrete subtasks.
- Acceptance: task links to this spec and subtasks map to implementation units.

### Phase 2: V1 wrapper and skill
- Add `bin/aoc-understand` with explicit install/status/doctor and operation dispatch.
- Add `.pi/skills/aoc-understand/SKILL.md`.
- Acceptance: help/status/doctor pass without UA installed.

### Phase 3: Rollout and teach deprecation
- Update docs and `aoc-init` seeding/filters.
- Mark `teach-workflow` deprecated and hidden.
- Acceptance: docs show `aoc-understand` as canonical; teach is not canonical.

### Phase 4: Verification
- Add and run smoke tests.
- Run `aoc-skill validate` and relevant shell checks.
- Acceptance: tests pass and no unrelated dirty files are touched.

## Test Strategy
- `bash -n bin/aoc-understand bin/aoc-init`.
- `bin/aoc-understand --help`, `status`, `doctor` without UA installed.
- Temp-project test for `map-sync` with a fixture `.understand-anything/knowledge-graph.json` and `.aoc/map` output.
- `aoc-skill validate`.

## Risks and Guardrails
- Do not pipe remote install scripts into shell; use explicit git clone/update.
- Do not silently install or run UA from `status`/`doctor`.
- Do not delete `.aoc/insight/` history automatically.
- Do not replace `aoc-map` now; preserve it as curated visual layer.
- Do not implement advanced OD/map convergence in V1.
