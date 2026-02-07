# Pulse IPC Protocol (NDJSON)

This document defines the shared Pulse IPC wire format for `aoc-hub-rs`, `aoc-agent-wrap-rs`, and `aoc-mission-control`.

## Framing

- Transport framing is NDJSON: one JSON envelope per line.
- Maximum frame size is 256 KiB (`DEFAULT_MAX_FRAME_BYTES`) unless an endpoint overrides it.
- Receivers should continue processing after malformed lines.
- Receivers should drop oversized frames and continue processing subsequent lines.

## Envelope Schema

Each line contains one envelope:

```json
{
  "version": "1",
  "type": "hello",
  "session_id": "aoc-session",
  "sender_id": "wrapper-42",
  "timestamp": "2026-02-07T21:00:00Z",
  "request_id": "optional-correlation-id",
  "payload": {}
}
```

- `version`: protocol version (string or integer on input; emitted as string)
- `type`: message type tag
- `session_id`: session scope key
- `sender_id`: unique sender identity
- `timestamp`: RFC3339 timestamp
- `request_id`: optional correlation key for commands
- `payload`: message-type-specific payload

## Message Types

The shared enum (`WireMsg`) currently supports:

- `hello`
- `subscribe`
- `snapshot`
- `delta`
- `heartbeat`
- `command`
- `command_result`

### hello

```json
{
  "version": "1",
  "type": "hello",
  "session_id": "aoc-session",
  "sender_id": "wrapper-42",
  "timestamp": "2026-02-07T21:00:00Z",
  "payload": {
    "client_id": "wrapper-42",
    "role": "publisher",
    "capabilities": ["state_update", "heartbeat"],
    "agent_id": "aoc-session::12",
    "pane_id": "12",
    "project_root": "/home/user/repo"
  }
}
```

### subscribe

```json
{
  "version": "1",
  "type": "subscribe",
  "session_id": "aoc-session",
  "sender_id": "pulse-client",
  "timestamp": "2026-02-07T21:00:02Z",
  "payload": {
    "topics": ["agent_state", "health"],
    "since_seq": 120
  }
}
```

### snapshot

```json
{
  "version": "1",
  "type": "snapshot",
  "session_id": "aoc-session",
  "sender_id": "aoc-hub",
  "timestamp": "2026-02-07T21:00:03Z",
  "payload": {
    "seq": 121,
    "states": [
      {
        "agent_id": "aoc-session::12",
        "session_id": "aoc-session",
        "pane_id": "12",
        "lifecycle": "running",
        "snippet": "indexing files",
        "last_heartbeat_ms": 1707335223000,
        "last_activity_ms": 1707335222950,
        "updated_at_ms": 1707335223000,
        "source": { "kind": "wrapper" }
      }
    ]
  }
}
```

### delta

```json
{
  "version": "1",
  "type": "delta",
  "session_id": "aoc-session",
  "sender_id": "aoc-hub",
  "timestamp": "2026-02-07T21:00:04Z",
  "payload": {
    "seq": 122,
    "changes": [
      {
        "op": "upsert",
        "agent_id": "aoc-session::12",
        "state": {
          "agent_id": "aoc-session::12",
          "session_id": "aoc-session",
          "pane_id": "12",
          "lifecycle": "needs_input"
        }
      }
    ]
  }
}
```

### heartbeat

```json
{
  "version": "1",
  "type": "heartbeat",
  "session_id": "aoc-session",
  "sender_id": "wrapper-42",
  "timestamp": "2026-02-07T21:00:05Z",
  "payload": {
    "agent_id": "aoc-session::12",
    "last_heartbeat_ms": 1707335225000,
    "lifecycle": "running"
  }
}
```

### command

```json
{
  "version": "1",
  "type": "command",
  "session_id": "aoc-session",
  "sender_id": "pulse-client",
  "timestamp": "2026-02-07T21:00:06Z",
  "request_id": "req-19",
  "payload": {
    "command": "stop_agent",
    "target_agent_id": "aoc-session::12",
    "args": { "reason": "user_request" }
  }
}
```

### command_result

```json
{
  "version": "1",
  "type": "command_result",
  "session_id": "aoc-session",
  "sender_id": "wrapper-42",
  "timestamp": "2026-02-07T21:00:06Z",
  "request_id": "req-19",
  "payload": {
    "command": "stop_agent",
    "status": "accepted",
    "message": "ctrl-c sent"
  }
}
```
