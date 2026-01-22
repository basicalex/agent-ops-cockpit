# Changelog

## [Unreleased]

### Added
- **Taskmaster Plugin**: **Subtasks**: Press `Space` to expand/collapse nested subtasks. Subtasks can be toggled independently.
- **Taskmaster Plugin**: **Tag Cycling**: Press `t` to switch between project tags (task lists) in `tasks.json`.
- **Taskmaster Plugin**: **Filtering**: Press `f` to cycle status filters (All/Pending/Done).
- **Taskmaster Plugin**: **Mouse Support**: Left-click to select, click again to toggle details. Scroll wheel moves selection.
- **Taskmaster Plugin**: **Help Panel**: Press `?` for a quick cheat-sheet overlay.
- **Taskmaster Plugin**: **Dynamic Pane Title**: The pane now renames itself to show live stats, progress, and active filter/tag.
- **Developer Experience**: Switched default editor from Vim/Neovim to **`micro`** across the entire cockpit.
- **Developer Experience**: Implemented a robust editor enforcement system using a wrapper script (`bin/tm-editor`) and environment variable injection in Zellij layouts.
- **Taskmaster Plugin**: Interactive root path management. Press **Shift+C** to manually set or correct the project root within the plugin.
- **Taskmaster Plugin**: New input bar UI for search and root path entry.
- **Zellij Layouts**: Explicit project root propagation to Taskmaster plugin to bypass WASM environment isolation.

### Fixed
- **Taskmaster Plugin**: Fixed infinite scrollback growth by disabling terminal line wrap (`?7l`) during rendering.
- **Taskmaster Plugin**: Fixed high CPU usage by throttling rendering to actual state changes.
- **Taskmaster Plugin**: Improved selection visibility with high-contrast highlighting (Black on Cyan).
- **Taskmaster Plugin**: Robust file persistence via shell fallback when direct WASM writes fail.
- **Session Management**: Hardened `aoc-session-watch` to prevent accidental session deletion. Added a 2-second timeout to client checks and implemented a requirement for 3 consecutive idle counts before a session is destroyed.
- **Process Cleanup**: Improved `aoc-cleanup` accuracy by refining the agent process pattern and adding a protected list for essential cockpit scripts, preventing them from being killed as orphans.
- **Taskmaster Plugin**: Resolved issue where the plugin would fail to find tasks due to incorrect root detection in WASM.
- **Taskmaster Plugin**: Fixed a bug in the root detection shell script that introduced leading spaces in paths.
- **Taskmaster Plugin**: Improved path validation to prevent defaulting to the system root.
- **Taskmaster Plugin**: Refactored monolithic code into modular structure (`model`, `state`, `ui`, `theme`).
- Ensure `codex` always runs through the tmux wrapper, including outside Zellij.
- Skip wrapper recursion when resolving the real Codex binary.

## 0.1.0
- Initial Zellij cockpit layout and install flow.
- Helper scripts for widget, sys details, taskmaster, and root anchoring.
- Yazi config with preview support.
