# Repository Guidelines

Scope: `crates/aoc-mission-control/src`

## Local Contracts
- Preserve Pulse hub ordering/resync semantics: ignore other-session or newer-protocol envelopes, drop stale deltas, reconnect on sequence gaps, and clear cached hub state before resync.
- Keep Mission Control runtime knobs in config.rs; new AOC_* env vars must use existing bool parsing/default conventions and clamp user-controlled refresh/poll intervals.
- Baseline rendering must not require a live Pulse hub or Zellij polling; local snapshot/presence fallback must still work when Pulse is disabled/offline.
- Keep Zellij in-session launch/navigation distinct from standalone aoc-launch/aoc-new-tab fallbacks; worker launch plans use program/args/env/cwd with Command::new, not shell-expanded strings.
- Mind consultation persistence must keep provenance, task/file links, and stable prompt/source identifiers, not display-only summaries.

## Verification
- `cargo test --manifest-path crates/aoc-mission-control/Cargo.toml`
