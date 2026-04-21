# AOC Architecture

Canonical product architecture and crate boundary definitions for Agent Ops Cockpit.

## Status Snapshot (2026-04-17)

The Mission Control refactor is no longer in the original monolith state.

- `crates/aoc-mission-control/src/main.rs` is down to roughly **900 lines** and now acts as crate root/runtime wiring rather than a 15k-line product blob.
- Mind query/loading logic now lives canonically in `crates/aoc-mind/src/query.rs`.
- Mission Control-specific Mind hosting is split into focused adapter modules:
  - `mind_glue.rs` — thin coordinator
  - `mind_artifact_drilldown.rs` — host-side drilldown/compaction presentation
  - `mind_host_render.rs` — host-side search/activity bridge helpers
  - `consultation_memory.rs` — consultation persistence/markdown helpers
- Legacy embedded Mission Control pulse-pane mode has been removed as a first-class surface; compatibility labels now degrade to normal Mission Control behavior.
- New standalone Mind foundation now exists in `crates/aoc-mind/src/standalone.rs` with direct Pi JSONL ingest helpers and `aoc-mind-service` bootstrap commands (`status`, `sync-pi`, `watch-pi`).

The next architectural move is no longer “split the Mission Control monolith” — that work is effectively done. The next move is to finish the ownership cut so **Mind becomes a project-scoped standalone runtime/service instead of remaining operationally coupled to `aoc-agent-wrap-rs` and Pulse transport**.

## Mental Model

AOC has **three independent surfaces** with clean product boundaries:

```
┌──────────────────────────────────────────────────────────────┐
│                    GLOBAL (cross-session)                    │
│                                                              │
│   Mission Control  ──  Fleet / Session / Agent oversight     │
│        │             ──  Overseer / Delegation / Commands    │
│        │             ──  Health & Diff rollups               │
│   (dedicated Zellij tab)                                     │
└──────────────────────────┬───────────────────────────────────┘
                           │  Pulse UDS / Hub IPC
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
   ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
   │   Pulse      │ │    Mind      │ │   Control    │
   │   (per-tab)  │ │ (per-project)│ │ (operator)   │
   │              │ │              │ │              │
   │ Agent status │ │ Project      │ │ Settings,    │
   │ telemetry    │ │ knowledge,   │ │ layouts,     │
   │ strip        │ │ retrieval,   │ │ integrations │
   │              │ │ synthesis    │ │              │
   └──────────────┘ └──────────────┘ └──────────────┘
```

## 1. Surfaces (Product Concepts)

### Mission Control — Global Fleet Orchestrator

**Role:** Session-crosscutting operational oversight. Lives in a dedicated Zellij tab.

**Responsibilities:**
- Fleet overview: all sessions, agents, health across the workspace
- Overseer view: per-worker status, timelines, consultation commands
- Session delegation: dispatch, focus, stop actions across sessions
- Health & Diff rollups: cross-agent change summary
- Operator commands via Pulse (focus_tab, stop_agent, etc.)

**Does NOT do:**
- Project-scoped knowledge retrieval (that's Mind)
- Per-project context injection (that's Mind)
- Settings / operator config (that's Control)
- Pulse telemetry strip (that's Pulse)

### Mind — Project-Scoped Knowledge Surface

**Role:** Floating, project-local knowledge pane. Invoked from any project tab.

**Responsibilities:**
- Project knowledge retrieval / search
- T0/T1/T2/T3 compaction pipeline state display
- Canon / handshake / watermark inspection
- Observer feed activity (T1 pre-filter, T2 synthesis, T3 output)
- Provenance / artifact drilldown
- Injection triggers (startup, tag-switch, resume, handoff)
- Search across project's Mind artifacts

**Does NOT do:**
- Cross-session fleet view (that's Mission Control)
- Agent dispatch commands (that's Mission Control)
- Per-tab telemetry strip (that's Pulse)
- Operator settings config (that's Control)

### Pulse — Per-Tab Telemetry Strip

**Role:** Minimal live status strip embedded in every project work tab.

**Responsibilities:**
- Current agent status (running / busy / idle / error / needs-input)
- Active task name & progress
- Minimal diff summary indicator
- Hub connection health indicator

**Does NOT do:**
- Overseer / consultation views
- Mind knowledge retrieval
- Fleet overview
- Anything interactive beyond status display

### Control — Operator Config Surface

**Role:** Alt+C config / setup / integrations panel.

**Responsibilities:**
- Theme management
- Layout defaults and custom layout creation
- RTK routing config
- PI agent installer
- PI compaction presets
- Agent Browser + Search config
- AOC Map microsite
- Vercel CLI integration access

**Does NOT do:**
- Runtime agent monitoring
- Mind knowledge display
- Pulse telemetry

---

## 2. Crate Boundaries (Implementation)

These align product concepts to code. Current state and next target:

| Concept | Canonical Crate | Current State |
|---------|------------------|---------------|
| Mind query/loading/parsing | `aoc-mind` | ✅ canonical in `query.rs` |
| Mind shared render primitives | `aoc-mind` | ✅ canonical in `render.rs` |
| Mind host adaptation inside Mission Control | `aoc-mission-control` | ✅ thin host adapter modules |
| Fleet / Overseer / Delegation | `aoc-mission-control` | ✅ modularized out of the old monolith |
| Pulse IPC protocol | `aoc-core` | ✅ canonical |
| Pi JSONL normalization/import | `aoc-pi-adapter` | ✅ canonical |
| Wrapper compatibility bridge | `aoc-agent-wrap-rs` | ⚠ still owns too much live Mind runtime/bootstrap |
| Standalone Mind bootstrap/service | `aoc-mind` | 🚧 foundation landed; ownership cut not complete |
| Operator config TUI | `aoc-control` | ✅ separate surface |

### The Core Problem

The main architecture problem has shifted.

The Mission Control file split is no longer the primary blocker. The primary blocker is now the remaining **runtime ownership** still held in `aoc-agent-wrap-rs`, even though Pi-side Mind ingest/status/manual operations are already standalone-service driven.

- Mind is conceptually a **project-scoped** surface
- Pi JSONL ingest already has a canonical adapter in `aoc-pi-adapter`
- detached Mind workers already use project-scoped `owner_plane=mind` semantics
- Pi now consumes project-local Mind state through `aoc-mind-service` without requiring Pulse transport

So the real next-step architecture target is:

1. `aoc-mind` owns canonical project runtime roots, ingest bootstrap, and standalone service lifecycle
2. `aoc-agent-wrap-rs` shrinks toward a telemetry/compatibility bridge
3. Mission Control remains a global operator surface only
4. Pi consumes fresh project-local Mind state directly from standalone Mind service/store state

### Target Crate Layout

```
aoc-core/          # Shared types: pulse_ipc, mind_contracts, mind_observer_feed,
                   #   session_overseer, consultation_contracts, zellij_cli, etc.

aoc-mind/          # Mind runtime library (already correct)
                   #   lib.rs           — storage APIs, T3 worker, canon, handshake
                   #   t3_runtime.rs    — T3 synthesis, revision lifecycle
                   #   observer_runtime.rs  — observer feed processing
                   #   reflector_runtime.rs — reflector / projection
                   #   + NEW: render.rs — shared Mind TUI rendering (Ratatui lines)

aoc-mission-control/   # Mission Control binary (fleet/overseer/delegation only)
                   #   main.rs          — App, Pulse IPC, Mission Control views
                   #   pulse_tabs.rs    — Pulse strip rendering
                   #   fleet.rs         — Fleet view rendering
                   #   overseer.rs      — Overseer, consultation, commands
                   #   diff.rs          — Diff view
                   #   health.rs        — Health view
                   #   (removes all Mind rendering + logic)

aoc-control/       # Control binary (Alt+C config surface, already separate)
```

---

## 3. Binary Launch Map

How the surfaces are invoked — current messy state vs. clean target.

### Current Reality (Problematic)

| Binary | What it does | Problem |
|--------|-------------|---------|
| `aoc-mission-control` | Renders **all** views (Overview, Overseer, Mind, Fleet, Work, Diff, Health, Pulse, Fleet, Search) depending on `--mode`/`--view` runtime flags | One binary pretending to be multiple surfaces |
| `aoc-mission-control-toggle` | Toggle floating Mission Control | Unclear which view it toggles |
| `aoc-mission-control-tab` | Launch in dedicated tab | Good, but reuses same binary |
| `aoc-mind-toggle` | ??? | Shell script, unclear |
| `aoc-pulse-pane` | Historical/aspirational pulse-strip binary name | Not a current crate or required layout dependency |
| `aoc-control` | Control pane (Alt+C) | Correct, but overlaps with some config flows |

### Target

| Surface | Binary | Launch Method |
|---------|--------|---------------|
| Mission Control | `aoc-mission-control` | Dedicated Zellij tab / `aoc-mission-control-tab` |
| Mind | `aoc-mission-control --view mind` **→** eventually `aoc-mind-tui` | Floating pane from any project tab |
| Pulse | Legacy compatibility label only (`pulse-pane` degrades to Mission Control) | Not a required current layout surface |
| Control | `aoc-control` | Alt+C floating pane |
| Hub | `aoc-hub-rs` | Background process via `aoc-launch` |
| Agent wrap | `aoc-agent-wrap-rs` | via `aoc-agent-run` for each agent |

**Phase 1 (practical):** Keep `aoc-mission-control` as the binary host for all TUI views, but enforce *compile-time* or *hardcoded* view routing so the code structure maps 1:1 to product concepts. Mind rendering moves into a clean module boundary.

**Phase 2 (clean):** Extract `aoc-mind` into its own binary (`aoc-mind-tui`) that can be invoked as a floating pane. Mission control shrinks to fleet-only.

---

## 4. Data Flow

```
Agent Process
    │
    ▼
aoc-agent-wrap-rs  ─── publishes status, diff, heartbeat, mind_* events
    │                            to Pulse UDS socket
    ▼
Pulse UDS Socket  ──  /run/user/<uid>/aoc/<session>/pulse.sock
    │
    ├────────────┬──────────────┬──────────────┐
    ▼            ▼              ▼              ▼
  Legacy      Mission        Mind          aoc-insight
  pulse       Control         (project-      (Pi
  label       (fleet)         scoped)        extension)
    ▲            ▲              ▲              ▲
    │            │              │              │
    └────────────┴──────────────┴──────────────┘
              aoc-core::pulse_ipc
```

### Message Types (aoc_core::pulse_ipc)

- `hello` / `subscribe` — subscriber registration
- `agent_status` — worker lifecycle state
- `delta` — patch to previous state
- `snapshot` — full state dump on subscribe
- `heartbeat` — liveness ping
- `mind_injection` — Mind injection trigger event
- `mind_observer_feed` — Mind observer progress/status
- `command` / `command_result` — operator → agent commands
- `observer_snapshot` / `observer_timeline` — overseer data

### Mind-Specific Pipeline

```
Agent activity (token flow, file changes, git state)
    │
    ▼
aoc-agent-wrap-rs  ──  publishes mind_* events to Pulse UDS
    │
    ▼
aoc-mind (lib)
    │
    ├── T0: raw token/activity capture (ingestion boundary)
    ├── T1: pre-filter compaction (bounded by token budget)
    ├── T2: synthesis (canon revision, semantic enrichment)
    └── T3: structured output (handshake compilation, export)
         │
         ▼
.aoc/mind/project.sqlite  ──  durable artifact store
                                ↑
                                │ queries for rendering
    ┌───────────────────────────┘
    ▼
Mind TUI (floating pane per project tab)
```

---

## 5. Naming & CLI Contract

| Surface | CLI Binary | Config Env | Doc |
|---------|-----------|------------|-----|
| Mission Control | `aoc-mission-control` | `AOC_CONTROL_*` | `docs/mission-control.md` |
| Mind | `aoc-mission-control --view mind` (→ `aoc-mind-tui`) | `AOC_MIND_*` | `docs/mind-*.md` |
| Pulse | Legacy compatibility labels only (`pulse-pane` / `pulse_pane` / `pulse`) | legacy `AOC_PULSE_*` / compatibility vars | stale docs should not be treated as active product truth |
| Control | `aoc-control` | `AOC_CONTROL_PANE_*` | `docs/control-pane.md` |
| Hub | `aoc-hub-rs` | `AOC_HUB_*` | embedded |

### Environment Variables (by surface)

**Shared:**
- `AOC_SESSION_ID` — current Zellij session
- `AOC_PANE_ID` — current pane identifier
- `AOC_PROJECT_ROOT` — project root path
- `AOC_PULSE_VNEXT_ENABLED` — Pulse UDS gate

**Mission Control:**
- `AOC_CONTROL_NO_FLOAT` — suppress floating launch
- `AOC_CONTROL_FLOATING_ACTIVE` — launched from floating keybind
- `AOC_FLEET_PLANE_FILTER` — fleet plane filter
- `AOC_MIND_PROJECT_SCOPED` — project isolation flag

**Mind:**
- `AOC_MIND_DB` — project SQLite path (default: `.aoc/mind/project.sqlite`)
- `AOC_MIND_STATE_DIR` — Mind runtime state directory

---

## 6. Known Drift / Debt

| Issue | Description | Severity |
|-------|-------------|----------|
| **Monolithic main.rs** | 15,189 lines in aoc-mission-control/src/main.rs | 🔴 |
| **Mind not used as library** | aoc-mind crate exists but mission-control re-implements everything | 🔴 |
| **Overloaded binary** | One binary serves 3 distinct surfaces via runtime flags | 🟡 |
| **Stale Pulse sockets** | 40+ orphaned .sock files from past sessions | 🟡 |
| **Pi Mind standalone cutover** | Pi Mind now uses standalone service commands; remaining work is wrapper/runtime ownership cleanup, not extension transport recovery | 🟢 |
| **Mind floating pane** | Task #182 pending since Feb | 🟡 |
| **aoc-insight UX** | Task #110 pending | 🟡 |
| **Naming drift** | "Mission Control" used for both fleet and Mind views in docs/UI | 🟡 |

---

## 7. Refactoring Phases

### Phase 0: Stop the Bleeding
- [ ] Document this architecture (this file)
- [ ] Update AGENTS.md to reference canonical boundaries

### Phase 1: Extract Mind Rendering
- [ ] Move all `render_mind_*` functions from `main.rs` to `aoc-mind/src/render.rs`
- [ ] Move all Mind state queries (artifact drilldown, search, injection, observer rows) to use `aoc-mind` library APIs
- [ ] Add `aoc-mind` as dependency of `aoc-mission-control`
- [ ] Wire Mission Control Mind tab to call into `aoc-mind` render + query APIs
- [ ] Ensure tests pass

### Phase 2: Split Crate Files
- [ ] Break `main.rs` 15k lines into: `app.rs`, `pulse.rs`, `overview.rs`, `overseer.rs`, `fleet.rs`, `diff.rs`, `health.rs`, `work.rs`, `help.rs`
- [ ] Each module handles its own state, rendering, and key handling
- [ ] App struct uses typed sub-state structs per view

### Phase 3: Mind as Independent Binary
- [ ] Create `aoc-mind-tui` binary that provides the floating Mind pane
- [ ] Keep it also available as a library consumer for Mission Control's Mind tab
- [ ] Update launch scripts / keybinds to use the new binary

### Phase 4: Cleanup
- [ ] Fix stale Pulse socket cleanup (hub cleanup on exit)
- [x] Cut Pi Mind extension over to standalone service commands for ingest/status/context-pack/finalize/observer-run
- [ ] Update all docs to use cleaned-up naming
