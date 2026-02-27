# Taskmaster Task Reflections PRD (RPG)

## Problem Statement
Taskmaster currently captures task metadata (title, description, details, dependencies, test strategy, status), but it lacks a dedicated reflection surface at task closure.

In AOC’s agent-only execution model, this creates a context gap:
- Developers directing agent intent lose the rationale/outcome trail after implementation
- Downstream agents consume incomplete context and may repeat analysis or diverge from prior intent
- Handoffs rely on ad-hoc notes in memory/STM instead of task-local, durable delivery context

We need a first-class task reflection capability that is concise, structured, and optimized for both human direction and agent continuation.

## Target Users
- **Directing developer/operator**: assigns intent and validates whether delivered work matched that intent.
- **Implementing agent**: records what was actually done and why.
- **Follow-up agent(s)**: consume reflection to continue related tasks with minimal rediscovery.

## Success Metrics
- >= 80% of tasks closed in targeted tags include a reflection within pilot period.
- >= 50% reduction in follow-up “why was this done?” clarification loops for reflected tasks.
- Reflection retrieval available in both CLI (`tm show`, JSON output) and Taskmaster TUI detail view.
- Backward compatibility maintained: 0 failures when reading existing tasks without reflections.

---

## Capability Tree

### Capability: Structured Reflection Capture
Capture delivery context at/near task completion.

#### Feature: Reflection schema
- **Description**: Define a stable, machine-readable reflection object for tasks.
- **Inputs**: Task completion event, optional operator guidance.
- **Outputs**: Persisted `taskReflection` payload on task.
- **Behavior**: Validate required fields; store optional metadata with sane defaults.

#### Feature: Completion-time authoring flow
- **Description**: Provide low-friction CLI/TUI prompts to add reflection when marking task done.
- **Inputs**: Existing task data + user/agent reflection text.
- **Outputs**: Task marked done with attached reflection.
- **Behavior**: Prompt once, support skip/partial based on policy.

### Capability: Agent-Oriented Context Semantics
Ensure reflections carry both intent alignment and execution context.

#### Feature: Intent alignment summary
- **Description**: Capture whether delivered outcome matched directing intent.
- **Inputs**: Original task scope + delivered outcome.
- **Outputs**: Structured summary field.
- **Behavior**: Require concise statement for delivered-vs-expected.

#### Feature: Decision and risk trace
- **Description**: Capture key tradeoffs, known gaps, and operational risks.
- **Inputs**: Implementation choices and unresolved constraints.
- **Outputs**: Decision/risk fields with optional follow-up links.
- **Behavior**: Encourage high-signal bullets, avoid long narrative logs.

### Capability: Reflection Retrieval for Handoffs
Expose reflections where follow-up work is planned/executed.

#### Feature: Task detail rendering
- **Description**: Show reflection in `tm show` and TUI task panel.
- **Inputs**: Task data with/without reflection.
- **Outputs**: Human-readable reflection block + JSON representation.
- **Behavior**: Render only when present; no breakage for legacy tasks.

#### Feature: Reflection-aware discovery
- **Description**: Make reflection text discoverable in task search/filter workflows.
- **Inputs**: Query text/tags/status.
- **Outputs**: Matched tasks where reflection contributes relevance.
- **Behavior**: Include reflection fields in textual matching and optional filters.

---

## Repository Structure (Target Touchpoints)

```text
project-root/
├── crates/
│   ├── aoc-core/
│   │   └── src/lib.rs                 # Task model extension (reflection type)
│   ├── aoc-cli/
│   │   └── src/task.rs                # CLI authoring/render/search integration
│   └── aoc-taskmaster/
│       └── src/{state.rs,ui.rs}       # TUI capture/display integration
├── docs/
│   └── (taskmaster or workflow docs)  # Reflection guidance and examples
└── .taskmaster/
    └── tasks/tasks.json               # Persisted task reflection data
```

## Module Definitions

### Module: `crates/aoc-core/src/lib.rs`
- **Maps to capability**: Structured Reflection Capture
- **Responsibility**: Define `TaskReflection` model and attach to `Task` with defaults and backward compatibility.
- **Exports**:
  - `TaskReflection` (new structured type)
  - `Task.task_reflection` (new optional field)

### Module: `crates/aoc-cli/src/task.rs`
- **Maps to capability**: Completion-time authoring flow + task detail rendering + discovery
- **Responsibility**: Support setting/editing/viewing reflections and completion prompt integration.
- **Exports/commands**:
  - `tm done` + optional reflection prompt flow
  - `tm show` reflection rendering
  - JSON output includes reflection object

### Module: `crates/aoc-taskmaster/src/{state.rs, ui.rs}`
- **Maps to capability**: Reflection retrieval for handoffs
- **Responsibility**: Display reflection in TUI task details and optional completion modal.

### Module: `docs/*`
- **Maps to capability**: Agent-oriented context semantics
- **Responsibility**: Define concise reflection writing standard for developer→agent and agent→agent handoff value.

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies.
- **Reflection contract**: canonical schema, required/optional fields, and policy (required-on-done vs recommended).

### Data Model Layer (Phase 1)
- **Core task schema integration**: Depends on [Reflection contract].

### CLI Integration Layer (Phase 2)
- **CLI authoring and rendering**: Depends on [Core task schema integration].

### TUI Integration Layer (Phase 3)
- **Taskmaster TUI reflection surfaces**: Depends on [Core task schema integration, CLI authoring semantics].

### Discovery + Docs Layer (Phase 4)
- **Search/filter, docs, rollout guidance**: Depends on [CLI integration, TUI integration].

---

## Development Phases

### Phase 0: Reflection Contract
**Goal**: Lock reflection structure and semantics for agent-only workflows.

**Entry Criteria**: Approved scope for task 133.

**Tasks**:
- [ ] Define `taskReflection` schema fields and validation rules.
  - Acceptance criteria: schema documented and agreed.
  - Test strategy: contract tests for parsing/serialization defaults.

**Exit Criteria**: Single source of truth for reflection payload and policy.

**Delivers**: Implementation-ready contract for all integrations.

---

### Phase 1: Core Data Model
**Goal**: Persist reflections in task data safely.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Extend task model with optional reflection object.
- [ ] Ensure legacy task JSON deserializes without reflection.

**Exit Criteria**: Read/write compatibility for tasks with and without reflections.

**Delivers**: Stable persistence layer.

---

### Phase 2: CLI Authoring + Visibility
**Goal**: Enable reflection capture and viewing in CLI.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Add reflection capture to completion flow (`tm done`, optional prompt).
- [ ] Add explicit reflection edit/update path (`tm edit` extension or dedicated subcommand).
- [ ] Render reflection block in `tm show` and JSON output.

**Exit Criteria**: Operator/agent can create, update, and view reflections fully in CLI.

**Delivers**: End-to-end non-TUI reflection workflow.

---

### Phase 3: TUI Integration
**Goal**: Surface reflection context in native Taskmaster interface.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Add reflection panel in task details view.
- [ ] Add completion-time reflection capture affordance in TUI.

**Exit Criteria**: Reflection equally available in TUI and CLI.

**Delivers**: Consistent UX for agent operators using Taskmaster TUI.

---

### Phase 4: Discovery, Guidance, and Rollout
**Goal**: Make reflections operationally useful.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Include reflection fields in task search indexing/matching.
- [ ] Document concise reflection standard tuned for developer intent + agent handoff.
- [ ] Pilot on selected tags and capture adoption metrics.

**Exit Criteria**: Reflections are discoverable, documented, and measured.

**Delivers**: Sustainable reflection practice rather than one-off storage.

---

## Test Pyramid

```text
        /\
       /E2E\       ← 10%
      /-----\
     / Int  \      ← 30%
    /--------\
   /  Unit    \    ← 60%
  /------------\
```

## Coverage Requirements
- Line coverage: 85% minimum on touched modules
- Branch coverage: 75% minimum on touched modules
- Function coverage: 85% minimum on touched modules
- Statement coverage: 85% minimum on touched modules

## Critical Test Scenarios

### Reflection schema + persistence
**Happy path**:
- Task with complete reflection serializes/deserializes correctly.
- Expected: no field loss; JSON shape stable.

**Edge cases**:
- Optional fields omitted.
- Expected: defaults apply; rendering still coherent.

**Error cases**:
- Invalid reflection payload types.
- Expected: validation error without corrupting task file.

**Integration points**:
- Existing tasks with no reflection continue to load.
- Expected: no regression in list/show/edit flows.

### CLI completion flow
**Happy path**:
- `tm done` captures reflection and marks task done.
- Expected: status + reflection persisted atomically.

**Edge cases**:
- User skips optional fields.
- Expected: minimal valid reflection accepted if policy allows.

**Error cases**:
- Interrupted prompt/write failure.
- Expected: clear error and no partial corruption.

**Integration points**:
- `tm show --json` includes reflection object.
- Expected: downstream agents can parse structured fields.

### TUI display/capture
**Happy path**:
- Reflection displayed in task detail panel.
- Expected: clear sections (outcome, decisions, risks, follow-ups).

**Edge cases**:
- No reflection present.
- Expected: empty-state messaging without UI break.

**Error cases**:
- Very long reflection text.
- Expected: truncation/scroll behavior remains usable.

## Test Generation Guidelines
- Prefer deterministic fixture-based tests for task JSON mutations.
- Add snapshot tests for CLI/TUI rendering only where stable output is expected.
- Ensure contract tests cover backward compatibility for pre-reflection tasks.

---

## System Components
- **Task persistence layer**: `.taskmaster/tasks/tasks.json` read/write via `aoc-core` models.
- **CLI orchestration layer**: `aoc-cli task` command handlers.
- **TUI presentation layer**: `aoc-taskmaster` state and rendering modules.

## Data Models

### Proposed `taskReflection` object
- `outcome`: delivered vs expected summary (required)
- `intentAlignment`: how delivery maps to directing intent (required)
- `decisions`: key tradeoffs/constraints (required)
- `knownGaps`: unresolved issues or risks (optional)
- `followUps`: next task candidates or links (optional)
- `confidence`: `low|medium|high` (optional)
- `updatedAt`: timestamp (auto-managed)

## Technology Stack
- Rust + serde-based model serialization (existing stack)
- Existing Taskmaster CLI/TUI command architecture

**Decision: Structured reflection object instead of free-text blob**
- **Rationale**: Improves agent parseability and retrieval quality.
- **Trade-offs**: Slightly more input friction.
- **Alternatives considered**: single markdown field on task details.

**Decision: Task-local reflection (not global memory-only)**
- **Rationale**: Keeps delivery context attached to execution unit.
- **Trade-offs**: Requires schema + UI changes.
- **Alternatives considered**: write-only to `.aoc/memory.md`.

---

## Technical Risks
**Risk**: Reflection prompts add operator friction.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: concise template, optional metadata, defaults.
- **Fallback**: make reflection recommended (not required) per tag policy.

**Risk**: Inconsistent reflection quality across agents.
- **Impact**: Medium
- **Likelihood**: High
- **Mitigation**: strict sectioned schema + examples in docs.
- **Fallback**: lint/refinement pass in completion workflow.

## Dependency Risks
- TUI changes may lag behind CLI support.
- Mitigation: ship CLI-first; keep TUI compatibility path.

## Scope Risks
- Expanding into broad “knowledge graph” features in same task.
- Mitigation: keep scope to task-level capture, retrieval, and handoff usefulness.

---

## References
- `.taskmaster/templates/example_prd_rpg.txt`
- `crates/aoc-core/src/lib.rs`
- `crates/aoc-cli/src/task.rs`
- `crates/aoc-taskmaster/src/ui.rs`

## Glossary
- **Reflection**: concise, structured delivery context added at/after task completion.
- **Intent alignment**: explanation of how output maps to directing developer’s objective.
- **Handoff context**: details needed by next agent to continue work without rediscovery.

## Open Questions
- Should reflection be mandatory for `done` in specific tags (policy by tag)?
- Should `followUps` auto-suggest creating dependent tasks?
- Should reflection search be opt-in filter or default full-text behavior?
