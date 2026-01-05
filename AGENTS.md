# Repository Guidelines

## Project Structure & Module Organization
- `bin/`: executable shell scripts (`aoc-*`) that power the cockpit utilities.
- `zellij/layouts/aoc.kdl`: Zellij layout definition for the workspace.
- `yazi/`: Yazi configuration and preview script.
- `install.sh`: installer that copies scripts and configs into user locations.
- `README.md`: usage, requirements, and launch instructions.

## Build, Test, and Development Commands
- `./install.sh`: installs scripts into `~/.local/bin` and layouts/configs into
  `~/.config/`.
- `ZELLIJ_PROJECT_ROOT="$PWD" zellij --layout aoc`: launches the cockpit from a
  project root.
- `aoc-star /path/to/project`: re-anchors panes to a new root (prompts first).

No formal build step exists; changes are applied by re-running `./install.sh`.

## Coding Style & Naming Conventions
- Shell scripts use `bash` with `set -euo pipefail` and 2-space indentation.
- Script names are prefixed with `aoc-` (e.g., `bin/aoc-taskmaster`).
- Keep config files in their native formats (`.kdl`, `.toml`, `.sh`) and avoid
  reformatting unless necessary.

## Testing Guidelines
There are no automated tests yet. When modifying scripts, do a quick manual
check by running the affected command (for example, `aoc-widget`) from a
terminal. If you add tests, document how to run them in this file and
`README.md`.

## Commit & Pull Request Guidelines
- Git history currently contains only `Initial commit`, so no established
  commit convention exists. Use short, imperative messages (e.g., "Add widget
  toggle").
- PRs should include a concise summary, the commands used to validate changes,
  and any relevant screenshots when layouts or previews change.
