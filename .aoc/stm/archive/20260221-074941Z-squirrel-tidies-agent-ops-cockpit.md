- Objective + active task/subtask IDs (and tag)
  - Stabilize agent-pane behavior and continue safe rollout toward AOC Mind PRD.
  - Active IDs: [49] (mission-control, in-progress), [85] (safety, pending/high), [83] (safety, marked done but needs rework after rollback).

- Done
  - Safety wave delivered: [77]-[84] marked done (CI gates, installer smoke SHA fix, shellcheck expansion, smoke tests, file locking).
  - Extracted cleanup Python logic to `lib/aoc_cleanup/core.py`; `bin/aoc-cleanup` now calls module path with fallback.
  - Hotfix applied for pane corruption: removed `setsid` launch path in `bin/aoc-agent-wrap`; restored foreground TTY execution.

- In progress
  - Reassessing task [83] implementation approach to avoid raw CSI/mouse sequence leakage.
  - PRD alignment planning for phased Taskmaster execution (MC -> Mind -> Insight) with risk gates.

- Blockers / risks
  - Task status drift: [83] is marked done but the risky process-group launch path was rolled back.
  - Dirty working tree with many unrelated edits/untracked dirs (`.venv`, `shellcheck-v0.10.0`, lock files) raises commit/review risk.
  - No full interactive Zellij regression run yet for the pane-corruption scenario after hotfix.

- Files touched
  - `bin/aoc-agent-wrap`
  - `bin/aoc-cleanup`
  - `lib/aoc_cleanup/core.py`
  - `.github/workflows/ci.yml`
  - `.github/workflows/installer-smoke.yml`
  - `scripts/lint.sh`
  - `scripts/smoke.sh`
  - `crates/aoc-cli/src/task.rs`
  - `crates/aoc-taskmaster/src/state.rs`
  - `bin/aoc-mem`
  - `install/bootstrap.sh`

- Last command outcomes (tests/lint/build)
  - `bash -n bin/aoc-agent-wrap` -> pass.
  - `AOC_SMOKE_TEST=1 bash scripts/smoke.sh` -> pass.
  - `bash ./install.sh` -> pass (Rust binaries built + installed successfully).
  - Earlier `cargo check --workspace && cargo test --workspace` -> pass with non-blocking warnings.

- Open decisions / assumptions
  - Decide whether task [83] should be reopened or updated to reflect rollback + revised acceptance criteria.
  - Assumption: interactive agent panes must remain foreground-TTY owned; wrapper-level `setsid` is unsafe here.
  - Durable decision candidate: avoid session-detaching process-group launch in shell wrapper for interactive agents; keep lifecycle escalation in PTY-aware Rust path. Promote to aoc-mem.

- Next 3-5 concrete steps
  - Reopen/update [83] with explicit no-CSI-leak acceptance test and safer implementation scope.
  - Implement [85] telemetry secret redaction in `aoc-agent-wrap-rs` with fixture tests for secret patterns.
  - Add an interactive regression checklist for Zellij mouse/input corruption and run it after wrapper changes.
  - Complete mission-control [49] remaining manual UX confirmation and land [51] observability/isolation plan.
  - Create phased PRD tasks/tags for AOC Mind foundations (storage/adapter/layout/bridge) with hard dependencies.
