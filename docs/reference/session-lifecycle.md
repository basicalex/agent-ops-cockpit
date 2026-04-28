# Session lifecycle

AOC runs inside Zellij with Pi-first agent panes, project-local metadata, and a lightweight hub for runtime state. This document explains how an AOC session starts, adds tabs, publishes metadata, and reconnects operator surfaces.

## Mental model

```text
aoc-launch
  -> choose project/layout/session id
  -> start Zellij layout
  -> start hub + panes
  -> wrap Pi agents with AOC env
  -> publish tab/session metadata
  -> Mission Control / Mind / Taskmaster observe state
```

## Main commands

| Command | Purpose |
|---|---|
| `aoc` | normal AOC entrypoint |
| `aoc-launch` | start/attach AOC Zellij session |
| `aoc-new-tab` | create new AOC-managed tab |
| `aoc-tab-metadata` | publish tab name/scope/position metadata |
| `aoc-session-state` | record/inspect project session state where enabled |
| `aoc-agent-wrap` / `aoc-agent-wrap-rs` | wrap Pi agent with AOC env, telemetry, Mind/hub bridges |
| `aoc-hub` / `aoc-hub-rs` | session-local websocket hub |
| `aoc-mission-control` | operator UI over hub/local fallback state |
| `aoc-control` | Alt+C control pane |

## Startup flow

```text
1. Resolve project root.
2. Load `.aoc/context.md`, layout, presets, and project config.
3. Choose or reuse a session id/name.
4. Start Zellij with the selected AOC layout.
5. Start one hub per session when configured.
6. Launch panes with AOC environment variables.
7. Pi agent panes run through AOC wrapper.
8. Metadata commands publish tab/project/session state.
9. Mission Control, Mind, Taskmaster, and widgets subscribe or fall back locally.
```

## Environment contract

Common env passed through layouts/wrappers:

| Variable | Purpose |
|---|---|
| `AOC_PROJECT_ROOT` | canonical project root |
| `AOC_SESSION_ID` | AOC session identity |
| `AOC_HUB_ADDR` | local hub host:port |
| `AOC_HUB_URL` | websocket URL for clients |
| `AOC_LAYOUT` | selected layout name |
| `AOC_AGENT` | selected agent kind/model surface |
| `AOC_TAB_SCOPE` | semantic tab scope when known |
| `ZELLIJ_PROJECT_ROOT` | project root for Zellij layout commands |

See also [Layouts](../layouts.md).

## Hub behavior

The hub is local to the AOC/Zellij session. It carries runtime snapshots and events such as:

- agent status
- active tasks
- diff summaries
- pane/tab metadata
- Mind/insight detached job snapshots
- Mission Control command results

If the hub is down or missing data, operator surfaces should degrade gracefully to local snapshots when possible.

Protocol details: [Pulse IPC protocol](pulse-ipc-protocol.md).

## Tab metadata

AOC tabs are more than pane names. Metadata helps Mission Control, Mind, and widgets know which project/workstream a tab belongs to.

Typical metadata:

- project root
- tab name
- tab position
- scope/mode
- active preset
- session id

`aoc-tab-metadata` is usually invoked by layouts or tab startup hooks, not by hand.

## Agent wrapper

The wrapper owns Pi-first runtime integration:

- inject AOC env and project context
- perform startup handshake
- bridge status/activity to hub
- support Mind ingestion/finalization paths
- redact secret-like activity snippets before durable storage/broadcast
- preserve AOC command/tool behavior across tabs

Mind details: [AOC Mind architecture](aoc-mind-architecture.md).

## Reconnect and fallback

AOC prefers resilient local behavior:

- existing Zellij session can be reattached
- hub clients reconnect to fresh snapshots
- Mission Control falls back if hub state is unavailable
- agents can still inspect files/tasks without Mind/hub availability
- RTK routing fails open for safe allowlisted commands

## Common lifecycle problems

### New tab appears without AOC metadata

Run:

```bash
aoc-tab-metadata sync
```

or create tabs through `aoc-new-tab` / AOC keybindings.

### Mission Control shows stale/no hub data

Check:

```bash
aoc-mission-control
```

Then verify session env:

```bash
echo "$AOC_HUB_URL"
echo "$AOC_SESSION_ID"
```

### Agent pane starts outside project

Check:

```bash
echo "$AOC_PROJECT_ROOT"
pwd
```

Restart through `aoc-launch` or create a managed AOC tab.

## Boundaries

Session lifecycle is separate from:

- Taskmaster task state
- Mind semantic memory
- RTK command-output routing
- HyperFrames project workspace
- AOC Map generated microsites

It wires those systems together, but each owns its own persistence and validation.
