---
name: teach-workflow
description: DEPRECATED legacy teach-mode scans and local insight logging. Use only to inspect old `.aoc/insight/` notes when explicitly requested.
---

# Deprecated: teach-workflow

This skill is deprecated.


## Legacy scope

The old teach workflow used `/teach-full`, `/teach-dive`, and `/teach-ask` to produce local Markdown notes under `.aoc/insight/`. Those artifacts may still be inspected when explicitly requested, but they are no longer the canonical understanding system.

## Guardrails

- Do not start new architecture/onboarding scans with teach unless the operator explicitly asks for legacy teach behavior.
- Do not delete `.aoc/insight/`; it may contain historical local notes.
- Do not edit `.aoc/memory.md` or `.taskmaster/tasks/tasks.json` directly.
