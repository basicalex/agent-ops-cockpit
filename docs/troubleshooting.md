# Troubleshooting

Start with:

```bash
aoc-doctor
```

Then use the symptom below.

## `aoc` does not start

Check:

```bash
which aoc
which herdr
aoc-doctor
```

Fix PATH first. AOC installs to user-local paths such as `~/.local/bin`.

## OMP agent fails

Check:

```bash
which omp
aoc-doctor
```

If model/auth setup is the issue, check OMP's own provider credentials and model selection in `~/.omp/agent/config.yml`, then restart `aoc` or `aoc omp`.

To bypass the AOC-aware OMP shim while isolating wrapper problems:

```bash
OMP_NO_AOC_WRAPPER=1 omp ...
```

## OMP extensions not loading

Check the current OMP log:

```bash
~/.omp/logs/omp.<date>.log
```

Look for `Failed to load extension` errors.

Known failure: OMP reports a `typebox.ts` resolution error because `src/extensibility/typebox.ts` is missing from the OMP binary dir. Fix by relinking the package source shim:

```bash
aoc-herdr-install --force
```

## Skills missing or stale

Run:

```bash
aoc-init
aoc-skill sync --root .
aoc-skill validate --root .
```

For HyperFrames specifically:

```bash
aoc-hyperframes sync-skills
aoc-hyperframes check --dir hyperframes
```

## Task list blank

Run:

```bash
tm list
```

If no tasks exist:

```bash
tm add "First task"
```

Do not edit `.taskmaster/tasks/tasks.json` directly.

## Memory/context stale

Run:

```bash
aoc-init
aoc-handshake --json
```

Use:

```bash
aoc-mem search "topic"
aoc-stm status
aoc-stm resume <archive-name>  # prefer a specific operator-provided archive
```

only when relevant to the current task. Do not treat latest STM as delivered/authoritative if stale warnings appear.

## HyperFrames check fails

Run:

```bash
aoc-hyperframes doctor
aoc-hyperframes bootstrap-asset-system --dir hyperframes
aoc-hyperframes check --dir hyperframes
```

Common fixes:

- install Node.js `>= 22`
- install FFmpeg
- ensure root `DESIGN.md` exists
- ensure `hyperframes/docs/DESIGN.md` references root design
- remove any whole-directory `hyperframes/` ignore from `.gitignore`

## Web research fails

See [Web research](web-research.md).

Check service state:

```bash
aoc services status
```

Then test the CLI path agents use:

```bash
aoc-search "test query"
```

Managed local search needs Docker or Docker Compose.

## Herdr workspace issues

Check Herdr:

```bash
herdr status
herdr server reload-config
```

Use the workspace picker with `Alt+W` to confirm the expected workspace is active.

## Master orchestration

Check:

```text
/master status
```

Common cases:

- `/master status` shows `off` when the master lease expired.
- `/master on` fails if another pane owns the lease.
- `aoc_orchestrate` mutating actions require `master_on` first.

## Need deeper logs

Start with:

```bash
aoc-doctor
```

Then inspect the current OMP log:

```bash
~/.omp/logs/omp.<date>.log
```

