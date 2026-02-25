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
- Seeds `.aoc/stm/current.md` and `.aoc/stm/archive/` without overwriting existing STM files
- Ensures task-level PRD directory `.taskmaster/docs/prds/` is available when used
- Seeds OpenCode `/stm` command in `.opencode/commands/stm.md` when missing and keeps STM guidance aligned (`aoc-stm` = current draft, `aoc-stm handoff` = seal+print handoff, `aoc-stm resume` = archived resume)
- Seeds OpenCode `/prd` command in `.opencode/commands/prd.md` when missing
- Syncs skills for existing agent targets
