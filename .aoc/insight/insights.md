# Developer Insight Log

Append-only entries for high-signal teaching insights.

## Entry template
- timestamp: YYYY-MM-DDTHH:MM:SSZ
- subsystem: <name>
- insight: <concise observation>
- evidence: <file refs>
- confidence: high|medium|low
- suggested action: <next step>
- promote to memory: yes|no

- timestamp: 2026-02-15T17:57:57Z
  subsystem: ingestion/parsing/chunking
  insight: Current implementation provides deterministic repository chunking via RLM, but no staged ingest pipeline exists in this checkout.
  evidence: `crates/aoc-cli/src/rlm.rs`, `.aoc/memory.md`
  confidence: high
  suggested action: Define whether RLM chunking is the intended long-term ingestion layer or add explicit ingest stages.
  promote to memory: no

- timestamp: 2026-02-15T17:57:57Z
  subsystem: indexing/embeddings/retrieval
  insight: Semantic retrieval stack (embeddings/vector search) is absent; current "indexing" is operational state caching for agents/tasks/diffs.
  evidence: `crates/aoc-hub-rs/src/main.rs`, `crates/aoc-hub-rs/src/pulse_uds.rs`, `.taskmaster/docs/prd.txt`
  confidence: high
  suggested action: Capture explicit retrieval scope in PRD before creating implementation tasks.
  promote to memory: no

- timestamp: 2026-02-15T17:57:57Z
  subsystem: api/state/queue
  insight: Pulse UDS path has robust queue/backpressure/stale-prune guardrails and explicit observability events, indicating production-minded local IPC design.
  evidence: `crates/aoc-hub-rs/src/pulse_uds.rs`, `crates/aoc-core/src/pulse_ipc.rs`, `docs/pulse-ipc-protocol.md`
  confidence: high
  suggested action: Add integration tests for end-to-end drop/reconnect behavior across hub, wrapper, and mission-control.
  promote to memory: no

- timestamp: 2026-02-15T18:11:26Z
  subsystem: api/state/queue
  insight: Command routing is strongly session-scoped and role-gated, with request_id-based result dedupe via short-lived command cache.
  evidence: `crates/aoc-hub-rs/src/pulse_uds.rs:956`, `crates/aoc-hub-rs/src/pulse_uds.rs:1232`, `crates/aoc-agent-wrap-rs/src/main.rs:1149`, `crates/aoc-mission-control/src/main.rs:3432`
  confidence: high
  suggested action: Add end-to-end tests for command replay/idempotence and stale request expiry behavior.
  promote to memory: no

- timestamp: 2026-02-15T18:11:26Z
  subsystem: api/state/queue
  insight: The system deliberately favors responsiveness over guaranteed delivery by dropping on full queues across hub, wrapper pulse updates, and mission-control command queue.
  evidence: `crates/aoc-hub-rs/src/pulse_uds.rs:489`, `crates/aoc-hub-rs/src/pulse_uds.rs:532`, `crates/aoc-agent-wrap-rs/src/main.rs:729`, `crates/aoc-mission-control/src/main.rs:920`
  confidence: high
  suggested action: Introduce a small replay window or queue occupancy telemetry to reduce operator-visible blind spots under burst load.
  promote to memory: no
