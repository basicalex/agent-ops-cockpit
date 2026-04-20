---
name: animejs-timelines
description: Design and implement Anime.js timeline sequencing, labels, relative offsets, add vs sync decisions, and multi-segment choreography.
---

Use this skill when sequencing is the hard part.

## Official-model rules
- Create timelines with `createTimeline()`.
- Use `.add()` when creating and adding a new animation directly into the timeline.
- Use `.sync()` when synchronising an existing animation or timeline into another one.
- Use labels and relative offsets instead of hardcoding every absolute time.
- Use `.revert()` for full teardown when the timeline lifecycle ends.

## What to optimize for
1. readable orchestration
2. stable composition
3. easy retiming
4. minimal duplication
5. clean teardown

## Output format
- Choreography summary
- Timeline structure
- Why `add()` vs `sync()`
- Labels / offsets
- Code
- Cleanup / replay notes

## Guidance
- Prefer 1–3 named labels for larger sequences.
- Use relative offsets like `'<-=500'` only when they improve readability.
- Break giant hero motion into sub-sequences when it improves maintenance.
- If a motion scene already exists as a standalone animation, prefer `sync()` rather than rewriting it inline.
- If the user needs responsive or component-safe rebuilding, mention pairing the timeline with `createScope()` or `scope.keepTime()`.

## Anti-patterns
- magic-number timeline positions everywhere
- a single giant timeline with no labels or semantic grouping
- mixing unrelated concerns into one timeline
- forgetting teardown via `revert()` in component contexts
- rebuilding timelines repeatedly without a lifecycle story

Use official behavior from `../animejs-core-api/references/official-docs-map.md`.
