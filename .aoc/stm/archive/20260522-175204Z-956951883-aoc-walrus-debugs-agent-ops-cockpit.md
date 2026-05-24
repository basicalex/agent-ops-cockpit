# AOC Directed STM Handoff

<!-- aoc-stm: v2 -->

## Metadata
- Created: 2026-05-22T17:52:04Z
- Project: /home/ceii/dev/agent-ops-cockpit
- Task/spec: tag:env-protec
- Purpose: continue
- Intended next agent/session: next-agent/session
- Operator focus: focusing on the skills architecture and components of the related system
- Source session: aoc-walrus-debugs
- Packet source: external packet file: /tmp/aoc-handoff-skills-presets.0jlLW0.md
- Archive: 20260522-175204Z-956951883-aoc-walrus-debugs-agent-ops-cockpit.md
- Delivery note: STM stores this packet locally; it does not notify another agent by itself. Pass the resume brief or exact archive to the next agent/session.

## Next-Agent Resume Brief
```text
You are receiving a directed AOC STM handoff.
Purpose: continue
Recipient: next-agent/session
Task/spec: tag:env-protec
Focus: focusing on the skills architecture and components of the related system

Read this exact packet with:
aoc-stm resume '20260522-175204Z-956951883-aoc-walrus-debugs-agent-ops-cockpit.md'

Important: STM is a local packet store, not a mailbox or durable memory. Treat this as an operator-provided continuation brief, verify claims against repository state, and use aoc-mem for durable decisions.
```

## Handoff Packet

# Directed Handoff — Preset/Skill Architecture

## Direction
- Purpose: continue
- Recipient: next-agent/session
- Operator focus: focusing on the skills architecture and components of the related system
- Active Taskmaster tag: `env-protec`
- Scope: AOC preset runtime, preset manifests/components, Pi skill visibility filters, default skill bloat reduction, and docs/init propagation.

## Current status
- Done/partial: Implemented leaner preset/skill routing and added focused dashboard/test capabilities.
- Not complete: Repo has many unrelated pre-existing/parallel dirty files; do not assume all `git status` changes belong to this work.
- Blocked: Not blocked, but next session should review/commit carefully due broad dirty worktree.

## Touched files/areas and why they matter
- `.aoc/presets/design/preset.toml`: trimmed design base active skills; added `dashboard` mode; dashboard guardrail is mode-scoped.
- `.aoc/presets/design/components/mode-dashboard.md`: dashboard-mode prompt component for dense admin/dev-tool/dashboard UX.
- `.pi/skills/enforce-dashboard-ux-guardrails/SKILL.md`: new kebab-case Pi skill with dashboard UX guardrails.
- `.pi/extensions/aoc-presets/skill-filters.ts`: managed skill filters now keep browser/design/media/obsolete skills hidden by default and expose them only via preset state.
- `.pi/settings.json`: current project lean default skill filters.
- `.pi/extensions/aoc-presets/commands.ts`: preset menu changed so umbrella appears once; Enter selects default, `l`/right opens modes. Design command accepts premium/funnel/dashboard.
- `.aoc/presets/test/`: new test preset with verify/browser/regression/preview modes.
- `bin/aoc-init`: future project seeding updated for lean defaults, dashboard skill, and test preset.
- `docs/presets.md`: docs updated for new routing/menu/test preset.

## Changes made / relevant evidence
- Design umbrella default `critique` now loads components `core`, `mode-critique`; active skills `frontend-design`, `architecture-design`, `design-director`; recommended `design-review`.
- Dashboard guardrail is hidden by default and active only in design `dashboard` mode.
- Default visible project skills reduced to `aoc-init-ops`, `aoc-map`, `rlm-analysis`, `spec-rpg-authoring`, `tm-cc`, `vercel-cli`, `web-research`.
- Hidden by default includes `agent-browser`, `custom-layout-ops`, `zellij-theme-ops`, design skills, dashboard guardrail, motion/anime/hyperframes helpers.
- New test preset default `verify` loads components `hook`, `core`, `mode-verify`; active `architecture-design`, `agent-browser`; recommended `rlm-analysis`.

## Validation commands and results
- `aoc-skill validate` → passed with 0 warnings.
- `bash scripts/pi/test-aoc-presets.sh` → passed: `OK: 5 presets validated; skill filters project-root safe`.
- Inline Python audits confirmed default visible count is 7 and `agent-browser` is hidden by default.

## Open risks / coordination warnings
- Worktree is very dirty with 100+ modified files and multiple untracked files unrelated to this focused work. Stage only scoped files.
- Existing `.aoc/stm/current.md` was stale/noisy and newer than latest archive; this packet was sealed from an external file and should not depend on clearing current draft.
- Scoped diff also shows pre-existing managed asset/hook changes in design/hyperframes and broader `bin/aoc-init` changes; review hunks carefully.
- Test preset has no `/test-director` slash command yet; use `/preset test`, `/preset test browser`, menu navigation, etc.

## Next safe actions
1. Review scoped diff only: `.aoc/presets/design/preset.toml`, `.aoc/presets/design/components/mode-dashboard.md`, `.aoc/presets/test/**`, `.pi/skills/enforce-dashboard-ux-guardrails/SKILL.md`, `.pi/extensions/aoc-presets/{commands.ts,skill-filters.ts}`, `.pi/settings.json`, `bin/aoc-init`, `docs/presets.md`.
2. Decide whether a `/test-director` command is needed.
3. Re-run `aoc-skill validate` and `bash scripts/pi/test-aoc-presets.sh` after any edits.
4. If preparing a commit, avoid unrelated `.aoc/memory.md`, `.taskmaster/tasks/tasks.json`, Mission Control, Mind, Zellij, vendor WASM, etc.

## Do-not-repeat notes
- Visibility exclusions use `!skills/...`; do not add `+skills/...` to defaults.
- Keep dashboard guardrails mode-scoped; do not make them base-active for design.
- Keep `custom-layout-ops` and `zellij-theme-ops` hidden by default due Omarchy theme alignment.
- Do not blindly seal or clear old `.aoc/stm/current.md`.
