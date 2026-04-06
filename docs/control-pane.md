# Alt+C Control Pane Guide

`Alt+C` opens `aoc-control`, the operator surface for Agent Ops Cockpit (AOC).

Use it to inspect runtime state, configure integrations, and run setup/verification flows without manually editing config files.

## What it manages

Main areas currently exposed through the control pane include:

- Theme management
- Layout defaults and custom layout creation/editing
- RTK routing
- PI agent installer
- PI compaction presets
- Agent Browser + Search
- AOC Map microsite
- Vercel CLI
- MoreMotion

## Navigation model

Typical flow:

1. Press `Alt+C`
2. Open **Settings**
3. Open **Tools**
4. Choose a tool/integration section
5. Read the right-hand detail pane before running the selected action

The detail pane explains:
- what the action does
- required dependencies
- current status
- where logs/config files live

## Background jobs and logs

Long-running setup and verification flows may run asynchronously in the control pane.

When a background job is active, the detail pane shows:
- running state
- log path
- recent output

Useful controls:

- `PgUp` / `PgDn` — scroll recent log output
- `x` — cancel the running job
- `Shift+O` — open the full log in a pager

## Layout creator/editor

Path:

- `Alt+C -> Settings -> Layout`

Available actions include:

- set the default layout
- create a project custom layout
- create a global custom layout
- edit an existing custom layout in `$EDITOR`

The generated starter template preserves:

- the managed `zjstatus-aoc` top bar
- grouped tab metadata sync via `aoc-tab-metadata sync`
- the standard AOC placeholder/env contract

Prefer project scope for shared repo workflows and global scope for personal machine-local layouts.

## Agent Browser + Search

This is the main web research integration surface.

Path:

- `Alt+C -> Settings -> Tools -> Agent Browser + Search`

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

## AOC Map microsite

Path:

- `Alt+C -> Settings -> Tools -> AOC Map microsite`

Use it to:
- sync `.pi/skills/aoc-map/SKILL.md` for the current repo
- run `aoc-map init`
- seed or confirm `.aoc/map/`
- migrate older AOC See workspaces when needed
- quickly see whether the AOC Map workspace and skill are present

## PI installer

Path:

- `Alt+C -> Settings -> Tools -> PI agent installer`

Use it to:
- check PI install state
- run install/update actions

## PI compaction presets

Path:

- `Alt+C -> Settings -> Tools -> PI compaction`

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
- [Managed Zellij Top Bar](./zellij-top-bar.md)
- [Custom Layout Skill](../.pi/skills/custom-layout-ops/SKILL.md)
- [Agents](./agents.md)
