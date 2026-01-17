# Changelog

## [Unreleased]

### Added
- **Developer Experience**: Switched default editor from Vim/Neovim to **`micro`** across the entire cockpit.
- **Developer Experience**: Implemented a robust editor enforcement system using a wrapper script (`bin/tm-editor`) and environment variable injection in Zellij layouts.
- **Taskmaster Plugin**: Interactive root path management. Press **Shift+C** to manually set or correct the project root within the plugin.
- **Taskmaster Plugin**: New input bar UI for search and root path entry.
- **Zellij Layouts**: Explicit project root propagation to Taskmaster plugin to bypass WASM environment isolation.

### Fixed
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
