# AOC Intel: Agent-Native Project Navigation and Mind-Backed Relevance PRD

<overview>

## Problem Statement
Agents and humans currently navigate AOC projects through generic tools (`rg`, `find`, `git`, `read`) plus scattered AOC context sources (`.aoc/context.md`, Taskmaster, STM, memory, Mind). This works, but it is inefficient for AOC v2-scale workflows: agents often spend extra turns discovering relevant files, may miss paired tests/docs, and cannot easily explain why a file is likely relevant to a task. UI fuzzy finders such as `fff.nvim` improve human file picking, but they do not provide the ranked, JSON, provenance-backed answers that coding agents need.

## Target Users
- **Coding agents**: need bounded, explainable candidate files, symbols, tests, and prior context before reading/editing.
- **AOC operators**: need fast task-aware navigation and auditability when supervising agents.
- **Human developers**: may consume the same project-intelligence results through CLI, Zellij panes, Neovim/fff-style frontends, or future AOC map views.

## Success Metrics
- Agents can identify the top relevant source/test/doc files for a task in one command for common AOC feature work.
- `aoc-intel` results include machine-readable scores and reason trails for every candidate.
- Mind integration is focused and reason-bound; no broad memory injection is required for normal startup.
- Initial implementation remains editor-agnostic while allowing optional editor/front-end integration later.
- The feature reduces repeated broad `rg`/`find` exploration during agent work without replacing deterministic local inspection.

</overview>

---

<functional-decomposition>

## Capability Tree

### Capability: Query-Based Project Retrieval
Find likely relevant project files for a natural-language or token query.

#### Feature: Ranked file search
- **Description**: Return relevant source, test, doc, config, and task files for a query.
- **Inputs**: Query string, project root, optional active task/tag, optional output format.
- **Outputs**: Ranked candidates with path, score, kind, reasons, and suggested next action.
- **Behavior**: Combine path fuzzy matching, lexical content search, git signals, ignore rules, and optional Mind/task context.

#### Feature: JSON-first agent output
- **Description**: Provide stable structured output for coding agents and wrappers.
- **Inputs**: Retrieval result set.
- **Outputs**: JSON schema with candidates, scoring components, provenance, and warnings.
- **Behavior**: Emit concise, parseable results by default when `--json` is used; avoid raw noisy logs.

### Capability: Task-Aware Navigation
Use Taskmaster and PRD context to identify implementation entry points.

#### Feature: Task retrieval
- **Description**: Retrieve files and constraints relevant to a Taskmaster task.
- **Inputs**: Task id, tag, task fields, linked PRD, subtasks, optional STM/Mind context.
- **Outputs**: Ranked candidate files, likely tests, relevant docs, constraints, and suggested commands.
- **Behavior**: Resolve active tag, task text, PRD override/default, and focused project intelligence.

#### Feature: Constraint surfacing
- **Description**: Surface project decisions or prior context that constrain implementation.
- **Inputs**: Task/query, Mind focused context, memory references, PRD sections.
- **Outputs**: Short list of constraints with provenance.
- **Behavior**: Ask Mind only with explicit reason and focused mode; never inject broad memory by default.

### Capability: File Relationship Discovery
Given a file, identify what else should be inspected or tested.

#### Feature: Related files
- **Description**: Find tests, docs, callers, callees, imports, config, and co-changed files related to a path.
- **Inputs**: File path, project root, optional relation types.
- **Outputs**: Ranked related files with relation reasons.
- **Behavior**: Combine import graph, filename/test pairing, git co-change, lexical mentions, and Mind summaries.

#### Feature: File explanation
- **Description**: Explain what a file appears to do and why it matters.
- **Inputs**: File path and optional task/query context.
- **Outputs**: Purpose summary, key symbols, relationships, constraints, and freshness metadata.
- **Behavior**: Prefer deterministic local metadata and cached summaries; use Mind for persisted project intelligence when available.

### Capability: Symbol and Structural Navigation
Move beyond text search to code-aware lookup.

#### Feature: Symbol lookup
- **Description**: Find definitions and candidate references for names, functions, structs, commands, or components.
- **Inputs**: Symbol query, language hints, project root.
- **Outputs**: Symbol candidates with file, line, kind, and confidence.
- **Behavior**: Start with `rg`/ctags-compatible indexing; later support tree-sitter/LSP where practical.

#### Feature: Structural search integration
- **Description**: Support AST-aware search for calls, imports, functions, and declarations.
- **Inputs**: Pattern or high-level structural query.
- **Outputs**: Matches with path, span, language, and reason.
- **Behavior**: Integrate `ast-grep`/tree-sitter as optional providers with graceful fallback.

### Capability: Mind-Backed Project Intelligence
Use AOC Mind as a focused, persistent intelligence layer.

#### Feature: Focused Mind enrichment
- **Description**: Enrich rankings and explanations with prior decisions, summaries, and provenance.
- **Inputs**: Query/task/file plus explicit reason.
- **Outputs**: Mind-derived signals and citations attached to candidates.
- **Behavior**: Follow AOC policy: metadata-only startup, lazy focused retrieval, provenance-aware context.

#### Feature: Index freshness and provenance
- **Description**: Track whether results came from current files, git state, task data, or Mind cache.
- **Inputs**: Source metadata and timestamps.
- **Outputs**: Freshness indicators and provenance records.
- **Behavior**: Warn on stale indexes or unavailable providers while failing open to deterministic local search.

### Capability: Human Frontends
Expose the same intelligence through optional UI integrations.

#### Feature: CLI human mode
- **Description**: Pretty-print ranked results for terminal use.
- **Inputs**: Same as agent queries.
- **Outputs**: Concise tables with reasons.
- **Behavior**: Keep output small and actionable.

#### Feature: Optional editor/TUI frontends
- **Description**: Allow tools such as `fff.nvim`, Zellij panes, or AOC map views to consume `aoc-intel` results.
- **Inputs**: `aoc-intel --json` result streams.
- **Outputs**: Interactive picker/navigation UI.
- **Behavior**: Treat UI integrations as clients, not as the source of intelligence.

</functional-decomposition>

---

<structural-decomposition>

## Repository Structure

```text
agent-ops-cockpit/
├── bin/
│   └── aoc-intel                         # Thin executable wrapper
├── crates/
│   ├── aoc-cli/                          # Command registration if exposed through `aoc-cli`
│   ├── aoc-core/                         # Shared project/git/task helpers where appropriate
│   ├── aoc-mind/                         # Existing Mind service/client integration points
│   └── aoc-intel/                        # New library/binary crate for retrieval and ranking
│       ├── src/
│       │   ├── cli.rs                    # CLI parsing and output modes
│       │   ├── model.rs                  # Result schemas and scoring model
│       │   ├── providers/                # Retrieval providers
│       │   │   ├── git.rs
│       │   │   ├── lexical.rs
│       │   │   ├── taskmaster.rs
│       │   │   ├── mind.rs
│       │   │   ├── symbols.rs
│       │   │   └── structural.rs
│       │   ├── rank.rs                   # Score aggregation and explanation assembly
│       │   ├── related.rs                # File relationship logic
│       │   └── schema.rs                 # Stable JSON schema/versioning
│       └── tests/
├── docs/
│   └── aoc-intel.md                      # User and agent workflow documentation
└── .taskmaster/docs/prds/
    └── aoc_intel_project_navigation_prd_rpg.md
```

## Module Definitions

### Module: `aoc-intel::cli`
- **Maps to capability**: CLI access for agents and humans.
- **Responsibility**: Parse commands, resolve project root/tag/task arguments, and format output.
- **Exports**:
  - `run()` - Execute CLI command.
  - `OutputMode` - Human or JSON output mode.

### Module: `aoc-intel::model`
- **Maps to capability**: JSON-first agent output.
- **Responsibility**: Define candidate, reason, score, provenance, and warning types.
- **Exports**:
  - `IntelResultSet`
  - `Candidate`
  - `Reason`
  - `Provenance`

### Module: `aoc-intel::providers`
- **Maps to capability**: Retrieval provider orchestration.
- **Responsibility**: Gather candidates and raw signals from deterministic and intelligence-backed sources.
- **Exports**:
  - `Provider` trait
  - `ProviderSignal`
  - provider implementations for git, lexical, Taskmaster, Mind, symbols, structural search.

### Module: `aoc-intel::rank`
- **Maps to capability**: Ranking and explainability.
- **Responsibility**: Merge provider signals into final candidate scores and reason trails.
- **Exports**:
  - `rank_candidates()`
  - `ScoreBreakdown`

### Module: `aoc-intel::related`
- **Maps to capability**: File relationship discovery.
- **Responsibility**: Identify related tests/docs/imports/co-changes for a given file.
- **Exports**:
  - `find_related(path, options)`

### Module: `aoc-intel::mind`
- **Maps to capability**: Mind-backed project intelligence.
- **Responsibility**: Request focused Mind context with explicit reasons and attach provenance.
- **Exports**:
  - `focused_context(reason, query_scope)`
  - `mind_signals_to_reasons()`

</structural-decomposition>

---

<dependency-graph>

## Functional Dependency Graph

```text
Query-Based Project Retrieval
├── depends on: local file inventory, lexical search, rank model
Task-Aware Navigation
├── depends on: Query-Based Project Retrieval, Taskmaster/PRD resolution
File Relationship Discovery
├── depends on: Query-Based Project Retrieval, git/import/test pairing signals
Symbol and Structural Navigation
├── depends on: provider abstraction, optional index providers
Mind-Backed Project Intelligence
├── depends on: Mind service/client availability and AOC focused retrieval policy
Human Frontends
└── depends on: stable JSON schema and CLI output
```

## Structural Dependency Graph

```text
model/schema
└── no internal dependencies

providers/git, providers/lexical
└── depend on model/schema and project root helpers

providers/taskmaster
└── depends on model/schema and Taskmaster/AOC task helpers

providers/mind
└── depends on model/schema and existing aoc-mind service/client contract

providers/symbols, providers/structural
└── depend on model/schema and optional external/index providers

rank
└── depends on model/schema and provider signals

related
└── depends on providers/git, providers/lexical, optional symbols/structural, rank

cli
└── depends on providers, rank, related, schema, output formatting
```

## Build Order
1. Define schemas and CLI contract.
2. Implement deterministic local providers: git inventory, path matching, lexical search.
3. Implement rank aggregation and reason trails.
4. Add Taskmaster/PRD context provider.
5. Add related-file and changed-file flows.
6. Add focused Mind enrichment with explicit reason strings.
7. Add optional symbols/structural providers.
8. Add docs, tests, and optional frontend integration guidance.

</dependency-graph>

---

<implementation-plan>

## Phase 0: Design Freeze for AOC v2
- **Goal**: Keep this as a planned AOC v2 feature; do not implement in current scope.
- **Entry Criteria**: PRD linked to a Taskmaster task.
- **Exit Criteria**: Task and subtasks exist; spec captures MVP, architecture, and guardrails.
- **Tests**: Review-only; no code changes required.

## Phase 1: CLI and Schema MVP
- **Goal**: Define `aoc-intel` command set and stable JSON result schema.
- **Commands**:
  - `aoc-intel find <query> [--json]`
  - `aoc-intel task <id> [--tag <tag>] [--json]`
  - `aoc-intel related <path> [--json]`
  - `aoc-intel explain <path> [--json]`
  - `aoc-intel changed [--related] [--json]`
- **Exit Criteria**: Schema examples and CLI help are documented before implementation begins.

## Phase 2: Deterministic Retrieval MVP
- **Goal**: Provide useful ranked results without Mind or embeddings.
- **Providers**: `git ls-files`, path/fuzzy scoring, ripgrep lexical search, git status/diff, ignore/noise filters.
- **Exit Criteria**: Agents receive top files with reasons and no dependency on editor UI.

## Phase 3: Task and PRD Awareness
- **Goal**: Let agents ask for files relevant to a Taskmaster task.
- **Providers**: Active tag, task fields, subtasks, tag/task PRDs, STM references where appropriate.
- **Exit Criteria**: `aoc-intel task <id> --json` returns likely implementation files, test files, docs, and constraints.

## Phase 4: Mind Enrichment
- **Goal**: Integrate AOC Mind as a focused project-intelligence provider.
- **Rules**:
  - No broad memory injection.
  - Every Mind request includes explicit reason.
  - Mind-derived reasons include provenance/freshness.
  - Unavailable Mind fails open to deterministic providers.
- **Exit Criteria**: Rankings can include prior decisions, summaries, and related-work hints with provenance.

## Phase 5: Relationship, Symbol, and Structural Providers
- **Goal**: Improve precision for larger repos.
- **Providers**: test/source pairing, import graph, git co-change, ctags/LSP-compatible symbol lookup, optional `ast-grep`/tree-sitter structural search.
- **Exit Criteria**: `related`, `symbols`, and structural lookup flows work with graceful fallbacks.

## Phase 6: Human Frontends and Documentation
- **Goal**: Document and optionally expose results in AOC UI surfaces.
- **Frontends**: CLI table output, Zellij pane ideas, AOC map integration, optional Neovim/fff.nvim consumer.
- **Exit Criteria**: Agents remain CLI-first; UI integrations consume the same JSON contract.

</implementation-plan>

---

<testing-strategy>

## Unit Tests
- Score aggregation from provider signals.
- JSON schema serialization/deserialization.
- Noise filtering and generated/vendor path penalties.
- Task/PRD input normalization.
- Mind unavailable/fail-open behavior.

## Integration Tests
- Fixture repos with known files and expected top candidates.
- `find`, `task`, `related`, `explain`, and `changed` command flows.
- Active tag and task PRD resolution.
- Focused Mind context invocation uses explicit reason and preserves provenance.

## Agent Workflow Tests
- Simulated agent asks `aoc-intel task <id> --json`, reads top candidates, and identifies expected implementation/test files.
- Verify output is concise enough for low-token workflows.
- Verify no command requires an interactive editor or UI.

## Regression Tests
- Stale/missing Mind service does not break deterministic retrieval.
- Missing optional providers (`ast-grep`, ctags, LSP) produce warnings, not failures.
- Generated, lockfile, vendor, and build-output noise is downranked.

</testing-strategy>

---

<risks-and-constraints>

## Risks
- Ranking may overfit to lexical/path matches and miss semantic relevance.
- Mind context may become stale or too broad if not reason-bound.
- Optional provider setup can become complex across languages.
- Human UI integrations could distract from the agent-first CLI contract.

## Mitigations
- Require reason trails and score breakdowns for auditability.
- Keep deterministic local search as the foundation.
- Treat Mind as enrichment, not the sole retrieval mechanism.
- Keep optional providers fail-open.
- Design JSON schema before frontends.

## Non-Goals for MVP
- No autonomous code editing.
- No replacing `rg`, `read`, or deterministic inspection.
- No mandatory Neovim/fff.nvim dependency.
- No broad startup memory injection.
- No embeddings-first retrieval; semantic retrieval is a later enhancement.

</risks-and-constraints>

---

<acceptance-criteria>

## AOC v2 Feature Acceptance Criteria
- A Taskmaster task exists and links this PRD as its task-level PRD.
- Subtasks describe design, MVP, task awareness, Mind integration, structural providers, tests, and docs.
- Future implementation starts with schema/CLI design, not editor integration.
- Agent-facing output is JSON-first, ranked, concise, and provenance-aware.
- Mind integration follows the AOC startup and focused retrieval policy.

</acceptance-criteria>
