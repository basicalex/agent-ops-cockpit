# Mind v2 Architecture Cutover Checklist

This note captures the concrete outputs for Taskmaster task **134**: finalize the Mind v2 architecture contracts, align pending work to the real phase model, and define the cutover acceptance gate.

## Source documents
- `.taskmaster/docs/prds/aoc_mind_memory_pipeline_prd_rpg.md`
- `.taskmaster/docs/prds/aoc-mind-v2_t3-project-canon_prd_rpg.md`
- `docs/insight-t3-alignment.md`

## Architecture contracts and invariants

### Layer model
- **T0** is the replayable substrate derived from Pi-native session/import/compaction data.
- **T1** is bounded session-scoped observation over T0 slices and checkpoint triggers.
- **T2** is bounded session-scoped synthesis/reflection over related T1 artifacts.
- **T3** is project-scoped canon/alignment over T2 plus memory, STM, Taskmaster, and project/session exports.
- **Context packs / handshake** are bounded renderings composed from memory + STM + T3 + latest session deltas.

### Source-of-truth rules
- **Pi-native session history** is authoritative for raw session substrate and bootstrap/backfill inputs.
- **Mind SQLite** is the canonical store for derived semantic/project memory and provenance links.
- **Session exports** (`t1.md`, `t2.md`, `manifest.json`) are the canonical sealed handoff of a finalized session slice.
- **T3 canon** is the canonical project-memory layer used by retrieval, handshake, and operator surfaces.

### Operational invariants
- Ingestion/checkpoint handling must be **idempotent**.
- Replay/rebuild paths must be **deterministic enough for recovery**.
- Output surfaces must remain **bounded**:
  - T1/T2 prose stays compact.
  - handshake/context-pack output stays token-bounded and deduped.
  - retrieval output stays budgeted by mode.
- Provenance must be **preserved structurally**, not by overstuffing prose.
- T3 canon must support **revision/supersede lifecycle**, not silent overwrite.
- Runtime failures must be **fail-open for developer workflow**, but visible to operators.
- Scope defaults remain **project/tag bounded** unless explicitly broadened.

## Pending-task phase model

### Phase 1 — substrate completion
- **155** Pi session importer / reconciler
- **151** first-class compaction-derived T0 slices
- follow-on structured evidence expansion under the same substrate model

### Phase 2 — runtime and retrieval completion
- **141** retrieval across session exports and project canon
- **110** finalize `aoc-insight` UX over Mind v2 retrieval facets
- **131** dev-tab Mind feed cutover aligned to Mind v2 pipeline
- detached runtime is now present for **T2 reflector** and **T3 backlog** workers via the shared insight detached-job substrate; remaining work is operator-surface polish, broader live validation, and explicit cutover confidence

### Phase 3 — provenance and traversal foundation
- **132** provenance/query foundation for cross-session and T3 visualization

### Phase 4 — hardening and rollout
- **142** hardening, migration, and rollout validation suite

### Phase 5 — advanced orchestration surfaces
- **129** PI specialist role interface with human-in-command dispatch

## Cutover acceptance gate

Mind v2 is ready for cutover when all of the following are true:

### A. Replayable substrate
- Pi-native session/import/compaction records can be replayed into Mind reliably.
- Compaction checkpoints can rebuild first-class T0 slices without context loss.
- Recovery paths do not depend only on ephemeral raw markers.

### B. Deterministic session sealing
- Session finalization drains remaining T1/T2 work predictably.
- Finalized sessions emit deterministic `t1.md`, `t2.md`, and `manifest.json` bundles.
- Finalization enqueues T3 backlog work idempotently.

### C. T3 canon correctness
- T3 backlog worker processes only eligible deltas.
- Canon updates preserve revision/supersede lifecycle.
- `project_mind.md` and `handshake.md` are reproducible and bounded.

### D. Bounded injection correctness
- startup / tag-switch / resume / handoff payloads remain bounded.
- duplicate payloads are hash-deduped.
- high context pressure suppresses or trims non-urgent injections.

### E. Retrieval correctness
- retrieval supports `session`, `project`, and `auto` scope planning.
- results are citation-first and traceable to canon/session sources.
- output budgets hold across brief / refs / snips / drilldown modes.
- local deterministic fallback remains authoritative.

### F. Observability and recovery
- operator surfaces expose T0/T1/T2/T3 health and backlog state.
- detached Mind workers (at least T2/T3) are visible as `owner_plane=Mind` jobs with recovery/fallback state in operator surfaces.
- compaction/checkpoint failures are visible and replayable.
- artifact drilldown can traverse handshake -> canon -> session evidence.

### G. Release safety
- migrations and replay/backfill paths are validated.
- no-context-loss regressions cover compaction, finalization, replay, and recovery.
- rollout guidance exists for operators and maintainers.

## Recommended execution order from current state
1. **134** — finalize architecture contracts, phase alignment, and acceptance gate.
2. **141** — complete retrieval across session/project scopes.
3. **132** — deliver provenance/query foundation needed for deep drilldown and visualization.
4. operator polish — keep Mission Control / project Mind aligned with real detached runtime state, including project-local search, activity bridge, and Mind-specific detached labels.
5. **142** — run hardening/migration/rollout validation suite, including live validation of detached T2/T3 workers and stale-lease recovery.
6. **131** and **110** — cut over dev-tab and finalize insight UX.
7. **129** — layer specialist role dispatch on top of the stabilized memory substrate.

## Immediate next implementation steps

### Near-term reality check
1. Treat detached **T2** and **T3** Mind workers as implemented substrate, not speculative architecture.
2. Keep docs and operator surfaces honest about the current boundary: project-scoped Mind for knowledge review, Mission Control Fleet for detached runtime supervision, and `pulse-pane` as the lightweight local surface.
3. Validate stale-lease, fallback, and cancel/recovery paths in live operator runbooks in addition to unit coverage.

### Next coding task: 141
1. Add retrieval scope planner for `session`, `project`, and `auto`.
2. Rank across session exports + T3 canon with citation-first output.
3. Enforce bounded output profiles for brief/refs/snips/drilldown.
4. Add golden tests for ranking, fallback, and budget behavior.

### Next platform task after 141: 132
1. Expose deterministic provenance/query payloads over artifacts, checkpoints, exports, canon revisions, files, and tasks.
2. Support drilldown from retrieval/canon entries into source session evidence.
3. Keep relational storage canonical; provide graph-like traversal without graph-first coupling.
