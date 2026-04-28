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

## Agent startup

Agents should use:

```bash
aoc-handshake --json
```

This gives project status without dumping broad memory. Retrieve memory only when needed for the current task.
