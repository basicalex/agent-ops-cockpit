# Agent Skills

## Overview
Skills are reusable workflow playbooks stored in `.aoc/skills/<name>/SKILL.md`. AOC syncs skills only for the active agent to avoid repo bloat.

Task PRD workflows use task-level links (`aocPrd`) and PRD docs under `.taskmaster/docs/prds/`.

## Sync behavior
- `aoc-agent --set <agent>` syncs skills for that agent.
- `aoc-agent --run <agent>` and `aoc-agent-run` re-sync before launch.
- `aoc-init` seeds default skills (if missing), syncs the active agent, and re-syncs existing targets.

Sync is additive: existing skills in agent directories are preserved. If a name collision exists, AOC skips that skill and logs a warning.

Manual commands:

```bash
# Sync skills for one agent
aoc-skill sync --agent oc

# Re-sync existing targets only
aoc-skill sync --existing
```

## Supported agents
- Codex
- Claude Code
- OpenCode
- Kimi

## Sync targets
- Codex: `.codex/skills/<name>/SKILL.md`
- Claude Code: `.claude/skills/<name>/SKILL.md`
- OpenCode: `.opencode/skills/<name>/SKILL.md`
- Kimi: `.agents/skills/<name>/SKILL.md`

Skills are symlinked to the canonical `.aoc/skills` definitions. Custom skills per repo can be added directly under `.aoc/skills/<name>/SKILL.md`.

## Skill format
Each `SKILL.md` must include YAML frontmatter with the required fields:

```markdown
---
name: my-skill
description: One-line description of the workflow
---
```

Naming rules (OpenCode-compatible):
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
- `aoc-init-ops`
- `memory-ops`
- `stm-ops`
- `taskmaster-ops`
- `rlm-analysis`
- `prd-dev`
- `prd-align`
- `tag-align`
- `task-breakdown`
- `task-checker`
- `release-notes`
- `skill-creator`

## Optional skills
Use `aoc-momo init` to add:

- `moremotion` (Remotion integration for React projects)
