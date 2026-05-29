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

## STM directed handoff layer

Use STM only for deliberate in-progress handoff packets between agents or sessions. It is not durable memory, not a generic work log, and not a mailbox: `aoc-stm handoff` creates a local packet and prints a next-agent brief, but the operator/orchestrator must explicitly pass that brief or archive name to the next agent.

In Pi, use `/handoff <focus>` to ask the agent to generate a clean directed packet for the current work, for example `/handoff focusing on the element refactor`. The slash command treats the text after the command as the operator focus and seals the packet with `aoc-stm handoff --from-file ...`, so stale current STM draft notes are not accidentally bundled.

Use `/rresume [archive-name]` to ask the agent to load a sealed STM handoff into context. With no archive argument, the agent first checks `aoc-stm status` and only uses latest when `safe_to_resume_latest: yes`; otherwise it should ask for the exact archive or permission to inspect the unsealed draft.

```bash
aoc-stm status
aoc-stm template --purpose review
aoc-stm add "Task 226 partial: hardened bin/aoc-stm; docs/tests still needed."
aoc-stm handoff --purpose review --to code-reviewer --focus "audit stale-resume and archive collision risks" --task "226.4"
aoc-stm resume <archive-name>
```

Use STM when:

- another agent/session needs to continue in-progress work
- the operator wants a directed handoff with a specific focus or recipient
- multiple agents may touch nearby files and need coordination context
- context window is getting tight while work is incomplete
- switching from builder to reviewer/tester/documenter

Choose the handoff purpose so the packet matches the next session:

- `continue` — builder/session continuation with next safe actions
- `review` — reviewer focus, changed areas, risks, and review questions
- `test` — behavior under test, commands, fixtures, and suspected gaps
- `debug` — symptom, evidence, hypotheses tried, and next diagnostics
- `docs` — user-facing behavior, examples, and caveats
- `commit` — commit scope, rationale, validation, and task/spec refs

A good STM handoff includes:

- intent and task/subtask IDs
- intended recipient/session and operator focus
- current status: done / partial / blocked
- touched files/areas and changes made
- validation commands and results
- coordination warnings and next safe actions
- do-not-repeat notes for dead ends or completed work

Starting from STM:

- prefer a specific archive or next-agent brief from the operator
- do not blindly trust `aoc-stm resume` latest if stale warnings appear
- verify claims against repository state before editing

Do not use STM for durable decisions, raw logs, every minor task, or information already captured in tasks/specs/commits. Promote durable decisions into `aoc-mem`.

## AOC compaction

AOC seeds a Pi compaction extension at `.pi/extensions/aoc-compaction.ts`. It hooks native `/compact [focus]` and automatic Pi compaction to preserve a small **AOC Operational Context** section in the compaction summary. This keeps post-compaction agents aware of AOC tools such as `aoc_codegraph`, safe commands, and safety rules without loading broad memory, latest STM, full specs, or raw diffs.

The extension may collect bounded metadata from safe commands such as `aoc-handshake --json`, `tm tag current`, `tm show <detected-task-id>`, and `git status --short`. It also includes Pi's recent kept context so newer live evidence can correct stale previous summaries. It should not read protected files directly (`.aoc/memory.md`, `.aoc/stm/current.md`, `.taskmaster/tasks/tasks.json`). Disable it for a run with `AOC_PI_COMPACTION=0` if native Pi compaction is needed.

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
