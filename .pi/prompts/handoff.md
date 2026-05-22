---
description: Create a directed AOC STM handoff packet for the next agent/session
argument-hint: "[handoff focus, e.g. focusing on the element refactor]"
---

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
