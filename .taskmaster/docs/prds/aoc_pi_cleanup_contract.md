# PI-First Ownership Contract (Task 121)

Status: **approved for implementation**  
Date: **2026-02-25**  
Scope: `aoc/pi_cleanup`

## 1) Contract Summary

AOC in PI-first mode uses a strict ownership split:

- **`.aoc/` is control plane only** (context, memory, STM, RTK policy, shared layouts, task metadata links).
- **`.pi/` is canonical for PI runtime assets** (settings, prompts, skills, extensions).

No PI runtime asset class may be canonical in both locations.

## 2) Canonical Ownership Map

| Asset class | Canonical path | Non-canonical / legacy paths | Rule |
|---|---|---|---|
| Project context | `.aoc/context.md` | none | `aoc-init` regenerates; keep in `.aoc`. |
| Long-term memory | `.aoc/memory.md` | none | Access via `aoc-mem`; never relocate. |
| Short-term memory | `.aoc/stm/**` | none | Access via `aoc-stm`; never relocate. |
| RTK project policy | `.aoc/rtk.toml` | none | Keep control-plane scoped. |
| Shared AOC layouts | `.aoc/layouts/**` | none | Keep in `.aoc`. |
| PI settings | `.pi/settings.json` | none | Required baseline file. |
| PI prompts | `.pi/prompts/*.md` | `.aoc/prompts/pi/*.md` (legacy source only during compatibility window) | Runtime + operator docs must point to `.pi/prompts`. |
| PI skills | `.pi/skills/<name>/SKILL.md` | `.aoc/skills/**` (legacy source/bridge only during compatibility window) | Runtime ownership is `.pi/skills`. |
| PI extensions | `.pi/extensions/**` | none | Keep PI runtime assets under `.pi`. |
| Task PRDs | `.taskmaster/docs/prds/**` | none | Track in git; linked via `aocPrd`. |

## 3) Invariants (Must Always Hold)

1. **Single owner per asset class**: every PI runtime artifact resolves to one canonical path in `.pi`.
2. **Idempotent init/repair**: `aoc-init` creates missing files but does not destructively overwrite existing canonical user files.
3. **No control-plane drift**: `.aoc/` stays limited to AOC orchestration/state files.
4. **Convergent migration**: one `aoc-init` run must move existing repos closer to target state.
5. **Deterministic alias surface**: canonical alias is `tm-cc`; legacy `tmcc` artifacts are deprecated and cleaned by migration logic.

## 4) Compatibility Window

Compatibility window spans implementation tasks **[122] -> [126]** and closes with rollout/docs task **[128]**.

During the window:

- `aoc-init` may read legacy seed/source locations (`.aoc/prompts/pi`, `.aoc/skills`) for backward compatibility.
- Writes for PI runtime assets must target `.pi/**` only.
- Legacy aliases/duplicates (`tmcc`) are removed when safe and non-destructive.
- Legacy paths may remain present temporarily but are treated as deprecated and non-canonical.

Window closes when:

- migration logic for existing repos is shipped (task 126),
- fresh/existing repo smoke tests pass (task 127), and
- release checklist + migration notes are published (task 128).

After closure:

- `.aoc/prompts/pi/**` and `.aoc/skills/**` are no longer canonical for PI runtime behavior.
- PI docs/prompts/skills guidance must reference `.pi/**` only.

## 5) Rollback Criteria + Plan

Rollback trigger (any one is sufficient):

1. `aoc-init` overwrites user-modified canonical files in `.pi/**`.
2. Fresh init fails to seed required `.pi` baseline (`settings.json`, prompts, skills).
3. Existing repo migration leaves conflicting canonical ownership with no deterministic precedence.

Rollback plan:

- Revert PI-first changeset(s) to last known good revision.
- Preserve user `.pi/**` content.
- Re-run prior stable `aoc-init` path and restore compatibility bridge behavior.
- Keep deprecation cleanup disabled until safety checks are restored.

## 6) Implementation Handoff

- Task 122: enforce deterministic `.pi` baseline seeding.
- Task 123: canonicalize prompt seeding to `.pi/prompts` and enforce alias hygiene.
- Task 124: move skill ownership toward `.pi/skills` and reduce bridge complexity.
- Task 125: remove non-PI scaffolding/docs from active path.
- Task 126: existing repo auto-migration and compatibility cleanup.
- Task 127: fresh + migration smoke tests.
- Task 128: release notes, operator migration checklist, compatibility window closeout.
