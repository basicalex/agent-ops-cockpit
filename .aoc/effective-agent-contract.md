# Effective AOC Agent Contract

Generated from layered AGENTS.md policy. Do not edit this generated output directly; edit source AGENTS.md files instead.

## Sources
- global: `~/AGENTS.md` (7861 bytes)
- workspace: `~/dev/AGENTS.md` (6861 bytes)
- project: `~/dev/agent-ops-cockpit/AGENTS.md` (4668 bytes)
- precedence: project > workspace > global
- source hash: `76c1a27a5622c118`
- raw AGENTS bytes: 19390

## Hard rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- Do not read `.taskmaster/tasks/tasks.json` directly; use Taskmaster CLI commands.
- Use root `DESIGN.md` before UI, docs-site, marketing, HyperFrames, or product-facing work when present.

## Startup policy
- Use `aoc-handshake --json` as the metadata-only startup packet, including Git repository state.

## Project overrides
- Use root `DESIGN.md` as the visual/product design contract before UI, docs-site, marketing, HyperFrames, or other product-facing work.
- If targeted inspection fails, escalate scope gradually and state why.
- Tasks: `tm tag current`, `tm tag spec show`, `aoc-task tag spec show --tag <tag>`, `aoc-task spec show <id> --tag <tag>`
- VCS: inspect detected mode with `aoc-handshake --json`; use `git status`/`git diff` in Git repositories.
- `DESIGN.md`: project-wide visual/product design contract; subsystem design docs extend it.
- `.taskmaster/docs/specs/`: spec documents linked to tags and tasks; `.taskmaster/docs/prds/` remains legacy-compatible.
- Tag default specs are currently stored via legacy key `aocPrd`; resolve with `aoc-task tag spec show --tag <tag>`.
- Task spec overrides are currently stored via legacy key `aocPrd`; resolve with `aoc-task spec show <id> --tag <tag>`.

## Lightweight validation
- Prefer OMP `lsp diagnostics` on touched files/globs for edit-loop validation before running build, lint, or typecheck commands.
- Use `lsp references` before changing exported symbols, and `lsp code_actions` for language-server fixes/imports when available.
- Do not run full project build/lint/test as a routine sanity check during active edits.
- In delegated multi-agent work, subagents should skip project-wide validation unless explicitly assigned.
- Final verification is still required; choose the smallest targeted command that proves the changed behavior.

## Task and project commands
- Tasks/specs: `tm list`, `tm show <id>`, `tm add`, `tm sub ...`, `tm tag current`, `tm tag spec show`.
- AOC health: `aoc-init`, `aoc-handshake --json`, `aoc-rtk status`, `aoc-rtk doctor`.
- VCS: inspect mode with `aoc-handshake --json`; use Git commands for Git repositories.

## Lazy-load policy
- Skills are index-only until user intent matches; load a `SKILL.md` only when needed.
- Prompts are registered until invoked; do not inject prompt bodies by default.
- Extension source and themes are not startup context unless the task is about them.
- Task/spec details are loaded on demand; default startup should use active-tag summaries only.

## Response defaults
- Keep responses concise by default.
- Use narrow, path-scoped searches before broad scans.
- Run targeted checks/tests first; escalate only when required.
