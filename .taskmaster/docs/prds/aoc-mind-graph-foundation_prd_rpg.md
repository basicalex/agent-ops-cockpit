# AOC Mind Conversation Graph Foundation

## PRD (Repository Planning Graph / RPG Method)

---

<overview>

## Problem Statement
AOC Mind now captures branch lineage and observer artifacts, but there is no first-class graph contract/API to reliably project this data into a session/conversation graph for operators. Without a formal graph layer, branch-aware context remains difficult to inspect, debug, and eventually visualize in Mission Control. This creates three concrete pains: (1) branch relationships are not queryable as a coherent graph object, (2) artifact provenance cannot be overlaid consistently onto branch trees, and (3) downstream UI work risks ad-hoc data contracts.

## Target Users
1) **AOC operator (primary)**
- Runs multiple branched sessions and needs to understand “what happened where” quickly.
- Needs deterministic graph exports for debugging and handoff.

2) **AOC maintainer (secondary)**
- Implements Mind runtime and Mission Control features.
- Needs stable graph schemas to avoid fragile one-off adapters.

3) **Future graph UI consumer (tertiary)**
- Mission Control graph pane / external renderer.
- Needs compact, versioned node-edge JSON suitable for tree/graph rendering.

## Success Metrics
- 100% of ingested branched conversations in a session have valid root/parent graph edges.
- Graph export command returns deterministic output ordering across repeated runs.
- Graph integrity checks catch invalid lineage states before runtime processing.
- Mission Control integration path can consume graph payload without storage schema changes.
- Export latency target: <100ms for 200 conversation nodes and 1000 artifact links on commodity dev hardware.

</overview>

---

<functional-decomposition>

## Capability Tree

### Capability: Graph-Native Lineage Contract
Define and enforce canonical branch metadata so graph edges are deterministic.

#### Feature: Canonical lineage schema enforcement
- **Description**: Validate `mind_lineage` metadata as authoritative graph lineage input.
- **Inputs**: Raw event attrs (`session_id`, `parent_conversation_id`, `root_conversation_id`).
- **Outputs**: Validated lineage metadata or explicit validation errors.
- **Behavior**: Reject partial/contradictory branch metadata, allow legacy fallback only when no branch metadata is declared.

#### Feature: Root/parent consistency guardrails
- **Description**: Ensure branch trees remain acyclic and root-consistent.
- **Inputs**: Conversation lineage rows from storage.
- **Outputs**: Integrity result (ok/errors) and normalized tree scope keys.
- **Behavior**: Enforce parent != self, parent implies root, and same-tree root consistency.

### Capability: Mind Graph Projection
Build graph nodes/edges from storage lineage and observer artifacts.

#### Feature: Conversation node projection
- **Description**: Project conversation lineage into graph nodes.
- **Inputs**: `conversation_lineage`, context state summaries.
- **Outputs**: Ordered node list with conversation/session metadata.
- **Behavior**: Deterministic sorting and stable node identifiers.

#### Feature: Branch edge projection
- **Description**: Project branch relationships as explicit edges.
- **Inputs**: parent/root lineage fields.
- **Outputs**: `branch_of` edges (child -> parent) and root anchors.
- **Behavior**: Emit only valid edges; include integrity diagnostics for dropped edges.

#### Feature: Artifact provenance overlay
- **Description**: Attach T1/T2 and provenance metadata to graph context.
- **Inputs**: observations, reflections, semantic provenance, task links.
- **Outputs**: Artifact nodes and/or edge annotations linked to conversations.
- **Behavior**: Keep lightweight default payload; optional expanded mode for deep diagnostics.

### Capability: Graph Query + Export Interface
Expose graph data for CLI/TUI/UI consumers.

#### Feature: Session tree graph query
- **Description**: Return graph scope by `(session_id, root_conversation_id)`.
- **Inputs**: Session and optional seed/root conversation.
- **Outputs**: Graph payload with nodes, edges, diagnostics, metadata.
- **Behavior**: Tree-scoped when lineage present; session-scoped fallback for legacy data.

#### Feature: Export formats
- **Description**: Provide machine and human renderable graph exports.
- **Inputs**: Graph payload + format flag.
- **Outputs**: JSON (canonical), Mermaid (optional), compact text summary.
- **Behavior**: JSON is source of truth; Mermaid/text are deterministic projections.

#### Feature: Runtime-ready feed adapter
- **Description**: Add adapter contract for Mission Control graph pane future consumption.
- **Inputs**: Graph payload + optional live observer feed deltas.
- **Outputs**: View-model payload with minimal UI transforms.
- **Behavior**: Keep adapter pure/side-effect-free and reconnect-safe.

</functional-decomposition>

---

<structural-decomposition>

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-core/
│   │   └── src/
│   │       └── mind_contracts.rs              # Graph lineage contract primitives
│   ├── aoc-storage/
│   │   └── src/
│   │       └── lib.rs                         # Lineage persistence + graph queries
│   ├── aoc-mind/
│   │   └── src/
│   │       └── graph_projection.rs (new)      # Node/edge projection and diagnostics
│   ├── aoc-cli/
│   │   └── src/
│   │       └── mind_graph.rs (new)            # Graph export command surface
│   └── aoc-mission-control/
│       └── src/
│           └── graph_adapter.rs (new)         # Graph payload adapter for future pane
└── docs/
    └── mind-graph.md (new)                    # Graph schema and operator usage
```

## Module Definitions

### Module: `aoc-core::mind_contracts`
- **Maps to capability**: Graph-Native Lineage Contract
- **Responsibility**: Canonical lineage schema + validation rules + graph payload types.
- **File structure**:
  ```text
  mind_contracts.rs
  ```
- **Exports**:
  - `parse_conversation_lineage_metadata()` - validates lineage attrs
  - `canonical_lineage_attrs()` - emits canonical attrs
  - `MindGraphPayload` / `MindGraphNode` / `MindGraphEdge` (new)

### Module: `aoc-storage`
- **Maps to capability**: Mind Graph Projection, Graph Query + Export Interface
- **Responsibility**: Persist lineage and provide deterministic graph query surfaces.
- **File structure**:
  ```text
  lib.rs
  migrations/0004_session_conversation_tree.sql
  ```
- **Exports**:
  - `conversation_lineage()`
  - `session_tree_conversations()`
  - `mind_graph_scope()` (new)
  - `mind_graph_integrity_report()` (new)

### Module: `aoc-mind::graph_projection` (new)
- **Maps to capability**: Mind Graph Projection
- **Responsibility**: Build graph nodes/edges + artifact overlays from storage models.
- **File structure**:
  ```text
  graph_projection.rs
  ```
- **Exports**:
  - `project_session_graph()`
  - `attach_artifact_overlay()`

### Module: `aoc-cli::mind_graph` (new)
- **Maps to capability**: Graph Query + Export Interface
- **Responsibility**: CLI entrypoint for graph exports.
- **File structure**:
  ```text
  mind_graph.rs
  ```
- **Exports**:
  - `aoc-mind graph export --session <id> [--root <conversation>] [--format json|mermaid|summary]`

### Module: `aoc-mission-control::graph_adapter` (new)
- **Maps to capability**: Runtime-ready feed adapter
- **Responsibility**: Convert graph payload into UI-ready rows/nodes/chips for future pane.
- **File structure**:
  ```text
  graph_adapter.rs
  ```
- **Exports**:
  - `build_graph_view_model()`

</structural-decomposition>

---

<dependency-graph>

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies.

- **mind contract primitives (`aoc-core`)**: lineage validation + graph payload schema.
- **graph serialization contract (`aoc-core`)**: canonical ordering and format rules.

### Storage Graph Layer (Phase 1)
- **lineage persistence/query (`aoc-storage`)**: Depends on [`aoc-core` graph lineage contract]
- **integrity checker (`aoc-storage`)**: Depends on [`aoc-storage` lineage persistence/query]

### Projection Layer (Phase 2)
- **graph projection (`aoc-mind`)**: Depends on [`aoc-storage` graph query, `aoc-core` graph payload schema]
- **artifact overlay (`aoc-mind`)**: Depends on [`graph projection`, `aoc-storage` artifact/provenance APIs]

### Interface Layer (Phase 3)
- **CLI graph export (`aoc-cli`)**: Depends on [`aoc-mind` projection]
- **Mission Control adapter (`aoc-mission-control`)**: Depends on [`aoc-core` graph schema, `aoc-mind` projection payload]

### Docs/Operational Layer (Phase 4)
- **operator docs (`docs/mind-graph.md`)**: Depends on [`CLI export`, `schema contracts`, `integrity checks`]

</dependency-graph>

---

<implementation-roadmap>

## Development Phases

### Phase 0: Graph Contract Baseline
**Goal**: Establish canonical graph schema and lineage validation semantics.

**Entry Criteria**: Existing lineage contract and migration v4 are merged.

**Tasks**:
- [ ] Define `MindGraphPayload` + node/edge contracts in `aoc-core` (depends on: none)
  - Acceptance criteria: schema includes stable IDs, node types, edge types, diagnostics list.
  - Test strategy: unit tests for canonical serialization and deterministic ordering.

- [ ] Extend lineage validation error taxonomy for graph integrity (depends on: none)
  - Acceptance criteria: invalid branch states map to typed, actionable errors.
  - Test strategy: table-driven validation tests.

**Exit Criteria**: Graph payload contract is versioned, tested, and used as source of truth.

**Delivers**: Stable model for storage/runtime/UI integration.

---

### Phase 1: Storage Graph Query + Integrity
**Goal**: Build deterministic graph-scope query and integrity reporting.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Implement `mind_graph_scope(session_id, root)` query in `aoc-storage` (depends on: [Phase 0 contract])
  - Acceptance criteria: returns deterministic conversation set and branch edges.
  - Test strategy: in-memory DB tests with multi-branch fixtures.

- [ ] Implement `mind_graph_integrity_report` (depends on: [mind_graph_scope])
  - Acceptance criteria: detects orphan parent refs, root mismatch, cycles.
  - Test strategy: adversarial lineage fixtures.

**Exit Criteria**: Graph data can be queried and validated from storage only.

**Delivers**: Reliable backend graph substrate.

---

### Phase 2: Runtime Projection + Overlay
**Goal**: Project storage graph into consumable runtime payload with artifact context.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Add `aoc-mind::graph_projection` for node/edge projection (depends on: [Phase 1 queries])
  - Acceptance criteria: emits canonical graph payload for tree scope.
  - Test strategy: projection snapshot tests with deterministic sorting.

- [ ] Add artifact/provenance overlay mode (depends on: [projection baseline])
  - Acceptance criteria: links observations/reflections/provenance to conversation nodes.
  - Test strategy: integration tests using seeded t1/t2 artifacts.

**Exit Criteria**: Runtime can produce complete graph payload for a session/root scope.

**Delivers**: Graph payload ready for CLI/UI consumers.

---

### Phase 3: Export Interfaces + Mission Control Adapter
**Goal**: Expose graph payload to operators and UI.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Implement `aoc-mind graph export` command (depends on: [Phase 2 projection])
  - Acceptance criteria: supports `json`, `mermaid`, and `summary` outputs.
  - Test strategy: command integration tests + golden output checks.

- [ ] Implement Mission Control graph adapter (depends on: [Phase 2 projection])
  - Acceptance criteria: adapter produces compact view-model for future graph pane.
  - Test strategy: adapter unit tests with branch/fallback scenarios.

**Exit Criteria**: Operators can export graph now; UI has stable adapter contract for future rendering.

**Delivers**: Immediate observability + low-risk path to visual graph pane.

---

### Phase 4: Documentation + Ops Guardrails
**Goal**: Make graph usage and maintenance operationally safe.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Author `docs/mind-graph.md` with schema examples and troubleshooting (depends on: [Phase 3])
- [ ] Add runbook checks for lineage integrity + export diagnostics (depends on: [Phase 3])

**Exit Criteria**: Engineers can reliably inspect, validate, and debug graph outputs.

**Delivers**: Production-ready graph operations guidance.

</implementation-roadmap>

---

<test-strategy>

## Test Pyramid

```text
        /\
       /E2E\       ← 10%
      /------\
     /Integration\ ← 35%
    /------------\
   /  Unit Tests  \ ← 55%
  /----------------\
```

## Coverage Requirements
- Line coverage: 85% minimum (new graph modules)
- Branch coverage: 80% minimum
- Function coverage: 90% minimum
- Statement coverage: 85% minimum

## Critical Test Scenarios

### Lineage Contract Validation
**Happy path**:
- Canonical nested `mind_lineage` metadata parses and stores.
- Expected: valid metadata with correct session/root/parent mapping.

**Edge cases**:
- Legacy event with no lineage metadata but `agent_id=session::pane`.
- Expected: session inferred, root=self, parent=None.

**Error cases**:
- Parent provided without root/session.
- Expected: typed validation failure, ingest rejected.

**Integration points**:
- Adapter -> storage insert path.
- Expected: invalid lineage never enters graph tables.

### Graph Scope Query + Projection
**Happy path**:
- Root + multiple branches + artifacts.
- Expected: deterministic node/edge ordering and correct edge cardinality.

**Edge cases**:
- Single-node root-only tree.
- Expected: one node, no branch edges, no errors.

**Error cases**:
- Orphan branch parent missing from lineage table.
- Expected: diagnostics emitted; graph still exported fail-open with degraded edge set.

**Integration points**:
- Storage scope query -> mind projection -> CLI export.
- Expected: output stable across repeated runs.

### Export Interfaces
**Happy path**:
- `json`, `mermaid`, `summary` outputs generated for same graph.
- Expected: format parity in node/edge counts.

**Edge cases**:
- Large tree (200+ nodes).
- Expected: export within latency target and bounded memory.

**Error cases**:
- Unknown format flag.
- Expected: explicit CLI validation error.

**Integration points**:
- Mission Control adapter consumes graph payload.
- Expected: adapter returns UI-ready model without graph mutation.

## Test Generation Guidelines
- Prefer deterministic fixture graphs with explicit root/parent maps.
- Snapshot test serialized JSON with sorted IDs to detect drift.
- Add adversarial lineage fixtures (orphans, cycles, duplicate parent definitions).
- Keep Mermaid output tests semantic (node/edge set) rather than whitespace-sensitive only.

</test-strategy>

---

<architecture>

## System Components
- **Contract layer (`aoc-core`)**: lineage + graph schema, validation, canonical serialization.
- **Persistence/query layer (`aoc-storage`)**: lineage table, scope queries, integrity diagnostics.
- **Projection layer (`aoc-mind`)**: graph payload construction + artifact overlays.
- **Interface layer (`aoc-cli`, `aoc-mission-control`)**: export and UI adapter surfaces.

## Data Models
- **MindGraphNode**
  - `node_id`, `node_type` (`conversation`, `artifact`), `conversation_id`, `session_id`, metadata map.
- **MindGraphEdge**
  - `edge_id`, `edge_type` (`branch_of`, `observed_by`, `reflected_into`, `task_linked`), `from`, `to`, metadata map.
- **MindGraphPayload**
  - `scope` (`session_id`, optional `root_conversation_id`), `nodes`, `edges`, `diagnostics`, `generated_at`.

## Technology Stack
- Rust workspace crates (`aoc-core`, `aoc-storage`, `aoc-mind`, `aoc-cli`, `aoc-mission-control`)
- SQLite lineage/provenance storage
- Serde JSON canonical output

**Decision: Canonical JSON graph payload as source of truth**
- **Rationale**: Enables stable API for CLI and UI consumers and straightforward snapshot testing.
- **Trade-offs**: Requires strict schema evolution discipline.
- **Alternatives considered**: Direct Mermaid-only generation (rejected: not machine-friendly).

**Decision: Fail-open export with diagnostics**
- **Rationale**: Operators should still see partial graphs even with bad lineage rows.
- **Trade-offs**: Consumers must handle diagnostics and partial edges.
- **Alternatives considered**: Hard-fail on any integrity issue (rejected: poor operational UX).

</architecture>

---

<risks>

## Technical Risks
**Risk**: Graph schema drift between core/storage/UI
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: Keep schema in `aoc-core` only; adapter consumes typed structs.
- **Fallback**: Version-gate payload parsing with compatibility checks.

**Risk**: Large graph payloads degrade performance
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: Default compact mode, pagination/windowing hooks, bounded overlays.
- **Fallback**: Summary mode export only for oversized scopes.

## Dependency Risks
- Mission Control graph pane implementation could lag while backend lands first.
- Mitigation: ship CLI export and adapter contract independently.

## Scope Risks
- Risk of over-expanding into full graph UI within same task.
- Mitigation: keep this PRD focused on graph foundation + export + adapter contract, not full rendering UX.

</risks>

---

<appendix>

## References
- `.taskmaster/templates/example_prd_rpg.txt`
- `.taskmaster/docs/prds/aoc-mind_prd.md`
- Task 108/131 implementation notes in AOC memory

## Glossary
- **Lineage**: session/root/parent relationship metadata for conversations.
- **Graph scope**: selected session/root tree boundary for projection.
- **Overlay**: attaching artifact/provenance/task context onto graph entities.

## Open Questions
- Should artifact overlays be separate nodes by default or edge annotations only?
- Should Mission Control receive full graph payload over Pulse, or a compact summary + on-demand fetch?
- Do we add historical branch timestamps to support time-sliced graph playback?

</appendix>
