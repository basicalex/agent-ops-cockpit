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
- selected lightweight handoff helpers only if they remain useful
- selected Pi skills only when they complement the new stack
- docs and install/bootstrap knowledge that can be simplified for Herdr

## What moves out of AOC

Treat these as legacy or transitional:

- managed Zellij layouts
- AOC Zellij tab bar / `zjstatus-aoc`
- AOC Zellij-specific keybindings
- AOC subagent control surfaces
- Mission Control features that duplicate Herdr or OMP
- AOC Mind sidecar/service/runtime
- AOC Mind context-pack system
- heavy install steps that existed only to support the old cockpit stack

## New model

- **Herdr** owns workspaces, tabs, panes, navigation, and agent status.
- **OMP** owns subagent orchestration.
- **AOC** becomes the compatibility/tooling layer around project setup, task workflows, and launch convenience.

The `aoc` command should eventually launch/focus the Herdr workspace instead of starting the old Zellij layout system.

A Herdr-first install should be much smaller than the old AOC install. It should not install or initialize old Zellij cockpit assets, the custom top bar, AOC subagent UI, or AOC Mind services by default.

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

## Migration intent

The first migration step is implemented with:

- `bin/aoc-herdr-install` — installs the lean Herdr config baseline and installs the Herdr OMP integration when `omp` is available
- `bin/aoc-herdr-launch` — launches/focuses Herdr for the current project root, reusing an existing workspace for the same root when possible
- `bin/aoc` — now delegates to Herdr by default

The old Zellij cockpit remains available during transition with:

```bash
AOC_LEGACY_ZELLIJ=1 aoc
```

A follow-up install redesign should define a reduced Herdr-first dependency set. The default install should be intentionally lean: Herdr, OMP integration, Pi/agent integration as needed, Taskmaster, and only the small AOC compatibility scripts that still prove useful.
