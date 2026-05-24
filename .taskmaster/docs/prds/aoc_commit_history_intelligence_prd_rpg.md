# AOC Commit History Intelligence — RPG PRD

## Problem Statement
AOC Mind already provides project intelligence, focused retrieval, and provenance, but Git commit history is not yet a first-class provenance source. Commits currently act as durable repository history for humans, while Mind mostly reasons over sessions, tasks, files, observations, reflections, context packs, and derived artifacts. This leaves a gap in the fullstack engineering intelligence loop: after code changes are made, the most durable record of intent, scope, validation, and file-level impact is not systematically linked back to tasks, PRDs, sessions, STM, or Mind artifacts.

Without commit intelligence, future agents and operators must reconstruct why a change happened by manually correlating git logs, diffs, task state, session logs, and PRDs. This is slow, error-prone, and weakens provenance/audit workflows.

## Target Users
- **AOC coding agents** that need a repeatable commit workflow and commit-message standard.
- **Human operators** who want readable, auditable project history.
- **Mind retrieval/provenance users** who ask why files changed, what implemented a task, or which validation evidence supports a feature.
- **Taskmaster/PRD maintainers** who need durable linkage from planning artifacts to implementation checkpoints.

## Success Metrics
- Agents can generate commit messages that include task/PRD/test/intention trailers without ad hoc prompting.
- A dedicated `/commit` Pi prompt exists and is discoverable in `.pi/prompts`.
- Future commit provenance ingestion can parse `/commit` message trailers without changing core Git semantics.
- Mind provenance can represent commits as source artifacts with edges to tasks, PRDs, sessions, files, tests, and Mind artifacts.
- `aoc insight provenance --task-id <id>` and file/task provenance exports can eventually include commit evidence.

---

## Capability Tree

### Capability: Commit Hygiene and Agent Workflow
Defines how agents inspect working tree state, group atomic changes, draft messages, ask for approval, and avoid unsafe commits.

#### Feature: Commit readiness inspection
- **Description**: Summarize staged/unstaged/untracked changes before commit creation.
- **Inputs**: `git status --short`, `git diff --stat`, `git diff --cached --stat`, optional active Taskmaster tag/task.
- **Outputs**: Concise commit readiness summary and candidate atomic groups.
- **Behavior**: Prefer narrow git summaries first; escalate to diffs only when needed to write an accurate message.

#### Feature: Atomic commit planning
- **Description**: Split mixed changes into coherent commit candidates.
- **Inputs**: Changed files, diffstat, task context, PRD context.
- **Outputs**: Proposed commit groups with rationale.
- **Behavior**: Group by intent, not by timestamp; avoid bundling unrelated features/fixes/docs.

#### Feature: Approval-gated commit execution
- **Description**: Ensure agents do not stage/commit/push unless explicitly authorized.
- **Inputs**: User approval, staged file list, commit message.
- **Outputs**: Commit SHA or safe no-op summary.
- **Behavior**: Ask before destructive or publishing actions; never push unless requested.

### Capability: AOC Commit Message Semantics
Defines the canonical message format for human readability and machine parsing.

#### Feature: Conventional commit subject
- **Description**: Use `<type>(<scope>): <imperative summary>` for fast scanning.
- **Inputs**: Change category and project scope.
- **Outputs**: One-line commit subject.
- **Behavior**: Prefer `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `perf`, `build`, `ci`.

#### Feature: Intent-rich body
- **Description**: Explain what changed, why, and any non-obvious design tradeoffs.
- **Inputs**: Diff summary, task/PRD context, session reasoning summary.
- **Outputs**: Short commit body suitable for future readers and Mind indexing.
- **Behavior**: Summarize durable intent and evidence; do not paste private chain-of-thought or secrets.

#### Feature: Machine-readable AOC trailers
- **Description**: Add parseable trailers linking commits to AOC systems.
- **Inputs**: Task IDs, PRD path, session ID, Mind artifact IDs, STM checkpoint, validation commands.
- **Outputs**: Git trailer block.
- **Behavior**: Include only known values; use minimal trailers when context is unavailable.

Recommended trailers:

```text
AOC-Task: <id>
AOC-Subtask: <id.n>
AOC-PRD: <path>
AOC-Intent: <short intent>
AOC-Session: <pi session id>
AOC-Mind: <artifact/provenance id>
AOC-STM: <handoff/checkpoint ref>
Tests: <commands run or not run reason>
Risk: low|medium|high; <reason>
```

### Capability: Mind Commit Provenance Model
Adds commit history as a first-class source in Mind provenance and retrieval.

#### Feature: Git commit source artifact
- **Description**: Represent each relevant commit as a durable Mind source artifact.
- **Inputs**: Commit SHA, parents, subject, body, trailers, author, timestamp, changed files, diffstat.
- **Outputs**: Mind artifact with source kind `git_commit`.
- **Behavior**: Store metadata and summary by default; avoid storing full diffs unless explicitly configured.

#### Feature: Provenance graph edges
- **Description**: Link commits to project intelligence objects.
- **Inputs**: Parsed trailers, changed files, active task metadata, session metadata.
- **Outputs**: Edges such as task→commit, PRD→commit, session→commit, commit→file, commit→test, commit→Mind artifact.
- **Behavior**: Prefer explicit trailer links; fall back to heuristic links only with provenance labels.

#### Feature: Retrieval and provenance query integration
- **Description**: Allow Mind context packs and provenance exports to cite commits.
- **Inputs**: Task/file/artifact query constraints.
- **Outputs**: Commit evidence in focused context packs and provenance graphs.
- **Behavior**: Keep commit evidence concise: subject, SHA, linked task, files, tests, and risk summary.

### Capability: `/commit` Prompt Workflow
Provides an operator-facing prompt workflow for safe, approval-gated, AOC-linked commits.

#### Feature: Commit plan
- **Description**: Show commit candidates from current working tree.
- **Inputs**: Git status/diffstat, optional task/spec context, validation evidence.
- **Outputs**: Candidate groups, excluded unrelated changes, and recommended commit message.
- **Behavior**: Read-only until explicit approval of exact files and exact message.

#### Feature: Commit creation guidance
- **Description**: Stage explicit approved paths and create the approved commit.
- **Inputs**: User-approved file list and commit message.
- **Outputs**: Commit SHA and summary.
- **Behavior**: No implicit push; no broad staging; approval required.

#### Feature: Future provenance ingestion
- **Description**: Future Mind/runtime support can ingest commit metadata from Git history.
- **Inputs**: SHA or range, parsed AOC trailers, changed files, tests, risk.
- **Outputs**: Mind provenance evidence and queryable commit links.
- **Behavior**: Idempotent by SHA.

---

## Repository Structure

```text
agent-ops-cockpit/
├── .pi/prompts/
│   └── commit.md                        # Agent-facing /commit workflow prompt
├── docs/
│   └── commit-intelligence.md           # Operator docs and message examples
├── crates/
│   └── aoc-mind/src/                    # Future commit source/provenance ingestion model
└── .taskmaster/docs/prds/
    └── aoc_commit_history_intelligence_prd_rpg.md
```

## Module Definitions

### Module: `/commit` prompt
- **Maps to capability**: Commit Hygiene and Agent Workflow; AOC Commit Message Semantics
- **Responsibility**: Always-available agent guidance for safe, structured, AOC-linked commits.
- **File structure**:
  ```text
  .pi/prompts/
  └── commit.md
  ```
- **Exports**:
  - Agent workflow steps
  - Commit message template
  - Trailer policy
  - Safety guardrails

### Module: `commit-message-contract`
- **Maps to capability**: AOC Commit Message Semantics
- **Responsibility**: Define stable human/machine-readable commit metadata.
- **File structure**:
  ```text
  docs/commit-intelligence.md
  ```
- **Exports**:
  - Conventional subject rules
  - AOC trailer schema
  - Examples and anti-patterns

### Module: `mind-git-provenance`
- **Maps to capability**: Mind Commit Provenance Model
- **Responsibility**: Store and query commit metadata as Mind provenance evidence.
- **File structure**:
  ```text
  crates/aoc-mind/src/
  ├── git_commit.rs
  ├── provenance_contracts.rs
  └── query.rs
  ```
- **Exports**:
  - Commit source artifact model
  - Trailer parser/normalizer
  - Idempotent commit ingestion
  - Provenance graph edges for tasks/files/PRDs/sessions/tests

---

## Dependency Chain

### Foundation Layer — Phase 0
No dependencies.

- **commit-message-contract**: Defines the canonical format and examples.
- **commit-prompt**: Can be implemented immediately using existing git, Taskmaster, STM, and Mind commands.

### Workflow Layer — Phase 1
Depends on: Foundation Layer.

- **/commit prompt**: Depends on commit-message-contract.
- **operator docs**: Depends on commit-message-contract.

### Provenance Layer — Phase 2
Depends on: Foundation Layer.

- **mind-git-provenance model**: Depends on commit-message-contract.
- **trailer parser**: Depends on commit-message-contract.
- **idempotent commit ingestion**: Depends on mind-git-provenance model.

### Integration Layer — Phase 3
Depends on: Workflow Layer and Provenance Layer.

- **insight provenance integration**: Depends on Mind commit provenance model and query plumbing.
- **context-pack commit citations**: Depends on query integration.

No circular dependencies: the prompt and message contract are foundation; future Mind ingestion builds on the contract; insight integration builds on ingestion.

---

## Implementation Phases

### Phase 0: Spec and Tasking
- **Goal**: Establish PRD, task, and subtasks.
- **Entry criteria**: User approves making commit history part of fullstack engineering intelligence.
- **Exit criteria**: PRD is tracked and linked to a Taskmaster task with subtasks.
- **Test strategy**: Task/PRD link can be shown with `aoc-task prd show <id> --tag <tag>`.

### Phase 1: Always-Present `/commit` Prompt
- **Goal**: Provide immediate agent guidance before implementing deeper tooling.
- **Entry criteria**: PRD and task exist.
- **Exit criteria**: `.pi/prompts/commit.md` documents workflow, message format, trailers, and guardrails.
- **Test strategy**: Manual review plus prompt discovery via `/commit` autocomplete; sample message conforms to contract.

### Phase 2: Commit Intelligence Documentation and Examples
- **Goal**: Make the convention understandable for humans and agents.
- **Entry criteria**: Prompt exists.
- **Exit criteria**: Documentation explains why commits matter, how to write them, and how trailers link to AOC systems.
- **Test strategy**: Documentation review for examples, anti-patterns, and privacy constraints.

### Phase 3: Future Commit Provenance Parsing
- **Goal**: Parse `/commit`-style AOC trailers from Git commits for future provenance ingestion.
- **Entry criteria**: Contract and prompt are stable.
- **Exit criteria**: Runtime can parse sample commit messages and normalize AOC trailers.
- **Test strategy**: Unit tests for trailer parsing, missing optional trailers, and malformed values.

### Phase 4: Mind Commit Provenance Ingestion
- **Goal**: Add commit metadata as Mind source artifacts and graph edges.
- **Entry criteria**: Message trailers and CLI metadata are stable.
- **Exit criteria**: Commit SHA/range ingestion is idempotent and queryable.
- **Test strategy**: Focused aoc-mind tests for trailer parsing, source artifact storage, duplicate SHA handling, and edge creation.

### Phase 5: Insight/Context-Pack Integration
- **Goal**: Surface commits in provenance and retrieval flows.
- **Entry criteria**: Mind ingestion exists.
- **Exit criteria**: `aoc insight provenance` and focused context packs can cite commit evidence for task/file queries.
- **Test strategy**: CLI tests and golden output tests for task/file provenance including commit nodes.

---

## Test Strategy

- **Prompt validation**: Ensure `/commit` prompt gives safe, concise, approval-gated instructions.
- **Contract validation**: Parse sample commit messages and trailers.
- **Mind unit tests**: Verify commit source artifacts, edge creation, and idempotent ingestion.
- **Insight tests**: Verify commit nodes appear in provenance exports only when relevant.
- **Security/privacy tests**: Verify commit workflows warn against secrets/private reasoning and avoid storing raw diffs by default.

## Risks and Mitigations

- **Risk**: Agents overcommit or commit unrelated changes.
  - **Mitigation**: `/commit` requires atomic planning and explicit user approval.
- **Risk**: Commit messages leak secrets or private reasoning.
  - **Mitigation**: Contract forbids secrets, raw logs, and chain-of-thought; Mind stores metadata/diffstat by default.
- **Risk**: Provenance graph becomes noisy.
  - **Mitigation**: Prefer explicit trailers; label heuristic edges; keep context-pack citations compact.
- **Risk**: CLI duplicates Git behavior unnecessarily.
  - **Mitigation**: Keep `/commit` as a thin intelligence/provenance workflow, not a Git replacement.

## Acceptance Criteria

- A Taskmaster task exists for AOC commit history intelligence and links to this PRD.
- Subtasks cover skill, docs, CLI planning/message generation, Mind ingestion, and insight integration.
- The first implementation slice can ship without waiting for Mind schema changes: create the `/commit` prompt and docs.
- Later implementation can make commits first-class Mind provenance artifacts without breaking existing Git workflows.
