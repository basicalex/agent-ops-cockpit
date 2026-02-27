# AOC Insight Sub-Agent Orchestration PRD (RPG)

## Problem Statement
AOC now has live T0/T1 trigger plumbing (ingest, handoff trigger, feed updates), but it still lacks a first-class **sub-agent orchestration architecture** for Insight operations in brown-field repositories.

Current gaps:
- No explicit "Insight" tool contract for controlled agent interaction with T0/T1/T2.
- No supervisor model to run specialist sub-agents with bounded scope and provenance.
- No standardized brown-field bootstrap that compares docs vs code, surfaces gaps, and projects those gaps into Taskmaster + T2 seed jobs.
- T2 detached worker runtime exists as a library capability, but is not yet integrated as a production orchestration loop in session runtime.

We need to formalize and implement **Insight Orchestration** so AOC can run predictable, safe, and observable multi-agent memory workflows while preserving fail-open behavior and operator control.

## Target Users
- AOC maintainers implementing Mind/Insight runtime and tooling.
- Advanced operators running multi-tab, multi-agent sessions in monorepos.
- Contributors onboarding brown-field projects where docs/code drift is significant.

## Success Metrics
- 100% of Insight orchestration actions run through explicit tool contracts (no implicit side channels).
- Brown-field bootstrap produces a gap report, Taskmaster plan draft, and T2 seed set in one guided flow.
- Sub-agent runs expose lifecycle telemetry (queued/running/success/fallback/error) in Mission Control/Pulse.
- Semantic observer/reflector failures preserve deterministic output (fail-open) with provenance.
- T2 worker lock/lease conflict safety remains deterministic under concurrent wrappers.

---

## Capability Tree

### Capability: Insight Naming + Contract Stabilization
Unify observational memory terminology and runtime contracts.

#### Feature: Insight alias contract
- **Description**: Promote "Insight" as the canonical product name for T0/T1/T2 observational memory while preserving backward-compatible `mind_*` feed fields/contracts.
- **Inputs**: existing `mind_observer` payloads, docs, CLI/help text.
- **Outputs**: dual-name compatibility map (`Mind` internal legacy, `Insight` external reference).
- **Behavior**: additive naming in UI/docs/contracts first; no breaking wire change in initial phase.

#### Feature: Trigger taxonomy hardening
- **Description**: Keep explicit trigger semantics for `token_threshold`, `task_completed`, `manual_shortcut`, `handoff`.
- **Inputs**: runtime trigger events from extension/wrapper/task telemetry.
- **Outputs**: normalized trigger events with reason + progress + timestamps.
- **Behavior**: dedupe and prioritize urgent triggers while enforcing singleton active run per session.

### Capability: Insight Supervisor + Sub-Agent Orchestration
Enable operator-controlled specialist delegation for Insight tasks.

#### Feature: Supervisor orchestration modes
- **Description**: Support three orchestration modes: dispatcher, sequential chain, and parallel experts.
- **Inputs**: operator intent + task payload + agent catalog.
- **Outputs**: sub-agent execution plan + lifecycle events.
- **Behavior**: supervisor delegates work, aggregates outputs, emits one synthesized result.

#### Feature: Isolated sub-agent execution
- **Description**: Spawn isolated Pi subprocess workers with scoped tools/system prompts/session files.
- **Inputs**: agent role definition, model profile, prompt payload.
- **Outputs**: streamed partial output + terminal summary + exit code.
- **Behavior**: run workers in bounded background processes; maintain per-agent session continuity when allowed by policy.

#### Feature: Specialist catalogs and teams
- **Description**: Define role-based specialists (scout/planner/builder/reviewer/documenter/red-team + insight experts).
- **Inputs**: `.pi/agents/*.md`, optional team/chain manifests.
- **Outputs**: validated active roster for orchestration.
- **Behavior**: enforce allowlist and per-role tool bounds at dispatch time.

#### Feature: T1 Observer specialist contract
- **Description**: Define strict role contract for `insight-t1-observer`.
- **Inputs**: conversation scope, evidence refs, trigger reason.
- **Outputs**: structured T1 bundle (`summary`, `key_points`, `risks`, `open_questions`, `evidence`, `confidence`).
- **Behavior**: read-only analysis; no code edits; no cross-conversation mixing by default.

#### Feature: T2 Reflector specialist contract
- **Description**: Define strict role contract for `insight-t2-reflector`.
- **Inputs**: one or more T1 bundles in same tag/workstream.
- **Outputs**: structured T2 reflection (`strategic_signals`, `priority_actions`, `taskmaster_projection`, `seed_candidates`, `uncertainty`).
- **Behavior**: read-only synthesis; explicit uncertainty handling; no auto-write mutations unless user-confirmed.

### Capability: Brown-Field Insight Bootstrap
Create a repeatable docs-vs-code analysis and gap plan flow.

#### Feature: docs-vs-code gap scanner
- **Description**: Compare repository documentation claims with implemented code paths/tests.
- **Inputs**: README/PRD/docs, code graph, test metadata.
- **Outputs**: structured gap set (missing, drifted, untested, orphaned).
- **Behavior**: classify gaps with confidence, evidence refs, and blast-radius.

#### Feature: Taskmaster proposal generation
- **Description**: Convert validated gaps into Taskmaster-ready plan drafts by phase/dependency.
- **Inputs**: gap set + existing tag/task context.
- **Outputs**: candidate tasks/subtasks/dependencies and PRD link recommendations.
- **Behavior**: dry-run by default; explicit operator confirmation before task writes.

#### Feature: T2 seed initialization
- **Description**: Project high-ambiguity gaps into T2 reflector queue as seeded reflection jobs.
- **Inputs**: prioritized gap set + active tag/workstream.
- **Outputs**: reflector seed jobs with provenance and retry policy.
- **Behavior**: enqueue only policy-eligible seeds; maintain scope/tag boundaries.

### Capability: Insight Tooling Surface
Provide explicit tools for agents and humans to interact with Insight runtime.

#### Feature: Core Insight tool API
- **Description**: Introduce first-class tools (`insight_status`, `insight_ingest`, `insight_handoff`, `insight_dispatch`, `insight_bootstrap`).
- **Inputs**: command args + conversation/session identity.
- **Outputs**: normalized action results with feed events.
- **Behavior**: all runtime mutations go through typed tool API; reject malformed/unauthorized actions.

#### Feature: Observability + provenance envelope
- **Description**: Persist and expose runtime/provenance metadata for every T1/T2 outcome.
- **Inputs**: distillation results, runtime errors/fallback reasons, timing/cost stats.
- **Outputs**: searchable provenance rows and operator-visible summaries.
- **Behavior**: retain deterministic fallback provenance when semantic stages fail.

### Capability: Runtime Integration of T2 Worker
Move detached reflector from library completeness to session/runtime operation.

#### Feature: Worker loop integration
- **Description**: Integrate detached reflector worker tick loop into active runtime path with lease/file lock safety.
- **Inputs**: queued reflector jobs + runtime guardrails.
- **Outputs**: completed/failed/requeued jobs with heartbeat updates.
- **Behavior**: non-blocking loop with bounded jobs per tick and lock-conflict fail-open behavior.

#### Feature: Runtime health controls
- **Description**: Add controls for enable/disable, diagnostics, and contention reporting.
- **Inputs**: operator commands/env policy.
- **Outputs**: status telemetry and actionable diagnostics.
- **Behavior**: explicit health surface in Mission Control/Pulse without chat-noise spam.

---

## Repository Structure (Target)

```text
project-root/
├── crates/
│   ├── aoc-core/
│   │   └── src/
│   │       ├── mind_contracts.rs
│   │       ├── mind_observer_feed.rs
│   │       └── insight_contracts.rs            # new typed tool/event contracts
│   ├── aoc-mind/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── observer_runtime.rs
│   │       └── reflector_runtime.rs
│   ├── aoc-agent-wrap-rs/
│   │   └── src/
│   │       ├── main.rs
│   │       └── insight_orchestrator.rs         # new supervisor + sub-agent runtime
│   ├── aoc-hub-rs/
│   │   └── src/pulse_uds.rs
│   └── aoc-mission-control/
│       └── src/main.rs
├── .pi/
│   ├── extensions/
│   │   └── minimal.ts
│   └── agents/
│       ├── insight-t1-observer.md               # T1 specialist (read-only observer)
│       ├── insight-t2-reflector.md              # T2 specialist (read-only reflector)
│       ├── teams.yaml                           # optional team map
│       └── agent-chain.yaml                     # optional chain map
├── docs/
│   ├── agents.md
│   └── insight-subagent-orchestration.md        # new architecture reference
└── .taskmaster/docs/prds/
    └── insight-subagent-orchestration_prd_rpg.md
```

## Module Definitions

### Module: `crates/aoc-core/src/insight_contracts.rs` (new)
- **Maps to capability**: Insight Tooling Surface + Naming Contract
- **Responsibility**: Typed command schemas, result envelopes, trigger/reason enums, compatibility aliases.
- **Exports**:
  - `InsightCommand` / `InsightCommandResult`
  - `InsightBootstrapGap`
  - `InsightSeedJob`

### Module: `crates/aoc-agent-wrap-rs/src/insight_orchestrator.rs` (new)
- **Maps to capability**: Supervisor + Sub-Agent Orchestration
- **Responsibility**: Dispatch worker subprocesses, stream lifecycle events, aggregate/summarize results.
- **Exports**:
  - `InsightSupervisor`
  - `dispatch_agent`, `run_chain`, `query_experts` orchestration primitives

### Module: `crates/aoc-mind/src/{observer_runtime,reflector_runtime}.rs`
- **Maps to capability**: Trigger taxonomy + T2 runtime integration
- **Responsibility**: Queue semantics, trigger priority/debounce, detached worker lock/lease processing.

### Module: `crates/aoc-hub-rs/src/pulse_uds.rs`
- **Maps to capability**: Insight tooling transport
- **Responsibility**: Route insight commands/events between extension/subscriber and wrapper publisher.

### Module: `.pi/extensions/minimal.ts`
- **Maps to capability**: Runtime/UI integration
- **Responsibility**: emit ingest/handoff events, show Insight progress state, expose orchestration shortcuts.

### Module: `.pi/agents/insight-t1-observer.md` + `.pi/agents/insight-t2-reflector.md` + manifests
- **Maps to capability**: specialist catalogs + T1/T2 role contracts
- **Responsibility**: define T1/T2 prompt contracts, output schemas, and tool bounds for sub-agent workers.

### Module: `docs/insight-subagent-orchestration.md`
- **Maps to capability**: documentation + reference architecture
- **Responsibility**: canonical operator/maintainer guide for Insight orchestration.

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies.
- **Contract + naming map** (`insight` terminology, compatibility alias policy)
- **Core typed schemas** (`insight_contracts`)

### Runtime Trigger Layer (Phase 1)
- **Live T0 ingest + T1 trigger routing**: Depends on [Foundation]
- **Feed event normalization**: Depends on [Foundation]

### Supervisor/Sub-Agent Layer (Phase 2)
- **Supervisor + worker subprocess orchestration**: Depends on [Phase 1]
- **Role/team/chain manifests + prompt templates**: Depends on [Foundation]

### Brown-Field Bootstrap Layer (Phase 3)
- **docs-vs-code gap scanner**: Depends on [Phase 2]
- **Taskmaster proposal generator**: Depends on [gap scanner, Foundation]
- **T2 seed projection**: Depends on [gap scanner, runtime trigger layer]

### T2 Runtime Productionization Layer (Phase 4)
- **Detached worker runtime loop integration**: Depends on [Phase 1, Phase 3]
- **Health controls/diagnostics**: Depends on [worker loop integration]

### Docs + Rollout Layer (Phase 5)
- **AOC docs publication + runbooks**: Depends on [Phases 1-4]
- **aoc-init seeding updates for insight assets**: Depends on [Phase 2 docs/contracts]

---

## Development Phases

### Phase 0: Insight Contract Baseline
**Goal**: Stabilize terminology and typed contracts.

**Entry Criteria**: Existing mind runtime contracts compile and tests pass.

**Tasks**:
- [ ] Define `insight` naming/alias policy (`mind_*` compatibility retained).
  - Acceptance criteria: docs + code comments define non-breaking alias policy.
  - Test strategy: contract serialization/deserialization compatibility tests.
- [ ] Add typed insight command/result schemas in `aoc-core`.
  - Acceptance criteria: command envelopes validated centrally.
  - Test strategy: unit tests for enum/value parsing and invalid payload rejection.

**Exit Criteria**: All insight commands/events have typed schemas and compatibility rules.

**Delivers**: Stable foundation for orchestrator and bootstrap flows.

---

### Phase 1: Runtime Trigger Reliability
**Goal**: Keep T0/T1 trigger flow robust and observable.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Ensure ingest/handoff/manual/task triggers emit normalized feed events with reason/progress.
- [ ] Add dedupe/debounce hardening and singleton enforcement metrics.

**Exit Criteria**: Trigger behavior is deterministic under rapid event bursts.

**Delivers**: Reliable live Insight progress and run-state semantics.

---

### Phase 2: Insight Supervisor + Sub-Agent Modes
**Goal**: Provide bounded sub-agent orchestration primitives.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Implement supervisor dispatch mode (single specialist delegation).
- [ ] Implement chain mode (sequential step pipeline with `$INPUT/$ORIGINAL` semantics).
- [ ] Implement parallel expert mode (`Promise.allSettled` style fanout with summary merge).
- [ ] Add role manifests and scoped tool policies in `.pi/agents/` for:
  - `insight-t1-observer`
  - `insight-t2-reflector`

**Exit Criteria**: Orchestration can run isolated workers and return merged results with lifecycle telemetry.

**Delivers**: Operational sub-agent platform for Insight tasks.

---

### Phase 3: Brown-Field Bootstrap Flow
**Goal**: Convert repository drift into actionable plans and memory seeds.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Build docs-vs-code scanner with evidence refs and confidence scoring.
- [ ] Build Taskmaster proposal generator (dry-run by default + confirm apply).
- [ ] Build T2 seed projector from high-priority ambiguity/drift gaps.

**Exit Criteria**: One command produces gap report + task proposal + seed set preview.

**Delivers**: Structured brown-field onboarding path.

---

### Phase 4: T2 Worker Runtime Integration
**Goal**: Move reflector from library-only to production runtime loop.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Wire detached reflector worker tick loop into active runtime service.
- [ ] Add health endpoints/events (lock conflict, lease owner, queue depth, fail/retry counts).
- [ ] Add safeguards for backpressure and requeue policy.

**Exit Criteria**: T2 jobs process continuously with deterministic lock/lease safety.

**Delivers**: Full T0/T1/T2 runtime operation.

---

### Phase 5: Documentation + Seeding Rollout
**Goal**: Ensure portability and operator understanding.

**Entry Criteria**: Phases 0-4 complete.

**Tasks**:
- [ ] Publish architecture reference in AOC docs.
- [ ] Update `aoc-init` to seed optional insight specialist templates/manifests.
- [ ] Add smoke tests for seeded assets + orchestrator command availability.

**Exit Criteria**: New repos can enable Insight orchestration consistently.

**Delivers**: Documented and portable Insight technology.

---

## Test Strategy

## Test Pyramid

```text
        /\
       /E2E\       ← 10%
      /------\
     /Integration\ ← 30%
    /------------\
   /  Unit Tests  \ ← 60%
  /----------------\
```

## Coverage Requirements
- Line coverage: 85% minimum (new modules)
- Branch coverage: 75% minimum (orchestration and fallback paths)
- Function coverage: 90% minimum
- Statement coverage: 85% minimum

## Critical Test Scenarios

### Insight Contracts
**Happy path**:
- Valid command payloads deserialize and route correctly.
- Expected: command accepted with typed response.

**Error cases**:
- Unknown trigger/tool mode/invalid args rejected.
- Expected: structured error code + message.

### Supervisor Dispatch
**Happy path**:
- Dispatch to one specialist returns streamed updates and terminal result.
- Expected: queued → running → success lifecycle.

**Edge cases**:
- Specialist already busy, stale session file, missing agent manifest.
- Expected: bounded retry or deterministic error.

### Chain/Parallel Modes
**Happy path**:
- Chain passes output across steps in order.
- Parallel experts return partial results even with one failure.

**Error cases**:
- One expert exits non-zero.
- Expected: all-settled aggregation with partial status.

### Brown-field Bootstrap
**Happy path**:
- docs-vs-code scan emits gaps with evidence links.
- Task proposal generated without applying writes by default.

**Error cases**:
- malformed docs, missing tests, large repo scan interruptions.
- Expected: partial report + resumable status.

### T2 Runtime
**Happy path**:
- Worker acquires lock/lease and completes queued jobs.

**Error cases**:
- lock conflict, stale lease takeover, handler failure with requeue policy.
- Expected: deterministic conflict reporting + safe retry semantics.

## Test Generation Guidelines
- Prefer deterministic fixture inputs for orchestration tests.
- Assert feed event ordering and terminal statuses.
- Include provenance assertions on fallback paths.
- Add stress tests for concurrent trigger bursts and worker contention.

---

## Architecture

## System Components
1. **Insight Event Producers**: Pi extension hooks, task-completion signals, manual shortcuts.
2. **Pulse Transport**: Hub routes typed insight commands/events.
3. **Insight Runtime (Wrapper)**: Ingest/compact/store + sidecar queue + supervisor orchestration.
4. **Distillation Engine (aoc-mind)**: T1 observer and T2 reflector logic with fail-open guarantees.
5. **Storage (aoc-storage/MindStore)**: raw events, T0 compact events, observations, reflections, provenance, leases, queue.
6. **Operator Surfaces**: Mission Control + minimal footer + CLI/tooling.

## Data Models
- **InsightCommandEnvelope**: `command`, `conversation_id`, `session_id`, `args`, `request_id`.
- **InsightGapRecord**: `gap_id`, `kind`, `evidence_refs[]`, `confidence`, `severity`, `suggested_owner`.
- **InsightTaskProposal**: `tag`, `tasks[]`, `dependencies[]`, `prd_link`.
- **InsightSeedJob**: `seed_id`, `scope_tag`, `source_gap_ids[]`, `priority`, `retry_policy`.
- **InsightProvenanceRow**: `stage`, `runtime`, `model/provider`, `fallback_used`, `failure_kind`, `latency_ms`.

## Technology Stack
- **Language**: Rust for core runtime/hub/wrapper; TypeScript for Pi extension layer.
- **Transport**: Pulse UDS NDJSON command/event channel.
- **Storage**: SQLite-backed MindStore (raw/T0/T1/T2/provenance/lease/queue).
- **UI**: Pi extension widgets/footer + Mission Control TUI.
- **Orchestration Execution**: Pi subprocess workers with scoped tools and prompts.

**Decision: Keep Pi subprocess sub-agent execution (supervised) instead of embedded in-process workers**
- **Rationale**: Strong isolation boundaries, role-specific tool caps, crash containment.
- **Trade-offs**: Higher process overhead, session file management complexity.
- **Alternatives considered**: in-process role multiplexing (rejected for weaker isolation).

**Decision: Preserve fail-open deterministic fallback for T1/T2**
- **Rationale**: Reliability and continuity are mandatory for operational workflows.
- **Trade-offs**: Semantic quality may degrade under provider failures.
- **Alternatives considered**: fail-closed semantic-only mode (rejected for operator workflow risk).

**Decision: Dry-run-by-default for brown-field task projection**
- **Rationale**: Prevent accidental task graph mutation on noisy scans.
- **Trade-offs**: Extra confirmation step for operators.
- **Alternatives considered**: direct-write mode by default (rejected for safety).

---

## Risks

## Technical Risks
**Risk**: Supervisor complexity creates race conditions between trigger queue and sub-agent orchestration.
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: strict state machine + idempotent request IDs + bounded concurrency tests.
- **Fallback**: degrade to single-dispatch mode only.

**Risk**: Brown-field scanner creates noisy or low-confidence gap output.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: confidence thresholds + evidence requirements + manual review gates.
- **Fallback**: report-only mode without task projection.

## Dependency Risks
- External model/provider volatility may impact semantic quality.
- Pi subprocess behavior/version changes can affect event parsing assumptions.
- Large-repo scanning cost may require batching/chunking controls.

## Scope Risks
- Overloading one milestone with naming, orchestration, bootstrap, and runtime worker integration.
- Mitigation: phase-gated delivery with independently shippable slices.

---

## References
- Existing Mind PRD: `.taskmaster/docs/prds/aoc-mind_prd.md`
- Mind graph PRD: `.taskmaster/docs/prds/aoc-mind-graph-foundation_prd_rpg.md`
- Comparative implementation studied: `/tmp/pi-vs-claude-code` (`subagent-widget.ts`, `agent-team.ts`, `agent-chain.ts`, `pi-pi.ts`)

## Glossary
- **Insight**: product-facing name for AOC observational memory system (T0/T1/T2).
- **T0**: compacted observational transcript lane.
- **T1**: observer distillation stage.
- **T2**: reflector synthesis stage.
- **Supervisor**: orchestrator that delegates to specialist sub-agents.

## Open Questions
- Should insight tool names be introduced as aliases first (`mind_*` + `insight_*`) or switched in one major release?
- Which specialist templates should be seeded by default vs optional packs?
- Should T2 worker run inside wrapper process or separate service binary in production?