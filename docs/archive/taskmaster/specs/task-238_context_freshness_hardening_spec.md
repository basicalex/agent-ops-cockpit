# Task 238 Spec: AOC coding tool context-freshness hardening

## Goal

Harden AOC so coding agents can discover and use the current coding/architecture support capabilities without broad context injection or stale wrapper binaries.

## Scope

Implement the first hardening slice from the context-freshness audit:

1. Add a shared compact capability surface to `aoc-context`.
2. Make `tm` / `aoc-task` prefer the current project CLI when available so newly implemented Taskmaster commands work before reinstall.
3. Keep startup and compaction philosophy compact/lazy; do not add broad Mind/STM/task DB injection.

## Non-goals

- No open-design/product creation tooling.
- No detached `tm context prepare` implementation.
- No full handshake task packet expansion in this slice.
- No broad memory or raw `.taskmaster/tasks/tasks.json` reads by agents.

## Acceptance criteria

- `aoc-context capabilities` prints a concise tool capability capsule.
- `aoc-context capabilities --json` emits machine-readable capability status.
- Capsule advertises current cockpit commands: `tm ready --explain`, `tm context`, `tm outcome show`, `tm audit outcomes`, `tm complete`.
- `tm ready --tag env-protec --limit 1` works from the repo checkout when a project-local built CLI exists.
- `aoc-task` preserves `--tm-root` behavior while using project-local binaries when present.
- Shell syntax checks pass for changed scripts.
- Existing `aoc-handshake --json` remains metadata-only.

## Test strategy

- `bash -n bin/aoc-context bin/tm bin/aoc-task`
- `aoc-context capabilities`
- `aoc-context capabilities --json` parsed by Python JSON loader
- `tm ready --tag env-protec --limit 1`
- temp-root `tm --tm-root <tmp> ...` smoke if practical
- `aoc-handshake --json` parse and verify `mode == metadata-only`
