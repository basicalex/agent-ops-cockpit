---
name: teach-workflow
description: DEPRECATED legacy teach-mode scans and local insight logging. Prefer aoc-understand for repository understanding, onboarding, architecture questions, and graph/dashboard flows.
---

# Deprecated: teach-workflow

This skill is deprecated.

Use `aoc-understand` / Understand-Anything for new repository-understanding work:

```bash
aoc-understand status
aoc-understand analyze --full
aoc-understand dashboard --open
aoc-understand chat "How does this subsystem work?"
aoc-understand explain path/to/file
```

## Legacy scope

The old teach workflow used `/teach-full`, `/teach-dive`, and `/teach-ask` to produce local Markdown notes under `.aoc/insight/`. Those artifacts may still be inspected when explicitly requested, but they are no longer the canonical understanding system.

## Guardrails

- Do not start new architecture/onboarding scans with teach unless the operator explicitly asks for legacy teach behavior.
- Do not delete `.aoc/insight/`; it may contain historical local notes.
- Do not edit `.aoc/memory.md` or `.taskmaster/tasks/tasks.json` directly.
