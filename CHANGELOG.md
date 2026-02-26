# Changelog

## [Unreleased]

### Added
- **PI-first init smoke test**: Added `scripts/pi/test-aoc-init-pi-first.sh` to validate fresh + existing repo migration behavior in one command.
- **PI-only agent surface smoke test**: Added `scripts/pi/test-pi-only-agent-surface.sh` to verify non-PI and PI-R harness wrappers are removed while `pi` remains operational.
- **PI-only rollout checklist**: Added `docs/pi-only-rollout-checklist.md` for release closeout, operator notice timing, and post-release verification.
- **Taskmaster TUI**: **Subtasks**: Press `Space` to expand/collapse nested subtasks. Subtasks can be toggled independently.
- **Taskmaster TUI**: **Tag Cycling**: Press `t` to switch between project tags (task lists) in `tasks.json`.
- **Taskmaster TUI**: **Filtering**: Press `f` to cycle status filters (All/Pending/Done).
- **Taskmaster TUI**: **Mouse Support**: Left-click to select, click again to toggle details. Scroll wheel moves selection.
- **Taskmaster TUI**: **Help Panel**: Press `?` for a quick cheat-sheet overlay.
- **Taskmaster TUI**: **Dynamic Pane Title**: The pane now renames itself to show live stats, progress, and active filter/tag.
- **Developer Experience**: Switched default editor from Vim/Neovim to **`micro`** across the entire cockpit.
- **Developer Experience**: Implemented a robust editor enforcement system using a wrapper script (`bin/tm-editor`) and environment variable injection in Zellij layouts.
- **Zellij Layouts**: Explicit project root propagation to the Taskmaster TUI.
- **Process Cleanup**: `aoc-launch` now runs `aoc-cleanup` asynchronously (disable with `AOC_CLEANUP=0`).

### Changed
- **PI-only agent lockdown**: `aoc-agent`, `aoc-agent-run`, `aoc-agent-install`, and `aoc-control` now expose only `pi`.
- **Legacy harness removal**: Non-PI launcher wrappers (`aoc-codex`, `aoc-gemini`, `aoc-cc`, `aoc-oc`, `aoc-kimi`, `aoc-omo`, and related aliases) are removed from the shipped bin surface.
- **Installer closeout**: `install.sh` now prunes retired PI-only deprecation stubs from previous installs.
- **PI-first ownership rollout**: `aoc-init` now treats `.pi/**` as canonical runtime ownership, keeps `.aoc/**` as control plane, and no longer auto-syncs non-PI skill targets in active PI-first flow.
- **PI migration behavior**: Existing repos are repaired non-destructively by migrating missing legacy `.aoc/prompts/pi` + `.aoc/skills` assets to `.pi/**`; safe `tmcc` prompt alias duplicates are cleaned automatically.
- **Taskmaster**: Removed the Zellij WASM plugin; the native `aoc-taskmaster` TUI is now the default.

### Fixed
- **Taskmaster TUI**: Improved selection visibility with high-contrast highlighting (Black on Cyan).
- **Session Management**: Hardened `aoc-session-watch` to prevent accidental session deletion. Added a 2-second timeout to client checks and implemented a requirement for 3 consecutive idle counts before a session is destroyed.
- **Process Cleanup**: Improved `aoc-cleanup` accuracy by refining the agent process pattern and adding a protected list for essential cockpit scripts, preventing them from being killed as orphans.
- **Process Cleanup**: `aoc-cleanup` now matches orphaned agent CLIs more accurately per active tab and supports `AOC_AGENT_PATTERN` for new agents.
- Ensure `codex` always runs through the tmux wrapper, including outside Zellij.
- Skip wrapper recursion when resolving the real Codex binary.

## 0.1.0
- Initial Zellij cockpit layout and install flow.
- Helper scripts for widget, sys details, taskmaster, and root anchoring.
- Yazi config with preview support.
