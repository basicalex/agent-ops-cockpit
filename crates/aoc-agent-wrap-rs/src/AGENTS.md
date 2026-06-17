# Repository Guidelines

Scope: `crates/aoc-agent-wrap-rs/src`

## Local Contracts
- Route Pulse insight commands through `build_pulse_command_response`/`InsightCommand`: validate target agent, return JSON `command_result`s, and emit runtime/detached `PulseUpdate`s for state changes.
- Spawn Mind/Insight child processes with the existing `env_clear` allowlist path (`configure_mind_child_std_command_env` or equivalent); test any added allowed env key.
- Sanitize/redact external output before it reaches agent-visible status, telemetry, command results, exports, or persisted text; extend secret-pattern tests for new formats.
- Preserve stop/cancel semantics: SIGINT/INT grace, TERM grace, final kill, persisted detached cancellation, and parent/child job cancellation together.
- Insight dispatch must fail deterministically when manifests or subprocess config are absent: return bounded fallback/error result objects, not panics or live `.pi` assumptions.

## Verification
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml mind_child_env_excludes_ambient_secrets_and_keeps_allowlisted_vars`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml pulse_insight_dispatch_returns_structured_result`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml sanitize_activity_line_redacts_common_secret_patterns`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml stop_tokio_child_escalates_when_sigint_ignored`
