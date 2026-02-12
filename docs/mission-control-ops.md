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
  `AOC_AGENT_RUN=1 exec ${AOC_AGENT_CMD:-${AOC_CODEX_CMD:-aoc-agent-run}}`
- Taskmaster pane runs `aoc-taskmaster`.
- Other panes (yazi, widget, clock, terminal) get `AOC_SESSION_ID` and
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

Goal: keep tmux scrollback (Codex) while capturing stdout/stderr for streaming.

Wrapper chain (Codex case):
```
tmux
  -> aoc-agent-wrap-rs (Rust wrapper)
    -> aoc-agent-wrap (bootloader + handshake)
      -> codex
```

Wrapper chain (OC/CC/Gemini cases):
```
aoc-agent-wrap-rs
  -> aoc-agent-wrap (bootloader + handshake)
    -> agent CLI
```

How it is wired:
- `bin/aoc-agent-run` chooses agent based on `AOC_AGENT_ID` or state file.
- `bin/aoc-oc`, `bin/aoc-cc`, `bin/aoc-gemini`, and `bin/aoc-codex` all respect
  `AOC_AGENT_RUN=1` and exec `aoc-agent-wrap`.
- `bin/aoc-agent-wrap` resolves the real agent binary, runs the bootloader for
  the handshake, and then (if available) wraps the bootloader with
  `aoc-agent-wrap-rs`.
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
- `Tab`: switch focus (Agents/Files)
- `j/k` or `Up/Down`: move selection
- `/`: search files
- `Enter`: request patch for selected file
- `r`: refresh patch
- `PageUp/PageDown`: scroll patch
- `Esc`: hides floating pane (runs `zellij action toggle-floating-panes`)
- `q`: quits Mission Control

Agent message display:
- Wrapper updates `agent_status.message` based on the latest non-empty line
  from stdout/stderr (rate-limited).
- UI shows `Message:` inside the agent details pane.

## 8) Mission Control Toggle Behavior

Shortcut (Zellij): `Alt+a`

`Alt+a` runs `aoc-mission-control-toggle`, which:
- Always opens Mission Control as a floating pane.
- Uses a PID file under `~/.local/state/aoc/mission-control-<session>.pid` to
  decide whether to toggle or create.
- If already running, it toggles floating panes instead of creating another.
- Closes the temporary split pane created by `Run` after launching MC.

Floating panes are tab-scoped in Zellij. You get one MC per tab.

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
2) Esc runs `zellij action toggle-floating-panes` (only if inside Zellij).

### D) Hub 500 errors
1) Ensure hub is rebuilt after code changes.
2) Restart the hub process so it picks up changes.

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
5) Select a file with diffs and press `Enter` to fetch patch.

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
