# Task 237 Spec: Smart Taskmaster Readiness Explanations

## Goal

Add `tm ready` as the next Taskmaster execution-cockpit primitive: a compact, agent-facing readiness view that answers “what should I work on now, and why?” without replacing `tm next` or turning Taskmaster into memory/log storage.

## Scope

Implement a local deterministic CLI command:

- `tm ready [--tag <tag>] [--json] [--limit <n>] [--explain]`
- default tag resolution follows existing Taskmaster behavior.
- default output lists top actionable tasks with concise reasons.
- `--explain` additionally reports blocked and parked candidates with reasons.
- `--json` emits stable machine-readable readiness packets.

## Semantics

- `backlog` and `deferred` are parked and excluded from default actionable work.
- `done` is terminal and dependency-fulfilling.
- `cancelled` is terminal but does not fulfill dependencies.
- `blocked` is not actionable, but should appear in `--explain` with its blocker reasons.
- tasks with unfinished dependencies are not actionable and should explain missing dependency ids.
- tasks with missing dependency ids are not actionable and should report missing references.
- active actionable statuses are `pending`, `in-progress`, and `review` once dependencies are fulfilled.
- readiness ranking should remain simple and explainable: active-agent work first, then in-progress, review, priority, and stable task id order.

## Non-goals

- Do not implement detached context preparation in this slice.
- Do not add claim leases or sidecar notes yet.
- Do not mutate task state when running `tm ready`.
- Do not change legacy `tm next` behavior except shared helper use if safe.

## Acceptance Criteria

- `tm ready` lists actionable non-parked tasks only.
- `tm ready --explain` includes blocked/parked/unready tasks with concise reasons.
- `tm ready --json` produces compact JSON with `ready`, `blocked`, `parked`, and `summary` sections.
- done dependencies fulfill readiness.
- cancelled dependencies do not fulfill readiness.
- parked dependencies do not accidentally make their dependents ready unless fulfilled.
- command compiles with targeted crates and passes temp-root smoke tests.

## Test Strategy

- `cargo check -p aoc-core -p aoc-cli --manifest-path crates/Cargo.toml`
- temp-root CLI smoke:
  - pending task appears ready
  - backlog/deferred tasks are parked
  - blocked task is not ready and appears under explain
  - task depending on done task is ready
  - task depending on cancelled task is not ready
  - JSON output contains expected readiness sections
