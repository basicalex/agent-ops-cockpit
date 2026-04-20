---
name: animejs-svg-motion
description: Implement Anime.js SVG morphing, line-drawing, and motion-path animation using morphTo(), createDrawable(), and createMotionPath().
---

Use this skill when the task is specifically SVG-centric.

## Official primitives
- `svg.morphTo(target, precision)`
- `svg.createDrawable(target)`
- `svg.createMotionPath(path, offset)`

## Choose the right SVG technique
- **morphTo** → shape-to-shape morphing (`path d`, `polyline/polygon points`)
- **createDrawable** → line draw / reveal using the synthetic `draw` property
- **createMotionPath** → move an element along a path with `translateX`, `translateY`, `rotate`

## Important official caveat
Animating drawable SVG with `vector-effect: non-scaling-stroke` can be slow because scale factor recalculation may happen every tick.

## Output format
- SVG motion type
- Why this primitive fits
- Target/path setup
- Precision / offset choices
- Code
- Performance and cleanup notes

## Guidance
- Use morphing only when the shape relationship feels intentional.
- Keep precision moderate unless visual fidelity clearly demands more points.
- For motion-path animation, confirm whether auto-rotation is desirable before applying path-based rotation blindly.
- For line drawing, define whether the reveal should be full-path, segment-based, or looping.
- Prefer transform-driven SVG motion when path morphing is unnecessary.

## Anti-patterns
- morphing unrelated shapes just for novelty
- over-precise morphing without a visual payoff
- path-follow rotation that makes logos/icons unreadable
- expensive line-draw effects with no product meaning
- ignoring the non-scaling-stroke perf caveat

Reference official SVG notes in `../animejs-core-api/references/official-docs-map.md`.
