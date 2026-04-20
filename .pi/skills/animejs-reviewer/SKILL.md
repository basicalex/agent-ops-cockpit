---
name: animejs-reviewer
description: Review existing Anime.js code for API correctness, sequencing quality, React lifecycle safety, cleanup, performance, and motion purpose.
---

Use this skill when code already exists and needs critique or refactoring guidance.

## Review checklist
Audit for:
- correct v4 APIs and imports
- target selection strategy
- timeline structure (`add` vs `sync`)
- scroll model correctness (`onScroll` thresholds/sync)
- text split lifecycle (`refresh` / `revert`)
- SVG primitive choice (`morphTo`, `createDrawable`, `createMotionPath`)
- React/component safety (`createScope`, cleanup, methods)
- performance and a11y
- motion purpose vs noise

## Output format
- What is solid
- What is risky
- What is incorrect
- Refactor priorities
- Proposed replacement structure
- If useful: corrected code

## Refactor priorities
1. wrong API / wrong lifecycle
2. cleanup leaks
3. motion that hurts UX
4. performance hotspots
5. readability / maintainability

## Hard-call issues to flag
- old Anime.js patterns presented as current v4 usage
- global selectors in component code without scoping
- no `revert()` on teardown
- random/function-based values without `refresh()` story
- giant unlabeled timelines
- scroll sync used where simple threshold triggers would be clearer
- char-splitting that damages readability
- path morphing used without a meaningful shape relationship

Reference official notes in `../animejs-core-api/references/official-docs-map.md`.
