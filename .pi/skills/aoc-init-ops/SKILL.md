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
- Ensures spec directory `.taskmaster/docs/specs/` is available for tag/task links
- Seeds `.pi/settings.json` when missing
- Seeds and refreshes managed core PI prompt templates in `.pi/prompts/` (`/aoc-ops`, `/tm-cc`, `/lexicon`, `/commit`); legacy `/implement`, `/handoff`, and `/rresume` prompts are not seeded for Herdr/OMP defaults
- Installs AOC OMP extensions (`aoc-codegraph.ts`, `aoc-mind.ts`, `aoc-commit.ts`, `aoc-jj-init.ts`, `aoc-brand-content.ts`, `aoc-web-search.ts`) into the configured OMP agent extension directory when available
- Installs AOC OMP agent templates (`brand-strategy.md`, `brand-concept.md`, `svg-asset.md`, `hyperframes-content.md`) into the configured OMP agent directory when available
- Seeds PI default extensions in `.pi/extensions/` (`minimal.ts`, `themeMap.ts`, `mind-ingest.ts`, `mind-ops.ts`, `mind-context.ts`, `mind-focus.ts`, `aoc-models.ts`, `aoc-codegraph.ts`, `aoc-compaction.ts`, plus `lib/mind.ts` and `lib/caveman.ts`) when missing
- Seeds the preset runtime family in `.pi/extensions/aoc-presets/` when missing
- Seeds reusable preset assets in `.aoc/presets/{design,hyperframes,ops,research,test}/` when missing
- Seeds `.aoc/init-state.json` with the current AOC project version and applies version-specific migrations on older repos
- Seeds the vendored local PI multi-auth package at `.pi/packages/pi-multi-auth-aoc` and wires `.pi/settings.json` to load it only when the package is available
- Removes legacy global npm `pi-multi-auth` package entries from `~/.pi/agent/settings.json` to avoid duplicate extension loading
- Migrates missing legacy project-local PI prompts/skills from `.aoc/prompts/pi/` and `.aoc/skills/` into `.pi/**` (non-destructive), and cleans safe `tmcc` prompt alias duplicates
- Ensures `.pi/skills` baseline (PI-first canonical)
