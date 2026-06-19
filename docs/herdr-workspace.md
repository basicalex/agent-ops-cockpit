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
- HyperFrames, OpenDesign, web research, and RTK as standalone project/tooling features
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
- `Alt+U` — reserved for moving the current tab left when Herdr exposes key-driven tab reordering; `Alt+P` is intentionally free for OMP agent selection
- `Alt+I/O` — previous/next tab
- `Alt+?` — keybindings/help

## Migration state

The Herdr-first path is implemented with:

- `bin/aoc-herdr-install` — installs the lean Herdr config baseline, installs the Herdr OMP integration when `omp` is available, installs the AOC-aware plain `omp` shim, installs the OMP assets declared in `.omp/manifest.toml` (extensions, skills, and specialist agent templates), and installs the read-only AOC Mind OMP extension without legacy Mind cockpit/runtime assets
- `bin/aoc-herdr-launch` — launches/focuses Herdr for the current project root, reusing an existing workspace for the same root when possible; Services workspace ensure/focus is opt-in through `AOC_HERDR_SERVICES=auto|focus` or `aoc services`
- `bin/aoc-herdr-services` — creates/focuses the project-scoped Herdr AOC Services workspace without starting Herdr behind the operator's back
- `bin/aoc-omp-context` — renders the compact metadata-only AOC startup capsule for OMP `--append-system-prompt`, including detected VCS mode and preferred command family
- `bin/aoc-omp` / `aoc omp` — launches upstream OMP with that startup capsule already appended
- `bin/aoc` — delegates to Herdr by default and exposes `aoc services`
- `install.sh` — defaults to Herdr/OMP assets and skips legacy Zellij cockpit assets unless `--legacy-zellij` is passed
- `bin/aoc-init` — installs the AOC OMP extensions, skills, and agent templates declared in `.omp/manifest.toml`, seeds the lean OMP tool baseline, and skips legacy `.aoc/layouts`, Zellij plugin repair, subagent manager extension, AOC agent presence extension, Mission Control, and Control pane defaults
- `/commit [scope]` — OMP slash command that triggers the safe atomic commit workflow: inspect changes, run targeted validation, use the handshake's preferred VCS workflow, commit only the intended atomic work, report the Git SHA or Jujutsu change/commit identity, and after a successful commit perform best-effort `codegraph sync` when a project `.codegraph/` index and CLI are present; colocated repos with an attached Git branch use Git staging; never push

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

## OMP AOC extensions, skills, and agents

`.omp/manifest.toml` is the canonical repo-owned OMP asset inventory. `aoc-init` and `aoc-herdr-install` read that manifest and copy its `extensions`, `skills`, and `agents` entries into the active OMP agent directory:

```text
${AOC_OMP_AGENT_DIR:-~/.omp/agent}/extensions/
${AOC_OMP_AGENT_DIR:-~/.omp/agent}/skills/
${AOC_OMP_AGENT_DIR:-~/.omp/agent}/agents/
```

Do not maintain a second active inventory in this document or under `.aoc/skills`; `.aoc/skills` is legacy/archive-only content, not an OMP runtime source. Runtime skill sources are `.omp/skills` entries named by `.omp/manifest.toml`.

The manifest-owned extension surface includes the operational tools and slash commands Herdr expects, including `aoc-codegraph.ts`, `aoc-mind.ts`, `aoc-commit.ts`, `aoc-state.ts`, `aoc-dox.ts`, `aoc-dox-command.ts`, `aoc-herdr.ts`, `aoc-master.ts`, `aoc-jj-init.ts`, `aoc-brand-content.ts`, `aoc-web-search.ts`, and `ponytail.ts`. The same manifest owns the installed OMP skill set, including the `ponytail`, `ponytail-review`, `ponytail-audit`, `ponytail-debt`, and `ponytail-help` skills.

`aoc-master.ts` registers `/master on [minutes]`, `/master off`, and `/master status` plus the gated `aoc_orchestrate` tool. `/master` routes through the agent turn; `aoc_orchestrate master_on` creates a Herdr-session/workspace lease owned by the current pane, and only that lease owner may `assign` or `send` bounded text to peer agents. Peer mutation is intentionally limited to `herdr agent send`: no shell commands, keystrokes, focus, pane control, spawning, move/resize, or broadcast actions are exposed. Use `aoc_herdr` first for grounded observation, then `aoc_orchestrate` only for explicit peer assignments/messages.

`aoc-codegraph.ts` exposes the read-only `aoc_codegraph` tool for code discovery: `status`, `files`, `search`, `context`, `callers`, `callees`, `impact`, and `affected`.
Use this as the agent graph/context tool in Herdr/OMP workspaces; Understand-Anything is not part of the active graph path.

`aoc-mind.ts` exposes the read-only `aoc_mind` tool for historical/provenance intelligence: `status`, `evidence`, `provenance`, and dry-run `mnemopi_candidates`. It augments OMP/Mnemopi with cited AOC Mind evidence; it does not write memories or inject broad startup context.

`aoc-commit.ts` registers `/commit` for safe atomic commits. It uses Git staging whenever `aoc-handshake --json` reports Git as the preferred tool, including colocated Jujutsu+Git workspaces with an attached Git branch; it uses Jujutsu current-change/fileset workflows only when Jujutsu is the preferred tool. CodeGraph refresh is post-commit advisory cache maintenance only; it does not initialize/index new projects.

`aoc-jj-init.ts` registers `/jj-init` for explicit Jujutsu setup. It asks the agent to inspect Git/JJ state and dirty work before running `jj git init --colocate`; ordinary `aoc-init` and startup handshakes only detect/report Jujutsu.

`aoc-brand-content.ts` registers `/brand-content` and `/hyperframes-director` for the branded HyperFrames/html-video pipeline. The related OMP agents are `brand-strategy`, `brand-concept`, `svg-asset`, and `hyperframes-content`.

`aoc-web-search.ts` exposes the `aoc_web_search` tool backed by `aoc-search`, so OMP agents can use local SearXNG or direct package/GitHub modes when built-in web-search providers are out of credits, unauthorized, or timing out.
Agents must not use `aoc_codegraph` to initialize, index, or sync projects. Operators run those commands explicitly, for example:

```bash
codegraph sync /path/to/project
```
