# Tasks and project context

AOC keeps work visible through Taskmaster, commits, and focused context tools.

## Tasks

Use `tm` for project work tracking:

```bash
tm list
tm add "Implement feature"
tm done 123
tm tag current
```

Rules:

- Use tasks for planned work, bugs, and implementation checkpoints.
- Do not edit `.taskmaster/tasks/tasks.json` by hand.
- Link PRDs or specs when work has product/architecture context.


## AOC compaction

AOC no longer ships a Pi compaction extension. OMP context health is handled by the OMP runtime plus AOC metadata tools such as `aoc-handshake --json` and `aoc-omp-context`. Task state remains CLI-mediated, not directly injected.

## Commit intelligence

Commits are durable engineering checkpoints. Use them to record why work changed, which tasks/PRDs it belongs to, and how it was validated.

See [commit-intelligence.md](commit-intelligence.md).

## Agent startup

Agents should use:

```bash
aoc-handshake --json
```

This gives project status without dumping broad project context. Retrieve deeper context only when needed for the current task.

## Focused context

AOC keeps startup context compact. Agents should retrieve deeper project context only for a specific task reason, using Taskmaster, Mnemopi, CodeGraph, and AOC CLI surfaces instead of broad startup dumps.
