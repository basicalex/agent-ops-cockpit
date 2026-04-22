# AOC Promoted Pane / Mux PRD (RPG)

## Zellij 0.44 Update Changes (Exact Delta)

This PRD has been re-scoped around **Zellij 0.44 native pane control**.

Exact update changes from pre-0.44 assumptions:
- `aoc-mux` still marks an explicitly agent-controllable pane, but it no longer implies a mandatory nested tmux domain.
- AOC should use `list-panes --json`, `list-tabs --json`, and `current-tab-info --json` as the primary inventory layer.
- AOC should use `dump-screen --pane-id` and `subscribe --pane-id` for bounded pane observation and live drilldown.
- AOC should use `paste --pane-id` and `send-keys --pane-id` for bounded mutating control of explicitly promoted panes.
- tmux remains optional only for fallback, inner-domain workflows, or cases where extra process isolation still matters.

Source alignment note: see `docs/research/zellij-0.44-aoc-alignment.md` for the grounded release/doc audit and the exact AOC-side implementation differences.

## Problem Statement
AOC’s default Zellij workspace already gives each tab a strong human-first layout: the PI agent pane, Yazi, Taskmaster, Pulse, and a general terminal pane. The missing piece is a safe, reliable way for PI to observe and control selected runtime panes without granting broad automation over the entire workspace.

Before Zellij 0.44, this pushed AOC toward a tmux-first plan because pane targeting, pane metadata, and non-focused pane capture were too brittle to treat as a first-class control substrate.

Zellij 0.44 materially changes that.

AOC can now rely on native pane/tab inventory, targeted pane capture, targeted pane input, and explicit floating/tab state queries. That means the right v1 model is no longer “PI controls a nested tmux island inside Zellij” but rather:

- keep **Zellij as the outer and primary pane control plane**,
- let a developer explicitly promote selected panes into **AOC-controlled panes**,
- keep mutating actions scoped only to those promoted panes and their managed runtimes,
- use tmux only when AOC still needs optional fallback or inner-pane multiplexing.

We need a constrained terminal automation layer where selected Zellij terminal panes can be explicitly promoted into **AOC mux panes**. In this updated PRD, “mux pane” means an explicitly registered AOC-controlled pane with stable session/tab/pane identity and bounded PI tools. Managed runtimes launched inside promoted panes should expose structured metadata and logs so PI can inspect dev servers and helper processes without transcript copy/paste.

## Target Users
- **AOC operator/developer**: wants PI to launch, inspect, and control dev servers or helper terminals without granting PI broad control over the whole workspace.
- **PI agent in the AOC agent pane**: needs reliable, bounded tools for runtime visibility and control in explicitly connected panes.
- **AOC maintainers**: need an incremental architecture that improves automation without replacing Zellij as the workspace shell.
- **Power users with custom layouts**: want layouts with one or more pre-planned agent-controlled panes for frontend, backend, tests, or scratch automation.

## Success Metrics
- A developer can convert the default lower-right terminal pane into an AOC-controlled mux/promoted pane using `aoc-mux`, and PI can discover it reliably.
- PI can list promoted panes and inspect their current state with >= 95% accurate pane/runtime status reporting.
- PI can launch a managed long-running command inside a promoted pane and inspect recent logs with no manual paste steps.
- PI mutating actions are limited to promoted panes and managed runtimes; no writes occur to non-promoted panes in the acceptance flow.
- Pane capture and live drilldown can use native Zellij `dump-screen` / `subscribe` without requiring pane focus hopping.
- A custom layout can include multiple promoted panes for focused dev workflows (eg. frontend, backend, test runner).
- The system remains fail-open: if promoted-pane metadata or native Zellij control is unavailable, Zellij still works normally and the pane remains usable as a terminal.

## Current Implementation Baseline (2026-04)
The native Zellij substrate cleanup that this PRD depends on is now substantially in place.

Already landed in repo:
- `crates/aoc-core/src/zellij_cli.rs` provides native session/tab/pane snapshot queries via Zellij 0.44 JSON APIs.
- `bin/aoc-zellij.sh` provides native floating-state, pane existence, bounded `dump-screen`, and live `subscribe` helpers.
- Mission Control / Overseer operator drilldown already uses bounded pane evidence capture and opt-in live follow.
- Live topology/floating-state hot paths in Mission Control, hub, align, and cleanup have been migrated away from `dump-layout`-first parsing to native Zellij inventory/session snapshot queries.

Not yet landed:
- promoted-pane registry and `aoc-mux`
- managed runtime registry / log-backed runtime lifecycle via `aoc-runtime`
- PI-facing promoted-pane tools for list/capture/follow/logs/run/stop/send
- final docs and acceptance coverage for the promoted-pane workflow

Implication for implementation order: Task 176 should now start from the promoted-pane contract/registry and runtime/log layer, not from another round of topology migration.

---

## Capability Tree

### Capability: Promoted Pane Activation and Identity
Allow a regular Zellij terminal pane to become an explicitly agent-controllable domain.

#### Feature: `aoc-mux` pane promotion
- **Description**: Promote the current Zellij terminal pane into an AOC-controlled pane and register it in project-local runtime state.
- **Inputs**: current shell environment, `ZELLIJ_PANE_ID`, current tab/session context, optional mux name.
- **Outputs**: active promoted pane record with stable AOC identity.
- **Behavior**: capture `session_id`, `tab_id`, `pane_id`, project root, friendly name, and optional role label; leave the pane usable as a normal interactive terminal.

#### Feature: Promoted-pane registry and identity model
- **Description**: Maintain a registry of active promoted panes and their associated control/runtime metadata.
- **Inputs**: activation events, mux names, AOC session ID, tab scope, tab ID, pane ID, project root, runtime refs.
- **Outputs**: structured pane records addressable by stable mux ID and human-friendly name.
- **Behavior**: persist per-pane metadata in project-local runtime state and refresh status on activity, exit, and reconciliation.

#### Feature: Explicit scope boundary
- **Description**: Distinguish promoted panes from all other workspace panes.
- **Inputs**: pane registration state and tool targets.
- **Outputs**: positive identification of controllable vs non-controllable panes.
- **Behavior**: PI mutating tools target only registered promoted panes; non-promoted panes remain outside the v1 control surface.

### Capability: Managed Runtime Lifecycle Inside Promoted Panes
Support long-running dev workflows with structured state.

#### Feature: Managed runtime launcher
- **Description**: Start a long-running process inside a promoted pane through an AOC wrapper that records metadata and tees logs.
- **Inputs**: pane target, runtime name, cwd, command, environment hints.
- **Outputs**: managed runtime record with pid, status, and log location.
- **Behavior**: launch via a thin AOC runtime helper, capture lifecycle metadata, and associate the runtime with the owning promoted pane.

#### Feature: Runtime status and reconciliation
- **Description**: Track whether managed processes are running, exited, failed, or stale.
- **Inputs**: runtime metadata, pid checks, wrapper lifecycle updates, latest output time.
- **Outputs**: compact runtime health summaries and updated registry state.
- **Behavior**: refresh process state deterministically and reconcile stale runtime metadata on every status action.

#### Feature: Managed stop lifecycle
- **Description**: Stop a managed runtime safely through the runtime layer.
- **Inputs**: runtime target and requested action.
- **Outputs**: updated process state and action result.
- **Behavior**: use pid-aware shutdown where possible and require policy checks for disruptive actions.

### Capability: Native Pane Context Retrieval
Let PI observe promoted panes and managed runtimes without transcript copy/paste.

#### Feature: Managed log tail access
- **Description**: Read recent stdout/stderr from managed runtimes in promoted panes.
- **Inputs**: runtime identifier, line/byte limits.
- **Outputs**: truncated recent log content with provenance.
- **Behavior**: tail logs by default, preserve full files on disk, and return bounded context to PI.

#### Feature: Native pane snapshot capture
- **Description**: Capture current viewport or scrollback from a specific promoted pane using Zellij-native CLI targeting.
- **Inputs**: pane target, capture mode, line/byte limits, ANSI preference.
- **Outputs**: bounded textual pane snapshot.
- **Behavior**: use `zellij action dump-screen --pane-id ...` as the preferred live-view mechanism when managed logs are unavailable or insufficient.

#### Feature: Native live pane subscription
- **Description**: Stream live updates from a promoted pane for explicit drilldown or follow mode.
- **Inputs**: pane target, scrollback depth, ANSI preference, operator/tool cancellation.
- **Outputs**: bounded live pane events or summarized live tail.
- **Behavior**: use `zellij subscribe --pane-id ... --format json` only for explicit live inspection, not as the default telemetry substrate.

#### Feature: Smart source selection
- **Description**: Choose the best context source automatically.
- **Inputs**: runtime registry, promoted-pane registry, tool parameters.
- **Outputs**: log-backed, snapshot-backed, or subscription-backed context result with provenance.
- **Behavior**: prefer managed runtime logs for long-running processes, then native pane snapshots for live shell state, then explicit live subscribe when the user or tool asks to follow activity.

### Capability: Native Pane Command and Control
Allow PI to act inside promoted panes without requiring general workspace control.

#### Feature: Paste text/command to promoted pane
- **Description**: Write text into a promoted pane and optionally submit it.
- **Inputs**: pane target, text payload, submit flag.
- **Outputs**: delivery result and audit metadata.
- **Behavior**: route through `zellij action paste --pane-id`, optionally followed by `send-keys Enter`, scope to promoted panes only, and surface failures clearly.

#### Feature: Send bounded control keys to promoted pane
- **Description**: Send human-readable keys such as `Enter`, `Ctrl c`, or `Up` to a promoted pane.
- **Inputs**: pane target, key sequence.
- **Outputs**: delivery result and audit metadata.
- **Behavior**: route through `zellij action send-keys --pane-id`, scope to promoted panes only, and require policy checks for risky actions.

#### Feature: Read-only promoted-pane inventory
- **Description**: Return a compact list of promoted panes and their contained managed runtimes.
- **Inputs**: pane registry, runtime registry, native Zellij pane/tab inventory.
- **Outputs**: addressable inventory for PI and the operator.
- **Behavior**: surface mux IDs, session/tab/pane IDs, friendly names, owning AOC metadata, and runtime summaries.

### Capability: PI Tool Surface and Safety Policy
Expose the workflow cleanly and safely to PI.

#### Feature: PI promoted-pane tools
- **Description**: Register PI tools for pane listing, capture, live follow, logs, send, run, status, and stop.
- **Inputs**: tool calls from PI.
- **Outputs**: structured tool results and optional streamed updates.
- **Behavior**: follow PI tool contracts, truncate output, and keep mutating actions scoped to promoted panes and managed runtimes.

#### Feature: Observability-first rollout
- **Description**: Phase read-only and mutating capabilities deliberately.
- **Inputs**: approved scope and implementation phase.
- **Outputs**: ordered delivery path for v1 and later expansion.
- **Behavior**: deliver discovery/status/log/capture first, then add managed run/stop/paste/send-key control once the pane substrate is stable.

#### Feature: Safety and confirmation policy
- **Description**: Gate disruptive runtime actions.
- **Inputs**: action type, target type, runtime state, user policy.
- **Outputs**: allow/confirm/deny decisions.
- **Behavior**: default to observability-first behavior; require confirmation for stop/restart/high-risk send scenarios.

### Capability: Optional tmux Compatibility Layer
Preserve tmux only where it still adds real value.

#### Feature: Optional tmux-backed inner domain
- **Description**: Allow a promoted pane to host an optional tmux-backed control domain when needed.
- **Inputs**: operator preference, compatibility mode, pane target.
- **Outputs**: optional nested control domain metadata.
- **Behavior**: do not require tmux for normal promoted-pane operation; use it only for fallback, inner multiplexing, or niche workflows.

#### Feature: Compatibility fallback policy
- **Description**: Define when AOC should degrade to tmux-backed helpers or compatibility mode.
- **Inputs**: Zellij version/capability probe, operator config, feature target.
- **Outputs**: explicit mode choice with provenance.
- **Behavior**: prefer Zellij-native control on 0.44+, fall back cleanly when capabilities are absent or unreliable.

### Capability: Layout and Workflow Integration
Make promoted panes a natural part of AOC layouts.

#### Feature: Default terminal-to-promotion path
- **Description**: Support the existing lower-right terminal pane as the simplest first promoted-pane candidate.
- **Inputs**: default AOC layout and operator activation command.
- **Outputs**: a documented standard path for activating agent control in today’s layout.
- **Behavior**: preserve the current layout while making pane promotion explicit and incremental.

#### Feature: Custom layouts with preplanned promoted panes
- **Description**: Support optional layouts that start with one or more dedicated promoted panes.
- **Inputs**: layout definitions and pane naming conventions.
- **Outputs**: dev/test-focused layouts with predictable agent-controlled panes.
- **Behavior**: allow future layouts such as frontend/backend/test panes without requiring broad workspace control.

---

## Repository Structure (Target)

```text
project-root/
├── .pi/
│   └── extensions/
│       └── aoc-mux/
│           ├── index.ts              # PI extension entrypoint
│           ├── tools.ts              # PI pane tool registration and handlers
│           ├── zellij.ts             # Zellij 0.44 helper operations
│           ├── mux-registry.ts       # promoted-pane metadata helpers
│           ├── runtime-registry.ts   # managed runtime metadata/log helpers
│           ├── policy.ts             # safety and confirmation rules
│           └── tmux.ts               # optional compatibility helpers only
├── bin/
│   ├── aoc-mux                       # promote/inspect the current pane as an AOC-controlled pane
│   └── aoc-runtime                   # thin runtime wrapper for managed processes inside promoted panes
├── .aoc/
│   └── runtime/
│       ├── mux-registry.json         # active promoted-pane index
│       ├── runtimes.json             # managed runtime index
│       ├── mux-<id>.json             # per-pane metadata
│       ├── runtime-<name>.json       # per-runtime metadata
│       └── runtime-<name>.log        # managed runtime logs
├── docs/
│   ├── agent-extensibility.md        # updated PI tool/operator guidance
│   ├── mux-pane-ops.md               # promoted-pane workflow and architecture guide
│   └── research/
│       └── zellij-0.44-aoc-alignment.md
└── zellij/
│   └── layouts/                      # Zellij remains the outer human-facing workspace shell
```

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **Promoted-pane identity contract**: defines mux ID, session/tab/pane linkage, and scope boundaries.
- **Runtime registry schema**: defines managed runtime metadata, status fields, and file layout.
- **Safety boundary contract**: defines that v1 mutating actions target promoted panes only.
- **Zellij capability gate**: defines the required 0.44+ commands and compatibility fallback behavior.

### Activation Layer (Phase 1)
- **`aoc-mux` promotion flow**: depends on [Promoted-pane identity contract, Safety boundary contract, Zellij capability gate]
- **Pane registry persistence helpers**: depends on [Promoted-pane identity contract]

### Runtime Layer (Phase 2)
- **Runtime wrapper (`aoc-runtime`)**: depends on [Runtime registry schema, Promoted-pane identity contract]
- **Runtime reconciliation/status helpers**: depends on [Runtime registry schema]

### Native Zellij Control Layer (Phase 3)
- **Zellij helper module**: depends on [Promoted-pane identity contract, Zellij capability gate]
- **Inventory/capture/input primitives**: depends on [Zellij helper module]
- **Optional tmux compatibility helper**: depends on [Zellij capability gate]

### PI Tool Layer (Phase 4)
- **PI read-only pane tools**: depends on [`aoc-mux` promotion flow, Pane registry helpers, Runtime reconciliation, native Zellij capture primitives]
- **PI mutating pane tools**: depends on [PI read-only pane tools, Runtime wrapper, native Zellij paste/send-keys primitives, Safety boundary contract]
- **Safety/confirmation policy**: depends on [Safety boundary contract, PI mutating pane tools]

### UX and Validation Layer (Phase 5)
- **Docs and layout integration**: depends on [PI read-only pane tools, PI mutating pane tools]
- **Smoke and acceptance coverage**: depends on [`aoc-mux` promotion flow, Runtime wrapper, PI tool layers]

---

## Development Phases

### Completed Baseline: Native Zellij Topology Substrate Cleanup
- Consolidate live inventory/floating-state flows on Zellij 0.44 native APIs (`list-panes`, `list-tabs`, `current-tab-info`, session snapshots).
- Remove `dump-layout`-first parsing from hot operator/runtime-discovery paths so future promoted-pane work builds on the native substrate.
- Exit criteria: Mission Control, hub, align, and cleanup no longer rely on `dump-layout` as their primary topology source.

### Phase 0: Contract and Capability Gate
- Define promoted-pane identity, runtime schema, `.aoc/runtime` registry layout, Zellij 0.44 capability requirements, scope boundaries, and v1 non-goals.
- Exit criteria: approved PRD and task breakdown with explicit promoted-pane-only mutation boundaries.

### Phase 1: Pane Promotion
- Implement `aoc-mux` and registry persistence so a regular terminal pane can become a discoverable promoted pane.
- Exit criteria: the lower-right terminal pane can be promoted into a discoverable AOC-controlled pane.

### Phase 2: Managed Runtime Foundation
- Implement `aoc-runtime` and runtime persistence for processes launched inside promoted panes.
- Exit criteria: a managed command can be launched, logged, and queried outside PI.

### Phase 3: Read-Only Native Pane Visibility
- Implement PI tools for listing promoted panes, viewing status, capturing pane state, following live output, and reading runtime logs.
- Exit criteria: PI can reliably discover and inspect promoted panes and managed runtimes using native Zellij operations.

### Phase 4: Mutating Native Pane Controls
- Implement managed run/stop/paste/send-keys actions scoped to promoted panes.
- Exit criteria: PI can launch and control managed workflows inside promoted panes without touching non-promoted panes.

### Phase 5: Layouts, Docs, and Validation
- Add docs, smoke coverage, and optional custom layouts with preplanned promoted panes.
- Exit criteria: operator acceptance flow passes and documented dev/test layouts exist or are clearly specified.

---

## Test Strategy
- **Unit/logic tests**: validate registry handling, runtime status transitions, pane target resolution, capability gating, and policy decisions.
- **Capability probe tests**: verify Zellij 0.44 feature detection for `list-panes`, `list-tabs`, `dump-screen`, `subscribe`, `paste`, and `send-keys`.
- **Activation tests**: verify `aoc-mux` registers the pane, records session/tab/pane IDs, and preserves normal terminal usability.
- **Wrapper integration tests**: verify `aoc-runtime run/status/stop` lifecycle, log teeing, and metadata persistence inside promoted panes.
- **Native Zellij smoke tests**: verify pane inventory, `dump-screen`, `subscribe`, `paste`, and `send-keys` behavior against a live promoted pane.
- **PI extension smoke tests**: confirm tool registration, output truncation, target scoping, and safe handling of unavailable panes/runtimes.
- **Operator acceptance flow**:
  1. Start AOC in the default layout.
  2. Run `aoc-mux` in the lower-right terminal pane.
  3. Verify PI can list the promoted pane.
  4. Launch a managed frontend or backend server through PI.
  5. Inspect recent logs through PI.
  6. Capture pane output and optionally follow live output through PI.
  7. Send a bounded command through paste/send-keys.
  8. Stop the managed runtime cleanly.
  9. Confirm no non-promoted pane received mutating control.

## Risks and Mitigations
- **Risk**: users assume PI can control the whole Zellij tab.
  - **Mitigation**: enforce and document the promoted-pane-only control boundary in v1.
- **Risk**: native pane capture becomes a noisy substitute for structured runtime state.
  - **Mitigation**: keep managed runtime logs/metadata as primary truth for long-running processes.
- **Risk**: runtime metadata drifts from actual process state.
  - **Mitigation**: refresh from pid state and reconcile status on every runtime action.
- **Risk**: new promoted-pane code reintroduces topology drift or rebuilds old `dump-layout` assumptions in parallel.
  - **Mitigation**: centralize Zellij JSON/session-snapshot queries in the canonical helper layer and do not add new `dump-layout`-based live paths.
- **Risk**: some workflows still need tmux-like inner multiplexing.
  - **Mitigation**: keep tmux as an optional compatibility layer rather than the default architecture.

## Non-Goals
- Replacing Zellij as the human-facing workspace shell in this scope.
- Granting PI unrestricted keystroke-level control over arbitrary non-promoted panes.
- Making raw pane capture the canonical runtime source when managed metadata can exist.
- Fully automating interactive TUIs beyond launch/basic capture in v1.
- Requiring tmux as the default inner control substrate for useful runtime workflows on Zellij 0.44+.
