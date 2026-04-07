# MoreMotion

MoreMotion is an optional Remotion workspace that can be embedded inside a project for animation work.

## Initialize

```bash
aoc-momo init
```

By default this:
- Resolves a local MoreMotion source (prefers `AOC_MOMO_SOURCE`, then `../MoreMotion` / `../moremotion`).
- Initializes/updates `moremotion/` under the project root from that source.
- Adds `moremotion/` to `.gitignore`.
- Adds the `moremotion` skill to `.pi/skills`.
- Seeds the PI `momo` prompt template in `.pi/prompts/momo.md`.

If no source can be resolved, `aoc-momo` exits with guidance to set `--source` / `AOC_MOMO_SOURCE`.

## Options

```bash
aoc-momo init --source /path/to/MoreMotion --dir moremotion
```

## Notes
- Do not run `aoc-momo` inside the MoreMotion repo itself. Use `aoc-init` there.
- Use `/momo` for Remotion animation work in host React projects.
- If you want to update the embedded repo, re-run with `--update`.
- In `Alt+C -> Settings -> Tools -> MoreMotion`, `Ensure local source repo`:
  - pulls with `git pull --ff-only` when the local source already exists
  - prompts before cloning when missing (requires `AOC_MOREMOTION_REPO_URL`)
