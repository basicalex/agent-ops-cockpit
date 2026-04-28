# Configuration

Most configuration should happen through AOC commands or `Alt+C`, not manual file edits.

## Project setup

```bash
aoc-init
aoc-init --status
```

Project-local config lives in:

```text
.aoc/
.pi/
.taskmaster/
```

See [Project contract](reference/project-contract.md).

## Control pane

Press `Alt+C` inside AOC for:

- tool setup
- optional integrations
- health checks
- logs
- HyperFrames setup
- Agent Browser/Search setup

See [Control pane](control-pane.md).

## Common environment variables

| Variable | Purpose |
|---|---|
| `AOC_PROJECT_ROOT` | Force project root for AOC commands |
| `AOC_AGENT` | Override selected agent for a launch |
| `AOC_LAYOUT` | Select layout for a launch |
| `AOC_INIT_SKIP_BUILD=1` | Skip build-heavy init steps |
| `AOC_PRESET_WIDGET_VERBOSE=1` | Show verbose preset widget details |
| `AOC_HYPERFRAMES_DIR` | Override HyperFrames workspace dir |

## Layouts

```bash
aoc-layout list
aoc-layout set <name>
```

See [Layouts](layouts.md).

## Skills

```bash
aoc-skill sync --root .
aoc-skill validate --root .
```

See [Skills](skills.md).

## Detailed reference

Older exhaustive config notes are preserved at [reference/configuration-details.md](reference/configuration-details.md).
