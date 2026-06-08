# Effective AOC Agent Contract

Generated from layered AGENTS.md policy. Do not edit this generated output directly; edit source AGENTS.md files instead.

## Sources
- workspace: `~/dev/AGENTS.md` (4897 bytes)
- project: `~/dev/agent-ops-cockpit/AGENTS.md` (9546 bytes)
- precedence: project > workspace > global
- source hash: `174e3998bbecc411`
- raw AGENTS bytes: 14443

## Hard rules
- Use `.aoc/context.md` for orientation; run `aoc-init` if it is missing or stale.
- Do not read `.aoc/memory.md`, `.aoc/stm/current.md`, or `.taskmaster/tasks/tasks.json` directly; use AOC CLI commands.
- Use root `DESIGN.md` before UI, docs-site, marketing, HyperFrames, or product-facing work when present.

## Startup and Mind policy
- Use `aoc-handshake --json` as the metadata-only startup packet, including VCS mode and preferred command family.
- Do not load broad Mind memories during startup.
- Request focused Mind context only after user intent is known and include an explicit reason.
- Prefer `jj` in Jujutsu repositories and Git only in Git-only repositories; do not initialize Jujutsu automatically.

## Project overrides
- `.taskmaster/`: Taskmaster state/specs. Use `tm`/`aoc-task`; do not edit task JSON directly.
- Keep product-facing language aligned with `DESIGN.md`: calm, concise, trustworthy, and terminal-native. Read `DESIGN.md` before UI/docs-presentation/theme/HyperFrames work.
- `DESIGN.md`: product/UI/docs tone and design contract.
- For product/design doc changes touching `DESIGN.md`, run `pnpm run design:lint`.

## Task, memory, and STM commands
- Memory: `aoc-mem read`, `aoc-mem add`, `aoc-mem search`.
- STM handoff layer: `aoc-stm status`, `aoc-stm template`, `aoc-stm resume`, `aoc-stm handoff`, `aoc-stm add` (handoffs only; durable decisions use `aoc-mem`).
- Tasks/specs: `tm list`, `tm show <id>`, `tm add`, `tm sub ...`, `tm tag current`, `tm tag spec show`.
- AOC health: `aoc-init`, `aoc-handshake --json`, `aoc-rtk status`, `aoc-rtk doctor`.
- VCS: inspect mode with `aoc-handshake --json`; use `jj status`/`jj diff` for Jujutsu and Git commands for Git-only repositories.

## Lazy-load policy
- Skills are index-only until user intent matches; load a `SKILL.md` only when needed.
- Prompts are registered until invoked; do not inject prompt bodies by default.
- Extension source and themes are not startup context unless the task is about them.
- Task/spec details are loaded on demand; default startup should use active-tag summaries only.

## Response defaults
- Keep responses concise by default.
- Use narrow, path-scoped searches before broad scans.
- Run targeted checks/tests first; escalate only when required.
