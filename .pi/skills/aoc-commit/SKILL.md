---
name: aoc-commit
description: Safe AOC commit workflow for coding agents. Use when preparing, writing, reviewing, or creating Git commits; when linking implementation work to Taskmaster tasks/PRDs, STM, tests, or Mind provenance; or when the user asks to commit changes, draft a commit message, inspect commit readiness, or explain commit history relevance.
---

## Goal
Turn Git commits into durable AOC engineering-intelligence checkpoints without replacing Git.

Use this skill to:
- Inspect commit readiness safely.
- Plan atomic commit groups.
- Draft readable, machine-linkable commit messages.
- Add AOC trailers that future Mind ingestion can parse.
- Commit only with explicit operator approval.

## Safety rules
- Never run `git add`, `git commit`, `git push`, `git reset`, `git clean`, `git checkout`, `git switch`, `git merge`, or `git rebase` unless the user explicitly asks or approves the exact action.
- Never push unless the user explicitly asks to push.
- Never commit secrets, tokens, raw private logs, or chain-of-thought.
- Do not include huge raw diffs in commit messages.
- Do not stage broad paths like `.` unless the user approves after seeing the planned file set.
- Prefer read-only commands until the user approves a commit plan.

## Quick workflow

### 1. Inspect read-only state
Use narrow Git summaries first:

```bash
git status --short
git diff --stat
git diff --cached --stat
git diff --name-only
git diff --cached --name-only
```

If a message requires more detail, inspect targeted diffs only:

```bash
git diff -- path/to/file
git diff --cached -- path/to/file
```

### 2. Resolve AOC context only as needed
Use only when it helps link the commit:

```bash
tm tag current
aoc-task show <id> --tag <tag>
aoc-task prd show <id> --tag <tag>
tm tag prd show
```

For focused prior context, use Mind only with an explicit reason:

```bash
aoc-mind-service context-pack --project-root "$PWD" --mode focused --reason "prepare AOC commit provenance" --json
```

### 3. Plan atomic commits
Group by intent, not by timestamp. Good commit groups are:
- one feature slice
- one bug fix
- one refactor
- one docs-only change
- one test-only change
- one generated/state update that belongs with its source change

Call out unrelated changes instead of hiding them.

### 4. Draft the message
Use the AOC commit message contract below.

### 5. Ask for approval
Before staging or committing, show:
- files to stage
- commit subject
- commit body/trailers
- tests run or not run
- risk level

Ask a direct approval question.

### 6. Commit only after approval
Prefer explicit paths:

```bash
git add path/one path/two
git commit -m "subject" -m "body and trailers"
```

If files are already staged and the user approves:

```bash
git commit -m "subject" -m "body and trailers"
```

After commit, report the SHA and concise summary.

## Message contract

### Subject

```text
<type>(<scope>): <imperative summary>
```

Preferred types:
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
feat(mind): add git commit provenance anchors
docs(commit): define AOC trailer contract
fix(init): preserve Mind launcher metadata during repair
test(mission-control): cover provenance toggle rendering
```

### Body
Explain durable intent, not private reasoning:

```text
Add a commit-level provenance contract so future Mind ingestion can link
Git history to tasks, PRDs, sessions, changed files, and validation evidence.

This makes implementation history queryable from task/file provenance flows
without changing core Git semantics.
```

### AOC trailers
Use Git-trailer-style lines at the bottom. Include known values only.

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
Tests: not run; docs/skill only
Risk: low; documentation-only workflow layer
```

## Recommended message template

```text
<type>(<scope>): <imperative summary>

<what changed and why, in 1-3 short paragraphs>

AOC-Task: <id>
AOC-Subtask: <id.n>
AOC-PRD: <path>
AOC-Intent: <short durable intent>
Tests: <commands run or not run reason>
Risk: <low|medium|high>; <reason>
```

## Good example

```text
feat(commit): add AOC commit workflow skill

Add an agent-facing commit workflow that turns Git commits into durable
engineering-intelligence checkpoints. The skill defines safe inspection,
atomic grouping, approval-gated commit execution, and machine-readable AOC
trailers for later Mind provenance ingestion.

AOC-Task: 193
AOC-Subtask: 193.1
AOC-PRD: .taskmaster/docs/prds/aoc_commit_history_intelligence_prd_rpg.md
AOC-Intent: establish first-layer commit intelligence without Mind schema changes
Tests: not run; skill/documentation only
Risk: low; no runtime behavior changed
```

## Anti-patterns
Avoid:

```text
fix stuff
```

```text
updates
```

```text
feat: huge mixed batch
```

Avoid trailers with fake or guessed references. If unknown, omit them.

## Mind integration posture
This first-layer workflow is designed to work before Mind schema changes. Treat trailers and structured messages as the compatibility contract for later ingestion.

Future Mind ingestion should be able to parse:
- commit SHA and parents
- subject/body
- AOC trailers
- author/timestamp
- changed files and diffstat
- linked tasks/PRDs/sessions/tests/Mind artifacts

## Final response format after committing

```text
Committed <short-sha>: <subject>

Files: <n> changed
Tests: <summary>
AOC: task <id>, subtask <id.n>, PRD <path>
```

If not committing, provide the proposed message and ask for approval.
