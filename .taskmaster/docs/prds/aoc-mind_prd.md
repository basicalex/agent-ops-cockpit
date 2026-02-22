# AOC Mission Control + AOC Mind + AOC Insight

## PRD (Repository Planning Graph / RPG Method)

---

<overview>

## Problem Statement

AOC (Agent Ops Cockpit) is a Zellij-based agentic workspace used to run coding agents and manage large, fast-growing monorepos. Today, each dev tab includes a “mini mission control / overview pane” that constantly resyncs agent status. This creates:

- **Resource overhead** (duplicated rendering + frequent sync across tabs)
- **Fragility** (buggy streaming pane and inconsistent state)
- **Poor observability** (status is scattered; deep details require hunting)
- **Weak continuity** across long-running work (conversations and decisions are hard to keep coherent without manual handoffs)

We need a **single, first-tab “Mission Control” command center** plus a **local-first “AOC Mind” memory engine** that continuously distills agent conversations into usable project context—without requiring external API keys.

## Target Users

1) **Solo power developer (primary)**  
   - Runs 2+ dev tabs in parallel, each with an agent + task tracking  
   - Needs fast switching, reliable context, no UI noise  
   - Wants to avoid “multi-agent chaos” and instead maximize context quality and deliberate control

2) **Small team developer (secondary)**  
   - Shares the same repo conventions and AOC workflows  
   - Benefits from standardized layouts + memory artifacts (observations, decisions, segments)

3) **Agent orchestrator / maintainer (internal)**  
   - Maintains AOC tooling, adds plugins/skills, ensures Apache-2.0 compliance

## Success Metrics

- **Tab overhead reduction**: remove per-dev-tab mission control pane → CPU/RAM drop measurable (target: -20% UI overhead under load)
- **Observability**: from Mission Control, user can (a) list active agents, (b) open any agent’s convo/history, (c) jump to dev tab in ≤ 2 keystrokes
- **Mind usefulness**: after 2 hours of work, a new handoff can be generated from AOC Mind in ≤ 30 seconds with minimal manual editing
- **Reliability**: agent “online + lifecycle” status accuracy ≥ 99% (no stale ghosts > 10s)
- **Local-first**: core features work with zero external API keys; optional external model use is additive, never required
- **Scalability**: supports monorepo size ~1M LOC and 10+ concurrent agents without collapsing UX

</overview>

---

<functional-decomposition>

## Capability Tree

### Capability: Mission Control (MC) Tab Command Center

A dedicated Zellij tab (always first) that hosts the Mission Control TUI layout.

#### Feature: MC Layout Bootstrapping

- **Description**: Start AOC with MC as tab #1 and stable naming (“MC”) every time.
- **Inputs**: Zellij layout config, startup command (`aoc`)
- **Outputs**: Zellij session with MC tab first
- **Behavior**: Enforce deterministic tab ordering + naming

#### Feature: Active Agents List

- **Description**: Show all currently running agents with live status.
- **Inputs**: Agent hub snapshot stream (local), heartbeats, runtime signals
- **Outputs**: Table with agent rows + lifecycle + age + activity
- **Behavior**: Incremental updates (delta stream), not full refresh spam

#### Feature: Agent Conversations Index

- **Description**: Browse/search agent conversation history (from local OpenCode store and/or SQLite mirror).
- **Inputs**: Conversation store path, agent/session identifiers
- **Outputs**: Selectable list (by agent, by time, by task)
- **Behavior**: Lazy loading; preview snippets; open full transcript on demand

#### Feature: Agent Details Inspector

- **Description**: Show selected agent details (last message/tool, last error, current task, repo path, locks).
- **Inputs**: Selected agent ID, hub snapshot, optional SQLite fetch
- **Outputs**: Right-pane inspector view
- **Behavior**: Contextual drill-down; minimal blocking IO

#### Feature: Jump-to-Tab / Jump-to-Layout

- **Description**: From MC, jump directly to an agent’s dev tab/layout.
- **Inputs**: Zellij tab mapping metadata, agent-to-tab association
- **Outputs**: Focus switched to correct tab/pane
- **Behavior**: Uses Zellij actions/CLI; consistent even after tab churn

---

### Capability: AOC Dev Layout (Per-Project Coding Cockpit)

A dedicated coding tab separate from MC.

#### Feature: Dev Layout Structure

- **Description**: Standardize a dev tab layout:
  - Left: Yazi file manager
  - Center Top: Agent pane
  - Center Bottom: Taskmaster pane
  - Top Right: Project AOC Mind view (“Mind Insight”)
  - Bottom Right: General terminal
- **Inputs**: Zellij layout config, project path, agent launch params
- **Outputs**: Consistent workspace
- **Behavior**: Zero mission-control streaming inside dev layout

#### Feature: Project Mind Insight Panel (Top Right)

- **Description**: Lightweight read-only “project compass” panel (current project only).
- **Inputs**: AOC Mind distilled artifacts for the current project/segment
- **Outputs**: Snapshot view: current goals, recent decisions, active constraints, “what changed”
- **Behavior**: Pull-based updates (interval or event-trigger), not high-frequency streaming

---

### Capability: AOC Mind (Local Memory Engine)

A local-first memory layer that distills conversations into structured, durable “mind artifacts”.

#### Feature: Conversation Ingestion (OpenCode Store Reader)

- **Description**: Read agent conversations as they are written by OpenCode (source of truth).
- **Inputs**: OpenCode conversation storage location(s)
- **Outputs**: Normalized event stream (messages, tool calls, metadata)
- **Behavior**: Incremental ingestion with checkpoints; multi-agent safe

#### Feature: Observational Memory Distillation (OM-like)

- **Description**: Distill raw conversation into observations at thresholds (Mastra-inspired).
- **Inputs**: Raw conversation chunks, token counters, thresholds
- **Outputs**: Observation blocks (tier-1) and Reflections (tier-2)
- **Behavior**:
  - Tier-1 “Observer” pass at ~T1 tokens per conversation (configurable)
  - Tier-2 “Reflector” pass when observation block exceeds ~T2 tokens
  - Maintain append-only artifacts with traceability

#### Feature: Segmented Mind Routing (Monorepo Domains)

- **Description**: Route distilled insights into correct “mind segments” (frontend, dashboards, gamification, branding, etc.).
- **Inputs**: Distilled observations + segment definitions + routing rules
- **Outputs**: Segment-specific mind artifacts + global rollup
- **Behavior**: Auto-routing with confidence score; allow manual override/patch

#### Feature: Additive Decision Memory (AOC Mem)

- **Description**: Maintain non-rewritten “key decisions” log (append-only).
- **Inputs**: Explicit decision entries (human/agent authored)
- **Outputs**: Immutable decision ledger
- **Behavior**: Never rewrites; references can be superseded by later entries

#### Feature: Handoff Generator (STM / Context Refresh)

- **Description**: Generate handoff text for starting a new chat/context window.
- **Inputs**: Segment + active task + recent observations/decisions
- **Outputs**: Compact handoff payload
- **Behavior**: Deterministic format; ≤ 1 screen by default; expandable

---

### Capability: AOC Insight (Terminal Tool + Optional Agent Tool)

A local CLI that answers natural-language queries using Mind artifacts (+ optional vector index).

#### Feature: `aoc-insight "<query>"`

- **Description**: Return actionable context: “what to read next”, “key files”, “recent decisions”, “relevant snippets”.
- **Inputs**: Natural language query, project selector, optional segment selector
- **Outputs**: Ranked answer with citations to local artifacts and file paths
- **Behavior**: Two-stage: retrieve → synthesize (local model optional)

#### Feature: Output Modes

- **Description**: Provide multiple response formats:
  - `--brief` (default): 5–10 bullets
  - `--refs`: artifact/file references only
  - `--snips`: include small code/doc snippets (bounded)
- **Inputs**: Flags
- **Outputs**: Structured terminal output
- **Behavior**: Never dump huge walls; always bounded by size

---

### Capability: Optional Semantic Index (ZVec)

Optional vectorization for stable artifacts (observations/docs), not required.

#### Feature: Observation Embedding + Re-embedding

- **Description**: Embed tier-2 reflections and/or stable docs; re-embed patched items after major changes.
- **Inputs**: Reflections/docs, embedding model, metadata
- **Outputs**: Vector index entries with trace IDs
- **Behavior**: Incremental; supports delete/replace for patched artifacts

#### Feature: Retrieval for Insight + Agents

- **Description**: Provide semantic retrieval to AOC Insight and to tool-using agents.
- **Inputs**: Query embedding, topK
- **Outputs**: Ranked chunks
- **Behavior**: Local-first; fast; bounded output

---

### Capability: Hub + Coordination (Live State, Locks)

A small local hub that provides real-time “now” state and coordination primitives.

#### Feature: Agent State Hub

- **Description**: Maintain in-memory snapshots of active agents (heartbeats + lifecycle).
- **Inputs**: Heartbeats, runtime events, optional process info
- **Outputs**: Snapshot + delta stream for MC UI
- **Behavior**: Local IPC (Unix socket); no SQLite polling loops

#### Feature: File/Scope Locking (Anti-Toe-Stepping)

- **Description**: Optional lock layer so agents don’t edit the same files simultaneously.
- **Inputs**: Agent ID, requested file(s), lock TTL
- **Outputs**: Grant/deny + lock records
- **Behavior**: Advisory locks; integrate with agent toolchain hooks

</functional-decomposition>

---

<structural-decomposition>

## Repository Structure

Assume Rust workspace for AOC core (adjust names to match your repo conventions):

```
aoc/
├── crates/
│   ├── aoc-cli/                 # `aoc` entrypoint; starts MC then dev layouts
│   ├── aoc-hub/                 # live agent state hub (IPC, snapshots, locks)
│   ├── aoc-mission-control/     # TUI app for MC tab
│   ├── aoc-layouts/             # Zellij layout templates + generators
│   ├── aoc-mind/                # ingestion + OM distillation + segmentation
│   ├── aoc-insight/             # `aoc-insight` CLI (retrieve + synthesize)
│   ├── aoc-storage/             # SQLite schema + migrations + access layer
│   ├── aoc-opencode-adapter/    # reads OpenCode conversation store
│   ├── aoc-zvec/                # optional vector index + embed/retrieval
│   ├── aoc-zellij-bridge/       # helpers: tab mapping, jump actions
│   └── aoc-zellij-plugin/       # optional: last-used-tab toggle plugin (WASM)
├── configs/
│   ├── zellij/
│   │   ├── layouts/
│   │   │   ├── mc.kdl
│   │   │   └── dev.kdl
│   │   └── config.kdl
├── docs/
│   ├── prd_rpg.md
│   ├── mission-control.md
│   ├── mind.md
│   └── insight.md
└── tests/
```

## Module Definitions

### Module: `aoc-cli`

- **Maps to capability**: Mission Control boot + dev layout launch
- **Responsibility**: Start AOC session deterministically (MC first), spawn components
- **Exports**:
  - `run_aoc()` - start session, load layouts, wire env vars

### Module: `aoc-layouts`

- **Maps to capability**: MC + Dev layout structure
- **Responsibility**: Provide layout templates and interpolation (project path, pane cmds)
- **Exports**:
  - `render_mc_layout()`
  - `render_dev_layout(project_id)`

### Module: `aoc-hub`

- **Maps to capability**: Hub + coordination
- **Responsibility**: Live snapshots + delta broadcast + advisory locks
- **Exports**:
  - `subscribe_snapshots()`
  - `publish_heartbeat(agent_id, status)`
  - `lock_files(agent_id, paths)`

### Module: `aoc-mission-control`

- **Maps to capability**: MC TUI command center
- **Responsibility**: UI for agent list, convo index, inspector, jump actions
- **Exports**:
  - `run_tui()`

### Module: `aoc-opencode-adapter`

- **Maps to capability**: Conversation ingestion
- **Responsibility**: Tail/parse OpenCode conversation store events
- **Exports**:
  - `stream_conversation_events()`

### Module: `aoc-mind`

- **Maps to capability**: AOC Mind engine
- **Responsibility**: OM distillation, segmentation routing, handoffs, artifacts
- **Exports**:
  - `distill_conversation(convo_id)`
  - `route_observations(observations)`
  - `generate_handoff(project, segment)`

### Module: `aoc-storage`

- **Maps to capability**: Persistence
- **Responsibility**: SQLite schema, migrations, query layer
- **Exports**:
  - `insert_raw_event()`
  - `upsert_observations()`
  - `fetch_agent_history()`

### Module: `aoc-insight`

- **Maps to capability**: AOC Insight CLI
- **Responsibility**: Retrieve relevant artifacts + produce bounded answers
- **Exports**:
  - `run_insight(query, flags)`

### Module: `aoc-zvec` (Optional)

- **Maps to capability**: Vector store and retrieval
- **Responsibility**: embed/index/search stable artifacts
- **Exports**:
  - `index_reflections()`
  - `search(query)`

### Module: `aoc-zellij-bridge`

- **Maps to capability**: Jump-to-tab/layout, tab mapping
- **Responsibility**: Wrap Zellij actions/CLI conventions
- **Exports**:
  - `goto_tab(name_or_index)`
  - `focus_pane(pane_id)`

### Module: `aoc-zellij-plugin` (Optional)

- **Maps to capability**: last-used-tab toggle
- **Responsibility**: Track active tab events; jump back on keybind
- **Exports**: WASM plugin entrypoints

</structural-decomposition>

---

<dependency-graph>

## Dependency Chain

### Foundation Layer (Phase 0)

No dependencies.

- **aoc-storage**: SQLite schema + migrations + data access
- **aoc-zellij-bridge**: Zellij action wrappers + env conventions
- **aoc-layouts**: Layout templates + rendering utilities
- **aoc-opencode-adapter**: Conversation store reader + parser primitives

### Live State Layer (Phase 1)

- **aoc-hub**: Depends on [aoc-storage] (optional persistence of heartbeats) and uses [aoc-zellij-bridge] for mappings

### Mind Layer (Phase 2)

- **aoc-mind**: Depends on [aoc-storage, aoc-opencode-adapter]
  - Optional dependency: [aoc-zvec] if semantic retrieval enabled

### UI Layer (Phase 3)

- **aoc-mission-control**: Depends on [aoc-hub, aoc-storage, aoc-zellij-bridge]
  - Optional dependency: [aoc-mind] for showing mind snapshots in MC

### Tooling Layer (Phase 4)

- **aoc-insight**: Depends on [aoc-mind, aoc-storage]
  - Optional dependency: [aoc-zvec]

### Orchestration Layer (Phase 5)

- **aoc-cli**: Depends on [aoc-layouts, aoc-mission-control, aoc-zellij-bridge]
  - Also starts/ensures availability of [aoc-hub, aoc-mind]

### Optional Plugin Layer (Phase 6)

- **aoc-zellij-plugin**: Depends on [aoc-zellij-bridge] (conceptually; compiled separately)

</dependency-graph>

---

<implementation-roadmap>

## Development Phases

### Phase 0: Foundations

**Goal**: Stable storage + layout + adapter primitives

**Entry Criteria**: Repo builds; basic CI; Rust workspace compiles

**Tasks**:

- [ ] Implement `aoc-storage` SQLite schema + migrations
  - Acceptance criteria: can insert/fetch raw events and observation artifacts
  - Test strategy: unit tests for migrations + queries; temp DB per test
- [ ] Implement `aoc-opencode-adapter` minimal parser + event stream abstraction
  - Acceptance criteria: can read existing OpenCode logs and emit normalized events
  - Test strategy: golden fixtures for log formats
- [ ] Implement `aoc-layouts` with two layout templates: `mc.kdl`, `dev.kdl`
  - Acceptance criteria: layouts render with correct panes/commands/paths
  - Test strategy: snapshot tests on rendered KDL strings
- [ ] Implement `aoc-zellij-bridge` (goto tab by name/index, focus pane)
  - Acceptance criteria: commands generated correctly for Zellij
  - Test strategy: unit tests on command generation

**Exit Criteria**: Can boot layouts manually and read conversation logs offline

**Delivers**: Building blocks to create MC + Dev flows without live hub yet

---

### Phase 1: Live Hub (Now-State)

**Goal**: Real-time agent status without per-tab polling

**Entry Criteria**: Phase 0 complete

**Tasks**:

- [ ] Implement `aoc-hub` IPC (Unix socket) + snapshot model
  - Acceptance criteria: publish heartbeat; subscribe to snapshot stream
  - Test strategy: integration tests with in-process server + client
- [ ] Add optional advisory lock system (files/scopes)
  - Acceptance criteria: grant/deny locks; TTL expires; no deadlocks
  - Test strategy: concurrency tests (multi-client)

**Exit Criteria**: Hub can represent 10+ agents reliably with correct TTL behavior

**Delivers**: A single source of truth for live status and coordination

---

### Phase 2: AOC Mind (OM-like Distillation + Segmentation)

**Goal**: Local memory engine producing useful artifacts

**Entry Criteria**: Phase 0 complete (hub optional)

**Tasks**:

- [ ] Define Mind artifact formats:
  - `raw_events` (normalized)
  - `observations_tier1`
  - `reflections_tier2`
  - `aoc_mem_decisions` (append-only)
  - Acceptance criteria: versioned JSON or markdown schema in `docs/mind.md`
  - Test strategy: schema validation tests
- [ ] Implement token thresholding + distillation pipeline
  - Acceptance criteria: at configurable thresholds T1/T2, produce stable artifacts
  - Test strategy: deterministic “mock model” tests + golden output fixtures
- [ ] Implement segmentation router (auto + override)
  - Acceptance criteria: observations routed to correct segments with confidence; manual override patches routing
  - Test strategy: rule-based baseline tests + regression fixtures

**Exit Criteria**: Given a set of OpenCode logs, Mind artifacts are generated consistently and are readable in dev top-right pane

**Delivers**: Project memory that evolves without needing cloud keys

---

### Phase 3: Mission Control TUI (MC Tab)

**Goal**: The one place to monitor and control everything

**Entry Criteria**: Phase 1 complete (hub); Phase 0 bridge/layout complete

**Tasks**:

- [ ] Build MC TUI with 3-pane structure:

  1) Conversations index (left)
  2) Active agents list (center)
  3) Details inspector (right)

  - Acceptance criteria: keyboard navigation, selection, search, preview
  - Test strategy: unit tests for state reducer; minimal “smoke render” tests

- [ ] Add jump-to-tab/layout action

  - Acceptance criteria: selecting agent → jump to its dev tab reliably
  - Test strategy: mocked bridge + contract tests

- [ ] Add “open transcript” action

  - Acceptance criteria: open local transcript view quickly (pager or TUI modal)
  - Test strategy: fixture-driven

**Exit Criteria**: MC replaces old buggy per-tab mission panel completely

**Delivers**: Command center tab that starts first and stays lean

---

### Phase 4: AOC Dev Layout Integration (Top-Right Mind)

**Goal**: Dev tabs become focused; mind is a compass, not a status dashboard

**Entry Criteria**: Phase 2 artifacts exist; Phase 0 layouts exist

**Tasks**:

- [ ] Implement Mind Insight renderer pane (read-only)
  - Acceptance criteria: shows current project segment summary + “what changed”
  - Test strategy: fixture outputs; bounded render tests
- [ ] Remove old per-tab mission control pane (deprecate binary/panel)
  - Acceptance criteria: dev layout includes no live overview sync
  - Test strategy: config snapshot + runtime smoke

**Exit Criteria**: Two-tab workflow works: MC first → dev tab(s) after

**Delivers**: Less noise, better performance, clearer focus

---

### Phase 5: AOC Insight CLI

**Goal**: Developer can ask questions from terminal and get actionable answers

**Entry Criteria**: Phase 2 Mind artifacts exist

**Tasks**:

- [ ] Implement `aoc-insight "<query>"` with modes (`--brief`, `--refs`, `--snips`)
  - Acceptance criteria: returns bounded output with citations to artifacts/files
  - Test strategy: golden tests on known queries + fixtures
- [ ] Optional local model synthesis (if available) with strict timeouts
  - Acceptance criteria: graceful fallback to retrieval-only mode if slow
  - Test strategy: timeboxed integration tests

**Exit Criteria**: AOC Insight is useful standalone and can be used by agents as a tool

**Delivers**: Local-first “smart grep+” for humans and agents

---

### Phase 6 (Optional): ZVec Semantic Index + Plugin

**Goal**: Add semantic retrieval and last-tab toggle without forcing complexity

**Entry Criteria**: Phase 5 complete

**Tasks**:

- [ ] Implement `aoc-zvec` indexing for reflections/docs
  - Acceptance criteria: index/search returns stable chunks fast
  - Test strategy: index build + retrieval regression tests
- [ ] Add `aoc-zellij-plugin` last-used-tab toggle
  - Acceptance criteria: keybind toggles previous visited tab
  - Test strategy: plugin event handling tests + manual validation doc

**Exit Criteria**: Optional enhancements do not degrade baseline UX

**Delivers**: Semantic recall + smoother navigation for power users

</implementation-roadmap>

---

<test-strategy>

## Test Pyramid

```
        /\
       /E2E\        ← 5%  (very small; manual-friendly, slow)
      /------\
     /Integration\  ← 25% (hub↔tui, ingestion↔storage, insight↔mind)
    /------------\
   /  Unit Tests  \ ← 70% (reducers, parsers, schema, command gen)
  /----------------\
```

## Coverage Requirements

- Line coverage: 75% minimum (core crates), 60% for TUI rendering specifics
- Branch coverage: 65% minimum
- Function coverage: 75% minimum
- Statement coverage: 75% minimum

## Critical Test Scenarios

### aoc-opencode-adapter

**Happy path**:

- parse known OpenCode log fixtures
- Expected: normalized events in correct order

**Edge cases**:

- truncated logs, partial writes
- Expected: tolerant parsing + resume via checkpoint

**Error cases**:

- corrupted entry
- Expected: skip with warning + continue; no crash loop

### aoc-mind

**Happy path**:

- distill at T1, reflect at T2
- Expected: deterministic observation + reflection artifacts

**Edge cases**:

- multi-agent interleaved logs
- Expected: correct convo separation, no cross contamination

**Error cases**:

- model unavailable / slow
- Expected: retrieval-only mode; mark artifact “stale” with timestamp

### aoc-hub

**Happy path**:

- 10 agents heartbeat
- Expected: stable snapshots + delta updates

**Edge cases**:

- agent disappears
- Expected: offline after TTL; no ghost online

**Error cases**:

- client disconnect storms
- Expected: hub remains stable; no memory leak

### aoc-mission-control

**Integration points**:

- hub stream updates UI without blocking
- open transcript is lazy and bounded
- jump-to-tab works via bridge

### aoc-insight

**Happy path**:

- query returns ranked references
- Expected: bounded answer + citations

**Error cases**:

- missing artifacts
- Expected: instructive output (“mind not built yet; run …”)

## Test Generation Guidelines

- Favor fixture-driven tests for parsing and distillation outputs
- Ensure all terminal outputs are bounded (golden tests assert max lines)
- Model calls must be mockable and time-boxed in tests
- Concurrency tests for locks and hub must include race conditions

</test-strategy>

---

<architecture>

## System Components

1) **Zellij Session**
   - Tab #1: **MC** (Mission Control layout hosting the MC TUI)
   - Subsequent tabs: **Dev layouts** per project/task

2) **AOC Hub (local IPC)**
   - Maintains live state and optional file locks
   - Broadcasts snapshot/delta streams to MC TUI

3) **OpenCode Conversation Store (source of truth)**
   - Real-time written conversation logs per agent/session

4) **AOC Mind Engine**
   - Ingests OpenCode logs
   - Runs OM-like distillation (T1 → observations, T2 → reflections)
   - Routes artifacts into mind segments + global rollup
   - Generates handoffs

5) **AOC Insight CLI**
   - Retrieves artifacts (+ optional vectors)
   - Produces actionable answers in terminal

6) **Optional ZVec**
   - Semantic indexing for stable artifacts/docs
   - Supports patch + re-embed flows

## Data Models

### Identifiers

- `ProjectId`: stable identifier for a repo/workspace
- `SegmentId`: domain bucket (frontend, dashboards, gamification, branding, etc.)
- `AgentId`: unique per running agent instance
- `ConversationId`: per OpenCode conversation thread

### SQLite Tables (high-level)

- `raw_events(conversation_id, agent_id, ts, kind, payload_json)`
- `observations_t1(conversation_id, ts, importance, text, trace_ids[])`
- `reflections_t2(conversation_id, ts, text, trace_ids[])`
- `segment_routes(artifact_id, segment_id, confidence, overridden_by?)`
- `aoc_mem_decisions(ts, project_id, segment_id?, text, supersedes_id?)`
- `locks(agent_id, path, ttl, acquired_ts)` (optional)
- `agent_snapshots(agent_id, ts, status_json)` (optional)

## Technology Stack

- Language: Rust (core crates + TUI)
- UI: Ratatui (or your existing TUI framework)
- Storage: SQLite (WAL enabled)
- IPC: Unix domain sockets (hub)
- Zellij: layouts in KDL; CLI/actions for navigation
- Optional: WASM plugin for last-tab toggle
- License: Apache-2.0 across repo (ensure dependencies are compatible)

**Decision: One MC tab instead of per-tab overview**

- **Rationale**: reduces duplicated work; makes observability coherent; improves reliability
- **Trade-offs**: requires fast jump actions; MC must be excellent
- **Alternatives considered**: keep per-tab mini panel (rejected due to overhead/bugs)

**Decision: OM-like distillation (threshold-based)**

- **Rationale**: scalable memory without constant retrieval; stable artifacts; traceability
- **Trade-offs**: distillation quality depends on model; needs good prompts + deterministic formats
- **Alternatives considered**: pure vector store memory (rejected as primary; too “static” alone)

**Decision: Vector store optional**

- **Rationale**: semantic search is helpful, but not required for baseline success
- **Trade-offs**: indexing/re-embedding adds complexity
- **Alternatives considered**: always-on vectors (rejected for MVP)

</architecture>

---

<risks>

## Technical Risks

**Risk**: Local model (Liquid foundation model) is too slow on older hardware  

- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: strict time budgets; background scheduling; retrieval-only fallback; batch distillation
- **Fallback**: disable synthesis; keep OM pipeline deterministic with lightweight summarizers

**Risk**: OpenCode log formats change  

- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: adapter abstraction + fixtures; version detection; tolerant parsing
- **Fallback**: manual import tool; degrade gracefully to “history unavailable”

**Risk**: Segmentation auto-routing is noisy → misfiled knowledge  

- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: confidence threshold + “uncertain” bucket; manual override + patch flow; audit UI in MC
- **Fallback**: default to global mind only until segments stabilized

**Risk**: File locking creates friction  

- **Impact**: Medium
- **Likelihood**: Low/Medium
- **Mitigation**: advisory + TTL; allow override; prefer “warn” mode before “block” mode
- **Fallback**: disable locks; only show “likely conflict” warnings

## Dependency Risks

- Zellij capabilities may limit “pin tab” semantics
  - Mitigation: enforce MC as first tab via layout; stable naming; optional plugin for last-tab toggle

## Scope Risks

**Risk**: Trying to build MC + Mind + Insight + Vectors + Plugin all at once  

- **Impact**: High
- **Likelihood**: High
- **Mitigation**: ship in phases; keep ZVec/plugin optional; prioritize MC + Mind artifacts first
- **Fallback**: lock MVP to Phase 0–4 only

</risks>

---

<appendix>

## References

- Observational memory concept (Mastra-inspired OM pattern: raw → observations → reflections)
- Zellij layouts (KDL) and CLI actions for tab navigation

## Glossary

- **MC**: Mission Control tab (first tab)
- **Dev layout**: project coding cockpit tab
- **AOC Mind**: local memory engine generating artifacts
- **AOC Mem**: append-only decisions ledger
- **OM**: observational memory (threshold-based distillation)
- **ZVec**: optional vector index for semantic retrieval
- **AOC Insight**: terminal query tool (human + agent tool use)

## Open Questions

1) What exact OpenCode conversation store formats/paths must be supported (versions)?
2) Exact thresholds T1/T2 (tokens) and how token counting is implemented deterministically
3) Minimum viable segmentation set for a 1M LOC monorepo (initial domain taxonomy)
4) Lock granularity: file, directory, or “task scope”?
5) How MC maps agents to dev tabs when tabs are created/destroyed dynamically

</appendix>
