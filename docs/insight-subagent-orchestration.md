# Insight Sub-Agent Orchestration (Architecture Reference)

This document defines the AOC reference architecture for **Insight orchestration**.

> Insight is the product-facing name for the T0/T1/T2 observational memory stack (currently still exposed in parts of the runtime as `mind_*` for compatibility).

## Why this exists

AOC already has live ingest and observer triggers, but operators still need a standardized way to:
- run specialist sub-agents safely,
- bootstrap brown-field repositories from docs-vs-code gaps,
- and project those gaps into Taskmaster + T2 seeds with provenance.

This architecture fills that gap while preserving fail-open runtime guarantees.

## Current baseline (already present)

- Pi extension emits live events (`message_end` ingest, handoff trigger).
- Hub routes Pulse commands to wrapper publishers.
- Wrapper runtime ingests raw events, compacts T0, and runs T1 sidecar triggers.
- Trigger kinds include: `token_threshold`, `task_completed`, `manual_shortcut`, `handoff`.
- T2 detached worker runtime exists and is tested at library level (lease/lock/queue semantics).

## Target architecture

## 1) Supervisor-controlled sub-agent execution

A supervisor component runs bounded sub-agent subprocesses (Pi workers) in three modes:

1. **Dispatch mode** (single specialist)
2. **Chain mode** (sequential pipeline with prior output forwarded)
3. **Parallel experts mode** (fanout + all-settled aggregation)

Each worker gets:
- role-specific system prompt,
- scoped tools,
- isolated session file policy,
- lifecycle telemetry (`queued/running/success/fallback/error`).

### Canonical T1/T2 specialist definitions

#### T1 sub-agent: `insight-t1-observer`
- **Primary goal**: conversation-scoped observation distillation.
- **Allowed tools**: `read,grep,find,ls,bash` (read/analyze only).
- **Must output**:
  - `T1 Summary`
  - `Key Points`
  - `Risks / Blockers`
  - `Open Questions`
  - `Evidence`
  - `Confidence`
- **Must not**:
  - edit source code,
  - merge unrelated conversations,
  - claim unsupported evidence.

#### T2 sub-agent: `insight-t2-reflector`
- **Primary goal**: cross-observation synthesis into prioritized action plans.
- **Allowed tools**: `read,grep,find,ls,bash` (read/analyze only).
- **Must output**:
  - `T2 Reflection`
  - `Strategic Signals`
  - `Priority Actions`
  - `Taskmaster Projection`
  - `Suggested T2 Seeds`
  - `Uncertainty / Validation Needed`
- **Must not**:
  - modify code,
  - auto-create tasks unless explicitly requested,
  - cross-contaminate tags/workstreams by default.

Current agent templates:
- `.pi/agents/insight-t1-observer.md`
- `.pi/agents/insight-t2-reflector.md`
- `.pi/agents/teams.yaml`
- `.pi/agents/agent-chain.yaml`

## 2) Insight tool contract (explicit API)

All runtime actions should be expressed via typed tools/commands:
- `insight_status`
- `insight_ingest`
- `insight_handoff`
- `insight_dispatch`
- `insight_bootstrap`

Design rules:
- typed request/response schemas,
- idempotent request IDs,
- explicit error codes,
- no hidden mutation paths outside command envelopes.

## 3) Brown-field bootstrap flow

`insight_bootstrap` (dry-run by default) runs:

1. **Docs-vs-code analysis**
   - detect missing implementation, drift, weak tests, orphaned behavior.
2. **Gap proposal**
   - classify severity/confidence with evidence references.
3. **Taskmaster projection**
   - propose dependency-aware tasks and phases.
4. **T2 seed projection**
   - queue high-ambiguity/high-risk gaps as reflection seed jobs.

Output is human-reviewable before applying task changes.

## 4) Runtime dataflow

```text
Pi hook events / task signals / manual command
  -> Pulse UDS command envelope
  -> wrapper Insight runtime
  -> T0 compaction + progress update
  -> T1 queue/claim/run (observer)
  -> T2 queue/worker (reflector)
  -> provenance + feed events
  -> Mission Control + footer/UI
```

## 5) Safety and reliability constraints

- Fail-open deterministic fallback is mandatory for semantic failures.
- Trigger queue must enforce single active run per session.
- T2 worker must respect file-lock + lease semantics.
- Brown-field task projection must be confirm-before-write.
- Sub-agent tool scopes are allowlisted per role.

## Technology choices

- **Rust runtime plane**: `aoc-core`, `aoc-agent-wrap-rs`, `aoc-mind`, `aoc-hub-rs`, `aoc-mission-control`
- **TypeScript extension plane**: `.pi/extensions/minimal.ts`
- **Transport**: Pulse UDS NDJSON envelopes
- **Storage**: SQLite-backed MindStore (raw/T0/T1/T2/provenance/queue/lease)
- **Execution**: supervised Pi subprocess workers for isolation

## Comparative patterns incorporated

A reference scan of `/tmp/pi-vs-claude-code` validated practical orchestration patterns:
- background subagents with streaming widgets,
- dispatcher-only primary mode,
- sequential chain pipelines,
- parallel expert fanout.

AOC adopts the useful orchestration primitives while adding stricter contract typing, provenance discipline, and fail-open controls.

## Related PRD

Canonical RPG PRD for this architecture:
- `.taskmaster/docs/prds/insight-subagent-orchestration_prd_rpg.md`

If this scope is active, link this PRD at tag level:

```bash
aoc-task tag prd set .taskmaster/docs/prds/insight-subagent-orchestration_prd_rpg.md --tag sub-agents
```
