---
name: animejs-scene-planner
description: Turn vague motion requests into an Anime.js-ready blueprint with intent, targets, timing, easing, reduced-motion fallback, and implementation notes.
---

Use this skill when the user wants **motion direction before code**.

Translate vague requests into:
- Intent
- Visual role of motion
- Targets
- Trigger
- Sequence steps
- Timing ranges
- Easing direction
- Reduced-motion fallback
- Implementation notes

## Planning rules
1. Start with motion purpose, not API calls.
2. Tie each motion beat to hierarchy, comprehension, feedback, or continuity.
3. Prefer a small number of focal events over many simultaneous ones.
4. Keep the plan implementable with Anime.js primitives the team can actually ship.
5. If scroll is involved, explicitly decide whether playback should be threshold-triggered or scroll-synchronised.
6. If text is involved, decide whether words, chars, or lines should be split.
7. If SVG is involved, decide whether the motion is line-draw, morph, path-follow, or transform-only.

## Output format
- Intent
- Visual role of motion
- Targets
- Trigger
- Sequence steps
- Timing
- Easing
- Reduced-motion fallback
- Anime.js implementation path
- Risks / watchouts

## Implementation path guidance
When the plan is ready, route to the narrowest specialist:
- generic implementation → `animejs-core-api`
- sequencing heavy → `animejs-timelines`
- scroll-bound → `animejs-scroll-interaction`
- SVG/path/morph → `animejs-svg-motion`
- text reveal / split words/chars/lines → `animejs-text-splitting`
- React / component lifecycle → `animejs-react-integration`
- perf/a11y hardening → `animejs-performance-a11y`
- existing code audit → `animejs-reviewer`

## Anti-patterns to call out
- “premium” motion with no product role
- too many simultaneous entrances
- long entrances that delay usability
- text effects that hurt reading
- scroll motion that competes with reading
- morphing or SVG spectacle without a semantic reason
- specifying extreme springiness without acknowledging the tone risk

Reference official notes in `../animejs-core-api/references/official-docs-map.md` when needed.
