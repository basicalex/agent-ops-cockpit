# Deprecations and removals

This page tracks intentional simplifications in the OMP-first AOC surface.

## Runtime surface

Removed from the active operator path:

- Legacy Pi settings, prompts, skills, extensions, packages, and agents.
- Multi-runtime wrapper behavior in core agent commands.
- Zellij cockpit-only layout/theme sync surfaces.

## Canonical supported runtime

- OMP runtime config: `~/.omp/agent/config.yml`
- Project OMP sources: `.omp/extensions/`, `.omp/agents/`, `.omp/skills/`
- AOC state/contracts: `.aoc/**`, `.taskmaster/**`, `AGENTS.md`, `DESIGN.md`

`aoc-init` no longer migrates or recreates legacy Pi paths.

## Skill sync behavior

Kept:

- OMP-only validation/sync via `aoc-skill sync --root .` and `aoc-skill validate --root .`

Removed:

- Legacy Pi skill sync and prompt alias cleanup.
