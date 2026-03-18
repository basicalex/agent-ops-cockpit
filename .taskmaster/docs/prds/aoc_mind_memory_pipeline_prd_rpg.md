# AOC Mind Memory Pipeline PRD (RPG)

## Problem Statement
AOC Mind has progressed beyond its original Mission Control + Mind sketch, but the remaining tracked work is partially stale relative to the actual architecture now in flight. The current system already has project-scoped Mind storage, T1/T2/T3 runtimes, session finalization, handshake compilation, compaction checkpoint ingestion, and early evidence links. What remains is to align the backlog to the real end-state: Pi-native session history as a source substrate, Mind SQLite as the canonical derived semantic store, bounded T1/T2/T3 layers with durable provenance, and operator-visible recovery/query surfaces.

Without that alignment, Taskmaster risks tracking outdated abstractions (for example graph-first framing), under-describing the remaining Pi compaction work, and weakening future attribution, retrieval, and rollout decisions.

## Target Users
- Maintainers evolving AOC Mind as the local-first memory system for Pi-driven development.
- Developers relying on compaction checkpoints, handoffs, retrieval, and Mission Control observability during long-running coding sessions.
- Future specialist/runtime integrations that need bounded context packs, provenance, and safe recovery paths.

## Success Metrics
- All pending `mind` tasks reflect the current layered architecture and no longer contradict the local relational-first provenance model.
- Pi compaction reliably records a durable checkpoint, triggers observer processing fail-open, and exposes status in operator surfaces.
- Remaining compaction work is explicitly tracked as first-class Pi-session/T0 normalization rather than vague checkpoint persistence.
- Retrieval, handoff, and UI tasks cite project canon plus session exports without overstating missing infrastructure.
- Release validation covers crash recovery, replay/idempotency, and no-context-loss behavior for compaction and session-finalization paths.

---

## Capability Tree

### Capability: Pi Session Substrate Ingestion
Normalize Pi-native session artifacts into replayable Mind substrate without replacing Mind as the canonical semantic store.

#### Feature: Pi session importer / reconciler
- **Description**: Read Pi JSONL/session-tree history and normalize messages, compaction entries, branch summaries, and detail payloads into Mind-ingestable substrate.
- **Inputs**: Pi session files, session metadata, compaction entries, branch summaries.
- **Outputs**: Replay-safe normalized records and ingestion checkpoints.
- **Behavior**: Treat Pi-native history as authoritative session substrate for bootstrap/backfill/recovery while keeping Mind SQLite as the derived semantic/project store.

#### Feature: Compaction-derived T0 slices
- **Description**: Convert Pi compaction checkpoints into first-class T0 slices/artifacts rather than raw markers only.
- **Inputs**: Stored compaction checkpoints, normalized Pi compaction details, provenance context.
- **Outputs**: T0 slices linked to compaction entry ids, sessions, conversations, and later T1/T2 outputs.
- **Behavior**: Preserve idempotency, replayability, and fail-open compaction semantics.

#### Feature: Structured compaction evidence capture
- **Description**: Persist compaction-derived evidence such as modified/read files and related metadata without bloating T1 prose.
- **Inputs**: Pi compaction details, wrapper git snapshots, normalized session metadata.
- **Outputs**: File/evidence links and provenance metadata attached to artifacts.
- **Behavior**: Start with file-level evidence, then extend toward diff refs, task refs, and commit refs.

### Capability: Layered Mind Runtime
Operate Mind as a layered local memory system with bounded responsibilities and explicit provenance.

#### Feature: T1 bounded observation layer
- **Description**: Produce concise session-scoped observations from T0 slices and checkpoint triggers.
- **Inputs**: T0 slices, compaction checkpoints, observer triggers, evidence links.
- **Outputs**: T1 artifacts with citations and structured evidence relationships.
- **Behavior**: Keep prose semantic and compact while attaching richer file/task provenance structurally.

#### Feature: T2 synthesis layer
- **Description**: Synthesize across related T1 groups when stronger triggers warrant it.
- **Inputs**: T1 artifacts, active tag context, session exports, evidence links.
- **Outputs**: T2 reflections and seeds.
- **Behavior**: Aggregate deliberately; avoid uncontrolled background churn.

#### Feature: T3 canon and alignment layer
- **Description**: Maintain project-level canon and alignment outputs over T1/T2 plus memory/task signals.
- **Inputs**: T2 outputs, memory, STM/handoffs, Taskmaster state, session/project exports.
- **Outputs**: Canon revisions, alignment reports, bounded handshake/context-pack inputs.
- **Behavior**: Remain read/analyze first, preserve provenance, and support manual/on-demand invocation.

### Capability: Retrieval, Handoff, and Operator Surfaces
Expose the layered system through bounded retrieval, context packs, and observable UI/runtime status.

#### Feature: Deterministic context-pack composition
- **Description**: Build startup/resume/handoff/specialist context packs from memory, STM, canon, and session deltas.
- **Inputs**: `aoc-mem`, STM, T3 canon, T2/T1 session exports, active tag.
- **Outputs**: Bounded context packs with citations and expansion controls.
- **Behavior**: Enforce precedence and stable rendering for agent handoffs and role dispatch.

#### Feature: Retrieval across session and project scopes
- **Description**: Let operators query project canon and per-session exports with citations-first bounded output.
- **Inputs**: Session exports, canon entries, retrieval scope, user query.
- **Outputs**: Brief/refs/snips results with provenance.
- **Behavior**: Keep local deterministic fallback authoritative.

#### Feature: Mission Control / dev-tab observability
- **Description**: Surface checkpoint health, T0/T1/T2/T3 progress, backlog state, and recovery controls.
- **Inputs**: Runtime telemetry, stored checkpoints, backlog queues, latest artifacts.
- **Outputs**: UI badges, status panels, query/replay controls.
- **Behavior**: Make failure obvious and recovery possible without blocking the developer.

### Capability: Provenance and Traversal Foundation
Support graph-like lineage and traversal semantics with relational storage and explicit link tables.

#### Feature: Provenance/query model
- **Description**: Expose deterministic traversals across conversations, checkpoints, artifacts, files, tasks, canon revisions, and session exports.
- **Inputs**: SQLite tables for artifacts, checkpoints, links, canon revisions, backlog/runtime state.
- **Outputs**: Query payloads and visualization adapters.
- **Behavior**: Use relational storage as the source of truth; support graph-like traversal without requiring a graph DB-first architecture.

#### Feature: Visualization/export adapters
- **Description**: Produce stable payloads for Mission Control and future visual views.
- **Inputs**: Provenance/query results.
- **Outputs**: Deterministic export payloads and adapter contracts.
- **Behavior**: Separate query correctness from UI experimentation.

---

## Repository Structure

```text
project-root/
├── .aoc/
│   ├── mind/
│   │   ├── project.sqlite
│   │   └── insight/
│   ├── context.md
│   ├── memory.md
│   └── stm/
├── .pi/
│   └── extensions/
├── crates/
│   ├── aoc-agent-wrap-rs/
│   ├── aoc-core/
│   ├── aoc-hub-rs/
│   ├── aoc-mind/
│   ├── aoc-mission-control/
│   ├── aoc-storage/
│   └── aoc-task-attribution/
├── docs/
│   └── insight-compaction-ingest.md
└── .taskmaster/
    └── docs/prds/
```

## Module Definitions

### Module: `.pi/extensions/minimal.ts`
- **Maps to capability**: Pi Session Substrate Ingestion
- **Responsibility**: Thin Pi-side hook bridge for compaction and related runtime events.

### Module: `crates/aoc-agent-wrap-rs`
- **Maps to capability**: Pi Session Substrate Ingestion + Layered Mind Runtime
- **Responsibility**: Validate/runtime-route pulse commands, checkpoint compaction, snapshot local evidence, and orchestrate observer flow fail-open.

### Module: `crates/aoc-storage`
- **Maps to capability**: Provenance and Traversal Foundation
- **Responsibility**: Canonical relational storage for checkpoints, artifacts, evidence links, canon state, and query primitives.

### Module: `crates/aoc-mind`
- **Maps to capability**: Layered Mind Runtime
- **Responsibility**: Run observer/reflector/T3 flows and expose deterministic runtime operations over project-scoped storage.

### Module: `crates/aoc-mission-control`
- **Maps to capability**: Retrieval, Handoff, and Operator Surfaces
- **Responsibility**: Surface checkpoint/runtime state, canon/project status, and future replay/query adapters.

### Module: `docs/insight-compaction-ingest.md`
- **Maps to capability**: Pi Session Substrate Ingestion
- **Responsibility**: Human-readable contract for compaction-triggered checkpointing, T0 persistence, T1 enqueue, and recovery expectations.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **Project-scoped Mind storage**: canonical SQLite store, migrations, and runtime invariants.
- **Pi extension/wrapper contract**: thin hook bridge, validation, and fail-open command routing.
- **Layer contracts**: explicit T0/T1/T2/T3 responsibilities and operator-visible status semantics.

### Session Substrate Layer (Phase 1)
- **Pi session importer / reconciler**: Depends on [Project-scoped Mind storage, Pi extension/wrapper contract]
- **Compaction-derived T0 slices**: Depends on [Pi session importer / reconciler, Layer contracts]
- **Structured evidence persistence**: Depends on [Compaction-derived T0 slices, Project-scoped Mind storage]

### Runtime Layer (Phase 2)
- **T1 checkpoint observation**: Depends on [Compaction-derived T0 slices, Structured evidence persistence]
- **T2 synthesis**: Depends on [T1 checkpoint observation]
- **T3 canon/alignment**: Depends on [T2 synthesis, Taskmaster/memory/STM signals]

### Retrieval and Surface Layer (Phase 3)
- **Context-pack composition**: Depends on [T3 canon/alignment, T1/T2 session exports]
- **Retrieval across session/project scopes**: Depends on [Context-pack composition, T1/T2 exports, T3 canon]
- **Mission Control/dev-tab observability**: Depends on [T1 checkpoint observation, T3 canon/alignment, provenance queries]

### Provenance and Release Layer (Phase 4)
- **Relational provenance/query foundation**: Depends on [Project-scoped Mind storage, T1/T2/T3 persisted outputs]
- **Visualization/export adapters**: Depends on [Relational provenance/query foundation]
- **Hardening and rollout validation**: Depends on [all prior layers]

---

## Development Phases

### Phase 0: Backlog/contract alignment
- Align remaining `mind` tasks to the current architecture.
- Promote compaction work from speculative design to tracked implementation/remainder.
- Exit: Taskmaster reflects the actual local-first layered Mind roadmap.

### Phase 1: Pi-native substrate completion
- Finish Pi session importer/reconciler.
- Convert compaction checkpoints into first-class T0 slices.
- Expand evidence capture beyond current minimal file links.
- Exit: Pi session history and compaction boundaries are replayable T0 substrate.

### Phase 2: Runtime and retrieval completion
- Finish context-pack composition and retrieval over session/project scopes.
- Complete dev-tab/operator surfaces for checkpoint and canon health.
- Exit: Handoff/retrieval/UI flows consume the canonical layered store.

### Phase 3: Provenance/traversal and specialist surfaces
- Deliver relational provenance queries and visualization adapters.
- Layer specialist dispatch and richer evidence traversal on top.
- Exit: Operators can inspect cross-session/task/file/canon lineage without graph-DB coupling.

### Phase 4: Hardening and rollout
- Validate concurrency, recovery, replay/idempotency, migration, and no-context-loss behavior.
- Publish rollout guidance and operational checks.
- Exit: Mind v2 is release-ready against the current architecture.

---

## Risks and Mitigations
- **Risk**: Treating Pi-native session history as a replacement for Mind storage.
  - **Mitigation**: Keep Pi as source substrate for T0/bootstrap/backfill; keep Mind SQLite canonical for derived semantic/project memory.
- **Risk**: Reintroducing graph-first architectural drift.
  - **Mitigation**: Frame provenance as relational link/query tables with graph-like traversal semantics.
- **Risk**: Overstuffing T1 prose with development trail detail.
  - **Mitigation**: Persist structured evidence links separately from T1 text.
- **Risk**: Blocking developer workflow on compaction/runtime failures.
  - **Mitigation**: Preserve fail-open hook behavior with visible status and replay/requeue paths.
- **Risk**: Task attribution and alignment becoming misleading under stale tags/PRDs.
  - **Mitigation**: Keep the `mind` workstream active while implementation is underway and align tasks/PRD before further rollout.

## Test Strategy
- Contract tests for compaction/checkpoint payload validation and idempotency.
- Storage regression tests for compaction checkpoints, artifact evidence links, and provenance query outputs.
- Runtime tests for fail-open checkpoint processing, observer enqueue/processing, and handoff/retrieval determinism.
- UI/runtime smoke tests for Mission Control and dev-tab status indicators plus recovery controls.
- End-to-end replay/no-context-loss tests spanning compaction, T0/T1 persistence, retries, and recovery.