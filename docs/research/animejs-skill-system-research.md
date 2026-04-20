# Anime.js Skill System Research

Date: 2026-04-20

## Goal
Build a high-fidelity Anime.js skill system for Pi/AOC design agents, grounded in official Anime.js v4 documentation rather than generic animation folklore.

## Research method
- search-first research using `aoc-search`
- focused extraction from official Anime.js docs
- synthesis into specialist skills aligned to implementation domains

## Most important official findings

### 1. Anime.js v4 is strongly modules-first
Anime.js explicitly supports importing from the main module or granular subpaths.
That matters for code recommendations because bundle-aware and no-bundler paths are both first-class.

### 2. React integration is not “just use a selector in useEffect”
Official docs recommend:
- `createScope({ root })`
- `scope.add(...)`
- `scope.current.revert()` on cleanup
- method exposure via `self.add()`

That means our React skill must be lifecycle-aware, not just syntax-aware.

### 3. Sequencing has a real composition model
Official timeline docs distinguish:
- `add()` for direct composition into a timeline
- `sync()` for existing animations/timelines

That distinction is important enough to deserve its own specialist skill.

### 4. Scroll choreography is richer than a trigger-once pattern
Official `onScroll()` docs expose:
- thresholds (`enter`, `leave`)
- sync modes
- callbacks
- container/target ownership

So scroll motion should be treated as a specialist domain, not a footnote.

### 5. SVG motion has three materially different primitives
Official docs separate:
- `morphTo()`
- `createDrawable()`
- `createMotionPath()`

These are different enough that conflating them leads to poor implementation guidance.

### 6. Text splitting has lifecycle and accessibility implications
Official docs position `splitText()` as responsive and accessible, with methods such as:
- `revert()`
- `refresh()`
- `addEffect()`

That means text motion should not be treated as a generic “just stagger chars” recipe.

### 7. Engine controls exist, but they are global levers
Official engine docs show:
- `engine.fps`
- `engine.precision`
- `engine.pauseOnDocumentHidden`

These belong in a performance hardening skill, not in routine animation snippets.

## Resulting skill architecture
Created / recommended specialist skills:
- `animejs-scene-planner`
- `animejs-core-api`
- `animejs-timelines`
- `animejs-scroll-interaction`
- `animejs-svg-motion`
- `animejs-text-splitting`
- `animejs-react-integration`
- `animejs-performance-a11y`
- `animejs-reviewer`

## Why this split is strong
It mirrors the actual complexity boundaries in the official docs:
- planning
- API correctness
- orchestration
- scroll systems
- SVG systems
- text systems
- React lifecycle
- performance/a11y
- auditing/refactoring

## Official docs used
See:
- `.pi/skills/animejs-core-api/references/official-docs-map.md`

That file includes the distilled source list and the primary official conclusions.
