# AOC RTK Integration PRD (RPG)

## Problem Statement
AOC already provides durable context (`.aoc/context.md`), memory (`aoc-mem`), STM (`aoc-stm`), and tasks (`tm`), but command output volume still consumes a large share of model context during active coding sessions. This creates avoidable token cost, slower loops, and lower signal density.

We need an RTK integration that is:
- Agent-agnostic (works for Codex, OpenCode, Gemini, Claude, Kimi)
- Optional and fail-open
- Centrally managed from AOC setup/control flows
- Safe by default (clear bypass and diagnostics)

## Target Users
- Developers using AOC with multiple agent CLIs in Zellij
- Teams that want predictable context efficiency across repositories

## Success Metrics
- 60%+ token reduction on routed high-noise commands in normal dev workflows
- 0 hard failures introduced when RTK is unavailable (fail-open behavior)
- RTK setup discoverable and operable through `Alt+C` (`aoc-control`) without manual file editing
- Existing repositories migrate cleanly by running `aoc-init`

---

## Capability Tree

### Capability: Agent-Agnostic Runtime Routing
Provide a shell-level integration that does not depend on any single model/provider hook implementation.

#### Feature: Command routing policy
- **Description**: Route supported noisy commands through RTK using an allowlist policy.
- **Inputs**: Command invocation, AOC RTK config, RTK availability state.
- **Outputs**: Routed command (RTK) or passthrough command.
- **Behavior**: Match command against allowlist/denylist, then execute RTK or original command.

#### Feature: Fail-open execution
- **Description**: Preserve normal command execution when RTK is missing or errors.
- **Inputs**: RTK command result/availability.
- **Outputs**: Successful fallback to native command path.
- **Behavior**: Never block execution due to RTK integration issues.

#### Feature: Session-local enablement
- **Description**: Enable RTK routing per AOC agent pane/session.
- **Inputs**: `aoc-agent-wrap` environment.
- **Outputs**: PATH/env with RTK proxy active only where intended.
- **Behavior**: Activate in AOC agent context; avoid global side effects.

### Capability: Control Plane Setup and Management
Expose install/config/status flows through `aoc-control` floating pane.

#### Feature: RTK setup wizard entry
- **Description**: Add a first-class RTK action group in Settings.
- **Inputs**: User selection in `aoc-control`.
- **Outputs**: Install/check/config actions executed.
- **Behavior**: Guided flow for initial setup and verification.

#### Feature: Status and diagnostics
- **Description**: Show whether RTK is installed, enabled, and working.
- **Inputs**: `rtk --version`, `rtk gain`, AOC config.
- **Outputs**: Human-readable status in TUI footer/status line.
- **Behavior**: Quick visibility and remediation suggestions.

### Capability: Initialization and Migration
Ensure old and new repositories converge to a consistent baseline.

#### Feature: `aoc-init` seeding
- **Description**: Seed RTK config defaults and docs hints where appropriate.
- **Inputs**: Project root, existing config files.
- **Outputs**: Updated config/contracts without destructive overwrite.
- **Behavior**: Idempotent updates; preserve non-AOC custom content.

#### Feature: Legacy migration
- **Description**: Update prior AOC setups to include new RTK integration contract.
- **Inputs**: Existing AGENTS/config/state files.
- **Outputs**: Updated files on next `aoc-init` run.
- **Behavior**: Safe in-place upgrades using canonical AOC patterns.

### Capability: Observability and Safety
Measure impact while keeping operators in control.

#### Feature: Explicit bypass
- **Description**: Allow disabling RTK quickly for debugging.
- **Inputs**: env flag/config switch.
- **Outputs**: Direct native command execution.
- **Behavior**: Fast toggle without uninstalling RTK.

#### Feature: Output recovery guidance
- **Description**: Support reading full raw logs when RTK output is compacted.
- **Inputs**: RTK tee path or command hints.
- **Outputs**: Recoverable debug workflow.
- **Behavior**: Minimize reruns while retaining debuggability.

---

## Repository Structure

```text
agent-ops-cockpit/
├── bin/
│   ├── aoc-agent-wrap            # Activate per-pane RTK routing context
│   ├── aoc-init                  # Seed/migrate RTK defaults
│   ├── aoc-rtk                   # RTK adapter/status/install helper (new)
│   └── aoc-rtk-proxy             # Command routing wrapper (new)
├── crates/
│   └── aoc-control/
│       └── src/main.rs           # RTK setup/status actions in Alt+C UI
├── docs/
│   └── configuration.md          # RTK config + operational docs
└── .taskmaster/docs/prds/
    └── aoc-rtk_prd_rpg.md        # This PRD
```

## Module Definitions

### Module: `bin/aoc-rtk`
- **Maps to capability**: Control Plane Setup and Management
- **Responsibility**: RTK install/status/check/doctor operations with stable AOC-facing output.
- **Exports/commands**:
  - `aoc-rtk status`
  - `aoc-rtk install`
  - `aoc-rtk doctor`
  - `aoc-rtk enable|disable`

### Module: `bin/aoc-rtk-proxy`
- **Maps to capability**: Agent-Agnostic Runtime Routing
- **Responsibility**: Resolve whether a command should route through RTK or passthrough.
- **Exports/commands**:
  - Internal wrapper contract used by command shims and/or agent-wrap env.

### Module: `bin/aoc-agent-wrap` updates
- **Maps to capability**: Session-local enablement
- **Responsibility**: Activate RTK routing only in managed AOC agent sessions.

### Module: `crates/aoc-control/src/main.rs` updates
- **Maps to capability**: RTK setup wizard entry + diagnostics
- **Responsibility**: Expose RTK actions in `Alt+C` settings UX.

### Module: `bin/aoc-init` updates
- **Maps to capability**: Initialization and Migration
- **Responsibility**: Idempotent seeding and migration of RTK defaults/contracts.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **RTK adapter contract**: canonical command interface and config semantics.
- **Config schema**: enable mode, allowlist/denylist, fail-open defaults.

### Runtime Layer (Phase 1)
- **Proxy and session wiring**: depends on [RTK adapter contract, config schema].

### Control Layer (Phase 2)
- **`aoc-control` RTK UX**: depends on [RTK adapter contract].

### Migration Layer (Phase 3)
- **`aoc-init` seeding/migration**: depends on [config schema, control/runtime choices].

---

## Development Phases

### Phase 0: Adapter + Policy Baseline
**Goal**: Define stable AOC-owned RTK integration contract.

**Tasks**:
- [ ] Create `aoc-rtk` command with `status/install/doctor/enable/disable`.
- [ ] Define config model for mode + allowlist + fail-open behavior.

**Exit Criteria**:
- Adapter works without `aoc-control` integration and reports clear status.

### Phase 1: Agent-Agnostic Runtime Path
**Goal**: Route supported commands through RTK inside AOC agent panes.

**Tasks**:
- [ ] Implement proxy routing with allowlist/denylist.
- [ ] Wire `aoc-agent-wrap` to activate routing in managed sessions.
- [ ] Add bypass mode for debugging (`off`/env toggle).

**Exit Criteria**:
- Commands route correctly in AOC sessions and pass through safely on RTK errors/missing binary.

### Phase 2: Alt+C Setup and Operations UX
**Goal**: Make RTK discoverable and manageable from `aoc-control`.

**Tasks**:
- [ ] Add RTK section/actions in Settings.
- [ ] Surface status + remediation text in TUI status line/modal flow.

**Exit Criteria**:
- User can install/check/enable/disable RTK from `Alt+C` without manual shell steps.

### Phase 3: Migration + Documentation
**Goal**: Ensure existing repos converge by rerunning `aoc-init`.

**Tasks**:
- [ ] Extend `aoc-init` to seed RTK defaults and migrate legacy setups.
- [ ] Update docs for behavior, safety, and operational troubleshooting.

**Exit Criteria**:
- Existing repos adopt integration on next `aoc-init`; docs match shipped behavior.

---

## Test Strategy

## Test Pyramid
- Unit: 70%
- Integration: 25%
- End-to-end/smoke: 5%

## Coverage Requirements
- Routing logic branch coverage: >= 90%
- Adapter command path coverage: >= 85%
- `aoc-init` migration path coverage: >= 85%

## Critical Scenarios
- RTK installed + enabled => routed commands use RTK
- RTK missing => native command fallback, no hard failure
- RTK returns error => fallback path executes original command
- Bypass mode => no routing
- `aoc-control` action invokes adapter correctly and updates status messaging
- `aoc-init` migrates old setup without clobbering non-AOC custom content

---

## Risks

## Technical Risks
- **Risk**: Overly aggressive routing breaks command semantics.
  - **Mitigation**: conservative allowlist first; explicit passthrough defaults.
- **Risk**: Agent-specific assumptions leak into implementation.
  - **Mitigation**: shell-level integration in `aoc-agent-wrap` + adapter, no model-hook dependence.

## Dependency Risks
- **Risk**: Upstream RTK behavior changes.
  - **Mitigation**: adapter abstraction, version checks, fail-open mode.

## Scope Risks
- **Risk**: Expanding to too many commands in v1.
  - **Mitigation**: phased rollout with measurable gain before broadening allowlist.

---

## Open Questions
- Should v1 default mode be `suggest` or `enforce` for routed commands?
- Which exact command allowlist ships in v1 (git/test/search/log families)?
- Do we expose per-project override in addition to global default at launch?
