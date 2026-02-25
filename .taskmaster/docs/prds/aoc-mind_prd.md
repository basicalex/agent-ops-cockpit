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
- **Outputs**: Normalized raw event stream (messages, tool calls, metadata) plus deterministic T0-ready inputs
- **Behavior**: Incremental ingestion with checkpoints; multi-agent safe; tolerant of partial writes/corrupt records

#### Feature: T0 Transcript Compaction (Pre-Distillation)

- **Description**: Build a deterministic compact transcript lane before T1/T2 distillation to reduce context bloat.
- **Inputs**: Normalized raw events, compaction policy, allowlist policy, redaction policy
- **Outputs**: T0 compact transcript containing message text plus lightweight tool metadata and optional allowlisted snippets
- **Behavior**:
  - Keep `system`/`user`/`assistant` conversational content by default
  - Strip bulky tool outputs by default
  - Retain one-line tool metadata (`tool_name`, success/fail, latency, exit code when present, output size)
  - Allow policy-versioned tool snippet retention for explicitly allowlisted tools only
  - Preserve dual-lane provenance: raw remains authoritative history; T0 is deterministic derived view

#### Feature: Observational Memory Distillation (OM-like)

- **Description**: Distill raw conversation into observations at thresholds (Mastra-inspired).
- **Inputs**: T0 compact conversation chunks, token counters, thresholds, parser budget policy
- **Outputs**: Observation blocks (tier-1) and Reflections (tier-2)
- **Behavior**:
  - Tier-1 “Observer” pass at ~T1 tokens per conversation (configurable) with one-conversation-per-pass policy
  - If a conversation T0 payload fits parser budget (target ~28k, hard cap 32k), run single-pass T1 on that conversation only
  - If over budget, chunk within the same conversation only (never mix multiple conversations in one T1 pass)
  - Tier-2 “Reflector” pass when observation block exceeds ~T2 tokens
  - T2 may aggregate across multiple T1 observation blocks when they share the same active Taskmaster tag/workstream (project-level synthesis)
  - T2 cross-tag mixing is disallowed by default; route to separate reflections or `global` synthesis only when policy explicitly allows
  - Maintain append-only artifacts with traceability

#### Feature: Segmented Mind Routing (Monorepo Domains)

- **Description**: Route distilled insights into retrieval segments using Taskmaster context first, then heuristics and semantic fallback.
- **Inputs**: Distilled observations, `tm tag current --json`, task tags, task IDs, segment map, routing rules
- **Outputs**: Segment metadata (`primary`, `secondary`, confidence), global rollup, and routing provenance
- **Behavior**: Prioritize Taskmaster tag-to-segment mapping; keep `global`/`uncertain` fallback; allow manual override/patch

#### Feature: Task/Tag Attribution and Backfill

- **Description**: Attach each artifact to one or more Taskmaster tasks/tags, even when task references appear late in long conversations.
- **Inputs**: Conversation events, Taskmaster command/tool events, active tag snapshots, previous attribution state
- **Outputs**: Many-to-many artifact-task links with relation and confidence
- **Behavior**:
  - Linear ingestion carries forward attribution context per conversation/session
  - Supports multiple task relationships per artifact (`active`, `worked_on`, `mentioned`, `completed`)
  - Runs retroactive backfill when stronger evidence appears later (for example, task completion at end of chat)

#### Feature: Provider-Aware Distillation Runtime (Optional External Enhancers)

- **Description**: Distill artifacts deterministically by default, while allowing optional quality boosters (Zen inference, Roam retrieval, Ouros runtime) behind guarded adapters.
- **Inputs**: Distillation job payload, provider availability, timeout budgets, redaction rules, cache state
- **Outputs**: Distilled artifacts with provider provenance, latency, and confidence metadata
- **Behavior**: Local deterministic baseline first; optional provider chain is additive with strict timeout/fallback/cost controls

#### Feature: Additive Decision Memory (AOC Mem)

- **Description**: Maintain non-rewritten “key decisions” log (append-only).
- **Inputs**: Explicit decision entries (human/agent authored)
- **Outputs**: Immutable decision ledger
- **Behavior**: Never rewrites; references can be superseded by later entries

#### Feature: Handoff Generator (STM / Context Refresh)

- **Description**: Generate handoff text for starting a new chat/context window.
- **Inputs**: `aoc-mem`, `aoc-stm`, active tag/task context, segmented mind artifacts
- **Outputs**: Compact handoff payload
- **Behavior**: Deterministic precedence (`aoc-mem` → `aoc-stm` → `aoc-mind`); ≤ 1 screen by default; expandable

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

#### Feature: Structural Retrieval Enrichment (Optional)

- **Description**: Optionally enrich Insight output with structural graph evidence (for example from Roam) while preserving local-first behavior.
- **Inputs**: Query, local artifacts, optional structural provider adapter
- **Outputs**: Ranked answer with structural citations and confidence
- **Behavior**: Local artifacts remain primary source of truth; enrichment is additive and can be disabled globally

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
│   ├── aoc-task-attribution/    # task/tag attribution + backfill engine
│   ├── aoc-provider-zen/        # optional external synthesis adapter
│   ├── aoc-provider-roam/       # optional structural retrieval adapter
│   ├── aoc-provider-ouros/      # optional sandboxed runtime adapter
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
- **Responsibility**: Tail/parse OpenCode conversation store events and materialize deterministic T0 compact transcripts
- **Exports**:
  - `stream_conversation_events()`
  - `stream_t0_compact_events()`

### Module: `aoc-task-attribution`

- **Maps to capability**: Task/Tag Attribution and Backfill
- **Responsibility**: Resolve artifact-task relationships from Taskmaster signals + conversation state
- **Exports**:
  - `apply_task_signal(event)`
  - `link_artifact_to_tasks(artifact_id, context)`
  - `run_backfill(conversation_id, window)`

### Module: `aoc-mind`

- **Maps to capability**: AOC Mind engine
- **Responsibility**: T0-aware OM distillation, segmentation routing, handoffs, artifacts
- **Exports**:
  - `distill_conversation(convo_id)`
  - `compose_t0_transcript(convo_id, policy_version)`
  - `route_observations(observations)`
  - `generate_handoff(project, segment)`
  - `compose_context_pack(project, tag)`

### Module: `aoc-provider-zen` (Optional)

- **Maps to capability**: Provider-Aware Distillation Runtime
- **Responsibility**: Time-bounded external synthesis adapter with token/cost guardrails
- **Exports**:
  - `synthesize_with_zen(input, policy)`

### Module: `aoc-provider-roam` (Optional)

- **Maps to capability**: Structural Retrieval Enrichment
- **Responsibility**: Pull structural context (callers/callees/blast-radius) from optional Roam integration
- **Exports**:
  - `query_roam_context(symbol_or_query)`

### Module: `aoc-provider-ouros` (Optional)

- **Maps to capability**: Provider-Aware Distillation Runtime
- **Responsibility**: Run isolated/forkable background reasoning jobs with replay-safe snapshots
- **Exports**:
  - `run_ouros_job(job)`
  - `resume_ouros_snapshot(snapshot_id)`

### Module: `aoc-storage`

- **Maps to capability**: Persistence
- **Responsibility**: SQLite schema, migrations, query layer
- **Exports**:
  - `insert_raw_event()`
  - `upsert_t0_compact_event()`
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
- **aoc-opencode-adapter**: Conversation store reader + parser primitives
- **Mind routing policy config**: tag→segment map + confidence thresholds + output budgets
- **Mind T0 compaction policy config**: role keep/drop rules, tool metadata retention, allowlisted snippet controls, parser budgets

### Attribution Layer (Phase 1)

- **aoc-task-attribution**: Depends on [aoc-storage, aoc-opencode-adapter] and Taskmaster command signals (`tm tag current --json`, task lifecycle commands)

### Mind Layer (Phase 2)

- **aoc-mind**: Depends on [aoc-storage, aoc-opencode-adapter, aoc-task-attribution]
  - Optional dependencies: [aoc-provider-zen, aoc-provider-roam, aoc-provider-ouros, aoc-zvec]

### Output Layer (Phase 3)

- **aoc-insight**: Depends on [aoc-mind, aoc-storage]
- **Mind Insight renderer**: Depends on [aoc-mind] for bounded segment snapshots

### Optional Provider Layer (Phase 4)

- **aoc-provider-zen**: Depends on [aoc-mind provider interface] + external inference credentials/policies
- **aoc-provider-roam**: Depends on [aoc-mind provider interface] + optional Roam CLI availability
- **aoc-provider-ouros**: Depends on [aoc-mind provider interface] + optional Ouros runtime availability
- **aoc-zvec**: Optional semantic index for recall augmentation

### Orchestration Layer (Phase 5)

- **aoc-cli / aoc-init integration**: Depends on [aoc-mind, aoc-insight] and enforces policy defaults (Taskmaster authority + provider guardrails)

### Optional Plugin Layer (Phase 6)

- **aoc-zellij-plugin**: Depends on [aoc-zellij-bridge] (conceptually; compiled separately)

</dependency-graph>

---

<implementation-roadmap>

## Development Phases

### Phase -1: Baseline (Completed)

**Goal**: Establish stable runtime foundation before Mind-specific implementation.

**Status**: Completed in prior workstreams (`mission-control`, `safety`, `rtk`).

**Includes**:
- Mission Control + hub baseline
- Agent wrapper hardening + telemetry redaction
- RTK routing integration and safety controls

**Delivers**: Stable platform to build Mind v1 without reopening solved runtime issues.

---

### Phase 0: Mind Contracts and Persistence Foundations

**Goal**: Define deterministic artifact contracts and persistent schema for Mind v1.

**Entry Criteria**: Repo builds; CI/smoke baseline is green.

**Tasks**:

- [ ] Define Mind artifact schemas and routing contracts
  - Acceptance criteria: versioned schemas for `raw_events`, `compact_events_t0`, `observations_t1`, `reflections_t2`, task links, and segment routing metadata
  - Test strategy: schema validation tests + fixture compatibility tests
- [ ] Define T0 compaction contract and policy model
  - Acceptance criteria: deterministic role/message retention rules, default tool-output stripping, mandatory tool metadata retention, and policy-versioned allowlist snippet controls
  - Test strategy: fixture tests proving stable compaction output for fixed policy and deterministic serialization hashes
- [ ] Implement `aoc-storage` migrations for Mind v1 tables
  - Acceptance criteria: create/read/write for raw events, T0 compact events, artifacts, task links, segment routes, and context state
  - Test strategy: migration + query tests using temp DB per test
- [ ] Define provider interface contracts (local + optional external)
  - Acceptance criteria: one adapter interface with deterministic local baseline and optional provider fallbacks
  - Test strategy: trait/contract tests with mock adapters

**Exit Criteria**: Storage and schema contracts are stable and testable.

**Delivers**: Deterministic data layer for all Mind workflows.

---

### Phase 1: Ingestion and Task Signal Capture

**Goal**: Capture normalized conversation events plus reliable Taskmaster context signals.

**Entry Criteria**: Phase 0 complete.

**Tasks**:

- [ ] Implement `aoc-opencode-adapter` incremental ingestion with checkpoints
  - Acceptance criteria: can parse known log formats, resume after restart, and tolerate truncation/corruption
  - Test strategy: golden fixtures + partial-write recovery tests
- [ ] Implement deterministic T0 compaction pass in ingestion pipeline
  - Acceptance criteria: keep conversational roles (`system`/`user`/`assistant`), strip bulky tool outputs by default, retain one-line tool metadata, and apply optional allowlisted snippets by policy version
  - Test strategy: mixed-event fixtures validating keep/drop policy, metadata retention, and deterministic outputs across reruns
- [ ] Implement Taskmaster signal adapter using command/event capture
  - Acceptance criteria: captures active tag (`tm tag current --json`) and task lifecycle signals for attribution context
  - Test strategy: fixture-driven signal parsing tests and command-simulation integration tests
- [ ] Persist conversation context state snapshots
  - Acceptance criteria: state carries forward across chunks/sessions and survives process restart
  - Test strategy: restart resilience integration tests

**Exit Criteria**: Event stream and task/tag context can be replayed deterministically.

**Delivers**: High-quality attribution inputs.

---

### Phase 2: Attribution, Backfill, and Segmentation

**Goal**: Build Taskmaster-first routing and many-to-many artifact-task linking.

**Entry Criteria**: Phase 1 complete.

**Tasks**:

- [ ] Implement `aoc-task-attribution` linking engine
  - Acceptance criteria: artifacts can link to multiple tasks with relation + confidence metadata
  - Test strategy: multi-task fixture tests and confidence-scoring assertions
- [ ] Implement retroactive backfill pass
  - Acceptance criteria: late task evidence updates earlier artifact links without dropping provenance
  - Test strategy: long-conversation fixtures where task IDs appear late
- [ ] Implement Taskmaster-first segment router
  - Acceptance criteria: tag-to-segment mapping is primary, with path/semantic fallback and `global`/`uncertain` buckets
  - Test strategy: rule-based routing tests + override patch regression tests

**Exit Criteria**: Artifacts are attributed and segmented with auditable provenance.

**Delivers**: Retrieval-grade metadata for context filtering.

---

### Phase 3: Distillation Runtime and Optional Provider Adapters

**Goal**: Produce stable T1/T2 artifacts with optional quality enhancers under strict guardrails.

**Entry Criteria**: Phase 2 complete.

**Tasks**:

- [ ] Implement deterministic local T1/T2 distillation pipeline
  - Acceptance criteria: stable output for fixed fixtures and config; T1 consumes T0 compact transcript lane; no cross-conversation mixing in a single T1 pass
  - Test strategy: deterministic golden tests + model-mock integration tests + parser-budget fixtures (single-pass <=32k, deterministic intra-conversation chunking when over cap)
- [ ] Add optional Zen synthesis adapter (`aoc-provider-zen`)
  - Acceptance criteria: strict timeout, retry, redaction, budget caps, and fallback behavior; Zen runs as optional background enhancer for T2 synthesis first
  - Test strategy: adapter integration tests with fake provider + timeout failure tests + non-blocking/background execution checks
- [ ] Add optional structural enrichment adapter (`aoc-provider-roam`)
  - Acceptance criteria: enrichment is additive and can be disabled without affecting baseline outputs
  - Test strategy: adapter availability/no-availability contract tests
- [ ] Add optional isolated runtime adapter (`aoc-provider-ouros`)
  - Acceptance criteria: background jobs can fork/resume safely without blocking interactive workflow
  - Test strategy: snapshot/resume integration tests with bounded runtime limits

**Exit Criteria**: Mind distillation is production-usable with safe optional enhancers.

**Delivers**: High-quality artifacts with deterministic fallback path.

---

### Phase 4: Handoff and Context Pack Integration

**Goal**: Generate compact, deterministic context payloads for agents and handoffs.

**Entry Criteria**: Phase 3 complete.

**Tasks**:

- [ ] Implement handoff/context pack composer in `aoc-mind`
  - Acceptance criteria: precedence `aoc-mem` → `aoc-stm` → `aoc-mind`; bounded output; stable formatting
  - Test strategy: fixture tests for ordering, line limits, and deterministic rendering
- [ ] Integrate active tag/task filtering in pack generation
  - Acceptance criteria: context is segment/task aware and includes cross-segment watchlist when relevant
  - Test strategy: integration tests with synthetic task/tag timelines
- [ ] Wire Mind Insight read model for dev pane
  - Acceptance criteria: pane displays bounded snapshot and “what changed” deltas without streaming spam
  - Test strategy: render fixture tests + runtime smoke

**Exit Criteria**: New sessions start with concise, high-signal context.

**Delivers**: Optimized contextualization engine for daily agent workflows.

---

### Phase 5: AOC Insight and Operationalization

**Goal**: Expose Mind retrieval to terminal and harden operational checks.

**Entry Criteria**: Phase 4 complete.

**Tasks**:

- [ ] Implement `aoc-insight "<query>"` modes (`--brief`, `--refs`, `--snips`)
  - Acceptance criteria: bounded output with citations to artifacts/files/tasks
  - Test strategy: golden query tests + missing-artifact fallback tests
- [ ] Add smoke/ops checks for Mind data health
  - Acceptance criteria: checks catch stale checkpoints, schema drift, and attribution gaps
  - Test strategy: integration smoke tests + failure-mode fixtures
- [ ] Publish runbook for backfill/routing/provider policy operations
  - Acceptance criteria: maintainers can diagnose and recover from common failures quickly
  - Test strategy: dry-run validation from docs

**Exit Criteria**: Mind is queryable, operable, and safe under real workflows.

**Delivers**: Practical retrieval and maintenance loop.

---

### Phase 6 (Optional): Semantic Index Layer

**Goal**: Add semantic recall acceleration without changing baseline behavior.

**Entry Criteria**: Phase 5 complete.

**Tasks**:

- [ ] Implement `aoc-zvec` indexing for stable reflections/docs
  - Acceptance criteria: index/search is fast and deterministic with patch/re-embed support
  - Test strategy: index build + retrieval regression tests

**Exit Criteria**: Optional semantic indexing improves recall without degrading reliability.

**Delivers**: Scalable semantic retrieval for large repositories.

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
- Expected: normalized raw events in correct order and deterministic T0 compact outputs

**Edge cases**:

- truncated logs, partial writes
- Expected: tolerant parsing + resume via checkpoint

**Error cases**:

- corrupted entry
- Expected: skip with warning + continue; no crash loop

- oversized tool outputs
- Expected: outputs are stripped from T0 by default while metadata is retained

### aoc-task-attribution

**Happy path**:

- single-task flow with clear task lifecycle events
- Expected: artifacts link to correct task with high confidence and correct relation type

**Edge cases**:

- multiple tasks active/mentioned in same window
- Expected: many-to-many links preserved with ranked confidence and provenance

**Error cases**:

- task evidence appears late in long chat
- Expected: backfill updates prior links within valid window; no data loss

### aoc-mind

**Happy path**:

- distill at T1, reflect at T2
- Expected: deterministic observation + reflection artifacts from T0 compact transcript

**Edge cases**:

- multi-agent interleaved logs
- Expected: correct convo separation, no cross contamination

- one conversation fits parser budget
- Expected: single-pass T1 for that conversation only (no multi-conversation batching)

- one conversation exceeds parser budget
- Expected: deterministic chunking within that conversation only

- T2 reflection across multiple conversations in same active tag
- Expected: deterministic cross-conversation synthesis is allowed only from T1 observations sharing the same tag/workstream; provenance remains intact

- segment routing with Taskmaster-first mapping
- Expected: primary segment follows active tag map; fallback to `global`/`uncertain` when confidence is low

**Error cases**:

- model unavailable / slow
- Expected: retrieval-only mode; mark artifact “stale” with timestamp

- optional provider adapter unavailable (Roam/Ouros/Zen)
- Expected: baseline local path continues; output remains bounded and deterministic

### handoff/context pack

**Happy path**:

- compose pack from `aoc-mem`, `aoc-stm`, `aoc-mind`
- Expected: strict precedence order, bounded output, stable formatting

**Edge cases**:

- missing one source layer (for example, no recent STM archive)
- Expected: graceful degradation with clear status markers

**Error cases**:

- oversized candidate context from one segment
- Expected: truncation policy keeps total output within budget

### provider adapters

**Integration points**:

- Zen adapter timeout/retry/fallback behavior
- Roam adapter enrichment on/off behavior
- Ouros adapter snapshot/resume behavior

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
- Attribution tests must include long-conversation backfill and multi-task overlap
- Provider tests must enforce fail-open deterministic fallback to local baseline
- T0 tests must verify default tool-output stripping, mandatory metadata retention, and policy-versioned allowlist behavior
- Distillation tests must assert one-conversation-per-pass behavior and prohibit cross-conversation T1 mixing

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
     - Builds deterministic T0 compact transcripts (raw + compact dual-lane)
     - Runs OM-like distillation (T1 → observations, T2 → reflections)
     - Routes artifacts into mind segments + global rollup
     - Generates handoffs

5) **Taskmaster Signal Adapter**
   - Captures active tag and task lifecycle signals (for example via `tm tag current --json`)
   - Feeds attribution and backfill state used by Mind routing

6) **AOC Insight CLI**
   - Retrieves artifacts (+ optional vectors)
   - Produces actionable answers in terminal

7) **Optional Provider Adapters**
   - Zen inference (quality booster for T2 synth)
   - Roam structural enrichment (graph evidence)
   - Ouros background runtime (isolated fork/resume jobs)

8) **Optional ZVec**
   - Semantic indexing for stable artifacts/docs
   - Supports patch + re-embed flows

## Data Models

### Identifiers

- `ProjectId`: stable identifier for a repo/workspace
- `SegmentId`: domain bucket (frontend, dashboards, gamification, branding, etc.)
- `AgentId`: unique per running agent instance
- `ConversationId`: per OpenCode conversation thread
- `ArtifactId`: unique identifier for each observation/reflection artifact
- `TaskId`: Taskmaster task ID
- `TagId`: Taskmaster tag name captured at event time

### SQLite Tables (high-level)

- `raw_events(conversation_id, agent_id, ts, kind, payload_json)`
- `compact_events_t0(conversation_id, ts, role, text, source_event_ids_json, tool_meta_json, policy_version, compact_hash)`
- `observations_t1(artifact_id, conversation_id, ts, importance, text, trace_ids[])`
- `reflections_t2(artifact_id, conversation_id, ts, text, trace_ids[])`
- `artifact_task_links(artifact_id, task_id, relation, confidence, source, start_ts, end_ts)`
- `conversation_context_state(conversation_id, ts, active_tag, active_tasks_json, signal_source)`
- `segment_routes(artifact_id, segment_id, confidence, routed_by, reason, overridden_by?)`
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

**Decision: T0 compact transcript lane before T1/T2**

- **Rationale**: removes tool-output bloat early, preserves conversational signal, and keeps parser costs predictable
- **Trade-offs**: some deep debugging detail is omitted from T1/T2 input by default; requires policy governance
- **Alternatives considered**: distill directly from full raw events (rejected due to context inefficiency/noise)

**Decision: Taskmaster-first segmentation and attribution**

- **Rationale**: Task tags/tasks are already canonical workflow metadata and provide high-signal routing context
- **Trade-offs**: requires signal capture + retroactive backfill for late task evidence
- **Alternatives considered**: semantic-only routing (rejected due to instability and cost)

**Decision: Many-to-many artifact-task relationships**

- **Rationale**: long/multi-task conversations need multiple valid links per artifact
- **Trade-offs**: more complex ranking and storage queries
- **Alternatives considered**: single `task_id` per artifact (rejected due to information loss)

**Decision: Optional provider chain (Zen/Roam/Ouros) behind deterministic baseline**

- **Rationale**: enables quality improvements without making external systems mandatory
- **Trade-offs**: adapter maintenance + policy complexity
- **Alternatives considered**: mandatory external provider stack (rejected for reliability/control)

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

**Risk**: Task attribution drift in long multi-task chats  

- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: Taskmaster signal capture + retroactive backfill + provenance/confidence scoring
- **Fallback**: downgrade uncertain links and rely on active-tag filtering + manual patch

**Risk**: External provider instability or cost spikes (Zen/Roam/Ouros)  

- **Impact**: Medium/High
- **Likelihood**: Medium
- **Mitigation**: strict timeout/budget policies, cache by input hash, deterministic local fallback
- **Fallback**: disable optional providers and run local baseline only

**Risk**: T0 compaction strips useful tool detail needed for attribution or diagnostics  

- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: always retain tool metadata, support policy-versioned allowlist snippets, and preserve raw lane for provenance queries
- **Fallback**: expand allowlist or temporarily route specific workflows to raw-assisted replay mode

**Risk**: File locking creates friction  

- **Impact**: Medium
- **Likelihood**: Low/Medium
- **Mitigation**: advisory + TTL; allow override; prefer “warn” mode before “block” mode
- **Fallback**: disable locks; only show “likely conflict” warnings

## Dependency Risks

- Zellij capabilities may limit “pin tab” semantics
  - Mitigation: enforce MC as first tab via layout; stable naming; optional plugin for last-tab toggle

- Optional provider interfaces may change upstream
  - Mitigation: adapter isolation, contract tests, and version pinning for optional integrations

## Scope Risks

**Risk**: Trying to build MC + Mind + Insight + Vectors + Plugin all at once  

- **Impact**: High
- **Likelihood**: High
- **Mitigation**: ship in phases; keep ZVec/plugin optional; prioritize MC + Mind artifacts first
- **Fallback**: lock MVP to Phase 0–4 only

**Risk**: Over-optimizing for providers before deterministic baseline quality is proven

- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: enforce baseline-first acceptance gates before enabling optional providers
- **Fallback**: defer provider adapters to post-MVP phase

</risks>

---

<appendix>

## References

- Observational memory concept (Mastra-inspired OM pattern: raw → observations → reflections)
- Zellij layouts (KDL) and CLI actions for tab navigation
- Roam Code (optional structural retrieval inspiration): `https://github.com/Cranot/roam-code`
- just-bash (sandbox execution policy patterns): `https://github.com/vercel-labs/just-bash`
- Ouros (optional stateful sandbox runtime inspiration): `https://github.com/parcadei/ouros`

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
2) Exact thresholds T1/T2 (tokens), T1 parser budget targets (for example 28k target / 32k hard cap), and deterministic token counting method
3) Final initial tag→segment mapping for production (beyond current core tags)
4) Provider rollout policy: which optional adapter is enabled first in default install
5) Cost budget defaults for Zen-enhanced synthesis paths
6) Default T0 tool allowlist/snippet policy (which tools, max snippet size, and redaction rules)

</appendix>
