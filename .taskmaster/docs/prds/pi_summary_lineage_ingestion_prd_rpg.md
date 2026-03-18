# Pi Summary Lineage Ingestion for AOC Mind PRD (RPG)

## Problem Statement
AOC already treats Pi compaction as a durable Mind checkpoint, but it still uses Pi summarization mostly as a trigger instead of as a richer summary-lineage substrate. Important Pi-native signals remain under-modeled: branch summaries from `/tree` navigation, split-turn compaction metadata, cumulative file-operation carry-forward, manual compaction instructions, and exact kept/summarized cut boundaries via `firstKeptEntryId`.

Without these signals, Mind loses fidelity at exactly the moments when Pi is compressing or switching context. That weakens replay precision, reduces T1/T2 confidence, hides operator intent, and misses a high-signal source of branch-local decisions and file-surface continuity.

## Target Users
- AOC maintainers extending Mind’s Pi-native ingestion model and provenance schema.
- Developers relying on long-lived Pi sessions, compaction, and `/tree` branch navigation without losing continuity.
- Operators using Mission Control / Mind panes to inspect checkpoint health, replayability, and summary fidelity.

## Success Metrics
- Pi branch summaries are ingested as first-class Mind checkpoints and can enqueue T1 deterministically.
- Compaction checkpoints persist split-turn and `firstKeptEntryId` lineage metadata end-to-end.
- Manual/custom compaction summary payloads and instructions are durably stored and visible in operator surfaces.
- Cumulative read/modified file surfaces carry forward across successive summary checkpoints.
- Mission Control exposes summary-checkpoint kind, fidelity, replay status, and latest T1 outcome without regressions to existing compaction flows.

---

## Capability Tree

### Capability: Pi Summary Checkpoint Capture
Capture Pi-native summary events as durable Mind inputs with explicit checkpoint kinds and lineage metadata.

#### Feature: Rich compaction checkpoint metadata
- **Description**: Extend compaction checkpoint payloads to preserve Pi-native summary-lineage fields.
- **Inputs**: `session_before_compact` preparation/event data, compaction entry data, optional custom summary/instructions.
- **Outputs**: Normalized checkpoint payload with `firstKeptEntryId`, split-turn metadata, summary text, details, trigger mode, and instructions.
- **Behavior**: Preserve fail-open compaction semantics while upgrading checkpoint fidelity.

#### Feature: Branch summary checkpoint capture
- **Description**: Convert Pi `/tree` branch summary events into a first-class checkpoint flow.
- **Inputs**: `session_before_tree` preparation/event data, target/ancestor ids, branch summary text/details.
- **Outputs**: `mind_branch_summary_checkpoint` payloads with branch lineage metadata.
- **Behavior**: Capture summaries only when produced, preserve navigation context, and avoid blocking branch switching.

### Capability: Summary Lineage Persistence
Persist checkpoint kinds, lineage, and cumulative summary details in canonical Mind storage.

#### Feature: Generalized checkpoint schema
- **Description**: Store compaction and branch summary checkpoints under a shared summary-lineage model.
- **Inputs**: Normalized checkpoint payloads from Pi hooks and replay/import paths.
- **Outputs**: Durable rows for checkpoint kind, lineage ids, summary text, detail JSON, trigger metadata, and fidelity flags.
- **Behavior**: Remain idempotent, replay-safe, and backward-compatible with existing compaction checkpoints.

#### Feature: Cumulative file-surface carry-forward
- **Description**: Normalize and inherit read/modified file surfaces across successive summary checkpoints.
- **Inputs**: Pi summary details, prior checkpoint detail state, wrapper-derived trail evidence.
- **Outputs**: Cumulative `read_files` / `modified_files` state on each checkpoint.
- **Behavior**: Prefer Pi-native details when present, with deterministic fallback derivation.

### Capability: Summary-Aware Distillation and Provenance
Use Pi-native summary-lineage metadata to improve T1/T2 substrate quality, confidence, and replay semantics.

#### Feature: Summary-aware T1 enqueue
- **Description**: Trigger T1 on branch summary checkpoints and richer compaction checkpoints.
- **Inputs**: Stored summary checkpoints, checkpoint kind, summary text/details, fidelity flags.
- **Outputs**: T1 artifacts and provenance rows linked to checkpoint kind and lineage ids.
- **Behavior**: Treat Pi summary text as privileged substrate when available; keep deterministic fallback authoritative.

#### Feature: Fidelity-aware provenance
- **Description**: Preserve checkpoint fidelity characteristics such as split-turn boundaries and summary origin.
- **Inputs**: Checkpoint metadata and semantic runtime outcomes.
- **Outputs**: Provenance annotations for `kind`, `summary_origin`, `is_split_turn`, and `first_kept_entry_id`.
- **Behavior**: Lower confidence or flag degraded fidelity when summaries represent split turns or partial state.

### Capability: Operator Visibility and Recovery
Expose summary checkpoint state, replayability, and recovery controls in Mission Control / Mind surfaces.

#### Feature: Summary checkpoint observability
- **Description**: Show latest compaction and branch summary checkpoint status, kind, and fidelity.
- **Inputs**: Stored checkpoints, checkpoint slices, observer runtime rows, evidence links.
- **Outputs**: UI labels, health states, replay indicators, and drilldown metadata.
- **Behavior**: Make branch-summary and split-turn cases visible without overwhelming the default view.

#### Feature: Replay/requeue parity
- **Description**: Provide rebuild/requeue controls for branch-summary-derived substrate alongside compaction.
- **Inputs**: Stored checkpoints, marker provenance, checkpoint kind.
- **Outputs**: Recovery commands and rebuilt slices/checkpoint-derived T1 runs.
- **Behavior**: Preserve fail-open workflow and idempotent operator recovery.

---

## Repository Structure

```text
project-root/
├── .pi/
│   └── extensions/
│       └── minimal.ts
├── crates/
│   ├── aoc-agent-wrap-rs/
│   ├── aoc-core/
│   ├── aoc-mind/
│   ├── aoc-mission-control/
│   └── aoc-storage/
├── docs/
│   └── insight-compaction-ingest.md
└── .taskmaster/
    └── docs/prds/
```

## Module Definitions

### Module: `.pi/extensions/minimal.ts`
- **Maps to capability**: Pi Summary Checkpoint Capture
- **Responsibility**: Hook Pi compaction/tree events and emit normalized summary-checkpoint payloads into Pulse.

### Module: `crates/aoc-core/src/mind_contracts.rs`
- **Maps to capability**: Summary Lineage Persistence
- **Responsibility**: Define checkpoint kinds, lineage metadata, normalized detail payloads, and slice-building contracts.

### Module: `crates/aoc-storage`
- **Maps to capability**: Summary Lineage Persistence
- **Responsibility**: Persist checkpoint rows, slices, lineage ids, detail JSON, and cumulative file surfaces.

### Module: `crates/aoc-agent-wrap-rs/src/main.rs`
- **Maps to capability**: Pi Summary Checkpoint Capture + Summary-Aware Distillation and Provenance
- **Responsibility**: Validate Pulse payloads, persist checkpoints/slices, enqueue T1, rebuild/requeue summary checkpoints, and attach provenance.

### Module: `crates/aoc-mind/src/lib.rs`
- **Maps to capability**: Summary-Aware Distillation and Provenance
- **Responsibility**: Consume summary-lineage substrate during T1/T2 and preserve fidelity-aware provenance.

### Module: `crates/aoc-mission-control/src/main.rs`
- **Maps to capability**: Operator Visibility and Recovery
- **Responsibility**: Surface summary checkpoint kind/fidelity/health and replay controls in Mind panes.

### Module: `docs/insight-compaction-ingest.md`
- **Maps to capability**: Pi Summary Checkpoint Capture
- **Responsibility**: Extend the compaction doc into a broader summary-lineage contract that also covers branch summaries and split-turn fidelity.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **Summary checkpoint contract**: shared payload/schema for compaction and branch summary kinds.
- **Storage compatibility rules**: backward-compatible persistence and migration strategy for existing compaction data.
- **Operator semantics**: visible definitions for kind, fidelity, replayability, and trigger mode.

### Capture Layer (Phase 1)
- **Rich compaction hook capture**: Depends on [Summary checkpoint contract]
- **Branch summary hook capture**: Depends on [Summary checkpoint contract]
- **Normalized detail extraction**: Depends on [Summary checkpoint contract, Storage compatibility rules]

### Persistence Layer (Phase 2)
- **Generalized summary checkpoint persistence**: Depends on [Rich compaction hook capture, Branch summary hook capture, Storage compatibility rules]
- **Cumulative file-surface carry-forward**: Depends on [Generalized summary checkpoint persistence, Normalized detail extraction]
- **Checkpoint slice builders**: Depends on [Generalized summary checkpoint persistence]

### Runtime Layer (Phase 3)
- **Summary-aware T1 enqueue**: Depends on [Checkpoint slice builders, Generalized summary checkpoint persistence]
- **Fidelity-aware provenance**: Depends on [Summary-aware T1 enqueue]
- **Replay/requeue parity**: Depends on [Checkpoint slice builders, Generalized summary checkpoint persistence]

### Surface Layer (Phase 4)
- **Mission Control summary checkpoint visibility**: Depends on [Fidelity-aware provenance, Replay/requeue parity]
- **Docs and rollout guidance**: Depends on [Mission Control summary checkpoint visibility, Storage compatibility rules]

---

## Development Phases

### Phase 0: Contract and storage design
- Define shared summary-checkpoint kinds and metadata.
- Decide whether to extend compaction tables or generalize to multi-kind checkpoint storage.
- Exit: compatible schema/contract approved with no circular dependencies.

### Phase 1: Pi hook capture
- Extend `session_before_compact` payload richness.
- Add `session_before_tree` branch summary capture.
- Exit: Pulse receives normalized summary-checkpoint payloads with kind-specific fields.

### Phase 2: Persistence and replay substrate
- Persist generalized checkpoints and slices.
- Carry forward cumulative file surfaces and lineage metadata.
- Exit: summary checkpoints are queryable, replay-safe, and idempotent.

### Phase 3: Runtime and provenance integration
- Enqueue T1 from branch summary checkpoints.
- Attach summary-origin and fidelity metadata to semantic provenance.
- Exit: T1 artifacts and provenance reflect checkpoint kind and fidelity.

### Phase 4: UI, recovery, and validation
- Surface branch summary / split-turn status in Mission Control.
- Add rebuild/requeue parity and regression coverage.
- Exit: operators can inspect and recover summary checkpoints without regressions to existing compaction workflows.

---

## Risks and Mitigations
- **Risk**: Generalizing checkpoint storage breaks existing compaction flows.
  - **Mitigation**: preserve backward-compatible compaction semantics and migrate incrementally behind compatibility helpers.
- **Risk**: Pi hook payloads vary across versions or omit optional details.
  - **Mitigation**: make all richer metadata optional and preserve deterministic fallback derivation.
- **Risk**: Branch summary ingestion creates noisy or low-value T1 churn.
  - **Mitigation**: gate T1 enqueue on actual summary creation and keep T2/T3 non-default.
- **Risk**: Split-turn fidelity is surfaced but not understood by operators.
  - **Mitigation**: add compact labels and drilldown text that explains degraded boundary fidelity.
- **Risk**: Cumulative file carry-forward grows unbounded.
  - **Mitigation**: normalize, dedupe, and cap stored file lists while keeping evidence links authoritative.

## Test Strategy
- Contract tests for rich compaction and branch summary checkpoint payload validation.
- Storage migration tests for backward-compatible reads/writes of existing compaction rows.
- Runtime tests for T1 enqueue/idempotency across compaction and branch-summary checkpoints.
- Provenance tests for `kind`, `summary_origin`, `is_split_turn`, and `first_kept_entry_id` annotations.
- Mission Control rendering tests for summary checkpoint kind/fidelity/recovery labels.
- End-to-end replay tests covering branch summary ingest, compaction split-turn metadata, and rebuild/requeue parity.
