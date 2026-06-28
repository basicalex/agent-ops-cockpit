# Installation details

AOC installation is OMP-first.

## Runtime assets

`aoc-init` and `aoc-herdr-install` install repo-owned OMP assets into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}`:

```text
extensions/aoc-codegraph.ts
extensions/aoc-mind.ts
extensions/aoc-commit.ts
extensions/aoc-state.ts
extensions/aoc-dox.ts
extensions/aoc-dox-command.ts
extensions/aoc-brand-content.ts
extensions/aoc-web-search.ts
agents/brand-strategy.md
agents/brand-concept.md
agents/svg-asset.md
agents/hyperframes-content.md
agents/dox-scout.md
agents/dox-mapper.md
agents/dox-critic.md
agents/dox-writer.md
skills/<ported AOC skill>/SKILL.md
```

Global/operator runtime configuration is `~/.omp/agent/config.yml`. AOC does not commit or seed secrets there.

## Project assets

AOC project initialization creates or repairs:

```text
.aoc/context.md
.aoc/rtk.toml
.aoc/mind-service.json (optional, when Mind runtime setup is enabled)
.omp/extensions/
.omp/agents/
.omp/skills/
.taskmaster/
AGENTS.md
DESIGN.md
```

## Smoke checks

```bash
test -f .omp/extensions/aoc-codegraph.ts
test -f .omp/extensions/aoc-state.ts
test -f .omp/agents/brand-strategy.md
test -f .omp/skills/aoc-init-ops/SKILL.md
AOC_OMP_CONTEXT_LEVEL=min bin/aoc-omp-context .
```
