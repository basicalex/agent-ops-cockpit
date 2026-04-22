# AOC Detached Orchestration PRD (RPG)

## Pi 0.62 / Zellij 0.44 Alignment Delta

This umbrella PRD now has two concrete implementation-alignment updates:
- **Pi 0.62 provenance**: detached jobs and specialist-role dispatch should adopt Pi-native `sourceInfo` so AOC can distinguish built-in, project-local, and extension-provided tools/agents when enforcing trust and capability policy.
- **Pi tool rendering**: `renderCall` / `renderResult` should be treated as a compact UX enhancement for detached inspection, status, and handoff surfaces rather than a new orchestration architecture.
- **Zellij 0.44 topology**: detached operator surfaces should prefer native pane/tab JSON inventory (`list-panes`, `list-tabs`, `current-tab-info`) over `dump-layout` parsing wherever possible.
- **Zellij 0.44 drilldown**: bounded pane evidence capture (`dump-screen --pane-id`) and opt-in live follow (`subscribe --pane-id`) are valid operator drilldown tools, but they remain secondary to Pulse/runtime state and the durable detached registry.

Source alignment note: see `docs/research/zellij-0.44-aoc-alignment.md`.

## Problem Statement
AOC now has the core ingredients for detached worker execution, but the end-to-end orchestration model is still split across multiple domain PRDs and implementation slices. Detached delegated specialist subagents are becoming real in Pi, Mind already has project-scoped T1/T2/T3 runtimes, and Mission Control / pulse panes have an increasingly clear UI boundary. What is still missing is one canonical architecture plan that defines how these parts fit together as a single system.

Without that unified plan, AOC risks implementation drift in several ways:
- Pi detached specialists may evolve as a session-local UX without a clear durable-control-plane contract.
- Mind workers may continue to activate through wrapper tick paths in ways that look coupled to open Zellij panes rather than project queue pressure.
- Mission Control and pulse panes may blur ownership boundaries and duplicate global fleet surfaces in normal work panes.
- Shared detached metadata such as `owner_plane` and `worker_kind` may exist in code without a clearly documented orchestration policy.
- Taskmaster may track detached runtime work, overseer work, and Mind runtime work separately without one explicit whole-system dependency chain.

We need an umbrella detached orchestration plan that defines the shared detached substrate, the distinct ownership models for delegated specialists versus Mind workers, the canonical Mission Control versus pulse-pane boundary, and the project-scoped worker admission policy that prevents one always-on Mind worker per Zellij pane.

## Target Users
- **AOC maintainers** evolving the shared detached runtime, Mission Control, and wrapper/runtime boundaries.
- **Directing developers/operators** who need detached specialists to feel native in Pi sessions while keeping global orchestration visible only where appropriate.
- **Mind/runtime contributors** who need a project-scoped, queue-driven detached worker model for T1/T2/T3 without inheriting delegated-subagent UX semantics.
- **Future contributors** who need one canonical document describing how detached ownership, fleet visibility, pane boundaries, and restart/recovery rules fit together.

## Success Metrics
- All detached orchestration work for delegated specialists, Mission Control fleet visibility, and Mind project-scoped worker dispatch is tracked under one canonical tag and umbrella PRD.
- Pi delegated specialists are session-scoped, on-demand, and visibly recoverable without requiring raw command-only workflows.
- Mind detached workers scale by project/repo queue pressure rather than by the number of open Zellij panes.
- Mission Control becomes the only global detached fleet surface; per-pane pulse remains local/tab-scoped.
- Detached jobs can be distinguished consistently by ownership metadata and worker kind across storage, wrapper, and Mission Control.
- Restart, stale-worker, and multi-pane dedup behavior are covered by explicit regression tests before rollout.

---

## Architectural Framing
This PRD is the umbrella architecture for detached orchestration across AOC.

It defines one shared detached control-plane substrate and two distinct ownership models:
- **Delegated ownership plane**: Pi/main-agent-invoked specialist helpers that are session-scoped, operator-facing, and on-demand.
- **Mind ownership plane**: project-scoped, queue-driven cognition workers that are runtime-facing, artifact-oriented, and not tied to any single Pi session or Zellij pane.

It also defines the canonical UI boundary:
- **Mission Control** is the global operator and fleet surface.
- **Per-pane pulse** is local-only and should never become a reduced copy of Mission Control.

This PRD owns:
- shared detached lifecycle and ownership policy,
- Pi specialist session UX expectations,
- Mission Control fleet-view ownership boundaries,
- project-scoped Mind dispatcher topology and admission policy,
- anti-duplication constraints for panes and sessions.

This PRD does **not** replace the domain PRDs for delegated specialist runtime details, Mind memory/runtime details, or overseer consultation semantics. Those documents remain authoritative for their local domains; this PRD aligns them into one orchestration model.

---

## Capability Tree

### Capability: Shared Detached Control Plane
Provide one durable detached lifecycle substrate reused by multiple ownership planes.

#### Feature: Detached ownership metadata
- **Description**: Stamp detached jobs with ownership and worker identity metadata so multiple runtime classes can share one registry.
- **Inputs**: dispatch request, runtime origin, worker role.
- **Outputs**: `owner_plane`, `worker_kind`, mode, lifecycle state, timestamps, and result metadata.
- **Behavior**: preserve one shared status/cancel/result contract while keeping delegated and Mind jobs distinguishable.

#### Feature: Durable job registry and recovery
- **Description**: Persist detached jobs in a durable registry and reconcile interrupted work after restart.
- **Inputs**: job create/start/update/finish events, restart reconciliation scans, cancellation state.
- **Outputs**: durable job rows, stale detection, restart-safe visibility, and bounded recent-history lookup.
- **Behavior**: treat the durable registry as the source of truth for lifecycle state; avoid extension-local orphan truth.

#### Feature: Shared cancellation and result capture
- **Description**: Use one structured status/result/cancel surface across worker classes.
- **Inputs**: running job ids, captured stdout/stderr/result bundles, cancel requests.
- **Outputs**: structured terminal states, excerpts, per-step results, and explicit cancelled/fallback/error outcomes.
- **Behavior**: preserve fail-open behavior while never hiding degraded execution.

### Capability: Delegated Specialist Session Experience
Make detached specialist subagents feel native inside Pi sessions without turning them into always-on background workers.

#### Feature: Session-native detached specialist UX
- **Description**: Show current-session specialist activity directly in the Pi runtime with clear active/recent/error states.
- **Inputs**: detached job status, session context, launch origin, recent terminal results.
- **Outputs**: widget/panel/status surfaces, inspect/review actions, and compact summaries.
- **Behavior**: keep specialist visibility session-scoped and operator-friendly rather than fleet-global.

#### Feature: Structured handoff back into the main session
- **Description**: Return detached specialist outcomes as concise structured handoffs rather than raw transcript dumps.
- **Inputs**: terminal step results, evidence paths/refs, fallback/error status.
- **Outputs**: summary, evidence references, suggested next action, and confidence/degradation indicators.
- **Behavior**: minimize context waste while preserving useful evidence.

#### Feature: Session recovery and inspect/cancel flows
- **Description**: Let reopened/reloaded Pi sessions discover stale or recent detached specialist jobs and recover operator visibility.
- **Inputs**: durable registry queries, current session identity, stale/running job state.
- **Outputs**: reviewable recent jobs, stale/interrupted status, cancel/inspect affordances.
- **Behavior**: make the durable registry authoritative; avoid silent loss on extension reload.

### Capability: Global Fleet Visibility in Mission Control
Make detached orchestration visible globally in one place without expanding pulse panes into fleet dashboards.

#### Feature: Mission Control fleet view
- **Description**: Render detached worker activity grouped by project/repo and ownership plane.
- **Inputs**: detached job registry, project identity, queue/runtime health signals.
- **Outputs**: fleet summaries for delegated specialists and Mind workers, plus stale/error/active counts.
- **Behavior**: Mission Control owns the global fleet view; pulse panes do not.

#### Feature: Ownership-aware summaries and drilldown
- **Description**: Distinguish delegated specialist jobs from Mind workers in rollups and drilldowns.
- **Inputs**: `owner_plane`, `worker_kind`, mode, recent results, queue depth.
- **Outputs**: grouped summaries, worker-kind breakdowns, latest terminal states, and bounded drilldown refs.
- **Behavior**: support operator reasoning without conflating specialist UX with Mind runtime behavior.

#### Feature: Local pulse boundary enforcement
- **Description**: Keep per-pane pulse focused on local work, local Mind state, and local health only.
- **Inputs**: current pane/session status, local task/work summary, compact local Mind state.
- **Outputs**: compact pane-local pulse rendering.
- **Behavior**: omit global detached fleet, global observer timeline, and cross-worker orchestration views.

### Capability: Project-Scoped Mind Detached Orchestration
Run Mind detached workers per project/repo, not per Zellij pane.

#### Feature: Project-scoped dispatcher/coordinator
- **Description**: Own Mind detached worker admission through one coordinator per project/repo.
- **Inputs**: project-scoped Mind queues, lease/lock state, runtime guardrails, backlog depth.
- **Outputs**: admission decisions, detached dispatches, reconciled worker state.
- **Behavior**: coordinate by project queue pressure and ownership, not by pane count.

#### Feature: Bounded worker admission policy
- **Description**: Scale Mind detached workers from idle to bounded concurrency based on actual backlog.
- **Inputs**: queue depth, worker kind, retry/dead-letter state, concurrency caps.
- **Outputs**: zero-worker idle state, single-worker light-load state, bounded burst fanout under pressure.
- **Behavior**: default to minimal background overhead; never spawn one permanent worker per pane.

#### Feature: Mind runtime bridge into detached registry
- **Description**: Launch the project-scoped Mind worker slices that already have a clear queue/lease boundary through the shared detached registry without adopting delegated-subagent UX semantics.
- **Inputs**: reflector/T3 queue claims, runtime ownership policy, project store path.
- **Outputs**: detached jobs stamped with `owner_plane=mind` and `worker_kind=t2|t3` for the first shipped slice.
- **Behavior**: preserve queue/lease correctness, fail-open behavior, and project-scoped isolation; keep T1 observer work session-scoped and inline until a real detached admission boundary exists.

### Capability: Reliability, Deduplication, and Restart Safety
Keep detached orchestration safe under multiple panes, multiple sessions, and restarts.

#### Feature: Multi-pane deduplication
- **Description**: Prevent open panes from multiplying equivalent Mind workers for the same project.
- **Inputs**: pane/session topology, project root, queue state, lock/lease state.
- **Outputs**: one effective coordinator/worker set per project according to policy.
- **Behavior**: use queue/lease/project identity to deduplicate work regardless of pane count.

#### Feature: Restart and stale-job reconciliation
- **Description**: Recover gracefully when sessions, wrappers, or workers restart during detached execution.
- **Inputs**: persisted detached jobs, active process checks, stale lease checks, restart time.
- **Outputs**: stale markings, takeover decisions, restart-safe summaries, and visible operator diagnostics.
- **Behavior**: preserve explicit degraded-state visibility instead of pretending work completed cleanly.

#### Feature: Regression coverage for orchestration policy
- **Description**: Verify the intended detached orchestration behavior end to end.
- **Inputs**: fixture projects, queue seeds, multi-session/multi-pane scenarios, forced restarts.
- **Outputs**: passing regression suites for idle/light/burst/restart/dedup cases.
- **Behavior**: assert project-scoped worker behavior, bounded concurrency, and clean surface separation.

---

## Repository Structure

```text
project-root/
├── .pi/
│   ├── agents/
│   │   ├── *.md
│   │   ├── teams.yaml
│   │   └── agent-chain.yaml
│   └── extensions/
│       └── subagent.ts
├── crates/
│   ├── aoc-core/
│   │   └── src/insight_contracts.rs
│   ├── aoc-storage/
│   │   └── src/lib.rs
│   ├── aoc-agent-wrap-rs/
│   │   └── src/
│   │       ├── main.rs
│   │       └── insight_orchestrator.rs
│   ├── aoc-mind/
│   │   └── src/
│   │       ├── reflector_runtime.rs
│   │       └── t3_runtime.rs
│   ├── aoc-hub-rs/
│   │   └── src/pulse_uds.rs
│   └── aoc-mission-control/
│       └── src/main.rs
├── docs/
│   ├── insight-subagent-orchestration.md
│   ├── mission-control.md
│   ├── mission-control-ops.md
│   └── detached-orchestration.md
└── .taskmaster/docs/prds/
    ├── aoc_detached_orchestration_prd_rpg.md
    ├── task-169_aoc_detached_pi_subagent_runtime_prd_rpg.md
    ├── aoc-session-overseer_prd_rpg.md
    └── aoc_mind_memory_pipeline_prd_rpg.md
```

## Module Definitions

### Module: `crates/aoc-core/src/insight_contracts.rs`
- **Maps to capability**: Shared Detached Control Plane
- **Responsibility**: Typed detached request/status/result contracts and ownership metadata.
- **Exports**:
  - detached ownership and worker-kind enums
  - detached dispatch/status/cancel envelopes
  - shared lifecycle state typing

### Module: `crates/aoc-storage/src/lib.rs`
- **Maps to capability**: Shared Detached Control Plane + restart recovery
- **Responsibility**: Durable detached registry persistence, query, and stale reconciliation helpers.
- **Exports**:
  - detached job upsert/list helpers
  - stale job marking / reconciliation helpers
  - ownership-aware detached queries

### Module: `.pi/extensions/subagent.ts`
- **Maps to capability**: Delegated Specialist Session Experience
- **Responsibility**: Session-facing specialist launch UX, command/tool entry points, and result presentation.
- **Exports**:
  - dispatch/status/cancel commands and tool
  - specialist launch helpers
  - session widget/status rendering

### Module: `crates/aoc-agent-wrap-rs/src/insight_orchestrator.rs`
- **Maps to capability**: Shared Detached Control Plane + Mind runtime bridge
- **Responsibility**: Detached job execution bridge, bounded concurrency, persistence, and ownership-aware orchestration.
- **Exports**:
  - detached dispatch/status/cancel runtime
  - bounded fanout / cancellation helpers
  - ownership-aware persistence bridge

### Module: `crates/aoc-agent-wrap-rs/src/main.rs`
- **Maps to capability**: Global Fleet Visibility + Project-Scoped Mind Detached Orchestration
- **Responsibility**: Publish detached status into Pulse, bridge Mind runtime triggers, and host dispatcher/control-loop integration.
- **Exports**:
  - Pulse detached status updates
  - Mind queue/runtime bridge hooks
  - local runtime orchestration loop wiring

### Module: `crates/aoc-mind/src/reflector_runtime.rs` + `t3_runtime.rs`
- **Maps to capability**: Project-Scoped Mind Detached Orchestration
- **Responsibility**: Queue/lease-safe Mind worker execution behind project-scoped dispatcher policy.
- **Exports**:
  - reflector and T3 worker tick/runtime helpers
  - lease/claim/report contracts

### Module: `crates/aoc-mission-control/src/main.rs`
- **Maps to capability**: Global Fleet Visibility in Mission Control
- **Responsibility**: Dedicated detached fleet summaries, ownership-aware drilldowns, and pulse-pane boundary enforcement.
- **Exports**:
  - Mission Control fleet rendering
  - pulse-pane local-only rendering path
  - detached summary/drilldown helpers

### Module: `docs/detached-orchestration.md`
- **Maps to capability**: Whole-system operator/runtime alignment
- **Responsibility**: Human-readable architecture and policy reference for detached orchestration across delegated and Mind ownership concerns.

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies - these are built first.

- **Detached contract types**: shared lifecycle states, ownership metadata, and detached request/result/status envelopes.
- **Durable detached registry**: persistent storage and ownership-aware query surface.
- **Mission Control / pulse-pane boundary policy**: dedicated global fleet surface versus local-only pulse rendering.

### Delegated Specialist Layer (Phase 1)
- **Pi detached specialist runtime**: Depends on [Detached contract types, Durable detached registry]
- **Pi session-native detached UX**: Depends on [Pi detached specialist runtime, Durable detached registry]
- **Structured specialist handoff/recovery**: Depends on [Pi session-native detached UX, Durable detached registry]

### Global Fleet Surface Layer (Phase 2)
- **Mission Control detached fleet view**: Depends on [Detached contract types, Durable detached registry, Mission Control / pulse-pane boundary policy]
- **Ownership-aware summaries and drilldowns**: Depends on [Mission Control detached fleet view]

### Mind Dispatcher Layer (Phase 3)
- **Project-scoped Mind dispatcher/coordinator**: Depends on [Detached contract types, Durable detached registry]
- **Mind detached metadata stamping**: Depends on [Project-scoped Mind dispatcher/coordinator]
- **Reflector/T3 runtime bridge into detached jobs**: Depends on [Project-scoped Mind dispatcher/coordinator, Mind detached metadata stamping]
- **Bounded worker admission policy**: Depends on [Project-scoped Mind dispatcher/coordinator, Durable detached registry]

### Reliability Layer (Phase 4)
- **Restart and stale-job reconciliation**: Depends on [Pi detached specialist runtime, Project-scoped Mind dispatcher/coordinator, Durable detached registry]
- **Multi-pane deduplication**: Depends on [Project-scoped Mind dispatcher/coordinator, Bounded worker admission policy]
- **Detached orchestration regression suite**: Depends on [Mission Control detached fleet view, Reflector/T3 runtime bridge into detached jobs, Restart and stale-job reconciliation, Multi-pane deduplication]

---

## Implementation Roadmap

### Phase 1: Consolidate detached substrate and session UX
**Goal**: finish delegated specialist runtime productization on top of the shared substrate.

- [ ] Complete Pi session-native detached specialist UX.
- [ ] Make the durable detached registry the source of truth for session status/recovery.
- [ ] Improve structured specialist result handoff and inspect/review flows.

**Exit Criteria**: delegated specialists feel native in Pi sessions without inventing a separate lifecycle model.

### Phase 2: Build the global Mission Control fleet surface
**Goal**: make detached fleet visibility global only where it belongs.

- [ ] Add dedicated Mission Control fleet view grouped by project/repo and ownership plane.
- [ ] Preserve pulse-pane local-only rendering and omit global fleet details from normal work panes.
- [ ] Add ownership-aware rollups and drilldown refs.

**Exit Criteria**: operators can inspect detached fleet activity in Mission Control without polluting per-pane pulse.

### Phase 3: Introduce project-scoped Mind dispatcher orchestration
**Goal**: move Mind detached work toward project-scoped, queue-driven orchestration.

- [ ] Define dispatcher ownership, queue watch policy, and worker admission model.
- [ ] Stamp Mind detached jobs with ownership metadata and worker kinds.
- [ ] Bridge reflector and T3 runtime launches through the detached dispatcher.
- [ ] Enforce bounded worker scaling by project backlog rather than pane count.

**Exit Criteria**: Mind detached work is project-scoped and queue-driven rather than pane-coupled.

### Phase 4: Harden restart, dedup, and rollout safety
**Goal**: make the combined detached system safe to ship.

- [ ] Add restart reconciliation and stale-job recovery coverage.
- [ ] Add multi-pane/multi-session dedup coverage.
- [ ] Validate idle/light/burst worker admission policy.
- [ ] Publish operator/runtime documentation for the full detached orchestration model.

**Exit Criteria**: detached orchestration is restart-safe, deduplicated, and operationally documented.

---

## Task Mapping

This umbrella PRD maps to the detached-orchestration Taskmaster tag and currently aligns with:
- **169** — detached Pi subagent runtime and session-facing delegated specialist UX.
- **149** — Mission Control / pulse-pane boundary and dedicated global fleet visibility.
- **178** — project-scoped Mind detached dispatcher and bounded worker orchestration.

Domain PRDs remain authoritative for detailed local requirements:
- `task-169_aoc_detached_pi_subagent_runtime_prd_rpg.md`
- `aoc-session-overseer_prd_rpg.md`
- `aoc_mind_memory_pipeline_prd_rpg.md`

---

## Risks and Mitigations

- **Risk**: delegated specialist UX and Mind worker orchestration drift into separate incompatible runtime models.
  - **Mitigation**: keep one detached contract/registry substrate and enforce ownership metadata from dispatch onward.

- **Risk**: Mind workers multiply with pane count and create unnecessary CPU churn.
  - **Mitigation**: make project-scoped dispatcher policy explicit; cover multi-pane dedup in regression tests.

- **Risk**: Mission Control and pulse panes re-converge into duplicated global surfaces.
  - **Mitigation**: keep dedicated fleet view only in Mission Control and test pulse-pane boundary behavior.

- **Risk**: extension-local session state becomes a competing source of truth.
  - **Mitigation**: require durable registry queries for status/recovery and treat extension state as UX cache only.

---

## Open Questions
- Should the initial project-scoped Mind dispatcher live inside wrapper runtime, hub-side coordination, or a later dedicated service boundary?
- What is the exact admission policy for T1 versus T2 versus T3 under mixed backlog pressure?
- How much of the detached fleet drilldown should be interactive in Mission Control versus CLI/tool-based?
- Should `owner_plane` remain the final field name long-term, or should naming be revisited after the architecture stabilizes?
