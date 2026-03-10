# AOC Session Overseer PRD (RPG)

## Problem Statement
AOC already gives each agent tab strong local isolation and basic Pulse visibility, but it does not yet provide a first-class way for one manager agent and the directing developer to understand whether parallel worker tabs are making plan-aligned progress in real time.

Today this creates several operational failures:
- developers cannot quickly distinguish genuine progress from busy-looking churn,
- worker tabs can drift into hyperfocus without surfacing blockers or misalignment early,
- parallel agents can duplicate effort or edit overlapping files without a shared supervisory view,
- handoffs and steering depend on ad-hoc chat review instead of structured, session-scoped status,
- existing Pulse/runtime signals show liveness and some summaries, but not enough manager-grade intent, progress, blocker, and command lifecycle context.

We need a session-scoped overseer control plane that lets AOC publish structured worker progress, derive manager-facing status and drift signals, expose a live session snapshot to Mission Control and manager agents, and route safe steering commands back to worker tabs through the existing Pulse hub path.

## Target Users
- **Directing developer/operator**: runs an AOC session with several parallel worker tabs and needs one reliable place to see who is doing what, who is blocked, and where intervention is needed.
- **Manager agent / planner tab**: consumes a machine-readable view of the whole session, compares live work against the plan, and proposes or issues steering actions.
- **Worker agents**: publish concise structured updates, receive bounded steering commands, and hand off progress without requiring transcript scraping.
- **AOC maintainers**: need a robust architecture that works across existing Pulse, Mind, wrapper, and Mission Control components without brittle terminal scraping.

## Success Metrics
- Manager snapshot is available for every live session with >= 95% of active worker panes represented within 5 seconds of startup or reconnect.
- >= 80% of worker sessions in pilot tags emit structured progress updates containing task, status, summary, and last update metadata.
- Mission Control can surface blocked/stale/drifting worker rows with < 2 seconds median event-to-render latency.
- Duplicate-work incidents (same file or task focus across workers without explicit coordination) are detected and surfaced in >= 80% of seeded test scenarios.
- Steering commands (`request_status_update`, `request_handoff`, `pause_and_summarize`, `run_validation`) complete with explicit accepted/terminal status and zero cross-session routing leaks.
- The overseer path remains fail-open: worker sessions continue functioning normally if observer enrichment or manager snapshot logic is unavailable.

---

## Capability Tree

### Capability: Worker Progress Publishing
Standardize how each worker tab reports meaningful progress.

#### Feature: Structured progress event contract
- **Description**: Define a canonical observer event schema for worker lifecycle, progress, blockers, and summaries.
- **Inputs**: Session metadata, pane identity, task context, wrapper/runtime activity, optional agent-declared summary.
- **Outputs**: Validated `observer_event` envelopes and latest `worker_snapshot` records.
- **Behavior**: Normalize identity to `session::pane`, accept partial updates, validate required fields, and preserve backward compatibility with existing Pulse state flow.

#### Feature: Wrapper-side event emission
- **Description**: Extend the wrapper/runtime path to publish structured worker updates at startup, milestones, blockers, and completion.
- **Inputs**: Agent process lifecycle, git/task signals, explicit progress hooks, idle/heartbeat timers.
- **Outputs**: Observer events attached to Pulse publisher traffic.
- **Behavior**: Debounce noisy updates, emit on meaningful transitions, and keep worker overhead low.

#### Feature: Lightweight periodic status refresh
- **Description**: Request or auto-produce concise heartbeat summaries so stale workers are visible before they silently drift.
- **Inputs**: Last update time, idle timer, manager command requests.
- **Outputs**: Refreshed worker summary with status age and reason.
- **Behavior**: Prefer structured refreshes over transcript scraping; fail open if the refresh cannot be generated.

### Capability: Session Overseer State Aggregation
Aggregate raw worker signals into a manager-usable session model.

#### Feature: Session observer snapshot
- **Description**: Maintain latest per-worker overseer state within the Pulse hub/session cache.
- **Inputs**: Observer events, existing agent state, layout watcher topology, command results.
- **Outputs**: Session-wide `observer_snapshot` with per-worker rows and timeline entries.
- **Behavior**: Merge latest worker state, evict closed panes safely, and expose reconnect-safe snapshots to subscribers.

#### Feature: Derived attention heuristics
- **Description**: Compute signals that distinguish productive motion from drift, staleness, or duplicate work.
- **Inputs**: Last meaningful update time, repeated command patterns, file overlap, task overlap, validation outcomes, blocker flags.
- **Outputs**: `plan_alignment`, `drift_risk`, `attention_needed`, and `duplicate_work` indicators.
- **Behavior**: Use deterministic heuristics first; leave semantic enrichment optional.

#### Feature: Timeline and provenance retention
- **Description**: Keep a bounded event timeline for recent worker transitions and steering actions.
- **Inputs**: Observer events, manager commands, command results.
- **Outputs**: Ordered recent timeline and provenance metadata.
- **Behavior**: Retain newest-first bounded history, include event source (`wrapper`, `hub`, `mind`, `manager`), and survive client reconnects.

### Capability: Manager Visibility and Planning Surfaces
Expose session state to humans and manager agents.

#### Feature: Mission Control overseer view
- **Description**: Add a manager-focused Mission Control mode for all workers in the current session.
- **Inputs**: Observer snapshot, timeline, active task/tag context.
- **Outputs**: Compact list of workers with task, status, blocker, age, drift, and overlap indicators.
- **Behavior**: Sort blocked/stale/drifting workers first, support session/tab filters, and preserve compact-width readability.

#### Feature: Machine-readable session snapshot command
- **Description**: Provide a CLI/API command that returns the current overseer snapshot for manager agents and scripts.
- **Inputs**: Current session context and optional output format flags.
- **Outputs**: JSON and human-readable snapshot payloads.
- **Behavior**: Read from Pulse/hub when available, degrade gracefully with clear provenance.

#### Feature: Plan alignment view
- **Description**: Correlate worker activity with Taskmaster tag/task intent so the manager can steer against the actual plan.
- **Inputs**: Active tag, assigned task IDs, task metadata, worker snapshots.
- **Outputs**: Row-level plan linkage, missing-assignment warnings, and unowned work gaps.
- **Behavior**: Highlight workers without assigned tasks, tasks with no active owner, and conflicting ownership.

### Capability: Steering and Coordination Commands
Allow safe intervention without raw pane scraping.

#### Feature: Manager-to-worker command contract
- **Description**: Define typed commands for bounded steering actions.
- **Inputs**: Manager intent, target worker id, optional arguments.
- **Outputs**: `command` and `command_result` envelopes with accepted/terminal states.
- **Behavior**: Route through Pulse hub; reject cross-session targets; log all command attempts.

#### Feature: Worker steering handlers
- **Description**: Implement wrapper/runtime handlers for overseer commands.
- **Inputs**: `request_status_update`, `request_handoff`, `pause_and_summarize`, `run_validation`, `switch_focus`, `finalize_and_report`.
- **Outputs**: Local action execution plus result event(s).
- **Behavior**: Prefer safe, explainable actions; require human confirmation for destructive or high-risk operations.

#### Feature: Developer-in-the-loop control policy
- **Description**: Ensure manager automation assists rather than silently takes over.
- **Inputs**: User settings, command type, current worker state.
- **Outputs**: Policy decisions (`allow`, `confirm_required`, `deny`).
- **Behavior**: Default to observation and nudges first; log rationale for denied or escalated commands.

### Capability: Optional Mind/Sidecar Enrichment
Use existing Mind infrastructure to improve overseer quality without making it mandatory.

#### Feature: Observer enrichment adapter
- **Description**: Allow Mind/T1 observer outputs to enrich worker snapshots with confidence, summarized evidence, or anomaly notes.
- **Inputs**: T1/T2 artifacts, observer events, provenance metadata.
- **Outputs**: Enriched overseer fields and badges in UI/API.
- **Behavior**: Merge enrichment only when available; never block core overseer flow.

#### Feature: Drift/anomaly explanation hints
- **Description**: Attach concise reason strings when heuristics or semantic observers mark a worker as stale, drifting, or duplicative.
- **Inputs**: Derived heuristics and optional semantic signals.
- **Outputs**: Human-readable explanation hints.
- **Behavior**: Keep explanations evidence-backed and bounded in length.

---

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-core/
│   │   └── src/                        # Shared overseer types + Pulse schema extensions
│   ├── aoc-agent-wrap-rs/
│   │   └── src/                        # Worker event emission + command handlers
│   ├── aoc-hub-rs/
│   │   └── src/                        # Observer snapshot cache, routing, retention
│   ├── aoc-mission-control/
│   │   └── src/                        # Overseer dashboard/view model
│   ├── aoc-mind/
│   │   └── src/                        # Optional enrichment adapter / derived signals
│   └── aoc-cli/
│       └── src/                        # Snapshot CLI / manager-facing commands
├── bin/
│   └── aoc-session-overseer            # Optional thin CLI wrapper over Rust command
├── docs/
│   ├── mission-control.md
│   ├── pulse-ipc-protocol.md
│   └── session-overseer.md             # New operator and architecture guide
└── tests/
    └── (integration fixtures / smoke scripts)
```

## Module Definitions

### Module: `crates/aoc-core/src/*`
- **Maps to capability**: Structured progress event contract; manager-to-worker command contract
- **Responsibility**: Define canonical observer types, topic identifiers, command enums, and serialization contracts shared across wrapper, hub, UI, and CLI.
- **Exports**:
  - `ObserverEvent`
  - `WorkerSnapshot`
  - `ObserverSnapshot`
  - `ObserverTimelineEntry`
  - `ManagerCommand`
  - `ManagerCommandResult`

### Module: `crates/aoc-agent-wrap-rs/src/*`
- **Maps to capability**: Wrapper-side event emission; worker steering handlers; lightweight periodic status refresh
- **Responsibility**: Produce worker observer events and execute safe command handlers against the running worker session.
- **Exports/behaviors**:
  - progress emitter hooks
  - summary refresh path
  - command handling for bounded steering verbs

### Module: `crates/aoc-hub-rs/src/pulse_uds.rs` and related files
- **Maps to capability**: Session observer snapshot; timeline retention; command routing
- **Responsibility**: Aggregate session-scoped overseer state, broadcast snapshot/delta updates, enforce session isolation, and route commands/results.
- **Exports/behaviors**:
  - observer topic subscription support
  - latest snapshot cache
  - timeline retention policy
  - session-safe command validation

### Module: `crates/aoc-mission-control/src/*`
- **Maps to capability**: Mission Control overseer view; plan alignment view
- **Responsibility**: Render manager-facing worker status, attention ordering, timeline, and steering affordances.
- **Exports/behaviors**:
  - overseer pane/view mode
  - row presenter with badges/chips
  - optional command dispatch actions

### Module: `crates/aoc-cli/src/*` and `bin/aoc-session-overseer`
- **Maps to capability**: Machine-readable session snapshot command
- **Responsibility**: Provide CLI access for manager agents, scripts, and debugging.
- **Exports/commands**:
  - `aoc-session-overseer snapshot --json`
  - `aoc-session-overseer timeline`
  - optional `aoc-session-overseer command <verb> --target <agent>`

### Module: `crates/aoc-mind/src/*`
- **Maps to capability**: Observer enrichment adapter; drift/anomaly explanation hints
- **Responsibility**: Optionally enrich overseer rows with semantic confidence and evidence-backed explanation strings.
- **Exports/behaviors**:
  - enrichment adapter from Mind observer outputs
  - provenance-aware merge helpers

### Module: `docs/session-overseer.md` and existing docs
- **Maps to capability**: Developer-in-the-loop policy; operator visibility
- **Responsibility**: Document the contract, operator workflow, rollout, and safety semantics.

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies - these contracts must exist first.

- **overseer-types**: Shared observer event, snapshot, timeline, and manager command schemas.
- **policy-rules**: Steering safety policy, retention defaults, and field semantics.

### Publisher Layer (Phase 1)
- **worker-emitter**: Depends on [overseer-types, policy-rules]
- **worker-refresh**: Depends on [overseer-types, policy-rules]

### Aggregation Layer (Phase 2)
- **hub-observer-cache**: Depends on [overseer-types, worker-emitter]
- **hub-command-routing**: Depends on [overseer-types, policy-rules, worker-emitter]
- **timeline-retention**: Depends on [overseer-types, hub-observer-cache]

### Intelligence Layer (Phase 3)
- **attention-heuristics**: Depends on [hub-observer-cache, timeline-retention, policy-rules]
- **plan-alignment-adapter**: Depends on [hub-observer-cache, attention-heuristics]
- **mind-enrichment-adapter**: Depends on [hub-observer-cache, attention-heuristics]

### Presentation Layer (Phase 4)
- **mission-control-overseer-view**: Depends on [hub-observer-cache, attention-heuristics, plan-alignment-adapter]
- **snapshot-cli**: Depends on [hub-observer-cache, attention-heuristics]

### Control Layer (Phase 5)
- **manager-command-ui**: Depends on [mission-control-overseer-view, hub-command-routing, policy-rules]
- **manager-agent-consumption**: Depends on [snapshot-cli, plan-alignment-adapter, hub-command-routing]

### Hardening + Rollout Layer (Phase 6)
- **integration-tests-and-smokes**: Depends on [mission-control-overseer-view, snapshot-cli, manager-command-ui, mind-enrichment-adapter]
- **docs-and-rollout**: Depends on [integration-tests-and-smokes]

---

## Development Phases

### Phase 0: Contract and Safety Foundation
**Goal**: Lock the overseer schema, command contract, and safety policy.

**Entry Criteria**: Scope approved and tag/epic created.

**Tasks**:
- [ ] Define `ObserverEvent`, `WorkerSnapshot`, `ObserverSnapshot`, timeline entry, and `ManagerCommand` schemas. (depends on: none)
  - Acceptance criteria: all fields, enums, and topics are documented and compile in shared types.
  - Test strategy: serde round-trip tests and backward-compat parsing tests.
- [ ] Define steering policy and retention defaults. (depends on: none)
  - Acceptance criteria: command allow/confirm/deny matrix and timeline retention behavior documented.
  - Test strategy: unit tests for policy evaluation and retention pruning.

**Exit Criteria**: Shared contract and policy are stable enough for wrapper and hub work to proceed independently.

**Delivers**: Implementation-ready overseer contract.

---

### Phase 1: Worker Publishing Baseline
**Goal**: Make worker tabs publish meaningful structured progress.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Implement wrapper progress emission hooks for startup, task start, blocker, milestone, idle, and completion. (depends on: [Phase 0 contract])
  - Acceptance criteria: worker updates appear as structured events with task, status, summary, and timestamps.
  - Test strategy: wrapper unit tests and fixture-driven publisher integration tests.
- [ ] Implement summary refresh path and stale update policy. (depends on: [Phase 0 policy])
  - Acceptance criteria: workers can respond to status-refresh requests or emit periodic lightweight updates.
  - Test strategy: timed integration tests for debounce and refresh behavior.

**Exit Criteria**: A live worker can publish useful overseer events without any Mission Control changes.

**Delivers**: Structured worker state stream.

---

### Phase 2: Hub Snapshot and Timeline
**Goal**: Aggregate worker events into a reconnect-safe session model.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Extend Pulse hub subscriptions with overseer snapshot/delta support. (depends on: [Phase 1 worker publishing])
  - Acceptance criteria: subscribers receive latest per-worker snapshot and delta updates.
  - Test strategy: hub integration tests for snapshot on subscribe, delta propagation, and session rejection.
- [ ] Add bounded timeline retention and pane-closure eviction semantics. (depends on: [hub snapshot support])
  - Acceptance criteria: closed panes are removed safely while history remains bounded and ordered.
  - Test strategy: layout-churn and stale-pane tests.
- [ ] Route manager commands and command results through the same session-safe path. (depends on: [Phase 0 contract, Phase 1 worker publishing])
  - Acceptance criteria: valid commands reach only target workers in the same session and results are observable.
  - Test strategy: command acceptance/rejection/idempotency tests.

**Exit Criteria**: The hub exposes a stable overseer state model and can route bounded commands.

**Delivers**: Session-scoped overseer data plane.

---

### Phase 3: Attention Heuristics and Plan Correlation
**Goal**: Turn raw state into manager-grade signals.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Implement deterministic stale/drift/duplicate-work heuristics. (depends on: [Phase 2 hub snapshot])
  - Acceptance criteria: attention flags populate from evidence-backed rules.
  - Test strategy: seeded fixtures for command loops, stale workers, duplicate file/task overlap.
- [ ] Correlate worker state with Taskmaster tag/task intent. (depends on: [hub snapshot, heuristics])
  - Acceptance criteria: workers show assigned task linkage and missing/conflicting ownership is surfaced.
  - Test strategy: task-assignment fixtures and taskless-worker cases.
- [ ] Add optional Mind enrichment adapter. (depends on: [hub snapshot, heuristics])
  - Acceptance criteria: semantic enrichment augments rows when present but never blocks base rendering.
  - Test strategy: provenance merge tests and fail-open tests when Mind data is absent.

**Exit Criteria**: Manager surfaces can distinguish active, blocked, stale, drifting, and conflicting work.

**Delivers**: Actionable overseer intelligence.

---

### Phase 4: Mission Control Overseer UX
**Goal**: Give the developer a compact live dashboard for multi-agent oversight.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Build Mission Control overseer view with row ordering and badges. (depends on: [Phase 3 heuristics, plan correlation])
  - Acceptance criteria: each worker row shows task, summary, age, blocker/drift/overlap chips, and provenance.
  - Test strategy: UI presenter tests and integration tests for sorting/filtering.
- [ ] Add timeline/detail drawer for recent worker transitions and steering events. (depends on: [Phase 2 timeline])
  - Acceptance criteria: operator can inspect recent meaningful activity without leaving Mission Control.
  - Test strategy: timeline rendering tests and reconnect-state tests.
- [ ] Add optional steering actions with clear confirmation semantics. (depends on: [Phase 2 command routing])
  - Acceptance criteria: supported commands can be triggered from UI with confirmation where required.
  - Test strategy: action dispatch tests and confirmation-path tests.

**Exit Criteria**: Mission Control acts as the primary human oversight surface.

**Delivers**: Human-facing overseer dashboard.

---

### Phase 5: Manager-Agent CLI and Automation Hooks
**Goal**: Let a manager agent consume overseer state and steer workers programmatically.

**Entry Criteria**: Phase 4 complete.

**Tasks**:
- [ ] Implement `aoc-session-overseer snapshot` and timeline commands. (depends on: [Phase 2 hub snapshot, Phase 3 heuristics])
  - Acceptance criteria: manager agents can read JSON snapshots with stable schema and provenance.
  - Test strategy: CLI output contract tests and no-hub fallback tests.
- [ ] Implement safe command dispatch CLI for manager agents and scripts. (depends on: [Phase 2 command routing, Phase 4 policy])
  - Acceptance criteria: manager agents can request refresh/handoff/validation with explicit results.
  - Test strategy: end-to-end command route tests.
- [ ] Document the recommended manager-agent workflow. (depends on: [snapshot CLI, command CLI])
  - Acceptance criteria: docs explain how the human, manager tab, and worker tabs cooperate.
  - Test strategy: smoke checklist for live sessions.

**Exit Criteria**: A manager agent can oversee the session without reading raw worker transcripts.

**Delivers**: Manager automation interface.

---

### Phase 6: Hardening, Rollout, and Pilot Validation
**Goal**: Prove the overseer system is safe, fast, and useful in real multi-agent sessions.

**Entry Criteria**: Phase 5 complete.

**Tasks**:
- [ ] Add multi-worker integration fixtures and churn/resume tests. (depends on: [all prior phases])
  - Acceptance criteria: tab churn, reconnects, command routing, and stale-eviction scenarios are covered.
  - Test strategy: cargo integration tests and scripted session smoke runs.
- [ ] Validate fail-open behavior under missing Mind/pulse partial outages. (depends on: [all prior phases])
  - Acceptance criteria: workers continue operating and basic visibility remains available under degraded conditions.
  - Test strategy: fault-injection tests and manual smoke cases.
- [ ] Publish rollout guide and pilot checklist for target tags. (depends on: [test completion])
  - Acceptance criteria: documented enablement, rollback, and operator guidance exists.
  - Test strategy: doc-driven dry run in a live session.

**Exit Criteria**: Overseer can be enabled for pilot workstreams with confidence.

**Delivers**: Production-ready rollout plan.

---

## Test Pyramid

```text
        /\
       /E2E\       ← 10%
      /-----\
     / Int  \      ← 35%
    /--------\
   /  Unit    \    ← 55%
  /------------\
```

## Coverage Requirements
- Line coverage: 85% minimum on touched modules
- Branch coverage: 75% minimum on touched modules
- Function coverage: 85% minimum on touched modules
- Statement coverage: 85% minimum on touched modules

## Critical Test Scenarios

### Shared overseer contract
**Happy path**:
- Observer events, snapshots, and commands serialize/deserialize across wrapper, hub, UI, and CLI.
- Expected: stable JSON shape and topic compatibility.

**Edge cases**:
- Partial worker updates omit optional fields.
- Expected: merge behavior preserves prior valid state and defaults.

**Error cases**:
- Unknown command or malformed payload arrives.
- Expected: explicit validation error without corrupting hub state.

**Integration points**:
- Existing Pulse message flow coexists with new overseer traffic.
- Expected: legacy consumers continue working.

### Worker event emission
**Happy path**:
- Worker starts a task, hits a milestone, and completes.
- Expected: ordered events produce correct latest snapshot and timeline.

**Edge cases**:
- Very frequent file/task changes occur.
- Expected: debounce limits noise while preserving last meaningful update.

**Error cases**:
- Worker cannot generate summary refresh.
- Expected: fail-open heartbeat/state still publishes with degradation reason.

**Integration points**:
- Wrapper emits observer events alongside existing agent state and heartbeats.
- Expected: no duplicate identity collisions.

### Hub aggregation and command routing
**Happy path**:
- Subscriber connects mid-session and receives a full overseer snapshot.
- Expected: all live workers represented with correct ages/status.

**Edge cases**:
- Pane closes and later pane id is reused.
- Expected: stale worker is evicted and new worker state is not polluted.

**Error cases**:
- Cross-session target command is attempted.
- Expected: command rejected with explicit error result.

**Integration points**:
- Layout watcher, observer cache, and command results all update the same session model coherently.
- Expected: no ghost workers after churn.

### Attention heuristics and plan correlation
**Happy path**:
- Blocked worker and duplicate file overlap are detected.
- Expected: attention flags and explanation hints surface correctly.

**Edge cases**:
- Worker has no assigned task but is actively editing.
- Expected: row marked unassigned rather than misclassified as drifting by default.

**Error cases**:
- Taskmaster state unavailable temporarily.
- Expected: snapshot degrades with clear provenance rather than failing entirely.

**Integration points**:
- Mind enrichment merges into heuristic rows.
- Expected: provenance badges indicate deterministic vs semantic source.

### Mission Control and CLI
**Happy path**:
- Overseer rows render in priority order and snapshot CLI returns the same underlying state.
- Expected: consistency between UI and machine-readable output.

**Edge cases**:
- Compact-width session with many workers.
- Expected: row compaction still preserves task, status, and highest-priority attention signal.

**Error cases**:
- Hub is unreachable.
- Expected: CLI/UI show clear degradation and fallback behavior where supported.

**Integration points**:
- UI command action updates timeline and worker row state.
- Expected: accepted and terminal command states are visible to the operator.

## Test Generation Guidelines
- Favor deterministic fixtures over transcript-based assertions.
- Treat `session::pane` identity correctness as a non-negotiable invariant.
- Cover layout churn, stale worker eviction, reconnects, and duplicate-work detection explicitly.
- Keep semantic Mind enrichment optional in tests; base overseer functionality must pass without it.
- Add regression tests for cross-session rejection and idempotent command handling.

---

## Architecture

## System Components
- **Worker runtime plane**: `aoc-agent-wrap-rs` emits observer events and handles bounded commands.
- **Session hub plane**: `aoc-hub-rs` aggregates worker snapshots, timeline entries, and command results under session isolation.
- **Presentation plane**: `aoc-mission-control` renders a developer-facing overseer dashboard.
- **Automation plane**: `aoc-cli` / `aoc-session-overseer` exposes machine-readable snapshots and command dispatch to manager agents.
- **Enrichment plane**: `aoc-mind` optionally provides semantic evidence/confidence overlays.

## Data Models
- **ObserverEvent**: append-style worker progress/blocker/milestone event with structured fields.
- **WorkerSnapshot**: latest merged state for a single worker in the current session.
- **ObserverSnapshot**: full session state containing worker rows, timeline summary, and health/provenance metadata.
- **ManagerCommand**: typed steering request constrained to a safe allowlist.
- **ManagerCommandResult**: accepted/terminal result payload with status, message, and request correlation.

## Technology Stack
- **Language**: Rust for shared types, wrapper, hub, Mission Control, and CLI.
- **Transport**: Existing Pulse UDS NDJSON protocol, extended with overseer topics/payloads.
- **UI**: Ratatui/crossterm Mission Control surfaces.
- **Task context**: Existing Taskmaster CLI/state access for plan correlation.
- **Optional semantics**: AOC Mind T1/T2 outputs as enrichment only.

**Decision: Use Pulse/hub as authoritative overseer bus**
- **Rationale**: Existing session-scoped transport, identity model, and routing logic already solve session isolation and reconnect-safe distribution.
- **Trade-offs**: Requires extending shared contracts and hub state instead of a quick local-only script.
- **Alternatives considered**: direct pane scraping, tmux/zellij capture, per-client polling.

**Decision: Prefer structured worker events over transcript scraping**
- **Rationale**: Structured events are cheaper, more reliable, and easier to reason about than raw terminal output.
- **Trade-offs**: Requires wrapper/runtime instrumentation and a concise event contract.
- **Alternatives considered**: raw scrollback parsing, shell history heuristics only.

**Decision: Deterministic heuristics first, semantic enrichment second**
- **Rationale**: Core oversight must remain fast, explainable, and fail-open even when Mind sidecars are unavailable.
- **Trade-offs**: Early drift detection is simpler and less nuanced until enrichment is added.
- **Alternatives considered**: semantic-only overseer scoring.

**Decision: Human-in-command steering policy by default**
- **Rationale**: The manager agent should assist coordination, not silently take over worker sessions.
- **Trade-offs**: Some automation will require confirmation steps.
- **Alternatives considered**: unrestricted manager command execution.

---

## Risks

## Technical Risks
**Risk**: Observer events become too noisy and degrade hub/UI performance.
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: Debounce at publisher, bounded timeline retention, compact row model, topic-specific subscriptions.
- **Fallback**: Reduce event granularity to milestone/blocker/heartbeat-only mode.

**Risk**: Drift heuristics misclassify legitimate deep work as lack of progress.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: Use evidence-backed heuristics, expose explanation hints, keep scores advisory not authoritative.
- **Fallback**: Ship blocked/stale detection first and gate more speculative drift signals behind rollout flag.

**Risk**: Command routing introduces session safety regressions.
- **Impact**: High
- **Likelihood**: Low
- **Mitigation**: Preserve strict `session::pane` validation, add explicit cross-session rejection tests, log all requests/results.
- **Fallback**: ship observation-only mode first and defer control actions.

**Risk**: Mission Control compact UX becomes overloaded with too many chips and states.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: prioritize a single primary attention signal per row, add detail drawer/timeline for deeper context.
- **Fallback**: keep overseer as an alternate mode rather than replacing existing views immediately.

## Dependency Risks
**Risk**: Taskmaster assignment metadata is incomplete or inconsistent across sessions.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: treat plan correlation as best-effort and surface missing assignment explicitly.
- **Fallback**: allow manual worker-task mapping in pilot workflows.

**Risk**: Mind enrichment contracts are still evolving.
- **Impact**: Medium
- **Likelihood**: High
- **Mitigation**: keep enrichment optional and provenance-tagged; do not block base overseer rollout.
- **Fallback**: ship without Mind integration initially.

## Scope Risks
**Risk**: Attempting full autonomous manager behavior in v1 delays delivery.
- **Impact**: High
- **Likelihood**: High
- **Mitigation**: scope v1 around observation + bounded commands + manager snapshot, then iterate.
- **Fallback**: freeze command set to refresh/handoff/validation only for pilot.

**Risk**: This effort sprawls across multiple tags (mind, mission-control, sub-agents) without a clear epic owner.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: create a dedicated tag and epic, but explicitly reference dependent components in the PRD.
- **Fallback**: stage work through one owning tag with cross-tag notes in task details.

---

## Appendix

## References
- `docs/pulse-ipc-protocol.md`
- `docs/mission-control.md`
- `docs/insight-subagent-orchestration.md`
- `.taskmaster/docs/prds/task-133_agent-task-reflections_prd_rpg.md`
- `.taskmaster/docs/prds/aoc-mind-graph-foundation_prd_rpg.md`

## Glossary
- **Overseer**: the session-scoped management/coordination layer for parallel worker agents.
- **Manager agent**: a planner/coordinator agent that reads overseer state and helps steer workers.
- **Worker agent**: an implementation-focused tab publishing structured progress to the overseer bus.
- **Plan alignment**: whether current worker activity matches the assigned task/tag intent.
- **Drift risk**: evidence-backed indicator that a worker may be stalled or over-focusing off-plan.

## Open Questions
- Should overseer snapshots be a new Pulse topic or encoded within existing snapshot/delta payloads with optional sections?
- Which worker commands should be available in v1 versus gated for later rollout?
- How should worker assignment be sourced initially: explicit Taskmaster field, wrapper env, or both?
- Should Mission Control overseer mode replace the current top-right Pulse mode for pilot tags or coexist as a toggle first?
- What is the minimal event frequency that provides value without making workers noisy?
