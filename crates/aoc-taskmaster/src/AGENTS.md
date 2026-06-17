# Repository Guidelines

Scope: `crates/aoc-taskmaster/src`

## Local Contracts
- Taskmaster mutations must update visible `App` state and the `ProjectData` mirror before persisting only through `save_project -> write_atomic -> touch_state_file`; keep `tasks.json` as pretty `ProjectData` JSON and `state.json` updates on that path.
- Root resolution stays non-creating: `AOC_TASKMASTER_ROOT`/`TM_ROOT`/`TASKMASTER_ROOT` must canonicalize to existing directories; otherwise use the nearest existing Taskmaster root, then `AOC_PROJECT_ROOT` only if it is already a Taskmaster root, else cwd.
- TUI runtime safety is part of the contract: restore raw mode, alternate screen, mouse capture, and cursor visibility after `run_app`; keep refresh watching non-recursive and bounded to `.taskmaster`, `.taskmaster/tasks`, or the root fallback with bounded signaling.

## Verification
- `cargo check --manifest-path crates/Cargo.toml -p aoc-taskmaster`

## Do Not
- Do not create `.taskmaster` from root detection, persist subtask `aocPrd`, remove legacy `parse_project_compat` formats, add recursive/project-wide watchers, or skip terminal restore after setup.
- Do not write `.taskmaster/tasks/tasks.json` or `.taskmaster/state.json` directly from handlers, or update `App.tasks` without matching `project.tags` changes.

## Update When
- `resolve_root*`, `find_taskmaster_root`, `is_taskmaster_root`, mutation handlers, `save_project`, `touch_state_file`, `write_atomic`, `parse_project_compat`, `validate_project`, `main`, terminal setup/restore, `run_app`, or `setup_watcher` change.
