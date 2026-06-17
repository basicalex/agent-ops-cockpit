# Repository Guidelines

Scope: `crates/aoc-hub-rs/src`

## Local Contracts
- Validate hub envelopes before registration or routing: protocol version, required fields, RFC3339 timestamp, session id, role, and publisher identity must pass first; publisher ids stay scoped as `{session_id}::{pane_or_agent}`. Routing/consultation changes must cover accepted route+ack and invalid-target error behavior.
- Keep transport bounds and observability constraints: WebSocket envelopes, patches, file lists, and UDS frames stay capped by the existing constants, and raw message bodies remain debug-only.
- Preserve private UDS lifecycle: create socket parents with user-private permissions, remove stale sockets before bind, bind sockets private to the user, and remove the socket path on shutdown.

## Verification
- `cargo test -p aoc-hub-rs --all-targets command_errors_include_code_and_message`
- `cargo test -p aoc-hub-rs --all-targets snapshot_on_connect_and_ordered_deltas`
- `cargo test -p aoc-hub-rs --all-targets stop_agent_command_routes_and_acks`
