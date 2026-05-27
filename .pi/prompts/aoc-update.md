---
description: Implement AOC platform changes with install/init propagation proof
argument-hint: "[task-id|spec-path|instructions]"
---

Run an AOC platform update journey for:

$ARGUMENTS

Use this when developing AOC itself: CLI commands, seeded config, prompts, skills, extensions, layouts, services, docs that define platform behavior, install paths, init/scaffold behavior, or defaults that should reach future AOC projects.

Strategy:

source → task/spec → implement → propagation proof → verify → report

Workflow:

1. Resolve source of truth
- Identify the current user request, task, subtask, spec, or PRD.
- Inspect task/spec with `tm`/`aoc-task` when applicable.
- Use `.aoc/context.md` for orientation.
- Use focused Mind context only when needed and with an explicit reason.

2. Classify lifecycle surfaces
For every change, decide which surfaces are affected:
- Repo source artifact: `bin/**`, `crates/**`, `.pi/**`, `.aoc/**`, docs, scripts, service templates, etc.
- Install surface: what `./install.sh` copies, builds, caches, or validates.
- Init surface: what `aoc-init` seeds, refreshes, migrates, adopts-if-managed, or intentionally preserves.
- Runtime/global surface: `~/.local/bin`, `~/.config/aoc/**`, managed services, shell PATH, generated caches.
- Existing project stance: preserve, adopt-if-managed, safe migration, manual update, or not applicable.

3. Implement narrowly
- Make the minimal coherent source changes.
- If adding a local-only AOC repo command/prompt, explicitly prevent `./install.sh` and `aoc-init` from propagating it.
- If adding a distributed prompt/skill/extension/config, update both install and init paths.
- Do not overwrite unrelated dirty work.

4. Propagation proof, mandatory
Before reporting done, prove the lifecycle outcome:
- Source artifact exists and contains the expected change.
- `./install.sh` path is covered, or intentionally excludes it with a clear reason.
- `aoc-init` future-project behavior is covered, or intentionally excludes/preserves it with a clear reason.
- Current installed/global copy is synced if this session needs to use it.
- Existing projects have a clear update path or preservation stance.

Use targeted commands such as `rg`, `cmp`, `install -m`, a temp-project `aoc-init`, or a smoke script. Prefer small proof commands over full installs unless needed.

5. Verify behavior
- Run targeted checks/tests for the changed behavior.
- Run broader tests only when risk or acceptance criteria require it.
- If verification fails, fix and rerun before marking complete.

6. Final report
Report concisely:
- task/spec/source used
- files changed
- install propagation status
- init/scaffold propagation status
- existing-project stance
- tests/checks run and result
- risks/blockers
- recommended next step

Safety:
- Do not stage, commit, or push unless explicitly asked.
- Do not silently propagate repo-local-only commands/prompts.
- Do not claim future install/init coverage without checking the actual install/init code paths.
- If a lifecycle surface is N/A, say why.
