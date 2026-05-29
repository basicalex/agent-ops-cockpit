# Alt+C Control Pane Guide

`Alt+C` opens `aoc-control`, the operator surface for Agent Ops Cockpit (AOC).

Use it to run the most common tools first, inspect runtime state, and launch setup/verification flows without manually editing config files.

## What it manages

Primary areas exposed through the control pane include:

- **Tools** — first/default surface for AOC Understand, Agent Browser + Search, AOC Map, CodeGraph, HyperFrames, Vercel, PI compaction, PI agent installer, and RTK routing
- **Projects** — open, create, search, and retarget project roots
- **Launch** — start sessions with selected defaults/overrides
- **Advanced** — background profile plus legacy/deprecated AOC-specific theme and custom layout utilities

AOC theme management is deprecated as a primary utility because Omarchy owns the system theme. Custom layout creation/editing is also legacy; prefer managed AOC/Zellij defaults unless you intentionally need an old custom layout path.

## Navigation model

Typical flow:

1. Press `Alt+C`
2. Open **Tools** (first/default nav item)
3. Choose a tool/integration section
4. Read the right-hand detail pane before running the selected action

The detail pane explains:
- what the action does
- required dependencies
- current status
- where logs/config files live

Use **Advanced** only for low-frequency runtime/legacy utilities such as background profile, deprecated AOC theme utilities, or legacy custom layout actions.

## Background jobs and logs

Long-running setup and verification flows run asynchronously in the control pane when supported. The default floating `Alt+C` pane is intentionally large so details and logs can be visible together.

When a background job is active, the right-hand area splits into details plus a dedicated log panel showing:
- running state
- log path
- recent output

Useful controls:

- `PgUp` / `PgDn` — scroll recent log output
- `x` — cancel the running job
- `Shift+O` — open the full log in a pager

## Advanced legacy layout utilities

Path:

- `Alt+C -> Advanced -> Legacy layout utilities`

Available legacy actions include:

- set the default layout
- create a project custom layout
- create a global custom layout
- edit an existing custom layout in `$EDITOR`

These actions are retained for compatibility. They are no longer a primary workflow now that managed AOC/Zellij defaults cover normal use.

## Agent Browser + Search

This is the main web research integration surface. See [Web research](web-research.md).

Path:

- `Alt+C -> Tools -> Agent Browser + Search`

Available actions include:

- install/update Agent Browser
- install/update PI browser skill
- install/update PI web research skill
- enable managed local search
- start/verify local search
- verify web research stack

### Recommended setup order

1. Install/update Agent Browser tool
2. Install/update PI browser skill
3. Install/update PI web research skill
4. Enable managed local search (writes `.aoc/search.toml` and `.aoc/services/searxng/*`)
5. Start/verify local search
6. Verify web research stack

### Verification contract

The strongest validation is:

- `aoc-search` healthy
- `bin/aoc-web-smoke` passes

That confirms:
- local search can return results
- `agent-browser` can open and inspect the top result

## AOC Understand

Path:

- `Alt+C -> Tools -> AOC Understand`

Use it to run background, logged actions for:
- `aoc-understand status`
- `aoc-understand doctor`
- explicit install/update with `aoc-understand install`
- analyze guidance for `/skill:understand --full`
- opening the Understand-Anything dashboard after a graph exists
- gap audit guidance for `/skill:aoc-gaps`
- syncing `.understand-anything/knowledge-graph.json` into a compact AOC Map overview

Status, doctor, and analyze guidance are non-installing. Use the install/update action only when you want the explicit network clone/update step.

## AOC Map microsite

Path:

- `Alt+C -> Tools -> AOC Map microsite`

Use it to:
- sync `.pi/skills/aoc-map/SKILL.md` for the current repo
- run `aoc-map init`
- seed or confirm `.aoc/map/`
- migrate older AOC See workspaces when needed
- quickly see whether the AOC Map workspace and skill are present

## CodeGraph agent index

Path:

- `Alt+C -> Tools -> CodeGraph agent index`

Use it to:
- install/update CodeGraph globally with `pnpm add -g @colbymchenry/codegraph`
- verify `codegraph status` in the current project
- confirm whether the global CLI and project `.codegraph/` index are present

AOC installs the CLI only. Project indexing remains explicit; after install, run `codegraph init -i` in the repo you want agents to query.

## PI installer

Path:

- `Alt+C -> Tools -> PI agent installer`

Use it to:
- check PI install state
- run install/update actions

## PI compaction presets

Path:

- `Alt+C -> Tools -> PI compaction`

Use it to:
- choose a context window preset
- apply compaction defaults without manual JSON edits
- inspect when a repo-level `.pi/settings.json` override takes precedence

## When to use Alt+C vs CLI

Prefer **Alt+C** when you want:
- discoverable setup flows
- status in one place
- guided integration actions
- background jobs with logs

Prefer **CLI** when you want:
- scripting
- automation
- direct smoke tests
- fast repeated operator commands

Examples:

```bash
aoc-doctor
aoc-search status
aoc-search start --wait
bin/aoc-web-smoke
```

## Related docs

- [Installation Guide](./installation.md)
- [Configuration Guide](./configuration.md)
- [Layouts](./layouts.md)
- [Managed Zellij Top Bar](reference/zellij-top-bar.md)
- [Agents](./agents.md)
