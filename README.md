# AOC — Herdr-first AI workspace

AOC (Agent Ops Cockpit) gives each project a Herdr workspace with OMP-native coding agents, repo-owned context, memory, tasks, and retained project tooling.

Use AOC when you want:

- a Herdr workspace with the OMP coding agent
- master orchestration through `/master`
- project context and memory stored in the repo
- Taskmaster tasks through `tm` / `aoc-task`
- CodeGraph code discovery for OMP agents
- HyperFrames video and campaign tooling
- web research fallback through local search or direct package/GitHub modes

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

1. Use the OMP pane for coding.
2. Use Taskmaster for tasks.
3. Use `/commit` for safe commits.
4. Use `/master` for orchestration.
5. Run `aoc-doctor` for health checks.

Common setup paths:

| Need | Path |
|---|---|
| Core project setup | `aoc-init` |
| Health check | `aoc-doctor` |
| OMP coding agent | `aoc omp`, `omp` |
| Safe commit workflow | `/commit [intent]` |
| Master orchestration | `/master on [minutes]`, `/master off`, `/master status` |
| Tasks | `tm list`, `aoc-task` |
| Memory and handoff CLI | `aoc-mem`, `aoc-stm` |
| Code discovery | `aoc_codegraph` in OMP, `codegraph sync /path/to/project` by operator |
| HyperFrames video/campaign work | `aoc-hyperframes`, `/hyperframes-director`, `/brand-content` |
| Web research fallback | `aoc_web_search`, `aoc-search`, `aoc services` |
| Open Design GUI studio | `aoc-od install`, then `aoc-od start --open` |

## Human docs

Start here:

- [Herdr workspace](./docs/herdr-workspace.md)
- [Quickstart](./docs/quickstart.md)
- [Installation](./docs/installation.md)
- [Troubleshooting](./docs/troubleshooting.md)
- [Agent extensibility](./docs/agent-extensibility.md)

Reference/maintainer docs live under `docs/reference/`, `docs/maintainer/`, and `docs/archive/`.

## Requirements

- Linux, macOS, or WSL
- Herdr
- OMP coding agent CLI (`omp`)
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
