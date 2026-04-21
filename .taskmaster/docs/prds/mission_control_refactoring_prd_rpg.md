# Mission Control Architecture Refactoring PRD (RPG)

> Alignment note: this PRD addresses the structural drift identified in the Mind/Mission Control architecture discussion. The goal is to extract the monolithic `aoc-mission-control/src/main.rs` into clean, bounded crates aligned with the canonical product split: Mission Control = global fleet, Mind = project knowledge, Pulse = per-tab telemetry.

## Problem Statement

`aoc-mission-control/src/main.rs` is **15,189 lines** of single-file Rust code that bundles three distinct product surfaces:

1. **Mission Control** (intended: global fleet/session/agent orchestration)
2. **Mind** (intended: project-local knowledge retrieval and curation — but the `aoc-mind` crate exists separately, is ~3,684 lines, and is effectively unused because mission-control re-implements all Mind rendering inline)
3. **Pulse pane** (intended: minimal per-tab telemetry strip, but rendered from the same binary)

This causes:
- Unmaintainable monolith (15k lines, no modularity)
- Dead code risk (`aoc-mind` crate is never consumed)
- Confused product boundaries (one binary pretending to be multiple surfaces)
- No clean path to add the Mind floating pane (task 182) without further entanglement

## Target Users

- AOC developers maintaining the codebase
- Operators using Mission Control for fleet oversight
- Operators needing project Mind view without the global fleet overhead
- Future subagent/extension consumers of Mind as a library, not a binary

## Success Metrics

1. `aoc-mission-control/src/main.rs` reduced to ≤2,000 lines (~87% reduction)
2. `aoc-mind` crate is consumed by at least one consumer (mission-control or new binary)
3. Each product surface (Mission Control, Mind, Pulse pane) maps to a clean crate boundary
4. New code is testable at the unit level (no 15k-line main.rs)
5. All existing functionality preserved (tests pass, behavior unchanged)
6. `aoc-mind` provides a clean, stable API for Mind operations (query, render, ingest)

---

## Capability Tree

### Capability: Mind Runtime Library
Expose the existing Mind runtime machinery as a reusable library.

#### Feature: Mind query API
- **Description**: Project-scoped Mind artifact queries (canon, handshake, exports, observer events)
- **Inputs**: project root, query type, filters
- **Outputs**: typed Mind structures (artifacts, events, summaries)
- **Behavior**: Wraps existing Mind SQLite queries from main.rs into a typed API

#### Feature: Mind render API
- **Description**: Render Mind state into Ratatui lines for any TUI consumer
- **Inputs**: Mind query results, theme, layout constraints
- **Outputs**: `Vec<ratatui::text::Line<'static>>`
- **Behavior**: Moved from main.rs render functions into aoc-mind render module

#### Feature: Mind ingest API
- **Description**: Process Pulse feed events into Mind artifacts
- **Inputs**: Pulse observer feed events
- **Outputs**: updated Mind store state
- **Behavior**: Extracted from main.rs Mind event processing pipeline

### Capability: Mission Control Fleet Surface
Global fleet/session/agent oversight as the canonical use case.

#### Feature: Fleet overview
- **Description**: Cross-session agent status, health, and activity dashboard
- **Inputs**: Pulse hub subscriptions, local zellij state
- **Outputs**: sorted, filtered overview of all active agents
- **Behavior**: Existing overview logic, extracted from main.rs

#### Feature: Overseer view
- **Description**: Deep inspection of individual agent sessions (tasks, diffs, timeline)
- **Inputs**: session-scoped Pulse subscriptions
- **Outputs**: agent-specific status panels with task/diff/health details
- **Behavior**: Existing overseer logic, extracted from main.rs

#### Feature: Fleet-wide operations
- **Description**: Cross-agent commands (focus, stop, delegation)
- **Inputs**: operator actions, Pulse command routing
- **Outputs**: command dispatch to target agents
- **Behavior**: Existing command routing, extracted from main.rs

### Capability: Mission Control Mind View
Project-scoped Mind view accessible from within Mission Control.

#### Feature: Mind artifact viewer
- **Description**: Display Mind artifacts (handshake, canon, exports, observer events) for the active project
- **Inputs**: project root, Mind query API results
- **Outputs**: formatted Mind view
- **Behavior**: Calls aoc-mind query + render APIs instead of re-implementing

#### Feature: Mind project context resolution
- **Description**: Auto-detect the project being worked on in Mission Control context
- **Inputs**: active session, tab, pane metadata
- **Outputs**: resolved project root path
- **Behavior**: Uses aoc-core project resolution utilities

### Capability: Pulse Pane Surface
Minimal per-tab telemetry strip.

#### Feature: Pulse telemetry strip
- **Description**: Compact status line showing agent state, tasks, and health
- **Inputs**: Pulse hub subscription for current session
- **Outputs**: minimal status rendering
- **Behavior**: Extracted from main.rs, becomes the canonical pulse-pane binary

### Capability: Clean Crate Boundaries
Establish the structural decomposition.

#### Feature: aoc-core utilities
- **Description**: Shared types used by all crates (Pulse IPC, session types, Mind contracts)
- **Inputs**: none (foundational)
- **Outputs**: reusable types and protocol definitions
- **Behavior**: Already mostly exists; ensure clean separation

#### Feature: aoc-mind library
- **Description**: Mind runtime, queries, and rendering as a reusable crate
- **Inputs**: aoc-core types, SQLite storage
- **Outputs**: Mind API for consumers
- **Behavior**: Expanded from existing crate to include query/render logic from main.rs

#### Feature: aoc-mission-control binary
- **Description**: Fleet/overseer binary that consumes aoc-mind for the Mind view
- **Inputs**: aoc-core, aoc-mind, Pulse hub
- **Outputs**: Mission Control TUI
- **Behavior**: Reduced to fleet-focused views; delegates Mind to aoc-mind

---

## Repository Structure

```
crates/
├── aoc-core/                      # Foundation: shared types, Pulse IPC, session types
│   └── src/
│       ├── lib.rs
│       ├── pulse_ipc.rs           # Pulse protocol types
│       ├── mind_contracts.rs       # Mind domain types
│       ├── session_overseer.rs     # Overseer types
│       └── ...
│
├── aoc-mind/                      # Mind library: runtime + query + render
│   ├── src/
│   │   ├── lib.rs                 # Public API surface
│   │   ├── observer_runtime.rs    # Existing observer runtime
│   │   ├── t3_runtime.rs          # Existing T3 runtime
│   │   ├── reflector_runtime.rs   # Existing reflector runtime
│   │   ├── query.rs               # Mind SQLite queries (moved from main.rs)
│   │   └── render.rs              # Mind TUI rendering (moved from main.rs)
│   └── Cargo.toml
│
├── aoc-mission-control/           # Fleet/overseer binary
│   ├── src/
│   │   ├── main.rs                # App + CLI (~2k lines after split)
│   │   ├── fleet.rs               # Fleet overview (extracted)
│   │   ├── overseer.rs            # Overseer view (extracted)
│   │   └── shared.rs              # Theme, config, CLI parsing
│   └── Cargo.toml
│
└── aoc-pulse-pane/                # Pulse strip binary (new)
    ├── src/
    │   └── main.rs                # Minimal panel rendering
    └── Cargo.toml
```

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **aoc-core**: Shared types already exist; audit and stabilize the public API

### Mind Extraction Layer (Phase 1)
- **aoc-mind query API**: Depends on [aoc-core]
- **aoc-mind render API**: Depends on [aoc-mind query API, ratatui]
- **mission-control uses aoc-mind**: Depends on [aoc-mind render API]

### Mission Control Split Layer (Phase 2)
- **fleet.rs extraction**: Depends on [Phase 1]
- **overseer.rs extraction**: Depends on [Phase 1]
- **mission-control modularization**: Depends on [fleet.rs, overseer.rs]

### Pulse Pane (Phase 3)
- **aoc-pulse-pane binary**: Depends on [aoc-core pulse_ipc]

---

## Implementation Roadmap

### Phase 0: Foundation & Audit
**Goal**: Audit current state, plan extraction boundaries

**Entry Criteria**: Repository builds cleanly

**Tasks**:
- [ ] Document all types currently in main.rs that should move
  - Acceptance: Extraction manifest with line ranges and dependencies
- [ ] Document all functions in main.rs by category (Mind, Fleet, Overseer, Pulse, Utility)
  - Acceptance: Categorized function inventory

**Exit Criteria**: Complete extraction map

---

### Phase 1: Extract aoc-mind Query + Render
**Goal**: Move all Mind-related code from main.rs into aoc-mind crate

**Entry Criteria**: Phase 0 complete

**Tasks**:
- [ ] Create `aoc-mind/src/query.rs` with Mind store artifacts and search
  - Acceptance: Compiles, tests pass
- [ ] Create `aoc-mind/src/render.rs` with all Mind rendering functions
  - Acceptance: Compiles with ratatui dependency
- [ ] Add `ratatui` dependency to aoc-mind Cargo.toml
  - Acceptance: Crate builds
- [ ] Add `aoc-mind` dependency to aoc-mission-control
  - Acceptance: Cargo workspace builds
- [ ] Wire Mission Control Mind view to use aoc-mind APIs
  - Acceptance: Existing tests pass
- [ ] Delete duplicated Mind code from main.rs
  - Acceptance: main.rs reduced significantly, all tests pass

**Exit Criteria**: aoc-mind is imported and used by aoc-mission-control; Mind rendering tests pass

---

### Phase 2: Split Mission Control Binary
**Goal**: Extract fleet/overseer logic from main.rs into separate modules

**Entry Criteria**: Phase 1 complete

**Tasks**:
- [ ] Extract fleet overview logic → fleet.rs module
- [ ] Extract overseer logic → overseer.rs module
- [ ] Split main.rs into modular App structure

**Exit Criteria**: Clean crate structure; testable units

---

### Phase 3: Pulse Pane Binary
**Goal**: Extract minimal pulse-pane rendering as standalone

**Entry Criteria**: Phase 2 complete

**Tasks**:
- [ ] Create aoc-pulse-pane crate

**Exit Criteria**: Pulse pane binary compiles and works independently

---

## Test Strategy

### Test Pyramid

```
        /\
       /E2E \    ← Integration tests: full app boot, Pulse hub, render cycles
      /------\
     /Module/   ← Module-level: fleet view, overseer view, Mind query/render
    /--------\
   /  Unit   \  ← Unit tests: render functions, query APIs, data transforms
  /----------\
```

### Critical Test Scenarios

#### Mind Query API
- Valid project root returns artifacts
- Empty/missing project root handled gracefully
- Large artifact lists paginate correctly

#### Mind Render API
- Observer feed events render correctly
- Empty feed shows placeholder
- Output matches existing main.rs output (golden testing)

---

## Architecture

## System Components

| Component | Role | Current | Target |
|-----------|------|---------|--------|
| Pulse IPC types | Protocol | aoc-core | aoc-core (stable) |
| Mind contracts | Domain types | aoc-core | aoc-core (stable) |
| Mind store queries | SQLite ops | main.rs | aoc-mind/src/query.rs |
| Mind render | TUI rendering | main.rs | aoc-mind/src/render.rs |
| Fleet overview | Agent dashboard | main.rs | aoc-mission-control/src/fleet.rs |
| Overseer view | Session inspection | main.rs | aoc-mission-control/src/overseer.rs |
| Pulse pane | Telemetry strip | main.rs | aoc-pulse-pane/ |

---

## Risks

**Risk**: Extracting Mind code breaks existing behavior
- **Impact**: High | **Likelihood**: Medium
- **Mitigation**: Golden file tests before extraction; compare output line-by-line

**Risk**: 15k line extraction takes longer than expected
- **Impact**: Medium | **Likelihood**: Medium
- **Mitigation**: Phased approach with incremental validation

---

## Appendix

## References
- `docs/mission-control.md` — Current architecture
- `docs/ARCHITECTURE.md` — Canonical architecture reference
- `crates/aoc-mission-control/src/main.rs` — 15,189 lines target
- `crates/aoc-mind/src/` — 3,684 lines existing (unused)

## Open Questions
1. Should aoc-mind expose sync or async API? → Start sync (current behavior)
2. Should render API support both project-scoped and session-scoped? → Yes, preserve both
