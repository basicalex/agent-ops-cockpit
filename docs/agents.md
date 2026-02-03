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
