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
- Ensures PRD directory `.taskmaster/docs/prds/` is available for tag/task links
- Seeds `.pi/settings.json` when missing
- Seeds PI prompt templates in `.pi/prompts/` (`/aoc-ops`, `/teach`, `/teach-full`, `/teach-dive`, `/teach-ask`, `/tm-cc`) when missing
- Seeds PI default extensions in `.pi/extensions/` (`minimal.ts`, `themeMap.ts`) when missing
- Migrates missing legacy PI prompts/skills from `.aoc/prompts/pi/` and `.aoc/skills/` into `.pi/**` (non-destructive), and cleans safe `tmcc` prompt alias duplicates
- Optional MoreMotion prompt `/momo` is seeded by `aoc-momo init`
- Ensures `.pi/skills` baseline (PI-first canonical)
