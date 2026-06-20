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
- Seeds project OMP assets under `.omp/extensions/`, `.omp/agents/`, `.omp/skills/`, and `.omp/manifest.toml`
- Installs AOC OMP extensions from `.omp/manifest.toml` into `${AOC_OMP_AGENT_DIR:-$HOME/.omp/agent}/extensions` when available
- Installs AOC OMP agent templates from `.omp/manifest.toml` into `${AOC_OMP_AGENT_DIR:-$HOME/.omp/agent}/agents` when available
- Installs OMP skills from `.omp/manifest.toml` into `${AOC_OMP_AGENT_DIR:-$HOME/.omp/agent}/skills` when available
- Seeds reusable preset assets in `.aoc/presets/{design,hyperframes,ops,research,test}/` when missing
- Seeds `.aoc/init-state.json` with the current AOC project version and applies version-specific migrations on older repos
- Validates `.omp/skills` as the canonical skill surface
