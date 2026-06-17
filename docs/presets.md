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

This means design and motion skills can stay installed in the repo without acting like the default AOC behavior.

With `preset: off`:
- no preset-specific prompt injection
- no design-first routing bias
- no preset-specific active skills

With `preset: design`:
- `frontend-design`, `architecture-design`, and `design-director` become active
- mode-specific design skills become recommended only when relevant
- `DESIGN.md` remains the durable handoff/source-of-truth contract

Shipped presets:
- `design`: product/design critique, specs, tokens, brand, motion-aware review
- `hyperframes`: video/campaign production and render workflow
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

`aoc-init` seeds these assets, plus `.aoc/layouts/design.kdl` and `retired Pi preset controls/`, into other projects. Managed preset/runtime/design assets are now refreshed in existing repos too.

## Commands

Generic:
- `/preset`
- `/preset status`
- `/preset menu`
- `/preset select`
- `/preset-menu`
- `/preset design`
- `/preset hyperframes`
- `/preset ops`
- `/preset research`
- `/preset test`
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
- `/design-director premium`
- `/design-director funnel`
- `/design-director dashboard`
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

## Preset skill routing

Current manifest behavior:
- design active: `frontend-design`, `architecture-design`, `design-director`; dashboard guardrails become active only in `dashboard` mode
- design recommended by mode: critique/spec/diff/tokens/brand/premium/funnel/motion/dashboard specialists only when that mode is selected
- hyperframes active: `aoc-hyperframes`
- hyperframes recommended by mode: `aoc-hyperframes`
- ops active: none by default; mode recommends `aoc-init-ops`, `vercel-cli`, `rlm-analysis`, `aoc-map`, or `tm-cc`
- research active: `web-research`; mode recommends `agent-browser` or `rlm-analysis` when useful
- test active: `architecture-design`, `agent-browser`; modes recommend `rlm-analysis`, `design-review`, or `vercel-cli` when useful

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
- Design
- HyperFrames
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

Focused lenses are available through either nested `Alt+X` sub-options or slash commands. Examples: `/design-director spec`, `/hyperframes-director review`, `/preset ops deploy`, `/preset research repo`.

Changing a preset/mode updates runtime routing immediately: the next agent turn receives the active preset prompt context. It also updates `~/.omp/agent/config.yml` skill filters. Run `/reload` only when you want Pi's visible skill inventory/list to match the selected preset.

## Relationship to Open Design

The Design preset is a terminal/session design-routing layer. It is useful for critique, specs, implementation handoff, tokens, brand guidance, and motion-aware review.

Open Design is the GUI studio layer. Use it when you need visual iteration, live preview, design-system selection, decks, templates, or polished prototype artifacts:

```bash
aoc-od start --open
# iterate in OD GUI
aoc-od import latest
```

Then return to AOC presets for implementation and campaign handoff:

```text
OD GUI exploration -> imported artifact -> Alt+X Design critique/spec/handoff -> implementation
OD GUI direction -> imported artifact -> Alt+X HyperFrames -> campaign/render factory
```

See [Open Design studio](open-design.md).

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
