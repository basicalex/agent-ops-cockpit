---
description: Load a directed AOC STM handoff packet into the current agent context
argument-hint: "[archive-name|handoff focus]"
---

Resume from an AOC STM handoff.

Operator resume target or focus:
$ARGUMENTS

Goal: load the relevant sealed handoff packet into this session's working context safely, then summarize what it means before continuing.

Workflow:
1. Inspect STM state first:
   - Run `aoc-stm status`.
   - If the operator provided an exact archive name/path, run `aoc-stm resume <archive>`.
   - If no archive was provided, use latest only when `safe_to_resume_latest: yes`.
   - If `safe_to_resume_latest: no` or stale warnings appear, do not blindly trust latest. Explain the stale condition and ask the operator for the exact archive or permission to inspect the current draft.

2. Load the packet:
   - Run `aoc-stm resume <archive>` when an archive is known.
   - If using latest safely, run `aoc-stm resume`.
   - Treat the printed packet as handoff context, not as verified truth.

3. Extract the working brief:
   - Purpose
   - Recipient / intended role
   - Operator focus
   - Task/spec reference
   - Current status
   - Touched files/areas
   - Validation already run
   - Risks/warnings
   - Next safe actions

4. Verify before acting:
   - Check narrow repo/task state needed for the next action.
   - Do not redo completed work unless evidence conflicts.
   - Do not load broad memory unless needed for this resume, and state the reason.

Final response:
- State which archive was loaded, or why resume was blocked.
- Give a concise resume summary.
- List the immediate next safe action.

Safety:
- STM is not durable memory and not a mailbox.
- Prefer exact archive names over latest.
- Do not mark tasks complete, commit, or push unless explicitly asked and validated.
