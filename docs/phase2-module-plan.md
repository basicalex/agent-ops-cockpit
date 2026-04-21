# Mission Control Refactor Status and Next Architecture Plan

## Refactor Status

The original Phase 2 module split is effectively complete.

### Landed Mission Control modules

```
crates/aoc-mission-control/src/
├── app.rs
├── collectors.rs
├── config.rs
├── consultation_memory.rs
├── diff.rs
├── fleet.rs
├── health.rs
├── hub.rs
├── input.rs
├── mind_artifact_drilldown.rs
├── mind_glue.rs
├── mind_host_render.rs
├── ops.rs
├── overview.rs
├── overview_support.rs
├── overseer.rs
├── render_host.rs
├── shared_render.rs
├── source_parse.rs
├── tests.rs
├── theme.rs
├── wire.rs
└── work.rs
```

### Current shape

- `main.rs` is now a small crate root/runtime/types file (~900 LOC), not the old 15k monolith.
- Canonical Mind query/loading logic moved into `crates/aoc-mind/src/query.rs`.
- `mind_glue.rs` is now a thin host coordinator rather than a second Mind implementation.
- Legacy embedded pulse-pane routing in Mission Control was removed instead of being modularized.
- Validation baseline is green for `aoc-mind` and `aoc-mission-control`.

## What changed about the plan

The old plan assumed the main problem was file splitting inside Mission Control.

That is no longer true.

The dominant remaining architecture problem is now **Mind runtime ownership**:

- direct Pi JSONL ingest exists canonically in `aoc-pi-adapter`
- shared Mind query/render logic exists canonically in `aoc-mind`
- detached project-scoped T2/T3 worker semantics already exist
- but live runtime/bootstrap/admission still lives largely inside `aoc-agent-wrap-rs`

## Next Plan: Standalone Project-Scoped Mind

### Goal

Move Mind from wrapper-coupled runtime behavior to a standalone project-scoped service/runtime that keeps project knowledge advancing even when Pulse is offline.

### Phase A — landed in this run

Foundation code now exists in `crates/aoc-mind`:

- `standalone.rs`
  - canonical project runtime paths
  - Pi session discovery helpers
  - direct `aoc-pi-adapter` ingest into project Mind store
- `src/bin/aoc-mind-service.rs`
  - `status`
  - `sync-pi`
  - `watch-pi`

This is the first real Option C foundation slice.

### Phase B — next implementation target

Extract live Mind runtime ownership from `aoc-agent-wrap-rs` into `aoc-mind`:

1. reusable service/runtime bootstrap
2. one-owner-per-project lease/health snapshot
3. T1 admission extraction from wrapper-local runtime
4. finalize/export ownership extraction
5. wrapper compatibility bridge instead of wrapper-owned runtime

### Phase C — detached/runtime convergence

Finish the ownership cut while preserving existing detached semantics:

1. standalone-owned T2/T3 admission and reconciliation
2. Mission Control visibility into standalone health/freshness
3. Pi context-pack/provenance retrieval backed by standalone-owned state
4. launch/bootstrap integration so the standalone service becomes default

## Tracking

- Mission Control refactor completion: task `184` ✅ done
- Standalone Mind service cutover: task `190` 🚧 in progress
- Full PRD: `.taskmaster/docs/prds/task-190_project_scoped_mind_standalone_service_prd_rpg.md`
