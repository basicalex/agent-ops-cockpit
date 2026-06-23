# Quickstart

## 1. Install

```bash
curl -fsSL https://raw.githubusercontent.com/basicalex/agent-ops-cockpit/main/install/bootstrap.sh | bash
```

Local clone:

```bash
./install.sh
```

## 2. Check install

```bash
aoc-doctor
```

Fix anything marked required.

## 3. Initialize project

```bash
cd ~/your-project
aoc-init
```

This seeds project-local AOC files without overwriting existing work.

## 4. Start AOC

```bash
aoc
```

## 5. Use the workspace

- OMP coding agent pane: coding work and OMP subagent orchestration
- Taskmaster: use `tm` commands for tasks/specs
- `/commit`: safe atomic commits
- `/master`: gated master orchestration
- `/dox`: DOX cartography
- `aoc-doctor`: install/project health checks

## 6. Basic commands

```bash
tm list                               # tasks
aoc-mem add "decision"                # durable project decision
aoc-stm template --purpose continue   # purpose-specific handoff shape
aoc-stm add "note"                    # short-term handoff draft note
aoc-stm handoff --purpose continue --to builder --focus "next safe step"
aoc-doctor                            # health check
```

OMP slash commands:

```text
/master on                            # enable gated master orchestration
/commit                               # run safe atomic commit workflow
```

## Optional: web research

OMP agents use the `aoc_web_search` tool for local SearXNG or direct package/GitHub search fallback.

Operators can manage the local search service through the Herdr AOC Services workspace:

```bash
aoc services
aoc services status
aoc services start search
```

See [Web research](web-research.md).

## Optional: HyperFrames

Use the AOC HyperFrames CLI or the OMP brand pipeline:

```bash
aoc-hyperframes
```

OMP slash command:

```text
/brand-content
```

See [HyperFrames](hyperframes.md).
