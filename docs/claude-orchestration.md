# Claude-orchestrates-OMP workflow

The `claude/` directory versions the Claude-side half of the machine's delegation-first workflow: the main Claude session (best available model) acts as planner/controller, and OMP agents (effectively unlimited usage) act as workhorses, coordinated through herdr.

## Assets

- `claude/CLAUDE.global.md` — installed to `~/.claude/CLAUDE.md`. Global policy loaded by every Claude session on the machine: delegate substantial code changes to OMP workers via the `herdr-orchestrate` skill; route visual/design slices to Opus subagents; minor surgical fixes may be done directly. Fable (Claude) escalation is a third, explicit-request-only tier for critical slices, since fable sessions burn metered Claude usage while OMP is effectively unlimited. Default form: an escalation agent via the Agent tool with no model override (inherits the fable model, runs in-session). Herdr fable worker panes are reserved for critical slices that must run as long-lived peers of a parallel campaign.
- `claude/skills/herdr-orchestrate/SKILL.md` — installed to `~/.claude/skills/herdr-orchestrate/`. User-level Claude skill encoding the full protocol: spawn one labeled herdr tab per worker (`herdr tab create` + `herdr pane run <root-pane> "aoc-omp"`, or `"claude --dangerously-skip-permissions"` for user-requested fable workers — standing-authorized for spawned worker panes so packets never stall on permission prompts, making packet file-scope CONSTRAINTS the only guardrail), dispatch fixed-section assignment packets (GOAL/CONTEXT/STEPS/CONSTRAINTS/ACCEPTANCE) with non-overlapping file scopes, monitor via herdr agent status (requiring 3+ consecutive idle checks to filter between-turn idle blips), then trust-but-verify: independent diff review, self-run tests, per-slice commits by the orchestrator (workers never commit).

## Install

`install.sh` seeds the Claude plane by default during AOC installation by copying these assets into `${AOC_CLAUDE_DIR:-$HOME/.claude}` with timestamped backups when overwriting differing files (same convention as `bin/aoc-herdr-install`). Set `AOC_CLAUDE_DIR` to target a different Claude config directory.

This is intentionally install-time-only seeding, not `aoc-init` managed-assets: `~/.claude` is edited live and resynced back to the repo, so init-time stamping from stale repo copies could clobber newer live edits.

Prerequisites on the target machine: `omp` must be on PATH, and `herdr` must have the `omp` integration installed (`herdr integration install omp`) so worker agent status is reliable.

## Relationship to AOC master orchestration

This plane coexists with AOC's OMP-native master orchestration (`aoc-master.ts`, `aoc_orchestrate`/`aoc_report`) with no hierarchy between them:

- **Claude/herdr-orchestrate**: interactive campaigns driven from a Claude session, dispatching via raw herdr CLI and packet files.
- **AOC master (`/master on`)**: OMP-native assignment flow with gated tools.

They share no state. Do not run both masters over the same worker pool at the same time — a worker receiving packets from a Claude orchestrator and assignments from an OMP master would mix file scopes.

Worker lifecycle rule of thumb: worker lifetime = slice lifetime (re-dispatch revisions of a slice to its original worker); tab lifetime = campaign lifetime (spawned tabs are closed after verification; never reuse workers across campaigns).
