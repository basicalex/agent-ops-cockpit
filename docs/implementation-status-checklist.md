# AOC Features and Implementation Status Checklist

Snapshot: 2026-03-31

This document is the operator/maintainer overview of the current AOC setup.
It summarizes what is implemented, what is partially implemented, what is intentionally deferred, and which commands/docs validate the shipped surface.

## Status legend

- `Shipped` — implemented, documented enough to use, and backed by targeted validation
- `Partial` — implemented enough to be useful, but with an explicit boundary or deferred follow-up
- `Deferred` — intentionally not part of the current v1 finish path
- `Needs decision` — not clearly blocked technically, but still a scope/cutover choice

## Current status summary

| Area | Status | Notes |
|---|---|---|
| PI-first wrapper/session model | Shipped | `aoc-agent-wrap-rs` is the preferred runtime path and Mind/Pulse wiring follows that model |
| Detached subagent runtime | Shipped | Detached jobs, teams, status, cancel, artifacts, and report output are implemented |
| Specialist role interface | Shipped | Specialist-role surface/runtime guards are green |
| Mind floating project UI | Shipped | Read-only/project-scoped overview, search, activity bridge, and docs are in place |
| Handshake briefing v2 | Shipped | Focus-first, task-aware, with explicit fallback semantics |
| Retrieval across session exports + project canon | Shipped | No longer limited to latest export only |
| Provenance/query foundation | Shipped | Query seeding includes task/file and supports drilldown graph building |
| Operator-facing insight CLI | Shipped | `aoc insight retrieve|provenance|status` and `bin/aoc-insight` exist |
| Mind runtime live validation | Shipped | `scripts/pi/validate-mind-runtime-live.sh` |
| Mind runtime hardening suite | Shipped | `scripts/pi/validate-mind-runtime-hardening.sh` |
| Detached Mind T2/T3 workers | Shipped | Visible as Mind-owned detached jobs; recovery/fallback paths covered |
| Detached Mind T1 worker | Partial | Current detached rollout is T2/T3; T1 remains inline/session-scoped |
| Mind curation/edit flows in floating UI | Deferred | Task 182 subtask 7 remains intentionally secondary |
| Dev-tab Mind feed cutover | Needs decision | Task 131 still appears in architecture docs as a remaining platform cutover item |

## 1) Core session and wrapper model

| Feature | Status | Evidence / entrypoint |
|---|---|---|
| PI-first runtime contract | Shipped | `docs/agents.md` |
| Wrapped Pi session preferred by default | Shipped | `docs/mind-runtime-validation.md` launch-mode section |
| Pulse/Mind integration through wrapper path | Shipped | live validator + Mission Control surfaces |
| Legacy direct-exec fallback still possible | Partial | Explicit fallback path exists, but wrapper is the intended primary route |

Checklist:
- [x] Wrapped Pi path is the default/expected operator path
- [x] Pulse state reaches operator surfaces
- [x] Mind runtime can be exercised non-interactively
- [x] Logs and durable state are written under `.aoc/`

## 2) Detached subagents and orchestration

| Feature | Status | Evidence / entrypoint |
|---|---|---|
| Detached dispatch/status/cancel | Shipped | `.pi/extensions/subagent.ts`, `docs/subagent-runtime.md` |
| Team fanout surface | Shipped | `dispatch_team`, `/subagent-team`, `/subagent-team-detail` |
| Manager-lite Teams tab | Shipped | Pi manager overlay |
| Per-member step/result visibility | Shipped | inspect/handoff/report flows and artifact output |
| Durable job metadata/artifacts | Shipped | `.pi/tmp/subagents/<job-id>/...` |
| Local fail-open fallback | Shipped | sequential/internal fallback kept intentionally safe |

Checklist:
- [x] Team dispatch is exposed in tool + slash command + manager surfaces
- [x] Team jobs preserve per-member outcomes
- [x] Cancellation prevents further fallback launches
- [x] Detached jobs can be inspected after the fact

Primary validation:
- `bash scripts/pi/test-subagent-ux-surface.sh`
- `bash scripts/pi/test-subagent-ux-runtime.sh`

## 3) Specialist role interface

| Feature | Status | Evidence / entrypoint |
|---|---|---|
| Specialist-role tool surface | Shipped | `aoc_specialist_role` integration + Pi surfaces |
| Runtime guards | Shipped | specialist-role runtime guard tests |

Checklist:
- [x] Specialist-role surface exists and is guarded
- [x] Current tree already passes specialist-role validation

Primary validation:
- `bash scripts/pi/test-specialist-role-surface.sh`
- `bash scripts/pi/test-specialist-role-runtime-guards.sh`

## 4) Project Mind UI and Mission Control

| Feature | Status | Evidence / entrypoint |
|---|---|---|
| Floating project-scoped Mind UI | Shipped | `Alt+M`, `/mind`, `bin/aoc-mind-toggle` |
| One named floating pane per tab | Shipped | toggle/reuse semantics documented |
| Project-local filtering | Shipped | Mind view filters by `AOC_PROJECT_ROOT` |
| Read-only overview surface | Shipped | canon, handshake, exports, detached rollup |
| Local search and drilldown | Shipped | `/`, `n`, `N` in Mind view |
| Activity summary + Mission Control bridge | Shipped | project-local guidance inside Mind view |
| Bounded curation/edit flows | Deferred | not part of the current finish path |

Checklist:
- [x] Mind can be opened from any AOC tab without a permanent heavy pane
- [x] Mind overview remains useful even with little/no fresh observer activity
- [x] Search/drilldown are local, bounded, and read-only
- [x] Operators are guided when to stay local vs switch to Fleet/Overview/Overseer
- [ ] In-pane editing/curation exists

Primary docs:
- `docs/mission-control.md`
- `docs/mission-control-ops.md`
- `docs/mind-runtime-validation.md`

## 5) Handshake, retrieval, and provenance

| Feature | Status | Evidence / entrypoint |
|---|---|---|
| Handshake briefing v2 | Shipped | focus-first sections + explicit fallback status |
| Task-aware workstream signals | Shipped | handshake uses Taskmaster-backed project snapshot |
| Explicit degraded/fallback semantics | Shipped | canon/task-state fallback is surfaced, not hidden |
| Retrieval across multiple session exports | Shipped | matching older/newer exports can be ranked |
| Retrieval over project canon | Shipped | project scope remains part of the ranking plan |
| Provenance query seeding by task/file | Shipped | task/file start points supported |
| Citation-first operator surface | Shipped | `aoc insight retrieve|provenance|status` |

Checklist:
- [x] Handshake is no longer just a raw canon dump
- [x] Retrieval is not limited to latest export only
- [x] Provenance can start from conversation/checkpoint/task/file/artifact-linked paths
- [x] Operators have a real CLI surface for retrieval/provenance/status

Primary docs / commands:
- `docs/configuration.md`
- `docs/mind-v2-architecture-cutover-checklist.md`
- `aoc insight retrieve ...`
- `aoc insight provenance ...`
- `aoc insight status`

## 6) Mind runtime rollout and release safety

| Feature | Status | Evidence / entrypoint |
|---|---|---|
| Live runtime validator | Shipped | `scripts/pi/validate-mind-runtime-live.sh` |
| One-command hardening suite | Shipped | `scripts/pi/validate-mind-runtime-hardening.sh` |
| Detached T2/T3 visibility checks | Shipped | hardening suite + docs |
| Stale-lease recovery checks | Shipped | hardening suite |
| Cancel/fallback checks | Shipped | hardening suite |
| Finalization drain/idempotence checks | Shipped | hardening suite |
| Migration safety checks | Shipped | hardening suite |
| Replay/checkpoint rebuild checks | Shipped | hardening suite |
| Deterministic compaction checks | Shipped | hardening suite |

Checklist:
- [x] Fast smoke validation exists
- [x] Broader rollout-confidence command exists
- [x] Detached recovery paths are covered in the bounded suite
- [x] Replay/finalization/compaction safety has explicit regression coverage

Primary commands:
- `bash scripts/pi/validate-mind-runtime-live.sh`
- `bash scripts/pi/validate-mind-runtime-hardening.sh`

Recommended release-confidence command:

```bash
AOC_VALIDATE_MIND_RUNTIME_USE_CARGO=1 bash scripts/pi/validate-mind-runtime-hardening.sh
```

## 7) Known boundaries and intentionally deferred items

| Item | Status | Why it is not a current blocker |
|---|---|---|
| Detached T1 Mind worker | Partial | Current detached rollout intentionally covers T2/T3 first; docs already state T1 remains inline |
| Task 182 subtask 7: Mind edit/curation flows | Deferred | Read-only/project-local Mind overview is already useful and lighter-risk |
| Exact root cause of native Zellij top-bar lag | Deferred | evidence points to whole-machine/session load, not AOC tab-bar logic |
| Taskmaster live state matching repo reality | Partial | PRDs/tests/docs were used as operational truth where Taskmaster lagged |
| Task 131 dev-tab Mind feed cutover | Needs decision | still listed in architecture docs, but not required for the runtime/retrieval/provenance/hardening path already completed |

## 8) Recommended operator/maintainer workflow

### Daily operator path
1. Use the normal wrapped Pi session flow.
2. Open project Mind with `Alt+M` or `/mind`.
3. Use Mission Control Fleet/Overview for detached-runtime supervision.
4. Use `aoc insight ...` for direct retrieval/provenance/status checks.

### Pre-release / confidence path
1. Run targeted Pi surface checks if those areas changed.
2. Run:
   ```bash
   AOC_VALIDATE_MIND_RUNTIME_USE_CARGO=1 bash scripts/pi/validate-mind-runtime-hardening.sh
   ```
3. Use `docs/pi-only-rollout-checklist.md` for broader release closeout.

## 9) Source-of-truth docs

Use these first when evaluating current setup status:

- `docs/implementation-status-checklist.md` — this overview
- `docs/mind-v2-architecture-cutover-checklist.md` — architecture and cutover gate
- `docs/mind-runtime-validation.md` — runtime validation and hardening commands
- `docs/mission-control.md` — operator surface model
- `docs/mission-control-ops.md` — practical operator runbook
- `docs/subagent-runtime.md` — detached subagent UX/runtime contract
- `docs/agents.md` — PI-first runtime contract and release framing

## 10) Current ship/no-ship call

### Safe to treat as shipped now
- Detached subagent runtime/team surface
- Specialist-role surface and guards
- Floating project Mind overview/search/activity bridge
- Handshake v2
- Retrieval + provenance + insight CLI
- Mind live validator and hardening suite

### Still optional / follow-up
- dev-tab Mind feed cutover if still desired
- in-Mind curation/editing flows
- deeper post-v1 polish and release admin work

Overall assessment: the core finish path is implemented and validated strongly enough to operate and release, with the remaining items mostly optional cutover/polish decisions rather than missing core substrate.
