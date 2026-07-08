# AOC Architecture

Canonical product architecture and crate boundary definitions for Agent Ops Cockpit.

## Current model

AOC is a Herdr-first project/tooling layer:

- **Herdr** owns workspaces, tabs, panes, navigation, agent status, and visible workspace health.
- **OMP** owns subagent orchestration and repo-installed agent/skill/extension surfaces.
- **AOC** owns project setup, task workflows, launch convenience, optional services, and retained standalone tools.
- Retained memory/provenance is requested lazily through Mnemopi, Taskmaster, CodeGraph, and AOC CLI surfaces rather than injected at startup.

## Kept product surfaces

| Surface | Owner | Notes |
|---|---|---|
| Workspace launch | Herdr + `aoc` | `aoc` opens/focuses the Herdr project workspace. |
| Services workspace | Herdr + AOC | `aoc services` owns visible project-scoped runtime/service status. |
| OMP startup capsule | AOC + OMP | `aoc-omp-context`, `aoc-omp`, and the OMP shim provide metadata-only startup context. |
| Taskmaster | AOC | `tm`, `aoc-task`, and `aoc-tm` remain task/spec surfaces. |
| CodeGraph | OMP extension | Read-only repo discovery when a project has an index. |
| Retained context | AOC + OMP/Mnemopi | Optional focused task, project-memory, and provenance lookup through supported CLI and OMP surfaces. |
| Standalone tools | AOC | HyperFrames, OpenDesign, web research, RTK, and selected handoff helpers. |

## Removed product surfaces

Retired cockpit surfaces are documented only in `docs/deprecations.md` and `docs/aoc-feature-inventory.md`. They are not active architecture dependencies:

- Mission Control fleet/operator UI.
- Control pane.
- Legacy hub/session health UI.
- AOC subagent manager/control UI.
- Layout/tab metadata systems that duplicated Herdr workspace state.
- Retired cockpit launcher/layout/keybinding/cleanup assets.

## Retained context flow

```text
Agent session/task state
   ↓
Taskmaster / Mnemopi / CodeGraph / AOC CLI
   ↓
Focused evidence pack / retained context / provenance query surfaces
```

Retained context is project-scoped and opt-in. It does not require retired cockpit UI, hub, or pane/session status surfaces.

## Compatibility boundary

New architecture should prefer direct Herdr, OMP, and AOC CLI surfaces over retired compatibility hosts.
