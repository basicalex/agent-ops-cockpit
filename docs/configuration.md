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
.omp/
.taskmaster/
```

See [Project contract](reference/project-contract.md).

Managed AOC assets use marker files and safe refresh rules. See [Managed assets](managed-assets.md).

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

## Startup context diagnostics

```bash
aoc-context doctor
aoc-context registry
aoc-context registry --json
aoc-context explain-startup
aoc-context stale
aoc-context why <source-id>
aoc-context agents
aoc-context agents --write
```

`aoc-context agents --write` generates `.aoc/effective-agent-contract.md`, a compact deduplicated AGENTS.md contract for startup use. Source AGENTS.md files remain authoritative. `aoc-context registry --json` is the modular source registry for loaded, indexed, preset-active, intent-triggered, manual-only, and never-inject-source context blocks.

Managed Pi launches default to `AOC_PI_CONTEXT_KERNEL=on`: `aoc-agent-wrap` refreshes the generated contract when stale, starts Pi with `--no-context-files`, and appends a compact AOC context kernel containing the effective contract, project snapshot, router explanation, and metadata-only handshake. Set `AOC_PI_CONTEXT_KERNEL=off` to fall back to Pi's raw context-file discovery.

## Skills

```bash
aoc-skill sync --root .
aoc-skill validate --root .
```

See [Skills](skills.md).

## RTK routing

RTK routing condenses noisy allowlisted command output while preserving native fallback for safe commands.

```bash
aoc-rtk status
aoc-rtk doctor
```

See [RTK routing](reference/rtk-routing.md).

## Detailed reference

Older exhaustive config notes are preserved at [reference/configuration-details.md](reference/configuration-details.md).
