# Teach Deep Dive - API, State, and Queue Flows (20260215T181126Z UTC)

## Scope and confidence
- Scope: producer -> transport -> consumer paths for both legacy WebSocket hub and Pulse UDS path; command ack/error/retry behavior; queue and stale-prune mechanics.
- Confidence: high for current Rust paths (`aoc-hub-rs`, `aoc-agent-wrap-rs`, `aoc-mission-control`, `aoc-core`), medium for operational defaults that depend on runtime env toggles.

## 1) End-to-end message flow (producer -> transport -> consumer -> ack/retry/error)

### Concept in plain English
Think of this as a local event bus with two lanes:
1) WebSocket lane (legacy/compat path) for JSON envelopes.
2) Pulse UDS lane (vNext) for NDJSON envelopes with typed messages.

Wrappers produce state, hub normalizes/fans out, Mission Control consumes snapshots+deltas, and command flows run back from subscriber to publisher with explicit `command_result` responses.

### How this repo implements it

#### Producer (wrapper)
- Wrapper starts with heartbeat/task/diff workers and pushes status updates into outbound channels (`crates/aoc-agent-wrap-rs/src/main.rs:463`, `crates/aoc-agent-wrap-rs/src/main.rs:520`, `crates/aoc-agent-wrap-rs/src/main.rs:528`).
- WebSocket producer path reconnects with exponential backoff and replays cached state after reconnect (`crates/aoc-agent-wrap-rs/src/main.rs:643`, `crates/aoc-agent-wrap-rs/src/main.rs:674`, `crates/aoc-agent-wrap-rs/src/main.rs:725`).
- Pulse producer path connects to UDS, sends hello + initial upsert, then emits heartbeat/status/delta updates (`crates/aoc-agent-wrap-rs/src/main.rs:749`, `crates/aoc-agent-wrap-rs/src/main.rs:790`, `crates/aoc-agent-wrap-rs/src/main.rs:834`).

#### Transport (hub)
- WebSocket endpoint validates handshake, session ID, role/agent identity, then routes by message type (`crates/aoc-hub-rs/src/main.rs:782`, `crates/aoc-hub-rs/src/main.rs:820`, `crates/aoc-hub-rs/src/main.rs:525`).
- Pulse UDS path requires first message as `hello`, validates protocol/session, and then routes typed `WireMsg` variants (`crates/aoc-hub-rs/src/pulse_uds.rs:1295`, `crates/aoc-hub-rs/src/pulse_uds.rs:1322`, `crates/aoc-hub-rs/src/pulse_uds.rs:1390`).
- State fanout happens via snapshot on subscribe and delta broadcasts (`crates/aoc-hub-rs/src/pulse_uds.rs:364`, `crates/aoc-hub-rs/src/pulse_uds.rs:434`, `crates/aoc-hub-rs/src/pulse_uds.rs:383`).

#### Consumer (Mission Control)
- Mission Control connects to Pulse UDS, sends `hello` and `subscribe` topics, decodes NDJSON incrementally, and applies snapshot/delta/heartbeat/layout updates (`crates/aoc-mission-control/src/main.rs:3340`, `crates/aoc-mission-control/src/main.rs:3361`, `crates/aoc-mission-control/src/main.rs:3379`, `crates/aoc-mission-control/src/main.rs:3410`).
- It enforces monotonic sequence handling; delta gaps trigger reconnect for state repair (`crates/aoc-mission-control/src/main.rs:3415`, `crates/aoc-mission-control/src/main.rs:3419`).

#### Ack / retry / error paths
- Commands carry `request_id`; hub returns `command_result` and caches per `(conn_id, request_id)` for dedupe/idempotent replay (`crates/aoc-hub-rs/src/pulse_uds.rs:1193`, `crates/aoc-hub-rs/src/pulse_uds.rs:1232`, `crates/aoc-hub-rs/src/pulse_uds.rs:1246`).
- Mission Control tracks pending commands; terminal statuses clear pending state (`crates/aoc-mission-control/src/main.rs:864`, `crates/aoc-mission-control/src/main.rs:923`).
- Wrapper validates target command and returns explicit error codes for unsupported/invalid target (`crates/aoc-agent-wrap-rs/src/main.rs:1149`, `crates/aoc-agent-wrap-rs/src/main.rs:1157`, `crates/aoc-agent-wrap-rs/src/main.rs:1178`).
- Retry/backoff appears on all key connections (wrapper->hub WS, wrapper->Pulse UDS, mission-control->Pulse UDS) (`crates/aoc-agent-wrap-rs/src/main.rs:649`, `crates/aoc-agent-wrap-rs/src/main.rs:752`, `crates/aoc-mission-control/src/main.rs:3345`, `crates/aoc-agent-wrap-rs/src/main.rs:3256`).

### Tradeoffs and alternatives
- Current approach is resilient and local-first; reconnect + snapshot heals many transient failures.
- It does not provide guaranteed delivery semantics under pressure (intentional queue-drop policy).
- Alternative for stricter guarantees: bounded persistent replay log keyed by seq + per-client cursor.

### Verification/debug steps
- Verify handshake + session scoping: watch hub logs for `handshake_ok` and mismatch warnings (`crates/aoc-hub-rs/src/main.rs:869`, `crates/aoc-hub-rs/src/main.rs:923`).
- Simulate reconnect by stopping hub briefly; confirm wrapper and mission-control reconnect with backoff (`crates/aoc-agent-wrap-rs/src/main.rs:655`, `crates/aoc-mission-control/src/main.rs:3352`).
- Induce seq gap (drop deltas) and confirm mission-control requests reconnect (`crates/aoc-mission-control/src/main.rs:3419`).

## 2) Key structs/types, queue bounds/backpressure, stale pruning, failure handling

### Concept in plain English
The system relies on strict envelope schemas + bounded in-memory queues. Bounded queues protect responsiveness by dropping or disconnecting slow consumers rather than blocking producers.

### Key structs/types
- Shared wire model: `WireEnvelope`, `WireMsg`, `SnapshotPayload`, `DeltaPayload`, `AgentState`, `CommandPayload`, `CommandResultPayload` (`crates/aoc-core/src/pulse_ipc.rs:92`, `crates/aoc-core/src/pulse_ipc.rs:106`, `crates/aoc-core/src/pulse_ipc.rs:140`, `crates/aoc-core/src/pulse_ipc.rs:198`, `crates/aoc-core/src/pulse_ipc.rs:221`, `crates/aoc-core/src/pulse_ipc.rs:230`).
- NDJSON decode safety: `NdjsonFrameDecoder` with malformed-line recovery and oversized-buffer guards (`crates/aoc-core/src/pulse_ipc.rs:319`, `crates/aoc-core/src/pulse_ipc.rs:342`, `crates/aoc-core/src/pulse_ipc.rs:362`).
- Pulse hub state containers: `PulseUdsHub`, `SubscriberEntry`, `AgentRecord`, `CommandCacheEntry` (`crates/aoc-hub-rs/src/pulse_uds.rs:233`, `crates/aoc-hub-rs/src/pulse_uds.rs:200`, `crates/aoc-hub-rs/src/pulse_uds.rs:206`, `crates/aoc-hub-rs/src/pulse_uds.rs:212`).
- Legacy WS hub state containers: `HubState`, `AgentState`, `Client` (`crates/aoc-hub-rs/src/main.rs:225`, `crates/aoc-hub-rs/src/main.rs:232`, `crates/aoc-hub-rs/src/main.rs:185`).

### Queue bounds and backpressure behavior
- Legacy WS per-connection outbound queue: `mpsc::channel::<Message>(256)` (`crates/aoc-hub-rs/src/main.rs:784`).
- Pulse UDS per-connection queue: `mpsc::channel::<WireEnvelope>(self.config.queue_capacity)` (`crates/aoc-hub-rs/src/pulse_uds.rs:1349`).
- Mission Control command queue: `COMMAND_QUEUE_CAPACITY = 64` with `try_send` drop logging (`crates/aoc-mission-control/src/main.rs:56`, `crates/aoc-mission-control/src/main.rs:920`, `crates/aoc-mission-control/src/main.rs:934`).
- Wrapper pulse update queue uses `try_send`; full queue drops update (`crates/aoc-agent-wrap-rs/src/main.rs:729`).
- Pulse hub explicitly drops/disconnects slow consumers and emits `pulse_queue_drop` / `pulse_send_backpressure` (`crates/aoc-hub-rs/src/pulse_uds.rs:467`, `crates/aoc-hub-rs/src/pulse_uds.rs:489`, `crates/aoc-hub-rs/src/pulse_uds.rs:532`).

### Stale session pruning
- Legacy WS: stale reaper closes clients by `last_seen`, then may emit synthetic offline status (`crates/aoc-hub-rs/src/main.rs:473`, `crates/aoc-hub-rs/src/main.rs:493`, `crates/aoc-hub-rs/src/main.rs:309`).
- Pulse UDS: stale reaper removes agents without timely heartbeat (`crates/aoc-hub-rs/src/pulse_uds.rs:574`, `crates/aoc-hub-rs/src/pulse_uds.rs:608`).
- Pulse UDS layout watcher prunes agents for closed panes (`crates/aoc-hub-rs/src/pulse_uds.rs:715`, `crates/aoc-hub-rs/src/pulse_uds.rs:828`).

### Failure handling patterns
- Frame safety limits: envelope size caps and decode error skip/continue (`crates/aoc-hub-rs/src/main.rs:804`, `crates/aoc-hub-rs/src/main.rs:903`, `crates/aoc-hub-rs/src/pulse_uds.rs:1541`).
- Write timeout safety on both hubs (`crates/aoc-hub-rs/src/main.rs:785`, `crates/aoc-hub-rs/src/pulse_uds.rs:1517`).
- Version/session mismatch drops or disconnects to preserve isolation (`crates/aoc-hub-rs/src/main.rs:824`, `crates/aoc-hub-rs/src/pulse_uds.rs:1312`, `crates/aoc-hub-rs/src/pulse_uds.rs:1380`).

### Practical takeaway
This design is optimized for operator correctness and bounded latency, not guaranteed delivery. For cockpit telemetry, that is usually the right default.

## 3) Command routing map (hub/core/clients + docs)

### Concept in plain English
Commands are subscriber-originated control actions that the hub authorizes/routs to publisher wrappers. The wrapper executes locally and returns a typed result.

### Routing map
1. Mission Control creates command with `request_id` and `target_agent_id` (`crates/aoc-mission-control/src/main.rs:902`, `crates/aoc-mission-control/src/main.rs:3527`).
2. Mission Control sends `WireMsg::Command` over UDS (`crates/aoc-mission-control/src/main.rs:3447`).
3. Pulse hub checks caller role is subscriber and validates target in same session (`crates/aoc-hub-rs/src/pulse_uds.rs:956`, `crates/aoc-hub-rs/src/pulse_uds.rs:1009`).
4. Hub forwards command to matching publisher connection(s), then returns immediate `accepted` or error (`crates/aoc-hub-rs/src/pulse_uds.rs:1043`, `crates/aoc-hub-rs/src/pulse_uds.rs:1059`).
5. Wrapper receives command, validates support/target, returns `CommandResult`, and optionally self-interrupts for `stop_agent` (`crates/aoc-agent-wrap-rs/src/main.rs:854`, `crates/aoc-agent-wrap-rs/src/main.rs:1149`, `crates/aoc-agent-wrap-rs/src/main.rs:1216`).
6. Hub broadcasts command result to subscribers and caches by `(conn_id, request_id)` for dedupe/replay (`crates/aoc-hub-rs/src/pulse_uds.rs:1422`, `crates/aoc-hub-rs/src/pulse_uds.rs:1213`, `crates/aoc-hub-rs/src/pulse_uds.rs:1232`).
7. Mission Control consumes result and clears pending state (`crates/aoc-mission-control/src/main.rs:3432`, `crates/aoc-mission-control/src/main.rs:864`).

### Where protocol is documented
- Canonical wire contract: `docs/pulse-ipc-protocol.md`.
- Shared type definitions used by all three components: `crates/aoc-core/src/pulse_ipc.rs`.
- System-level architecture context: `docs/mission-control.md`.

### Practical takeaway
The request/response loop is explicit, typed, and session-safe. That is a strong foundation for adding more commands later.

## 4) Strengths, fragile points, and quick hardening opportunities

### Strengths
- Strong session isolation and handshake validation in both transport lanes (`crates/aoc-hub-rs/src/main.rs:820`, `crates/aoc-hub-rs/src/pulse_uds.rs:1312`).
- Good resilience mechanics: reconnect backoff, snapshots for late joiners, seq-gap detection in client (`crates/aoc-agent-wrap-rs/src/main.rs:649`, `crates/aoc-hub-rs/src/pulse_uds.rs:364`, `crates/aoc-mission-control/src/main.rs:3419`).
- Backpressure is explicit and observable via structured events (`crates/aoc-hub-rs/src/pulse_uds.rs:475`, `crates/aoc-hub-rs/src/pulse_uds.rs:532`, `docs/pulse-vnext-rollout.md:24`).

### Fragile points
- Drop-on-full can hide critical transient state under heavy churn (by design) (`crates/aoc-hub-rs/src/pulse_uds.rs:489`, `crates/aoc-agent-wrap-rs/src/main.rs:733`).
- Mission Control command queue uses non-blocking `try_send`, so operator actions can be dropped during bursts (`crates/aoc-mission-control/src/main.rs:920`, `crates/aoc-mission-control/src/main.rs:934`).
- Layout watcher quality depends on external `zellij dump-layout`; parse or command failures impact pane-prune fidelity (`crates/aoc-hub-rs/src/pulse_uds.rs:694`, `crates/aoc-hub-rs/src/pulse_uds.rs:1583`).

### Quick hardening opportunities
1. Add a tiny replay buffer for latest N deltas per topic per subscriber to reduce data loss after short backpressure events.
2. Add command timeout + explicit "expired" status in Mission Control for pending entries with no terminal result.
3. Add integration tests for full control loop: `command` -> wrapper execution -> `command_result` with reconnect and dedupe scenarios.
4. Expose queue occupancy gauges (not just drop counters) for earlier saturation detection.

## 5) Verification checklist (practical)
- Protocol validation: compare runtime envelopes against `docs/pulse-ipc-protocol.md` and `crates/aoc-core/src/pulse_ipc.rs`.
- Backpressure behavior: monitor `pulse_queue_drop` and `pulse_send_backpressure` while stressing multiple subscribers (`crates/aoc-hub-rs/src/pulse_uds.rs:475`).
- Command loop: issue `stop_agent` from Mission Control and verify accepted -> wrapper result -> pending clear (`crates/aoc-hub-rs/src/pulse_uds.rs:1059`, `crates/aoc-agent-wrap-rs/src/main.rs:1200`, `crates/aoc-mission-control/src/main.rs:864`).
- Stale cleanup: kill a publisher and verify stale/prune events remove ghost rows (`crates/aoc-hub-rs/src/pulse_uds.rs:574`, `crates/aoc-hub-rs/src/pulse_uds.rs:828`).
