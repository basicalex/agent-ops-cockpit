---
name: aoc-stm
description: Create or resume directed AOC STM handoff packets with explicit purpose and focus.
---

## Handoff

Create a proper directed handoff for the current work.

Operator handoff focus:
$ARGUMENTS

Treat the text after `/handoff` as the operator's requested handoff focus. If it is empty, infer a concise continuation focus from the current conversation and repo state.

Goal: produce a sealed AOC STM packet that is useful for the next agent/session, not a generic progress dump.

Workflow:
1. Determine handoff shape.
   - Default purpose: `continue`.
   - Use `review` if the focus asks for review/audit.
   - Use `test` if the focus asks for validation/repro/QA.
   - Use `debug` if the focus asks about a failure, bug, or investigation.
   - Use `docs` if the focus asks for documentation/user-facing explanation.
   - Use `commit` if the focus asks for commit/release preparation.
   - Recipient defaults to `next-agent/session` unless the focus names a role.

2. Inspect only targeted handoff context.
   - Run `aoc-stm status` to detect stale/noisy current STM, but do not blindly seal old current draft.
   - Run narrow status/evidence commands as needed, usually `git status --short`, `git diff --stat`, active task/tag via `tm tag current` or `tm show <id>` if known.
   - Do not load broad memory. Use focused memory only if necessary and with a reason.

3. Write a concise purpose-matched handoff packet in Markdown.
   Include:
   - Direction: purpose, recipient, operator focus, task/spec if known
   - Current status: done/partial/blocked
   - Touched files/areas and why they matter
   - Changes made / relevant evidence
   - Validation commands and results
   - Open risks / coordination warnings
   - Next safe actions
   - Do-not-repeat notes

4. Seal it through STM without polluting or depending on stale current draft.
   - Write the packet to a temp file.
   - Run:
     `aoc-stm handoff --from-file "$tmp" --purpose "$purpose" --to "$recipient" --focus "$focus" [--task "$task_ref"]`
   - This creates a directed archive and prints the next-agent resume brief.

5. Final response to the operator:
   - archive path/name
   - purpose/recipient/focus
   - the ready-to-paste next-agent brief
   - any risk if repo state was too dirty/ambiguous to summarize confidently

Safety:
- STM is not a mailbox; explicitly return the printed brief/archive so the operator can pass it on.
- Do not clear or overwrite existing STM current draft for this slash command.
- Do not include secrets, raw logs, or huge diffs.
- Do not mark tasks complete unless the user explicitly asked for task completion and validation supports it.

## Resume

Resume from an AOC STM handoff. This prompt is intentionally named `/rresume` to avoid clashing with Pi's native `/resume` command.

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
