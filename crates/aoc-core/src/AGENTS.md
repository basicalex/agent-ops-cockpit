# Repository Guidelines

Scope: `crates/aoc-core/src`

## Local Contracts
- Treat exported serde structs/enums as wire/storage contracts: preserve `rename_all`, tagged layouts, schema defaults, and backward-compatible `#[serde(default)]` unless a versioned migration and compatibility tests land together.
- Preserve Pulse IPC framing: newline-delimited JSON, `ProtocolVersion::CURRENT`, `DEFAULT_MAX_FRAME_BYTES`, oversize rejection, and decoder recovery after malformed frames.
- Do not persist or emit unredacted Mind event secrets; new `RawEventBody`/attrs text must use the sanitizer and keep `mind_sanitized` / `mind_sanitized_reasons` plus deterministic canonical JSON/hash behavior.
- Changes to consultation caps, T0/T1/T2 constraints, context-layer precedence, or overseer command policy require behavior tests for truncation/defaulting/error/allow-confirm-deny branches.

## Verification
- `cargo test -p aoc-core consultation_contracts::tests`
- `cargo test -p aoc-core mind_contracts::tests`
- `cargo test -p aoc-core mind_contracts::tests::sanitizer_redacts_message_and_nested_payload_secrets`
- `cargo test -p aoc-core pulse_ipc::tests`
- `cargo test -p aoc-core session_overseer::tests`
