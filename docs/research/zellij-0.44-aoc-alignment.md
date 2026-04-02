# Zellij 0.44 → AOC Alignment Plan

This note captures the **exact AOC-side changes** implied by the Zellij `v0.44.0` update so they do not get lost across detached-orchestration, Mission Control, mux/pane control, and handshake/layout work.

## Source grounding

Reviewed against official sources:
- Zellij release: `https://github.com/zellij-org/zellij/releases/tag/v0.44.0`
- CLI docs: `https://zellij.dev/documentation/cli-actions.html`
- CLI control overview: `https://zellij.dev/documentation/controlling-zellij-through-cli.html`
- subscribe docs: `https://zellij.dev/documentation/zellij-subscribe.html`
- plugin API docs: `https://zellij.dev/documentation/plugin-api.html`

## Confirmed Zellij 0.44 capabilities relevant to AOC

### Pane / tab inventory
- `zellij action list-panes --json`
- `zellij action list-tabs --json`
- `zellij action current-tab-info --json`
- pane IDs and tab IDs are stable and returned by CLI
- new pane/tab creation returns created pane ID / tab ID

### Pane capture / observation
- `zellij action dump-screen --pane-id <id> --full --ansi`
- `zellij subscribe --pane-id <id> --format json --scrollback <n> --ansi`
- subscribe emits NDJSON pane updates and `pane_closed`

### Pane control
- `zellij action paste --pane-id <id> ...`
- `zellij action send-keys --pane-id <id> ...`
- `zellij action show-floating-panes --tab-id <id>`
- `zellij action hide-floating-panes --tab-id <id>`
- `zellij action set-pane-borderless --pane-id <id> --borderless <bool>`
- `zellij action override-layout <layout>`

### Session / operator affordances
- `zellij watch <session>` for read-only viewing
- read-only web tokens / remote attach over HTTPS

### Plugin API additions
- `get_pane_scrollback`
- `get_session_environment_variables`
- `save_session`
- pane highlighting / regex highlight click events
- pane color changes

---

## Exact AOC update changes

These are the changes AOC should make because of Zellij 0.44.

### 1) Replace `dump-layout`-first topology parsing with native JSON inventory

**Current AOC paths using `dump-layout`:**
- `bin/aoc-align`
- `bin/aoc-cleanup`
- `bin/aoc-control-toggle`
- `bin/aoc-mission-control-toggle`
- `crates/aoc-hub-rs/src/pulse_uds.rs`
- `crates/aoc-mission-control/src/main.rs`

**Update change:**
- Make `list-panes --json`, `list-tabs --json`, and `current-tab-info --json` the primary source for pane/tab inventory and floating visibility.
- Keep `dump-layout` only as fallback/compatibility during rollout.

**Why this is a real change:**
- native `tab_id`, `pane_id`, `pane_cwd`, `pane_command`, focus/floating/exited state removes brittle KDL parsing.

### 2) Keep Pulse/wrapper telemetry primary, but add native pane evidence capture for drilldown

**Update change:**
- Add bounded operator-only capture via `dump-screen --pane-id --full --ansi`.
- Add opt-in live drilldown via `zellij subscribe --pane-id --format json --scrollback N --ansi`.
- Use this for Overseer / Mission Control inspection and debugging, not as the default telemetry substrate.

**Why this is a real change:**
- pre-0.44 assumptions treated non-focused pane capture as too weak for reliable use.
- post-0.44, bounded pane evidence is viable for explicit operator drilldown.

### 3) Replace floating-pane toggle heuristics with explicit tab-aware show/hide

**Current AOC paths:**
- `bin/aoc-control-toggle`
- `bin/aoc-mission-control-toggle`

**Update change:**
- stop inferring floating visibility from `dump-layout`
- query tab state via `list-tabs --json` / `current-tab-info --json`
- use `show-floating-panes --tab-id` / `hide-floating-panes --tab-id`
- prefer explicit created pane ID / tab ID handling over toggle-only behavior

### 4) Re-scope Task 176 from tmux-first mux panes to Zellij-first promoted panes

**Update change:**
- promoted panes remain explicit allowlisted PI-control domains
- use Zellij-native pane inventory, capture, and targeted input as the primary substrate
- keep tmux optional only for fallback / inner control domains / niche workflows

**Meaningful architectural difference:**
- `aoc-mux` should no longer imply “mandatory nested tmux session”
- it should mean “explicitly promoted AOC-controlled pane with bounded tools and registry metadata”

### 5) Add runtime layout override modes instead of only launch-time static layout selection

**Update change:**
- keep current KDL template injection model
- add narrow runtime mode switching with `override-layout`
- first uses: focus mode, inspection mode, compact-bar/status-bar swaps, Mission Control review mode

### 6) Do not remove tmux-backed PI agent runtime just because Zellij improved

**Update change:**
- keep tmux-backed PI scrollback/runtime isolation for now
- only re-scope the mux/promoted-pane architecture and inspector surfaces

**Why this matters:**
- Zellij 0.44 improves pane targeting and observation
- it does not automatically replace all tmux-value in agent runtime isolation

### 7) Add plugin compatibility / performance verification for 0.44

**Current AOC surface:**
- `zjstatus.wasm` in `zellij/aoc.config.kdl.template` and `zellij/layouts/aoc.kdl.template`

**Update change:**
- explicitly verify plugin behavior after the runtime change from `wasmtime` to `wasmi`
- treat this as rollout validation before leaning harder on 0.44 features

---

## Task/PRD impact map

### Task 176 — `pi_terminal_ops_prd_rpg.md`
**Biggest change.**
- rewrite around Zellij-native promoted panes
- tmux becomes optional, not foundational

### Task 149 — `aoc-session-overseer_prd_rpg.md`
**Major enhancement.**
- native pane/tab inventory
- bounded pane evidence capture and live drilldown
- explicit floating-pane visibility control

### Task 169 — `task-169_aoc_detached_pi_subagent_runtime_prd_rpg.md`
**Moderate enhancement.**
- tab/pane-aware inspector overlays
- borderless + close-on-exit transient panes
- on-demand pane evidence capture for detached job drilldown

### Task 177 — `task-177_aoc_handshake_briefing_v2_prd_rpg.md`
**Light enhancement.**
- runtime layout overrides for focus/inspection/compact-bar modes
- keep handshake content work primary

### Task 62 — `62-define-pulse-vnext-architecture-and-prd.md`
**Light correction.**
- wrapper/hub telemetry remains primary
- but `dump-screen --pane-id` and `subscribe --pane-id` are now viable for explicit operator capture and debugging

---

## Implementation order

1. Add shared Zellij 0.44 capability/query adapter in shell + Rust.
2. Migrate `dump-layout`-based inventory/floating logic to `list-panes` / `list-tabs` / `current-tab-info`.
3. Add Mission Control / Overseer pane evidence capture and live drilldown.
4. Re-scope `aoc-mux` / promoted-pane runtime around Zellij-native control.
5. Add narrow runtime layout override modes.
6. Validate `zjstatus.wasm` and related plugin behavior on 0.44.

---

## Anti-drift note

To avoid losing the actual 0.44-specific deltas, any future implementation or PRD update in this area should explicitly label whether it is one of these:
- **inventory change**
- **pane evidence/drilldown change**
- **floating-pane control change**
- **promoted-pane / mux architecture change**
- **runtime layout override change**
- **plugin compatibility change**

If a proposed change does not map to one of those buckets, it is probably adjacent work rather than a direct Zellij 0.44 alignment change.
