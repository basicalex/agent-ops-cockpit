# PRD: Define Pulse vNext architecture and PRD

## Metadata
- Task ID: 62
- Tag: pulse-hub-spoke
- Status: pending
- Priority: high

## Problem
Pulse needs near-real-time, session-wide agent visibility without breaking interactive agent TUIs.

The previous PTY proxy approach proved brittle for mouse tracking and complex terminal rendering, and hub-only scraping via `zellij action dump-screen` is not viable for background panes in current Zellij.

We need an architecture that is:
- fast enough for Mission Control (target sub-second reconciliation)
- safe for interactive terminal behavior (no accidental terminal emulation bugs)
- deterministic for tab/pane lifecycle changes
- incremental to adopt within existing Rust crates (`aoc-hub-rs`, `aoc-agent-wrap-rs`, `aoc-mission-control`)

## Goals
- Implement a passive hub and active wrapper telemetry model.
- Standardize a shared Pulse IPC protocol for snapshot + delta state streaming.
- Achieve predictable pane close/open handling with low-latency overview updates.
- Preserve transparent PTY passthrough so agent UX is unchanged.
- Provide command routing that does not depend on non-targetable Zellij actions.
- Ship with observability, latency metrics, and rollback guardrails.

## Non-Goals
- Build a full terminal emulator in AOC components.
- Depend on `zellij action dump-screen` for non-focused pane telemetry.
- Replace all existing websocket paths in one cutover without compatibility gates.
- Introduce multi-worktree behavior or repository isolation changes.
- Implement ML/NLP-heavy state inference for v1 parser; regex + heuristic logic is sufficient.

## Requirements
- **Architecture boundary**
  - `aoc-hub-rs` is state keeper and broadcaster.
  - `aoc-agent-wrap-rs` is transparent runtime publisher.
  - `aoc-mission-control` consumes snapshot+deltas and emits commands.
  - Shared protocol/types live in reusable Rust module (`aoc-core` preferred).
- **Identity model**
  - Primary key remains `agent_id = <session_id>::<pane_id>`.
  - Human labels are metadata only.
- **Transport**
  - Local Unix Domain Socket (UDS), session-scoped path:
    `/run/user/<uid>/aoc/<session_slug>/pulse.sock`.
  - Socket permissions `0600`.
- **Hub state**
  - In-memory store keyed by identity with heartbeat timestamps and status payload.
  - Snapshot-on-subscribe plus ordered deltas.
  - Per-subscriber bounded queues and slow-consumer strategy.
- **Lifecycle**
  - Layout watcher uses `zellij action dump-layout` (fast cadence) for pane topology.
  - Immediate state prune on confirmed pane close.
  - Heartbeat-based stale eviction as safety net.
- **Wrapper tap behavior**
  - PTY I/O remains transparent user <-> child process path.
  - Tap copies output bytes to bounded ring buffer for analysis only.
  - Debounced reporter emits updates on significant state/content changes.
- **Command model**
  - Commands route `Pulse -> Hub -> Wrapper` where wrapper owns child PTY control.
  - SIGINT semantics: inject Ctrl-C first, then escalate if child does not exit.
- **Compatibility**
  - Preserve existing fallback behavior when hub or wrapper telemetry unavailable.
  - Gate rollout behind feature/config switch until stable.

## Acceptance Criteria
- [ ] New tag `pulse-hub-spoke` contains implementation tasks for architecture, IPC, hub, wrapper, integration, and rollout.
- [ ] Task 62 is linked to this PRD document and covers full implementation contract.
- [ ] Shared Pulse IPC module defines message schema with tested encode/decode framing.
- [ ] Hub UDS service supports handshake, snapshot-on-connect, delta broadcast, and stale pruning.
- [ ] Wrapper telemetry works as transparent tap with no observed regression in agent interaction.
- [ ] Pane close/open events reconcile in Pulse within latency target (P95 <= 1.0s for close removal).
- [ ] SIGINT handling reaches child agent process reliably (Ctrl-C behavior preserved).
- [ ] Mission Control can consume new transport and render consistent overview/work/diff/health states.
- [ ] Metrics and logs expose latency, queue pressure, parser transitions, and watcher health.
- [ ] Rollout includes fallback and rollback path.

## Detailed Architecture

### Component Responsibilities
- **aoc-hub-rs**
  - Maintain authoritative per-session runtime state.
  - Accept publisher updates from wrappers.
  - Broadcast snapshots and deltas to pulse clients.
  - Reconcile topology via layout watcher and prune dead panes.
- **aoc-agent-wrap-rs**
  - Launch agent command in PTY mode.
  - Transparently pass stdin/stdout between user terminal and child.
  - Capture analyzed output in bounded ring buffer.
  - Publish debounced status updates + heartbeats.
  - Execute control commands against child PTY (Ctrl-C, optional typed input).
- **aoc-mission-control**
  - Subscribe to hub feed.
  - Merge snapshot + deltas into app state.
  - Render mode-specific UI and command interactions.
  - Surface degraded states and reconnection status.

### State Model
- `AgentState`
  - `agent_id`, `session_id`, `pane_id`, `tab_index?`, `tab_name?`
  - `lifecycle` (`running|idle|busy|needs_input|blocked|stale|offline|error`)
  - `snippet` (bounded)
  - `last_heartbeat_ms`, `last_activity_ms`, `updated_at_ms`
  - `source` metadata (`wrapper`, merge flags)
- `HubState`
  - map of `agent_id -> AgentState`
  - sequence counter for ordered deltas
  - subscriber registry and telemetry counters

### IPC Protocol
- NDJSON framing (one JSON envelope per line).
- Required envelope fields:
  - `version`, `type`, `session_id`, `sender_id`, `timestamp`, `payload`
- Core message types:
  - `hello`
  - `subscribe`
  - `snapshot`
  - `delta`
  - `heartbeat`
  - `state_update`
  - `command`
  - `command_result`
- Limits:
  - max frame size guard (for example 256 KB)
  - snippet truncation
  - bounded queue per subscriber

### Hub Event Loops
- **UDS accept loop**
  - validate handshake and session scope
  - classify publisher/subscriber role
- **Reducer loop**
  - apply incoming updates to store
  - increment sequence and produce deltas
  - broadcast to subscribers
- **Layout watcher loop (200-300ms)**
  - parse pane IDs/tab mapping from `dump-layout`
  - detect pane opens/closes
  - prune closed pane identities immediately
- **Heartbeat monitor loop**
  - mark stale/offline when heartbeats expire
  - clean long-dead entries

### Wrapper Runtime Loops
- **PTY passthrough loops**
  - stdin copy to child PTY writer
  - child PTY reader copy to stdout
- **Tap capture**
  - append observed output bytes into ring buffer (fixed size)
  - store rolling hash and last-significant timestamp
- **Debounced reporter loop (250ms)**
  - parse normalized buffer view
  - apply rule confidence + hysteresis
  - emit `state_update` only on meaningful changes or heartbeat interval
- **Signal loop**
  - on Ctrl-C from wrapper runtime, write ETX (`0x03`) to child PTY
  - escalate to terminate/kill after timeout if required

### Parser and Debounce Strategy
- Normalize output:
  - strip ANSI/control sequences for parser input only
  - collapse whitespace and limit line window
- Heuristic signals:
  - busy/thinking keywords
  - explicit error/panic/traceback markers
  - prompt-ready/waiting-input patterns
- Stability controls:
  - state flip requires confidence threshold or repeated confirmation
  - minimum dwell before changing low-confidence states

## Performance and Reliability Targets
- Overview close/open reconciliation:
  - P50 <= 400ms
  - P95 <= 1.0s
- Wrapper reporter overhead:
  - avoid parser on every byte; reporter loop bounded and debounced
- Hub memory:
  - bounded state and queue growth
- CPU budget:
  - stable for 5-20 active agents under normal churn

## Security and Isolation
- UDS path is user-local and permission restricted.
- Session ID mismatch is rejected at handshake.
- Command handling allowed only for same-session clients.
- No remote network exposure for new Pulse transport.

## Risks and Mitigations
- **Risk:** parser false positives cause state flapping.
  - **Mitigation:** confidence + hysteresis, bounded snippet context.
- **Risk:** slow subscribers back up hub broadcast.
  - **Mitigation:** bounded channels and drop/disconnect policy.
- **Risk:** layout watcher parse drift with Zellij output changes.
  - **Mitigation:** robust parser, fallback to heartbeat pruning, watcher error metrics.
- **Risk:** command race during reconnect.
  - **Mitigation:** request IDs, explicit command result statuses, idempotent wrapper handlers.

## Implementation Phases
1. Protocol and schema foundation (`Task 63`).
2. Hub UDS state service (`Task 64`).
3. Fast pane lifecycle watcher (`Task 65`).
4. Wrapper transparent tap + reporter (`Task 66`).
5. Command routing and SIGINT-safe control path (`Task 67`).
6. Mission Control integration (`Task 68`).
7. Observability, benchmarks, rollout safeguards (`Task 69`).

## Test Strategy
- **Unit tests**
  - IPC framing, schema round-trips, parser heuristics, debounce logic.
- **Integration tests**
  - hub + wrapper + pulse in same session with reconnect and tab churn.
- **Behavioral tests**
  - verify interactive agent usage (input, mouse, resize) remains unaffected.
- **Lifecycle tests**
  - pane close removes state within SLA.
- **Command tests**
  - Ctrl-C propagation and timeout escalation.
- **Perf tests**
  - synthetic output + 5-20 agent sessions to validate CPU/latency budgets.

## Review Sign-Off
- Review PRD with implementation team before coding starts.
- Confirm each implementation task references this PRD and has explicit acceptance criteria.
- Confirm latency targets and fallback behavior are agreed before enabling by default.
