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
- Preserving existing repo skills and avoiding collisions

`aoc-init` will not overwrite existing `.opencode/agents/aoc-ops.md`.

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
