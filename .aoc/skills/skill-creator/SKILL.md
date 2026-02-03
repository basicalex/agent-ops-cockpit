---
name: skill-creator
description: Create or update AOC skills with correct structure and rules.
---

## When to use
Use this when you need to add a new skill or update an existing skill.

## Steps
1. Pick a name: lowercase letters/numbers with single hyphens.
2. Create the directory: `.aoc/skills/<name>/`.
3. Create `SKILL.md` with required frontmatter:

```markdown
---
name: my-skill
description: One-line description of the workflow
---
```

4. Add the workflow instructions below the frontmatter.
5. Run `aoc-skill validate` to confirm naming and frontmatter rules.
6. Sync to the active agent: `aoc-skill sync --agent <agent>`.

## Rules (OpenCode-compatible)
- `name` must match the directory name.
- Allowed frontmatter keys: `name`, `description`, `license`, `compatibility`, `metadata`.
- Description must be 1-1024 characters.

Regex: `^[a-z0-9]+(-[a-z0-9]+)*$`
