---
name: aoc-dox-cartography
description: Map sparse, evidence-backed AGENTS.md local contracts using AOC DOX, CodeGraph, and OMP subagents.
---

# AOC DOX Cartography

Use this workflow to create sparse operational context, not general documentation.

## Operator workflow

1. Use `aoc_dox_writer` action `map` only when the active agent is `dox-writer` or the main agent is intentionally refreshing DOX metadata; otherwise use CLI `aoc dox map --json` under normal write-capable implementation context.
2. Use `.aoc/dox/map.json` resolution coverage before launching scouts.
3. Launch `dox-scout` in parallel only for high-risk or insufficient-coverage paths.
4. Launch `dox-mapper` only for scout-approved candidate areas.
5. Launch `dox-critic` on every create/update proposal.
6. Use `dox-writer` only after critic approval; writer may dry-run only.
7. Human/operator runs `aoc dox apply --yes` only after dry-run review.
8. Finish with `aoc_dox` action `doctor` or CLI `aoc dox doctor`.

## Guardrails

- `aoc_dox` is read-only plus apply dry-run; `aoc_dox_writer` is metadata-writing and only for writer workflows; neither exposes `apply --yes`.
- `aoc dox map` may update only `.aoc/dox/` metadata, never `AGENTS.md` files.
- CodeGraph is evidence only. Do not run CodeGraph init, sync, index, install, unlock, or uninit from this workflow.
- Local `AGENTS.md` files are for durable operational contracts only; reject architecture summaries and obvious directory descriptions.
