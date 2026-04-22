<overview>
## Problem Statement
AOC Mind has the right layered architecture on paper (T0/T1/T2/T3, retrieval, handshake, provenance, context packs), but current semantic quality is uneven across the pipeline. In practice, T2 reflection is often preview-like instead of synthetic, T3 canon entries can be too blob-like to serve as durable steering memory, retrieval ranking is still largely lexical, handshake output can inherit noisy canon text, and memory classes are under-specified. This makes AOC Mind less useful as the single authoritative background thinking/memory layer than intended.

The core pain point is not missing infrastructure; it is insufficiently distilled, weakly typed, and weakly ranked memory semantics. Operators and maintainers need a tracked audit/refactor program that inspects each layer explicitly, records pass/fail findings with evidence, and translates those findings into implementation-ready refactors without introducing a second memory plane or duplicating semantics in TypeScript.

## Target Users
- **AOC maintainers**: need an evidence-backed map of where Mind architecture is sound vs where semantic quality is weak.
- **AOC operators**: need handshake/context outputs that are short, current, trustworthy, and actually useful for steering live work.
- **Future implementation agents**: need precise audit outputs, file references, and dependency-aware refactor targets rather than vague improvement ideas.

## Success Metrics
- 100% of the targeted Mind layers are audited with explicit scope, evidence, pass/fail, risks, and recommended changes.
- A linked PRD and umbrella task exist in Taskmaster, with one subtask per audit slice and a final synthesis subtask.
- Audit outputs identify concrete Rust-owned refactor targets for T2 synthesis, T3 canon shaping, retrieval ranking, handshake rendering, and memory typing.
- The resulting refactor map preserves AOC Mind as the one true background memory/thinking layer and keeps TypeScript limited to Pi-native adapter/UI responsibilities.
</overview>

<functional-decomposition>
## Capability Tree

### Capability: Architecture boundary audit
Verify that Rust remains the canonical owner of durable Mind semantics and that TypeScript stays a thin Pi-native adapter layer.

#### Feature: Boundary inspection
- **Description**: Inspect Rust and TypeScript Mind flows to confirm where canonical semantics live.
- **Inputs**: `crates/aoc-mind`, `crates/aoc-agent-wrap-rs`, `crates/aoc-storage`, `.pi/extensions/mind-*.ts`, `.pi/extensions/lib/mind.ts`
- **Outputs**: Boundary audit with pass/fail, leak points, and ownership rules.
- **Behavior**: Review ingest, retrieval, context-pack, and command wiring to identify any semantic drift into TS.

#### Feature: Ownership rule consolidation
- **Description**: Turn audit findings into enforceable guidance for future Mind work.
- **Inputs**: Boundary audit findings, current docs, current runtime behavior.
- **Outputs**: Explicit ownership rules for Rust vs TS responsibilities.
- **Behavior**: Capture allowed TS enrichment vs forbidden duplicated semantics.

### Capability: Ingest and replay audit
Verify that Mind ingest captures the right substrate for later synthesis and replay without over-weighting low-signal content.

#### Feature: Ingest fidelity inspection
- **Description**: Audit how Pi messages, tool results, focus metadata, and compaction checkpoints are captured.
- **Inputs**: `.pi/extensions/lib/mind.ts`, `.pi/extensions/mind-ingest.ts`, exported T1 artifacts, runtime contracts.
- **Outputs**: Findings on signal density, metadata quality, and replay fitness.
- **Behavior**: Compare raw ingest payload shaping against produced T1 exports and downstream artifact usefulness.

#### Feature: Replay and evidence path audit
- **Description**: Check whether replayable substrate and evidence trails are sufficient for later layers.
- **Inputs**: T0/T1 contracts, compaction checkpoint flow, artifact trace/evidence links.
- **Outputs**: Gaps and recommendations for provenance, file/task linking, and replay correctness.
- **Behavior**: Trace a representative path from message/tool events to T1/T2/T3 artifacts.

### Capability: Synthesis quality audit
Evaluate whether T2 is generating true synthesis instead of lightly reformatted previews.

#### Feature: T2 reflection inspection
- **Description**: Audit reflection generation logic and sampled artifacts.
- **Inputs**: `crates/aoc-mind/src/lib.rs`, recent `t2.md` exports, T1→T2 batching logic.
- **Outputs**: Assessment of synthesis depth, information loss, and steering value.
- **Behavior**: Compare intended reflection semantics against actual generated text and artifact structure.

#### Feature: Synthesis contract refactor targets
- **Description**: Identify the minimal contract changes needed to produce structured synthesis.
- **Inputs**: Reflection audit findings, current provenance/store contracts.
- **Outputs**: Refactor targets for themes, actions, uncertainties, and follow-up seeds.
- **Behavior**: Recommend structured outputs that remain Rust-owned and replayable.

### Capability: Canon lifecycle audit
Evaluate T3 canon promotion, revision behavior, staleness, supersession, and contradiction handling.

#### Feature: Canon promotion inspection
- **Description**: Audit how T1/T2 artifacts become canon entries.
- **Inputs**: `crates/aoc-agent-wrap-rs/src/main.rs`, `crates/aoc-storage/src/lib.rs`, exported `project_mind.md`.
- **Outputs**: Findings on summary quality, evidence thresholds, and durability.
- **Behavior**: Inspect promotion rules, evidence refs, freshness/confidence scoring, and sampled canon outputs.

#### Feature: Canon hygiene inspection
- **Description**: Audit whether revisions, supersession, staleness, and contradictions are handled meaningfully.
- **Inputs**: Canon revision schema and queries, sampled active/stale entries.
- **Outputs**: Refactor map for supersession semantics, stale handling, and contradiction review.
- **Behavior**: Compare lifecycle machinery against semantic quality of the entries it manages.

### Capability: Retrieval quality audit
Evaluate whether retrieval can scale from deterministic lexical recall toward high-value hybrid ranking.

#### Feature: Retrieval ranking inspection
- **Description**: Audit retrieval source collection, scoring, and fallback behavior.
- **Inputs**: `compile_insight_retrieval`, ranking helpers, retrieval tests, sampled artifacts.
- **Outputs**: Findings on ranking quality and missing signals.
- **Behavior**: Measure how current heuristics weight lexical overlap, source bias, freshness, and canon quality.

#### Feature: Ranking v2 requirements
- **Description**: Define the next-stage ranking signals without replacing canonical Rust retrieval.
- **Inputs**: Retrieval audit findings, package inspiration already evaluated, current schema.
- **Outputs**: Requirements for freshness/type/usefulness-aware ranking and optional semantic sidecar candidate generation.
- **Behavior**: Preserve deterministic lexical baseline while identifying hybrid improvements.

### Capability: Handshake and context-pack audit
Evaluate whether handshake/context packs turn Mind into useful bounded steering context.

#### Feature: Handshake inspection
- **Description**: Audit handshake ranking, rendering, focus logic, and sampled output quality.
- **Inputs**: handshake compiler code, `handshake.md`, related tests, task-state integration.
- **Outputs**: Findings on noise, focus quality, and steering usefulness.
- **Behavior**: Compare intended focus-first briefing behavior against actual output.

#### Feature: Context-pack composition inspection
- **Description**: Audit how memory, STM, handshake, canon, and session exports are composed for Pi-visible context.
- **Inputs**: `compile_mind_context_pack`, `.pi/extensions/mind-context.ts`, context-pack tests.
- **Outputs**: Findings on precedence, truncation, and source quality.
- **Behavior**: Check whether composition is stable, bounded, and high signal.

### Capability: Memory taxonomy audit
Evaluate whether Mind has the right typed memory classes for steering, ranking, and contradiction handling.

#### Feature: Taxonomy inspection
- **Description**: Audit current canon schema and rendered outputs for memory typing gaps.
- **Inputs**: canon revision models, exports, ranking/rendering consumers.
- **Outputs**: Proposed typed memory classes and weighting rules.
- **Behavior**: Determine how decisions, corrections, preferences, risks, workflow patterns, and hypotheses should differ.

#### Feature: Contradiction and staleness requirements
- **Description**: Define audit-backed requirements for stale or conflicting memory.
- **Inputs**: taxonomy audit findings, canon lifecycle audit findings.
- **Outputs**: Requirements for contradiction scans, class-aware freshness, and supersession discipline.
- **Behavior**: Specify how conflicting or obsolete canon should be surfaced and demoted.

### Capability: Refactor program synthesis
Turn all audit outputs into a dependency-aware Mind refactor plan aligned with existing Mind tasks.

#### Feature: Cross-audit synthesis
- **Description**: Combine all audit findings into a prioritized refactor map.
- **Inputs**: all audit subtasks, current tasks 142/145/146/147/168/177 and related docs.
- **Outputs**: Consolidated implementation backlog and dependency chain.
- **Behavior**: Group findings into contract/schema, ranking/rendering, and validation/hardening follow-ons.

#### Feature: Task alignment
- **Description**: Align new findings with current Mind task inventory to avoid duplicated planning.
- **Inputs**: existing Mind tasks and PRDs, consolidated findings.
- **Outputs**: Recommended updates, follow-up tasks, or task realignment notes.
- **Behavior**: Map audit outcomes onto the current Mind roadmap while preserving authoritative task ownership.
</functional-decomposition>

<structural-decomposition>
## Repository Structure
```text
project-root/
├── crates/
│   ├── aoc-mind/                 # T1/T2 observation and reflection runtime
│   ├── aoc-agent-wrap-rs/        # retrieval, T3 canon, handshake, context-pack compilation
│   ├── aoc-storage/              # canon revisions, evidence links, provenance, lifecycle state
│   └── aoc-cli/                  # user-facing insight commands
├── .pi/extensions/
│   ├── mind-ingest.ts            # Pi ingest hooks
│   ├── mind-context.ts           # Pi context-pack commands
│   ├── mind-ops.ts               # Pi operational commands
│   └── lib/mind.ts               # TS transport/payload shaping helpers
├── docs/
│   ├── mind-v2-architecture-cutover-checklist.md
│   ├── insight-compaction-ingest.md
│   ├── insight-t3-alignment.md
│   ├── implementation-status-checklist.md
│   └── mind-runtime-validation.md
└── .aoc/mind/
    ├── insight/<session>_<time>_<slice>/
    │   ├── t1.md
    │   ├── t2.md
    │   └── manifest.json
    └── t3/
        ├── project_mind.md
        └── handshake.md
```

## Module Definitions

### Module: `crates/aoc-mind`
- **Maps to capability**: Ingest and replay audit; Synthesis quality audit
- **Responsibility**: Produce T1/T2 artifacts from replayable event substrate with provenance-safe semantics.
- **File structure**:
  ```text
  aoc-mind/src/lib.rs
  ```
- **Exports**:
  - observation/reflection batching and synthesis helpers
  - deterministic provenance persistence
  - semantic runtime guardrail helpers

### Module: `crates/aoc-agent-wrap-rs`
- **Maps to capability**: Canon lifecycle audit; Retrieval quality audit; Handshake and context-pack audit
- **Responsibility**: Compile retrieval results, T3 canon, handshake exports, and context packs from project Mind state.
- **File structure**:
  ```text
  aoc-agent-wrap-rs/src/main.rs
  ```
- **Exports**:
  - `compile_insight_retrieval()` and ranking helpers
  - canon promotion/export helpers
  - handshake/context-pack compilation paths

### Module: `crates/aoc-storage`
- **Maps to capability**: Canon lifecycle audit; Memory taxonomy audit
- **Responsibility**: Persist revisioned canon, provenance, evidence refs, and lifecycle state used by Mind.
- **File structure**:
  ```text
  aoc-storage/src/lib.rs
  ```
- **Exports**:
  - canon revision models and queries
  - evidence/provenance accessors
  - lifecycle transitions for active/stale/superseded state

### Module: `.pi/extensions/mind-*` and `.pi/extensions/lib/mind.ts`
- **Maps to capability**: Architecture boundary audit; Ingest and replay audit; Handshake and context-pack audit
- **Responsibility**: Pi-native command/hooks surface and payload shaping only.
- **File structure**:
  ```text
  .pi/extensions/
  ├── mind-ingest.ts
  ├── mind-context.ts
  ├── mind-ops.ts
  └── lib/mind.ts
  ```
- **Exports**:
  - ingest hook registration
  - context-pack command registration
  - operational commands and transport helpers

### Module: `docs/` and `.aoc/mind/` artifacts
- **Maps to capability**: All audit capabilities
- **Responsibility**: Provide intended architecture and real emitted outputs for comparison.
- **File structure**:
  ```text
  docs/*.md
  .aoc/mind/insight/**
  .aoc/mind/t3/**
  ```
- **Exports**:
  - architecture intent
  - runtime validation expectations
  - live artifact evidence
</structural-decomposition>

<dependency-graph>
## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies - these are audited first.

- **architecture-boundary-audit**: Establish canonical Rust-vs-TS ownership rules.
- **artifact-evidence-baseline**: Sample representative T1/T2/T3/handshake/context-pack outputs for all later audits.

### Semantic Input Layer (Phase 1)
- **ingest-and-replay-audit**: Depends on [architecture-boundary-audit, artifact-evidence-baseline]
- **t2-synthesis-audit**: Depends on [ingest-and-replay-audit]

### Canon and Retrieval Layer (Phase 2)
- **canon-lifecycle-audit**: Depends on [t2-synthesis-audit, artifact-evidence-baseline]
- **retrieval-quality-audit**: Depends on [canon-lifecycle-audit]

### Steering Surface Layer (Phase 3)
- **handshake-and-context-pack-audit**: Depends on [canon-lifecycle-audit, retrieval-quality-audit]
- **memory-taxonomy-audit**: Depends on [canon-lifecycle-audit, handshake-and-context-pack-audit]

### Planning Consolidation Layer (Phase 4)
- **refactor-program-synthesis**: Depends on [architecture-boundary-audit, ingest-and-replay-audit, t2-synthesis-audit, canon-lifecycle-audit, retrieval-quality-audit, handshake-and-context-pack-audit, memory-taxonomy-audit]
</dependency-graph>

<implementation-roadmap>
## Development Phases

### Phase 0: Audit framing and baseline evidence
**Goal**: Establish the baseline contract, artifact sample set, and ownership constraints for the rest of the audit.

**Entry Criteria**: Current Mind docs, code, and representative exports are available.

**Tasks**:
- [ ] Audit Rust/TS boundary and source-of-truth ownership (depends on: [none])
  - Acceptance criteria: Written audit identifies canonical semantic owner per major Mind surface and any TS leak points.
  - Test strategy: Inspect command/ingest/context-pack flows across Rust and TS.
- [ ] Capture representative artifact evidence baseline (depends on: [none])
  - Acceptance criteria: Sampled T1, T2, T3, handshake, and context-pack outputs are referenced in later audits.
  - Test strategy: Compare docs and live/exported artifacts.

**Exit Criteria**: A stable baseline exists for interpreting later semantic-quality findings.

**Delivers**: Evidence-backed framing for the full audit program.

---

### Phase 1: Input and synthesis audit
**Goal**: Determine whether ingest and T2 provide the right substrate and synthesis quality for downstream memory.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Audit ingest fidelity and replay substrate (depends on: [Phase 0 baseline])
  - Acceptance criteria: Findings cover metadata quality, signal density, replay fitness, and evidence path sufficiency.
  - Test strategy: Trace representative raw events through T1 exports.
- [ ] Audit T2 synthesis quality and contract gaps (depends on: [ingest audit])
  - Acceptance criteria: Findings show whether T2 is synthetic vs preview-like and identify contract-level refactor targets.
  - Test strategy: Compare T1/T2 generation code against recent T2 outputs.

**Exit Criteria**: Input and synthesis shortcomings are explicit and tied to code paths.

**Delivers**: Clear substrate/synthesis refactor targets.

---

### Phase 2: Canon and retrieval audit
**Goal**: Determine whether durable project memory is being promoted, revised, and recalled in a steering-grade way.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Audit T3 canon promotion, revision, staleness, supersession, and contradiction handling (depends on: [T2 synthesis audit])
  - Acceptance criteria: Findings cover quality of promoted canon, evidence thresholds, lifecycle semantics, and contradiction gaps.
  - Test strategy: Inspect promotion code, storage schema, and sampled canon exports.
- [ ] Audit retrieval ranking and recall quality (depends on: [canon audit])
  - Acceptance criteria: Findings cover lexical ranking limits, missing signals, fallback behavior, and v2 ranking requirements.
  - Test strategy: Inspect ranking helpers and current retrieval tests.

**Exit Criteria**: Durable memory quality and recall weaknesses are explicit and prioritized.

**Delivers**: Canon/ranking refactor requirements.

---

### Phase 3: Steering-surface audit
**Goal**: Verify that bounded memory renderings are actually useful for live steering.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Audit handshake rendering and context-pack composition quality (depends on: [canon audit, retrieval audit])
  - Acceptance criteria: Findings cover focus quality, noise, precedence, truncation, and bounded usefulness.
  - Test strategy: Compare compiler logic/tests against emitted handshake/context outputs.
- [ ] Audit memory taxonomy / steering classes and staleness rules (depends on: [canon audit, handshake audit])
  - Acceptance criteria: Findings define required memory classes and class-aware ranking/freshness behavior.
  - Test strategy: Inspect schema and rendering consumers for type gaps.

**Exit Criteria**: Steering surface shortcomings are translated into explicit schema and rendering requirements.

**Delivers**: Typed-memory and handshake/context refactor requirements.

---

### Phase 4: Consolidated refactor plan
**Goal**: Turn all audit outputs into a dependency-aware implementation map aligned with the existing Mind roadmap.

**Entry Criteria**: Phases 0-3 complete.

**Tasks**:
- [ ] Consolidate audit findings into an implementation-ready refactor plan (depends on: [all previous audit tasks])
  - Acceptance criteria: Consolidated plan groups findings into contract/schema, synthesis/canon, retrieval/handshake, and validation tracks with suggested task alignment.
  - Test strategy: Cross-check resulting plan against current Mind tasks and docs to avoid duplicate or contradictory work.

**Exit Criteria**: AOC Mind has a tracked, evidence-backed refactor map suitable for follow-on implementation tasks.

**Delivers**: Prioritized follow-up plan and Taskmaster alignment notes.
</implementation-roadmap>

<test-strategy>
## Test Pyramid

```text
        /\
       /E2E\       ← 10% (live validator scripts, end-to-end runtime smoke checks)
      /------\\
     /Integration\ ← 35% (retrieval/handshake/canon path tests across crates)
    /------------\\
   /  Unit Tests  \ ← 55% (ranking helpers, synthesis helpers, schema/lifecycle helpers)
  /----------------\\
```

## Coverage Requirements
- Line coverage: maintain or improve existing targeted coverage on touched Mind areas.
- Branch coverage: require targeted branch coverage for ranking, fallback, stale/supersession, and composition paths touched by follow-up refactors.
- Function coverage: add or update tests for every changed ranking/rendering/lifecycle helper.
- Statement coverage: maintain existing crate-level standards; do not accept untested semantic rewrites.

## Critical Test Scenarios

### Ingest and replay
**Happy path**:
- Pi message/tool-result ingest captures conversation, timing, focus metadata, file/task cues, and compaction checkpoints.
- Expected: T1 exports remain replayable and provenance-linked.

**Edge cases**:
- Sparse sessions, tool-heavy sessions, and summary-heavy sessions.
- Expected: low-signal filler is not over-promoted relative to concrete evidence.

**Error cases**:
- Missing pulse/runtime availability or partially populated metadata.
- Expected: fail-open behavior without corrupting durable memory semantics.

**Integration points**:
- Pi hook payload shaping ↔ Rust ingest ↔ T1 export.
- Expected: consistent provenance and useful downstream evidence.

### T2 synthesis and canon promotion
**Happy path**:
- A meaningful sequence of T1 observations yields a concise, structured T2 synthesis and durable T3 canon.
- Expected: summaries are steering-grade, not replay blobs.

**Edge cases**:
- Single-observation sessions, repeated observations, stale revisions.
- Expected: no noisy over-promotion and correct freshness/stale handling.

**Error cases**:
- Empty evidence sets or unresolved trace refs.
- Expected: promotion rejects invalid canon or degrades visibly.

**Integration points**:
- T1/T2 outputs ↔ canon revision store ↔ exported project_mind.
- Expected: durable lineage and meaningful revision semantics.

### Retrieval and handshake
**Happy path**:
- Retrieval returns the most useful current canon/session evidence for a scoped query.
- Expected: ranked hits and handshake output emphasize focus, risk, and current steering rules.

**Edge cases**:
- Active-tag mismatch, stale canon, missing task state, or degraded context sources.
- Expected: fallback is explicit and low-value entries are demoted.

**Error cases**:
- Missing exports or unavailable context sources.
- Expected: bounded, fail-open behavior with truthful fallback status.

**Integration points**:
- canon/retrieval ↔ handshake/context-pack rendering ↔ Pi commands.
- Expected: stable, bounded, citation-backed context surfaces.

## Test Generation Guidelines
- Prefer small deterministic tests for ranking/rendering/lifecycle helpers in Rust.
- Keep TS tests focused on transport/command wiring and boundary enforcement, not semantic duplication.
- Add fixture-based artifact tests for representative T1/T2/T3/handshake/context-pack outputs.
- When changing ranking or rendering behavior, include before/after regression cases proving higher signal and truthful degradation.
</test-strategy>

<architecture>
## System Components
- **Pi ingest adapters (TS)**: register hooks/commands, extract lightweight context cues, and forward payloads to the Rust runtime.
- **Mind runtime (Rust)**: owns durable ingest handling, T1/T2 processing, T3 backlog processing, retrieval, handshake compilation, and context-pack composition.
- **Mind storage (Rust)**: persists artifacts, provenance, canon revisions, evidence refs, and lifecycle state.
- **Exported artifacts**: provide operator-visible evidence of current semantic quality and are used for audit sampling.

## Data Models
- **T0/T1/T2 artifacts**: replayable or distilled session memory with provenance and trace IDs.
- **CanonEntryRevision**: revisioned project memory with topic, confidence, freshness, evidence refs, and lifecycle state.
- **Handshake snapshot**: bounded export of priority canon and work context for live steering.
- **Context pack**: bounded composition of AOC memory, STM, handshake, canon, and session slices.

## Technology Stack
- Rust for canonical Mind runtime, storage, retrieval, ranking, provenance, and exports.
- TypeScript for Pi-native commands, hook registration, payload shaping, and UI notifications.
- Markdown and SQLite as durable/operator-visible artifact formats.

**Decision: Rust remains the canonical owner of Mind semantics**
- **Rationale**: durable memory, retrieval, provenance, ranking, revision, queue/lease, and handshake truth must live in one place.
- **Trade-offs**: TS convenience is reduced; some adapter logic must stay intentionally thin.
- **Alternatives considered**: moving more semantic logic into TS or adopting third-party memory packages directly; both would fracture the single-source-of-truth model.

**Decision: AOC Mind remains the only true background memory/thinking layer**
- **Rationale**: observations, reflections, alignment, retrieval, and handshake already cover the needed planes.
- **Trade-offs**: improvements must occur within the existing architecture instead of outsourcing semantics.
- **Alternatives considered**: parallel package-based memory systems; rejected as additional memory planes rather than useful inspiration.
</architecture>

<risks>
## Technical Risks
**Risk**: Audit findings overlap ambiguously with existing Mind tasks.
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: Include a final task-alignment pass that maps findings onto 142/145/146/147/168/177 and other active Mind work.
- **Fallback**: Create narrowly scoped follow-up tasks with explicit cross-references instead of rewriting existing tasks blindly.

**Risk**: Live artifact samples reflect temporary degraded states rather than representative semantics.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: Sample both code paths and multiple artifact classes, not a single export.
- **Fallback**: Prefer code-backed findings where artifacts are sparse or anomalous.

**Risk**: Refactor recommendations become too broad and lose implementation value.
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: Require file-level references, explicit pass/fail, and concrete contract/schema/ranking/rendering changes.
- **Fallback**: Split oversized findings into additional follow-up tasks.

## Dependency Risks
- Existing handshake/retrieval tasks may land while the audit is in progress, changing the current baseline.
- Some desired improvements (for example hybrid semantic recall) may require schema or infrastructure work beyond the initial refactor slice.

## Scope Risks
- The audit could drift into immediate implementation instead of first producing the evidence-backed refactor map.
- There is a risk of treating package inspiration as architecture replacement instead of implementation inspiration.
</risks>

<appendix>
## References
- `docs/mind-v2-architecture-cutover-checklist.md`
- `docs/insight-compaction-ingest.md`
- `docs/insight-t3-alignment.md`
- `docs/implementation-status-checklist.md`
- `docs/mind-runtime-validation.md`
- `crates/aoc-mind/src/lib.rs`
- `crates/aoc-agent-wrap-rs/src/main.rs`
- `crates/aoc-storage/src/lib.rs`
- `.pi/extensions/mind-ingest.ts`
- `.pi/extensions/mind-context.ts`
- `.pi/extensions/mind-ops.ts`
- `.pi/extensions/lib/mind.ts`

## Glossary
- **T0**: replayable/raw compact substrate.
- **T1**: bounded observation distillation.
- **T2**: bounded synthesis/reflection layer.
- **T3**: project-level canon/alignment layer.
- **Handshake**: bounded high-priority briefing derived from canon/task state and related context.
- **Context pack**: composed bounded context surface exposed to Pi-native flows.

## Open Questions
- Which typed canon classes should be first-class in schema vs derived at render/ranking time?
- Should usefulness scoring be stored durably or computed from access history on demand?
- What is the minimal semantic-sidecar design that improves retrieval without creating a second memory plane?
</appendix>
