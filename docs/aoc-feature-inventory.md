# AOC Feature Inventory

This inventory defines the Herdr/OMP cutover target. AOC should become a project/tooling layer, not a competing workspace UI.

## Keep

| Feature | Current files / commands | Future owner | Notes |
|---|---|---|---|
| Familiar launcher | `bin/aoc`, `bin/aoc-herdr-launch` | AOC + Herdr | `aoc` should launch/focus Herdr. |
| OMP launcher/context | `bin/aoc-omp`, `bin/aoc-omp-context`, `bin/aoc-handshake --prompt/--json` | AOC + OMP | Metadata-only startup capsule; no broad Mind injection. |
| Taskmaster integration | `bin/aoc-task`, `bin/tm`, `bin/aoc-tm`, `crates/aoc-taskmaster` | AOC | Keep as task/spec source of truth. |
| CodeGraph | `.omp/extensions/aoc-codegraph.ts`, `codegraph` CLI | OMP extension | Read-only agent discovery; indexing/sync remains operator-controlled. |
| Agent wrapper if useful for OMP | `bin/aoc-agent-wrap`, `crates/aoc-agent-wrap-rs` | AOC + OMP | Keep only if it can wrap OMP cleanly for lifecycle/context/provenance without Zellij coupling. |
| HyperFrames | `bin/aoc-hf`, `bin/aoc-hf-u`, `bin/aoc-hyperframes`, `docs/hyperframes.md`, related skills | AOC tooling | Keep. |
| OpenDesign | `bin/aoc-od`, `docs/open-design.md`, related skills | AOC tooling | Keep. |
| Web research | `.omp/extensions/aoc-web-search.ts`, `bin/aoc-search`, `docs/web-research.md`, `scripts/test-web-research-stack.sh`, related skills/scripts | OMP extension + AOC tooling | Keep. Local fallback for agent web search when paid/native providers fail. |
| AOC Services workspace | `bin/aoc-herdr-services`, `bin/aoc-services`, `aoc services`, `docs/operator/aoc-services.md` | Herdr + AOC tooling | Retained Herdr runtime owner for project-scoped service health/startup, especially managed local SearXNG. Distinct from retired Mission Control/status UI. |
| RTK | `bin/aoc-rtk`, `bin/aoc-rtk-proxy`, `docs/reference/rtk-routing.md` | AOC tooling | Keep only for allowlisted noisy-command routing with raw-output preservation. |
| AOC Mind evidence for Mnemopi | `.omp/extensions/aoc-mind.ts`, `crates/aoc-mind`, `crates/aoc-storage`, `aoc-mind-service serve/evidence-pack/mnemopi-candidates` | AOC + OMP/Mnemopi | Default project background T0/T1/T2/T3 processing plus lazy focused evidence/provenance; no startup injection or automatic Mnemopi writes. |
| Selected skills/prompts/docs | `.omp/skills`, docs | AOC tooling | Keep only if they complement Herdr/OMP workflows. |

## Remove / retire from default AOC

These are out of the default Herdr/OMP stack. Transitional compatibility may remain behind explicit legacy flags, but default install/init/launch paths must not install, start, or require them.

| Feature | Current files / commands | Replacement owner | Notes |
|---|---|---|---|
| Zellij cockpit launcher | `bin/aoc-launch`, `bin/aoc-new-tab`, `AOC_LEGACY_ZELLIJ=1 aoc` | Herdr | Retired from default; compatibility requires `AOC_LEGACY_ZELLIJ=1`. |
| AOC tab bar / top bar | removed | Herdr | Purged; Herdr owns workspace/status UI. |
| Zellij layouts/keybindings | `zellij/aoc.config.kdl.template`, `zellij/layouts/aoc.kdl.template`, `.aoc/layouts/*.kdl`, `bin/aoc-layout`, `bin/aoc-zellij.sh`, `bin/aoc-zellij-resize` | Herdr | Managed layout seeding removed from default install/init. |
| Mission Control | `bin/aoc-mission-control`, `bin/aoc-mission-control-tab`, `bin/aoc-mission-control-toggle`, `crates/aoc-mission-control`, `.aoc/layouts/mission-control.kdl`, docs/operator Mission Control docs | Herdr + OMP | Not built by default. Remove rather than port; overlapping functionality belongs in Herdr/OMP. |
| AOC subagent manager/control surfaces | `bin/aoc-subagent-supervision*`, docs/reference/subagent-runtime.md` | OMP | Not seeded by default. OMP owns subagent orchestration. |
| Control pane | `bin/aoc-control`, `bin/aoc-control-toggle`, `crates/aoc-control`, `docs/control-pane.md` | Herdr | Not built by default. Remove floating/control pane UX. |
| Agent status surfaces | `bin/aoc-agent`, `bin/aoc-agent-run`, `bin/aoc-agent-install` UI/status behavior, retired Pi presence extension | Herdr + OMP | Presence extension not seeded by default; status display belongs to Herdr/OMP. |
| Legacy pane/workspace/session health UI | `bin/aoc-session-state`, `bin/aoc-pane-evidence`, `bin/aoc-pulse-pane`, `bin/aoc-hub`, `crates/aoc-hub-rs`, Pulse/session docs | Herdr | Legacy hub/status UI not installed by default; `bin/aoc-services` is retained for the Herdr AOC Services workspace. |
| Tab/project metadata | `bin/aoc-tab-metadata`, `bin/aoc-tab-group`, `bin/aoc-pane-rename`, layout metadata sync calls | Herdr | Not required by default install; Herdr workspaces/tabs/panes are the metadata source. |
| Zellij cleanup/inventory | `bin/aoc-cleanup`, `bin/aoc-cleanup-core.py`, Zellij inventory helpers | Herdr | Retire with Zellij cockpit. |
| Mind startup/cockpit dependency | startup broad context packs, always-on cockpit Mind service expectations | AOC optional recall | Optional behind `--mind` / `AOC_INIT_MIND_RUNTIME=1`; no startup injection or workspace dependency. |

## Installer cutover requirements

Default install must become lean and must not install old cockpit assets.

### Default Herdr/OMP install should include

- Herdr config baseline: `herdr/config.toml`
- OMP integration: `herdr integration install omp` where available
- AOC OMP context commands: `aoc-omp`, `aoc-omp-context`, `aoc-handshake`
- Taskmaster commands: `tm`, `aoc-task`, `aoc-tm`
- CodeGraph OMP extension: `.omp/extensions/aoc-codegraph.ts`
- AOC Mind OMP extension: `.omp/extensions/aoc-mind.ts` for read-only status/evidence/provenance/candidate memory synthesis
- Kept tooling: HyperFrames, OpenDesign, web research, RTK if selected
- Herdr AOC Services workspace command: `aoc services` / `bin/aoc-herdr-services`

### Default install no longer does

- requiring/installing Zellij
- generating `~/.config/zellij/layouts/aoc.kdl`
- generating `~/.config/zellij/aoc.config.kdl`
- installing removed Zellij top-bar assets
- building/installing Mission Control and Control pane binaries by default
- seeding Zellij-specific `.aoc/layouts/*.kdl`
- installing AOC subagent control surfaces as default agent infrastructure
- installing broad Mind prompt injection or automatic Mnemopi promotion by default

## Remaining removal order

1. Keep legacy installer/launcher access explicitly opt-in (`./install.sh --legacy-zellij`, `AOC_LEGACY_ZELLIJ=1 aoc`) until compatibility users are migrated.
2. Delete Mission Control, Control pane, subagent UI, agent status UI, session health UI, and tab metadata code after no explicit legacy users remain.
3. Re-evaluate `aoc-agent-wrap-rs` for an OMP-native lifecycle/context role; keep only if Zellij-independent.
