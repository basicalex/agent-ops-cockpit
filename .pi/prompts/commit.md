---
description: Run the automated AOC commit workflow after implementation/polish
argument-hint: "[instructions]"
---

Run the AOC commit workflow for:

$ARGUMENTS

Use this after implementation and any polish passes. The goal is a clean, atomic, provenance-rich Git commit. The user's `/commit` invocation is approval to run the full commit flow directly: inspect, select a safe atomic file set, stage exact paths, commit, and report the result. Never push unless explicitly requested.

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

5. Validate and commit directly
- Run targeted validation appropriate to the selected files when practical.
- Stage only explicit approved-by-workflow paths with `git add -- path ...`; never stage broad paths like `.`.
- Commit with the drafted provenance-rich message.
- If there are mixed/unrelated changes and no safe atomic file set can be inferred, stop and ask a concise clarification instead of committing.
- Never push unless explicitly requested.

Final response after commit:
- commit SHA
- subject
- files committed
- tests noted
- remaining unrelated changes, if any

Safety:
- Treat `/commit` as approval to stage and commit the safe atomic file set inferred by this workflow.
- Never commit secrets/tokens/private logs.
- Never stage broad paths or unrelated/pre-existing changes.
- If the atomic set or message is ambiguous, ask before staging/committing.
- Never push without explicit push approval.
- Do not include raw chain-of-thought or huge diffs in commit messages.
