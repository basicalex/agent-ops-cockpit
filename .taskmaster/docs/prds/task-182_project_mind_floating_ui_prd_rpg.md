# Task 182 — Project Mind Floating UI PRD (RPG)

> Alignment note: this PRD intentionally replaces the earlier idea of a persistent per-tab Pulse/Mind pane with a lighter **invoke-anywhere project-scoped Mind surface**. Mission Control remains the global runtime/fleet surface; the new Mind UI is the project-local knowledge/curation surface.

## Source grounding

Reviewed against:
- `docs/mind-v2-architecture-cutover-checklist.md`
- `docs/research/zellij-0.44-aoc-alignment.md`
- `docs/mission-control-ops.md`
- Zellij docs: `https://zellij.dev/documentation/creating-a-layout.html`
- Zellij docs: `https://zellij.dev/documentation/keybindings-possible-actions.html`
- Zellij docs: `https://zellij.dev/documentation/cli-actions.html`
- Zellij release/docs evidence confirms:
  - floating panes are **tab-scoped**
  - layouts support `floating_panes { ... }` and `hide_floating_panes=true`
  - runtime actions support `new-pane --floating`, `show-floating-panes --tab-id`, `hide-floating-panes --tab-id`, `toggle-floating-panes`, `toggle-pane-embed-or-floating`, `toggle-pane-pinned`, and `change-floating-pane-coordinates`
  - newer Zellij versions provide native tab/pane inventory via `list-panes --json`, `list-tabs --json`, and `current-tab-info --json`

## Problem Statement
AOC needs a first-class way to inspect and curate a project's Mind without paying the steady-state cost of a dedicated Pulse/Mind pane in every AOC tab. The original per-tab pane idea is too resource-heavy and creates UI duplication for information that is fundamentally **project-scoped**, not pane-scoped.

Today the pain points are:
- Mind state is architecturally project-level, but there is no lightweight project-local operator surface dedicated to browsing and editing it.
- Per-tab persistent panes would multiply render/update cost and make Zellij layouts noisier.
- Mission Control is the wrong primary surface for day-to-day project Mind browsing/editing because it is global, operational, and fleet-oriented.
- Existing Mind mechanics already produce exports, canon, handshake state, context packs, and backlog activity, but these remain hard to inspect as one coherent project view.
- AOC tabs are already project-aligned, but there is no canonical “open the Mind for the project this tab is working on” command.

We need an invoke-anywhere floating Mind UI that resolves the active project from the current AOC tab/agent pane, opens a project-scoped TUI only on demand, and gives operators a low-overhead place to inspect, search, and curate project Mind artifacts.

## Target Users
- **Primary operator/developer** working in AOC tabs, who wants to quickly inspect or adjust the Mind for the project associated with the current tab.
- **AOC maintainers** who need a low-overhead Mind UX that does not regress Zellij performance or blur Mission Control with project knowledge navigation.
- **Mind/runtime contributors** who need a concrete product surface for canon, handshake, exports, retrieval, and curation while preserving the delegated-vs-Mind and project-vs-global boundaries.

## Success Metrics
- Opening the project Mind from an AOC tab takes one shortcut or one slash command in the common case.
- The UI resolves the current project automatically from the active agent pane route/title in >= 95% of normal AOC tabs.
- No persistent per-tab Mind pane is required for normal operation.
- The Mind UI can inspect at minimum: current handshake snapshot, latest canon revision, recent session exports, and recent Mind/runtime health for the active project.
- The UI can perform at least one edit/curation flow on project Mind state without forcing users into raw file navigation.
- Runtime overhead remains near-zero while the Mind UI is closed; refresh/subscription behavior is active only while the surface is open.
- Mission Control remains the authoritative global runtime/fleet surface and does not become the only path for project Mind inspection.

---

## Architectural Framing
The implementation should preserve a strict product split:

1. **Mission Control**
   - global runtime/fleet supervision
   - cross-project detached jobs
   - queue depth, health, failures, operator drilldown

2. **Project Mind Floating UI**
   - current-project knowledge/curation surface
   - canon, handshake, exports, retrieval, provenance, edits
   - invoked from any AOC tab on demand

3. **Pi delegated subagent manager**
   - delegated specialist launch/inspect/rerun UX
   - not the default home for Mind internals

Core rule:

> **Mind is project-scoped and on-demand; Mission Control is global and operational.**

This means the Mind UI should not be implemented as a permanent per-tab pulse pane, and it should not inherit delegated-specialist manager semantics.

---

## Capability Tree

### Capability: Project Resolution and Invocation
Open the correct project Mind surface from any AOC tab with minimal friction.

#### Feature: Active project resolver
- **Description**: Resolve the project associated with the currently focused AOC tab.
- **Inputs**: active Zellij tab/pane metadata, agent pane route metadata, pane title, optional cwd fallback.
- **Outputs**: normalized project identity (`project_root`, route key, display label).
- **Behavior**: prefer structured route metadata from the active agent pane; fall back to pane title parsing and cwd-based heuristics only when needed.

#### Feature: Invoke-anywhere shortcut and command
- **Description**: Provide a single canonical entry point to open the project Mind UI.
- **Inputs**: operator shortcut/command, current session/tab context.
- **Outputs**: focused or newly opened Mind floating pane/TUI for the resolved project.
- **Behavior**: support one-key invocation (eg. reworked `Alt+M`) and a slash/CLI command, while keeping launch logic centralized.

#### Feature: Ambiguity fallback
- **Description**: Recover gracefully when project resolution is ambiguous.
- **Inputs**: multiple candidate projects or no confident route match.
- **Outputs**: compact project picker or explicit failure guidance.
- **Behavior**: default to the active agent pane's project, otherwise present recent/current routed projects rather than silently guessing.

### Capability: Floating Pane Lifecycle and Zellij Integration
Use newer Zellij floating-pane primitives safely and explicitly.

#### Feature: Canonical floating Mind pane launcher
- **Description**: Open the Mind UI as one named floating pane for the current tab.
- **Inputs**: resolved project identity, Zellij session/tab IDs, runtime mode, preferred pane geometry.
- **Outputs**: newly created or focused floating pane instance.
- **Behavior**: use `zellij action new-pane --floating` for creation, preserve explicit pane naming, and avoid duplicate floating Mind panes per tab when one already exists.

#### Feature: Explicit show/hide behavior
- **Description**: Control floating pane visibility without brittle toggle heuristics.
- **Inputs**: current tab ID and pane existence/visibility state.
- **Outputs**: deterministic open/focus/hide behavior.
- **Behavior**: prefer `show-floating-panes --tab-id` and `hide-floating-panes --tab-id` on supported Zellij versions; only fall back to toggle semantics when required.

#### Feature: Pane geometry and pinning policy
- **Description**: Define the default geometry and operator affordances for the Mind surface.
- **Inputs**: terminal size, optional config overrides, current layout constraints.
- **Outputs**: stable floating pane size/position/pinning behavior.
- **Behavior**: choose a readable centered geometry, allow optional pinned mode, and support later refinement with `change-floating-pane-coordinates` if the operator resizes/moves the pane.

#### Feature: Single-surface runtime discipline
- **Description**: Ensure the Mind UI is active only when invoked.
- **Inputs**: open/close events and runtime mode.
- **Outputs**: no idle per-tab background pane when unused.
- **Behavior**: subscribe/refresh only while the Mind pane is open and leave no persistent duplicate pulse surface behind.

### Capability: Project Mind Navigation
Provide one coherent project-local place to inspect Mind state.

#### Feature: Overview screen
- **Description**: Show the most important high-level project Mind state in one screen.
- **Inputs**: project root, latest handshake snapshot, latest canon revision, recent exports, runtime health summary.
- **Outputs**: concise overview panel.
- **Behavior**: prioritize freshness, current active canon, recent export/finalize activity, and any project-local Mind errors.

#### Feature: Canon browser
- **Description**: Let operators inspect current and recent canon revisions.
- **Inputs**: project canon revision history and provenance metadata.
- **Outputs**: list/detail views for canon revisions.
- **Behavior**: show active/superseded state, timestamps, summaries, and drilldown into provenance.

#### Feature: Handshake and context-pack view
- **Description**: Show what project handshake/context state is currently available to Pi/Mind consumers.
- **Inputs**: latest handshake snapshot and context-pack compilation inputs.
- **Outputs**: readable handshake/context details.
- **Behavior**: expose freshness, source inputs, and what will be attached or derived for downstream use.

#### Feature: Session export browser
- **Description**: Show recent finalized exports that feed Mind processing.
- **Inputs**: export manifests plus `t1.md` / `t2.md` artifacts.
- **Outputs**: export list/detail views.
- **Behavior**: let operators inspect what a session finalized for this project and correlate it with later canon/retrieval state.

#### Feature: Retrieval/search view
- **Description**: Query project Mind memory from inside the floating UI.
- **Inputs**: search/query text, project scope, retrieval mode.
- **Outputs**: bounded search results with provenance.
- **Behavior**: support project-local retrieval and drilldown without forcing users into separate tooling.

### Capability: Editing and Curation
Allow bounded human edits to project Mind without conflating it with raw runtime internals.

#### Feature: Project Mind edit flow
- **Description**: Provide a safe path to edit curated project Mind state.
- **Inputs**: selected target such as project mind doc, canon notes, or operator curation fields.
- **Outputs**: persisted project Mind update plus visible success/failure feedback.
- **Behavior**: prefer explicit edit targets and structured save flows rather than raw in-place mutation of opaque runtime tables.

#### Feature: Promotion / supersede controls
- **Description**: Allow operators to mark canon state as updated, superseded, or operator-curated.
- **Inputs**: selected canon/export item and operator action.
- **Outputs**: updated revision metadata and audit trail.
- **Behavior**: preserve provenance and revision semantics instead of destructive overwrite.

#### Feature: Project-local notes and curation trail
- **Description**: Record small operator notes tied to project Mind state.
- **Inputs**: note text, selected context object.
- **Outputs**: persisted note/reference visible in the UI.
- **Behavior**: provide lightweight curation without turning the surface into a freeform transcript sink.

### Capability: Runtime Signals and Boundaries
Expose enough activity to keep the UI useful while preserving the global/runtime split.

#### Feature: Project-local Mind activity summary
- **Description**: Show recent Mind processing activity relevant to the current project.
- **Inputs**: project-scoped backlog/runtime events, recent failures, finalize activity.
- **Outputs**: compact activity list or badges.
- **Behavior**: show only project-relevant activity, not the entire fleet.

#### Feature: Jump-to-Mission-Control bridge
- **Description**: Hand operators off to Mission Control when the issue is operational rather than project-curation-centric.
- **Inputs**: selected runtime issue or user intent.
- **Outputs**: navigation/handoff into Mission Control.
- **Behavior**: keep the Mind UI narrow and provide an explicit escape hatch for global/fleet diagnosis.

#### Feature: Ownership boundary preservation
- **Description**: Keep delegated specialist UX, Mind project UI, and Mission Control runtime UX distinct.
- **Inputs**: owner-plane metadata, runtime mode, invocation source.
- **Outputs**: filtered and role-appropriate views.
- **Behavior**: the Mind UI is not a delegated subagent manager and not a global fleet dashboard.

### Capability: Documentation and Rollout
Document the floating-Mind interaction model and deprecate the per-tab pane assumption.

#### Feature: Product/runtime docs update
- **Description**: Update docs to describe the floating project-scoped Mind model and its Zellij behavior.
- **Inputs**: final invocation model, routing model, Zellij pane behavior, Mission Control boundary.
- **Outputs**: updated documentation and operator guidance.
- **Behavior**: explicitly state that AOC no longer targets one persistent Mind/Pulse pane per tab for this workflow.

#### Feature: Compatibility and regression validation
- **Description**: Add focused validation for Zellij floating-pane behavior and project resolution.
- **Inputs**: tab-aware show/hide behavior, route/title resolution, duplicate-pane avoidance, open/close lifecycle.
- **Outputs**: regression coverage and rollout checklist.
- **Behavior**: validate against newer Zellij behavior, especially explicit `show-floating-panes` / `hide-floating-panes` and `new-pane --floating` semantics.

---

## Repository Structure

```text
project-root/
├── bin/
│   ├── aoc-mind-toggle                    # new invoke/focus/hide launcher
│   └── aoc-mind-project-resolve           # optional resolver helper
├── crates/
│   ├── aoc-core/
│   │   └── src/
│   │       └── zellij_cli.rs              # tab/pane inventory and floating-pane helpers
│   ├── aoc-agent-wrap-rs/
│   │   └── src/
│   │       └── main.rs                    # Mind data/query endpoints already feeding context/runtime
│   └── aoc-mission-control/
│       └── src/
│           └── main.rs                    # optional shared TUI primitives or route handoff integration
├── .pi/
│   └── extensions/
│       └── minimal.ts                     # shortcut/command integration for invoke-anywhere Mind UI
├── docs/
│   ├── mission-control-ops.md
│   ├── mind-v2-architecture-cutover-checklist.md
│   └── research/
│       └── zellij-0.44-aoc-alignment.md
└── .taskmaster/docs/prds/
    └── task-182_project_mind_floating_ui_prd_rpg.md
```

## Module Definitions

### Module: `bin/aoc-mind-toggle`
- **Maps to capability**: Project Resolution and Invocation + Floating Pane Lifecycle and Zellij Integration
- **Responsibility**: canonical open/focus/hide entry point for the project Mind floating UI.
- **Exports**:
  - resolve current tab/project
  - create/focus named floating pane
  - explicit show/hide behavior

### Module: `crates/aoc-core/src/zellij_cli.rs`
- **Maps to capability**: Floating Pane Lifecycle and Zellij Integration
- **Responsibility**: provide stable Zellij JSON inventory and floating-pane helper logic.
- **Exports**:
  - current tab discovery
  - pane inventory helpers
  - floating visibility helpers
  - compatibility checks for explicit show/hide actions

### Module: `.pi/extensions/minimal.ts`
- **Maps to capability**: Project Resolution and Invocation
- **Responsibility**: bind the Mind UI into Pi session commands/shortcuts.
- **Exports**:
  - slash command and shortcut integration
  - optional route/title handoff metadata

### Module: Mind query/runtime endpoints
- **Maps to capability**: Project Mind Navigation + Runtime Signals and Boundaries
- **Responsibility**: provide project-local Mind data to the floating UI.
- **File structure**:
  ```text
  crates/aoc-agent-wrap-rs/src/main.rs
  crates/aoc-core/src/mind_contracts.rs
  crates/aoc-core/src/mind_observer_feed.rs
  ```
- **Exports**:
  - handshake/context-pack reads
  - export/canon/retrieval queries
  - project-local activity summaries

### Module: Mind floating TUI surface
- **Maps to capability**: Project Mind Navigation + Editing and Curation
- **Responsibility**: render the project-local TUI and route user actions.
- **File structure**:
  ```text
  crates/aoc-mission-control/src/main.rs        # if reusing existing TUI runtime
  # or a dedicated future crate/bin if split later
  ```
- **Exports**:
  - overview/canon/handshake/exports/search views
  - edit/curation actions
  - Mission Control handoff action

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies — built first.

- **Zellij inventory + floating capability adapter**: stable discovery of tab IDs, pane IDs, visibility, and floating-pane actions.
- **Project route resolver contract**: normalized route/title/cwd-to-project resolution contract.
- **Mind read-model query contract**: bounded project-local reads for overview, canon, handshake, exports, and retrieval.

### Invocation Layer (Phase 1)
- **Mind launcher command**: Depends on [Zellij inventory + floating capability adapter, Project route resolver contract]
- **Pi shortcut/command wiring**: Depends on [Mind launcher command, Project route resolver contract]

### UI Read Layer (Phase 2)
- **Overview/canon/handshake/export/search views**: Depends on [Mind read-model query contract, Mind launcher command]
- **Project-local activity summary**: Depends on [Mind read-model query contract, Zellij inventory + floating capability adapter]

### Editing/Curation Layer (Phase 3)
- **Project Mind edit flow**: Depends on [Overview/canon/handshake/export/search views]
- **Promotion/supersede controls**: Depends on [Overview/canon/handshake/export/search views]
- **Notes/curation trail**: Depends on [Overview/canon/handshake/export/search views]

### Integration and Rollout Layer (Phase 4)
- **Mission Control handoff bridge**: Depends on [Overview/canon/handshake/export/search views, Project-local activity summary]
- **Docs + regression validation**: Depends on [Mind launcher command, Pi shortcut/command wiring, UI read layer, Editing/Curation Layer]

---

## Implementation Roadmap

### Phase 0 — Confirm substrate and boundaries
**Goals**
- Lock the product split: floating project Mind UI vs Mission Control global runtime.
- Confirm the project resolver contract and Zellij floating-pane capabilities.

**Deliverables**
- project-resolution rules documented
- Zellij helper coverage for `new-pane --floating`, `show-floating-panes --tab-id`, `hide-floating-panes --tab-id`
- decision on whether the UI reuses Mission Control runtime or gets its own mode/bin

**Exit Criteria**
- a deterministic algorithm exists for deriving the current project from an AOC tab
- floating-pane create/focus/hide behavior is explicit, not toggle-heuristic-only

### Phase 1 — Invocation and routing
**Goals**
- Make `Alt+M` / `/mind` open the right project surface.

**Deliverables**
- `aoc-mind-toggle`
- Pi shortcut/command integration
- ambiguous-project fallback picker or error flow

**Exit Criteria**
- common-case open flow works from any normal project-aligned AOC tab
- duplicate floating panes are not created on repeated invocation

### Phase 2 — Read-only project Mind navigation
**Goals**
- Ship a useful read-only project Mind surface first.

**Deliverables**
- overview view
- canon browser
- handshake/context view
- session export browser
- retrieval/search view
- project-local activity summary

**Exit Criteria**
- operator can inspect current project Mind state without leaving the TUI
- surface stays project-local and low-noise

### Phase 3 — Editing and curation
**Goals**
- Add bounded editing without destabilizing runtime internals.

**Deliverables**
- project Mind edit flow
- promotion/supersede controls
- project-local notes/curation trail

**Exit Criteria**
- at least one safe curation path is production-usable
- revision/provenance semantics remain intact

### Phase 4 — Integration polish and rollout
**Goals**
- document and harden the final operator model.

**Deliverables**
- docs updates
- regression scripts/tests
- Mission Control handoff bridge
- deprecation of per-tab Mind-pane assumptions in docs/configs

**Exit Criteria**
- operator documentation matches runtime behavior
- Zellij 0.44+ floating-pane semantics are validated

---

## Test Strategy

### Unit / logic validation
- route/title/cwd project resolution cases
- duplicate floating-pane detection and reuse
- explicit show/hide decision logic per Zellij capability
- project-local query shaping for overview/canon/handshake/exports/search

### Integration validation
- invoke from an AOC tab with route metadata present
- invoke from an AOC tab with title-only fallback
- invoke when ambiguity requires a project picker
- repeated open/focus/hide cycles without pane duplication
- project handoff from Mind UI to Mission Control

### Manual/operator validation
- verify no persistent Mind pane exists when closed
- verify floating pane opens with readable geometry on common terminal sizes
- verify tab-scoped behavior matches expectation in Zellij
- verify low-noise refresh behavior while open

### Regression checks
- no regression to Mission Control floating-tab behavior
- no regression to delegated subagent manager shortcuts/flows
- no regression to project route metadata in normal AOC tabs

---

## Risks and Mitigations

### Risk: Project resolution is too heuristic
- **Mitigation**: prefer structured route metadata first; make ambiguity explicit rather than silently guessing.

### Risk: Floating panes remain awkward/tab-scoped in ways that confuse operators
- **Mitigation**: document tab-scoped behavior clearly; open one named Mind pane per tab and reuse it predictably.

### Risk: UI scope expands into a second Mission Control
- **Mitigation**: keep the surface project-local; bridge to Mission Control for fleet/runtime diagnosis instead of duplicating fleet features.

### Risk: Editing flows mutate opaque runtime state unsafely
- **Mitigation**: restrict editing to curated/high-level artifacts and revision-aware operations, not arbitrary low-level table mutation.

### Risk: Refresh/live updates recreate the resource problems of a persistent pane
- **Mitigation**: subscribe only while open and keep default refresh bounded/lightweight.

---

## Non-Goals
- Do not restore or introduce one persistent Pulse/Mind pane per normal AOC tab for this workflow.
- Do not collapse the project Mind UI into Mission Control’s global fleet/operator role.
- Do not present Mind internals primarily through the delegated specialist manager.
- Do not rely on pane-title scraping alone when structured route metadata is available.
- Do not make live subscriptions/run loops active for every tab when the Mind UI is not open.
