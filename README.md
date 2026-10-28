# AOC — Herdr-first AI workspace

AOC (Agent Ops Cockpit) gives each project a Herdr workspace with OMP-native coding agents, repo-owned context, and project tooling.

Use AOC when you want:

- a Herdr workspace with the OMP coding agent
- master orchestration through `/master`
- project context stored in the repo

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
2. Use `/commit` for safe commits.
3. Use `/master` for orchestration.
4. Run `aoc-doctor` for health checks.

Common setup paths:

| Need | Path |
|---|---|
| Core project setup | `aoc-init` |
| Health check | `aoc-doctor` |
| OMP coding agent | `aoc omp`, `omp` |
| Safe commit workflow | `/commit [intent]` |
| Master orchestration | `/master on [minutes]`, `/master off`, `/master status` |

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
- Bun

## Troubleshooting

Run:

```bash
aoc-doctor
```

Then see [Troubleshooting](./docs/troubleshooting.md).

## License

Apache-2.0. See [LICENSE](./LICENSE).
