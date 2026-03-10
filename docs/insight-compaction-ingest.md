# Pi Compaction -> AOC Mind Ingestion

This document defines how Pi compaction should integrate with the AOC Mind pipeline so that context is durably preserved and semantic processing can continue without blocking the developer.

## Goal

Treat **Pi compaction** as a first-class AOC Mind checkpoint event.

When compaction happens, AOC should:

1. preserve enough pre-compact context to avoid loss,
2. create or update a durable T0 slice,
3. enqueue or trigger T1 on that slice,
4. surface status/failure visibly,
5. keep the developer fully in control of when compaction happens.

## Why compaction is the right trigger

Compaction is already a natural context boundary:

- the active context is getting dense,
- the user or agent is intentionally compressing state,
- the session is crossing from one working chunk into the next.

That makes compaction an ideal moment to checkpoint Mind state.

## Design principles

### 1. Developer control is preserved

- Developers can compact whenever they want.
- AOC may suggest compaction around a configurable threshold, but compaction remains a deliberate user/agent action.
- Mind ingestion must not force or block compaction.

### 2. Mind capture is mandatory once compaction occurs

Once Pi compaction happens, AOC should treat it as a durable checkpoint opportunity.

### 3. Fail open for workflow, fail loud for observability

- If Mind ingestion fails, compaction should still complete.
- But AOC must surface the failure clearly and preserve enough fallback data for replay/requeue.

### 4. Replay must be possible

If T0/T1 processing does not complete during compaction, operators must be able to rebuild or requeue from persisted artifacts.

## Layer responsibilities

### T0 — durable session continuity

T0 is the lowest-level Mind layer responsible for preserving compacted session continuity.

T0 should:
- ingest raw or compacted context slices,
- persist session/project/tag/timestamp provenance,
- mark trigger source as `pi_compact`,
- retain enough data to rebuild or re-run higher semantic layers later.

T0 should not:
- do strategic planning,
- mutate tasks,
- infer broad project-level alignment.

### T1 — bounded observation distillation

T1 should run on a compacted T0 slice and produce a concise, session-scoped observation bundle.

Expected outputs:
- T1 Summary
- Key Points
- Risks / Blockers
- Open Questions
- Evidence
- Confidence

### T2 — synthesis across slices

T2 should not run on every compaction by default.

Instead, T2 should be triggered by stronger conditions such as:
- handoff,
- multiple T1 slices accumulated,
- task completion,
- explicit operator/main-agent request.

### T3 — project alignment reflection

T3 should not run on every compaction by default.

It should use T0/T1/T2 outputs as substrate for manual or on-demand alignment runs.

## Trigger model

### Manual compaction

Developer or main agent explicitly compacts.

Required AOC behavior:
- capture compaction event,
- persist T0 checkpoint,
- enqueue T1,
- expose success/failure state.

### Threshold-suggested compaction

If AOC/Pi suggests compaction around a threshold (e.g. ~40%), the same ingestion flow should happen only after compaction is actually performed.

## Required post-compaction flow

When Pi compaction occurs:

1. capture compaction metadata:
   - session id
   - project root
   - active tag/workstream if available
   - timestamp
   - trigger source = `pi_compact`
   - compaction reason (manual, threshold, operator requested, etc.)
2. persist a durable T0 slice or equivalent compacted export artifact
3. enqueue/trigger T1 against that slice
4. update runtime telemetry/state
5. expose lifecycle and errors in Mission Control / Pi UI

## Failure handling

If semantic processing fails:

- do not roll back the compaction,
- keep the compacted/fallback artifact,
- record failure state,
- allow replay/requeue.

Minimum fallback requirement:
- every compaction should leave behind enough data to reconstruct a useful summary later.

## Observability requirements

Operators should be able to see:

- last compaction-triggered ingest time,
- last successful T0 checkpoint,
- whether T1 was queued/completed/failed,
- pending backlog depth,
- replay/requeue availability,
- last failure reason.

## Recommended UI surfaces

### Mission Control

Show:
- compaction-triggered ingest health,
- latest checkpoint timestamp,
- T0/T1 success/failure indicators,
- replay/requeue controls.

### Pi UI / extension surface

Show:
- that compaction caused a Mind checkpoint,
- whether the checkpoint processed successfully,
- how to re-run or inspect the latest artifact.

## Suggested artifact model

Compaction should produce or update a durable artifact that can be used for replay and higher-order processing.

Suggested properties:
- session-scoped,
- project-root scoped,
- timestamped,
- linked to subsequent T1 export,
- attributable to the active workstream/tag where possible.

## Operational rules

### Always
- compact -> T0 checkpoint
- compact -> T1 enqueue

### Sometimes
- compact -> mark T2 candidate if enough T1 slices exist

### Not by default
- compact -> T3 run

## Main-agent integration

The main Pi agent should be able to rely on this invariant:

> if compaction happened, AOC attempted to preserve and distill that context chunk.

That gives the main agent a reliable development-time checkpoint boundary.

## Planned implementation phases

### Phase 1 — contract + artifact persistence
- define compaction event contract
- persist T0 checkpoint artifacts with trigger metadata
- ensure replay-safe storage

### Phase 2 — T1 coupling
- automatically enqueue T1 from compaction-triggered checkpoints
- add lifecycle and failure visibility

### Phase 3 — observability and operator recovery
- surface state in Mission Control / Pi UI
- add replay/requeue controls
- expose latest compaction-derived report/artifact

### Phase 4 — deeper main-agent integration
- let the main agent explicitly request/check compaction-derived mind state
- use compaction checkpoints to improve handoff and adaptive context injection

## Relationship to T3

Compaction should strengthen T3 indirectly by ensuring the lower layers are trustworthy.

Reliable compaction-triggered T0/T1 processing gives T2/T3 better substrate and reduces long-session context loss.

## Recommended tracked work

1. Define compaction event contract for Mind
2. Persist T0 checkpoint artifacts on Pi compaction
3. Auto-enqueue T1 on compaction checkpoints
4. Surface compaction-ingest health and recovery controls
5. Add regression coverage for no-context-loss compaction scenarios
