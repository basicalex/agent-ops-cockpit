# Spec: AOC Understand Control Pane Integration

## Role
AOC operators and developers using `Alt+C` need a discoverable way to install, verify, and launch AOC Understand without memorizing CLI commands.

## Purpose
Expose `aoc-understand` in the existing `Alt+C -> Settings -> Tools` flow as the canonical repository-understanding bridge, complementing the CLI/skill integration already completed in Task 231.

## Game
Add an `AOC Understand` nested tools section to `aoc-control` with safe actions that delegate to the existing `aoc-understand` wrapper. The UI should make install/status/doctor/dashboard/map-sync discoverable and preserve the explicit install/security model.

## Scope
- Add a Tools menu entry for AOC Understand.
- Add a nested section with actions:
  - Install/update Understand-Anything via `aoc-understand install`.
  - Check status via `aoc-understand status`.
  - Run doctor via `aoc-understand doctor`.
  - Show analyze guidance via `aoc-understand analyze --full`.
  - Open dashboard via `aoc-understand dashboard --open`.
  - Sync graph overview into AOC Map via `aoc-understand map-sync`.
- Add detail-pane guidance for using the feature in another AOC project.
- Update human docs for the Alt+C path.
- Add targeted test coverage for the menu/detail/action strings.

## Out of Scope
- Replacing AOC Map.
- Adding a full terminal wizard for Understand-Anything slash-command conversations.
- Implicit network installs from status/doctor.
- Advanced AOC Map + Open Design convergence.

## Acceptance Criteria
- `Alt+C -> Settings -> Tools` lists AOC Understand.
- Selecting AOC Understand opens a nested section with safe actions.
- Actions invoke the existing `aoc-understand` wrapper, not duplicated install logic.
- Status/doctor/analyze actions remain non-installing.
- Docs describe the UI path and CLI fallback.
- Targeted validation passes.

## Test Strategy
- Run Rust tests for `aoc-control`.
- Run targeted text checks for new menu/action strings.
- Run `cargo check -p aoc-control` if tests are insufficient or compile risk is nontrivial.
