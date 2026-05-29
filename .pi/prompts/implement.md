---
description: Run the classic AOC implementation journey from current plan/spec/task to verified completion
argument-hint: "[task-id|spec-path|instructions]"
---

Run an AOC implementation journey for:

$ARGUMENTS

Use the classic strategy:

source → semantics → task → subtasks → implement → test → lexicon delta → mark complete

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

1.5. Lexicon preflight, only when useful
- Locate the AOC lexicon, preferring `.aoc/lexicon.md` if present, otherwise `AOC_LEXICON.md`.
- Use the lexicon only when the task touches AOC domain language, system concepts, workflows, commands, task lifecycle, memory behavior, agent coordination, specs, user-facing terminology, naming, or docs.
- For generic bug fixes, refactors, tests, formatting, dependency updates, or implementation details with no terminology impact, skip lexicon work.
- Keep lexicon access cheap: search for relevant terms first; read only matching entries or nearby sections. Read the whole lexicon only if it is small or the task is terminology/doc/spec focused.
- Identify relevant canonical terms before planning implementation.
- Map terms from the spec/task/conversation to canonical lexicon terms.
- If the current plan/spec uses terminology that conflicts with the lexicon:
  - Prefer the current user/spec/task as implementation source of truth.
  - Do not silently reinterpret requirements.
  - Record a lexicon conflict for the Lexicon Delta step.
- Use canonical terms in new code, task titles, subtasks, comments, docs, and final response when doing so does not create unnecessary churn.
- Do not perform broad renames merely to satisfy the lexicon unless the task explicitly requires naming cleanup.

2. Build or repair task breakdown
- Ensure the parent task has goal, scope, acceptance criteria, and test strategy.
- Create or edit relevant subtasks so each is independently implementable and verifiable.
- Omit steps already completed, but confirm completion before relying on them.
- Keep subtasks concrete. Avoid vague planning-only subtasks unless planning is the deliverable.
- Use canonical lexicon terms in task and subtask titles where appropriate.
- If terminology uncertainty affects task scope, capture it explicitly in the task or final lexicon proposal.

3. Implement
- Work one subtask at a time.
- Inspect narrow files before editing.
- Make minimal coherent changes.
- Avoid unrelated files and pre-existing dirty work.
- Prefer canonical AOC terms for new names, docs, comments, task labels, and user-facing language.
- Avoid broad renames unless required by the task.
- Record durable decisions with `aoc-mem add` when useful.
- Use `aoc-stm` only for deliberate directed handoff packets when another agent/session needs to continue incomplete work.


4. Verify
- Run targeted tests/checks for changed behavior first.
- Run broader tests only when required by the spec/test strategy or risk level.
- If tests fail, fix and rerun.
- If blocked, stop, report exact blocker, and do not mark complete.

4.5. Lexicon delta, non-blocking by default
After verification, decide whether this work changed or clarified AOC language.

Check for:
- New AOC-specific concepts.
- Existing concepts with clearer definitions.
- Synonyms, aliases, or confusing terms that should be avoided.
- Overloaded terms used for multiple meanings.
- Relationships between concepts, workflows, states, commands, or artifacts.
- Terms from the implementation that should become canonical.
- Existing lexicon entries that now conflict with implemented behavior.

If there is no meaningful terminology change, do not edit the lexicon.

If there is a high-confidence, low-risk lexicon update:
- Update `.aoc/lexicon.md` or `AOC_LEXICON.md`.
- If neither file exists, create `.aoc/lexicon.md` only when there is an actual grounded entry to add.
- Keep definitions concise.
- Add aliases to avoid where useful.
- Add relationships only when they clarify lifecycle, ownership, containment, or responsibility.

If the update is uncertain, broad, or potentially disruptive:
- Do not update the lexicon directly.
- Add a “Lexicon proposal” to the final response with:
  - proposed term
  - definition
  - reason
  - affected existing terms
  - recommendation

Do not let lexicon maintenance block task completion unless the task itself is about terminology, docs, specs, agent behavior, or AOC system semantics.

5. Mark complete
- Mark subtasks complete only after their verification passes.
- Mark the parent task complete only after all acceptance criteria pass.
- Do not mark complete if a terminology conflict changes the meaning of the acceptance criteria.
- Do not stage, commit, or push unless explicitly instructed.

6. Final response
Report concisely:
- task id / spec path
- what was already complete
- subtasks completed now
- files changed
- tests run/results
- lexicon skipped / used / updated / proposed updates
- risks/blockers
- recommended next step, usually `/commit` when user is satisfied

Safety:
- Do not overwrite user work.
- Do not hide unrelated changes.
- Do not load broad memory by default.
- Do not mark complete based only on intent; verify first.
- Do not mutate the lexicon for generic implementation details.
- Do not silently change canonical terminology when the meaning is uncertain.
- Treat the lexicon as authoritative for terminology, not requirements.

Authority order:
1. Explicit current user instruction.
2. Active spec / PRD / task acceptance criteria.
3. Current repo behavior.
4. Existing AOC lexicon.
5. Existing memory.
6. Agent inference.
