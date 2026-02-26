# Deprecations and removals (PI-first AOC)

This page tracks intentional simplifications in the PI-first AOC surface.

## Runtime surface

### Removed from core support

- Non-PI runtime wrappers/installers in active AOC paths
- Multi-runtime selector behavior in core agent commands

### Canonical supported runtime

- `pi` only (`aoc-agent`, `aoc-agent-run`, `aoc-agent-install`, `aoc-control`)

If you need another CLI, use the wrapper path described in [Agent Extensibility](agent-extensibility.md).

## Project-local runtime files

### Canonical location (current)

- `.pi/**`
  - settings
  - prompts
  - skills
  - extensions

### Legacy fallback sources (migration only)

- `.aoc/prompts/pi/**`
- `.aoc/skills/**`

`aoc-init` migrates missing assets from these legacy locations into `.pi/**` non-destructively.

## Skill sync behavior

### Removed behavior

- Auto-sync of non-PI skill targets (`.codex`, `.claude`, `.opencode`, `.agents`) in `aoc-init`

### Current behavior

- PI-only sync surface (`.pi/skills/**`)

## Prompt alias cleanup

### Legacy alias

- `/tmcc`

### Canonical prompt

- `/tm-cc`

`aoc-init` removes safe duplicate aliases and warns when manual merge is required.

## Why this was done

- Fewer moving parts in fresh setup
- Lower maintenance and support drift
- Predictable release validation and troubleshooting
