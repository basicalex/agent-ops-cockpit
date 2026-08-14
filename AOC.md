# Agent Ops Cockpit (AOC)

AOC is a Herdr-first, OMP-native workspace layer for AI-assisted development.

Core pieces:

- **Herdr workspace**: workspaces, tabs, panes, navigation, agent status, and operator UI.
- **OMP runtime**: canonical coding agent runtime for AOC projects.
- **Master orchestration**: `/master` and `aoc_orchestrate` provide gated peer coordination.
- **Taskmaster**: project task tracking through `tm` / `aoc-task`.
- **AOC context + memory**: repo-owned `.aoc/` and `.omp/` assets plus metadata-only OMP startup capsules keep project continuity; durable agent memory lives in the shared `~/.aoc/memory/<repo>/` store.
- **OMP extension surface**: `.omp/manifest.toml` owns installed extensions, skills, and agents, including CodeGraph, `/commit`, `/master`, HyperFrames, and web research fallback.

Start with:

```bash
aoc-init
aoc
```

Human docs start at [docs/herdr-workspace.md](docs/herdr-workspace.md) and [docs/quickstart.md](docs/quickstart.md).

