# AOC Pi Subagent UX Alignment PRD (RPG)

> Follow-on alignment note: this PRD does **not** replace the detached runtime/control-plane work owned by task 169 or the explicit specialist-role work owned by task 129. It productizes the delegated-specialist Pi session experience on top of the existing AOC-native substrate while preserving the Mind ownership boundary defined by task 178 and the fleet/control-plane boundary defined by task 149.

> Comparative reference note: this scope is informed by a review of `https://github.com/nicobailon/pi-subagents`. The goal is to absorb the strongest Pi-native UX patterns (manager overlay, clarify-before-run flow, async observability, history, chain ergonomics) **without** giving up AOC-owned detached lifecycle truth, Pulse/Mission Control integration, provenance-aware tool policy, or delegated-vs-Mind plane separation.

## Problem Statement
AOC already has the hard architectural pieces for delegated specialist subagents: canonical `.pi/agents/*` manifests, detached/background execution, durable job-state recovery, Pulse/Mission Control integration, Pi 0.62 provenance-aware tool policy, specialist-role approval gates, and explicit stale/recovery semantics. What it does **not** yet have is an operator experience that feels as complete and fast as the runtime substrate underneath it.

Current pain points:
- The Pi session surface is still closer to a runtime inspector than a polished operator workflow.
- Launching delegated work often relies on raw slash-command syntax rather than a focused launch flow.
- The current inspector is useful but narrow: it is cycle-first, not manager-first.
- Stable per-job report artifacts are not yet a first-class operator contract, so deep inspection still relies too much on inline excerpts.
- Chain discovery and specialist-role discovery are technically present but not yet presented as an integrated product surface.
- AOC already knows that delegated specialist UX must stay separate from Mind worker UX, but that boundary needs to be reinforced as the session surfaces become richer.

`pi-subagents` demonstrates that Pi-native subagent UX can feel much more productized: manager overlay, clarify-before-run, run history, async observability, chain authoring, and strong rerun ergonomics. AOC should adopt those product lessons while keeping AOC-specific guarantees:
- durable registry / Pulse remain the lifecycle source of truth,
- builtin + project-local provenance policy remains enforced,
- builder/red-team approval gates remain explicit,
- Mind workers reuse detached contracts without inheriting delegated-specialist UX semantics.

## Target Users
- **Primary operators / developers** using Pi sessions who want to dispatch, inspect, rerun, and hand off delegated specialist work without leaving the main flow.
- **AOC maintainers** responsible for making detached specialist work feel native inside Pi while keeping control-plane semantics correct.
- **Mind/runtime contributors** who need the delegated-specialist UX work to reuse shared lifecycle contracts without leaking delegated assumptions into Mind-owned worker planes.

## Success Metrics
- 100% of terminal delegated specialist jobs persist stable artifact/report references while the durable detached registry remains the lifecycle source of truth.
- Operators can launch, inspect, cancel, and rerun delegated specialists/chains/roles from one canonical Pi overlay in three interactions or fewer for the common case.
- Default Pi-session detached chrome stays low-noise: one compact status line plus a bounded widget/overlay model rather than a persistent multiline wall.
- Role-aware launch UX exposes write-approval requirements and Mind context-pack attachment state before dispatch.
- Recent history, recent failures, and chain/role catalogs are visible without requiring raw slash-command recall.
- Operators have a one-shortcut fast path to open/focus a warm Zellij floating-pane supervision surface for delegated runs without replacing the Pi launch/clarify flow.
- Mind-owned detached jobs continue to appear in fleet/global control-plane views without being presented as local delegated-specialist session jobs by default.
- No regressions occur in Pi 0.62 provenance-aware tool restrictions, role approval gates, stale/recovery semantics, or durable-registry-backed status/cancel behavior.

---

## Architectural Framing
This PRD is a **productization layer** over the existing AOC detached runtime substrate.

It intentionally distinguishes three things:
1. **Detached substrate** — lifecycle, registry, provenance, Pulse, Mission Control, recovery.
2. **Delegated specialist session UX** — launch, inspect, handoff, rerun, clarify, history.
3. **Mind ownership plane** — project-scoped detached worker orchestration that may reuse substrate contracts but must not inherit delegated-specialist product semantics.

The core implementation rule is:

> **Reuse substrate contracts, not product semantics.**

That means AOC should borrow UX patterns from `pi-subagents`, but it should **not** replace AOC-native registry/Pulse truth with temp-dir-only status files, and it should **not** collapse Mind workers into Pi session specialist UX.

---

## Capability Tree

### Capability: Launch Preparation and Execution Modes
Make delegated specialist launch flows fast, explicit, and role-aware.

#### Feature: Manager-lite launch surface
- **Description**: Provide a Pi overlay entry point that lets operators select an agent, chain, or specialist role and launch it without raw command memorization.
- **Inputs**: canonical agent/chain/role catalogs, cwd, task text, current session context.
- **Outputs**: normalized launch request and immediate queued-job acknowledgement.
- **Behavior**: present a focused launch flow with bounded controls, reuse canonical manifests as the source of truth, and avoid duplicating hidden dispatch paths.

#### Feature: Clarify-before-run flow
- **Description**: Let operators edit launch parameters before dispatch when the task is ambiguous or high-value.
- **Inputs**: selected agent/chain/role, initial task text, available models, role metadata, context-pack availability.
- **Outputs**: confirmed launch configuration.
- **Behavior**: support editing task, cwd, optional model override, execution mode, and role-specific approval/context hints before the job is queued.

#### Feature: Explicit execution-mode contract
- **Description**: Distinguish background and parent-session handoff behaviors as named execution modes.
- **Inputs**: operator choice or tool/command arguments.
- **Outputs**: normalized execution-mode metadata on the job and predictable post-completion behavior.
- **Behavior**: support at least `background`, `inline_wait`, and `inline_summary` semantics, while leaving room for future session-fork modes without overloading detached behavior into one ambiguous path.

### Capability: Operator Drilldown and History
Make delegated work inspectable and debuggable without cluttering the main Pi session.

#### Feature: Stable report artifact persistence
- **Description**: Persist a stable report bundle for every terminal delegated specialist job.
- **Inputs**: terminal job summary, final assistant output, selected stream events, stderr, provenance metadata.
- **Outputs**: stable artifact directory and report references for UI/handoff surfaces.
- **Behavior**: keep inline UI compact while preserving full drilldown under stable per-job paths such as `.pi/tmp/subagents/<job-id>/`.

#### Feature: Manager list/detail views
- **Description**: Replace the current cycle-only inspector with a manager-style overlay showing lists, details, and recent results.
- **Inputs**: active jobs, recent jobs, catalogs, artifact refs, role metadata.
- **Outputs**: browsable Pi overlay with list/detail/state views.
- **Behavior**: allow operators to switch between catalogs and recent jobs, inspect one job in detail, and trigger bounded actions such as cancel or rerun.

#### Feature: Zellij floating-pane fast path
- **Description**: Provide a near-instant toggle into a warm floating supervision surface for delegated subagent status and drilldown.
- **Inputs**: current Zellij session context, named pane metadata, delegated detached job state from Pulse/Mission Control, and operator shortcut intent.
- **Outputs**: focused or newly opened floating pane targeting the delegated supervision surface.
- **Behavior**: open/focus one canonical floating pane without spawning duplicates, preserve Pulse/durable-registry truth, keep Pi as the launch/clarify surface, and avoid turning the shortcut into a parallel runtime.

#### Feature: Run history and recent-failure visibility
- **Description**: Track and surface recent delegated runs per agent/role/chain.
- **Inputs**: terminal job metadata and artifact refs.
- **Outputs**: recent-run summaries for detail screens, filters, and quick reruns.
- **Behavior**: store lightweight history entries without changing registry truth, highlight recent failures, and keep the common rerun/debug loop fast.

### Capability: Chain and Role Catalog UX
Make existing orchestration assets feel discoverable and usable.

#### Feature: Chain catalog and detail views
- **Description**: Present canonical chains as first-class operator-visible items with step previews and status-aware reuse.
- **Inputs**: `.pi/agents/agent-chain.yaml`, agent catalog, recent run history.
- **Outputs**: chain listings, chain detail previews, and launch actions.
- **Behavior**: show ordered steps, referenced agents, and launch affordances without forcing users to remember chain names or raw slash syntax.

#### Feature: Role-aware operator controls
- **Description**: Surface specialist-role semantics directly in the launch/detail UX.
- **Inputs**: role mapping, write-approval requirements, context-pack availability, job metadata.
- **Outputs**: role-aware launch/inspect/handoff views.
- **Behavior**: expose approval state, role labels, and Mind context attachment status wherever operators launch or inspect explicit roles.

#### Feature: Rerun and handoff ergonomics
- **Description**: Allow operators to rerun prior delegated work or reopen concise handoff artifacts without reconstructing context manually.
- **Inputs**: prior job metadata, prior task text, artifact refs.
- **Outputs**: rerun launch config and handoff/open actions.
- **Behavior**: support rerun-as-is, rerun-with-edits, and handoff reopening from the manager/detail surfaces.

### Capability: Boundary and Guardrail Enforcement
Keep UX improvements aligned with AOC safety and ownership boundaries.

#### Feature: Delegated-vs-Mind plane filtering
- **Description**: Ensure the delegated specialist manager only presents the intended local/session operator surface by default.
- **Inputs**: detached registry entries, owner-plane metadata, worker-kind metadata.
- **Outputs**: filtered lists and clear ownership labels.
- **Behavior**: delegated manager defaults to delegated/specialist work, while Mission Control/fleet surfaces continue to show cross-plane summaries.

#### Feature: Recursion and session-mode guardrails
- **Description**: Prevent runaway subagent nesting and ambiguous context inheritance.
- **Inputs**: nested dispatch depth, execution mode, session persistence state, future context-mode choices.
- **Outputs**: allow/deny decisions and explicit failure guidance.
- **Behavior**: enforce bounded nesting, keep default execution fresh/detached unless explicitly changed, and fail fast on unsupported session-mode requests.

### Capability: Documentation and Rollout
Make the operator contract explicit before and during rollout.

#### Feature: Canonical operator/runtime reference
- **Description**: Publish a dedicated runtime/operator document for delegated subagent UX.
- **Inputs**: lifecycle semantics, execution modes, artifact layout, commands/tools, recovery rules, provenance model.
- **Outputs**: `docs/subagent-runtime.md`.
- **Behavior**: document the operator mental model clearly enough that developers do not need to reverse-engineer `.pi/extensions/subagent.ts`.

#### Feature: UX regression and rollout validation
- **Description**: Add focused validation for manager/launch/history/artifact flows.
- **Inputs**: extension behavior, role behavior, artifact persistence, registry state, Mission Control visibility.
- **Outputs**: regression scripts/tests and rollout checklist updates.
- **Behavior**: catch regressions in low-noise UX, policy enforcement, artifact references, and delegated-vs-Mind boundary handling.

---

## Repository Structure

```text
project-root/
├── .pi/
│   ├── agents/
│   │   ├── *.md
│   │   ├── teams.yaml
│   │   └── agent-chain.yaml
│   ├── extensions/
│   │   ├── minimal.ts
│   │   ├── themeMap.ts
│   │   └── subagent.ts
│   └── tmp/
│       └── subagents/
│           └── <job-id>/
│               ├── report.md
│               ├── meta.json
│               ├── events.jsonl
│               ├── prompt.md
│               └── stderr.log
├── crates/
│   ├── aoc-agent-wrap-rs/
│   │   └── src/
│   │       └── insight_orchestrator.rs
│   ├── aoc-core/
│   │   └── src/
│   │       └── insight_contracts.rs
│   └── aoc-mission-control/
│       └── src/
│           └── main.rs
├── docs/
│   ├── agents.md
│   ├── insight-subagent-orchestration.md
│   └── subagent-runtime.md
└── scripts/
    └── pi/
        ├── test-specialist-role-surface.sh
        ├── test-specialist-role-runtime-guards.sh
        ├── test-subagent-ux-surface.sh
        └── test-subagent-ux-runtime.sh
```

## Module Definitions

### Module: `.pi/extensions/subagent.ts`
- **Maps to capability**: Launch Preparation and Execution Modes + Operator Drilldown and History + Chain and Role Catalog UX
- **Responsibility**: remain the canonical delegated specialist Pi extension surface for tool/command registration, manager overlay state, launch clarification, compact status, artifact references, and handoff UX.
- **Exports**:
  - `aoc_subagent` / `aoc_specialist_role` entry-point behavior
  - manager/inspector overlay helpers
  - execution-mode normalization helpers
  - artifact/history summary helpers
  - compact status/widget rendering helpers

### Module: `.pi/agents/*.md` + `agent-chain.yaml`
- **Maps to capability**: Chain and Role Catalog UX
- **Responsibility**: remain the canonical source of truth for delegated specialist agents and chain definitions.
- **Exports**:
  - agent manifest metadata
  - chain definitions
  - role-backed agent identity

### Module: `docs/subagent-runtime.md`
- **Maps to capability**: Documentation and Rollout
- **Responsibility**: document lifecycle states, execution modes, artifact layout, commands, recovery, trust policy, delegated-vs-Mind boundaries, and the split between Pi launch UX and Zellij supervision fast paths.

### Module: `bin/aoc-mission-control-toggle` + related Zellij launcher/toggle glue
- **Maps to capability**: Operator Drilldown and History + Boundary and Guardrail Enforcement
- **Responsibility**: provide the canonical floating-pane toggle/focus path for delegated supervision without collapsing local Pi launch UX into the global Mission Control control surface.

### Module: `scripts/pi/test-subagent-ux-surface.sh` + `scripts/pi/test-subagent-ux-runtime.sh`
- **Maps to capability**: UX regression and rollout validation
- **Responsibility**: verify operator-visible launch/manager/history/artifact behavior and runtime guardrails.

### Module: `crates/aoc-core/src/insight_contracts.rs` + `crates/aoc-agent-wrap-rs/src/insight_orchestrator.rs`
- **Maps to capability**: Boundary and Guardrail Enforcement
- **Responsibility**: preserve detached-registry truth, ownership metadata, and compatibility-safe status/result envelopes used by the Pi extension.

### Module: `crates/aoc-mission-control/src/main.rs`
- **Maps to capability**: Boundary and Guardrail Enforcement
- **Responsibility**: keep global fleet summaries and cross-plane visibility available without turning local delegated session UX into the global control surface.

---

## Dependency Chain

### Foundation Layer (Phase 0)
No intra-PRD dependencies.

- **Delegated specialist UX contract**: defines what this PRD owns vs what remains in tasks 169 / 129 / 178 / 149.
- **Execution-mode vocabulary**: defines `background`, `inline_wait`, `inline_summary`, and future extension rules.
- **Artifact/history schema**: defines stable report bundle layout and summary-reference contract.
- **Operator/runtime doc outline**: defines the canonical doc structure for rollout.

### Artifact and State Layer (Phase 1)
- **Stable report persistence**: Depends on [Artifact/history schema].
- **History summary persistence**: Depends on [Artifact/history schema].
- **Compact status/report references**: Depends on [Stable report persistence, History summary persistence].

### Session UX Layer (Phase 2)
- **Manager-lite list/detail overlay**: Depends on [Compact status/report references, Delegated specialist UX contract].
- **Job detail drilldown actions**: Depends on [Manager-lite list/detail overlay, Stable report persistence].
- **Catalog views for chains and roles**: Depends on [Manager-lite list/detail overlay, `.pi/agents/*.md` + `agent-chain.yaml`].
- **Zellij floating-pane fast path**: Depends on [Manager-lite list/detail overlay, detached status visibility from Pulse/Mission Control, and explicit Pi-vs-Mission-Control boundary notes].

### Launch Ergonomics Layer (Phase 3)
- **Clarify-before-run launch flow**: Depends on [Manager-lite list/detail overlay, Execution-mode vocabulary].
- **Role-aware launch controls**: Depends on [Clarify-before-run launch flow, task-129 specialist metadata].
- **Rerun / rerun-with-edits flow**: Depends on [History summary persistence, Stable report persistence, Clarify-before-run launch flow].

### Hardening and Rollout Layer (Phase 4)
- **Delegated-vs-Mind plane filtering**: Depends on [Manager-lite list/detail overlay, detached ownership metadata from task 178].
- **Recursion/session-mode guardrails**: Depends on [Execution-mode vocabulary, Clarify-before-run launch flow].
- **Docs and regression rollout**: Depends on [all prior phases].

---

## Development Phases

### Phase 0: Contract, Scope, and Doc Baseline
**Goal**: Freeze the product boundary and operator contract before UI work expands.

**Entry Criteria**:
- Existing detached specialist runtime baseline from task 169 is present.
- Existing specialist-role baseline from task 129 is present.
- Detached ownership-plane framing from tasks 149 and 178 is available for reference.

**Tasks**:
- [ ] Define the delegated-specialist UX ownership boundary versus detached substrate and Mind planes (depends on: [none])
  - Acceptance criteria: PRD and task metadata explicitly state that registry/Pulse truth, provenance policy, and Mind ownership remain outside the scope of UX-only rewrites.
  - Test strategy: Review against tasks 169, 129, 149, and 178 for non-overlap and no circular ownership.

- [ ] Define the execution-mode vocabulary and operator semantics (depends on: [none])
  - Acceptance criteria: `background`, `inline_wait`, and `inline_summary` are defined clearly with expected completion/handoff behavior.
  - Test strategy: Tool/command argument tests confirm normalized mode mapping and no ambiguous defaults.

- [ ] Define the artifact/history schema and document outline (depends on: [none])
  - Acceptance criteria: stable per-job artifact files and history summaries have named paths/fields and doc sections.
  - Test strategy: fixture/schema checks validate path/field stability.

**Exit Criteria**:
- Product boundary, execution modes, and artifact schema are explicit enough to guide implementation without reopening architecture.

**Delivers**:
- A stable spec for low-noise delegated-specialist UX work.

---

### Phase 1: Stable Artifacts and Summary References
**Goal**: Make deep inspection possible without bloating inline session chrome.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Persist stable per-job report bundles under `.pi/tmp/subagents/<job-id>/` (depends on: [Define the artifact/history schema and document outline])
  - Acceptance criteria: every terminal delegated job has stable artifact refs for report/meta/events/prompt/stderr as available.
  - Test strategy: regression tests verify bundle creation for success, fallback, error, and cancelled jobs.

- [ ] Add lightweight run-history summaries keyed by job/agent/role/chain (depends on: [Define the artifact/history schema and document outline])
  - Acceptance criteria: recent runs can be listed without reparsing full artifacts; summaries include status, duration, cwd, model, and artifact path.
  - Test strategy: history persistence tests verify append/update behavior and bounded retention.

- [ ] Update status/handoff/report surfaces to prefer summary refs over oversized inline payloads (depends on: [Persist stable per-job report bundles under `.pi/tmp/subagents/<job-id>/`, Add lightweight run-history summaries keyed by job/agent/role/chain])
  - Acceptance criteria: compact status stays concise while drilldown paths remain discoverable.
  - Test strategy: render tests verify low-noise summaries and artifact-link visibility.

**Exit Criteria**:
- Delegated job drilldown no longer depends primarily on oversized inline excerpts.

**Delivers**:
- Stable report bundles and lightweight history suitable for manager/detail UX.

---

### Phase 2: Manager-Lite Overlay and Drilldown
**Goal**: Turn the current inspector into a useful operator surface.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Build a manager-lite overlay with list/detail views for agents, chains, active jobs, and recent jobs (depends on: [Update status/handoff/report surfaces to prefer summary refs over oversized inline payloads])
  - Acceptance criteria: operators can browse catalogs and recent work from one overlay rather than only cycling inspector entries.
  - Test strategy: overlay interaction tests cover empty state, browse state, recent state, and active-job state.

- [ ] Add job-detail drilldown actions for inspect, handoff reopen, cancel, and rerun (depends on: [Build a manager-lite overlay with list/detail views for agents, chains, active jobs, and recent jobs])
  - Acceptance criteria: one selected job exposes bounded actions and summary fields without dumping raw transcript content into the main session.
  - Test strategy: action tests verify command routing, artifact opening references, and cancel/rerun wiring.

- [ ] Improve compact widget/status lines to show actionable progress rather than registry-only identifiers (depends on: [Build a manager-lite overlay with list/detail views for agents, chains, active jobs, and recent jobs])
  - Acceptance criteria: active rows include concise role/agent/progress outcome hints while preserving low clutter.
  - Test strategy: render regression tests verify bounded line counts and readable active/recent summaries.

**Exit Criteria**:
- Delegated specialist UX has a real manager surface rather than only a status line plus cycle-only inspector.

**Delivers**:
- AOC-native manager-lite UX for delegated specialists.

---

### Phase 3: Clarify Launch Flow and Role/Chain Ergonomics
**Goal**: Make launch, rerun, and role-aware operation feel fast and explicit.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Add clarify-before-run launch flow with task/cwd/model/mode editing (depends on: [Build a manager-lite overlay with list/detail views for agents, chains, active jobs, and recent jobs, Define the execution-mode vocabulary and operator semantics])
  - Acceptance criteria: operators can confirm or edit launch parameters before dispatch when desired.
  - Test strategy: interaction tests cover default launch, edited launch, and cancellation from clarify state.

- [ ] Surface role-aware approval/context controls in launch/detail views (depends on: [Add clarify-before-run launch flow with task/cwd/model/mode editing])
  - Acceptance criteria: builder/red-team approval state and Mind context-pack availability are visible before dispatch and in subsequent detail views.
  - Test strategy: role tests verify approval gating remains enforced and context-pack state is shown fail-open when unavailable.

- [ ] Improve chain catalog/detail UX and add rerun-with-edits ergonomics (depends on: [Build a manager-lite overlay with list/detail views for agents, chains, active jobs, and recent jobs, Add lightweight run-history summaries keyed by job/agent/role/chain, Add clarify-before-run launch flow with task/cwd/model/mode editing])
  - Acceptance criteria: chains are browseable, previewable, and relaunchable without raw-name recall.
  - Test strategy: chain tests verify step previews, recent-run links, and relaunch parameter carryover.

**Exit Criteria**:
- Delegated launch flows are manager-first and role-aware, not slash-command-first.

**Delivers**:
- A more complete Pi-native launch and rerun experience.

---

### Phase 4: Guardrails, Boundary Hardening, and Rollout
**Goal**: Land the UX improvements without harming control-plane semantics.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Enforce delegated-vs-Mind plane filtering and ownership labels in the local manager surface (depends on: [Build a manager-lite overlay with list/detail views for agents, chains, active jobs, and recent jobs])
  - Acceptance criteria: local delegated manager defaults to delegated/specialist jobs; Mind-owned detached workers stay in fleet/global surfaces unless explicitly requested.
  - Test strategy: integration tests validate owner-plane filtering and no delegated/Mind leakage in the local overlay.

- [ ] Add recursion/session-mode guardrails compatible with future context modes (depends on: [Add clarify-before-run launch flow with task/cwd/model/mode editing, Define the execution-mode vocabulary and operator semantics])
  - Acceptance criteria: bounded nesting and unsupported session-mode requests fail fast with actionable errors.
  - Test strategy: guardrail tests cover max depth, invalid mode/state combinations, and restart-safe behavior.

- [ ] Publish docs and validation for rollout (`docs/subagent-runtime.md`, scripts, checklist updates) (depends on: [all prior phase tasks])
  - Acceptance criteria: operator/runtime docs exist, reference linked docs are updated, and regression scripts cover the new UX path.
  - Test strategy: documentation/smoke validation plus targeted scripts for manager surface, artifact refs, role controls, and boundary handling.

**Exit Criteria**:
- Delegated specialist UX is productized without regressing AOC detached control-plane behavior or Mind separation.

**Delivers**:
- A documented, testable, low-noise delegated specialist UX aligned with AOC architecture.

---

## Global Test Strategy
- Extend existing detached-subagent and specialist-role coverage rather than replacing it.
- Add focused surface tests for manager overlay states, launch clarification, role-aware controls, compact status rendering, and chain browsing.
- Add runtime tests for stable artifact bundle persistence, rerun wiring, recent history, recursion guardrails, and delegated-vs-Mind filtering.
- Add Mission Control/pulse integration regressions ensuring fleet/global summaries continue to show cross-plane work without leaking Mind-owned workers into the local delegated manager by default.
- Validate that provenance-aware tool policies and approval gates remain unchanged while UX/product layers evolve.

## Risks and Open Questions
- **Risk: accidental ownership creep** — richer delegated UX could begin to own runtime semantics that should remain in task 169 / wrapper contracts. Mitigation: keep registry/Pulse truth and ownership metadata outside this PRD’s rewrite scope.
- **Risk: Mind/delegated leakage** — making the manager too global could blur the delegated-vs-Mind boundary. Mitigation: local manager defaults to delegated work only; Mission Control remains the global fleet surface.
- **Risk: UI bloat** — a richer manager could reintroduce the widget clutter task 169 intentionally moved away from. Mitigation: compact status defaults, bounded widget rows, overlay-first drilldown.
- **Risk: artifact sprawl** — stable reports can become noisy or unbounded. Mitigation: bounded retention and summary/history indirection rather than dumping everything inline.
- **Open question: session fork mode timing** — the product surface should leave room for future forked-session execution, but this PRD does not require full adoption now.

## Architectural Decisions
- **Decision: keep AOC-native detached registry/Pulse as source of truth**
  - **Rationale**: pi-subagents-style status files are useful artifact/debug aids, but AOC already has a stronger durable-registry model integrated with Pulse and Mission Control.
  - **Alternative considered**: temporary-file-first async truth modeled after external packages.

- **Decision: treat `pi-subagents` as a UX reference, not a runtime dependency**
  - **Rationale**: AOC needs repo-owned manifests, provenance policy, explicit role approval, and control-plane alignment that should not be outsourced.
  - **Alternative considered**: adopting the package wholesale and adapting around it.

- **Decision: preserve delegated-specialist product semantics as distinct from Mind workers**
  - **Rationale**: Mind workers and delegated specialists share lifecycle primitives but serve different operator workflows.
  - **Alternative considered**: one unified detached-worker UX surface.

## Related Work
- `.taskmaster/docs/prds/task-169_aoc_detached_pi_subagent_runtime_prd_rpg.md`
- `.taskmaster/docs/prds/task-129_pi-specialist-role-interface_prd.md`
- `.taskmaster/docs/prds/aoc_detached_orchestration_prd_rpg.md`
- `docs/insight-subagent-orchestration.md`
- Pi example: `examples/extensions/subagent/README.md`
- Comparative reference: `https://github.com/nicobailon/pi-subagents`
