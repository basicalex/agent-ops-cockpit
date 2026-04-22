# AOC Architecture

Canonical product architecture and crate boundary definitions for Agent Ops Cockpit.

## Status Snapshot (2026-04-22)

The original Mission Control/Mind architecture drift has been substantially corrected.

- `crates/aoc-mission-control/src/main.rs` is no longer a giant mixed-responsibility product blob; the high-value Phase 2 seams are now split into focused modules.
- Mission Control host-side Mind integration is thin:
  - `mind_glue.rs` — thin coordinator
  - `mind_summary_render.rs` — Mind summary/activity presentation
  - `mind_host_render.rs` — host bridge/search helpers
  - `mind_artifact_drilldown.rs` — drilldown/compaction presentation
- Mission Control Overseer rendering is now split into focused modules:
  - `overseer.rs` — section coordinator
  - `overseer_consultation.rs` — consultation packet/render policy
  - `overseer_worker_render.rs` — worker and semantic row rendering
  - `overseer_ops_render.rs` — orchestration tool and timeline rendering
- Pi Mind is now standalone-service driven rather than Pulse-coupled.
- `aoc-mind-service` now exposes standalone Mind command surfaces for:
  - `status`
  - `sync-pi`
  - `watch-pi`
  - `context-pack`
  - `provenance-query`
  - `observer-run`
  - `finalize-session`
- `aoc-mind` now owns the major canonical seams that previously lived in wrapper host code:
  - project-scoped runtime authority
  - detached dispatch policy
  - detached lifecycle shaping
  - runtime tick/health semantics
  - runtime failure/completion policy
  - finalize/export preparation policy
  - context-pack compilation
  - provenance graph/export compilation
- The largest remaining Mind closeout seam is no longer runtime authority. It is the small set of **compatibility/manual command hosts still left in `aoc-agent-wrap-rs`**, especially `insight_retrieve` and legacy `mind_*` / `insight_*` Pulse command handling.

## Mental Model

AOC has three independent product surfaces plus one transport/runtime substrate:

```text
Mission Control  = global fleet / session / worker oversight
Mind             = project-scoped knowledge + runtime state
Control          = operator configuration surface
Pulse            = transport / IPC / telemetry bus
```

## 1. Product Surfaces

### Mission Control — Global Operator Surface

**Role:** cross-session operational oversight in a dedicated tab.

**Responsibilities:**
- fleet/session/worker supervision
- overseer timelines and consultation requests
- delegated runtime visibility
- global health and diff rollups
- operator command dispatch

**Non-goals:**
- project-local knowledge retrieval
- project-local context injection authorship
- per-tab telemetry strip branding
- operator settings editing

### Mind — Project-Scoped Knowledge Runtime

**Role:** project-local knowledge/runtime surface backed by a project store.

**Responsibilities:**
- Pi ingest into project-scoped Mind store
- T0/T1/T2/T3 runtime ownership
- project canon / handshake / watermark semantics
- bounded context-pack and provenance compilation
- observer/finalize flows
- project-local drilldown/search/render inputs

**Non-goals:**
- cross-session fleet supervision
- global delegation/focus orchestration
- generic transport ownership
- operator config UX

### Control — Operator Config Surface

**Role:** settings/config/integration surface.

### Pulse — Transport / IPC Layer

**Role:** shared socket/protocol/runtime substrate.

Pulse is still a real system where it names protocol/runtime transport accurately:
- `aoc-core::pulse_ipc`
- `pulse.sock`
- `AOC_PULSE_SOCK`
- `AOC_PULSE_VNEXT_ENABLED`
- hub/client transport code

Pulse is **not** the canonical product identity for Pi Mind anymore.

---

## 2. Crate Boundaries

| Concern | Canonical Crate | Current Status |
|---|---|---|
| Mind storage/query/render/runtime/finalize/provenance | `aoc-mind` | ✅ canonical |
| Pi JSONL normalization/import semantics | `aoc-pi-adapter` | ✅ canonical |
| Pulse protocol/shared contracts | `aoc-core` | ✅ canonical |
| Mission Control fleet/overseer presentation | `aoc-mission-control` | ✅ host/presenter only |
| Wrapper compatibility bridge | `aoc-agent-wrap-rs` | ⚠ compatibility/manual command host still present |
| Standalone Mind service/bootstrap | `aoc-mind` | ✅ canonical service surface landed |
| Operator configuration TUI | `aoc-control` | ✅ separate |

### Current architectural truth

The main problem is no longer “split the monolith.”

The main remaining architecture question is:

**how much legacy compatibility command/result shaping should remain in `aoc-agent-wrap-rs`, and how much should continue moving into `aoc-mind`?**

The important part is that live runtime authority is already on the correct side:
- standalone Pi Mind no longer depends on Pulse
- Mind runtime/service lease/health state is standalone and project-scoped
- finalize/export planning is Mind-owned
- provenance/context-pack compilation is Mind-owned
- Mission Control is now a host/presenter, not a second Mind implementation

---

## 3. Current Launch / Runtime Model

| Surface | Current Host | Notes |
|---|---|---|
| Mission Control | `aoc-mission-control` | dedicated global operator UI |
| Mind view | `aoc-mission-control --view mind` | acceptable current TUI host |
| Standalone Mind runtime/service | `aoc-mind-service` | canonical project-scoped ingest/runtime/status/query surface |
| Control | `aoc-control` | operator config surface |
| Hub | `aoc-hub-rs` | background transport/runtime host |
| Agent wrapper | `aoc-agent-wrap-rs` | compatibility bridge + transport/host edges |

### Important decision

Near-term extraction of `aoc-mind-tui` is **not** the finish-line requirement.

The success gate is:
- canonical Mind ownership in `aoc-mind`
- thin hosts
- explicit compatibility boundaries
- validated standalone service behavior

A separate `aoc-mind-tui` may still happen later, but it is not required to declare the Mind ownership cut successful.

---

## 4. Data Flow

### Canonical standalone Mind path

```text
Pi session JSONL
   ↓
aoc-pi-adapter
   ↓
aoc-mind / aoc-mind-service
   ↓
.aoc/mind/project.sqlite
   ↓
Pi extension / Mission Control Mind view / standalone status-query surfaces
```

### Compatibility Pulse path

```text
Mission Control / wrapper / legacy operator commands
   ↓
Pulse command transport
   ↓
aoc-agent-wrap-rs compatibility host
   ↓
aoc-mind canonical runtime/query/finalize/provenance APIs
```

The compatibility path still exists, but it is no longer the source of truth for Pi Mind.

---

## 5. Canonical vs Compatibility vs Deprecated Surfaces

### Canonical

These are the source-of-truth Mind surfaces now:

- `aoc-mind` library for:
  - runtime authority
  - service lease/health semantics
  - detached job policy/lifecycle/result shaping
  - finalize/export preparation
  - context-pack compilation
  - provenance graph/export compilation
- `aoc-mind-service` for standalone operator/extension entrypoints:
  - `status`
  - `sync-pi`
  - `watch-pi`
  - `context-pack`
  - `provenance-query`
  - `observer-run`
  - `finalize-session`
- project store path:
  - `.aoc/mind/project.sqlite`

### Compatibility

These still exist for migration/transport compatibility and are intentionally thinner than before:

- wrapper-hosted Pulse command handling for:
  - `mind_handoff`
  - `mind_resume`
  - `mind_finalize_session`
  - `mind_compaction_rebuild`
  - `mind_t3_requeue`
  - `mind_handshake_rebuild`
  - `mind_context_pack`
  - `mind_provenance_query`
  - `insight_*` command family
- wrapper-hosted `insight_retrieve` compilation/result shaping
- Mission Control command emission that still targets compatibility command names

### Deprecated / legacy

These should not be treated as active product truth:

- Pi Mind via Pulse transport
- `.pi/extensions/pulse/index.ts` for Pi Mind behavior
- `Mind ingest unavailable: pulse offline` style UX framing
- `pulse-pane` as an active required product surface
- docs implying Mind is still primarily wrapper/Pulse coupled

---

## 6. What Is Actually Done

### Done enough to count as architectural closure on the big seams

- Pi Mind works standalone without requiring Pulse.
- `aoc-mind-service` is the canonical standalone service surface for Pi Mind operations.
- `aoc-mind` owns runtime authority, provenance, context-pack, and finalize planning.
- wrapper no longer owns the high-value runtime policy seams.
- Mission Control Mind host code is thin.
- Overseer is no longer a dense mixed consultation/render/policy knot.

### Still remaining

These are the remaining bounded closeout items, in priority order:

1. **Wrapper compatibility thinning**
   - especially `insight_retrieve`
   - and any remaining manual command policy that is still better owned by `aoc-mind`

2. **Docs/acceptance alignment**
   - keep architecture docs honest
   - keep compatibility boundaries explicit

3. **Taskmaster closeout hygiene**
   - mark finished subtasks accurately
   - avoid leaving obviously stale in-progress markers for completed seams

---

## 7. Acceptance Status

Mind ownership cut is now best described as:

- **Canonical ownership:** mostly complete
- **Host thinning:** substantially complete
- **Compatibility boundary clarity:** good, now explicit
- **Standalone Pi operation:** complete
- **Mission Control presentation split:** substantially complete
- **Final closeout remaining:** bounded and small

This means the project is past the “architecture still fundamentally wrong” phase.

The remaining work is closeout work, not another large rewrite.
