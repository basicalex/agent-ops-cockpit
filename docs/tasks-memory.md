# Tasks and memory

AOC keeps work visible through Taskmaster, durable memory, and short-term handoff notes.

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

## Durable memory

Use durable memory for decisions that should survive sessions:

```bash
aoc-mem add "Decision: API clients use retry budget X because Y."
aoc-mem search "retry budget"
aoc-mem read
```

Good memory entries:

- architecture decisions
- user preferences
- constraints that affect future work
- project-specific conventions

Bad memory entries:

- raw logs
- temporary TODOs
- guesses
- long pasted files

## Short-term memory

Use STM for session notes and handoffs:

```bash
aoc-stm add "Implemented parser; tests still needed."
aoc-stm
aoc-stm handoff
aoc-stm resume
```

Use STM when:

- pausing mid-task
- handing work to another session
- recording next actions after a large change

Promote durable decisions from STM into `aoc-mem`.

## Commit intelligence

Commits are durable engineering checkpoints. Use them to record why work changed, which tasks/PRDs it belongs to, and how it was validated.

See [commit-intelligence.md](commit-intelligence.md).

## Agent startup

Agents should use:

```bash
aoc-handshake --json
```

This gives project status without dumping broad memory. Retrieve memory only when needed for the current task.

## AOC Mind

AOC Mind is the deeper project-memory and provenance system behind focused context packs, session-derived knowledge, and operator views. It is intentionally lazy: agents request Mind context only for a specific reason, not at every startup.

Use:

```bash
aoc-mind-service context-pack --project-root "$PWD" --mode focused --reason "resume task 123" --json
```

Architecture details: [reference/aoc-mind-architecture.md](reference/aoc-mind-architecture.md).
