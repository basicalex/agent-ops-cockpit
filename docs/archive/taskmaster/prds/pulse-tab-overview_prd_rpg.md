# AOC Pulse Tab Overview PRD (RPG)

## Problem Statement
AOC’s Zellij `0.44.x` upgrade exposed severe CPU spikes whenever custom status-bar plugins are active, especially during tab switching and mouse movement. Even after reducing `zjstatus` subscriptions, disabling mouse, and collapsing to a single bar, the plugin path still produces unacceptable resource churn in normal AOC work tabs. Operators still need a fast way to see all session tabs, distinguish active work, and identify each agent/chat context without paying the plugin runtime cost.

The missing capability is not “a prettier status bar”; it is a low-overhead tab overview that surfaces tab identity and agent context inside AOC’s own UI. Pulse / Mission Control already owns session-local status, task, diff, and health summaries, and Zellij `0.44` already exposes native JSON tab/pane inventory. AOC should use those native APIs plus Pulse’s existing session state to replace plugin bars with a Pulse-native tab overview.

The first local/native Pulse MVP proved the replacement direction, but it also exposed two architectural gaps that should not become permanent: tab focus and roster changes still feel laggy when they depend on periodic polling, and Pi chat/session titles should not be guessed from broad filesystem scans or launch-time session-dir coupling. The superior architecture is event-driven: Pulse should consume pushed layout/focus updates from AOC/Hub layout-state wiring, Pi panes should start as normal fresh chats, the subtitle should default to `new`, and Pi session naming/resume changes should flow through a small AOC-owned Pi extension/hook path with fail-open fallbacks only when the event path is unavailable.

## Target Users
- **Primary: daily AOC operators inside normal work tabs**
  - Need to quickly see all tabs, which tab is focused, and what each agent is doing.
  - Need this without triggering redraw storms or layout watcher churn.
- **Secondary: AOC maintainers debugging session behavior**
  - Need a deterministic, inspectable source of tab state using native Zellij inventory.
  - Need a clear architectural split between Zellij layout control and AOC status rendering.
- **Tertiary: power users running multi-tab/multi-agent sessions**
  - Need compact tab summaries, sensible titles, and eventually optional jump/focus workflows.

## Success Metrics
- Default AOC operation on Zellij `0.44.x` no longer depends on custom status-bar plugins for tab visibility.
- Pulse pane shows the full current session tab roster with focused-tab indication and a useful per-tab subtitle.
- Pulse tab overview updates from native inventory and pushed hub/layout state without requiring background plugin redraw loops.
- Fresh Pi panes render subtitle `new` until the conversation is explicitly named or resumed.
- Pi session-name changes and session-switch/resume flows update Pulse from an explicit hook/extension path rather than broad directory polling.
- `aoc.unstat` remains the stable operational layout while Pulse provides the missing session overview information.
- Manual validation across `3+` tabs confirms no plugin-related CPU spike from tab hover/switch because no status-bar plugin is loaded.
- Focus marker updates feel operator-immediate because the normal path is event-driven; periodic local polling remains only as fail-open fallback.
- If focus actions are enabled later, focusing a tab from Pulse succeeds through native Zellij actions within operator-perceived instant response (< 1 second).

---

## Capability Tree

### Capability: Native Tab Inventory Collection
Collect authoritative session tab and pane topology from Zellij `0.44` native JSON actions.

#### Feature: Session snapshot query
- **Description**: Query current tabs, panes, and focused-tab state using native Zellij actions.
- **Inputs**: `session_id`
- **Outputs**: Normalized session snapshot containing tabs, panes, focused tab metadata, and project mappings
- **Behavior**: Call `list-tabs --json`, `list-panes --json`, and `current-tab-info --json`; fall back only when native actions are unavailable.

#### Feature: Pane-to-tab normalization
- **Description**: Normalize pane records so Pulse can reliably map agents and runtime snapshots to tabs.
- **Inputs**: Native pane inventory, tab inventory, existing runtime rows
- **Outputs**: Pane→tab map, project→tab map, focused-tab index/name
- **Behavior**: Resolve tab indices/names consistently and preserve stable mappings across refreshes.

#### Feature: Layout-source policy
- **Description**: Prefer event-driven layout/focus updates for Pulse tab overview without reintroducing heavy background watcher churn.
- **Inputs**: Runtime mode, hub connection state, layout source configuration, layout-state topic availability
- **Outputs**: Deterministic source-of-truth policy for Pulse tab summaries and focused-tab freshness
- **Behavior**: Use pushed hub/layout-state updates as the normal focus/roster freshness path, reconcile against native local inventory for correctness, and keep periodic local polling only as fail-open fallback.

### Capability: Pulse Tab Summary Derivation
Convert raw session inventory plus existing agent/task state into operator-meaningful tab summaries.

#### Feature: Tab roster aggregation
- **Description**: Group agent/runtime rows by tab and produce one compact summary per tab.
- **Inputs**: Overview rows, task summaries, diff/task signals, normalized tab metadata
- **Outputs**: Ordered `PulseTabSummary` list
- **Behavior**: Group by tab index/name, preserve layout order, and mark the focused/current viewer tab.

#### Feature: Title and subtitle policy
- **Description**: Pick the best human-readable title/subtitle for each tab while respecting Pi's normal fresh-chat behavior.
- **Inputs**: Tab name, agent label, explicit Pi session-title hook data, session lifecycle state, active task titles, latest snippet
- **Outputs**: Primary label and secondary subtitle per tab
- **Behavior**: For fresh Pi chats with no explicit title, render subtitle `new`. Prefer explicit Pi session title from the extension/hook path after rename/resume/switch events, then fall back to active task title, current snippet, or agent label only when no explicit session title exists.

#### Feature: State rollup
- **Description**: Roll up lifecycle, freshness, and attention signals for each tab.
- **Inputs**: Tab member rows with status and age metadata
- **Outputs**: Focus marker, attention chip, freshness marker, optional busy/blocked/needs-input rollup
- **Behavior**: Surface the strongest signal per tab while remaining compact.

### Capability: Pulse Pane Presentation
Render the tab overview directly in Pulse without a status-bar plugin.

#### Feature: Compact tab overview section
- **Description**: Render a `Tabs` section near the top of Pulse.
- **Inputs**: `PulseTabSummary` list, available width, runtime mode
- **Outputs**: Width-aware ratatui lines for compact tab display
- **Behavior**: Show focus marker, tab index/name, primary label, and truncated subtitle; degrade gracefully on narrow widths.

#### Feature: Responsive formatting
- **Description**: Adapt the overview to compact versus wide pane widths.
- **Inputs**: Pane width, summary list
- **Outputs**: Width-budgeted text presentation
- **Behavior**: Keep all rows readable by truncating lower-priority fields first.

#### Feature: Pulse-mode integration
- **Description**: Integrate tab rendering into existing `pulse-pane` without breaking local work/mind/health sections.
- **Inputs**: Existing pulse pane render flow
- **Outputs**: Updated Pulse pane body
- **Behavior**: Insert the new tab section while preserving current local-status summaries below it.

### Capability: Operator Navigation and Actions
Provide a bounded path for tab navigation from Pulse when useful.

#### Feature: Optional focus selected tab
- **Description**: Allow Pulse to focus a selected tab through native Zellij actions or existing hub command plumbing.
- **Inputs**: Selected tab summary, session id, optional hub connection
- **Outputs**: Focused target tab or actionable operator error
- **Behavior**: Reuse `go_to_tab` / `focus_tab`; keep the feature gated until the read-only view is stable.

#### Feature: Safe non-goal defaults
- **Description**: Keep Pulse useful even if interactive tab focus is deferred.
- **Inputs**: Runtime mode and feature flags
- **Outputs**: Read-only but informative tab overview
- **Behavior**: Avoid blocking the core replacement on navigation UX.

### Capability: Event-Driven Metadata and Documentation Rollout
Document the new design and wire fast update paths for title/focus changes.

#### Feature: Pi session-title sync hook
- **Description**: Add a first-class Pi-owned path that reports session title changes to AOC/Pulse.
- **Inputs**: Pi extension lifecycle events, `pi.getSessionName()`, AOC pane/session env, optional rename command hook
- **Outputs**: Explicit `session_title` updates for the current pane/tab
- **Behavior**: Publish `new` on fresh session start, then publish the real title after rename/resume/switch/fork events; avoid launch-time session-dir coupling and broad directory polling as the primary mechanism.

#### Feature: Layout and operator guidance
- **Description**: Update docs to make Pulse the canonical tab overview and `unstat` the stable no-plugin layout.
- **Inputs**: Final architecture and rollout decision
- **Outputs**: Updated docs and guidance
- **Behavior**: Explain why AOC prefers native Zellij inventory plus Pulse over plugin bars on `0.44.x`, and why pushed layout/title updates are preferred over polling.

---

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-core/
│   │   └── src/
│   │       └── zellij_cli.rs              # Native Zellij inventory and normalization
│   ├── aoc-mission-control/
│   │   └── src/
│   │       ├── main.rs                    # Runtime wiring and Mission Control integration
│   │       ├── overview.rs                # Overview rendering/helpers
│   │       └── overview_support.rs        # Overview summary/support helpers
│   └── aoc-agent-wrap-rs/
│       └── src/
│           └── main.rs                    # Runtime metadata + fail-open title fallback only
├── docs/
│   ├── mission-control.md                 # Runtime behavior and Mission Control semantics
│   ├── mission-control-ops.md             # Operator workflows and troubleshooting
│   ├── configuration.md                   # Config toggles / layout source notes
│   └── research/
│       └── zellij-0.44-aoc-alignment.md   # Alignment rationale with Zellij 0.44
└── zellij/
    └── layouts/
        └── unstat.kdl.template            # Stable no-plugin operational fallback
```

## Module Definitions

### Module: `crates/aoc-core/src/zellij_cli.rs`
- **Maps to capability**: Native Tab Inventory Collection
- **Responsibility**: Provide normalized session snapshots from native Zellij JSON actions.
- **Exports**:
  - `query_session_snapshot(session_id)` - fetch tabs, panes, current-tab metadata, and project-tab mapping

### Module: `crates/aoc-mission-control/src/overview.rs` + `overview_support.rs`
- **Maps to capability**: Mission Control overview derivation + presentation
- **Responsibility**: Convert runtime/layout/task state into compact Mission Control overview summaries and renderable lines.
- **Exports**:
  - Mission Control overview rendering helpers and support derivation used by the active TUI surface

### Module: `crates/aoc-mission-control/src/main.rs`
- **Maps to capability**: Mission Control integration + operator actions
- **Responsibility**: Wire overview collection into local snapshot refresh, pushed layout-state updates, Mission Control rendering, and optional focus actions.
- **Exports**:
  - existing Mission Control binary behavior with overview support

### Module: `.pi/extensions/pulse/index.ts`
- **Status**: removed/deprecated
- **Notes**: Pi-native naming replaced the old Pulse-backed session-title sync hook, so this project-local extension is no longer part of the active runtime surface.

### Module: `crates/aoc-agent-wrap-rs/src/main.rs`
- **Maps to capability**: Fail-open fallback metadata
- **Responsibility**: Preserve stable runtime identity metadata and provide bounded fallback title discovery when the explicit Pi hook path is unavailable.
- **Exports**:
  - existing runtime snapshot / source metadata with optional title field

### Module: `docs/*`
- **Maps to capability**: Metadata and Documentation Rollout
- **Responsibility**: Document the Pulse-native replacement strategy and operating guidance.

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies - these are built first.

- **Native session snapshot normalization**: Reliable tab/pane inventory from `zellij_cli`
- **Pulse tab summary data contract**: `PulseTabSummary` model, ordering rules, and subtitle policy including Pi default `new`
- **Source policy decision**: Native inventory establishes truth, but pushed layout-state updates are the normal freshness path for focus/roster changes

### Session Mapping Layer (Phase 1)
- **Pulse local snapshot wiring**: Depends on [Native session snapshot normalization, Source policy decision]
- **Pane/project/tab association updates**: Depends on [Native session snapshot normalization]
- **Hub/layout-state subscribe wiring for pulse-pane**: Depends on [Source policy decision]

### Summary Derivation Layer (Phase 2)
- **Tab roster aggregation**: Depends on [Pulse local snapshot wiring, Pane/project/tab association updates]
- **Title/subtitle policy engine**: Depends on [Pulse tab summary data contract, Tab roster aggregation, Pi session-title sync hook]
- **Attention/freshness rollup**: Depends on [Tab roster aggregation, Hub/layout-state subscribe wiring for pulse-pane]

### Presentation Layer (Phase 3)
- **Pulse `Tabs` section rendering**: Depends on [Tab roster aggregation, Title/subtitle heuristic engine, Attention/freshness rollup]
- **Responsive compact/wide formatting**: Depends on [Pulse `Tabs` section rendering]
- **Pulse pane integration**: Depends on [Pulse `Tabs` section rendering, Responsive compact/wide formatting]

### Operator Actions and Rollout Layer (Phase 4)
- **Optional focus-tab action**: Depends on [Pulse pane integration]
- **Pi session-title sync hook**: Depends on [Title/subtitle policy engine]
- **Docs and rollout guidance**: Depends on [Pulse pane integration, Optional focus-tab action, Pi session-title sync hook]

---

## Development Phases

### Phase 0: Foundation and Contracts
**Goal**: Establish the source strategy and data contract for Pulse-native tab overview.

**Entry Criteria**: Existing audit confirms plugin bars are not acceptable on Zellij `0.44.x`.

**Tasks**:
- [ ] Confirm Pulse uses native/local Zellij inventory as the default tab overview source (depends on: none)
  - Acceptance criteria: source strategy is written down in code/docs comments and implementation plan.
  - Test strategy: verify no plugin runtime or background watcher dependency is required for the base view.
- [ ] Define `PulseTabSummary` and subtitle priority rules (depends on: none)
  - Acceptance criteria: summary model covers focus, label, subtitle, attention, freshness, and member rows.
  - Test strategy: unit tests for summary derivation with representative runtime rows.

**Exit Criteria**: There is a stable design contract for how Pulse will build and present tab summaries.

**Delivers**: A clear implementation target with no ambiguity about data sources or UI semantics.

---

### Phase 1: Snapshot and Mapping Plumbing
**Goal**: Make Pulse capable of collecting a complete tab roster without plugin support.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Extend Pulse local snapshot/layout plumbing to retain full tab roster and focused tab metadata (depends on: [Phase 0 source strategy])
  - Acceptance criteria: Pulse can access ordered tabs, pane membership, and focused-tab metadata on refresh.
  - Test strategy: unit/integration tests around native snapshot parsing and fallback behavior.
- [ ] Wire pulse-pane to consume pushed hub/layout-state updates for focus/roster freshness (depends on: [full tab roster plumbing])
  - Acceptance criteria: focused-tab marker and layout changes update without waiting for the next periodic poll when hub/layout-state is available.
  - Test strategy: subscription tests for `layout_state` topics plus event-handling tests for local tab overlay updates.
- [ ] Normalize pane/project association for tab summaries (depends on: [full tab roster plumbing])
  - Acceptance criteria: agent/runtime rows consistently map to tab index/name for common AOC layouts.
  - Test strategy: test mixed pane/project associations and stable ordering across refreshes.

**Exit Criteria**: Pulse has a complete, ordered, local-native view of session tabs.

**Delivers**: Reliable raw ingredients for a tab overview.

---

### Phase 2: Summary Derivation
**Goal**: Convert raw tabs plus existing runtime/task state into operator-meaningful summaries.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Build tab aggregation and ordering helpers (depends on: [Phase 1 plumbing])
  - Acceptance criteria: one summary per tab, preserving layout order and focus state.
  - Test strategy: unit tests for grouping multiple panes/agents under the same tab.
- [ ] Implement title/subtitle policy with explicit Pi title hook and fallbacks (depends on: [tab aggregation])
  - Acceptance criteria: fresh Pi chats render `new`, explicit Pi session titles override immediately after rename/resume/switch events, and non-Pi tabs still get stable fallback context.
  - Test strategy: heuristic/policy tests covering fresh Pi sessions, rename/resume events, active task titles, snippets, fallback labels, and missing metadata.
- [ ] Roll up attention/freshness per tab (depends on: [tab aggregation])
  - Acceptance criteria: blocked/error/stale/focused signals are compact and deterministic.
  - Test strategy: unit tests for rollup precedence rules.

**Exit Criteria**: Pulse can derive useful, compact summaries for every tab in the session.

**Delivers**: A session-wide tab model designed for Pulse presentation.

---

### Phase 3: Pulse Pane UI
**Goal**: Render the tab overview in Pulse and keep the rest of Pulse useful.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Render a `Tabs` section near the top of Pulse (depends on: [Phase 2 summaries])
  - Acceptance criteria: Pulse shows focused and unfocused tabs with readable labels and subtitles.
  - Test strategy: render tests and manual checks in compact and wide pane widths.
- [ ] Integrate width-aware truncation and responsive formatting (depends on: [Tabs section rendering])
  - Acceptance criteria: narrow panes stay readable; lower-priority content truncates first.
  - Test strategy: snapshot/render tests across representative widths.
- [ ] Preserve Pulse local work/mind/health usefulness after adding tabs (depends on: [Tabs section rendering])
  - Acceptance criteria: tab overview does not crowd out critical local summaries.
  - Test strategy: manual comparison using realistic AOC sessions.

**Exit Criteria**: Pulse becomes the practical replacement for plugin-based tab visibility.

**Delivers**: Operators can see all tabs and current work context from the Pulse pane.

---

### Phase 4: Optional Navigation and Rollout
**Goal**: Add bounded navigation support and complete rollout documentation.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Gate and wire optional tab-focus action from Pulse (depends on: [Phase 3 integration])
  - Acceptance criteria: when enabled, Pulse can focus a target tab using native actions or hub command plumbing.
  - Test strategy: manual verification in live sessions plus focused action tests where practical.
- [ ] Implement Pi session-title sync hook and fail-open fallback behavior (depends on: [Phase 2 title/subtitle policy])
  - Acceptance criteria: fresh Pi chats publish `new`, rename/resume/switch/fork flows publish the real title, and Pulse keeps bounded fallback behavior if the hook path is unavailable.
  - Test strategy: extension/runtime tests for hook payloads plus backward-compatibility checks for fallback metadata.
- [ ] Update docs and rollout guidance (depends on: [Phase 3 integration, optional focus action, Pi session-title sync hook])
  - Acceptance criteria: docs clearly direct operators to Pulse + `unstat` instead of plugin bars and explain the push-first architecture.
  - Test strategy: doc review and install/layout guidance verification.

**Exit Criteria**: The new architecture is documented, testable, and operator-friendly.

**Delivers**: A supported Pulse-native tab overview path for AOC on Zellij `0.44.x`.

---

## Test Pyramid

```text
        /\
       /E2E\       ← 10% (manual/operator session validation)
      /------\
     /Integration\ ← 30% (snapshot + aggregation + render wiring)
    /------------\
   /  Unit Tests  \ ← 60% (heuristics, ordering, rollups, width budgets)
  /----------------\
```

## Coverage Requirements
- Line coverage: 80% minimum for new tab-summary helpers
- Branch coverage: 70% minimum for new heuristic/rollup logic
- Function coverage: 85% minimum for new helper functions
- Statement coverage: 80% minimum for new Pulse tab overview code paths

## Critical Test Scenarios

### Native session snapshot normalization
**Happy path**:
- Zellij `0.44` native actions return multiple tabs and panes
- Expected: ordered tab and pane mappings are normalized without `dump-layout`

**Edge cases**:
- Tabs exist with sparse or missing pane metadata
- Expected: fallback naming remains stable and summaries stay renderable

**Error cases**:
- Native action unavailable or malformed output
- Expected: fail-open fallback remains bounded and does not crash Pulse

**Integration points**:
- `query_session_snapshot(...)` feeds Pulse local refresh
- Expected: Pulse receives enough metadata to render the tab roster

### Tab aggregation and subtitle heuristics
**Happy path**:
- Multiple tabs each have an agent row with active task/snippet
- Expected: one summary per tab with correct title/subtitle choice

**Edge cases**:
- No task title, no snippet, duplicate labels, multiple panes in one tab
- Expected: stable fallback ordering and readable output

**Error cases**:
- Conflicting or partial metadata between local and hub rows
- Expected: deterministic precedence with no duplicated tab entries

**Integration points**:
- Overview rows + task summaries combine into `PulseTabSummary`
- Expected: summaries preserve focus order and attention state

### Pulse tab presentation
**Happy path**:
- Pulse pane width is normal and session has `3+` tabs
- Expected: tabs section renders above local work/mind/health sections

**Edge cases**:
- Very narrow Pulse pane
- Expected: truncation degrades gracefully without broken formatting

**Error cases**:
- Empty session or no active runtime rows
- Expected: clear empty-state copy, not a broken panel

**Integration points**:
- `render_pulse_pane_lines(...)` with new tab lines
- Expected: other Pulse sections remain intact and readable

### Optional focus action
**Happy path**:
- Selected tab has a valid index
- Expected: native `go-to-tab` / hub `focus_tab` focuses the target tab

**Edge cases**:
- Pulse is read-only or action is gated off
- Expected: no accidental navigation; clear operator messaging

**Error cases**:
- Zellij action fails or target tab disappears
- Expected: user-visible error note, no crash

## Test Generation Guidelines
- Prefer pure helper extraction for aggregation, ordering, subtitle selection, and width budgeting so those rules are unit-testable.
- Add render tests for representative widths rather than snapshotting huge session dumps.
- Use deterministic synthetic rows/tabs in tests; avoid requiring a live Zellij session for most logic coverage.
- Add event-path tests for hub/layout-state subscriptions and Pi title-hook payload handling.
- Keep live/manual validation focused on operator workflows: `aoc.unstat`, `3+` tabs, focused-tab changes, fresh Pi tabs showing `new`, rename/resume title propagation, and absence of plugin-induced CPU spikes.

---

## System Components
- **Zellij native inventory layer**: authoritative source for tabs/panes/current-tab metadata on `0.44.x`
- **Hub/layout-state event path**: push-based freshness path for focused-tab and roster updates
- **Pulse local snapshot loop**: bounded reconciliation/fail-open path already used by `pulse-pane`
- **Pulse tab summary layer**: new aggregation logic that turns raw state into operator summaries
- **Pulse renderer**: ratatui line rendering for the new `Tabs` section
- **Pi Pulse extension hook**: explicit session-title publisher for fresh chats, rename, resume, switch, and fork flows

## Data Models
- **`ZellijQuerySnapshot`**: native session snapshot with tabs, panes, current-tab metadata, and project mappings
- **`OverviewRow`**: existing per-agent/session row enriched with tab metadata
- **`PulseTabSummary`** (new):
  - `tab_index`
  - `tab_name`
  - `focused`
  - `primary_label`
  - `subtitle`
  - `attention`
  - `freshness`
  - `member_rows`
- **Pi title state**:
  - `session_title` for explicit current Pi session name
  - `new` as the canonical default subtitle for fresh unnamed Pi chats
  - fallback context hint only when no explicit Pi title is available

## Technology Stack
- **Language**: Rust
- **UI**: ratatui + crossterm
- **Session control**: Zellij `0.44.x` native CLI JSON actions
- **State transport**: Pulse UDS / existing hub cache where helpful

**Decision: Native Zellij inventory over plugin status bars**
- **Rationale**: Native JSON inventory gives tab state without plugin redraw/runtime overhead.
- **Trade-offs**: Pulse becomes the primary status surface instead of a persistent top/bottom bar.
- **Alternatives considered**: Keep patching `zjstatus`; use one plugin bar; use native compact UI experiments.

**Decision: Pulse-first read-only overview before interactive navigation**
- **Rationale**: The main operator pain is visibility, not first-click navigation.
- **Trade-offs**: Initial release may require keyboard tab navigation outside Pulse.
- **Alternatives considered**: Add focus actions immediately; continue bar plugin experimentation.

**Decision: Fresh Pi chats default to `new`; explicit Pi session-title hooks beat heuristics**
- **Rationale**: This preserves normal Pi startup semantics, avoids brittle launch-time coupling, and gives Pulse fast accurate titles after rename/resume/switch flows.
- **Trade-offs**: Requires a small AOC-owned Pi extension/hook path and bounded fallback behavior when the hook is unavailable.
- **Alternatives considered**: broad session-file polling; forcing custom `--session-dir` coupling at launch; heuristic-only subtitles.

**Decision: Push layout/title updates first; polling only as fail-open fallback**
- **Rationale**: Focus-marker lag and title lag are operator-visible UX bugs best solved with event-driven updates.
- **Trade-offs**: Requires finishing hub/layout-state subscription wiring for pulse-pane and adding explicit Pi hook publishing.
- **Alternatives considered**: raising polling frequency; periodic broad filesystem scans; accepting stale tab focus markers.

---

## Technical Risks
**Risk**: Pulse local snapshot refresh still becomes too expensive when collecting tab roster
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: Reuse native `query_session_snapshot(...)`, prefer push-based layout-state for freshness, keep reconciliation polls bounded, and unit-test aggregation separately from live queries.
- **Fallback**: Reduce refresh cadence and/or use cached tab roster until explicit refresh.

**Risk**: `aoc-mission-control/src/main.rs` remains too monolithic for safe changes
- **Impact**: Medium
- **Likelihood**: High
- **Mitigation**: Extract summary/render helpers into a dedicated `pulse_tabs.rs` module.
- **Fallback**: Land helper-only refactors first, then wire rendering changes.

**Risk**: Pi title hook path is incomplete or misses some rename/resume flows
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: Publish on session start/switch/fork plus an AOC-owned naming command path, and keep bounded fallback metadata discovery for fail-open behavior.
- **Fallback**: Temporarily fall back to context hints and local/native reconciliation rather than broad high-frequency polling.

## Dependency Risks
- Zellij native JSON output shape may vary slightly across versions; normalization must remain tolerant.
- Hub layout-state streaming is only partially wired today; the pulse-pane subscriber path must be completed carefully so focus freshness improves without destabilizing other subscribers.
- Pi extension loading and AOC hook publishing must remain optional/fail-open so normal Pi startup is never blocked.
- Operator expectations may drift toward interactivity before the read-only overview proves its value.

## Scope Risks
- It is easy to turn this into a full Pulse navigation TUI instead of a compact overview.
- Mixing launch-time Pi session plumbing with the title-sync work risks breaking normal Pi startup; the event-driven hook path should stay decoupled from launch semantics.
- Attempting to fully solve every Pulse layout or Mission Control refactor in one task would over-expand the effort.

---

## References
- `docs/mission-control.md`
- `docs/mission-control-ops.md`
- `docs/configuration.md`
- `docs/research/zellij-0.44-aoc-alignment.md`
- `crates/aoc-core/src/zellij_cli.rs`
- `crates/aoc-mission-control/src/main.rs`
- `crates/aoc-hub-rs/src/pulse_uds.rs`

## Glossary
- **Pulse**: the small per-tab Mission Control surface shown in normal AOC work tabs
- **Mission Control**: the larger orchestration TUI used in dedicated/floating control flows
- **Viewer tab**: the tab currently hosting the Pulse pane instance
- **Tab roster**: ordered list of all tabs in the current AOC/Zellij session
- **Subtitle heuristic**: rule set used to pick the most useful secondary label for a tab

## Open Questions
- Should Pulse v1 remain fully read-only, or should `Enter`/selection support tab focus immediately once push-based focus freshness is in place?
- Should AOC ship a dedicated Pi naming command/hook (for immediate Pulse updates) in addition to supporting Pi's built-in naming flows?
- Should pulse-pane subscribe to `layout_state` in all modes by default, or only when `runtime_mode.is_pulse_pane()`/local tab rendering is active?
- Should the tab overview also surface lightweight per-tab diff/task counts, or is that too dense for the Pulse pane?
