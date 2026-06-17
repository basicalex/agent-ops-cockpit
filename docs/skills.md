# Skills

## Overview

Skills are reusable workflow playbooks stored in `.omp/skills/<name>/SKILL.md`.

Task spec workflows use task-level links (legacy key `aocPrd`) and specs under `.taskmaster/docs/specs/`; `.taskmaster/docs/prds/` remains legacy-compatible.

## Sync behavior

- `aoc-init` seeds/repairs `.omp/skills` and installs those skills into `${AOC_OMP_AGENT_DIR:-~/.omp/agent}/skills`.
- Manual sync/validation:

```bash
aoc-skill sync --root .
aoc-skill validate --root .
```

`.omp/skills` is the only canonical project skill source. Legacy Pi skill paths are not active runtime evidence.
