# AOC Presets

AOC Presets are a project-local orchestration layer for reusable session modes.

A preset coordinates:
- bounded prompt components
- OMP runtime state
- slash commands
- installed vs active vs recommended skill routing
- transition handoff summaries
- optional convenience bootstraps

A preset is **not**:
- a nested-skill runtime
- a replacement for Herdr workspaces or tabs
- a replacement for skills

## Runtime-first model

The primary entrypoint is still regular AOC:

```bash
aoc
```

Then switch to an available preset:

```text
/preset ops
/preset off
```

## Runtime model

The preset runtime lives in:

```text
retired Pi preset controls/
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
- **installed**: present in `.omp/skills` and manually invocable when explicitly needed
- **active**: currently part of preset routing bias
- **recommended**: suggested only when the task matches the active preset/mode

Preset routing distinguishes installed skills from those active for a selected mode.

With `preset: off`:
- no preset-specific prompt injection
- no preset-specific active skills

Shipped presets:
- `ops`: production operations, health, deploys, repo mapping, tasks
- `research`: evidence gathering across web, repo, and source sets
- `test`: implementation verification, browser QA, preview smoke checks, and regression testing

## Preset assets

Preset assets live in:

```text
.aoc/presets/<id>/
  preset.toml
  components/
```

`aoc-init` seeds the remaining preset assets into other projects.

## Commands

Generic:
- `/preset`
- `/preset status`
- `/preset menu`
- `/preset select`
- `/preset-menu`
- `/preset ops`
- `/preset research`
- `/preset test`
- `/preset off`
- `/preset skills`
- `/preset handoff`
- `/preset history`
- `/preset clear-handoff`


## Preset skill routing

Current manifest behavior:
- ops active: none by default; mode recommends `aoc-init-ops`, `vercel-cli`, `rlm-analysis`, or `aoc-map`
- research active: `web-research`; mode recommends `agent-browser` or `rlm-analysis` when useful
- test active: `architecture-design`, `agent-browser`; modes recommend `rlm-analysis` or `vercel-cli` when useful

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

`Alt+X` intentionally shows only umbrella modes:
- Ops
- Research
- Test
- Preset off

Inside the navigator:
- `j` / `k` or arrow keys move
- `enter` applies the currently selected item
- On an umbrella preset with sub-options, `enter` selects the umbrella/default mode, while `l` / `→` opens specific modes
- `h` / `←` / `esc` goes back from sub-options; `q` closes
- `x` rotates Caveman level
- `Alt+X` is the global shortcut to reopen the mode switcher

Focused lenses are available through nested `Alt+X` sub-options or slash commands such as `/preset ops deploy`.

Changing a preset/mode updates runtime routing immediately: the next agent turn receives the active preset prompt context. It also updates `~/.omp/agent/config.yml` skill filters. Run `/reload` only when you want Pi's visible skill inventory/list to match the selected preset.

## Operator mental model

Use these terms consistently:
- **installed skill**: exists in the repo
- **visible skill**: currently exposed in Pi after the preset-managed skill filter is applied
- **active skill**: currently shaping routing for the active preset/mode
- **recommended skill**: suggested because it matches the current preset/mode
- **primary flow**: `aoc` then live preset switching
- **convenience bootstrap**: a preset selected at startup

