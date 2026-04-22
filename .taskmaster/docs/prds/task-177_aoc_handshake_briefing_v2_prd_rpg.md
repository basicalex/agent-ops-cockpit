# AOC Focus-First Handshake Briefing PRD (RPG)

## Zellij 0.44 Update Changes (Exact Delta)

This PRD remains primarily about handshake content and focus semantics, but Zellij 0.44 adds one concrete implementation opportunity that should not get lost:
- keep the current launch-time KDL template model as the baseline
- add narrow runtime mode switching with `zellij action override-layout` for focus mode, inspection mode, and compact-bar/status-bar swaps
- treat runtime layout overrides as a presentation/mode aid for the briefing surface, not as a replacement for handshake selection logic

Source alignment note: see `docs/research/zellij-0.44-aoc-alignment.md`.

## Problem Statement
AOC already injects a startup handshake so agents enter a managed environment with immediate project orientation. The current handshake succeeds at proving environment scope and exposing core artifacts, but it still over-indexes on raw inventory and under-indexes on actionable prioritization.

Current gaps:
- The full handshake shows large static sections such as repository tree snapshots, README headings, and done task lists that consume attention without helping the agent choose the next action.
- `Current Task Tag` is presented as if authoritative even though focus may be tab-local, stale for a newly spawned tab, or absent entirely in multi-agent workflows.
- Active workstreams are displayed as raw tag counts, which say little about urgency, remaining work, health, or whether the stream is actually active.
- Recent memory is useful but arrives as a flat log; agents must infer themes, unresolveds, and planning-vs-shipped state themselves.
- The task section does not privilege open PRD-linked tasks, active parent tasks, or blocked/high-value work, so agents can spend attention on historical completions instead of spec-bearing work.
- STM guidance currently occupies more handshake weight than its role justifies; STM is a handoff mechanism rather than the primary orientation plane.
- The handshake implementation is split across static context emission, wrapper rules, and Mind-generated handshake artifacts, but the product contract for how these layers combine has not been redefined around focus-first briefing semantics.

We need a new handshake contract that keeps the bounded, deterministic properties introduced by Mind task 139 while shifting the visible output toward current focus, scope-aware prioritization, Mind T2/T3 synthesis, unresolved architectural gaps, and PRD-led open work. The result should help any agent answer: where am I, what matters right now, what remains unresolved, and which spec-bearing tasks should guide my next move.

## Target Users
- Primary-session agents launched inside AOC who need fast orientation without reading broad project inventory first.
- Detached or specialized AOC/Pi agents that need a compact but trustworthy project briefing before deeper analysis.
- Maintainers designing project context and Mind synthesis surfaces who need a consistent handshake contract across repos.
- Operators working in multi-tab, multi-agent setups where tab-local focus may diverge from project-wide active work.

## Success Metrics
- Full handshake output privileges open, active, and unresolved work over done/historical inventory in at least the first visible screenful.
- When a tab-local tag is missing or stale, the handshake clearly distinguishes tab-local focus from project-global active work instead of overclaiming a single current tag.
- Workstream summaries report status-aware progress (for example in-progress/pending/recently-completed) rather than raw tag counts.
- Handshake content uses Mind-derived T2/T3 synthesis where available while preserving traceable fallback behavior to canonical memory/tasks/context sources.
- Open PRD-linked tasks and in-progress parent tasks are surfaced ahead of low-value task noise.
- STM is reduced to compact handoff guidance unless a resume/handoff context explicitly elevates it.
- Compact and full handshake modes remain deterministic, bounded, and backward-compatible with existing startup flow controls.

---

## Architectural Framing
This PRD treats the handshake as a **briefing surface**, not a raw artifact dump.

The handshake should combine multiple context planes with explicit roles:
- **Context / repository snapshot**: stable environment orientation and major entry points.
- **Taskmaster state**: open spec-bearing work, active parent tasks, and progress-aware workstream health.
- **Memory**: durable architectural facts and decision log.
- **Mind T2/T3**: synthesized presentation layer for recent developments, unresolveds, and cross-session relevance.
- **STM**: optional continuity aid for handoff/resume, not the default orientation core.

The redesign should preserve the bounded adaptive injection machinery from task 139 while changing the content-selection and rendering policy so the handshake becomes a focus-first working brief. It must also acknowledge scope ambiguity explicitly: tab-local focus, project-global focus, and inferred focus are different signals and should not be collapsed silently.

## Capability Tree

### Capability: Focus Scope Modeling
Represent the difference between tab-local, project-global, and inferred focus in the handshake.

#### Feature: Scope-aware focus header
- **Description**: Display current focus with provenance rather than asserting a single global active tag.
- **Inputs**: tab-local tag/env, project task state, recent Mind/task activity, runtime/session metadata.
- **Outputs**: a compact focus briefing with source labels such as tab-local, project-active, or inferred.
- **Behavior**: prefer explicit tab focus when valid, show fallback project activity when tab focus is absent/stale, and avoid misleading certainty.

#### Feature: Focus fallback policy
- **Description**: Determine what the handshake should show when no trustworthy tab-local focus exists.
- **Inputs**: tag presence, task freshness, open PRD-backed work, recent T2/T3 activity.
- **Outputs**: deterministic fallback focus candidate(s).
- **Behavior**: rank project-wide open and recently active workstreams so new tabs still get useful directional context.

### Capability: Mind-Derived Briefing Synthesis
Use Mind tiers to summarize recent developments and unresolveds more effectively than raw memory tails.

#### Feature: T2 recent developments summary
- **Description**: Surface recent implementation and planning themes using T2/T3-aware synthesis.
- **Inputs**: Mind handshake/canon artifacts, recent memory entries, optional session deltas.
- **Outputs**: concise bullets describing recent important changes.
- **Behavior**: cluster related memory items into themes, distinguish planned/in-progress/completed states, and preserve deterministic selection.

#### Feature: T3 unresolveds and follow-up fronts
- **Description**: Surface open gaps, strategic follow-ups, and unresolved architecture fronts.
- **Inputs**: T3 canon, open tasks, recent completions with explicit remaining gaps.
- **Outputs**: compact unresolveds section with operator/agent value.
- **Behavior**: prioritize incomplete, high-signal gaps over historical accomplishment summaries.

#### Feature: Explainable fallback rendering
- **Description**: Fall back to canonical sources when Mind synthesis is missing or stale.
- **Inputs**: availability/health of Mind artifacts and raw source files/CLI outputs.
- **Outputs**: briefing sections with equivalent semantics and clear source lineage.
- **Behavior**: degrade gracefully without hiding missing Mind data or changing the startup contract unpredictably.

### Capability: High-Value Work Prioritization
Make the handshake lead with open work that should shape agent behavior.

#### Feature: PRD-backed open task surfacing
- **Description**: Show open tasks with linked PRDs before low-value task inventory.
- **Inputs**: Taskmaster open tasks, PRD linkage metadata, tags, status, priority, recent updates.
- **Outputs**: ordered high-value work list.
- **Behavior**: prioritize open PRD-backed parent tasks and epics, then other in-progress/high-priority work.

#### Feature: Workstream health summaries
- **Description**: Replace raw tag counts with status-aware progress summaries.
- **Inputs**: tasks grouped by tag and status, recency markers, optional Mind activity.
- **Outputs**: workstream rows such as in-progress/pending/recently-completed plus short meaning text.
- **Behavior**: emphasize active or recently changed streams; suppress stale/noisy streams when budget is tight.

#### Feature: Recently completed context pruning
- **Description**: Keep only high-value recent completions when they help explain current open work.
- **Inputs**: completed tasks, recent memory, dependency relationships.
- **Outputs**: short recent-completion notes or omission when not useful.
- **Behavior**: avoid long historical done lists in the primary handshake.

### Capability: Operating Guidance Compression
Keep essential rules while reducing low-value operational noise.

#### Feature: Compact operational directives
- **Description**: Present only the minimum rules agents need to act safely and correctly.
- **Inputs**: AOC operating rules, RTK status, PRD/task guidance, STM policy.
- **Outputs**: a concise directives block.
- **Behavior**: preserve source-of-truth hierarchy and commands, but avoid dominating the handshake with policy boilerplate.

#### Feature: STM demotion and conditional elevation
- **Description**: Treat STM as optional handoff context unless the session is explicitly resumed/handoff-driven.
- **Inputs**: STM archive presence, current draft presence, startup reason, resume/handoff triggers.
- **Outputs**: footer guidance or elevated STM callout when relevant.
- **Behavior**: default to a small footer hint; expand only when the launch context warrants it.

### Capability: Handshake Rendering and Compatibility
Integrate the new briefing design into wrapper and Mind surfaces without breaking bounded startup behavior.

#### Feature: Full-mode focus-first layout
- **Description**: Reorder full handshake output into executive summary, current work briefing, Mind synthesis, high-value work, and compact directives.
- **Inputs**: all selected briefing sections.
- **Outputs**: new full-mode handshake markdown/text.
- **Behavior**: keep deterministic ordering, bounded size, and clear section labels.

#### Feature: Compact-mode policy refresh
- **Description**: Ensure compact mode still provides useful orientation aligned with the new mental model.
- **Inputs**: project root, focus signals, RTK state, optional high-value hints.
- **Outputs**: a smaller but still actionable compact handshake.
- **Behavior**: preserve low-token startup while exposing focus and key next-step cues.

#### Feature: Provenance and validation hooks
- **Description**: Support testing and operator inspection of how the handshake briefing was derived.
- **Inputs**: selected focus/workstream/task/mind inputs, hashes, source refs.
- **Outputs**: reproducible rendering behavior and inspectable source lineage.
- **Behavior**: avoid silent heuristics and keep rebuild/debug paths available for operators.

---

## Repository Structure

```text
project-root/
├── bin/
│   ├── aoc-agent-wrap                        # shell-side startup handshake rendering (current)
│   └── aoc-init                             # context snapshot generation primitives
├── crates/
│   ├── aoc-agent-wrap-rs/
│   │   └── src/main.rs                      # Mind-backed handshake retrieval/injection path
│   ├── aoc-mission-control/
│   │   └── src/main.rs                      # handshake -> canon -> evidence operator drilldown
│   ├── aoc-storage/
│   │   └── src/lib.rs                       # handshake snapshot persistence / retrieval
│   └── aoc-taskmaster/ or related task APIs # task and PRD metadata surfaces if shared code is needed
├── docs/
│   ├── configuration.md                     # handshake mode docs and operator configuration
│   ├── mind-v2-architecture-cutover-checklist.md
│   └── research/
│       └── aoc-handshake-briefing-v2.md     # proposed design/rollout notes (new)
├── .aoc/
│   ├── context.md                           # project snapshot fallback
│   ├── memory.md                            # durable decision log fallback
│   └── mind/
│       └── t3/
│           ├── handshake.md                 # bounded handshake artifact
│           └── project_mind.md              # broader canon context
└── .taskmaster/
    └── docs/prds/
        └── task-177_aoc_handshake_briefing_v2_prd_rpg.md
```

## Module Definitions

### Module: `bin/aoc-agent-wrap`
- **Maps to capability**: Handshake Rendering and Compatibility
- **Responsibility**: preserve startup boot flow while rendering compact/full fallback handshake sections and shell-level guidance.
- **Exports**:
  - startup handshake rendering
  - mode selection (`compact`, `full`, `off`)
  - shell fallback behavior when Mind artifacts are unavailable

### Module: `crates/aoc-agent-wrap-rs/src/main.rs`
- **Maps to capability**: Mind-Derived Briefing Synthesis + Provenance and validation hooks
- **Responsibility**: retrieve Mind handshake artifacts, apply overrides, and align runtime injection behavior with the redesigned briefing contract.
- **Exports**:
  - handshake snapshot retrieval
  - startup/context injection orchestration
  - source-aware fallback policy

### Module: task/PRD status selection surface (shared Rust or shell helper as implementation chooses)
- **Maps to capability**: High-Value Work Prioritization + Focus Scope Modeling
- **Responsibility**: select open PRD-backed tasks, summarize workstream progress, and compute focus provenance.
- **Exports**:
  - focus status selection
  - PRD-linked open work queries
  - workstream health summary generation

### Module: Mind handshake synthesis pipeline (`.aoc/mind/t3/handshake.md` and related generators)
- **Maps to capability**: Mind-Derived Briefing Synthesis
- **Responsibility**: produce bounded recent-developments and unresolveds content suitable for handshake injection.
- **Exports**:
  - T2/T3-derived briefing entries
  - deterministic handshake payload
  - handshake snapshot hashes and provenance

### Module: `crates/aoc-mission-control/src/main.rs`
- **Maps to capability**: Provenance and validation hooks
- **Responsibility**: keep operator visibility into handshake -> canon -> evidence lineage and enable rebuild/debug flows.
- **Exports**:
  - handshake preview/drilldown
  - rebuild controls
  - status diagnostics for stale/missing handshake data

### Module: `docs/*`
- **Maps to capability**: Operating Guidance Compression + rollout communication
- **Responsibility**: document the new handshake contract, focus semantics, mode behavior, and rollout guidance.
- **Exports**:
  - updated operator docs
  - maintainer guidance for new handshake expectations
  - implementation notes for downstream repos

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies - these are built first.

- **handshake-briefing-contract**: defines section ordering, signal priority, budget rules, and source-of-truth hierarchy.
- **focus-scope-model**: defines tab-local vs project-global vs inferred focus semantics and fallback rules.

### Synthesis and Selection Layer (Phase 1)
- **mind-briefing-selection**: Depends on [handshake-briefing-contract, focus-scope-model]
- **task-priority-selection**: Depends on [handshake-briefing-contract, focus-scope-model]
- **workstream-health-summary**: Depends on [handshake-briefing-contract, task-priority-selection]

### Rendering Layer (Phase 2)
- **full-mode-layout-refresh**: Depends on [mind-briefing-selection, task-priority-selection, workstream-health-summary]
- **compact-mode-layout-refresh**: Depends on [focus-scope-model, task-priority-selection]
- **stm-conditional-rendering**: Depends on [handshake-briefing-contract]

### Runtime Integration Layer (Phase 3)
- **wrapper-fallback-integration**: Depends on [full-mode-layout-refresh, compact-mode-layout-refresh, stm-conditional-rendering]
- **mind-handshake-generation-refresh**: Depends on [mind-briefing-selection, full-mode-layout-refresh]
- **operator-provenance-alignment**: Depends on [mind-handshake-generation-refresh]

### Validation and Docs Layer (Phase 4)
- **regression-and-golden-tests**: Depends on [wrapper-fallback-integration, mind-handshake-generation-refresh, operator-provenance-alignment]
- **docs-and-rollout-guidance**: Depends on [wrapper-fallback-integration, regression-and-golden-tests]

---

## Development Phases

### Phase 0: Contract and Scope Semantics
**Goal**: define what the new handshake is optimizing for and how focus provenance works.

**Entry Criteria**: task approved; current wrapper, task, and Mind handshake surfaces reviewed.

**Tasks**:
- [ ] Define handshake briefing contract and section priority order (depends on: none)
  - Acceptance criteria: a written contract defines executive-summary-first rendering, section budgets, and which sources feed each section.
  - Test strategy: spec review plus golden fixture expectations for section presence/order.
- [ ] Define focus provenance and fallback rules (depends on: none)
  - Acceptance criteria: tab-local, project-global, and inferred focus semantics are explicit and deterministic.
  - Test strategy: unit-style matrix for explicit tag, missing tag, stale tag, and conflicting activity cases.

**Exit Criteria**: maintainers can describe the handshake without referring to the old raw-inventory layout.

**Delivers**: an implementation-ready contract for focus-first handshake behavior.

---

### Phase 1: Briefing Inputs and Prioritization
**Goal**: compute the right content before changing rendering.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Add Mind-derived recent developments and unresolveds selection policy (depends on: [Phase 0 tasks])
  - Acceptance criteria: recent implementation themes and unresolved gaps can be generated from Mind T2/T3 or a documented fallback chain.
  - Test strategy: fixtures proving selection prefers synthesized themes over raw tail logs when available.
- [ ] Add open PRD-backed task prioritization (depends on: [Phase 0 tasks])
  - Acceptance criteria: open PRD-linked tasks and active parent tasks are selected ahead of done/history-heavy items.
  - Test strategy: task fixtures covering PRD-linked, non-PRD, done, blocked, and in-progress cases.
- [ ] Add workstream health summary generation (depends on: [open PRD-backed task prioritization])
  - Acceptance criteria: tags render as status-aware summaries rather than raw counts.
  - Test strategy: grouped task fixtures confirm in-progress/pending/recent-complete rollups.

**Exit Criteria**: the system can compute actionable briefing inputs independent of final text layout.

**Delivers**: reusable selectors for focus, workstreams, and high-value tasks.

---

### Phase 2: Handshake Rendering Refresh
**Goal**: apply the new content model to full and compact handshake modes.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Implement full-mode focus-first layout (depends on: [Phase 1 tasks])
  - Acceptance criteria: the primary handshake begins with executive summary, scoped focus, Mind synthesis, and high-value open work.
  - Test strategy: golden rendering tests for full mode with and without Mind data.
- [ ] Refresh compact mode around focus and minimal next-step cues (depends on: [focus provenance and fallback rules, open PRD-backed task prioritization])
  - Acceptance criteria: compact mode remains low-token while surfacing useful focus and work guidance.
  - Test strategy: golden rendering tests asserting bounded length and presence of key cues.
- [ ] Demote STM to conditional footer/elevation behavior (depends on: [handshake briefing contract])
  - Acceptance criteria: STM appears as lightweight footer guidance unless resume/handoff context elevates it.
  - Test strategy: startup reason fixtures for normal launch vs resume/handoff contexts.

**Exit Criteria**: both handshake modes reflect the new briefing model without regressing bounded output.

**Delivers**: visible user-facing handshake redesign.

---

### Phase 3: Runtime, Provenance, and Rollout
**Goal**: align Mind generation, operator surfaces, and regressions with the new handshake contract.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Align Mind handshake generation with the new briefing sections (depends on: [full-mode focus-first layout, Mind-derived recent developments and unresolveds selection policy])
  - Acceptance criteria: generated handshake artifacts feed the new visible sections and preserve hash/provenance behavior.
  - Test strategy: rebuild tests verifying stable output and traceable source linkage.
- [ ] Preserve operator drilldown and diagnostics for handshake provenance (depends on: [Align Mind handshake generation])
  - Acceptance criteria: Mission Control/operator surfaces still explain handshake -> canon -> evidence lineage and stale/missing states.
  - Test strategy: integration tests for preview/drilldown and rebuild paths.
- [ ] Document and validate rollout behavior (depends on: [all prior phase tasks])
  - Acceptance criteria: docs explain new semantics, focus provenance, compact/full differences, and fallback behavior; regressions cover expected startup cases.
  - Test strategy: targeted startup/injection regressions plus docs review.

**Exit Criteria**: the redesign is shippable with documented behavior and operator confidence.

**Delivers**: end-to-end handshake v2 implementation readiness.

---

## Test Pyramid

```text
        /\
       /E2E\       ← 10% (startup/render integration and operator drilldown)
      /------\
     /Integration\ ← 35% (selector + renderer + Mind/task data interactions)
    /------------\
   /  Unit Tests  \ ← 55% (focus fallback, task filtering, workstream summaries)
  /----------------\
```

## Coverage Requirements
- Line coverage: 85% minimum on new selector/rendering helpers
- Branch coverage: 80% minimum on focus fallback and conditional rendering logic
- Function coverage: 90% minimum for new handshake selection functions
- Statement coverage: 85% minimum across affected handshake rendering paths

## Critical Test Scenarios

### Focus scope model
**Happy path**:
- Tab-local tag is set and still matches active open work.
- Expected: handshake labels the focus as tab-local and uses it as the leading workstream.

**Edge cases**:
- New tab has no tag but there are multiple active project streams.
- Expected: handshake shows no authoritative tab focus and falls back to project-active/inferred summaries.
- Tab-local tag exists but has no open tasks while another stream is actively changing.
- Expected: handshake marks provenance clearly and avoids overstating stale focus.

**Error cases**:
- Task query fails or returns malformed output.
- Expected: handshake degrades to simpler context while preserving startup flow.

**Integration points**:
- Focus computation interacts correctly with Mind-generated handshake content and shell fallback rendering.
- Expected: visible output stays deterministic regardless of which layer supplied the data.

### High-value task prioritization
**Happy path**:
- Multiple open tasks exist and some have PRD links.
- Expected: PRD-backed tasks appear first, followed by other in-progress/high-priority tasks.

**Edge cases**:
- All tasks in the current tag are done, but other project tags have active PRD-backed work.
- Expected: handshake avoids showing only done tasks and uses project-level fallbacks.
- PRD-linked tasks are pending while a non-PRD task is in-progress.
- Expected: ordering respects both status and strategic value per defined policy.

**Error cases**:
- PRD metadata unavailable.
- Expected: fallback task ordering still surfaces open work without hiding the metadata failure.

**Integration points**:
- Workstream health summaries align with task selection and do not double-count completed-only streams.
- Expected: tag summaries and open-task lists remain internally consistent.

### Mind-derived briefing synthesis
**Happy path**:
- T2/T3 handshake artifacts exist with recent developments and unresolveds.
- Expected: handshake surfaces synthesized themes instead of raw memory tail lines.

**Edge cases**:
- Mind data exists but is stale or sparse.
- Expected: fallback logic supplements from memory/task sources while keeping the same section structure.

**Error cases**:
- Handshake artifact missing or unreadable.
- Expected: startup stays fail-open and shell/runtime fallback content appears.

**Integration points**:
- Mission Control drilldown can still trace handshake text back to canon/evidence.
- Expected: provenance links remain intact after redesign.

### STM conditional rendering
**Happy path**:
- Standard startup with STM archive present.
- Expected: handshake shows only a compact footer hint.

**Edge cases**:
- Resume/handoff trigger is active.
- Expected: STM guidance is elevated with explicit commands.

**Error cases**:
- STM directory unreadable.
- Expected: handshake omits STM details without blocking startup.

## Test Generation Guidelines
- Prefer golden rendering fixtures for compact/full mode snapshots to catch accidental salience regressions.
- Build focused selector tests around tab focus, PRD linkage, task statuses, and missing-data fallbacks.
- Keep integration tests bounded: startup should never require a live full Mind pipeline to validate basic fallback behavior.
- Preserve existing fail-open startup semantics and hash-dedupe expectations from task 139 coverage.

---

## Architecture

## System Components
- **Shell wrapper handshake renderer**: current startup entrypoint that chooses compact/full/off behavior and can emit fallback context directly.
- **Rust wrapper / Mind runtime path**: retrieves Mind-generated handshake artifacts and injects them into the runtime path for bounded contextualization.
- **Taskmaster query layer**: provides task, subtask, tag, and PRD linkage metadata that should now drive the handshake’s work briefing sections.
- **Mind T2/T3 synthesis**: supplies recent developments, unresolveds, and canon-aligned summaries more usefully than raw memory tails.
- **Mission Control provenance surface**: validates and explains how handshake content maps back to canon and evidence.

## Data Models
- **Focus signal**: `{ kind: tab_local | project_active | inferred, tag?: string, confidence/source metadata }`
- **Workstream summary**: `{ tag, in_progress, pending, recent_completed, brief }`
- **High-value task summary**: `{ task_id, title, status, priority, tag, has_prd, brief, remaining_gaps? }`
- **Mind briefing block**: `{ recent_developments[], unresolveds[], source = t2_t3 | fallback }`
- **Handshake section contract**: ordered sections with budget ceilings and fallback paths.

## Technology Stack
- Bash for shell startup orchestration and lightweight fallback rendering.
- Rust for Mind-backed runtime integration, persistence, and operator surfaces.
- Taskmaster CLI/state for task and PRD metadata.
- AOC Mind storage/artifacts for synthesized context and provenance.

**Decision: Prefer focus-first briefing over raw inventory dump**
- **Rationale**: agents need prioritization and next-action guidance more than broad repository census during startup.
- **Trade-offs**: some immediately visible static context is demoted or omitted unless requested.
- **Alternatives considered**: keep the current dump and only trim length; rejected because the deeper issue is salience, not just volume.

**Decision: Treat Mind T2/T3 as presentation layer with explainable fallback**
- **Rationale**: T2/T3 better express themes, unresolveds, and cross-session importance while memory remains the durable fact store.
- **Trade-offs**: increases dependence on synthesis quality and freshness, so fallback behavior must be explicit.
- **Alternatives considered**: only show raw memory tail; rejected because it leaves too much interpretation burden on the agent.

**Decision: Distinguish tab-local focus from project-global active work**
- **Rationale**: multi-agent/tab workflows make a single `current tag` unreliable as a universal truth.
- **Trade-offs**: slightly more complex wording in the handshake.
- **Alternatives considered**: keep one current-tag line; rejected because it overstates certainty and can mislead new tabs.

**Decision: Surface open PRD-backed tasks first**
- **Rationale**: PRD-linked tasks usually represent epics or direction-setting work with clearer intent and subtasks.
- **Trade-offs**: some active but unlinked work may appear later.
- **Alternatives considered**: show all tasks equally or preserve done-task lists; rejected as too noisy.

## Risks

## Technical Risks
**Risk**: Mind synthesis may be too stale, sparse, or differently shaped across repos.
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: define strict fallback semantics that preserve the same visible section model even when Mind data is absent.
- **Fallback**: render sections from memory/task/context sources and label provenance as fallback.

**Risk**: Focus inference may mis-rank active project work in multi-agent setups.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: make provenance explicit and keep fallback selection deterministic and conservative.
- **Fallback**: show multiple candidate workstreams without asserting one authoritative focus.

**Risk**: Task/PRD selection logic may become expensive or brittle in shell-only rendering.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: keep selectors bounded, cacheable, and shared where practical between shell/Rust paths.
- **Fallback**: degrade to simple open-task summaries when richer metadata is unavailable.

## Dependency Risks
- Taskmaster query surfaces may need small extensions or robust parsing helpers to expose PRD-linked open work cleanly.
- Mind handshake generation may require follow-up changes beyond wrapper layout if current artifacts do not already encode recent developments/unresolveds explicitly.
- Operator/provenance surfaces must stay aligned so the new briefing does not become less explainable than the current one.

## Scope Risks
- It is easy to over-expand this into a full dynamic workspace dashboard rather than a startup briefing.
- Compact mode could regress into another too-minimal shell banner if the same prioritization model is not applied there too.
- There is a temptation to perfect project-global focus resolution before shipping any improvement; the MVP should support provenance-aware uncertainty rather than blocking on global coordination primitives.

---

## References
- `.taskmaster/docs/prds/aoc-mind-v2_t3-project-canon_prd_rpg.md`
- `.taskmaster/docs/prds/task-169_aoc_detached_pi_subagent_runtime_prd_rpg.md`
- `.taskmaster/docs/prds/task-175_aoc_search_stack_prd_rpg.md`
- `docs/mind-v2-architecture-cutover-checklist.md`
- `docs/configuration.md`
- `bin/aoc-agent-wrap`

## Glossary
- **Handshake**: startup/context injection briefing shown to agents in AOC-managed environments.
- **T2**: session-scoped synthesis/reflection over related T1 artifacts.
- **T3**: project-scoped canon/alignment over T2 plus memory, tasks, STM, and project/session exports.
- **PRD-backed task**: Taskmaster task with an explicit linked PRD document.
- **Focus provenance**: explanation of whether a shown focus came from tab-local state, project-global activity, or inference.

## Open Questions
- Should project-global active work ranking consider only Taskmaster state, or also recent memory/Mind activity when tasks lag reality?
- Should compact mode show one best candidate stream or multiple candidate streams when tab focus is absent?
- Should the handshake surface a confidence label or just provenance wording for inferred focus?
- Which portions of the workstream/task summarization should live in Rust vs shell helpers for maintainability?

---

# How Task Master Uses This PRD
This PRD defines the handshake redesign as a dependency-aware effort:
- focus and contract semantics first,
- synthesis and task prioritization second,
- rendering refresh third,
- runtime alignment, regressions, and docs last.

The resulting task structure should keep implementation topologically buildable, preserve startup safety, and avoid coupling the visual redesign to any one data source or rendering layer.