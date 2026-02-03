# MoreMotion

MoreMotion is an optional Remotion workspace that can be embedded inside a project for animation work.

## Initialize

```bash
aoc-momo init
```

By default this:
- Clones `../MoreMotion` into `moremotion/` under the project root.
- Adds `moremotion/` to `.gitignore`.
- Adds the `moremotion` skill to `.aoc/skills`.
- Seeds the OpenCode `momo` subagent in `.opencode/agents/momo.md`.

## Options

```bash
aoc-momo init --source /path/to/MoreMotion --dir moremotion
```

## Notes
- Do not run `aoc-momo` inside the MoreMotion repo itself. Use `aoc-init` there.
- Use `@momo` for Remotion animation work in React projects.
- If you want to update the embedded repo, re-run with `--update`.

## Host vs MoreMotion repo

**Host React repo** (embed MoreMotion):

```bash
aoc-momo init
aoc-agent --set oc
aoc-agent --run oc
```

Then in OpenCode:

```
@momo
```

**Inside MoreMotion itself** (standalone studio):

```bash
aoc-init
aoc-agent --set oc
aoc-agent --run oc
```

Use `@aoc-ops` for setup tasks. `@momo` is only for host repos.
