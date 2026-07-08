# AOC Feature Inventory

This inventory defines the Herdr/OMP cutover target. AOC should become a project/tooling layer, not a competing workspace UI.

## Keep

| Feature | Current files / commands | Future owner | Notes |
|---|---|---|---|
| Familiar launcher | `bin/aoc`, `bin/aoc-herdr-launch` | AOC + Herdr | `aoc` should launch/focus Herdr. |
| OMP launcher/context | `bin/aoc-omp`, `bin/aoc-omp-context`, `bin/aoc-handshake --prompt/--json` | AOC + OMP | Metadata-only startup capsule; no broad Mind injection. |
| Taskmaster integration | `bin/aoc-task`, `bin/tm`, `bin/aoc-tm`, `crates/aoc-taskmaster` | AOC | Keep as task/spec source of truth. |
| CodeGraph | `.omp/extensions/aoc-codegraph.ts`, `codegraph` CLI | OMP extension | Read-only agent discovery; indexing/sync remains operator-controlled. |
| Master orchestration | `.omp/extensions/aoc-master.ts`, `/master`, `aoc_orchestrate` | OMP extension + Herdr | Keep as gated peer coordination. Requires an active master lease for the current pane; mutating surface is bounded text sends only, after read-only observation through native `herdr` and the `herdr-agent-observation` skill. |
| HyperFrames | `bin/aoc-hf`, `bin/aoc-hf-u`, `bin/aoc-hyperframes`, `docs/hyperframes.md`, related skills | AOC tooling | Keep. |
| OpenDesign | `bin/aoc-od`, `docs/open-design.md`, related skills | AOC tooling | Keep. |
| Web research | `.omp/extensions/aoc-web-search.ts`, `bin/aoc-search`, `docs/web-research.md`, `scripts/test-web-research-stack.sh`, related skills/scripts | OMP extension + AOC tooling | Keep. Local fallback for agent web search when paid/native providers fail. |
| AOC Services workspace | `bin/aoc-herdr-services`, `bin/aoc-services`, `aoc services`, `docs/operator/aoc-services.md` | Herdr + AOC tooling | Retained Herdr runtime owner for project-scoped service health/startup, especially managed local SearXNG. Distinct from retired Mission Control/status UI. |
| RTK | `bin/aoc-rtk`, `bin/aoc-rtk-proxy`, `docs/reference/rtk-routing.md` | AOC tooling | Keep only for allowlisted noisy-command routing with raw-output preservation. |
| Selected skills/prompts/docs | `.omp/skills`, docs | AOC tooling | Keep only if they complement Herdr/OMP workflows. |

## Remove / retire from default AOC

These items have been removed from the active Herdr/OMP stack. Historical compatibility notes are retained only to identify retired surfaces; default install/init/launch paths do not install, start, or require them.

| Feature | Current files / commands | Replacement owner | Notes |
|---|---|---|---|
| Zellij cockpit launcher | removed: `bin/aoc-launch`, `bin/aoc-new-tab`; legacy flag `AOC_LEGACY_ZELLIJ=1 aoc` no longer describes the default path | Herdr | Removed from the active launcher surface; `aoc` is Herdr-first. |
| AOC tab bar / top bar | removed | Herdr | Purged; Herdr owns workspace/status UI. |
| Zellij layouts/keybindings | removed: `zellij/aoc.config.kdl.template`, `zellij/layouts/aoc.kdl.template`, `.aoc/layouts/*.kdl`, `bin/aoc-layout`, `bin/aoc-zellij.sh`, `bin/aoc-zellij-resize` | Herdr | Removed; Herdr owns layouts, panes, tabs, and keybinding UX. |
| Mission Control | removed: `bin/aoc-mission-control`, `bin/aoc-mission-control-tab`, `bin/aoc-mission-control-toggle`, `crates/aoc-mission-control`, `.aoc/layouts/mission-control.kdl`, operator Mission Control docs | Herdr + OMP | Removed rather than ported; overlapping functionality belongs in Herdr/OMP. |
| AOC subagent manager/control surfaces | removed: `bin/aoc-subagent-supervision*`, `docs/reference/subagent-runtime.md` | OMP | Removed; OMP owns subagent orchestration. |
| Control pane | removed: `bin/aoc-control`, `bin/aoc-control-toggle`, `crates/aoc-control`, `docs/control-pane.md` | Herdr | Removed; Herdr and direct CLI surfaces own operator actions. |
| Legacy pane/workspace/session health UI | removed: `bin/aoc-session-state`, `bin/aoc-pane-evidence`, `bin/aoc-pulse-pane`, `bin/aoc-hub`, `crates/aoc-hub-rs`, Pulse/session docs | Herdr | Removed; `bin/aoc-services` is retained for the Herdr AOC Services workspace. |
| Tab/project metadata | `bin/aoc-tab-metadata`, `bin/aoc-tab-group`, `bin/aoc-pane-rename`, layout metadata sync calls | Herdr | Not required by default install; Herdr workspaces/tabs/panes are the metadata source. |
| Zellij cleanup/inventory | removed: `bin/aoc-cleanup`, `bin/aoc-cleanup-core.py`, Zellij inventory helpers | Herdr | Removed with the Zellij cockpit. |

## Installer cutover requirements

Default install must become lean and must not install old cockpit assets.

### Default Herdr/OMP install should include

- Herdr config baseline: `herdr/config.toml`
- OMP integration: `herdr integration install omp` where available
- AOC OMP context commands: `aoc-omp`, `aoc-omp-context`, `aoc-handshake`
- Taskmaster commands: `tm`, `aoc-task`, `aoc-tm`
- CodeGraph OMP extension: `.omp/extensions/aoc-codegraph.ts`
- Kept tooling: HyperFrames, OpenDesign, web research, RTK if selected
- Herdr AOC Services workspace command: `aoc services` / `bin/aoc-herdr-services`
- Native Herdr observation skill: `.omp/skills/herdr-agent-observation/SKILL.md`


### Default install no longer does

- requiring/installing Zellij
- generating `~/.config/zellij/layouts/aoc.kdl`
- generating `~/.config/zellij/aoc.config.kdl`
- installing removed Zellij top-bar assets
- building/installing Mission Control, Control pane, or hub binaries
- seeding Zellij-specific `.aoc/layouts/*.kdl`
- installing AOC subagent control surfaces as default agent infrastructure
- installing broad memory prompt injection or automatic Mnemopi promotion by default

## Removal status

The zellij-era launcher/layout scripts, Mission Control, Control pane, subagent UI, hub/session UI, Zellij cleanup surfaces, legacy agent lifecycle scripts, retired project-memory runtime, and lexicon integrations are removed from the active Herdr-first docs and default runtime.
