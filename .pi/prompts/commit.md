---
description: Run the automated AOC commit workflow after implementation/polish
argument-hint: "[instructions]"
---

Run the AOC commit workflow for:

$ARGUMENTS

Use this after implementation and any polish passes. The goal is a clean, atomic, provenance-rich commit using the repository's detected VCS. The user's `/commit` invocation is approval to run the full VCS-aware commit flow directly: inspect, select a safe atomic change set, commit with the detected workflow, and report the result. Never push unless explicitly requested.

Workflow:

1. Detect VCS mode, then inspect read-only state
- Prefer startup context VCS metadata. If it is unavailable or stale, run `aoc-handshake --json`.
- For Jujutsu repositories, run narrow summaries:
  - `jj status`
  - `jj diff --summary`
  - `jj diff --stat`
  - targeted `jj diff -- <filesets>` when needed
- For Git-only repositories, run narrow summaries:
  - `git status --short`
  - `git diff --stat`
  - `git diff --cached --stat`
  - targeted diffs for candidate files only
- Identify unrelated/pre-existing changes and exclude or split them before committing.

2. Resolve AOC provenance
- Identify relevant task/subtask/spec/PRD from recent implementation context or Taskmaster.
- Use `tm`/`aoc-task` as needed.
- Use STM/Mind only if needed for focused provenance, with explicit reason.

3. Plan atomic commit(s)
- Group by intent, not by timestamp.
- Prefer one commit for one coherent implementation slice.
- Git uses explicit staging; never stage broad paths like `.`.
- Jujutsu has no Git staging area: the working copy is the current mutable `@` change, and `jj commit` without filesets selects all current changes.
- If Jujutsu `@` is mixed, split unrelated work first with `jj split` / `jj commit <filesets>` / `jj squash -i` as appropriate; if the intended split is unclear, ask one concise clarification before mutating.

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
- Git-only: stage only explicit approved-by-workflow paths with `git add -- path ...`; never stage broad paths like `.`.
- Jujutsu: verify `@` contains only the intended atomic work, then use `jj commit -m <message>` or `jj describe -m <message>` plus the workflow-appropriate new-change step.
- Jujutsu selected filesets: when the intended fileset is clear but `@` is mixed, use `jj commit -m <message> <filesets>` or `jj split <filesets>` according to the desired split direction.
- If no safe atomic set can be inferred, ask one concise clarification before staging or mutating.
- Never push unless explicitly requested.

Final response after commit:
- Git commit SHA or Jujutsu change/commit identity
- subject
- files committed
- tests noted
- remaining unrelated changes, if any

Safety:
- Treat `/commit` as approval to commit only the safe atomic change set inferred by this workflow.
- Never commit secrets/tokens/private logs.
- Never stage broad Git paths or include unrelated/pre-existing changes.
- If the atomic set or message is ambiguous, ask before staging/committing.
- Never push without explicit push approval.
- Do not include raw chain-of-thought or huge diffs in commit messages.
