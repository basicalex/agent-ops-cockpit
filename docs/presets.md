# AOC Presets

AOC Presets are a project-local orchestration layer for reusable session modes.

A preset coordinates:
- a layout entrypoint
- bounded prompt components
- Pi runtime state
- slash commands
- recommended skills

A preset is **not**:
- a nested-skill runtime
- a replacement for layouts
- a replacement for skills
- part of AOC Mind

## First preset: design

The first shipped preset is `design`.

Canonical entrypoint:

```bash
aoc.design
```

This works because AOC custom layouts automatically expose `aoc.<layout>` shell shortcuts, and the preset layout name is `design`.

## Runtime model

The preset runtime lives in:

```text
.pi/extensions/aoc-presets/
```

It:
- loads preset manifests from `.aoc/presets/*/preset.toml`
- restores/persists active preset state in the Pi session
- injects prompt components in `before_agent_start`
- exposes preset commands

## Design preset assets

```text
.aoc/presets/design/
  preset.toml
  components/
```

`aoc-init` now seeds these assets, plus `.aoc/layouts/design.kdl` and `.pi/extensions/aoc-presets/`, into other projects when they are missing.

## Commands

Generic:
- `/preset`
- `/preset status`
- `/preset design`
- `/preset off`

Design:
- `/design-director`
- `/design-director critique`
- `/design-director spec`
- `/design-director diff`
- `/design-director handoff`
- `/design-director tokens`
- `/design-director brand`
- `/design-director motion`
- `/design-off`

Motion:
- `/motion-director`
- `/motion-director plan`
- `/motion-director timeline`
- `/motion-director scroll`
- `/motion-director svg`
- `/motion-director text`
- `/motion-director react`
- `/motion-director audit`
- `/motion-off`

## Separation from AOC Mind

AOC Mind remains a separate subsystem.

Presets may later consume Mind-derived context explicitly, but the preset framework does not require Mind to function.

## Current V1 design contract

- one active preset per session
- one core prompt component plus one mode component by default
- layout boot can activate a preset through env
- commands can activate/deactivate/switch modes without relaunching the layout
