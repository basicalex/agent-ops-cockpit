# AOC Mind v2 — Project-Scoped T3 Canon, Backlog, and Adaptive Handshake Injection

## PRD (Repository Planning Graph / RPG Method)

---

<overview>

## Problem Statement

AOC Mind currently performs strong in-session distillation (T0/T1/T2), but project continuity remains fragmented across pane/session-local memory lanes.

This creates four core problems:

1. **Session siloing**: valuable insights stay trapped in per-session artifacts and are not consolidated into a durable project mind.
2. **Inconsistent continuity**: startup handshakes can miss important cross-session context because there is no continuously updated project canon.
3. **Token inefficiency risk**: without a bounded canonical layer, repeated context hydration can drift toward additive bloat.
4. **Weak observability of memory pipeline health**: operators cannot reliably inspect pipeline backlog and lifecycle state for T0/T1/T2/T3 from one place.

We need a complete architecture where:
- T0/T1/T2 run during active sessions,
- finished/idle sessions automatically emit consolidation inputs,
- T3 continuously updates project-scoped canonical insight,
- handshake injection stays small, adaptive, and high-signal.

## Target Users

1) **Primary: Solo multi-tab AOC developer**
- Runs multiple Pi sessions in parallel per project.
- Needs continuity when switching tabs, tags, and sessions.
- Wants memory support without manual retrieval overhead.

2) **Secondary: Project maintainer / reviewer**
- Needs auditable project growth over time.
- Must inspect how current guidance was derived (traceability/provenance).

3) **Internal: AOC runtime/operator maintainers**
- Need deterministic pipelines, fail-open behavior, robust recovery, and observable runtime health.

## Success Metrics

- **Coverage**: 100% of closed/idle sessions produce a T3 backlog candidate within 10s of finalization trigger.
- **Consolidation reliability**: ≥ 99% successful T3 job completion without manual intervention over 7-day run.
- **Handshake quality**: default handshake payload remains bounded (target <= 500 tokens) while preserving top-priority canonical context.
- **Token efficiency**: > 70% reduction in repeated reinjection of identical memory blocks (hash-dedupe vs baseline naive reinjection).
- **Continuity benefit**: resume/tag-switch tasks require <= 1 manual recall query in 80% of sampled workflows.
- **Auditability**: every handshake line can be traced to canonical entries and upstream T1/T2 artifacts.

</overview>

---

<functional-decomposition>

## Capability Tree

### Capability: Session Runtime Distillation (T0/T1/T2)

Active-session memory capture and distillation with deterministic fallback.

#### Feature: Continuous T0 ingestion in active sessions
- **Description**: Ingest session events and compact into deterministic T0 records in real time.
- **Inputs**: message/tool events, session identifiers, lineage metadata, compaction policy.
- **Outputs**: `raw_events`, `compact_events_t0`, progress counters.
- **Behavior**: Always-on ingest; idempotent event IDs; deterministic compact hashes.

#### Feature: Triggered T1 observer runs
- **Description**: Run observer distillation when token thresholds/manual/task/handoff triggers fire.
- **Inputs**: T0 stream, trigger kind, queue state, guardrails.
- **Outputs**: T1 observation artifacts with provenance.
- **Behavior**: Debounced queue; one active run per session; semantic-with-fallback.

#### Feature: Triggered T2 reflection runs
- **Description**: Produce reflection artifacts from tag-bounded T1 observations.
- **Inputs**: T1 observations, active tag context, trigger thresholds.
- **Outputs**: T2 reflection artifacts with provenance.
- **Behavior**: No cross-tag mixing by default; deterministic fallback preserved.

#### Feature: Session finalization drain
- **Description**: On session inactivity/close, flush remaining T1/T2 work and seal session artifact export.
- **Inputs**: lifecycle events (idle timeout, shutdown), queue states, pending artifacts.
- **Outputs**: finalized session export manifest and T3 enqueue signal.
- **Behavior**: Best-effort drain with bounded timeout; idempotent re-run.

---

### Capability: T3 Backlog and Project Consolidation

Project-level async pipeline that consumes completed session outputs and updates canonical project memory.

#### Feature: T3 backlog job creation
- **Description**: Create backlog jobs from finalized session deltas.
- **Inputs**: session export manifest, watermark range, active tag context.
- **Outputs**: T3 backlog records (`pending`).
- **Behavior**: idempotent job key; reject duplicate slice processing.

#### Feature: Singleton T3 worker with lock/lease
- **Description**: Run exactly one consolidation owner per project root.
- **Inputs**: backlog queue, lock path, durable lease state.
- **Outputs**: claimed/running/completed/failed job transitions.
- **Behavior**: advisory file lock + DB lease; stale lease takeover; bounded retries.

#### Feature: Delta-only processing via watermarks
- **Description**: Process only unconsumed artifacts since last consolidation watermark.
- **Inputs**: per-project/per-tag watermark table, session artifact refs.
- **Outputs**: applied delta set and updated watermarks.
- **Behavior**: exactly-once logical semantics over artifact IDs.

---

### Capability: Project Canon (Temporal, Revisioned Memory)

Curated project memory that evolves over time instead of append-only sprawl.

#### Feature: Canon entry synthesis
- **Description**: Synthesize project-level insights from cross-session T1/T2 evidence.
- **Inputs**: eligible artifacts, tag/workstream scope, prior canon state.
- **Outputs**: proposed canon entries.
- **Behavior**: confidence/freshness scoring; concise, operator-readable entries.

#### Feature: Revision/supersede lifecycle
- **Description**: Keep canon mutable with explicit temporal lifecycle.
- **Inputs**: new evidence conflicting/updating prior entries.
- **Outputs**: `active`, `superseded`, `stale` states + revision links.
- **Behavior**: never silently overwrite; preserve full lineage.

#### Feature: Canon export for humans
- **Description**: Emit readable project memory files from canonical state.
- **Inputs**: canonical entry graph.
- **Outputs**: `project_mind.md` and revision metadata.
- **Behavior**: deterministic ordering; stable anchors and references.

---

### Capability: Adaptive Handshake Injection Controller

Bounded, event-aware context hydration for active agents.

#### Feature: Baseline handshake compilation
- **Description**: Compile compact baseline from T3 canon for session startup.
- **Inputs**: active canonical entries, active tag, open risks.
- **Outputs**: `handshake.md` + hash/version metadata.
- **Behavior**: hard token budget; deterministic rendering.

#### Feature: Triggered refresh on tag/resume/handoff
- **Description**: Refresh injection payload on high-value continuity events.
- **Inputs**: tag change signal, `aoc-stm resume`, `mind_handoff`, canon revision.
- **Outputs**: pending injection action for next turn.
- **Behavior**: inject on next turn boundary; avoid mid-stream interruption.

#### Feature: Context-pressure and dedupe gating
- **Description**: Suppress/trim injection under high context pressure and duplicate payloads.
- **Inputs**: context usage %, last injected hash, cooldown state.
- **Outputs**: inject/skip decision and bounded payload.
- **Behavior**: one injection max per turn; cooldown for non-urgent updates.

---

### Capability: Mind/Insight Interactive Observability

Operator-visible pipeline and artifact inspection UX.

#### Feature: Pipeline backlog dashboard
- **Description**: Show per-session T0/T1/T2 and project T3 backlog status.
- **Inputs**: runtime status events + storage queries.
- **Outputs**: queue/running/success/fallback/error lanes.
- **Behavior**: filter by session/tag/project; sortable by freshness/latency.

#### Feature: Artifact drilldown
- **Description**: Browse exported session artifacts and canonical entries with provenance.
- **Inputs**: artifact IDs, manifest refs, provenance rows.
- **Outputs**: detail panels and trace links.
- **Behavior**: jump from handshake lines -> canon entry -> T2/T1 evidence.

#### Feature: Operational controls
- **Description**: Allow safe requeue/rebuild operations.
- **Inputs**: operator command (requeue T3, rebuild handshake, force finalize session).
- **Outputs**: command result + updated backlog state.
- **Behavior**: audit logged; idempotent where possible.

</functional-decomposition>

---

<structural-decomposition>

## Repository Structure

```text
project-root/
├── .aoc/
│   └── mind/
│       ├── project.sqlite
│       ├── insight/
│       │   └── <session-name>_<datetime>/
│       │       ├── t1.md
│       │       ├── t2.md
│       │       └── manifest.json
│       ├── t3/
│       │   ├── backlog/
│       │   ├── project_mind.md
│       │   └── handshake.md
│       └── locks/
│           ├── reflector.lock
│           └── t3.lock
├── crates/
│   ├── aoc-agent-wrap-rs/
│   ├── aoc-mind/
│   ├── aoc-storage/
│   ├── aoc-mission-control/
│   └── aoc-cli/
└── .pi/
    └── extensions/
        └── minimal.ts
```

## Module Definitions

### Module: `crates/aoc-storage`
- **Maps to capability**: T3 Backlog and Project Consolidation, Project Canon
- **Responsibility**: Persist backlog jobs, canon revisions, handshake snapshots, and watermarks.
- **File structure**:
  ```text
  migrations/
    0005_t3_backlog.sql
    0006_project_canon.sql
    0007_handshake_state.sql
  src/lib.rs
  ```
- **Exports**:
  - `enqueue_t3_job()`
  - `claim_next_t3_job()`
  - `complete_t3_job()` / `fail_t3_job()`
  - `upsert_canon_entry_revision()`
  - `active_canon_entries()`
  - `upsert_handshake_snapshot()`
  - `project_watermark()` / `advance_project_watermark()`

### Module: `crates/aoc-mind`
- **Maps to capability**: Session finalization drain, T3 worker, canon synthesis
- **Responsibility**: Distillation orchestration and consolidation runtime.
- **File structure**:
  ```text
  src/
    lib.rs
    observer_runtime.rs
    reflector_runtime.rs
    t3_runtime.rs
    canon_compiler.rs
    handshake_compiler.rs
  ```
- **Exports**:
  - `finalize_session_slice()`
  - `run_t3_once()`
  - `compile_project_canon()`
  - `compile_handshake_snapshot()`

### Module: `crates/aoc-agent-wrap-rs`
- **Maps to capability**: Lifecycle triggering, runtime event wiring
- **Responsibility**: Detect session inactivity/shutdown, queue T3 jobs, publish pipeline status.
- **File structure**:
  ```text
  src/
    main.rs
    insight_orchestrator.rs
    mind_injection_controller.rs
  ```
- **Exports**:
  - `mind_finalize_session` command path
  - `mind_t3_status` health payload updates
  - injection trigger publication events

### Module: `.pi/extensions/minimal.ts` (and successor extension)
- **Maps to capability**: Adaptive Handshake Injection Controller
- **Responsibility**: Turn-boundary injection gating and UI indicators.
- **File structure**:
  ```text
  .pi/extensions/
    minimal.ts
    mind-injection.ts (new)
  ```
- **Exports/Handlers**:
  - `on("before_agent_start")`
  - `on("context")`
  - `on("session_start")`
  - pulse command bridge for tag/resume/handoff triggers

### Module: `crates/aoc-mission-control`
- **Maps to capability**: Mind/Insight Interactive Observability
- **Responsibility**: Backlog panel, artifact drilldown, requeue controls.
- **Exports**:
  - T0/T1/T2/T3 lane views
  - command actions (`run_observer`, `mind_finalize`, `mind_t3_requeue`, `mind_handshake_rebuild`)

### Module: `crates/aoc-cli`
- **Maps to capability**: Retrieval and operations
- **Responsibility**: Add/extend CLI commands for deep artifact retrieval and canon inspection.
- **Exports**:
  - `aoc-insight` modes (`--brief`, `--refs`, `--snips`, `--scope session|project`)
  - optional `aoc-mind` operator commands for T3 controls

</structural-decomposition>

---

<dependency-graph>

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies.

- **Storage schema v2 (T3)**: backlog/canon/handshake/watermark tables and migrations.
- **Filesystem layout contract**: `.aoc/mind/` directory and export path conventions.
- **Canonical IDs/hashes contract**: deterministic IDs for session exports and T3 jobs.

### Session Finalization Layer (Phase 1)
- **Session finalizer runtime**: Depends on [Storage schema v2, Filesystem layout contract].
- **Export writer (`insight/<session>_<datetime>`)**: Depends on [Filesystem layout contract, Canonical IDs/hashes contract].

### T3 Runtime Layer (Phase 2)
- **T3 backlog worker**: Depends on [Storage schema v2, Session finalizer runtime].
- **Lease/lock coordination for T3**: Depends on [Storage schema v2].
- **Watermark advancement engine**: Depends on [T3 backlog worker, Canonical IDs/hashes contract].

### Canon Layer (Phase 3)
- **Canon compiler**: Depends on [T3 backlog worker, Watermark advancement engine].
- **Revision/supersede lifecycle**: Depends on [Canon compiler, Storage schema v2].
- **Project canon exporter**: Depends on [Canon compiler].

### Handshake & Injection Layer (Phase 4)
- **Handshake compiler**: Depends on [Canon compiler, Revision/supersede lifecycle].
- **Injection controller policy engine**: Depends on [Handshake compiler, session/runtime signals].
- **Trigger adapters (tag change / resume / handoff)**: Depends on [Injection controller policy engine].

### UX & Retrieval Layer (Phase 5)
- **Mind TUI backlog panel**: Depends on [T3 runtime layer, canon layer].
- **Artifact drilldown UX**: Depends on [project/session exports, provenance queries].
- **`aoc-insight` scope extensions**: Depends on [canon + export indexes].

### Hardening & Rollout Layer (Phase 6)
- **E2E reliability suite**: Depends on [all prior layers].
- **Performance budgets & tuning**: Depends on [all prior layers].
- **Cutover and migration tooling**: Depends on [all prior layers].

</dependency-graph>

---

<implementation-roadmap>

## Development Phases

### Phase 0: Project Mind Storage Foundation
**Goal**: Establish canonical project-scoped storage and file layout.

**Entry Criteria**: Current T0/T1/T2 runtime available.

**Tasks**:
- [ ] Add migrations for T3 backlog, project canon, handshake snapshots, and watermarks (depends on: none)
  - Acceptance criteria: migrations apply cleanly on new and existing DBs.
  - Test strategy: migration integration tests for upgrade and rollback guard checks.

- [ ] Implement project-root resolver for `.aoc/mind/project.sqlite` and lock paths (depends on: none)
  - Acceptance criteria: all runtime components resolve the same canonical project paths.
  - Test strategy: path resolution tests with environment overrides and defaults.

**Exit Criteria**: Runtime can read/write canonical project-level mind store.

**Delivers**: Stable storage substrate for T3 pipeline.

---

### Phase 1: Session Finalization and Export
**Goal**: Turn completed session lanes into deterministic export bundles.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Implement lifecycle trigger detection for inactivity/shutdown session finalization.
- [ ] Drain pending T1/T2 work with bounded timeout before export.
- [ ] Write `insight/<session>_<datetime>/t1.md`, `t2.md`, `manifest.json`.
- [ ] Enqueue T3 delta job from export manifest.

**Exit Criteria**: Session close/idle reliably produces export + T3 enqueue.

**Delivers**: Complete handoff from live session memory to project pipeline.

---

### Phase 2: T3 Backlog Worker
**Goal**: Build singleton consolidator runtime with robust retry and watermark semantics.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Implement T3 backlog claim/complete/fail APIs.
- [ ] Implement singleton lock + durable lease for T3 worker.
- [ ] Implement idempotent delta processing and watermark advancement.
- [ ] Add requeue mechanics and dead-letter metadata for repeated failures.

**Exit Criteria**: T3 worker runs continuously and safely under contention.

**Delivers**: Durable project-level consolidation pipeline.

---

### Phase 3: Canon Compiler and Temporal Lifecycle
**Goal**: Generate and maintain revisioned project memory.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Compile canon entries from T1/T2 deltas with confidence/freshness scores.
- [ ] Add revision/supersede/stale transitions.
- [ ] Emit `t3/project_mind.md` deterministic export.

**Exit Criteria**: Canon updates are auditable, non-additive, and traceable.

**Delivers**: Project mind that evolves without uncontrolled growth.

---

### Phase 4: Handshake and Adaptive Injection
**Goal**: Deliver bounded, high-signal automatic context hydration.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Compile `t3/handshake.md` with strict budget and stable hash/version.
- [ ] Implement injection controller with trigger matrix:
  - session start baseline,
  - tag switch refresh,
  - `aoc-stm resume` / `mind_handoff` refresh,
  - sparse canon-update deltas.
- [ ] Add dedupe/cooldown/context-pressure suppression.

**Exit Criteria**: Automatic injection improves continuity without context bloat.

**Delivers**: Production-grade memory hydration policy.

---

### Phase 5: Mind/Insight UX + Retrieval
**Goal**: Make pipeline states and artifacts directly explorable.

**Entry Criteria**: Phase 4 complete.

**Tasks**:
- [ ] Add T0/T1/T2/T3 backlog lanes to Mind/Insight UI.
- [ ] Add artifact drilldown and provenance tracing.
- [ ] Extend `aoc-insight` for project/session scoped retrieval with citations.

**Exit Criteria**: Operators can inspect, debug, and steer the full memory lifecycle.

**Delivers**: Observable and actionable project memory operations.

---

### Phase 6: Reliability, Performance, and Cutover
**Goal**: Production hardening and rollout.

**Entry Criteria**: Phases 0–5 complete.

**Tasks**:
- [ ] Multi-session concurrency and crash recovery tests.
- [ ] Token/latency budget verification for injection.
- [ ] Migration and rollout playbook from legacy session-scoped stores.

**Exit Criteria**: System meets reliability and token-efficiency SLOs.

**Delivers**: Safe cutover and maintainable long-term operation.

</implementation-roadmap>

---

<test-strategy>

## Test Pyramid

```text
        /\
       /E2E\       ← 15%
      /------\
     /Integr. \    ← 35%
    /----------\
   /   Unit     \  ← 50%
  /--------------\
```

## Coverage Requirements
- Line coverage: 85% minimum (new/changed modules)
- Branch coverage: 75% minimum
- Function coverage: 90% minimum
- Statement coverage: 85% minimum

## Critical Test Scenarios

### T3 backlog lifecycle
**Happy path**:
- Session finalization enqueues one T3 job.
- Worker claims and completes once.
- Watermark advances.

**Edge cases**:
- Duplicate enqueue for same delta slice.
- Empty session export.
- Out-of-order timestamps.

**Error cases**:
- Worker crash mid-job.
- Lease conflict and takeover.
- Storage write failure.

**Integration points**:
- `aoc-agent-wrap-rs` finalizer -> `aoc-storage` queue -> `aoc-mind` t3 runtime.

### Canon revision lifecycle
**Happy path**:
- New evidence creates active canon entry.
- Stronger later evidence supersedes prior entry.

**Edge cases**:
- Conflicting evidence with equal confidence.
- Multi-tag relevance where default policy is tag-bounded.

**Error cases**:
- Corrupt provenance reference.
- Missing source artifact during replay.

### Injection policy
**Happy path**:
- Session start injects baseline handshake.
- Tag change injects tag-focused refresh on next turn.
- Resume/handoff injects continuity slice.

**Edge cases**:
- Repeated identical handshake payloads.
- High context pressure (>70%) suppression.

**Error cases**:
- Handshake file missing.
- stale hash state in extension cache.

**Integration points**:
- pulse signals + extension hooks (`before_agent_start`, `context`).

## Test Generation Guidelines
- Prefer deterministic fixtures with stable timestamps/IDs.
- Validate idempotency under repeated finalization and repeated worker runs.
- Assert provenance chain integrity from handshake line -> canon revision -> source artifact IDs.
- Include cross-process contention tests for lock + lease behavior.

</test-strategy>

---

<architecture>

## System Components

1. **Session Distillation Runtime (T0/T1/T2)**
- Existing in-session ingest and observer/reflector paths.
- Enhanced with explicit session-finalization drain and export.

2. **Project Consolidation Runtime (T3)**
- Singleton project worker consuming finalized session deltas.
- Maintains canon, watermarks, and handshake snapshots.

3. **Adaptive Injection Controller**
- Event-driven trigger matrix + token budget gates.
- Applies hydration at turn boundaries only.

4. **Mind/Insight Observability Plane**
- Queue and artifact visibility across all stages.
- Operational controls for requeue/rebuild/finalize.

## Data Models

### T3 Backlog Job
- `job_id`
- `project_root`
- `session_id`
- `pane_id`
- `slice_start_id/slice_end_id`
- `artifact_refs_json`
- `status`
- `attempts`
- `last_error`
- `created_at/updated_at`

### Canon Entry Revision
- `entry_id`
- `revision`
- `state (active|superseded|stale)`
- `topic/tag`
- `summary`
- `confidence_bps`
- `freshness_score`
- `supersedes_entry_id`
- `evidence_refs_json`
- `created_at`

### Handshake Snapshot
- `snapshot_id`
- `scope (project|tag|session)`
- `payload_text`
- `payload_hash`
- `token_estimate`
- `created_at`

### Watermark
- `scope_key`
- `last_artifact_ts`
- `last_artifact_id`
- `updated_at`

## Technology Stack

- Rust runtimes (`aoc-agent-wrap-rs`, `aoc-mind`, `aoc-storage`)
- SQLite WAL as canonical local store
- Pi extension hooks for injection/runtime UI
- Pulse IPC for command + event propagation

**Decision: Project-scoped canonical mind storage**
- **Rationale**: enables cross-session consolidation and auditable growth.
- **Trade-offs**: requires migration and stronger locking semantics.
- **Alternatives considered**: continue pane-scoped stores with ad-hoc retrieval (rejected: fragmentation).

**Decision: Layered handshake (T3 baseline + session delta)**
- **Rationale**: strongest continuity with bounded token cost.
- **Trade-offs**: more policy logic in injection controller.
- **Alternatives considered**: startup-only handshake (rejected: stale continuity).

**Decision: Turn-boundary-only injection**
- **Rationale**: avoids stream interruption and UI instability.
- **Trade-offs**: slight delay before refresh applies.
- **Alternatives considered**: immediate mid-run steer injection (rejected for stability concerns).

</architecture>

---

<risks>

## Technical Risks

**Risk**: T3 backlog duplicates or missed slices
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: idempotency keys + watermark invariants + replay tests
- **Fallback**: manual requeue with deterministic dedupe checks

**Risk**: Canon quality drift (over-compression or stale claims)
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: confidence/freshness scoring + supersede lifecycle + operator audit tools
- **Fallback**: revert to prior canon revision and rebuild from watermark

**Risk**: Injection bloat under rapid trigger churn
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: strict token budgets, dedupe hashes, cooldown windows, pressure gating
- **Fallback**: emergency mode (startup-only baseline injection)

## Dependency Risks

- Migration complexity from session-scoped default DB paths.
- Event consistency for tag change/resume/handoff triggers across runtime components.
- Lock/lease correctness under process crash and restart storms.

## Scope Risks

- Overloading single milestone with storage, runtime, injection, and UX.
- Ambiguous boundaries with existing pending tasks (109/110/131/132).
- Mitigation: explicit dependency graph and phased delivery with acceptance gates.

</risks>

---

<appendix>

## References
- `.taskmaster/docs/prds/aoc-mind_prd.md`
- `.taskmaster/docs/prds/task-108_semantic-om-background-layer_prd.md`
- `.taskmaster/docs/prds/insight-subagent-orchestration_prd_rpg.md`
- `crates/aoc-mind/src/lib.rs`
- `crates/aoc-agent-wrap-rs/src/main.rs`
- `.pi/extensions/minimal.ts`

## Glossary
- **T0**: deterministic compact transcript lane.
- **T1**: observer artifacts from compact transcript.
- **T2**: reflection artifacts from grouped observations.
- **T3**: project-level cross-session consolidation tier.
- **Canon**: revisioned temporal project memory, not append-only log.
- **Handshake**: bounded injection payload for agent continuity.

## Open Questions
1. Should T3 operate per project root only, or optionally per tag partition with separate workers?
2. What default inactivity timeout best balances responsiveness and churn?
3. Should handshake compilation include role-targeted variants for specialist dispatch by default, or phase later?
4. What retention policy should apply to old session export bundles in `.aoc/mind/insight/`?

</appendix>
