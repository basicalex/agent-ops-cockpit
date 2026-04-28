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
which zellij
aoc-doctor
```

Fix PATH first. AOC installs to user-local paths such as `~/.local/bin`.

## Pi pane fails

Check:

```bash
which pi
aoc-agent --set pi
aoc-doctor
```

If model/auth setup is the issue, use Pi's own model/auth commands, then restart `aoc`.

## `Alt+C` does not open

Check you are inside the AOC Zellij session. Then run manually:

```bash
aoc-control
```

If it fails, reinstall:

```bash
./install.sh
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
aoc-stm resume
```

only when relevant to the current task.

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

Use:

```text
Alt+C -> Settings -> Tools -> Agent Browser + Search
```

Run install/verify actions top to bottom.

Managed local search needs Docker or Docker Compose.

## Zellij layout looks wrong

Run:

```bash
aoc-doctor
aoc-layout list
```

Then restart the AOC session.

## Need deeper logs

AOC writes logs under project and user-local state paths. Start with the command that failed, then use `Alt+C` details/log actions when available.
