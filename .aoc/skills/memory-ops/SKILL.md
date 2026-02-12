---
name: memory-ops
description: Use aoc-mem to read, search, and record project decisions.
---

## Commands
- `aoc-mem read`
- `aoc-mem search "<term>"`
- `aoc-mem add "<decision>"`
- `aoc-stm add "<handoff note>"`
- `aoc-stm edit`
- `aoc-stm archive`
- `aoc-stm` (read latest archive)
- `aoc-stm history`

## Recording guidelines
- Capture the "why" behind decisions.
- Keep entries short and scoped.
- Record one decision per line.

## Guardrail
- Never edit `.aoc/memory.md` directly.
- Keep `.aoc/stm/current.md` as an in-progress draft and archive it often so STM becomes a durable project diary.
