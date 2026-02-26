# Agent Skills

## Overview
Skills are reusable workflow playbooks stored in `.pi/skills/<name>/SKILL.md` (PI-first canonical path).

Task PRD workflows use task-level links (`aocPrd`) and PRD docs under `.taskmaster/docs/prds/`.

## Sync behavior (PI-first)
- `aoc-init` seeds default PI skills into `.pi/skills` (if missing) and syncs PI skills only.
- `aoc-agent --set pi` and `aoc-agent --run pi` re-sync PI skills before launch.
- Manual sync:

```bash
aoc-skill sync --agent pi
```

Sync is additive: existing skills in target directories are preserved. If a name collision exists, AOC skips that skill and logs a warning.

## Compatibility window
- Legacy `.aoc/skills` remains a fallback **source** only while migration is in progress.
- `aoc-init` migrates missing project-local legacy skills from `.aoc/skills` to `.pi/skills` (non-destructive).
- Non-PI skill targets are no longer auto-synced by `aoc-init`.
- In PI-only mode, use `aoc-skill sync --agent pi` as the canonical sync path.

## Skill format
Each `SKILL.md` must include YAML frontmatter with the required fields:

```markdown
---
name: my-skill
description: One-line description of the workflow
---
```

Naming rules:
- Lowercase letters, numbers, and single hyphens
- 1-64 characters
- Must match the directory name

Regex: `^[a-z0-9]+(-[a-z0-9]+)*$`

## Validation
Run this to validate skill frontmatter and naming:

```bash
aoc-skill validate
```

## Built-in skills
- `aoc-workflow`
- `teach-workflow`
- `aoc-init-ops`
- `memory-ops`
- `stm-ops`
- `taskmaster-ops`
- `tm-cc`
- `rlm-analysis`
- `prd-dev`
- `prd-intake`
- `prd-align`
- `tag-align`
- `task-breakdown`
- `task-checker`
- `release-notes`
- `skill-creator`
- `zellij-theme-ops`

`zellij-theme-ops` pairs with the `aoc-theme` CLI for global theme workflows (`~/.config/zellij/themes`), including `aoc-theme tui` for interactive selection.

## Optional skills
Use `aoc-momo init` to add:

- `moremotion` (Remotion integration for React projects)
