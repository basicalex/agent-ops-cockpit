# Claude-orchestrates-OMP workflow

The `claude/` directory versions the Claude-side half of the machine's delegation-first workflow: the main Claude session (best available model) acts as planner/controller, and OMP agents (effectively unlimited usage) act as workhorses, coordinated through herdr.

## Assets

- `claude/CLAUDE.global.md` — installed to `~/.claude/CLAUDE.md`. Global policy loaded by every Claude session on the machine: delegate substantial code changes to OMP workers via the `herdr-orchestrate` skill; route visual/design slices to Opus subagents; minor surgical fixes may be done directly.
- `claude/skills/herdr-orchestrate/SKILL.md` — installed to `~/.claude/skills/herdr-orchestrate/`. User-level Claude skill encoding the full protocol: spawn one labeled herdr tab per worker (`herdr tab create` + `herdr pane run <root-pane> "omp"`), dispatch fixed-section assignment packets (GOAL/CONTEXT/STEPS/CONSTRAINTS/ACCEPTANCE) with non-overlapping file scopes, monitor via herdr agent status, then trust-but-verify: independent diff review, self-run tests, per-slice commits by the orchestrator (workers never commit).

## Install

```bash
bin/aoc-claude-install
```

Copies assets into `~/.claude` with timestamped backups when overwriting differing files (same convention as `bin/aoc-herdr-install`). Prerequisites on the target machine: `herdr` with the `omp` integration installed (`herdr integration install omp`) so worker agent status is reliable, and `omp` on PATH.

## Relationship to AOC master orchestration

This plane coexists with AOC's OMP-native master orchestration (`aoc-master.ts`, `aoc_orchestrate`/`aoc_report`) with no hierarchy between them:

- **Claude/herdr-orchestrate**: interactive campaigns driven from a Claude session, dispatching via raw herdr CLI and packet files.
- **AOC master (`/master on`)**: OMP-native assignment flow with gated tools.

They share no state. Do not run both masters over the same worker pool at the same time — a worker receiving packets from a Claude orchestrator and assignments from an OMP master would mix file scopes.

Worker lifecycle rule of thumb: worker lifetime = slice lifetime (re-dispatch revisions of a slice to its original worker); tab lifetime = campaign lifetime (spawned tabs are closed after verification; never reuse workers across campaigns).
