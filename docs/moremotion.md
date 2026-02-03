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
