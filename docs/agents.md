# Agents (PI-first)

AOC now standardizes on **PI prompt templates** for agent personas and workflows.

## PI Prompt Templates
`aoc-init` seeds project-local PI templates into:

```
.pi/prompts/
```

Seeded templates:
- `/aoc-ops` — AOC setup/ops mode
- `/teach` — repo mentor mode
- `/teach-full` — full architecture scan + checkpoint
- `/teach-dive <subsystem>` — targeted deep dive
- `/teach-ask <question>` — direct answer-only mentor Q&A
- `/tm-cc` — cross-project Taskmaster control mode

`aoc-init` is idempotent and preserves existing prompt files.

## MoreMotion (optional)
Run `aoc-momo init` in a host repo to seed:

```
.pi/prompts/momo.md
```

Use `/momo` for Remotion animation work.

## Deprecation Note
OpenCode project subagent seeding (`.opencode/agents`, `.opencode/commands`) is deprecated in this repo. PI prompt templates are the supported path.
