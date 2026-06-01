# Herdr-first workspace direction

AOC is moving from a Zellij-managed cockpit to a Herdr-first workspace.

## Decision

Herdr is now the structural multiplexer and operator surface for agentic workspaces.

The old AOC/Zellij stack solved important problems at the time: persistent panes, project layout, top-bar status, and operator shortcuts. Herdr now handles the structural multiplexer role better, so AOC should stop treating Zellij layouts and the custom top bar as the canonical workspace layer.

## What stays from AOC

Keep only AOC pieces that are still valuable as project/tooling primitives:

- `aoc` as the familiar launcher command
- minimal project initialization conventions where useful
- Taskmaster / `tm` integration
- metadata-only context handshakes for OMP startup
- CodeGraph as a read-only OMP tool when a project has an index
- `aoc-agent-wrap-rs` only if it can provide OMP-native lifecycle/context/provenance without Zellij coupling
- HyperFrames, OpenDesign, Understand, web research, and RTK as standalone project/tooling features
- selected lightweight handoff helpers only if they remain useful
- Mind only as optional, lazy focused recall/provenance after user intent is known
- selected Pi/OMP skills only when they complement the new stack
- docs and install/bootstrap knowledge that can be simplified for Herdr

## What moves out of AOC

Treat these as legacy or transitional:

- managed Zellij layouts
- AOC Zellij tab bar / `zjstatus-aoc`
- AOC Zellij-specific keybindings
- Mission Control
- AOC subagent manager/control surfaces
- Control pane
- agent status surfaces owned by AOC
- pane/workspace/session health displays owned by AOC
- tab/project metadata systems owned by AOC
- AOC Mind as a startup context injector or always-on cockpit dependency
- broad Mind context packs during startup
- heavy install steps that existed only to support the old cockpit stack

## New model

- **Herdr** owns workspaces, tabs, panes, navigation, agent status, workspace health, and operator UI.
- **OMP** owns subagent orchestration.
- **AOC** becomes the compatibility/tooling layer around project setup, task workflows, launch convenience, and retained standalone tools.

The `aoc` command launches/focuses the Herdr workspace by default. The default installer and initializer should stay lean: no Zellij cockpit assets, custom top bar, AOC subagent UI, Mission Control, Control pane, AOC-owned status/health panels, tab metadata systems, or AOC Mind services by default.

## Current Herdr UX baseline

The current baseline config is tracked at:

- `herdr/config.toml`

Important shortcuts:

- `Alt+W` — workspace picker
- `j` / `k` inside workspace picker — move workspace selection
- `Alt+Shift+N` — new workspace
- `Alt+N` — new tab
- `Alt+Q` — close focused pane
- `Alt+H/J/K/L` — move pane focus
- `Alt+I/O` — previous/next tab
- `Alt+?` — keybindings/help

## Migration state

The Herdr-first path is implemented with:

- `bin/aoc-herdr-install` — installs the lean Herdr config baseline, installs the Herdr OMP integration when `omp` is available, installs the AOC-aware plain `omp` shim, and installs AOC OMP extensions including CodeGraph and `/commit`
- `bin/aoc-herdr-launch` — launches/focuses Herdr for the current project root, reusing an existing workspace for the same root when possible
- `bin/aoc-omp-context` — renders the compact metadata-only AOC startup capsule for OMP `--append-system-prompt`
- `bin/aoc-omp` / `aoc omp` — launches upstream OMP with that startup capsule already appended
- `bin/aoc` — delegates to Herdr by default
- `install.sh` — defaults to Herdr/OMP assets and skips legacy Zellij cockpit assets unless `--legacy-zellij` is passed
- `bin/aoc-init` — installs AOC OMP extensions, seeds the lean PI prompt/tool baseline, and skips legacy `.aoc/layouts`, Zellij plugin repair, subagent manager extension, AOC agent presence extension, Mission Control, and Control pane defaults
- `/commit [scope]` — OMP slash command that triggers the safe atomic commit workflow: inspect changes, run targeted validation, stage explicit paths only, commit, and report SHA; never push

The old Zellij cockpit remains available during transition with:

```bash
AOC_LEGACY_ZELLIJ=1 aoc
./install.sh --legacy-zellij
```

The working feature classification is tracked in:

- `docs/aoc-feature-inventory.md`

## OMP startup capsule

Use the metadata-only capsule to give new OMP sessions the project operating contract without injecting broad memories or raw task state:

```bash
aoc-omp-context
```

For direct use, prefer the wrapper:

```bash
aoc omp
```

After install, plain `omp` is also AOC-aware: in a directory initialized by `aoc-init`, the shim routes to `aoc-omp`; outside AOC projects, it delegates to upstream `omp-raw`. Use `OMP_NO_AOC_WRAPPER=1 omp ...` to bypass the shim.

Launchers that need explicit control can write the capsule to a runtime file, then pass that file to OMP:

```bash
capsule="${XDG_RUNTIME_DIR:-/tmp}/aoc-omp-context.$$.md"
aoc-omp-context > "$capsule"
omp --append-system-prompt "$capsule"
```

The capsule is generated by `aoc-handshake --prompt`; JSON consumers should use `aoc-handshake --json`.

## OMP CodeGraph extension

`aoc-herdr-install` copies the repo-tracked extension from:

```text
.omp/extensions/aoc-codegraph.ts
```

to:

```text
${PI_CODING_AGENT_DIR:-~/.omp/agent}/extensions/aoc-codegraph.ts
```

The extension exposes the read-only `aoc_codegraph` tool for:

- `status`
- `files`
- `search`
- `context`
- `callers`
- `callees`
- `impact`
- `affected`

Agents must not use this tool to initialize, index, or sync projects. Operators run those commands explicitly, for example:

```bash
codegraph sync /path/to/project
```
