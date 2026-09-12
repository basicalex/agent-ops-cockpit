# Spec: Taskmaster Execution Cockpit

## Metadata
- Task ID: 232
- Tag: env-protec
- Status: pending
- Priority: high

## Problem
Taskmaster is already AOC's project-local execution ledger, but it mostly stores planned work state. Agents need a compact, task-scoped way to inherit what happened, what was proven, what remains uncertain, and which work is actually actionable without loading broad task history, Mind, STM, or raw logs.

## Goals
- Keep Taskmaster as a task-scoped execution index, not a replacement for Mind, STM, Git, or raw logs.
- Separate active queue work from parked backlog/deferred work.
- Add compact durable outcome metadata for completed/reviewed/cancelled tasks.
- Add agent-facing commands that compile bounded context and audit missing outcome quality.
- Preserve existing Taskmaster JSON compatibility and legacy `aocPrd` spec links.

## Non-Goals
- Do not implement full detached subagent context preparation in the first slice.
- Do not store high-churn process notes in `tasks.json`.
- Do not replace Git commits, STM handoffs, or Mind durable memory.
- Do not break existing `tm done`, `tm list`, or spec/PRD aliases.

## Requirements
- Add `backlog` task status. Default `tm list` should hide parked `backlog` and `deferred` tasks unless explicit filtering or inclusion is requested.
- Add an optional `aocOutcome` task field with summary, status, timestamp, actor, artifacts, verification, gaps, refs, and cancellation/replacement metadata.
- Add `tm complete <id>` for recording outcome metadata and moving to review/done/cancelled.
- Add `tm outcome show <id>` for compact outcome inspection.
- Add `tm context <id> --mode <planning|coding|review|debug>` for bounded local task briefing.
- Add basic `tm audit outcomes` checks for done/review/cancelled outcome gaps.
- Preserve atomic writes and validation rules.

## Acceptance Criteria
- [ ] `tm list` default excludes `backlog` and `deferred`; `--include-parked` or explicit `--status` includes them.
- [ ] `tm status <id> backlog` parses and persists.
- [ ] `tm complete <id> --summary ... --test ... --artifact ...` writes `aocOutcome` and updates status.
- [ ] `tm outcome show <id>` prints stored outcome without dumping unrelated task history.
- [ ] `tm context <id> --mode coding` prints a bounded briefing with intent, spec link, active subtasks, dependencies, outcome/risks, and suggested next step.
- [ ] `tm audit outcomes` reports completed/review/cancelled tasks missing summaries, verification, or cancellation reason.
- [ ] Targeted cargo checks/tests pass.

## Test Strategy
- Use a temporary Taskmaster root to validate CLI behavior without relying on the live project task file.
- Run targeted Rust checks for `aoc-core` and `aoc-cli`.
- Verify JSON compatibility through `tm show --json` or equivalent temp-root command.

## Future Work
- Structured note sidecars under `.taskmaster/notes/tasks/<id>.jsonl`.
- `tm ready --explain` smart readiness.
- `tm context prepare` detached context-gathering subagent packet generation.
- Agent claim leases.
- Spec freshness hashing and artifact history.
