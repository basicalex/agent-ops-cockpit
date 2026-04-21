# Project-Scoped Mind Standalone Service PRD (RPG)

> Purpose: complete the architectural cut from wrapper-coupled Mind runtime to a project-scoped standalone Mind service that keeps ingest, compaction, reflection, canon, and retrieval working even when Pulse is offline.

---

## Problem Statement

Today, live AOC Mind ingest is still operationally coupled to `aoc-agent-wrap-rs` and Pulse command transport:

- Pi session activity can be normalized from JSONL via `aoc-pi-adapter`, but the **live Mind runtime** (raw-event ingest, token-threshold triggers, finalize, detached T2/T3 dispatch, runtime health) still lives inside `crates/aoc-agent-wrap-rs/src/main.rs`.
- Pi-side/session-side flows commonly reach Mind through Pulse commands like `mind_ingest_event`, `mind_finalize_session`, `mind_context_pack`, and `mind_provenance_query`.
- When Pulse is offline or unavailable, the current operational path degrades badly: Mind ingest stalls, T0/T1/T2/T3 drift behind real session state, and Pi sessions accumulate bloated raw context instead of relying on fresh project-local Mind state.
- This also preserves the wrong ownership boundary: wrapper/session runtime still acts like the Mind service, even though Mind is conceptually **project-scoped** and already has detached orchestration semantics under `owner_plane=mind`.

The result is product and implementation drift:

1. **Operational fragility** — Pulse outages interrupt Mind ingest.
2. **Wrong ownership model** — Mind runtime is still session/wrapper-owned, not project-owned.
3. **Context bloat in Pi** — without fresh project-local Mind updates, sessions keep carrying too much raw history.
4. **Migration drag** — task 178 detached workers exist, but T1/admission/bootstrap still depend on wrapper-local runtime.
5. **Boundary confusion** — Mission Control, wrapper, Pi, and Mind all partially own pieces of the same project-local state machine.

## Target Users

- **Primary:** AOC operators working in Pi sessions who need project-local Mind ingest/retrieval to keep up even when Pulse is degraded or offline.
- **Secondary:** AOC developers maintaining Mind/runtime code who need a clean source-of-truth boundary for project-local ingest and detached processing.
- **Tertiary:** Mission Control operators who need explicit global visibility into project-scoped Mind backlog/health without Mind being session-coupled.

## Success Metrics

1. Pi session JSONL can be ingested directly into project Mind without Pulse.
2. Project-local Mind state continues to advance during Pulse outages.
3. One project has one canonical Mind runtime root/store and bounded worker admission, regardless of pane/session count.
4. Mission Control shows standalone Mind health/backlog explicitly.
5. Wrapper compatibility remains during migration, but wrapper is no longer the canonical ingest/runtime owner.
6. Pi context-pack freshness improves because recent project state lands in Mind continuously instead of only through Pulse-mediated paths.

---

## Capability Tree

### Capability: Standalone Project Runtime Ownership
Mind becomes a project-scoped runtime/service rather than a wrapper-local subsystem.

#### Feature: Canonical project runtime root
- **Description**: Resolve one canonical runtime root per project for store, locks, queue state, and health.
- **Inputs**: project root, environment (`XDG_STATE_HOME`, `HOME`), optional operator overrides.
- **Outputs**: typed runtime paths (store path, legacy path, lock paths, runtime root).
- **Behavior**: Uses project-root-derived canonical storage and lock paths independent of session/pane identity.

#### Feature: Service bootstrap and liveness
- **Description**: Start or attach to a standalone Mind process per project.
- **Inputs**: project root, service mode, health paths/locks.
- **Outputs**: live runtime loop, health snapshot, restart-safe ownership.
- **Behavior**: Ensures only one service owns project-local admission/bootstrap at a time.

### Capability: Direct Pi Ingest
Mind ingests Pi session data directly from JSONL/session artifacts instead of requiring Pulse commands.

#### Feature: Pi session discovery
- **Description**: Discover the latest Pi session JSONL for a project from canonical Pi session roots.
- **Inputs**: project root, optional explicit session file/root, Pi settings root.
- **Outputs**: latest matching JSONL file path.
- **Behavior**: Uses project-root bucket resolution and newest-file selection.

#### Feature: Incremental JSONL ingest
- **Description**: Incrementally import Pi session JSONL into the Mind store with checkpoints.
- **Inputs**: session JSONL path, project store path, agent/service identity.
- **Outputs**: raw event insertions, T0 compactions, compaction checkpoints, ingest progress.
- **Behavior**: Reuses `aoc-pi-adapter` normalization and checkpointing so repeated syncs are idempotent.

#### Feature: Live polling/watch mode
- **Description**: Continuously re-ingest the latest Pi session file without Pulse.
- **Inputs**: project root, optional explicit file, interval.
- **Outputs**: ongoing store updates and operator-visible ingest telemetry.
- **Behavior**: Polls or later watches session artifacts and advances checkpoints incrementally.

### Capability: Project-Scoped Mind Processing
Once raw events land, Mind continues T1/T2/T3 work from the project store.

#### Feature: T1 admission decoupled from wrapper session runtime
- **Description**: Move token-threshold and finalize-adjacent admission to the project-scoped service.
- **Inputs**: project store deltas, queue watermarks, service policy.
- **Outputs**: T1 jobs/slices, observer feed/runtime status.
- **Behavior**: Service computes when to distill or finalize based on project-local store state rather than one wrapper process.

#### Feature: Detached T2/T3 orchestration continuity
- **Description**: Preserve the detached `owner_plane=mind` T2/T3 model under standalone ownership.
- **Inputs**: queue/backlog state, detached registry, service lease.
- **Outputs**: bounded T2/T3 dispatch, reconciliation, cancel/restart behavior.
- **Behavior**: Reuses task 178 semantics while moving admission/bootstrapping out of wrapper-local runtime.

#### Feature: Export/finalize continuity
- **Description**: Continue producing exports/manifests/handshake/canon from project state.
- **Inputs**: finalized artifacts, compaction state, project runtime policy.
- **Outputs**: insight exports, manifests, handshake/canon updates.
- **Behavior**: Service owns finalize/export timing and keeps wrapper/Pi as consumers.

### Capability: Compatibility Bridge
During migration, existing wrapper/Pulse features must continue to work.

#### Feature: Wrapper compatibility mode
- **Description**: Wrapper forwards commands or queries to standalone Mind when available.
- **Inputs**: existing Pulse commands and wrapper runtime hooks.
- **Outputs**: compatibility responses without duplicated ownership.
- **Behavior**: Wrapper degrades to client/bridge role rather than authoritative runtime owner.

#### Feature: Mission Control health visibility
- **Description**: Show standalone service health/backlog/ingest freshness in Mission Control.
- **Inputs**: runtime snapshot/health store, detached registry, project store metadata.
- **Outputs**: Mind lane health, fleet summaries, stale/offline warnings.
- **Behavior**: Mission Control remains global surface only; it does not become Mind owner.

#### Feature: Pi retrieval compatibility
- **Description**: Pi context-pack and provenance queries continue to work during migration.
- **Inputs**: existing Pi tools/commands.
- **Outputs**: same retrieval outputs, but sourced from standalone-owned project state.
- **Behavior**: Existing tool contracts survive while transport/ownership moves underneath.

---

## Repository Structure

```
crates/
├── aoc-core/
│   ├── src/
│   │   ├── mind_contracts.rs
│   │   ├── mind_observer_feed.rs
│   │   └── ...
│
├── aoc-pi-adapter/
│   ├── src/
│   │   └── lib.rs                    # canonical Pi JSONL normalization + checkpoints
│
├── aoc-mind/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── query.rs
│   │   ├── render.rs
│   │   ├── observer_runtime.rs
│   │   ├── reflector_runtime.rs
│   │   ├── t3_runtime.rs
│   │   ├── standalone.rs            # NEW foundation: runtime paths + direct Pi ingest
│   │   └── bin/
│   │       └── aoc-mind-service.rs  # NEW standalone service/ingest CLI surface
│
├── aoc-agent-wrap-rs/
│   ├── src/
│   │   └── main.rs                  # migrates from runtime owner → compatibility bridge
│
└── aoc-mission-control/
    ├── src/
    │   ├── app.rs
    │   ├── mind_glue.rs
    │   └── fleet.rs                 # reads standalone mind health/registry state
```

## Module Definitions

### Module: `aoc-mind::standalone`
- **Maps to capability**: Standalone Project Runtime Ownership + Direct Pi Ingest
- **Responsibility**: Canonical runtime roots, Pi session discovery, direct JSONL ingest into project Mind store.
- **Exports**:
  - `MindProjectPaths::for_project_root()` — canonical project-local paths
  - `mind_runtime_root()` — standalone runtime root
  - `default_pi_session_root()` — canonical Pi session bucket for project
  - `latest_pi_session_file()` — latest JSONL selection
  - `discover_latest_pi_session_file()` — project-scoped session discovery
  - `sync_session_file_into_project_store()` — one-shot direct ingest
  - `sync_latest_pi_session_into_project_store()` — one-shot discovery + ingest

### Module: `aoc-mind-service` binary
- **Maps to capability**: Service bootstrap and liveness
- **Responsibility**: Operator/service entrypoint for standalone Mind bootstrap and direct ingest.
- **Current exports/commands**:
  - `status`
  - `sync-pi`
  - `watch-pi`
- **Future responsibility**: long-lived project runtime loop with health snapshots, leases, admission, finalize, and compatibility IPC.

### Module: `aoc-agent-wrap-rs` compatibility bridge
- **Maps to capability**: Compatibility Bridge
- **Responsibility**: Keep Pulse-facing transport/telemetry working while delegating authoritative Mind ownership to standalone runtime.
- **Future migration**:
  - remove direct raw-event/store ownership
  - forward compatible query/finalize/context-pack requests
  - publish service health instead of pretending to be the service

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies beyond existing base crates.

- **aoc-core mind contracts**: raw event, observer feed, injection, detached metadata contracts
- **aoc-storage MindStore**: canonical durable store and migrations
- **aoc-pi-adapter**: canonical Pi JSONL normalization, checkpointing, compaction checkpoint import

### Standalone Foundation (Phase 1)
- **aoc-mind::standalone paths/session discovery**: Depends on [aoc-storage, aoc-pi-adapter]
- **aoc-mind-service bootstrap CLI**: Depends on [aoc-mind::standalone]

### Standalone Runtime Ownership (Phase 2)
- **service health + lease ownership**: Depends on [standalone foundation]
- **T1 admission extraction from wrapper**: Depends on [service health + lease ownership, aoc-mind observer runtime]
- **finalize/export ownership extraction**: Depends on [T1 admission extraction]

### Detached Runtime Alignment (Phase 3)
- **detached T2/T3 service-owned admission**: Depends on [standalone runtime ownership, task 178 detached substrate]
- **restart/reconciliation under service ownership**: Depends on [detached T2/T3 service-owned admission]

### Compatibility Layer (Phase 4)
- **wrapper as compatibility bridge**: Depends on [standalone runtime ownership]
- **Mission Control standalone health visibility**: Depends on [wrapper bridge or runtime health snapshots]
- **Pi query/context-pack compatibility**: Depends on [wrapper bridge, standalone runtime ownership]

### Cleanup Layer (Phase 5)
- **remove wrapper-coupled authoritative Mind ownership**: Depends on [all previous phases]
- **doc/ops launch integration**: Depends on [cleanup readiness]

---

## Implementation Roadmap

### Phase 0: Refactor Completion and Architecture Reset
**Goal**: close stale planning drift and ground Option C in the real repository state.

**Tasks**:
- [x] Mark task 184 done for the completed Mission Control refactor.
- [x] Update architecture docs to reflect `aoc-mind` query ownership and thin Mission Control `mind_glue`.
- [x] Create tracked standalone Mind service task/PRD.

**Exit Criteria**: task state and docs reflect reality instead of the old monolith plan.

### Phase 1: Standalone Foundation
**Goal**: land direct Pi ingest and canonical project runtime path ownership in `aoc-mind`.

**Tasks**:
- [x] Add `aoc-mind::standalone` module with canonical runtime paths.
- [x] Add project-scoped Pi session discovery helpers.
- [x] Add direct `aoc-pi-adapter`-backed store ingest helpers.
- [x] Add `aoc-mind-service` binary with `status`, `sync-pi`, and `watch-pi` commands.

**Exit Criteria**: project-local Mind ingest can advance directly from Pi JSONL without Pulse.

### Phase 2: Standalone Runtime Ownership Extraction
**Goal**: move live admission/finalize responsibility out of `aoc-agent-wrap-rs`.

**Tasks**:
- [ ] Extract reusable runtime state/bootstrap from wrapper-local `MindRuntime` into `aoc-mind`.
- [ ] Introduce standalone service lease/health snapshot for one-owner-per-project semantics.
- [ ] Move token-threshold / idle-finalize / export bootstrap into the standalone runtime loop.
- [ ] Keep wrapper compatibility by forwarding or observing instead of owning.

**Exit Criteria**: wrapper no longer acts as authoritative project Mind runtime.

### Phase 3: Detached T2/T3 Realignment Under Service Ownership
**Goal**: preserve task 178 detached substrate while making the standalone service the admission owner.

**Tasks**:
- [ ] Move reflector/T3 dispatch decisions behind standalone-owned runtime state.
- [ ] Reuse detached registry metadata (`owner_plane=mind`, `worker_kind=t2|t3`) unchanged.
- [ ] Add service startup reconciliation and stale-lease takeover.

**Exit Criteria**: detached Mind work remains bounded and project-scoped without relying on one wrapper process.

### Phase 4: Compatibility + Surface Integration
**Goal**: keep operators productive while the ownership cut settles.

**Tasks**:
- [ ] Wrapper forwards compatible `mind_*` commands/queries to standalone runtime when available.
- [ ] Mission Control renders standalone health/backlog/freshness explicitly.
- [ ] Pi tooling continues to get `mind_context_pack` / provenance results from standalone-owned data.

**Exit Criteria**: external surfaces keep working while runtime ownership is centralized.

### Phase 5: Cleanup and Default Cutover
**Goal**: remove obsolete wrapper-owned Mind authority.

**Tasks**:
- [ ] Delete or deaden wrapper-only authoritative ingest/finalize paths after migration.
- [ ] Promote standalone runtime/service launch into AOC session/project bootstrap.
- [ ] Document operator flows for service health/restart/recovery.

**Exit Criteria**: standalone project-scoped Mind service is the default and authoritative mode.

---

## Test Strategy

### Unit / Module
- `aoc-mind::standalone` tests for runtime path resolution, Pi session bucket discovery, latest JSONL selection, and direct ingest correctness.
- `aoc-pi-adapter` remains the normalization/checkpoint truth for imported Pi JSONL.

### Integration
- `aoc-mind-service sync-pi` ingests a synthetic Pi JSONL into a project-local store without Pulse.
- `aoc-mind-service watch-pi` incrementally advances checkpoints across appended JSONL lines.
- wrapper compatibility tests ensure old `mind_*` commands still function during migration.

### Runtime / Ownership
- multi-pane/multi-session tests prove one project-scoped runtime owner at a time.
- detached T2/T3 tests preserve bounded dispatch and reconciliation under service ownership.
- restart tests prove service recovers from stale leases and resumes project state safely.

### Operator Validation
- Mission Control shows standalone health/freshness when Pulse is degraded.
- Pi session can obtain fresh context-pack/provenance from project-local Mind after standalone ingest.

---

## Risks and Mitigations

### Risk: two sources of truth during migration
- **Mitigation**: keep one canonical project store and introduce explicit service lease ownership before moving admission logic.

### Risk: wrapper compatibility regressions
- **Mitigation**: preserve existing `mind_*` contracts while changing only the ownership behind them.

### Risk: duplicate detached workers across panes
- **Mitigation**: keep task 178 bounded detached semantics and make service lease ownership explicit.

### Risk: operator confusion about Pulse vs Mind health
- **Mitigation**: surface standalone Mind health separately from Pulse connection health in Mission Control and docs.

### Risk: partial migration leaves T1 still session-coupled
- **Mitigation**: explicitly track T1 admission extraction as a required phase, not optional cleanup.

---

## Current Implementation Snapshot

Already landed in this run:

- task `184` marked done
- new task `190` created to track standalone Mind service cutover
- `crates/aoc-mind/src/standalone.rs`
  - canonical project runtime paths
  - Pi session discovery helpers
  - direct `aoc-pi-adapter`-backed ingest helpers
- `crates/aoc-mind/src/bin/aoc-mind-service.rs`
  - `status`
  - `sync-pi`
  - `watch-pi`
- regression coverage for standalone path resolution, session discovery, and direct ingest

This is intentionally the **foundation phase**, not the full cutover. The remaining work is to migrate live runtime/bootstrap/ownership from `aoc-agent-wrap-rs` into `aoc-mind` so Pulse becomes an optional telemetry bridge instead of the ingest bottleneck.
