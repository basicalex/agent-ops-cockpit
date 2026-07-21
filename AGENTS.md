# AOC Architecture & Agent Guidelines

This file defines the always-on rules for agents in this repo. Procedural playbooks live in AOC skills.

## Always-on rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- Use root `DESIGN.md` as the visual/product design contract before UI, docs-site, marketing, HyperFrames, or other product-facing work.
- **DO NOT manually read these files** - use the Bash tool to run CLI commands instead (see below).
- Run AOC commands via Bash tool; do not read `.taskmaster/tasks/tasks.json` directly.
- RTK routing is default-on for new AOC projects (`.aoc/rtk.toml` mode=`on`); existing explicit mode=`off` is preserved.
- RTK exists to improve context health: allowlisted noisy commands are condensed for better signal density, with fail-open native fallback.

## Startup handshake
- `aoc-handshake --json` is the metadata-only startup packet for agents: AOC status, Taskmaster tag, Git repository state, and usage policy.

## Low-Token Default Mode
- Keep responses concise by default; do not print full files or raw logs unless explicitly requested.
- Start with the smallest viable step; use narrow, path-scoped searches before broad scans.
- When `.codegraph/` exists and the OMP `aoc_codegraph` tool is available, use it before broad text/file scans for code discovery, call-flow mapping, impact probes, and affected-test selection.
- Read files in bounded chunks and avoid rereading unchanged large files.
- Summarize command/tool output with actionable lines only (key errors, next actions).
- Run targeted checks/tests first; run full-suite commands only when required.
- If targeted inspection fails, escalate scope gradually and state why.
- Use fresh sessions after major milestones or context drift to reduce replay overhead.
- For narrow diagnostics/Q&A, use at most 3 tool calls before first answer; ask before broader escalation.
- Do not open/read image binaries unless the user explicitly asks to view/open one now.
- Use one narrow diagnostic path first; avoid retry spray with variant commands unless first attempt fails.

## Lightweight Validation
- Prefer OMP `lsp diagnostics` on touched files/globs for edit-loop validation before running build, lint, or typecheck commands.
- Use `lsp references` before changing exported symbols, and `lsp code_actions` for language-server fixes/imports when available.
- Do not run full project build/lint/test as a routine sanity check during active edits.
- In delegated multi-agent work, subagents should skip project-wide validation unless explicitly assigned.
- Final verification is still required; choose the smallest targeted command that proves the changed behavior.

## AOC CLI Commands (run via Bash tool - NOT Read tool)
These commands are in PATH and work without loading any skill:
- Startup/repair: `aoc-handshake --json`, `aoc-init`
- Tasks: `tm tag current`, `tm tag spec show`, `aoc-task tag spec show --tag <tag>`, `aoc-task spec show <id> --tag <tag>`
- RTK: `aoc-rtk status`, `aoc-rtk doctor`, `aoc-rtk install --auto`, `aoc-rtk enable|disable`
- VCS: inspect detected mode with `aoc-handshake --json`; use `git status`/`git diff` in Git repositories.


## Core files
- `.aoc/context.md`: auto-generated project snapshot.
- `.aoc/rtk.toml`: project-local RTK routing policy and install contract.
- `DESIGN.md`: project-wide visual/product design contract; subsystem design docs extend it.
- `.taskmaster/docs/specs/`: spec documents linked to tags and tasks; `.taskmaster/docs/prds/` remains legacy-compatible.
- Tag default specs are currently stored via legacy key `aocPrd`; resolve with `aoc-task tag spec show --tag <tag>`.
- Task spec overrides are currently stored via legacy key `aocPrd`; resolve with `aoc-task spec show <id> --tag <tag>`.
- Effective precedence is task spec override -> tag default spec.
- Keep task specs in git: `.taskmaster/docs/specs/**` and legacy `.taskmaster/docs/prds/**` should always be tracked.
- Keep AOC/task/config/source state in git: `.aoc/**`, `.taskmaster/**`, `.omp/extensions/**`, `.omp/agents/**`, `.omp/skills/**`, `AGENTS.md`, and `DESIGN.md` should not be ignored except explicit high-churn runtime artifacts such as logs, locks, caches, and debug output.

## Task Management
- `.taskmaster/tasks/tasks.json` is task state; use the Taskmaster TUI, `aoc-task`, or `tm` (alias for `aoc-task`). Do not edit the file directly.

## Skills (load when needed)
Load a skill only when its description matches the user request. Keep always-on guidance here minimal; procedural playbooks belong in skill files.

Note: `tm` is a basic CLI command (see above) and needs no skill.
