---
name: animejs-performance-a11y
description: Audit Anime.js work for performance, reduced-motion behavior, engine settings, scroll readability, SVG costs, and cleanup discipline.
---

Use this skill when the implementation needs hardening.

## Audit priorities
1. reduced-motion support
2. transform/opacity-first motion
3. cleanup discipline
4. frame budget awareness
5. SVG/path cost awareness
6. scroll readability
7. engine-level settings only when justified

## Official facts to use
- `engine.fps` is global and defaults to `120`
- `engine.precision` is global and defaults to `4`
- `engine.pauseOnDocumentHidden` defaults to `true`
- lowering precision can help many-element cases but can visibly harm quality
- `createDrawable()` with `vector-effect: non-scaling-stroke` can be slow
- `splitText()` is responsive/accessibility-aware, but choice of lines/words/chars still matters for legibility

## Output format
- Findings
- Severity
- Why it matters
- Safer Anime.js alternative
- Code fix or config fix
- Validation checklist

## Guidance
- Default to fixing architecture before touching engine globals.
- Use global engine tuning only when there is a demonstrated systemic need.
- Prefer reducing animated targets and simplifying choreography before dropping quality globally.
- Explicitly mention reduced motion for substantial motion systems.
- Call out text readability risk, scroll interference, and lifecycle leaks.

## Anti-patterns
- using engine globals as the first performance fix
- dropping precision aggressively without visual justification
- no reduced-motion path for major motion
- scroll-bound motion that harms reading
- SVG line-draw/morph overuse in dense interfaces
- leaked timelines/animations due to missing revert

Reference official notes in `../animejs-core-api/references/official-docs-map.md`.
