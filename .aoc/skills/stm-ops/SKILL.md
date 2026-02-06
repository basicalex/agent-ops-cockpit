---
name: stm-ops
description: Capture, archive, and inject short-term handoff memory with aoc-stm.
---

## When to use
- Context window is getting tight and you need to continue in a fresh tab/session.
- You need a clean handoff artifact another agent can resume from.

## Commands
- `aoc-stm add "<note>"`
- `aoc-stm edit`
- `aoc-stm --last` (default)
- `aoc-stm history`
- `aoc-stm use <archive>`

## Handoff format (recommended)
- Objective and task/subtask IDs
- Done / in-progress / blocked
- Files touched and key command outcomes
- Open decisions + assumptions
- Next 3-5 concrete steps

## Guardrails
- Keep STM ephemeral in `.aoc/stm/current.md`; archive for reuse with `aoc-stm`.
- Promote durable decisions to `aoc-mem add`, not STM.
- Do not edit `tasks.json` directly while preparing handoff state.
