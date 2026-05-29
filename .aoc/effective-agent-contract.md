# Effective AOC Agent Contract

Generated from layered AGENTS.md policy. Do not edit this generated output directly; edit source AGENTS.md files instead.

## Sources
- workspace: `~/dev/AGENTS.md` (4897 bytes)
- project: `~/dev/agent-ops-cockpit/AGENTS.md` (6164 bytes)
- precedence: project > workspace > global
- source hash: `f3173af964dd966b`
- raw AGENTS bytes: 11061

## Hard rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- Do not read `.aoc/memory.md`, `.aoc/stm/current.md`, or `.taskmaster/tasks/tasks.json` directly; use AOC CLI commands.
- Use root `DESIGN.md` before UI, docs-site, marketing, HyperFrames, or product-facing work when present.

## Startup and Mind policy
- Use `aoc-handshake --json` as the metadata-only startup packet.
- Do not load broad Mind memories during startup.
- Request focused Mind context only after user intent is known and include an explicit reason.

## Project overrides
- Use root `DESIGN.md` as the visual/product design contract before UI, docs-site, marketing, HyperFrames, or other product-facing work.
- Request focused Mind context only after user intent is known, for resume/continuation, prior decisions, task/spec grounding, debugging previous attempts, provenance/audit, or when targeted local inspection is insufficient.
- If targeted inspection fails, escalate scope gradually and state why.
- If CodeGraph is missing, stale, or uninitialized, report the limitation and fall back to targeted `bash`/`read` inspection.
- Tasks: `tm tag current`, `tm tag spec show`, `aoc-task tag spec show --tag <tag>`, `aoc-task spec show <id> --tag <tag>`
- `DESIGN.md`: project-wide visual/product design contract; subsystem design docs extend it.
- `.taskmaster/docs/specs/`: spec documents linked to tags and tasks; `.taskmaster/docs/prds/` remains legacy-compatible.
- Tag default specs are currently stored via legacy key `aocPrd`; resolve with `aoc-task tag spec show --tag <tag>`.

## Task, memory, and STM commands
- Memory: `aoc-mem read`, `aoc-mem add`, `aoc-mem search`.
- STM handoff layer: `aoc-stm status`, `aoc-stm template`, `aoc-stm resume`, `aoc-stm handoff`, `aoc-stm add` (handoffs only; durable decisions use `aoc-mem`).
- Tasks/specs: `tm list`, `tm show <id>`, `tm add`, `tm sub ...`, `tm tag current`, `tm tag spec show`.
- AOC health: `aoc-init`, `aoc-handshake --json`, `aoc-rtk status`, `aoc-rtk doctor`.

## Lazy-load policy
- Skills are index-only until user intent matches; load a `SKILL.md` only when needed.
- Prompts are registered until invoked; do not inject prompt bodies by default.
- Extension source and themes are not startup context unless the task is about them.
- Task/spec details are loaded on demand; default startup should use active-tag summaries only.

## Response defaults
- Keep responses concise by default.
- Use narrow, path-scoped searches before broad scans.
- Run targeted checks/tests first; escalate only when required.
