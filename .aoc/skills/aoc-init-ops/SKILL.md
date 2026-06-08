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
- Seeds managed core PI prompt templates in `.pi/prompts/`; legacy `/teach*` prompts are deprecated and removed from the active surface.
- Seeds PI default extensions in `.pi/extensions/` (`minimal.ts`, `themeMap.ts`, `mind-ingest.ts`, `mind-ops.ts`, `mind-context.ts`, `mind-focus.ts`, `aoc-models.ts`, plus `lib/mind.ts`) when missing
- Seeds vendored local PI package `.pi/packages/pi-multi-auth-aoc` and wires `.pi/settings.json` to load it by local path
- Removes legacy global npm `pi-multi-auth` package entries from `~/.pi/agent/settings.json` to avoid duplicate extension loading
- Migrates missing legacy project-local PI prompts/skills from `.aoc/prompts/pi/` and `.aoc/skills/` into `.pi/**` (non-destructive), and cleans safe `tmcc` prompt alias duplicates
- Optional HyperFrames prompt `/hyperframes` and video skills are seeded by `aoc-hyperframes init`
- Ensures `.pi/skills` baseline (PI-first canonical)
