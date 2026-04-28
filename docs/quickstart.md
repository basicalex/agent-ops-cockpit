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

- Pi pane: coding agent work
- Taskmaster pane: tasks
- File pane: project navigation
- Shell pane: manual commands
- `Alt+C`: tools, setup, logs, health checks
- `Alt+X`: presets/modes

## 6. Basic commands

```bash
tm list                 # tasks
aoc-mem add "decision"  # durable project decision
aoc-stm add "note"      # short-term handoff note
aoc-doctor              # health check
```

## Optional: web research

In AOC, press `Alt+C`:

```text
Settings -> Tools -> Agent Browser + Search
```

Run the install/verify actions top to bottom.

## Optional: HyperFrames

In AOC, press `Alt+C`:

```text
Settings -> Tools -> HyperFrames -> Init workspace + campaign factory
```

Then use:

```text
Alt+X -> AOC HyperFrames
```
