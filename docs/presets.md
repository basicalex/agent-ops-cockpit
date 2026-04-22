# AOC Presets

AOC Presets are a project-local orchestration layer for reusable session modes.

A preset coordinates:
- bounded prompt components
- Pi runtime state
- slash commands
- installed vs active vs recommended skill routing
- transition handoff summaries
- optional convenience bootstraps

A preset is **not**:
- a nested-skill runtime
- a replacement for layouts
- a replacement for skills
- part of AOC Mind

## Runtime-first model

The primary entrypoint is still regular AOC:

```bash
aoc
```

Then switch presets live inside the same Pi session:

```text
/preset design
/design-director spec
/motion-director react
/preset off
```

`aoc.design` still exists as a convenience bootstrap, but it is no longer the primary flow. It simply starts AOC with the `design` preset preactivated.

## Runtime model

The preset runtime lives in:

```text
.pi/extensions/aoc-presets/
```

It:
- loads preset manifests from `.aoc/presets/*/preset.toml`
- restores/persists active preset state in the Pi session
- computes active and recommended skills from preset + mode + submode
- keeps installed-but-inactive skills dormant in routing guidance
- injects prompt components in `before_agent_start`
- stores a compact handoff summary when switching modes/presets
- stores a short preset transition history
- exposes preset commands and runtime UI state

## Skill activation model

Preset skills are now treated as:
- **installed**: present in `.pi/skills` and manually invocable when explicitly needed
- **active**: currently part of preset routing bias
- **recommended**: suggested only when the task matches the active preset/mode

This means design and motion skills can stay installed in the repo without acting like the default AOC behavior.

With `preset: off`:
- no preset-specific prompt injection
- no design-first routing bias
- no preset-specific active skills

With `preset: design`:
- `design-director` becomes active
- mode-specific design skills become recommended
- motion submodes bias toward the relevant Anime.js specialist skills

## First preset: design

The first shipped preset is `design`.

Design assets live in:

```text
.aoc/presets/design/
  preset.toml
  components/
```

`aoc-init` seeds these assets, plus `.aoc/layouts/design.kdl` and `.pi/extensions/aoc-presets/`, into other projects when they are missing.

## Commands

Generic:
- `/preset`
- `/preset status`
- `/preset menu`
- `/preset select`
- `/preset-menu`
- `/preset design`
- `/preset off`
- `/preset skills`
- `/preset handoff`
- `/preset history`
- `/preset clear-handoff`

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

## Design preset skill routing

Current design manifest behavior:
- active by default: `design-director`
- active in motion mode: `motion-director`
- recommended by mode:
  - critique -> `design-review`
  - spec -> `design-spec`
  - diff -> `design-diff`
  - handoff -> `design-handoff`
  - tokens -> `design-tokens`
- recommended by motion submode:
  - react -> `animejs-react-integration`, `animejs-core-api`, `animejs-performance-a11y`
  - scroll -> `animejs-scroll-interaction`, `animejs-core-api`, `animejs-performance-a11y`
  - svg -> `animejs-svg-motion`, `animejs-core-api`, `animejs-performance-a11y`
  - text -> `animejs-text-splitting`, `animejs-core-api`, `animejs-performance-a11y`
  - timeline -> `animejs-timelines`, `animejs-core-api`, `animejs-performance-a11y`
  - audit -> `animejs-reviewer`, `animejs-performance-a11y`

## Handoff behavior

When switching preset state, the runtime stores a compact handoff summary that captures:
- where the session came from
- where it is going
- what should be carried forward
- which preset skills were active/recommended before the switch

This handoff is prompt-injected only while a preset is active and can be inspected with `/preset handoff`.

The runtime also keeps a short transition trail, inspectable with `/preset history`.

## Interactive navigator

Use `/preset menu`, `/preset select`, `/preset-menu`, or `Alt+X` to open the mode switcher overlay.

Inside the navigator:
- `j` / `k` or arrow keys move
- `l` / `enter` opens a nested level or applies the selection
- `h` / `esc` goes back
- `q` closes
- `Alt+X` is the global shortcut to reopen the mode switcher

This is the main exploration UI when you want to browse preset, mode, and submode choices without remembering commands.

Changing a preset/mode/submode updates `.pi/settings.json` skill filters and then triggers a runtime reload so the visible Pi skill inventory matches the selected preset.

## Operator mental model

Use these terms consistently:
- **installed skill**: exists in the repo
- **visible skill**: currently exposed in Pi after the preset-managed skill filter is applied
- **active skill**: currently shaping routing for the active preset/mode
- **recommended skill**: suggested because it matches the current preset/mode
- **primary flow**: `aoc` then live preset switching
- **convenience bootstrap**: `aoc.design` preactivating a preset at startup

## Separation from AOC Mind

AOC Mind remains a separate subsystem.

Presets may later consume Mind-derived context explicitly, but the preset framework does not require Mind to function.
