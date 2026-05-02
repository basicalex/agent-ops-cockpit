---
description: Run the safe AOC commit workflow after implementation/polish
argument-hint: "[instructions]"
---

Run the AOC commit workflow for:

$ARGUMENTS

Use this after implementation and any polish passes. The goal is a clean, atomic, provenance-rich Git commit. Do not stage, commit, or push until the user explicitly approves the exact plan.

Workflow:

1. Inspect read-only state
- Run narrow Git summaries:
  - `git status --short`
  - `git diff --stat`
  - `git diff --cached --stat`
- Inspect targeted diffs for candidate files only.
- Identify unrelated/pre-existing changes and exclude them from the plan.

2. Resolve AOC provenance
- Identify relevant task/subtask/spec/PRD from recent implementation context or Taskmaster.
- Use `tm`/`aoc-task` as needed.
- Use STM/Mind only if needed for focused provenance, with explicit reason.

3. Plan atomic commit(s)
- Group by intent, not by timestamp.
- Prefer one commit for one coherent implementation slice.
- If changes are mixed, propose multiple commits.
- Never stage broad paths like `.`.

4. Draft commit message
Use:

`<type>(<scope>): <imperative summary>`

Include concise body plus AOC trailers when known:

AOC-Task: <id>
AOC-Subtask: <id.n>
AOC-PRD: <path>
AOC-Intent: <durable intent>
Tests: <commands run/results>
Risk: low|medium|high; <reason>

5. Ask for approval
Before staging/committing, show:
- exact files to stage
- commit subject/body/trailers
- tests run
- excluded unrelated files
- risk level

Ask a direct approval question.

6. Commit only after approval
- Stage only approved explicit paths.
- Commit only approved message.
- Never push unless explicitly requested.

Final response after commit:
- commit SHA
- subject
- files committed
- tests noted
- remaining unrelated changes, if any

Safety:
- Never commit secrets/tokens/private logs.
- Never stage/commit/push without explicit approval of the exact file set and exact commit message.
- Do not include raw chain-of-thought or huge diffs in commit messages.
