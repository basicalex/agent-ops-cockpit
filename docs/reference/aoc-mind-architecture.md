# AOC Mind architecture

AOC Mind is the project-memory and provenance layer for AOC. It turns agent sessions, task state, handoffs, and selected project artifacts into bounded, cited context that agents can request when needed.

Mind is not a prompt dump. It is a staged memory pipeline with strict scope, redaction, and runtime-state boundaries.

## Mental model

```text
OMP/Pi session + AOC tools
  -> T0 replayable session substrate
  -> T1 bounded session observations
  -> T2 session reflection/synthesis
  -> T3 project canon/alignment
  -> evidence packs + Mnemopi candidate memories + handshake metadata
```

Agents should start with `aoc-handshake --json`, use OMP/Mnemopi as active memory, then request focused AOC Mind evidence only when intent justifies it.

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
| `aoc-mind-service serve` | OMP/Herdr-era project service loop; ingests Pi sessions, heartbeats health, runs T1 threshold checks, and ticks T2/T3 queues without Zellij or wrapper ownership |
| `aoc-pi-adapter` | Converts Pi-native session/import/compaction data into AOC Mind event/slice contracts |
| `aoc-core` | Shared contracts for Mind events, semantic stages, provenance, graph/lineage primitives, and context payloads |
| `aoc-storage` | Persistent store APIs for artifacts, checkpoints, provenance links, canon, and query surfaces |
| `aoc-mind` | Pipeline logic: ingestion, redaction, T0/T1/T2/T3 processing, evidence/context-pack compilation, detached job policy, renderer helpers |
| `aoc-mind-service` | Project-local CLI/API surface for status, service loop, context packs, evidence packs, Mnemopi candidates, provenance, and health |
| `.omp/extensions/aoc-mind.ts` | Read-only OMP tool exposing Mind status/evidence/provenance/dry-run candidates to agents |
| `aoc-agent-wrap-rs` | Compatibility wrapper path only; no longer the target owner for default Mind background processing |

## Data flow

```text
1. Pi session emits messages, tool results, branch summaries, compactions.
2. Adapter normalizes events and drops/compacts noisy or unsafe data.
3. Storage records replayable artifacts and provenance links.
4. T1 observes bounded session slices.
5. T2 reflects over related T1/session evidence.
6. T3 updates project canon from eligible T2 + memory + STM + Taskmaster + PRD/export evidence.
7. Retrieval composes cited evidence/context packs for explicit agent reasons.
8. OMP tools and CLI status expose stale/degraded state and detached worker health without broad prompt injection.
```

## Retrieval policy

Mind retrieval is lazy and reason-bound.

Default startup path:

```bash
aoc-handshake --json
```

Focused evidence path:

```bash
aoc-mind-service evidence-pack \
  --project-root "$PWD" \
  --reason "debug previous implementation attempt" \
  --mode focused \
  --json
```

Context-pack compatibility path:

```bash
aoc-mind-service context-pack --project-root "$PWD" --mode focused --reason "..." --json
```

Supported context-pack modes are `startup`, `tag-switch`, `focused`, `resume`, `handoff`, and `dispatch`. Unknown modes are rejected instead of silently falling back. `focused` mode is reason-bound and does not include volatile STM by default unless the reason explicitly asks to resume, continue, inspect STM, or prepare/use a handoff.

Use focused context for:

- resuming prior work
- grounding a task/PRD
- debugging previous attempts
- checking prior architectural decisions
- provenance/audit
- when targeted local inspection is insufficient

Avoid broad recall by default.

## Mnemopi integration boundary

OMP/Mnemopi remains the active memory backend. AOC Mind augments it with cited historical evidence and dry-run candidate memories:

```bash
aoc-mind-service mnemopi-candidates \
  --project-root "$PWD" \
  --reason "promote durable project decisions about memory architecture" \
  --json
```

Candidate output is conservative and derived: it includes source refs and provenance seeds, but it does not write to Mnemopi. Promotion must be an explicit reviewed action; startup, handshake, init, and normal launch must not auto-promote or bulk import memories.

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

## Mind service workers

The default OMP/Herdr path is `aoc-mind-service serve`:

```bash
aoc-mind-service serve --project-root "$PWD" --agent-id omp --json
```

The service loop ingests the latest Pi session, runs T1 token-threshold checks, ticks the T2 reflector queue, ticks the T3 backlog queue, heartbeats the project health lease, and reports queue depths/stale state. Use `--once` for tests and bounded smoke checks.

T1 remains session-scoped. T2 and T3 keep their lease/queue semantics and inline fallback behavior where available.
Service/status surfaces report Mind-owned detached rows with:

- owner plane: `Mind`
- worker kind: T2/T3
- status: queued/running/success/error/cancelled/stale
- attention/recovery guidance
- project/session grouping

Use `aoc_mind` or `aoc-mind-service status/evidence/provenance` for default Herdr/OMP work. Legacy Mission Control views are compatibility-only.

## Command surfaces

Common commands:

```bash
aoc-handshake --json
aoc-mind-service status --project-root "$PWD" --json
aoc-mind-service serve --project-root "$PWD" --agent-id omp --once --json
aoc-mind-service evidence-pack --project-root "$PWD" --mode focused --reason "..." --json
aoc-mind-service mnemopi-candidates --project-root "$PWD" --reason "..." --json
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
| `Alt+M` / `/mind` | instant Pi-native Mind overlay for status, focused/resume context, observer, finalize, store, and debug actions |
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

- OMP-era `aoc-mind-service serve` owner for default project T0/T1/T2/T3 background processing
- T0 replay from Pi-native session/import/compaction data
- T1 session observation via token-threshold/manual/finalization triggers
- T2/T3 queue/lease substrate with fallback/stale handling
- focused context-pack and evidence-pack command surfaces
- dry-run Mnemopi candidate synthesis with citations/provenance metadata
- read-only OMP `aoc_mind` tool surface
- runtime hardening and secret-safety validation

Still evolving:

- reviewed non-dry-run promotion into Mnemopi
- richer semantic ranking over T1/T2/T3 evidence
- broader graph export/visualization
- richer commit-history ingestion as Mind provenance
- in-Mind curation/edit flows
- additional operator polish around detached worker recovery

Maintainer details:

- [Mind v2 architecture cutover checklist](../maintainer/mind-v2-architecture-cutover-checklist.md)
- [Mind runtime validation](../maintainer/mind-runtime-validation.md)
- [Implementation status checklist](../maintainer/implementation-status-checklist.md)
