# Agents

## OpenCode AOC Ops Subagent
`aoc-ops` is a project subagent that handles AOC setup and maintenance tasks.

### Location
`aoc-init` seeds the agent into:

```
.opencode/agents/aoc-ops.md
```

### Usage
In OpenCode, invoke it with:

```
@aoc-ops
```

### Behavior
`aoc-ops` focuses on:
- Running `aoc-init` and verifying `.aoc/` and `.taskmaster/`
- Managing skills with `aoc-skill validate` and `aoc-skill sync`
- Managing custom layouts with project `.aoc/layouts/` + `aoc-layout`
- Managing global Zellij themes with `aoc-theme` (`aoc-theme tui`, presets, custom themes)
- Preserving existing repo skills and avoiding collisions

`aoc-init` will not overwrite existing `.opencode/agents/aoc-ops.md`.

## OpenCode Teach Subagent
`teach` is a project subagent focused on guided repository learning.

### Location
`aoc-init` seeds the agent into:

```
.opencode/agents/teach.md
```

### Usage
In OpenCode, invoke it with:

```
@teach
```

### Behavior
`teach` focuses on:
- Explaining architecture and subsystem boundaries in plain English
- Mapping implementation details to concrete file references
- Discussing tradeoffs and practical alternatives
- Providing verification/debug checklists and hands-on exercises
- Keeping teach-session continuity in local `.aoc/insight/` artifacts

Default mode is read-only exploration. `teach` should not edit product code unless explicitly asked.

## OpenCode Teach Commands
`aoc-init` also seeds teach-mode commands for OpenCode:

```
.opencode/commands/teach-full.md
.opencode/commands/teach-dive.md
.opencode/commands/teach-ask.md
```

Usage in OpenCode:

```
/teach-full
/teach-dive ingestion
/teach-ask how are you useful?
```

`/teach-full` runs a broad subsystem scan and pauses with numbered next-step choices.
`/teach-dive` deep-dives one subsystem with tradeoffs, debugging checklist, and exercises.
`/teach-ask` is a direct answer-only teaching path that suppresses orchestration/meta wrapper text.

`aoc-init` will not overwrite existing teach command or agent files.

## OpenCode STM Command
`aoc-init` also seeds a project command for OpenCode:

```
.opencode/commands/stm.md
```

Usage in OpenCode:

```
/stm
```

This command asks the agent to contextualize current work into `.aoc/stm/current.md`; review that draft with `aoc-stm`, persist and print the handoff with `aoc-stm handoff`, and resume context in a new session with `aoc-stm resume` (or `aoc-stm read`).

`aoc-init` will not overwrite an existing `.opencode/commands/stm.md`.

## OpenCode PRD Command
`aoc-init` also seeds a PRD orchestration command for OpenCode:

```
.opencode/commands/prd.md
```

Usage in OpenCode:

```
/prd
```

This command asks the agent to orchestrate PRD intake directly from the PRD document, refine with sub-agents, then persist tasks via `aoc-task add/edit` (+ `aoc-task prd set`) safely.

`aoc-init` will not overwrite an existing `.opencode/commands/prd.md`.

## MoreMotion (optional)
Run `aoc-momo init` in a repo to seed the `momo` subagent:

```
.opencode/agents/momo.md
```

Use `@momo` for Remotion animation work in React projects.

## Planned: Sub-agent Rotation (Future)
This is a planned feature and is not implemented yet. The goal is to ship a curated set of OpenCode sub-agents and a rotation flow so users can cycle between them quickly while staying in the same AOC session.

**Planned sub-agent catalog:**
- `architect`
- `task-breaker`
- `implementer`
- `test-planner`
- `code-reviewer`
- `docs-maintainer`
- `security-reviewer`
- `perf-analyzer`
- `release-notes`
- `incident-debugger`
- `rlm-curator`

**Planned UX:**
- `Tab` continues to toggle OpenCode plan/build.
- `Shift+Tab` rotates through the sub-agents in the configured order.
- Active sub-agent is shown in the agent pane label.

**Planned integration:**
- `aoc-init` seeds `.opencode/agents/<name>.md` templates for each sub-agent.
- An OpenCode CLI plugin owns the rotation logic and per-tab selection.
