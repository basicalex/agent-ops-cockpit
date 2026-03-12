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
- `consultation_request`
- `consultation_response`

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

### consultation_request

```json
{
  "version": "1",
  "type": "consultation_request",
  "session_id": "aoc-session",
  "sender_id": "mission-control",
  "timestamp": "2026-02-07T21:00:07Z",
  "request_id": "consult-19",
  "payload": {
    "consultation_id": "consult-19",
    "requesting_agent_id": "aoc-session::12",
    "target_agent_id": "aoc-session::24",
    "packet": {
      "schema_version": 1,
      "packet_id": "packet-19",
      "kind": "review",
      "identity": {
        "session_id": "aoc-session",
        "agent_id": "aoc-session::12",
        "pane_id": "12",
        "role": "builder"
      },
      "summary": "Need review on migration sequencing"
    }
  }
}
```

### consultation_response

```json
{
  "version": "1",
  "type": "consultation_response",
  "session_id": "aoc-session",
  "sender_id": "wrapper-24",
  "timestamp": "2026-02-07T21:00:08Z",
  "request_id": "consult-19",
  "payload": {
    "consultation_id": "consult-19",
    "requesting_agent_id": "aoc-session::12",
    "responding_agent_id": "aoc-session::24",
    "status": "completed",
    "message": "review completed",
    "packet": {
      "schema_version": 1,
      "packet_id": "packet-20",
      "kind": "review",
      "identity": {
        "session_id": "aoc-session",
        "agent_id": "aoc-session::24",
        "pane_id": "24",
        "role": "reviewer"
      },
      "summary": "Rollback path still needs validation"
    }
  }
}
```

Notes:
- `consultation_request` is session-scoped and must target a worker in the same session.
- `payload.packet.identity.session_id` and `payload.requesting_agent_id` must agree.
- `consultation_response.status` is one of `accepted`, `completed`, `rejected`, or `failed`.
- subscribers must opt into `consultation_request` and/or `consultation_response` topics explicitly.
