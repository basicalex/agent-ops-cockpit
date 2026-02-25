# Mission Control Architecture and Event Schema

## Goals
- Provide a per-session, local-only hub for agent status, task, and diff signals.
- Keep agents agnostic by sending summaries only (no parsing of agent output).
- Allow multiple UI clients (Mission Control, Taskmaster) to subscribe safely.
- Preserve strict session isolation and predictable message contracts.

## Components
- aoc-hub-rs (Rust websocket hub)
  - Session-scoped message router and state cache.
  - Enforces hello handshake, schema validation, and session isolation.
- aoc-agent-wrap-rs (Rust wrapper)
  - Launches any agent command and streams status, task summary, and diff summary.
  - Watches .taskmaster JSON and git state with debounce.
- aoc-taskmaster (Rust Ratatui TUI)
  - Reads/writes Taskmaster JSON; optionally publishes task_update events.
- aoc-mission-control (Rust Ratatui TUI)
  - Subscribes to hub state, shows per-agent diffs, requests patches on demand.

## Session Scoping and Environment
Each Zellij session is an isolation boundary. Every message must include
the session_id and the hub must reject mismatched sessions.

### Environment Variables
| Env var | Purpose | Default / Derivation |
|--------|---------|----------------------|
| AOC_SESSION_ID | Unique session identifier used for routing | Prefer ZELLIJ_SESSION_NAME; else existing AOC_SESSION_ID; else generated "<noun>-<verb>" |
| AOC_PANE_ID | Pane identifier for this client | Prefer ZELLIJ_PANE_ID; else "pid-<pid>" |
| AOC_AGENT_ID | Human-readable agent label metadata | Prefer explicit AOC_AGENT_ID; else project name |
| AOC_AGENT_LABEL | Optional explicit label for display only | Empty |
| AOC_PROJECT_ROOT | Project root used for task and git scans | Prefer AOC_PROJECT_ROOT; else current working directory |
| AOC_HUB_ADDR | Hub listen address (host:port) | 127.0.0.1:<port-from-session> |
| AOC_HUB_URL | Websocket URL for hub | ws://AOC_HUB_ADDR/ws |
| AOC_TAB_SCOPE | Logical tab identity shared by panes in the same tab | Derived from launch layout tab name |
| AOC_PULSE_THEME | Pulse palette mode (`terminal`, `auto`, `dark`, `light`) | `terminal` |
| AOC_PULSE_OVERVIEW_ENABLED | Enable Pulse Overview mode in mission-control | 1 (enabled by default) |
| AOC_PULSE_LAYOUT_WATCH_ENABLED | Enable hub background layout watcher (`dump-layout`) | 0 (disabled by default) |
| AOC_PULSE_LAYOUT_WATCH_MS | Hub layout poll interval while layout subscribers are active | 3000 ms |
| AOC_PULSE_LAYOUT_IDLE_WATCH_MS | Hub layout poll interval when no layout subscribers are active | max(4x active, 12000 ms) |
| AOC_MISSION_CONTROL_LAYOUT_REFRESH_MS | Mission Control local layout refresh interval (fallback/overview) | 3000 ms |
| AOC_LOG_DIR | Log output directory | .aoc/logs |

## AOC Pulse Data Source Strategy

The default top-right pane is **AOC Pulse** and starts in Overview mode.
Work, Diff, and Health remain available as companion operational modes.

By default, Pulse uses `AOC_PULSE_THEME=terminal`, which keeps the pane surface
and text aligned with the active terminal/system theme.

### v1 Fallback (No Hub Required)
- Pulse must run headless-safe and useful even when hub is down.
- Data sources are local-only:
  - runtime/process introspection for session/pane liveness
  - Taskmaster JSON for work state
  - git status/diff for repo change summaries
  - dependency/marker checks for health

### v2 Preferred (Hub Available)
- If hub is reachable, Pulse prefers hub snapshots/events for:
  - `agent_status` + `heartbeat` (Overview, when enabled)
  - `task_summary` (Work)
  - `diff_summary` (Diff)
- If hub disconnects or lacks data, Pulse automatically falls back to v1.

### Overview Availability
- Overview is enabled by default (`AOC_PULSE_OVERVIEW_ENABLED=1`).
- Set `AOC_PULSE_OVERVIEW_ENABLED=0` to run Work/Diff/Health-only mode.
- Layout polling remains decoupled; keep `AOC_PULSE_LAYOUT_WATCH_ENABLED=0`
  unless layout-state streaming is explicitly required.

### Identity Model (Collision-Safe)
- Primary publisher/consumer identity key is always:
  - `agent_id = "<session_id>::<pane_id>"`
- Human labels (`AOC_AGENT_ID` / `AOC_AGENT_LABEL`) are metadata only.
- This prevents collisions when multiple tabs use the same label (for example, multiple `codex` panes).

### Port Derivation
To avoid collisions across sessions, derive a stable port from session_id:

```
port = 42000 + (fnv1a_32(session_id) % 2000)
```

This yields a port in 42000-43999. AOC_HUB_ADDR can override this.

## Data Flow and Lifecycle
1. aoc-hub-rs starts once per Zellij session and listens on AOC_HUB_ADDR.
2. Clients connect and send a hello message with session_id and role.
3. Hub validates session_id and role, then accepts publish/subscribe.
4. Publishers (wrappers/taskmaster) send status, task, and diff summaries.
5. Hub caches last-known state per agent_id and sends a snapshot to new
   subscribers (agent_status, diff_summary, task_summary for each agent).
6. Subscribers request patches on demand via diff_patch_request.
7. Heartbeats keep liveness; hub prunes stale clients and cached state.

## Rust Implementation Notes
- Runtime: tokio for async; axum websockets in the hub; tokio-tungstenite for clients.
- UI: ratatui + crossterm for Mission Control and Taskmaster TUIs.
- Logging: tracing + tracing-subscriber; honor AOC_LOG_LEVEL and AOC_LOG_DIR; log files are per-session and component.
- Binaries: aoc-hub-rs, aoc-agent-wrap-rs, aoc-taskmaster, aoc-mission-control.

## Concurrency and Backpressure
- Use bounded channels between IO and UI; drop or disconnect on persistent backpressure.
- Hub keeps last-known state per agent and broadcasts snapshots to new subscribers.
- Publishers debounce filesystem and git updates; prefer last-write-wins summaries.
- Subscribers request patches on demand; no polling or streaming patches.

## Message Envelope
All messages are JSON objects with a fixed envelope.

Required fields:
- version (string): Protocol version. Start with "1".
- type (string): Message type string.
- session_id (string): Must match hub session_id.
- sender_id (string): Unique sender identity.
- timestamp (string): RFC3339 UTC timestamp.
- payload (object): Message-specific payload.

Optional fields:
- request_id (string): For request/response correlation.

Example envelope:

```json
{
  "version": "1",
  "type": "diff_summary",
  "session_id": "aoc-dev",
  "sender_id": "agent-1",
  "timestamp": "2026-01-28T18:42:00Z",
  "payload": {}
}
```

## Message Types

### hello
Handshake required before publish or subscribe.

Required payload fields:
- client_id (string): Must equal sender_id.
- role (string): "publisher" or "subscriber".
- capabilities (array): List of supported message types.

Optional payload fields:
- agent_id (string): Required if role=publisher and representing an agent.
- pane_id (string)
- project_root (string)

Example:

```json
{
  "version": "1",
  "type": "hello",
  "session_id": "aoc-dev",
  "sender_id": "agent-1",
  "timestamp": "2026-01-28T18:42:00Z",
  "payload": {
    "client_id": "agent-1",
    "role": "publisher",
    "capabilities": ["agent_status", "diff_summary", "task_summary", "heartbeat"],
    "agent_id": "agent-1",
    "pane_id": "pane-3",
    "project_root": "/home/user/dev/agent-ops-cockpit"
  }
}
```

### agent_status
Indicates agent lifecycle and basic state.

Required payload fields:
- agent_id (string)
- status (string): "idle" | "running" | "error" | "offline"
- pane_id (string)
- project_root (string)

Optional payload fields:
- cwd (string)
- message (string)

### diff_summary
Summary of git changes for the agent's project_root.

Required payload fields:
- agent_id (string)
- repo_root (string)
- git_available (boolean)
- summary (object)
- files (array)

If git_available is false, include:
- reason (string): "not_git_repo" | "git_missing" | "error"

### diff_patch_request
Request a patch for a specific file.

Required payload fields:
- agent_id (string)
- path (string)

Optional payload fields:
- context_lines (number, default 3)
- include_untracked (boolean, default true)
- request_id (string)

### diff_patch_response
Response to diff_patch_request.

Required payload fields:
- agent_id (string)
- path (string)
- status (string): "modified" | "added" | "deleted" | "renamed" | "untracked"
- is_binary (boolean)
- patch (string or null)

Optional payload fields:
- request_id (string)
- error (object)

### task_summary
Summary of Taskmaster state for the agent.

Required payload fields:
- agent_id (string)
- tag (string)
- counts (object): total, pending, in_progress, done, blocked

Optional payload fields:
- active_tasks (array): {id, title, status, priority, active_agent}
- error (object)

### task_update
Event emitted when a task changes (optional; wrappers may rely on file watch).

Required payload fields:
- agent_id (string)
- tag (string)
- action (string): "update" | "add" | "delete"
- task (object)

### heartbeat
Periodic liveness signal.

Required payload fields:
- agent_id (string)
- pid (number)
- cwd (string)
- last_update (string, RFC3339)

### error
Report local errors to the hub or subscribers.

Required payload fields:
- code (string)
- message (string)

Optional payload fields:
- component (string): "hub" | "wrapper" | "taskmaster" | "mission-control"
- fatal (boolean)
- context (object)

## Diff Summary Schema
diff_summary.payload.summary:

```
{
  "staged": {"files": 0, "additions": 0, "deletions": 0},
  "unstaged": {"files": 0, "additions": 0, "deletions": 0},
  "untracked": {"files": 0}
}
```

diff_summary.payload.files entries:

```
{
  "path": "relative/path.txt",
  "status": "modified|added|deleted|renamed|untracked",
  "additions": 3,
  "deletions": 1,
  "staged": false,
  "untracked": false
}
```

Untracked handling:
- Include untracked files in files[] with status="untracked".
- additions/deletions may be 0 for untracked unless computed separately.

## Diff Patch Response Schema (Untracked Handling)
For tracked files, patch contains a unified diff (git diff).

For untracked text files:
- status="untracked"
- patch contains a unified diff against /dev/null (git diff --no-index)

For untracked binary files or oversized patches:
- patch is null
- error.code = "patch_unavailable"
- error.message explains "binary" or "too_large"

## Update Cadence and Throttling
- agent_status: on start, on status change, on graceful exit.
- diff_summary: debounce 500ms on git changes, max 1 update per 2s.
- task_summary: debounce 500ms on tasks.json changes.
- heartbeat: every 5-10s; hub drops clients after 30s of silence.
- diff_patch_request: on demand only; no polling.

## Error Semantics and Degraded States
When a dependency is missing or invalid, publishers should emit a degraded
state rather than omitting messages:

- Non-git repo:
  - diff_summary.git_available=false
  - diff_summary.reason="not_git_repo"
- Git missing or failed:
  - diff_summary.git_available=false
  - diff_summary.reason="git_missing" or "error"
- Missing tasks.json:
  - task_summary.error.code="tasks_missing"
  - counts set to zero
- Malformed tasks.json:
  - task_summary.error.code="tasks_malformed"
  - include error.message with parse details

Clients should surface these errors in their UI and continue to function
for other signals.

## Security Constraints
- Hub must bind to 127.0.0.1 only.
- Reject any message without hello or with mismatched session_id.
- Enforce size limits:
  - max envelope size: 256 KB
  - max patch size: 1 MB
  - max files list length: 500
- Drop invalid JSON or schema violations and emit error to sender.

## Example Messages

diff_summary:

```json
{
  "version": "1",
  "type": "diff_summary",
  "session_id": "aoc-dev",
  "sender_id": "agent-1",
  "timestamp": "2026-01-28T18:42:05Z",
  "payload": {
    "agent_id": "agent-1",
    "repo_root": "/home/user/dev/agent-ops-cockpit",
    "git_available": true,
    "summary": {
      "staged": {"files": 0, "additions": 0, "deletions": 0},
      "unstaged": {"files": 2, "additions": 8, "deletions": 1},
      "untracked": {"files": 1}
    },
    "files": [
      {
        "path": "docs/mission-control.md",
        "status": "modified",
        "additions": 8,
        "deletions": 1,
        "staged": false,
        "untracked": false
      },
      {
        "path": "notes/todo.txt",
        "status": "untracked",
        "additions": 0,
        "deletions": 0,
        "staged": false,
        "untracked": true
      }
    ]
  }
}
```

diff_patch_request:

```json
{
  "version": "1",
  "type": "diff_patch_request",
  "session_id": "aoc-dev",
  "sender_id": "mission-control",
  "timestamp": "2026-01-28T18:42:10Z",
  "request_id": "req-001",
  "payload": {
    "agent_id": "agent-1",
    "path": "docs/mission-control.md",
    "context_lines": 3,
    "include_untracked": true,
    "request_id": "req-001"
  }
}
```

diff_patch_response:

```json
{
  "version": "1",
  "type": "diff_patch_response",
  "session_id": "aoc-dev",
  "sender_id": "agent-1",
  "timestamp": "2026-01-28T18:42:11Z",
  "request_id": "req-001",
  "payload": {
    "agent_id": "agent-1",
    "path": "docs/mission-control.md",
    "status": "modified",
    "is_binary": false,
    "patch": "@@ -1,3 +1,4 @@\n-Old line\n+New line\n"
  }
}
```

task_summary:

```json
{
  "version": "1",
  "type": "task_summary",
  "session_id": "aoc-dev",
  "sender_id": "agent-1",
  "timestamp": "2026-01-28T18:42:15Z",
  "payload": {
    "agent_id": "agent-1",
    "tag": "mission-control",
    "counts": {"total": 9, "pending": 9, "in_progress": 0, "done": 0, "blocked": 0},
    "active_tasks": [
      {"id": "43", "title": "Define Mission Control architecture", "status": "pending", "priority": "high", "active_agent": false}
    ]
  }
}
```

heartbeat:

```json
{
  "version": "1",
  "type": "heartbeat",
  "session_id": "aoc-dev",
  "sender_id": "agent-1",
  "timestamp": "2026-01-28T18:42:20Z",
  "payload": {
    "agent_id": "agent-1",
    "pid": 12345,
    "cwd": "/home/user/dev/agent-ops-cockpit",
    "last_update": "2026-01-28T18:42:20Z"
  }
}
```

error:

```json
{
  "version": "1",
  "type": "error",
  "session_id": "aoc-dev",
  "sender_id": "agent-1",
  "timestamp": "2026-01-28T18:42:25Z",
  "payload": {
    "code": "tasks_malformed",
    "message": "Failed to parse tasks.json: unexpected character at line 12",
    "component": "wrapper",
    "fatal": false,
    "context": {"path": ".taskmaster/tasks/tasks.json"}
  }
}
```
