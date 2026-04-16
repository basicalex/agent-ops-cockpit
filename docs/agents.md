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
  - `mind-ingest.ts`
  - `mind-ops.ts`
  - `mind-context.ts`
  - `mind-focus.ts`
  - `aoc-models.ts`
  - `lib/mind.ts`
- Optional orchestration assets:
  - `.pi/agents/` (specialists, teams, chain manifests)

Control-plane state remains under `.aoc/**` (`context.md`, `memory.md`, `stm/`, `rtk.toml`).

## Default extensions and theme behavior

AOC now guarantees the baseline PI extensions are present after `aoc-init`:

- `minimal.ts` — default footer/status UX (mind + context meters)
- `themeMap.ts` — extension-to-theme defaults + title behavior
- `mind-ingest.ts` — native Pi→Mind ingest + compaction checkpoints
- `mind-ops.ts` — `/mind`, `/mind-status`, `/aoc-status`, finalize, and operator controls
- `mind-context.ts` — `mind_context_pack` retrieval commands
- `mind-focus.ts` — local focus/task/file inference helpers
- `aoc-models.ts` — legacy OpenRouter bridge migration/status shim; Pi native `/model` + `/scoped-models` own catalog scope
- `lib/mind.ts` — shared Pulse/Mind transport + state helpers

PI auto-discovers `.pi/extensions/*.ts`, so seeded defaults are active after session start (`/reload` if already running).

`aoc-init` seeds a vendored local PI package at `.pi/packages/pi-multi-auth-aoc` and wires `.pi/settings.json` to load it by path, so Codex/OpenRouter multi-auth rotation is part of the baseline AOC environment without relying on a global npm package. Pi now owns the native OpenRouter provider/catalog surface, while the vendored multi-auth package wraps `openrouter` for credential storage, TUI account management, and rotation/failover. The `aoc-models.ts` shim only migrates legacy AOC-managed OpenRouter bridge state out of `~/.pi/agent/models.json` when detected.

## OpenCode Zen + PI model defaults

This repo now seeds project-local PI defaults as follows:

- `defaultProvider: "openai-codex"`
- `defaultModel: "gpt-5.4"`
- `defaultThinkingLevel: "medium"`
- seeded `enabledModels` filter:
  - `openai-codex/gpt-5.4`
  - `opencode/glm-5`
  - `opencode/gemini-3-flash`
  - `opencode/gemini-3.1-pro`
  - `openrouter/anthropic/claude-sonnet-4`
  - `openrouter/openai/gpt-5.1-codex`
  - `openrouter/google/gemini-2.5-pro`
  - `openrouter/google/gemini-2.5-flash`
  - `openrouter/qwen/qwen3.6-plus`

This keeps OpenCode Zen available while also exposing a small curated OpenRouter slice for low-noise model cycling.

Credential handling stays out of the repo:

- set `OPENCODE_API_KEY` in your shell, or
- store an `opencode` API key entry in `~/.pi/agent/auth.json`

When the vendored multi-auth package is active, AOC now bootstraps `OPENCODE_API_KEY` and `OPENROUTER_API_KEY` from the environment into PI auth storage on startup, deduplicates matching keys, and lets multi-auth own rotation state in `~/.pi/agent/multi-auth.json`.

Do **not** commit API keys into `.pi/settings.json`. PI already ships native OpenCode Zen support, so AOC only seeds project defaults and model visibility.

## Pi-native OpenRouter + multi-auth rotation

This repo now treats OpenRouter as a **Pi-native** provider surface:

- extension file: `.pi/extensions/aoc-models.ts` (migration/status shim only)
- provider id: `openrouter`
- vendored multi-auth package: `.pi/packages/pi-multi-auth-aoc`
- credential env var: `OPENROUTER_API_KEY`
- auth storage: PI multi-auth / PI auth storage in `~/.pi/agent/auth.json`
- optional endpoint override: `OPENROUTER_BASE_URL` or `AOC_OPENROUTER_BASE_URL`
- model scope UI: Pi native `/model` and `/scoped-models`
- rotation UI: `/multi-auth`

Multi-auth still wraps `openrouter` so multiple API keys can be added, selected, and rotated from the `/multi-auth` TUI, but provider/model metadata now comes from Pi's native OpenRouter integration first. `~/.pi/agent/models.json` is no longer AOC-owned; any surviving legacy AOC-managed OpenRouter snapshot is backed up and removed automatically.

AOC still seeds a curated OpenRouter slice into `enabledModels`, so Ctrl+P cycling stays intentionally small without replacing Pi's native catalog.

## Prompt templates

Seeded prompt templates:

- `/aoc-ops` — AOC setup/ops mode
- `/teach` — repo mentor mode
- `/teach-full` — full architecture scan + checkpoint
- `/teach-dive <subsystem>` — targeted deep dive
- `/teach-ask <question>` — direct answer-only mentor Q&A
- `/tm-cc` — cross-project Taskmaster control mode

## PI-first migration notes

Canonical ownership is `.pi/**`.

Legacy project-local assets are only used for one-way migration into `.pi/**` when present:

- `.aoc/prompts/pi/` -> `.pi/prompts/` (missing files only)
- `.aoc/skills/` -> `.pi/skills/` (missing files only)
- safe alias cleanup: `.pi/prompts/tmcc.md` -> `.pi/prompts/tm-cc.md`

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
   - `.pi/extensions/mind-ingest.ts`
   - `.pi/extensions/mind-ops.ts`
   - `.pi/extensions/mind-context.ts`
   - `.pi/extensions/mind-focus.ts`
   - `.pi/extensions/aoc-models.ts`
   - `.pi/extensions/lib/mind.ts`
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
.pi/skills/moremotion/
.pi/prompts/momo.md
```

Use `/momo` for Remotion animation work.

## PI-only release operations

Use `docs/pi-only-rollout-checklist.md` for release closeout and post-release checks.

## Rollback quick path

- Keep `.pi/**` as source of truth; do not delete canonical files.
- Revert the migration commit and re-run `aoc-init`.
- If `tmcc` and `tm-cc` both remain with different content, merge manually into `tm-cc.md` and remove `tmcc.md`.
