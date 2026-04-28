# AOC Mind architecture

AOC Mind is the project-memory and provenance layer for AOC. It turns agent sessions, task state, handoffs, and selected project artifacts into bounded, cited context that agents can request when needed.

Mind is not a prompt dump. It is a staged memory pipeline with strict scope, redaction, and runtime-state boundaries.

## Mental model

```text
Pi session + AOC tools
  -> T0 replayable session substrate
  -> T1 bounded session observations
  -> T2 session reflection/synthesis
  -> T3 project canon/alignment
  -> handshake + context packs + operator views
```

Agents should start with `aoc-handshake --json`, then request focused Mind context only when intent justifies it.

## What Mind stores

Mind stores derived project knowledge, provenance, and citations:

- session/import/compaction-derived slices
- observer artifacts and summaries
- project canon entries
- links to files, tasks, STM, memory, PRDs, exports, and commits
- retrieval/context-pack evidence
- runtime health and detached-job status summaries

Mind should not store raw secrets. Runtime state does not belong in normal Git commits.

## Phase model

| Phase | Scope | Purpose | Typical trigger |
|---|---|---|---|
| T0 | replayable session substrate | Normalize Pi session/import/compaction events into compact, replayable slices | session import, compaction checkpoint, finalization |
| T1 | bounded session observation | Extract session-local facts, file/task mentions, risk signals, and checkpoint notes | live observer, manual `o`, compaction |
| T2 | bounded session synthesis | Reflect over related T1 artifacts and session deltas | manual chain `O`, background reflector, finalization |
| T3 | project canon | Align project-level knowledge with memory, STM, tasks, PRDs, and exports | backlog worker, finalization, explicit requeue |
| Context pack | request-scoped rendering | Produce cited, token-bounded context for a specific agent reason | explicit context request |
| Handshake | startup metadata | Report AOC/Mind availability without broad recall | agent startup |

T0 is reproducibility substrate. T3 is the durable project-memory layer used by retrieval and operator surfaces.

## Runtime components

| Component | Responsibility |
|---|---|
| `aoc-agent-wrap-rs` | Pi wrapper path; emits/bridges live events, command surfaces, finalization, detached-worker dispatch/fallback |
| `aoc-pi-adapter` | Converts Pi-native session/import/compaction data into AOC Mind event/slice contracts |
| `aoc-core` | Shared contracts for Mind events, semantic stages, provenance, graph/lineage primitives, and context payloads |
| `aoc-storage` | Persistent store APIs for artifacts, checkpoints, provenance links, canon, and query surfaces |
| `aoc-mind` | Pipeline logic: ingestion, redaction, T0/T1/T2/T3 processing, context-pack compilation, detached job policy, renderer helpers |
| `aoc-mind-service` | Standalone project-local service/CLI surface for status, search, context packs, exports, and startup/service health |
| `aoc-mission-control` | Operator UI host for Mind lanes, artifact drilldown, detached Mind worker rollups, and Fleet handoff |
| Pi extensions | Human/agent entrypoints such as `/mind`, `Alt+M`, Mind status/context commands, and panel integration |

## Data flow

```text
1. Pi session emits messages, tool results, branch summaries, compactions.
2. Adapter normalizes events and drops/compacts noisy or unsafe data.
3. Storage records replayable artifacts and provenance links.
4. T1 observes bounded session slices.
5. T2 reflects over related T1/session evidence.
6. T3 updates project canon from eligible T2 + memory + STM + Taskmaster + PRD/export evidence.
7. Retrieval composes cited context packs for explicit agent reasons.
8. Operator UIs show status, lanes, stale/degraded state, and detached worker health.
```

## Retrieval policy

Mind retrieval is lazy and reason-bound.

Default startup path:

```bash
aoc-handshake --json
```

Focused context path:

```bash
aoc-mind-service context-pack \
  --project-root "$PWD" \
  --mode focused \
  --reason "debug previous implementation attempt" \
  --json
```

Use focused context for:

- resuming prior work
- grounding a task/PRD
- debugging previous attempts
- checking prior architectural decisions
- provenance/audit
- when targeted local inspection is insufficient

Avoid broad recall by default.

## Storage and Git policy

Live Mind runtime state belongs under the AOC state directory, not in committed project files.

Default runtime root:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/aoc/mind/
```

Repo-visible files should be validated exports or configuration only. `.aoc/mind/**` runtime databases, locks, caches, and live artifacts must stay ignored/non-committed unless a command explicitly creates a safe export.

Security playbook: [../security/mind-secret-incident-response.md](../security/mind-secret-incident-response.md).

## Secret handling boundary

Mind must assume session/tool output can contain credentials.

Required controls:

- redact common secret patterns before durable storage or broadcast
- isolate runtime artifacts from Git paths
- verify forbidden runtime artifacts are not tracked
- keep prompt/context packs bounded and cited
- rotate/revoke any token that ever appeared in committed history or exported artifacts

Validation:

```bash
scripts/verify-mind-runtime-safety.sh
bash scripts/pi/validate-mind-runtime-hardening.sh
```

## Detached Mind workers

Current detached substrate covers T2 and T3 work.

```text
T2 reflector -> Mind-owned detached job -> result/fallback/lease state
T3 backlog   -> Mind-owned detached job -> canon update/requeue/fallback state
```

T1 remains inline/session-scoped in the current rollout.

Operator surfaces show Mind-owned detached rows with:

- owner plane: `Mind`
- worker kind: T2/T3
- status: queued/running/success/error/cancelled/stale
- attention/recovery guidance
- project/session grouping

Use Mission Control Mind view for project knowledge. Use Fleet view for detached job drilldown/cancel/recovery.

## Command surfaces

Common commands:

```bash
aoc-handshake --json
aoc-mind-service status --json
aoc-mind-service context-pack --project-root "$PWD" --mode focused --reason "..." --json
aoc-mem read
aoc-stm
```

Validation/runbook commands:

```bash
bash scripts/pi/validate-mind-runtime-live.sh
bash scripts/pi/validate-mind-runtime-hardening.sh
scripts/verify-mind-runtime-safety.sh
```

Project task/PRD grounding:

```bash
tm list
tm tag prd show
```

## Operator surfaces

| Surface | Purpose |
|---|---|
| `Alt+M` / `/mind` | project-local Mind overview, search, lanes, artifact drilldown |
| Mission Control Mind mode | richer operator view over Mind artifacts and status |
| Mission Control Fleet mode | detached job groups, cancellation, stale/error recovery |
| `aoc-mind-service status --json` | machine-readable service health and stale/degraded status |
| `aoc-handshake --json` | startup metadata without broad memory loading |

## Failure model

Mind should fail safe:

- service unavailable -> agent continues with local inspection and explicit note
- stale service -> status reports degraded/stale, not healthy
- detached worker spawn fails -> deterministic inline fallback when allowed
- T2/T3 job lease expires -> operator surfaces show stale/recovery guidance
- context pack unavailable -> do not substitute broad raw memory dump
- secret detector hit -> stop, rotate/revoke, isolate runtime state, run safety checks

## Current boundary

Shipped/current:

- Pi-first wrapper/session model
- T0 replay from Pi-native session/import/compaction data
- T1 inline/session observation
- T2/T3 detached substrate with fallback/cancel/stale handling
- project-scoped Mind UI and Mission Control integration
- focused context-pack command surface
- runtime hardening and secret-safety validation

Still evolving:

- broader graph export/visualization
- richer commit-history ingestion as Mind provenance
- in-Mind curation/edit flows
- additional operator polish around detached worker recovery

Maintainer details:

- [Mind v2 architecture cutover checklist](../maintainer/mind-v2-architecture-cutover-checklist.md)
- [Mind runtime validation](../maintainer/mind-runtime-validation.md)
- [Implementation status checklist](../maintainer/implementation-status-checklist.md)
