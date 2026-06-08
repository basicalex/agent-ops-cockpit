# AOC — terminal-first AI workspace

AOC (Agent Ops Cockpit) gives each project a Pi-powered terminal workspace with context, memory, tasks, and operator controls.

Use AOC when you want:

- one persistent Pi coding pane inside Zellij
- project context and decisions stored in the repo
- Taskmaster tasks beside the agent
- `Alt+C` for setup, tools, logs, and health checks
- `Alt+X` for focused modes like Design and HyperFrames
- `Alt+M` for Pi-native Mind/memory actions
- `Alt+A` for Pi-native subagent/delegation management
- optional Open Design GUI studio bridge for higher-quality visual design iteration

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
5. Press `Alt+M` for Mind/memory actions.
6. Press `Alt+A` for subagent/delegation management.

Common setup paths:

| Need | Path |
|---|---|
| Core project setup | `aoc-init` |
| Health check | `aoc-doctor` |
| Tool/install UI | `Alt+C -> Tools` |
| Switch mode/preset | `Alt+X` |
| Mind/memory overlay | `Alt+M`, `/mind` |
| Subagent manager | `Alt+A`, `/subagent-manager` |
| Tasks | `tm list`, Taskmaster pane |
| Memory CLI | `aoc-mem`, `aoc-stm` |
| Open Design GUI studio | `aoc-od install`, then `aoc-od start --open` |
| HyperFrames video/campaign work | `Alt+C -> Tools -> HyperFrames video -> Init workspace + campaign factory` |
| Web research | `Alt+C -> Tools -> Agent Browser + Search` |

## Human docs

Start here:

- [Docs index](./docs/index.md)
- [Quickstart](./docs/quickstart.md)
- [Installation](./docs/installation.md)
- [Control pane](./docs/control-pane.md)
- [Tasks and memory](./docs/tasks-memory.md)
- [Open Design studio](./docs/open-design.md)
- [HyperFrames](./docs/hyperframes.md)
- [Troubleshooting](./docs/troubleshooting.md)

Reference/maintainer docs live under `docs/reference/`, `docs/maintainer/`, and `docs/archive/`.

## Requirements

- Linux, macOS, or WSL
- Zellij `>= 0.44.0` recommended
- Pi coding agent CLI
- Git; optional Jujutsu (`jj`) is detected and supported when a repository already uses it, including colocated Git-backed workspaces. Use explicit `/jj-init` or `jj git init --colocate` to opt a Git repo into Jujutsu; AOC does not auto-initialize it.
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
