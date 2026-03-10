# Insight T3 Alignment Flow

This document defines the planned **T3 alignment layer** for AOC.

> T3 is the project-level reflective layer above T1 observation and T2 synthesis. It is intended to help operators and the main Pi agent answer: _are memory, plan, and execution still aligned?_

## Goal

T3 should produce a high-signal, project-wide alignment report by reconciling:

- T2 reflections and seeds
- `.aoc/memory` architectural decisions
- current STM / handoff context
- Taskmaster progress and dependency reality
- optional PRD / tag-linked planning documents
- eventually Scout/browser-derived evidence and other specialist outputs

T3 is **read/analyze first**. It should default to proposing actions, not mutating state.

## Why T3 exists

T1 and T2 help with observation and planning, but they do not fully answer questions like:

- Are tasks drifting away from architectural intent?
- Are memory decisions stale, contradicted, or missing?
- Does Taskmaster progress match what the project claims to be doing?
- Are there strategic gaps between current work, PRDs, and recent reflections?
- Has the project accumulated too much unresolved ambiguity?

T3 exists to create an explicit project-alignment checkpoint.

## Core outcomes

A successful T3 run should help the operator or main agent understand:

1. **Current alignment state**
2. **Strategic drift or mismatch**
3. **Missing/stale architectural memory**
4. **Taskmaster vs implementation vs plan inconsistencies**
5. **Highest-leverage realignment actions**

## Position in the Insight stack

- **T1**: conversation/workstream observation distillation
- **T2**: synthesis, planning, Taskmaster projection, uncertainty reduction
- **T3**: project-level alignment reflection across memory, plans, and work state

T3 should not replace the main agent. It should provide **bounded strategic context** back to the main agent and developer.

## Trigger modes

### 1. Manual operator trigger

A developer/operator can request an immediate T3 run when they want strategic insight.

Examples:
- "Run T3 now"
- "Give me a project alignment snapshot"
- "Check whether we are drifting from plan"

This should be available through operator-facing surfaces such as:
- Mission Control
- a CLI/command surface
- eventually Pi UI commands/actions

### 2. Main-agent pull trigger

The main Pi agent should be able to invoke T3 at any time during development when it needs strategic alignment help.

Examples:
- before a major implementation push
- after a long exploratory session
- before handoff
- when tasks, memory, and current code direction feel inconsistent

This mode should be **pull-based** and explicit, not hidden background mutation.

### 3. Deferred/background trigger (later)

A queued/background T3 pass may be valuable later, but should come after manual/on-demand flows are stable and observable.

## Inputs

Minimum T3 inputs:

- latest T2 reflections / seeds
- `.aoc/memory` decisions
- current STM / latest handoff snapshot
- Taskmaster state for current project/tag
- tag PRD / task PRD linkage when available

Future optional inputs:

- Scout/browser recon reports
- deployment/ops signals (e.g. Vercel)
- test health summaries
- mission control feed rollups
- compaction-derived T0/T1 checkpoints from Pi sessions

## Output contract

T3 should return markdown with these sections, in order:

1. `## T3 Alignment Summary`
2. `## Strategic Drift`
3. `## Memory / Task Mismatch`
4. `## Decision Gaps`
5. `## Priority Realignment Actions`
6. `## Suggested Memory / Task Updates`
7. `## Confidence / Validation Needed`

## Guardrails

T3 must:

- preserve provenance for claims
- separate evidence from recommendation
- remain scoped to the active project/tag unless explicitly broadened
- default to read/analyze-only behavior
- avoid automatic task or memory mutation unless explicitly confirmed

T3 must not:

- silently rewrite memory
- silently create or close Taskmaster tasks
- blur unrelated workstreams together
- claim alignment confidence without citing evidence

## Typical workflow integration

### Flow A: Operator requests T3 manually

1. Operator triggers T3
2. T3 gathers current memory + T2 + Taskmaster + handoff context
3. T3 emits an alignment report
4. Operator or main agent chooses whether to:
   - add memory
   - adjust tasks
   - refine plan
   - request deeper specialist analysis

### Flow B: Main agent pulls T3 during development

1. Main agent detects uncertainty/drift
2. Main agent invokes T3
3. T3 returns project-level alignment report
4. Main agent integrates findings into planning and execution

### Flow C: Handoff quality check

1. T1/T2 handoff chain runs
2. T3 checks whether handoff reflects project reality and current priorities
3. Main agent or operator uses that report to improve handoff quality

## UI/runtime expectations

T3 should be visible and observable.

Recommended surfaces:

- Mission Control: queue depth, last run, success/failure, latest report
- Pi UI: explicit invoke/read-report flow
- wrapper/runtime telemetry: queued/running/completed/requeued/dead-lettered

## Implementation phases

### Phase 1 — Contract and operator flow

- define canonical T3 contract
- support manual trigger
- support report rendering and artifact storage
- ensure no-write default behavior

### Phase 2 — Main-agent pull integration

- expose T3 to the main agent as an explicit invocation path
- allow the main agent to request T3 during normal development
- keep provenance and observability intact

### Phase 3 — Better source aggregation

- reconcile T2 + memory + Taskmaster + PRD context more deeply
- add stronger project-mind artifact generation
- improve mismatch detection and confidence scoring

### Phase 4 — Broader specialist integration

- incorporate Scout/browser evidence
- incorporate ops/deploy evidence
- optionally allow periodic/background alignment checks

## Relationship to existing runtime pieces

The repo already contains T3-oriented storage/runtime scaffolding, including:

- backlog jobs
- runtime leases
- detached worker logic
- Mission Control T3 stats and controls

This document clarifies the intended **product behavior and operator workflow** on top of that runtime substrate.

## Recommended first tracked work

1. Define T3 prompt/output contract
2. Implement manual T3 trigger
3. Implement main-agent pull path
4. Surface T3 state/results in Mission Control and Pi UI
5. Connect T3 more directly to memory + Taskmaster + PRD evidence
6. Add tests for trigger, reporting, and non-destructive behavior
