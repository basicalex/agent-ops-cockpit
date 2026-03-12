# Consultation Packet Contract

## Purpose
Define the bounded structured packet used for cross-agent consultation, Mission Control orchestration, and manager/peer review flows.

This contract is intentionally aligned to AOC's existing architecture:
- Pi session history / compaction outputs remain the source substrate.
- AOC Mind SQLite remains the canonical derived memory system.
- Consultation is derived from checkpoints, T1/T2 outputs, evidence refs, and runtime/task state.
- Raw transcript exchange is not the default consultation path.

## Design goals
- bounded and prompt-safe
- provenance-aware
- checkpoint-aware
- replay-friendly
- fail-open under partial Mind/Pulse/importer availability
- compact enough for Mission Control and fresh-agent prompt injection

## Primary packet fields
A `ConsultationPacket` should carry:
- packet identity and schema version
- session / agent / pane / conversation identity
- active tag and task ids
- current focus summary
- current bounded summary
- current plan items
- blocker list
- latest checkpoint reference
- latest artifact refs
- structured evidence refs
- freshness / source-status / degraded-input metadata
- confidence / uncertainty metadata
- optional help/review request block

## Expected derivation sources
Preferred source families:
1. runtime/overseer session state
2. latest compaction checkpoint
3. T1/T2 bounded summaries and reflections
4. evidence/provenance refs
5. task/tag state
6. importer/replay-backed T0 substrate when direct live data is missing

## Degradation contract
Packet generation must fail open.

If some upstream inputs are partial, stale, or unavailable:
- still produce a packet when core identity/runtime state exists,
- set `freshness.source_status` appropriately,
- record degraded inputs,
- set a bounded `degraded_reason` when useful,
- preserve checkpoint/evidence refs that are still available.

## Non-goals
- transcript-sharing packet format
- graph-native storage schema
- collapsing memory, orchestration, and operational state into one opaque model
- replacing handoff as a supported fallback/operator artifact
