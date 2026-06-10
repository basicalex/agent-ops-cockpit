# AOC Architecture & Agent Guidelines

This file defines the always-on rules for agents in this repo. Procedural playbooks live in AOC skills.

## Always-on rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- Use root `DESIGN.md` as the visual/product design contract before UI, docs-site, marketing, HyperFrames, or other product-facing work.
- **DO NOT manually read these files** - use the Bash tool to run CLI commands instead (see below).
- Run AOC commands via Bash tool - do NOT use Read tool for `.aoc/memory.md`, `.aoc/stm/current.md`, or `.taskmaster/tasks/tasks.json`.
- RTK routing is default-on for new AOC projects (`.aoc/rtk.toml` mode=`on`); existing explicit mode=`off` is preserved.
- RTK exists to improve context health: allowlisted noisy commands are condensed for better signal density, with fail-open native fallback.

## Startup handshake
- `aoc-handshake --json` is the metadata-only startup packet for agents: AOC status, Taskmaster tag, Mind availability, VCS mode/preferred command family, and usage policy.
- Startup must not load broad Mind memories or context packs by default; use the handshake to discover Mind, not to prime direction.
- Mind may sync/ingest in the background, but retrieval should be lazy and intent-bound.
- Request focused Mind context only after user intent is known, for resume/continuation, prior decisions, task/spec grounding, debugging previous attempts, provenance/audit, or when targeted local inspection is insufficient.
- Always pass an explicit reason when requesting Mind context; prefer focused/provenance/resume modes over broad recall.
- Prefer `jj` commands when handshake VCS metadata reports Jujutsu, including colocated Jujutsu+Git repositories; prefer Git commands only in Git-only repositories.
- Jujutsu setup/import/init/export is operator-controlled. AOC init reports existing `.jj` state but must not create or initialize Jujutsu; use explicit `/jj-init` or a direct shell command for setup.

## Low-Token Default Mode
- Keep responses concise by default; do not print full files or raw logs unless explicitly requested.
- Start with the smallest viable step; use narrow, path-scoped searches before broad scans.
- Read files in bounded chunks and avoid rereading unchanged large files.
- Summarize command/tool output with actionable lines only (key errors, next actions).
- Run targeted checks/tests first; run full-suite commands only when required.
- If targeted inspection fails, escalate scope gradually and state why.
- Use fresh sessions after major milestones or context drift to reduce replay overhead.
- For narrow diagnostics/Q&A, use at most 3 tool calls before first answer; ask before broader escalation.
- Do not open/read image binaries unless the user explicitly asks to view/open one now.
- Use one narrow diagnostic path first; avoid retry spray with variant commands unless first attempt fails.

## AOC CLI Commands (run via Bash tool - NOT Read tool)
These commands are in PATH and work without loading any skill:
- Startup/repair: `aoc-handshake --json`, `aoc-init`
- Memory: `aoc-mem read`, `aoc-mem search "query"`, `aoc-mem add "decision"`
- STM directed handoff only: `aoc-stm status`, `aoc-stm template --purpose <kind>`, `aoc-stm`, `aoc-stm add "note"`, `aoc-stm handoff --purpose <kind> --to <recipient> --focus <focus>`, `aoc-stm resume <archive>`
- Tasks: `tm tag current`, `tm tag spec show`, `aoc-task tag spec show --tag <tag>`, `aoc-task spec show <id> --tag <tag>`
- RTK: `aoc-rtk status`, `aoc-rtk doctor`, `aoc-rtk install --auto`, `aoc-rtk enable|disable`
- VCS: inspect detected mode with `aoc-handshake --json`; use `jj status`/`jj diff` in Jujutsu repos and `git status`/`git diff` in Git-only repos; use `/jj-init` only for explicit colocated Jujutsu setup.

STM is for deliberate directed in-progress handoff packets only; it is not a mailbox and does not notify another agent by itself. Pass the printed next-agent brief or exact archive explicitly. In Pi, `/handoff <focus>` asks the agent to generate a clean purpose-matched packet for the current work; `/rresume [archive]` asks the agent to load a sealed handoff into context safely. Do not use STM for durable decisions, generic logs, raw command output, or every minor task.

## Core files
- `.aoc/context.md`: auto-generated project snapshot.
- `.aoc/rtk.toml`: project-local RTK routing policy and install contract.
- `.aoc/mind-service.json`: optional project-local launcher metadata for the standalone Mind service.
- `.aoc/layouts/`: legacy-only Zellij layouts, created only when `AOC_INIT_LEGACY_ZELLIJ=1`.
- `DESIGN.md`: project-wide visual/product design contract; subsystem design docs extend it.
- `.taskmaster/docs/specs/`: spec documents linked to tags and tasks; `.taskmaster/docs/prds/` remains legacy-compatible.
- Tag default specs are currently stored via legacy key `aocPrd`; resolve with `aoc-task tag spec show --tag <tag>`.
- Task spec overrides are currently stored via legacy key `aocPrd`; resolve with `aoc-task spec show <id> --tag <tag>`.
- Effective precedence is task spec override -> tag default spec.
- Keep task specs in git: `.taskmaster/docs/specs/**` and legacy `.taskmaster/docs/prds/**` should always be tracked.
- Keep AOC/task/config state in git: `.aoc/**`, `.taskmaster/**`, and `.pi/**` should not be ignored except explicit high-churn runtime artifacts such as logs, locks, and `.aoc/mind/project.sqlite`.

## Task Management
- `.taskmaster/tasks/tasks.json` is task state; use the Taskmaster TUI, `aoc-task`, or `tm` (alias for `aoc-task`). Do not edit the file directly.
- Record major decisions and constraints in memory (`aoc-mem add "..."`).

## Skills (load when needed)
Load a skill only when its description matches the user request. Keep always-on guidance here minimal; procedural playbooks belong in skill files.

Note: `aoc-mem`, `aoc-stm`, and `tm` are basic CLI commands (see above) - no skill needed.
