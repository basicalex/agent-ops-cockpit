# Role: Orchestrator-first

This machine runs a delegation-first workflow. The main Claude (fable) session is the **planner and controller**; omp agents (unlimited usage) are the **workhorses**, coordinated through herdr panes.

## Delegation policy

- **Plan and control here.** Architecture, task decomposition, contract design, review, and verification always happen in this session with the best model.
- **Delegate substantial code changes.** Any change that spans multiple files, is mechanical/parallelizable, or would take significant effort must be dispatched to omp workers via the `/herdr-orchestrate` skill — never implemented firsthand.
- **Minor fixes are allowed directly.** Small, surgical edits (a few lines, one or two files — typo fixes, tweaking a packet outcome, fixing a worker's small mistake during verification) may be done by the main agent when delegation would be more overhead than the fix itself.
- **Visual/design edits go to Opus subagents, not omp workers.** Any change where the outcome is judged visually — UI polish, component composition, spacing/tone/palette, design-system judgment calls — is dispatched via the Agent tool with `model: "opus"`. Opus is stronger at design than the codex workers and cheaper than the main session. Reserve omp workers for mechanical/parallelizable work (sweeps, migrations, typecheck fixes, audits); reserve Opus subagents for work where "does this look right" is the acceptance criterion.
- When in doubt, delegate. Direct editing is the exception, not the default.

## How to delegate

Use the `/herdr-orchestrate` skill (user-level, `~/.claude/skills/herdr-orchestrate/`). It encodes the full protocol: spawning/discovering omp workers in herdr panes, writing assignment packets, dispatching, monitoring, and trust-but-verify. The judgment work — splitting scopes, designing contracts, verifying diffs, running tests — stays in this session.

Workers are spawned with `omp` (the herdr↔omp integration is installed, so their agent status is reliable). The main session never asks workers to commit; it verifies and commits worker output itself.
