---
name: aoc-lexicon
description: Run an AOC lexicon journey for terminology review, proposals, and governed updates.
---

Run an AOC lexicon journey for:

$ARGUMENTS

Goal: work on the project terminology plane without derailing implementation work. Use the lexicon as a governed semantic artifact, not a memory dump or broad documentation sink.

Lexicon location:
- Prefer `.aoc/lexicon.md`.
- Fall back to `AOC_LEXICON.md`.
- If neither exists and the operator asks to add/update/apply lexicon content, create `.aoc/lexicon.md`.
- If neither exists and the operator asks only to review/report/propose, report that no lexicon exists and include a starter proposal instead of creating a file.

Default mode:
- Read-only proposal/report mode unless the operator explicitly asks to add, update, edit, write, apply, normalize, or create lexicon entries.
- Phrases like “review”, “check”, “audit”, “report”, “propose”, “scan”, or “inspect” mean do not edit the lexicon.
- If edit intent is ambiguous, ask before editing.

Common scopes:
- “last N commits”: inspect `git log -n N --stat` and targeted commit diffs.
- “current changes” / “diff”: inspect `git status --short`, `git diff --stat`, and targeted diffs.
- “task <id>”: inspect the task via `tm`/`aoc-task` plus relevant spec if needed.
- “spec <path>”: read that spec and compare terminology to the lexicon.
- “full repo check”: run a bounded terminology scan using filenames, headings, command names, docs, prompts, specs, and targeted ripgrep results. Avoid reading huge/generated/vendor files.
- “add/update term ...”: edit the lexicon directly if the requested term is clear and grounded.

Workflow:

1. Resolve scope
- Parse operator instructions from `$ARGUMENTS`.
- Identify mode: report/propose/apply.
- Identify source scope: commits, current diff, task, spec, docs, prompts, full repo, or explicit terms.
- Use `.aoc/context.md` for project orientation only if needed.
- Use focused Mind context only if terminology history is required, and state the reason.

2. Load lexicon cheaply
- Locate the lexicon.
- Read the full lexicon only if it is small or the task is lexicon-focused.
- For large lexicons, search relevant terms first and read matching sections.
- Track existing canonical terms, accepted aliases, avoid terms, relationships, and evidence.

3. Gather terminology evidence
- Inspect only sources needed for the requested scope.
- Prefer high-signal sources:
  - `.omp/skills/aoc-*.md` workflows
  - `.omp/skills/*/SKILL.md`
  - `.taskmaster/docs/specs/*`
  - `docs/**/*.md`
  - `README.md`, `AOC.md`, `DESIGN.md` when relevant
  - command names under `bin/`
  - targeted source files touched by the scope
- For full repo checks, start with headings, filenames, exported command/help strings, prompt names, spec terms, and repeated capitalized/domain phrases before reading many files.
- Exclude `.git/`, `node_modules/`, generated outputs, caches, backups, logs, vendor unless explicitly relevant.
- Do not read `.aoc/memory.md`, `.aoc/stm/current.md`, or `.taskmaster/tasks/tasks.json` directly.

4. Classify findings
For each finding, classify as one of:
- Existing canonical term used correctly.
- Missing term worth adding.
- Existing term needing clearer definition.
- Alias worth accepting.
- Confusing/deprecated alias to avoid.
- Overloaded term with multiple meanings.
- Conflict between lexicon and source/spec/repo behavior.
- Relationship worth recording.
- Not lexicon-worthy implementation detail.

5. Decide proposal vs edit
Apply edits only when all are true:
- Operator explicitly requested edits, or the requested task is plainly “add/update this term”.
- Change is grounded by source evidence.
- Change is low-risk and narrow.
- Change does not rename established concepts or reinterpret requirements.

Propose instead when:
- Operator asked to review/check/report/propose.
- Term is ambiguous.
- Multiple canonical names are plausible.
- Existing lexicon conflicts with current source/spec/repo behavior.
- Change would affect many files, tasks, docs, prompts, or agent behavior.
- Evidence is inferred rather than explicit.

6. Edit rules, when applying
- Preserve existing lexicon structure.
- Keep definitions concise.
- Include Evidence for each new or materially changed entry.
- Prefer adding aliases/avoid notes over broad renames.
- Do not add generic programming terms.
- Do not add transient implementation notes, decisions, or history better suited for `aoc-mem add`.
- Do not perform broad repo renames unless separately requested.

7. Output
Report concisely with these sections:
- Scope checked.
- Sources inspected.
- Lexicon status: missing / used / updated / proposal-only.
- Findings grouped by classification.
- Applied changes, if any, with file path.
- Lexicon proposals, if any, with:
  - term
  - definition
  - aliases / avoid terms
  - relationships
  - evidence
  - recommendation
- Conflicts or open questions.
- Suggested next action.

Safety:
- Lexicon is authoritative for terminology, not requirements.
- Current user instruction and active spec/task beat lexicon when requirements conflict.
- Do not let lexicon work expand into unrelated implementation.
- Do not stage, commit, or push unless explicitly instructed.
