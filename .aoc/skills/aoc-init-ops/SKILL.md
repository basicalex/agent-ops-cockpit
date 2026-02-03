---
name: aoc-init-ops
description: Initialize or repair AOC context, memory, and tasks safely.
---

## When to use
- New repository setup
- Missing `.aoc/` or `.taskmaster/`
- Stale or inconsistent context

## Run
- `aoc-init`
- To skip Rust builds: `AOC_INIT_SKIP_BUILD=1 aoc-init`

## What it does
- Creates `.aoc/` and `.taskmaster/` if missing
- Generates `.aoc/context.md`
- Seeds `.aoc/memory.md`
- Syncs skills for existing agent targets
