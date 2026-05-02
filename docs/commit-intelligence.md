# AOC Commit Intelligence

AOC treats commits as durable engineering-intelligence checkpoints. A good commit records not only what changed, but why it changed, how it was validated, and which AOC planning or Mind context it belongs to.

This first layer does not require Mind schema changes. It establishes a stable human and machine-readable convention that later Mind ingestion can consume.

## Why commits matter to AOC Mind

AOC already links project understanding through PRDs, Taskmaster tasks, sessions, STM, memory, artifacts, file links, and provenance graphs. Git commits are the durable implementation endpoint of that chain.

A commit can connect:

```text
PRD -> task/subtask -> agent session -> changed files -> tests -> commit -> future Mind provenance
```

This helps future operators and agents answer:

- Why did this file change?
- Which commits implemented task 193?
- What validation supported this behavior?
- Which PRD requirement led to this implementation?
- What changed since the last checkpoint?
- Which commits are relevant for a focused context pack?

## Current integration level

Implemented immediately:

- `.pi/prompts/commit.md` (`/commit`)
- Commit message and trailer contract
- Approval-gated agent workflow
- Human-readable docs and examples

Planned later under Taskmaster task `193`:

- Optional commit provenance ingestion support
- Mind commit source artifacts
- Provenance graph commit nodes/edges
- `aoc insight provenance` and context-pack commit citations

## `/commit` workflow

### 1. Inspect read-only state

Start with concise summaries:

```bash
git status --short
git diff --stat
git diff --cached --stat
git diff --name-only
git diff --cached --name-only
```

Use targeted diffs only when needed:

```bash
git diff -- path/to/file
git diff --cached -- path/to/file
```

### 2. Plan atomic groups

A commit should represent one coherent intent:

- one feature slice
- one bug fix
- one refactor
- one docs update
- one test update
- one generated/state update that belongs with its source change

Avoid mixing unrelated changes just because they happened in the same session.

### 3. Link AOC context

Use Taskmaster/PRD context when available:

```bash
tm tag current
aoc-task show <id> --tag <tag>
aoc-task prd show <id> --tag <tag>
tm tag prd show
```

Use Mind context only when it is needed and with an explicit reason:

```bash
aoc-mind-service context-pack --project-root "$PWD" --mode focused --reason "prepare AOC commit provenance" --json
```

### 4. Ask before committing

Agents should show:

- files to stage
- commit subject/body/trailers
- tests run or not run
- risk level

Then ask for approval before `git add`/`git commit`. Never push unless explicitly requested.

## Message format

Use a conventional subject:

```text
<type>(<scope>): <imperative summary>
```

Recommended types:

- `feat` — user-visible capability
- `fix` — bug fix
- `docs` — documentation only
- `test` — tests only
- `refactor` — behavior-preserving code change
- `chore` — maintenance/state/tooling
- `perf` — performance improvement
- `build` — build/dependency system
- `ci` — CI/release automation

Examples:

```text
feat(commit): add AOC commit prompt workflow
docs(commit): define AOC trailer contract
fix(mind): preserve focused context retrieval policy
test(mission-control): cover provenance drilldown toggle
```

## Body guidance

The body should explain durable project intent:

- What changed?
- Why did it change?
- What non-obvious tradeoff matters later?
- How was it validated?

Do not include:

- secrets or tokens
- raw private logs
- chain-of-thought
- huge diffs
- unverifiable claims

## AOC trailers

Use Git-trailer-style metadata at the bottom. Include only known values.

```text
AOC-Task: <id>
AOC-Subtask: <id.n>
AOC-PRD: <path>
AOC-Intent: <short durable intent>
AOC-Session: <pi/aoc session id>
AOC-Mind: <artifact/provenance id>
AOC-STM: <handoff/checkpoint ref>
Tests: <commands run or not run reason>
Risk: low|medium|high; <reason>
```

Minimal useful trailers:

```text
AOC-Task: 193
Tests: not run; docs/prompt only
Risk: low; documentation-only workflow layer
```

## Example: prompt/docs commit

```text
feat(commit): add AOC commit intelligence workflow

Add the first-layer `/commit` prompt workflow so agents can prepare atomic,
approval-gated commits with structured AOC trailers. This establishes the
message contract that future Mind ingestion can parse without requiring an
immediate Mind schema refactor.

AOC-Task: 193
AOC-Subtask: 193.1
AOC-PRD: .taskmaster/docs/prds/aoc_commit_history_intelligence_prd_rpg.md
AOC-Intent: establish commit history as durable AOC engineering intelligence
Tests: not run; documentation and prompt only
Risk: low; no runtime behavior changed
```

## Example: future Mind integration commit

```text
feat(mind): ingest git commits as provenance artifacts

Add idempotent SHA-based commit ingestion so Mind can represent Git history as
source artifacts. Parsed AOC trailers create explicit links to tasks, PRDs,
sessions, files, tests, and Mind artifacts while preserving concise metadata by
default.

AOC-Task: 193
AOC-Subtask: 193.4
AOC-PRD: .taskmaster/docs/prds/aoc_commit_history_intelligence_prd_rpg.md
AOC-Intent: make commit history queryable from Mind provenance
Tests: cargo test -p aoc-mind
Risk: medium; adds provenance storage/query behavior
```

## Anti-patterns

Bad:

```text
fix stuff
```

Bad:

```text
updates
```

Bad:

```text
feat: everything from today
```

Bad:

```text
AOC-Task: maybe 193?
```

If a reference is unknown, omit it rather than guessing.

## Future Mind model

The later Mind integration should treat commits as source artifacts with concise metadata:

```text
commit_sha
parent_shas
author
timestamp
subject
body
trailers
changed_files
diffstat
linked_task_ids
linked_prd_paths
linked_session_ids
linked_mind_artifact_ids
validation_evidence
risk
```

The provenance graph can then include commit evidence for task/file/artifact queries without replacing existing artifact, task, file, canon, or semantic provenance nodes.
