---
name: animejs-core-api
description: Anime.js v4 API correctness, imports, targeting, value mapping, refresh/revert semantics, and sane implementation defaults. Use when writing or fixing Anime.js code.
---

Use this skill for **actual Anime.js implementation decisions**, not vague motion ideation.

Prefer official Anime.js v4 patterns:
- main module imports for convenience
- subpath imports for tighter bundles when useful:
  - `animejs/animation`
  - `animejs/timeline`
  - `animejs/utils`
  - `animejs/svg`
  - `animejs/text`
  - `animejs/events`
  - `animejs/scope`
  - `animejs/draggable`
  - `animejs/waapi`

## What to optimize for
1. API correctness
2. transform/opacity-first motion
3. reusable targeting patterns
4. stable cleanup (`revert()`)
5. function-based values only where they add clear value
6. implementation that survives component rerenders/resizes

## Core rules

### 1. Prefer Anime.js v4 naming and module shape
- Use `animate()` for direct animations.
- Use `createTimeline()` for sequencing.
- Use `createScope()` for component-scoped work.
- Use `onScroll()` for scroll-bound playback.
- Use `svg.createMotionPath()`, `svg.createDrawable()`, and `svg.morphTo()` for SVG-specific work.
- Use `splitText()` for text splitting.

### 2. Targeting strategy
Prefer, in this order:
1. explicit refs / DOM elements in component code
2. scoped selectors inside `createScope({ root })`
3. broad global selectors only for static non-component demos

If a user is in React/Vue/component land, do **not** recommend brittle global selectors first.

### 3. Use function-based values deliberately
Anime.js supports function-based values with signature:
`(target, index, length) => value`

Use them for:
- per-item offsets
- reading per-element data attributes
- controlled randomness
- recalculable values with `animation.refresh()`

Do **not** use them just to look clever.

### 4. Use `stagger()` instead of hand-rolling index math when sequencing many targets
Good uses:
- delays
- scale ramps
- position offsets
- timeline staggering

### 5. Cleanup model
Recommend `revert()` whenever the animation lifecycle must fully clean up:
- timeline teardown
- component unmount
- media-query rebuild
- scoped teardown

Use `refresh()` when function-based property values must be recomputed from current state.
Do not confuse `refresh()` with full teardown.

### 6. Utility guidance
Use `utils.get()` to read current values.
Use `utils.set()` for immediate complex state setting, but note:
- repeated updates on same targets are often better modeled with an Animatable pattern than repeated `set()` calls
- `utils.set()` cannot set an attribute that does not already exist on a DOM/SVG element

### 7. Easing guidance
- default easing in WAAPI wrapper land differs from native WAAPI: Anime.js uses `ease`, not `easing`, and defaults to `'out(2)'`
- springs override perceived duration with settling duration
- keep spring bounce values moderate unless the user explicitly wants extreme motion

## Output format
When giving code, use this structure:
1. **Recommendation**
2. **Why this API shape**
3. **Code**
4. **Load-bearing decisions**
5. **Cleanup / resize / rerender notes**

## Strong defaults to recommend
- duration: usually 200–900ms for UI moments unless otherwise justified
- transforms and opacity before layout-affecting properties
- one clear orchestration layer (`animate`, `createTimeline`, or `createScope`) instead of tangled mixtures
- `revert()` on teardown
- `refresh()` only when function-based values genuinely need recomputation

## Anti-patterns to call out
- mixing demo-style global selectors into component code without scoping
- animating layout properties when transform would do
- using random values without a restart/refresh story
- forgetting `revert()` in components
- recommending native WAAPI syntax when the user asked for Anime.js v4
- using old Anime.js API shapes from older versions without calling it out

See `references/official-docs-map.md` for the distilled official v4 research notes.
