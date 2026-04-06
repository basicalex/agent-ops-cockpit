---
name: custom-layout-ops
description: Create and maintain custom AOC Zellij layouts with the managed top bar, explicit tab metadata, and the Alt+C creator/editor flow.
---

## When to use
- You want a new project-specific AOC layout under `.aoc/layouts/`.
- You want a personal reusable layout under `~/.config/zellij/layouts/`.
- You need to edit an existing custom layout without touching the managed `aoc` layout.
- You want to preserve the managed AOC top bar and grouped-tab behavior while changing pane geometry or commands.

## Canonical flow
Prefer the built-in creator/editor flow first:

- `Alt+C -> Settings -> Layout`
- `aoc-layout --create <name> --scope project`
- `aoc-layout --create <name> --scope global`
- `aoc-layout --edit <name>`
- `aoc-layout --set <name>`
- `aoc-new-tab --layout <name>`

## Scope model
- **Project scope**: `.aoc/layouts/<name>.kdl`
  - Use for layouts that should be shared in git with the repo/team.
- **Global scope**: `~/.config/zellij/layouts/<name>.kdl`
  - Use for personal layouts that should not affect the repo.

Resolution order for launches:
1. `.aoc/layouts/<name>.kdl`
2. `~/.config/zellij/layouts/<name>.kdl`

## Guardrails
- Do **not** edit the managed `aoc` layout directly.
- Do **not** reuse deprecated/internal managed names:
  - `unstat`
  - `minimal`
  - `aoc-zjstatus-single`
  - `aoc-zjstatus-test`
  - `aoc.hybrid`
  - `mission-control`
- Keep custom layout names simple and shell-safe: letters, numbers, `.`, `_`, `-`.
- Prefer project scope when the layout is part of the repo workflow.
- Prefer global scope for personal experiments and machine-local ergonomics.

## Required layout conventions
Custom layouts should preserve these AOC behaviors where relevant:

1. **Managed top bar plugin**
   - plugin path: `file:{{HOME}}/.config/zellij/plugins/zjstatus-aoc.wasm`
2. **Explicit tab metadata sync**
   - call `aoc-tab-metadata sync >/dev/null 2>&1 || true` during pane startup
3. **Injected AOC env**
   - `AOC_PROJECT_ROOT`
   - `AOC_SESSION_ID`
   - `AOC_HUB_ADDR`
   - `AOC_HUB_URL`
   - `AOC_TAB_SCOPE`
4. **Launch placeholders**
   - `__AOC_TAB_NAME__`
   - `__AOC_PROJECT_ROOT__`
   - `__AOC_AGENT_ID__`
   - `__AOC_SESSION_ID__`
   - `__AOC_HUB_ADDR__`
   - `__AOC_HUB_URL__`

## Starter commands
Create and edit a project layout:

```bash
aoc-layout --create review --scope project
aoc-layout --set review
aoc-new-tab --layout review
```

Create and edit a personal global layout:

```bash
aoc-layout --create scratch-review --scope global
aoc-layout --edit scratch-review
aoc-new-tab --layout scratch-review
```

## Recommended authoring approach
1. Start from the generated template.
2. Change pane splits/sizes first.
3. Keep the managed top bar pane intact.
4. Keep `aoc-tab-metadata sync` in panes that represent the tab's project.
5. Launch a fresh tab with `aoc-new-tab --layout <name>`.
6. If the layout becomes the preferred default, run `aoc-layout --set <name>`.

## Validation
After editing:

```bash
bash -n bin/aoc-layout
bin/aoc-layout --list
bin/aoc-doctor
```

Then launch a fresh tab/session and confirm:
- top bar renders
- grouped tabs still color/group correctly
- mouse tab click still works
- repo/project grouping stays correct

## Related docs
- `docs/layouts.md`
- `docs/control-pane.md`
- `docs/zellij-top-bar.md`
