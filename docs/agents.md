# Agents (PI-first)

AOC now runs in **PI-only mode** and standardizes on **PI prompt templates** for agent personas and workflows.

## PI Prompt Templates
`aoc-init` seeds project-local PI templates into:

```
.pi/prompts/
```

Seeded templates:
- `/aoc-ops` — AOC setup/ops mode
- `/teach` — repo mentor mode
- `/teach-full` — full architecture scan + checkpoint
- `/teach-dive <subsystem>` — targeted deep dive
- `/teach-ask <question>` — direct answer-only mentor Q&A
- `/tm-cc` — cross-project Taskmaster control mode

`aoc-init` is idempotent and preserves existing prompt files.

Compatibility window note:
- Legacy prompt templates under `.aoc/prompts/pi/` are treated as fallback seed sources only.
- `aoc-init` migrates missing project-local legacy prompts from `.aoc/prompts/pi/` into `.pi/prompts/` and cleans safe `tmcc -> tm-cc` duplicates.
- Canonical PI prompt ownership is `.pi/prompts/`.

## MoreMotion (optional)
Run `aoc-momo init` in a host repo to seed:

```
.pi/prompts/momo.md
```

Use `/momo` for Remotion animation work.

## Runtime support
OpenCode project subagent seeding (`.opencode/agents`, `.opencode/commands`) is removed from the active init path. Non-PI runtime launchers/installers are removed from AOC. PI prompt templates and runtime (`pi`) are the supported path.

Need a non-PI CLI anyway? Use the bring-your-own wrapper path in [Agent Extensibility](agent-extensibility.md). Core support remains PI-only, but extension is intentionally open.

## PI-first migration checklist
1. Run `aoc-init` at repo root (`AOC_INIT_SKIP_BUILD=1 aoc-init` for doc-only migration).
2. Verify canonical runtime paths exist:
   - `.pi/settings.json`
   - `.pi/prompts/tm-cc.md`
   - `.pi/skills/<name>/SKILL.md`
3. Verify control-plane paths remain under `.aoc/` (`context.md`, `memory.md`, `stm/`, `rtk.toml`).
4. Check migration logs for warnings:
   - prompt/skill conflicts are preserved (no overwrite)
   - `tmcc` alias duplicates are cleaned when safe.
5. If needed, run the smoke script: `bash scripts/pi/test-aoc-init-pi-first.sh`.

## PI-only release operations
- Use `docs/pi-only-rollout-checklist.md` for release closeout, user notice timing, and post-release validation steps.

## Rollback quick path
- Keep `.pi/**` as source of truth; do not delete canonical files.
- Revert the migration commit and re-run `aoc-init`.
- If `tmcc` and `tm-cc` both remain with different content, merge manually into `tm-cc.md` and then remove `tmcc.md`.
