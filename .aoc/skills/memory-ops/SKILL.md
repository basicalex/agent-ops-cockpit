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
- `aoc-stm --last`
- `aoc-stm history`

## Recording guidelines
- Capture the "why" behind decisions.
- Keep entries short and scoped.
- Record one decision per line.

## Guardrail
- Never edit `.aoc/memory.md` directly.
- Keep `.aoc/stm/current.md` ephemeral; archive/hand off with `aoc-stm` and store durable decisions in `aoc-mem`.
