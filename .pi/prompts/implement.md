---
description: Run the classic AOC implementation journey from current plan/spec/task to verified completion
argument-hint: "[task-id|spec-path|instructions]"
---

Run an AOC implementation journey for:

$ARGUMENTS

Use the classic strategy:

spec → task → subtasks → implement → test → mark complete

Fit the current actual discussed plan and project state. If some phases are already done, verify them briefly and continue from the right point. Do not redo completed work unnecessarily. If the user has just discussed a plan, treat that conversation context as authoritative unless repo/task/spec evidence conflicts.

Workflow:

1. Resolve source of truth
- Identify the active spec, PRD, task id, or current plan from input/conversation.
- If a spec/PRD path is provided, read it.
- If a task id is provided, inspect that task via `tm`/`aoc-task`.
- If both exist, align task/subtasks with the spec.
- If no task exists and implementation needs one, create it via Taskmaster.
- Use `.aoc/context.md` for orientation.
- Use focused Mind context only when needed, with explicit reason.

2. Build or repair task breakdown
- Ensure the parent task has goal, scope, acceptance criteria, and test strategy.
- Create or edit relevant subtasks so each is independently implementable and verifiable.
- Omit steps already completed, but confirm completion before relying on them.
- Keep subtasks concrete. Avoid vague planning-only subtasks unless planning is the deliverable.

3. Implement
- Work one subtask at a time.
- Inspect narrow files before editing.
- Make minimal coherent changes.
- Avoid unrelated files and pre-existing dirty work.
- Record durable decisions with `aoc-stm add` when useful.

4. Verify
- Run targeted tests/checks for changed behavior first.
- Run broader tests only when required by the spec/test strategy or risk level.
- If tests fail, fix and rerun.
- If blocked, stop, report exact blocker, and do not mark complete.

5. Mark complete
- Mark subtasks complete only after their verification passes.
- Mark the parent task complete only after all acceptance criteria pass.
- Do not stage, commit, or push unless explicitly instructed.

6. Final response
Report concisely:
- task id / spec path
- what was already complete
- subtasks completed now
- files changed
- tests run/results
- risks/blockers
- recommended next step, usually `/commit` when user is satisfied

Safety:
- Do not overwrite user work.
- Do not hide unrelated changes.
- Do not load broad memory by default.
- Do not mark complete based only on intent; verify first.
