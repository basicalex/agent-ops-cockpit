# Mission Control Operations Guide (AOC)

Mission Control was a legacy cockpit-era operator surface. It has been removed from the active Herdr-first runtime and should not be used as an operator runbook for current AOC installs.

## Current operator model

- Use Herdr/OMP for worker orchestration, pane ownership, liveness, focus, and assignment review.
- Use current `aoc` entrypoints only for the supported Herdr-first launcher path.
- Treat the old Mission Control hub, floating-pane toggles, dedicated layouts, and pane-evidence helpers as removed implementation history, not active procedures.

## What not to resurrect from this page

Do not add new operator instructions for removed Mission Control scripts, hub crates, layout templates, or cockpit-specific pane toggles. If a workflow needs operator visibility, document it against Herdr/OMP behavior instead of the removed Mission Control stack.

## Related status docs

- `docs/aoc-feature-inventory.md` lists removed cockpit-era surfaces and their Herdr/OMP replacement direction.
- `docs/deprecations.md` records the supported deprecation boundary.
