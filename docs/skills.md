# Skills

## Overview

Skills are reusable workflow playbooks stored in `.omp/skills/<name>/SKILL.md`.

Task spec workflows use task-level links (legacy key `aocPrd`) and specs under `.taskmaster/docs/specs/`; `.taskmaster/docs/prds/` remains legacy-compatible.

## Sync behavior

- `aoc-init` seeds/repairs `.omp/skills`; `.omp/manifest.toml` remains the full canonical skill inventory plus profile tables, and active profiles decide which skills install into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}/skills`.
- Manual sync/validation:

```bash
aoc-skill sync --root .
aoc-skill validate --root .
```

`.omp/skills` is the source for skill bodies. `.omp/manifest.toml` profiles decide which canonical skills are active in the runtime install surface. Legacy Pi skill paths are not active runtime evidence.
