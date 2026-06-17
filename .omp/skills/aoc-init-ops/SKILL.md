---
name: aoc-init-ops
description: Initialize or repair AOC context, memory, OMP assets, and tasks safely.
---

## When to use
- New repository setup
- Missing `.aoc/`, `.omp/`, or `.taskmaster/`
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
- Seeds project OMP assets under `.omp/extensions/`, `.omp/agents/`, and `.omp/skills/`
- Installs AOC OMP extensions into `${AOC_OMP_AGENT_DIR:-$HOME/.omp/agent}/extensions` when available
- Installs AOC OMP agent templates into `${AOC_OMP_AGENT_DIR:-$HOME/.omp/agent}/agents` when available
- Installs OMP skills into `${AOC_OMP_AGENT_DIR:-$HOME/.omp/agent}/skills` when available
- Seeds reusable preset assets in `.aoc/presets/{design,hyperframes,ops,research,test}/` when missing
- Seeds `.aoc/init-state.json` with the current AOC project version and applies version-specific migrations on older repos
- Validates `.omp/skills` as the canonical skill surface
