# Spec: AOC Control Background Jobs and Log Pane Polish

## Role
AOC operators using `Alt+C` need long-running tool actions to stay observable and cancellable without freezing the control pane.

## Purpose
Polish the control surface so tool actions run as background jobs and their logs are visible in the right-hand details area. Increase the default Alt+C floating pane size so the split details/logs surface is more usable.

## Game
Extend the existing `aoc-control` background-job pattern to AOC Understand, render a dedicated logs split when a job is active, and enlarge the floating Control pane defaults.

## Scope
- AOC Understand actions in `Alt+C -> Settings -> Tools -> AOC Understand` run in the background.
- AOC Understand jobs write logs under the existing AOC control log directory with action-specific names.
- Active tool jobs surface log tails in a split right-hand panel instead of burying logs in prose.
- Existing Agent Browser, Search, and HyperFrames jobs participate in the same active-log split.
- Footer controls mention log scrolling/cancel/open-log when a job is active.
- Default floating Control pane size is increased.

## Acceptance Criteria
- Starting an AOC Understand install/status/doctor/analyze/dashboard/map-sync action does not block the TUI event loop.
- Active job logs appear in the right details region as a separate logs panel.
- PgUp/PgDn scrolls logs, `x` cancels, and `Shift+O` opens the active log for supported active jobs.
- Alt+C default floating size is larger than the previous 70% x 70%.
- Targeted Rust tests and checks pass.

## Test Strategy
- `cargo test --manifest-path crates/Cargo.toml -p aoc-control`
- `cargo check --manifest-path crates/Cargo.toml -p aoc-control`
- `bash -n bin/aoc-control bin/aoc-control-toggle`
- targeted grep for new size defaults and Understand background-job/log strings.
