# Repository Guidelines

Scope: `crates/aoc-mind/src`

## Local Contracts
- Treat project Mind state layout and compatibility seams as stable API: derive runtime/store/legacy/lock/health paths through `MindProjectPaths` and resolver helpers, sanitize project/session/pane path components, and keep legacy imports/readers plus `AOC_MIND_FEED_COMPAT`, `AOC_PI_SESSION_DIR`, and `AOC_PI_SETTINGS_PATH` intentional.
- Preserve runtime coordination as dual ownership: service/reflector/T3 work requires the advisory file lock plus the store lease before claiming jobs, lock conflicts are not claims, and service ticks keep heartbeat/health snapshots current.
- Preserve deterministic provenance through ingestion, observer fallback, retrieval, T3, and finalization: semantic/guardrail failures fall back deterministically, export manifests keep schema/slice/artifact/tag/watermark/T3 fields, and watermarks/T3 backlog jobs advance only with slice provenance.

## Verification
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib explicit_overrides_and_legacy_paths_are_supported`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib guardrail_budget_exceeded_falls_back_to_deterministic_t1`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib latest_pi_session_file_prefers_newest_jsonl`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib prepare_session_finalize_execution_builds_host_plan_and_enqueues_t3`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib project_paths_match_expected_layout`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib runtime_owns_scope_and_lease_queries`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib runtime_owns_tick_health_and_observer_effects`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib semantic_failure_falls_back_to_deterministic_t1`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib session_export_bundle_renders_markdown_and_manifest`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib sync_session_file_into_project_store_ingests_pi_jsonl`
