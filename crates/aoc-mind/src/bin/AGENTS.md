# Repository Guidelines

Scope: `crates/aoc-mind/src/bin`

## Local Contracts
- Keep `aoc-mind-service` project-root scoped and path/store construction delegated to library APIs (`MindProjectPaths`, `open_project_store`, `MindRuntimeCore`); do not duplicate Mind path derivation in the binary.
- Preserve CLI machine contracts: JSON mode emits structured JSON without human prose on stdout, human errors go to stderr, success exits `0`, operational failures exit `1`, and existing parse/mode validation failures continue to exit `2`.
- Long-running `serve`/`watch-pi` paths must compose existing single-tick/sync helpers, keep service heartbeat/queue health updates, and retain minimum sleep clamps to avoid busy loops.
- Finalization writes remain safety ordered: validate every prepared export file with `ensure_safe_export_text`, write only `prepared.host_plan.export_files`, and advance the project watermark only after all writes succeed.
- Doctor memory checks stay project-scoped: external `aoc-mem` calls run with `current_dir(project_root)` and validate path/header consistency before reporting healthy memory.

## Verification
- `cargo run --manifest-path crates/aoc-mind/Cargo.toml --bin aoc-mind-service -- status --project-root /tmp/aoc-mind-dox-smoke --json`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --bins --no-run`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib prepare_session_finalize_execution_builds_host_plan_and_enqueues_t3`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib session_export_bundle_renders_markdown_and_manifest`
