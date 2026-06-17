# Repository Guidelines

Scope: `crates/aoc-cli`

## Local Contracts
- Add/change user-facing `aoc` commands only through `main.rs::Commands` and the existing module `handle_*_command` dispatch; preserve public names and aliases such as `map`/`see` unless fully migrated.
- State-mutating commands must use existing project-root/path/write helpers for Taskmaster, DOX, and map outputs; do not hand-roll writes to `.taskmaster/*`, `.aoc/dox/*`, or `.aoc/map/*`.
- Keep DOX review/apply conservative: approvals need evidence plus safe verification, verification commands pass `validate_verification_command`, and AGENTS writes stay dry-run/`--yes` guarded with unmanaged-content protection.

## Verification
- `cargo test -p aoc-cli`
- `cargo test -p aoc-cli dox`
- `cargo test -p aoc-cli map`
