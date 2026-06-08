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
- AOC Services workspace for project-scoped runtime health, especially managed local search

## What moves out of AOC

Treat these as legacy or transitional:

- managed Zellij layouts
- removed AOC Zellij tab bar assets
- AOC Zellij-specific keybindings
- legacy Mission Control
- AOC subagent manager/control surfaces
- Control pane
- agent status surfaces owned by legacy AOC UI
- legacy pane/workspace/session health displays outside Herdr
- tab/project metadata systems owned by AOC
- AOC Mind as a startup context injector or always-on cockpit dependency
- broad Mind context packs during startup
- heavy install steps that existed only to support the old cockpit stack

## New model

- **Herdr** owns workspaces, tabs, panes, navigation, agent status, workspace health, and operator UI.
- **Herdr AOC Services workspace** owns project-scoped long-lived AOC runtime status/startup, especially managed local SearXNG search.
- **OMP** owns subagent orchestration.
- **AOC** becomes the compatibility/tooling layer around project setup, task workflows, launch convenience, and retained standalone tools.

The `aoc` command launches/focuses the Herdr workspace by default. The default installer and initializer should stay lean: no Zellij cockpit assets, custom top bar, AOC subagent UI, Mission Control, Control pane, legacy AOC-owned status/health panels, tab metadata systems, or AOC Mind services by default.

## Services workspace

Use a separate Services workspace when you want visible runtime ownership without cluttering the coding workspace:

```bash
aoc services
aoc services status
aoc services start search
```

`aoc services` creates/focuses a project-scoped Herdr workspace named `AOC Services · <project> · <hash>`. It keeps an `Overview` tab running `aoc-services up --watch --interval 30` and a `Search` tab with managed-search status and next commands.

Launch behavior is conservative:

| Variable | Behavior |
|---|---|
| `AOC_HERDR_SERVICES=auto` | default; if a Herdr server is already running, ensure the Services workspace without focusing it |
| `AOC_HERDR_SERVICES=off` | never ensure the Services workspace from `aoc` launch |
| `AOC_HERDR_SERVICES=focus` | ensure/focus Services for explicit service-ops sessions |

Agents still consume `aoc_web_search` / `aoc-search`; they do not call Docker or SearXNG directly. `aoc-search` auto-start remains a safety fallback when project policy allows it, not the primary runtime UX.

## Current Herdr UX baseline

The current baseline config is tracked at:

- `herdr/config.toml`

Important shortcuts:

- `Alt+W` — workspace picker
- `j` / `k` inside workspace picker — move workspace selection
- `Alt+Shift+N` — new workspace
- `Alt+Shift+J/K` — next/previous workspace
- `Ctrl+B Shift+R` — rename workspace
- `Alt+N` — new tab
- `Alt+Shift+R` — rename tab
- `Alt+R` — rename focused pane
- `Alt+Q` — close focused pane
- `Alt+H/J/K/L` — move pane focus
- `Alt+U/P` — reserved for moving the current tab left/right when Herdr exposes key-driven tab reordering
- `Alt+I/O` — previous/next tab
- `Alt+?` — keybindings/help

## Migration state

The Herdr-first path is implemented with:

- `bin/aoc-herdr-install` — installs the lean Herdr config baseline, installs the Herdr OMP integration when `omp` is available, installs the AOC-aware plain `omp` shim, installs AOC OMP extensions including CodeGraph, `/commit`, `/brand-content`, and web search, and installs AOC OMP specialist agent templates
- `bin/aoc-herdr-launch` — launches/focuses Herdr for the current project root, reusing an existing workspace for the same root when possible, and best-effort ensuring the Services workspace when `AOC_HERDR_SERVICES=auto`
- `bin/aoc-herdr-services` — creates/focuses the project-scoped Herdr AOC Services workspace without starting Herdr behind the operator's back
- `bin/aoc-omp-context` — renders the compact metadata-only AOC startup capsule for OMP `--append-system-prompt`, including detected VCS mode and preferred command family
- `bin/aoc-omp` / `aoc omp` — launches upstream OMP with that startup capsule already appended
- `bin/aoc` — delegates to Herdr by default and exposes `aoc services`
- `install.sh` — defaults to Herdr/OMP assets and skips legacy Zellij cockpit assets unless `--legacy-zellij` is passed
- `bin/aoc-init` — installs AOC OMP extensions and AOC OMP agent templates, seeds the lean PI prompt/tool baseline, and skips legacy `.aoc/layouts`, Zellij plugin repair, subagent manager extension, AOC agent presence extension, Mission Control, and Control pane defaults
- `/commit [scope]` — OMP slash command that triggers the safe atomic commit workflow: inspect changes, run targeted validation, use Git staging or Jujutsu current-change semantics based on detected VCS, commit only the intended atomic work, and report the Git SHA or Jujutsu change/commit identity; never push

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

The capsule is generated by `aoc-handshake --prompt`. Set `AOC_OMP_CONTEXT_LEVEL=min|compact|full` to choose the startup footprint; `compact` is the default, `min` is the lowest-context handshake, and `full` is verbose/debug. JSON consumers should use `aoc-handshake --json`, which includes VCS mode (`git`, `jj`, `none`) and the preferred command family for agents.

## OMP AOC extensions and agents

`aoc-init` and `aoc-herdr-install` copy repo-tracked OMP extensions from:

```text
.omp/extensions/aoc-codegraph.ts
.omp/extensions/aoc-mind.ts
.omp/extensions/aoc-commit.ts
.omp/extensions/aoc-jj-init.ts
.omp/extensions/aoc-brand-content.ts
.omp/extensions/aoc-web-search.ts
```

to:

```text
${PI_CODING_AGENT_DIR:-~/.omp/agent}/extensions/
```

They also copy repo-tracked OMP agent templates from:

```text
.omp/agents/
```

to:

```text
${PI_CODING_AGENT_DIR:-~/.omp/agent}/agents/
```

`aoc-codegraph.ts` exposes the read-only `aoc_codegraph` tool for code discovery: `status`, `files`, `search`, `context`, `callers`, `callees`, `impact`, and `affected`.

`aoc-mind.ts` exposes the read-only `aoc_mind` tool for historical/provenance intelligence: `status`, `evidence`, `provenance`, and dry-run `mnemopi_candidates`. It augments OMP/Mnemopi with cited AOC Mind evidence; it does not write memories or inject broad startup context.

`aoc-commit.ts` registers `/commit` for safe atomic commits. It uses Git staging in Git-only repositories and Jujutsu current-change/fileset workflows in Jujutsu repositories, including colocated Jujutsu+Git workspaces.

`aoc-jj-init.ts` registers `/jj-init` for explicit Jujutsu setup. It asks the agent to inspect Git/JJ state and dirty work before running `jj git init --colocate`; ordinary `aoc-init` and startup handshakes only detect/report Jujutsu.

`aoc-brand-content.ts` registers `/brand-content` and `/hyperframes-director` for the branded HyperFrames/html-video pipeline. The related OMP agents are `brand-strategy`, `brand-concept`, `svg-asset`, and `hyperframes-content`.

`aoc-web-search.ts` exposes the `aoc_web_search` tool backed by `aoc-search`, so OMP agents can use local SearXNG or direct package/GitHub modes when built-in web-search providers are out of credits, unauthorized, or timing out.
Agents must not use `aoc_codegraph` to initialize, index, or sync projects. Operators run those commands explicitly, for example:

```bash
codegraph sync /path/to/project
```
