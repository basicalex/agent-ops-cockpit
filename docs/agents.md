# Agents (PI-first)

AOC runs in **PI-only mode** and standardizes on a single project-local runtime surface under `.pi/**`.

## AOC PI runtime contract (seeded by `aoc-init`)

`aoc-init` is the canonical setup/repair command and is idempotent (existing files are preserved).

Expected project-local runtime paths:

- `.pi/settings.json`
- `.pi/prompts/`
  - `aoc-ops.md`
  - `teach.md`
  - `teach-full.md`
  - `teach-dive.md`
  - `teach-ask.md`
  - `tm-cc.md`
- `.pi/skills/<name>/SKILL.md` (baseline PI skills)
- `.pi/extensions/`
  - `minimal.ts`
  - `themeMap.ts`
- Optional orchestration assets:
  - `.pi/agents/` (specialists, teams, chain manifests)

Control-plane state remains under `.aoc/**` (`context.md`, `memory.md`, `stm/`, `rtk.toml`).

## Default extensions and theme behavior

AOC now guarantees the two baseline PI extensions are present after `aoc-init`:

- `minimal.ts` — default footer/status UX (mind + context meters)
- `themeMap.ts` — extension-to-theme defaults + title behavior

PI auto-discovers `.pi/extensions/*.ts`, so seeded defaults are active after session start (`/reload` if already running).

## Prompt templates

Seeded prompt templates:

- `/aoc-ops` — AOC setup/ops mode
- `/teach` — repo mentor mode
- `/teach-full` — full architecture scan + checkpoint
- `/teach-dive <subsystem>` — targeted deep dive
- `/teach-ask <question>` — direct answer-only mentor Q&A
- `/tm-cc` — cross-project Taskmaster control mode

## Compatibility window

Legacy sources remain migration fallbacks only:

- `.aoc/prompts/pi/` -> `.pi/prompts/` (missing files only)
- `.aoc/skills/` -> `.pi/skills/` (missing files only)
- safe alias cleanup: `.pi/prompts/tmcc.md` -> `.pi/prompts/tm-cc.md`

Canonical ownership is `.pi/**`.

## Runtime support boundary

- Core-supported runtime: `pi`
- Non-PI wrappers/installers are removed from active AOC paths
- Bring-your-own runtime remains possible via wrapper strategy in [Agent Extensibility](agent-extensibility.md)

See also: [Deprecations and removals](deprecations.md), [Insight sub-agent orchestration](insight-subagent-orchestration.md).

## PI-first migration checklist

1. Run `aoc-init` at repo root (`AOC_INIT_SKIP_BUILD=1 aoc-init` for doc-only migration).
2. Verify canonical runtime paths exist:
   - `.pi/settings.json`
   - `.pi/prompts/tm-cc.md`
   - `.pi/skills/<name>/SKILL.md`
   - `.pi/extensions/minimal.ts`
   - `.pi/extensions/themeMap.ts`
3. Verify control-plane paths under `.aoc/` (`context.md`, `memory.md`, `stm/`, `rtk.toml`).
4. Check migration logs for warnings:
   - prompt/skill conflicts are preserved (no overwrite)
   - `tmcc` alias duplicates are cleaned when safe
5. Run smoke validation if needed:

```bash
bash scripts/pi/test-aoc-init-pi-first.sh
bash scripts/pi/test-pi-only-agent-surface.sh
```

## MoreMotion (optional)

Run `aoc-momo init` in a host repo to seed:

```
.pi/prompts/momo.md
```

Use `/momo` for Remotion animation work.

## PI-only release operations

Use `docs/pi-only-rollout-checklist.md` for release closeout and post-release checks.

## Rollback quick path

- Keep `.pi/**` as source of truth; do not delete canonical files.
- Revert the migration commit and re-run `aoc-init`.
- If `tmcc` and `tm-cc` both remain with different content, merge manually into `tm-cc.md` and remove `tmcc.md`.
