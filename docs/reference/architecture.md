# AOC Architecture

Canonical product architecture and crate boundary definitions for Agent Ops Cockpit.

## Current model

AOC is a Herdr-first project/tooling layer:

- **Herdr** owns workspaces, tabs, panes, navigation, agent status, and visible workspace health.
- **OMP** owns subagent orchestration and repo-installed agent/skill/extension surfaces.
- **AOC** owns project setup, task workflows, launch convenience, optional services, and retained standalone tools.
- **AOC Mind** remains optional focused evidence/provenance, not a default startup injector or cockpit dependency.

## Kept product surfaces

| Surface | Owner | Notes |
|---|---|---|
| Workspace launch | Herdr + `aoc` | `aoc` opens/focuses the Herdr project workspace. |
| Services workspace | Herdr + AOC | `aoc services` owns visible project-scoped runtime/service status. |
| OMP startup capsule | AOC + OMP | `aoc-omp-context`, `aoc-omp`, and the OMP shim provide metadata-only startup context. |
| Taskmaster | AOC | `tm`, `aoc-task`, and `aoc-tm` remain task/spec surfaces. |
| CodeGraph | OMP extension | Read-only repo discovery when a project has an index. |
| Mind | AOC + OMP/Mnemopi | Optional focused evidence/provenance through `aoc-mind`, `aoc-storage`, and `aoc-mind-service`. |
| Pi JSONL normalization/import semantics | `aoc-pi-adapter` | Kept because `aoc-mind` depends on it. |
| Standalone tools | AOC | HyperFrames, OpenDesign, web research, RTK, and selected handoff helpers. |

## Removed product surfaces

Retired cockpit surfaces are documented only in `docs/deprecations.md` and `docs/aoc-feature-inventory.md`. They are not active architecture dependencies:

- Mission Control fleet/operator UI.
- Control pane.
- Legacy hub/session health UI.
- AOC subagent manager/control UI.
- Layout/tab metadata systems that duplicated Herdr workspace state.
- Retired cockpit launcher/layout/keybinding/cleanup assets.

## Mind data flow

```text
Pi session/import/compaction data
   ↓
aoc-pi-adapter
   ↓
aoc-mind / aoc-mind-service
   ↓
.aoc/mind/project.sqlite
   ↓
OMP extension / focused evidence-pack / provenance query surfaces
```

The Mind path is project-scoped and opt-in. It does not require retired cockpit UI, hub, or pane/session status surfaces.

## Compatibility boundary

`aoc-agent-wrap-rs` may remain only where it provides OMP-native lifecycle/context/provenance value without coupling AOC to retired cockpit surfaces. New architecture should prefer direct Herdr, OMP, and AOC CLI surfaces over compatibility hosts.
