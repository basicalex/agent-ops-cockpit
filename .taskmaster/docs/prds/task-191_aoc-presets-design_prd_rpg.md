# Task 191 — AOC Presets Framework + Design Preset PRD (RPG)

## Problem Statement
AOC already has strong building blocks for agent ergonomics:
- custom layouts and `aoc.<layout>` shell shortcuts
- Pi skills for domain workflows
- Pi extensions for persistent runtime behavior
- AOC handshake/orientation for managed startup
- AOC Mind as a separate project-intelligence subsystem

What AOC does **not** yet have is a first-class way to package these pieces into a reusable, opt-in session mode that can be started deliberately for a certain kind of work.

Today, if we want a design-focused session, we can approximate it with ad hoc prompts, manually loading skills, or a custom layout, but we cannot yet say:

> open a design-oriented AOC session with the right layout, prompt behavior, commands, and skill context from the start.

That gap creates three operational problems:
- **session setup friction**: operators must manually recreate the same working mode at the start of design-heavy work
- **missing runtime persistence**: skills alone do not provide stable per-session mode state or reliable enable/disable semantics
- **unclear composition model**: AOC lacks a canonical architecture for combining layouts, prompt components, commands, and skills into a reusable preset

We need a generic **AOC Presets** framework, with **`design`** as the first real preset, so that `aoc.design` opens a preset-aware design session and Pi stays in the correct design mode until explicitly changed or disabled.

## Architectural Decision Summary
This PRD establishes the following top-level decisions:

1. **AOC Presets are a new orchestration layer**
   - Presets are not nested skills and not a new memory subsystem.
   - Presets coordinate layouts, prompt components, runtime state, and skills.

2. **The first preset is `design`**
   - The operator entrypoint is `aoc.design`.
   - The layout name is `design`, which naturally exposes the shell shortcut `aoc.design`.

3. **Pi extensions own runtime preset behavior**
   - Persistent preset activation, mode switching, and bounded prompt injection belong in a Pi extension.
   - Skills remain reusable expertise assets, not the persistence mechanism.

4. **AOC Mind remains a separate subsystem**
   - Presets must work without Mind.
   - Mind may later be used as an optional adapter/input, but presets do not depend on it.

5. **Shell handshake remains generic**
   - The shell-side startup handshake can expose lightweight preset awareness if useful.
   - Mode behavior and semantic prompt composition must live in the Pi runtime, not in shell boot logic.

## Target Users
- Operators who want to enter a design-focused AOC session with one command.
- Maintainers who want a reusable architecture for future presets beyond design.
- Teams who need a consistent, sharable, project-local way to activate domain-specific session behavior.

## Success Metrics
- Running `aoc.design` opens a custom design layout and starts a preset-aware session.
- The active preset is visible and persisted for the session.
- The default design mode is `critique` unless explicitly overridden.
- Runtime commands can switch design/motion modes and disable the preset cleanly.
- Prompt injection remains bounded: one core component plus one mode component by default.
- The design preset works when AOC Mind is unavailable.
- The architecture can support a second preset without redesigning the framework.

---

## Product Framing

### Definition: Preset
A preset is a **session orchestration bundle** that specifies:
- which layout should launch the session
- which prompt components should shape agent behavior
- which commands/modes are available
- which skills are recommended for deeper workflows
- which runtime controller manages session state

### Definition: Design Preset
The `design` preset is a preset that optimizes AOC/Pi sessions for:
- UI critique
- design specs
- implementation-vs-design diffing
- design handoff
- design tokens analysis
- brand/art-direction review
- motion direction and motion-specific task routing

### Explicit Non-Goals
This work does **not**:
- create a new “AOC intelligence” subsystem
- move AOC Mind into preset architecture
- turn skills into a persistent state machine
- replace the generic managed `aoc` layout
- require Alt+C preset management in V1

---

## Capability Tree

### Capability: Generic Preset Runtime
Provide a reusable architecture for project-local session presets.

#### Feature: Preset manifest loading
- **Description**: Load preset definitions from project-local manifest files.
- **Inputs**: `.aoc/presets/*/preset.toml`
- **Outputs**: validated in-memory preset registry
- **Behavior**: manifests define preset identity, layout, default mode, component mapping, and runtime metadata

#### Feature: Session preset activation state
- **Description**: Persist the active preset and mode for the current Pi session.
- **Inputs**: environment activation, explicit commands, restored session entries
- **Outputs**: current preset runtime state
- **Behavior**: one active preset per session; explicit commands override environment defaults

#### Feature: Bounded prompt-component composition
- **Description**: Assemble prompt behavior from modular components rather than giant hardcoded blobs.
- **Inputs**: preset manifest + current mode
- **Outputs**: appended system-prompt text for the turn
- **Behavior**: inject a small core component plus the active mode component by default

### Capability: Design Preset Boot Path
Make `aoc.design` the canonical way to start a design-aware AOC session.

#### Feature: Design layout entrypoint
- **Description**: Add a project-shared custom layout named `design`.
- **Inputs**: `.aoc/layouts/design.kdl`
- **Outputs**: layout-discovered shell shortcut `aoc.design`
- **Behavior**: preserve managed top bar, metadata sync, and normal AOC launch conventions

#### Feature: Preset-aware environment activation
- **Description**: Start Pi with explicit preset env values when the design layout boots.
- **Inputs**: layout pane startup commands
- **Outputs**: env such as `AOC_PRESET=design` and `AOC_PRESET_MODE=critique`
- **Behavior**: the runtime extension observes these envs on session start and activates the matching preset

### Capability: Design Runtime Mode System
Support persistent design and motion modes within the active session.

#### Feature: Design mode switching
- **Description**: Switch between critique, spec, diff, handoff, tokens, brand, and motion modes.
- **Inputs**: slash commands
- **Outputs**: updated preset mode state and prompt composition
- **Behavior**: explicit mode switch persists for later turns until changed again or disabled

#### Feature: Motion subdomain control
- **Description**: Support motion-focused submodes while keeping the design preset active.
- **Inputs**: `/motion-director ...` commands
- **Outputs**: motion-specific mode selection
- **Behavior**: motion remains part of the design preset family rather than a completely separate session preset in V1

#### Feature: Disable/reset behavior
- **Description**: Return to ordinary non-preset behavior cleanly.
- **Inputs**: `/design-off` or `/preset off`
- **Outputs**: cleared preset state
- **Behavior**: no stale design prompt components continue injecting after disable

### Capability: Design Skill Bundle
Provide reusable design-specialist skills without making them the preset controller.

#### Feature: Umbrella design router skill
- **Description**: Add a `design-director` skill that expresses the design domain and mode map.
- **Inputs**: project-local skill files
- **Outputs**: reusable design workflow guidance
- **Behavior**: aligns language, structure, and routing expectations for design tasks

#### Feature: Supporting design specialist skills
- **Description**: Add focused skills for review, spec, diff, handoff, tokens, and motion workflows.
- **Inputs**: skill directories under `.pi/skills/`
- **Outputs**: on-demand expert workflow packages
- **Behavior**: runtime preset behavior stays light; deep task guidance remains in skills

### Capability: Validation and Documentation
Make the preset framework understandable and safe to extend.

#### Feature: Preset validation rules
- **Description**: Validate manifest references and mode/component consistency.
- **Inputs**: manifests, component files, layout names, skill references
- **Outputs**: actionable validation feedback
- **Behavior**: fail clearly for missing components or invalid default modes; degrade safely at runtime when possible

#### Feature: Operator documentation
- **Description**: Document the preset architecture and the `aoc.design` boot path.
- **Inputs**: final architecture and file layout
- **Outputs**: docs for maintainers and users
- **Behavior**: explain presets as orchestration bundles distinct from layouts, skills, and Mind

---

## Architecture Layers

### Layer 1 — Preset manifests
Canonical project-local manifests live under:

```text
.aoc/presets/<id>/preset.toml
```

Responsibilities:
- identify the preset
- declare its default mode
- map modes to prompt components
- describe runtime metadata and recommended skills

### Layer 2 — Prompt components
Prompt components live beside the manifest:

```text
.aoc/presets/design/components/
  core.md
  mode-critique.md
  mode-spec.md
  mode-diff.md
  mode-handoff.md
  mode-tokens.md
  mode-brand.md
  mode-motion.md
```

Responsibilities:
- define bounded, composable system-prompt fragments
- keep runtime behavior editable without rewriting extension code

### Layer 3 — Runtime controller
Project-local Pi extension:

```text
.pi/extensions/aoc-presets/
```

Responsibilities:
- load preset manifests
- restore and persist preset session state
- register commands
- append prompt components in `before_agent_start`
- expose active preset/mode status

### Layer 4 — Layout entrypoint
Project-local layout:

```text
.aoc/layouts/design.kdl
```

Responsibilities:
- boot the design session shape
- preserve AOC layout conventions
- export preset env vars for the runtime controller

### Layer 5 — Skills
Project-local Pi skills:

```text
.pi/skills/design-director/
.pi/skills/motion-director/
.pi/skills/design-review/
.pi/skills/design-spec/
.pi/skills/design-diff/
.pi/skills/design-handoff/
.pi/skills/design-tokens/
```

Responsibilities:
- provide deep workflow knowledge
- stay reusable independently of preset activation

---

## Repository Structure

```text
project-root/
├── .aoc/
│   ├── layouts/
│   │   └── design.kdl
│   └── presets/
│       └── design/
│           ├── preset.toml
│           └── components/
│               ├── core.md
│               ├── mode-critique.md
│               ├── mode-spec.md
│               ├── mode-diff.md
│               ├── mode-handoff.md
│               ├── mode-tokens.md
│               ├── mode-brand.md
│               └── mode-motion.md
├── .pi/
│   ├── extensions/
│   │   └── aoc-presets/
│   │       ├── index.ts
│   │       ├── manifest.ts
│   │       ├── registry.ts
│   │       ├── state.ts
│   │       ├── commands.ts
│   │       └── renderer.ts
│   ├── prompts/
│   │   ├── design-review.md
│   │   ├── design-spec.md
│   │   ├── design-diff.md
│   │   └── design-handoff.md
│   └── skills/
│       ├── design-director/
│       │   └── SKILL.md
│       ├── motion-director/
│       │   └── SKILL.md
│       ├── design-review/
│       │   └── SKILL.md
│       ├── design-spec/
│       │   └── SKILL.md
│       ├── design-diff/
│       │   └── SKILL.md
│       ├── design-handoff/
│       │   └── SKILL.md
│       └── design-tokens/
│           └── SKILL.md
├── docs/
│   └── presets.md
└── .taskmaster/
    └── docs/prds/
        └── task-191_aoc-presets-design_prd_rpg.md
```

---

## Manifest Contract (V1)

Example:

```toml
id = "design"
label = "Design"
layout = "design"
defaultMode = "critique"
version = 1

[runtime]
controller = "aoc-presets"
statusBadge = "DESIGN"
persistSessionState = true

[activation]
envPreset = "design"
envMode = "critique"

[components]
core = ["core"]
default = ["core", "mode-critique"]

[components.modes]
critique = ["core", "mode-critique"]
spec = ["core", "mode-spec"]
diff = ["core", "mode-diff"]
handoff = ["core", "mode-handoff"]
tokens = ["core", "mode-tokens"]
brand = ["core", "mode-brand"]
motion = ["core", "mode-motion"]

[commands]
enable = ["design-director"]
disable = ["design-off"]

[skills]
recommended = [
  "design-director",
  "motion-director",
  "design-review",
  "design-spec",
  "design-diff",
  "design-handoff",
  "design-tokens"
]

[integrations.mind]
policy = "separate"
available = true
default = "off"
```

### Manifest Invariants
- preset `id` must match directory name
- `layout` must match an existing custom layout if specified
- `defaultMode` must exist in `components.modes`
- every referenced component name must resolve to a component markdown file
- Mind integration flags are descriptive only in V1; they do not imply coupling or required activation

---

## Runtime State Contract

### State shape
The runtime controller persists a custom session entry similar to:

```json
{
  "preset": "design",
  "mode": "critique",
  "submode": null,
  "source": "layout",
  "updatedAt": 0
}
```

### Rules
- exactly one active preset per session in V1
- explicit command changes override env activation
- `off` clears active preset state
- design motion commands update `mode` inside the same preset family in V1

---

## Command Surface (V1)

### Generic preset commands
- `/preset`
- `/preset status`
- `/preset off`
- `/preset design`

### Design commands
- `/design-director`
- `/design-director critique`
- `/design-director spec`
- `/design-director diff`
- `/design-director handoff`
- `/design-director tokens`
- `/design-director brand`
- `/design-director motion`
- `/design-off`

### Motion commands
- `/motion-director`
- `/motion-director plan`
- `/motion-director timeline`
- `/motion-director scroll`
- `/motion-director svg`
- `/motion-director text`
- `/motion-director react`
- `/motion-director audit`
- `/motion-off`

### Command Behavior Rules
- commands mutate runtime state, not skill installation state
- disabling the preset stops future prompt injection immediately
- invalid modes return allowed values clearly
- commands should update any preset status widget/badge if present

---

## Runtime Flow: `aoc.design`

### Boot flow
1. Operator runs `aoc.design`.
2. AOC shell shortcut resolves to `aoc-new-tab --layout design`.
3. `design.kdl` starts panes using normal AOC layout conventions.
4. The agent pane exports `AOC_PRESET=design` and `AOC_PRESET_MODE=critique` before launching Pi/AOC.
5. Pi starts and loads project extensions.
6. `aoc-presets` sees preset env activation during `session_start`.
7. The extension validates and activates preset state.
8. `before_agent_start` appends the preset’s core prompt component and active mode component.
9. Design behavior remains active until explicitly changed or disabled.

### Restore flow
1. Existing session resumes.
2. `aoc-presets` rebuilds runtime state from custom session entries.
3. If no explicit override is present, env activation may initialize the preset.
4. Prompt-component composition resumes from restored state.

---

## Module Definitions

### Module: `.pi/extensions/aoc-presets/index.ts`
- **Maps to capability**: Generic Preset Runtime
- **Responsibility**: boot the preset runtime, subscribe to Pi lifecycle hooks, and coordinate registry/state/commands
- **Exports**:
  - preset runtime initialization
  - session-start restoration
  - `before_agent_start` prompt injection

### Module: `.pi/extensions/aoc-presets/manifest.ts`
- **Maps to capability**: Preset manifest loading
- **Responsibility**: read and validate preset manifests from `.aoc/presets/*/preset.toml`
- **Exports**:
  - manifest parsing
  - invariant checking
  - error reporting

### Module: `.pi/extensions/aoc-presets/state.ts`
- **Maps to capability**: Session preset activation state
- **Responsibility**: define runtime state shape and session-entry persistence/restore behavior
- **Exports**:
  - state serialization
  - state restoration
  - active preset resolution

### Module: `.pi/extensions/aoc-presets/commands.ts`
- **Maps to capability**: Design Runtime Mode System
- **Responsibility**: register and implement `/preset`, `/design-director`, `/design-off`, `/motion-director`, and `/motion-off`
- **Exports**:
  - command registration
  - mode validation helpers
  - status update helpers

### Module: `.pi/extensions/aoc-presets/renderer.ts`
- **Maps to capability**: Bounded prompt-component composition
- **Responsibility**: resolve component files and render bounded prompt snippets for the current preset/mode
- **Exports**:
  - component lookup
  - prompt assembly
  - safe fallback behavior when components are missing

### Module: `.aoc/layouts/design.kdl`
- **Maps to capability**: Design Preset Boot Path
- **Responsibility**: provide the design session layout and export preset env vars during launch

### Module: `.pi/skills/design-*`
- **Maps to capability**: Design Skill Bundle
- **Responsibility**: provide reusable expert workflows independent of runtime preset state

---

## Validation Plan

### Functional validation
- `aoc-layout --list` shows `design`
- `aoc.design` launches the design layout successfully
- the agent session reports active preset `design`
- the default mode is `critique`
- `/design-director spec` changes the active mode to `spec`
- `/design-off` clears the active preset
- `/preset design` reactivates the design preset without requiring layout relaunch

### Robustness validation
- missing Mind does not block preset startup
- missing optional prompt templates do not block preset startup
- invalid mode names fail with clear guidance
- missing component files produce clear runtime degradation instead of crashes
- resumed sessions restore preset state correctly

### Architecture validation
- no shell-side mode logic duplication is required for design behavior
- design skill content remains separately usable even with preset inactive
- a second preset can be added by introducing another manifest/component/layout set without redesigning the runtime controller

---

## Expedite-Oriented Implementation Plan

1. define manifest schema and core invariants
2. implement preset extension runtime + persistence
3. add design preset assets and prompt components
4. add `design.kdl` and confirm `aoc.design` boot path
5. port the design skill bundle locally
6. add command surface and status feedback
7. validate end-to-end and document operator usage

---

## Acceptance Criteria
- A generic preset runtime exists under `.pi/extensions/aoc-presets/`.
- A project-local design preset manifest and component set exist under `.aoc/presets/design/`.
- A project-local `design` layout exists and boots a preset-aware design session.
- `aoc.design` is the canonical design-session entrypoint.
- Design and motion commands update persistent preset state correctly.
- The architecture remains explicitly separate from AOC Mind.
- Documentation explains presets as an orchestration layer distinct from layouts, skills, and Mind.
