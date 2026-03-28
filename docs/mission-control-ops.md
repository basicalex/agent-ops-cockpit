# Mission Control Operations Guide (AOC)

This is a verbose, end-to-end reference for how Mission Control works in AOC,
including the hub, wrappers, panes, and runtime flows. Use this document to
continue debugging in a new chat session with a clear context.

## 1) System Overview

Mission Control is a Rust TUI that subscribes to a per-session, local-only
websocket hub. The hub aggregates agent status, task summaries, diff summaries,
and on-demand diff patches. Agents do not need custom instrumentation; they are
wrapped by `aoc-agent-wrap-rs`, which emits signals based on filesystem and git
state, plus live agent output.

Key components:
- Hub: `crates/aoc-hub-rs` (binary `aoc-hub-rs`)
- Wrapper: `crates/aoc-agent-wrap-rs` (binary `aoc-agent-wrap-rs`)
- UI: `crates/aoc-mission-control` (binary `aoc-mission-control`)
- Toggle launcher: `bin/aoc-mission-control-toggle`

## Pulse Overview Status (2026-02)

- Decision: Pulse Overview is re-enabled by default.
- Default behavior: Mission Control starts in `Overview` mode.
- Gate: set `AOC_PULSE_OVERVIEW_ENABLED=0` to run `Work`/`Diff`/`Health` only.
- CPU guardrail: keep background layout watcher off by default with
  `AOC_PULSE_LAYOUT_WATCH_ENABLED=0`.

## 2) Session Scoping and Environment

All routing is scoped to a session ID. Session scope is enforced by the hub.

Session ID resolution (shared across tools):
1) `AOC_SESSION_ID`
2) `ZELLIJ_SESSION_NAME`
3) generated `<noun>-<verb>` fallback (eg, `otter-refactors`)

Hub address and URL:
- `AOC_HUB_ADDR` default: `127.0.0.1:<port>`
- `AOC_HUB_URL` default: `ws://<addr>/ws`

Port derivation:
```
port = 42000 + (fnv1a_32(session_id) % 2000)
```

Project root used by wrappers:
- `AOC_PROJECT_ROOT` or current working directory

Agent identity:
- `AOC_AGENT_ID` or pane id or repo name

## 3) Startup Flow (New Session)

Entering from a normal shell:
```
aoc
```

Flow:
1) `bin/aoc` detects no Zellij session and runs `aoc-launch`.
2) `bin/aoc-launch`:
   - Resolves session id and hub address.
   - Starts hub once per session via `aoc-hub` and stores PID in
     `~/.local/state/aoc/hub-<session_slug>.pid`.
   - Renders a layout template into a temp KDL file with injected env.
   - Starts Zellij session with layout.

Layout (default `~/.config/zellij/layouts/aoc.kdl`):
- Agent pane launches with:
  `AOC_AGENT_RUN=1 exec ${AOC_AGENT_CMD:-aoc-agent-run}`
- Taskmaster pane runs `aoc-taskmaster`.
- Other panes (yazi, pulse, terminal) get `AOC_SESSION_ID` and
  `AOC_HUB_ADDR` exported.

## 4) Startup Flow (New Tab)

Inside a Zellij session:
```
aoc
```

Flow:
1) `bin/aoc` detects Zellij and runs `aoc-new-tab`.
2) `bin/aoc-new-tab` injects env into the layout (same placeholders) and opens
   a new tab in the current session.

Note: Hub is started only once per session by `aoc-launch`, not by `aoc-new-tab`.

## 5) Agent Wrapper Chain

Goal: keep tmux scrollback (PI by default) while capturing stdout/stderr for streaming.

Wrapper chain (tmux-enabled agents, default `pi`):
```
tmux
  -> aoc-agent-wrap-rs (Rust wrapper)
    -> aoc-agent-wrap (bootloader + handshake)
      -> pi
```

Wrapper chain (other agents, unless allowlisted for tmux):
```
aoc-agent-wrap-rs
  -> aoc-agent-wrap (bootloader + handshake)
    -> agent CLI
```

To run a custom agent via the main layout, set `AOC_AGENT_CMD` to your wrapper command/script.

How it is wired:
- `bin/aoc-agent-run` chooses agent based on `AOC_AGENT_ID` or state file.
- `bin/aoc-pi` respects `AOC_AGENT_RUN=1` and execs `aoc-agent-wrap`.
- `bin/aoc-agent-wrap` resolves the real agent binary, runs the bootloader for
  the handshake, and then (if available) wraps the bootloader with
  `aoc-agent-wrap-rs`.
- Pi now prefers the Rust wrapper by default when it is available so live Pulse
  and Mind runtime wiring matches the documented session model. Use
  `AOC_PI_USE_WRAP_RS=0` only as an explicit legacy direct-exec fallback.
- `bin/aoc-agent-wrap` also ensures `AOC_SESSION_ID`, `AOC_HUB_ADDR`, and
  `AOC_AGENT_ID` are exported before entering tmux or running the agent.

Important: `aoc-agent-wrap-rs` must be on PATH or present in
`<project_root>/crates/target/(debug|release)/aoc-agent-wrap-rs` or it will
fall back to the legacy chain (no streaming).

## 6) Hub Behavior

Hub: `crates/aoc-hub-rs`
- Binds only to loopback (127.0.0.1).
- Websocket endpoint: `/ws`
- Health endpoint: `/health` (returns `ok`).
- Requires a `hello` handshake with session_id and role.
- Maintains last-known state per agent_id (status/diff/task).
- Sends snapshot to new subscribers on connect.

Message routing:
- Publishers (wrappers) send: `agent_status`, `diff_summary`, `task_summary`,
  `heartbeat`, `diff_patch_response`.
- Subscribers (Mission Control) send: `diff_patch_request`.

## 7) Mission Control UI

Binary: `aoc-mission-control` (Rust TUI)

Layout:
- Left: agent list + status
- Right-top: diff files list (filtered)
- Right-bottom: patch view

Keybindings:
- `Tab`: switch modes/views
- `1..7`: jump directly to Overview / Overseer / Mind / Fleet / Work / Diff / Health
- `j/k` or `Up/Down`: move selection / scroll
- `Enter`: focus the selected worker tab from Overseer mode
- `x`: stop the selected worker
- `c`: request peer review
- `u`: request peer unblock/help
- `s`: spawn a fresh worker tab
- `d`: delegate the selected worker into a fresh worker tab and write a bounded brief
- `r`: refresh local snapshot
- `Esc`: hides the floating pane (prefers explicit `zellij action hide-floating-panes --tab-id ...` on Zellij `>= 0.44.0`, otherwise falls back to toggle behavior)
- `q`: quits Mission Control

Mind-mode project UI additions:
- `/`: edit the local project Mind search query
- `n` / `N`: browse next / previous search result
- the `Retrieval / search` section is project-scoped and currently searches local handshake, canon, and recent export summaries
- the `Activity summary` section is project-local and the `Mission Control bridge` section explains when to switch to Fleet / Overview / Overseer

Fleet mode is intended for detached specialist supervision. It groups detached jobs by project root and ownership plane so operators can distinguish:
- delegated/operator-launched detached specialists
- Mind-owned detached work (for example T2/T3 runtime activity in the current detached slice; T1 remains inline/session-scoped)

Fleet mode controls:
- `j/k` select a fleet group
- `Left/Right` or `[/]` select a specific job within the selected group
- `Enter` focus a live tab for the selected project's session when available
- `i` launch an inspect follow-up tab with a bounded brief for the selected detached job
- `h` launch a handoff follow-up tab with a bounded brief for the selected detached job
- `x` cancel the selected active detached job
- `f` cycle plane filter: all → delegated → mind
- `S` cycle sort mode: project → newest → active-first → error-first
- `A` toggle active-only groups
- lower drilldown panel shows the selected group's selected job, recovery guidance, and recent jobs

Overseer mode also renders a reviewable orchestration compile section:
- graph summary counts (nodes / edges / review paths)
- bounded compile previews for side-effectful actions such as review/help/observe/stop/spawn/delegate
- this is a dry review surface only; it does not auto-execute actions without the matching keybinding

Agent message display:
- Wrapper updates `agent_status.message` based on the latest non-empty line
  from stdout/stderr (rate-limited).
- UI shows `Message:` inside the agent details pane.

## 8) Mission Control Launch Modes

### Floating toggle
Shortcut (Zellij): `Alt+a`

`Alt+a` runs `aoc-mission-control-toggle`, which:
- Always opens Mission Control as a floating pane.
- Uses a PID file under `~/.local/state/aoc/mission-control-<session>.pid` to
  decide whether to toggle or create.
- If already running, it toggles floating panes instead of creating another.
- Closes the temporary split pane created by `Run` after launching MC.

Floating panes are tab-scoped in Zellij. You get one MC per tab.

### Delegated subagent supervision fast path
Use:

```bash
aoc-subagent-supervision-toggle
```

This wrapper reuses the Mission Control floating-pane launcher but starts the surface with:
- `mission-control` runtime mode
- `Fleet` as the initial view
- `delegated` as the initial plane filter
- a distinct floating pane name (`Subagent Supervision`) so it can coexist with the general Mission Control pane if desired

Recommended use:
- bind `aoc-subagent-supervision-toggle` to a Zellij shortcut when you want a one-keystroke detached-supervision surface separate from Pi's launch/clarify overlay

Product boundary:
- Pi manager remains the launch / clarify / approval surface for delegated runs
- the floating supervision pane is the fast detached-status / drilldown surface
- Pulse and the durable detached registry remain the source of truth

### Dedicated Mission Control tab
For longer orchestration sessions, use the dedicated Mission Control tab flow.
This runs `aoc-mission-control` in `mission-control` mode, while normal AOC tab
right panes should run `aoc-pulse-pane` / `AOC_MISSION_CONTROL_MODE=pulse-pane`.

### Project-scoped floating Mind UI
For lightweight project knowledge review, use the project-scoped floating Mind surface instead of keeping a persistent Mind pane in every AOC tab.

Entry points:
- `Alt+M` inside Pi
- `/mind` inside Pi
- `bin/aoc-mind-toggle`

Behavior:
- resolves the active project from the current AOC tab, preferring the current Agent pane project root when available
- outside Zellij, starts `aoc-mission-control` directly in `Mind` view
- inside Zellij, creates or reuses one named floating pane per tab (`Project Mind` by default)
- if the pane already exists, invocation toggles current-tab visibility instead of spawning duplicates
- the floating Mind surface runs Mission Control in `Mind` view with `AOC_MIND_PROJECT_SCOPED=1`

Boundary note:
- this floating Mind surface is the project-local knowledge UI
- Fleet remains the global detached-runtime surface
- normal Pulse panes remain lightweight local status panes; AOC no longer targets one persistent Mind pane per work tab

Boundary note: the normal `pulse-pane` stays local-only. It does not allow switching into Fleet/Overseer orchestration surfaces or running pane evidence/live-follow drilldown; those belong to the dedicated Mission Control surface.

For longer orchestration sessions, use the dedicated Mission Control tab flow:

```bash
aoc-mission-control-tab
# or

aoc-new-tab --mission-control
```

Outside Zellij, bootstrap directly into the dedicated layout with:

```bash
AOC_LAYOUT=mission-control aoc-launch
```

The dedicated layout lives at `.aoc/layouts/mission-control.kdl` and currently
starts:
- Mission Control
- Taskmaster
- an operator shell

## 9) Logs

Default log dir: `.aoc/logs` (can override with `AOC_LOG_DIR`).

Hub logs:
- `.aoc/logs/aoc-hub-<session_id>.log`

Wrapper logs:
- `.aoc/logs/aoc-agent-wrap-<session_id>-<agent_id>.log`

Mission Control logs:
- Currently stdout/stderr only (no log file)

## 10) Debug Checklist

### A) Mission Control does not see agents
1) Ensure the agent pane was launched through wrappers.
2) Inside the agent pane:
   ```
   echo "$AOC_SESSION_ID"
   echo "$AOC_HUB_ADDR"
   echo "$AOC_AGENT_ID"
   ```
3) If empty, restart the agent tab so it picks up env.
4) Confirm the hub is running:
   ```
   curl "http://$AOC_HUB_ADDR/health"
   ```

### B) MC keeps creating new panes
1) Verify you are using the updated toggle script (`bin/aoc-mission-control-toggle`).
2) Confirm the keybind uses it (in `~/.config/zellij/aoc.config.kdl`).
3) Ensure the PID file exists after the first open:
   `~/.local/state/aoc/mission-control-<session>.pid`.

### C) MC does not hide on Esc
1) Rebuild `aoc-mission-control` and restart MC:
   `cargo build -p aoc-mission-control`
2) Esc hides floating panes in the current tab; on Zellij `>= 0.44.0` AOC prefers explicit `hide-floating-panes --tab-id ...`, otherwise it falls back to toggle behavior.

### D) Hub 500 errors
1) Ensure hub is rebuilt after code changes.
2) Restart the hub process so it picks up changes.

### E) Need direct pane evidence
On Zellij `>= 0.44.0`, use the operator drilldown helper:
```
aoc-pane-evidence --pane-id <pane-id>
aoc-pane-evidence --pane-id <pane-id> --follow --scrollback 300
```
- default mode captures a bounded full-screen snapshot via `dump-screen`
- `--follow` opens a live NDJSON stream via `zellij subscribe`

For a non-interactive live Mind runtime validation runbook, see `docs/mind-runtime-validation.md` and `scripts/pi/validate-mind-runtime-live.sh`.

## 11) End-to-End Smoke Test

1) Start new session:
```
aoc
```

2) Toggle Mission Control:
```
Alt+a
```

3) Verify hub health:
```
curl "http://$AOC_HUB_ADDR/health"
```

4) In MC, you should see the agent list populate and live message updates.
5) In Overview / Overseer / Mind mode, press `e` to capture pane evidence for the selected/focused worker into the local AOC state dir.
6) Press `E` to open a live floating follow pane for the selected/focused worker via `zellij subscribe --pane-id`.
7) Select a file with diffs and press `Enter` to fetch patch.

## 12) Files of Interest

- `bin/aoc-launch`
- `bin/aoc-new-tab`
- `bin/aoc-agent-run`
- `bin/aoc-agent-wrap`
- `bin/aoc-mission-control-toggle`
- `bin/aoc-mission-control`
- `crates/aoc-hub-rs/src/main.rs`
- `crates/aoc-agent-wrap-rs/src/main.rs`
- `crates/aoc-mission-control/src/main.rs`
- `~/.config/zellij/layouts/aoc.kdl`
- `~/.config/zellij/aoc.config.kdl`
