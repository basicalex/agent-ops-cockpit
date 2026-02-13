---
name: stm-ops
description: Capture and read short-term diary context with aoc-stm.
---

## When to use
- Context window is getting tight and you need to summarize current execution state.
- You need to load current draft STM context into the terminal transcript.

## Commands
- `aoc-stm add "<note>"`
- `aoc-stm edit`
- `aoc-stm` (default read current draft)
- `aoc-stm read-current`
- `aoc-stm archive`
- `aoc-stm read` (read latest archive)
- `aoc-stm history`
- `aoc-stm read <archive>`

## Handoff format (recommended)
- Objective and task/subtask IDs
- Done / in-progress / blocked
- Files touched and key command outcomes
- Open decisions + assumptions
- Next 3-5 concrete steps

## Guardrails
- Keep STM as a project diary: work in current draft via `aoc-stm`, then archive frequently with `aoc-stm archive`.
- Promote durable decisions to `aoc-mem add`, not STM.
- Do not edit `tasks.json` directly while preparing handoff state.
