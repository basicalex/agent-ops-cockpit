# Agent Ops Cockpit (AOC)

AOC is a Pi-first terminal workspace for AI-assisted development.

Core pieces:

- **Zellij workspace**: persistent panes for files, Pi, tasks, and shell work.
- **Pi runtime**: canonical agent runtime for AOC projects.
- **Taskmaster**: project task tracking through `tm` / `aoc-task`.
- **AOC context + memory**: `.aoc/context.md`, `aoc-mem`, and `aoc-stm` keep project continuity.
- **Alt+C control pane**: installs tools, starts checks, opens logs, manages integrations.
- **Alt+X presets**: switches the agent into focused modes such as HyperFrames.

Start with:

```bash
aoc-init
aoc
```

Human docs start at [docs/index.md](docs/index.md).

Older multi-runtime/OpenCode notes are archived under `legacy/` and `docs/archive/` where still useful for maintenance history.
