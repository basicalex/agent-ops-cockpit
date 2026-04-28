# AOC — terminal-first AI workspace

AOC (Agent Ops Cockpit) gives each project a Pi-powered terminal workspace with context, memory, tasks, and operator controls.

Use AOC when you want:

- one persistent Pi coding pane inside Zellij
- project context and decisions stored in the repo
- Taskmaster tasks beside the agent
- `Alt+C` for setup, tools, logs, and health checks
- `Alt+X` for focused modes like HyperFrames

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

Local clone:

```bash
./install.sh
```

Then:

```bash
aoc-doctor
cd ~/your-project
aoc-init
aoc
```

## First run

Inside AOC:

1. Use the Pi pane for coding.
2. Use Taskmaster for tasks.
3. Press `Alt+C` for setup, integrations, logs, and health checks.
4. Press `Alt+X` to switch project modes/presets.

Common setup paths:

| Need | Path |
|---|---|
| Core project setup | `aoc-init` |
| Health check | `aoc-doctor` |
| Tool/install UI | `Alt+C -> Settings -> Tools` |
| Switch mode/preset | `Alt+X` |
| Tasks | `tm list`, Taskmaster pane |
| Memory | `aoc-mem`, `aoc-stm` |
| HyperFrames video/campaign work | `Alt+C -> HyperFrames -> Init workspace + campaign factory` |
| Web research | `Alt+C -> Agent Browser + Search` |

## Human docs

Start here:

- [Docs index](./docs/index.md)
- [Quickstart](./docs/quickstart.md)
- [Installation](./docs/installation.md)
- [Control pane](./docs/control-pane.md)
- [Tasks and memory](./docs/tasks-memory.md)
- [HyperFrames](./docs/hyperframes.md)
- [Troubleshooting](./docs/troubleshooting.md)

Reference/maintainer docs live under `docs/reference/`, `docs/maintainer/`, and `docs/archive/`.

## Requirements

- Linux, macOS, or WSL
- Zellij `>= 0.44.0` recommended
- Pi coding agent CLI
- Git
- Optional: Docker for managed local search
- Optional: Node.js `>= 22` and FFmpeg for HyperFrames

## Troubleshooting

Run:

```bash
aoc-doctor
```

Then see [Troubleshooting](./docs/troubleshooting.md).

## License

Apache-2.0. See [LICENSE](./LICENSE).
