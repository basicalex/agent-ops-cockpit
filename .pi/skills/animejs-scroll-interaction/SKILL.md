---
name: animejs-scroll-interaction
description: Build Anime.js scroll-triggered and scroll-synchronised motion with onScroll(), thresholds, sync modes, targets, containers, and callback strategy.
---

Use this skill when motion depends on scroll.

## Official-model rules
Anime.js `onScroll()` can drive timers, animations, and timelines.
Key parameters:
- `container`
- `target`
- `enter`
- `leave`
- `sync`
- `debug`
- callbacks like `onEnter`, `onLeave`, `onUpdate`, `onSyncComplete`

Official threshold defaults:
- enter: `'end start'`
- leave: `'start end'`

## Decision framework
First decide which of these the interaction is:
1. **threshold-triggered** — play when section enters / leaves
2. **progress-synchronised** — animation progress maps to scroll progress
3. **smoothed / eased sync** — follows scroll with intentional lag/smoothing

## Output format
- Scroll intent
- Container and target
- Thresholds
- Sync mode
- Motion mapping
- Reduced-motion behavior
- Code
- Debug / tuning notes

## Guidance
- Use `debug` while tuning thresholds.
- Be explicit about axis and container when not using the viewport.
- Keep reading-first content readable; don’t let scroll choreography fight comprehension.
- For long pages, only scroll-sync what truly benefits from continuous linkage.
- For section reveals, threshold-triggered motion is often better than full progress sync.
- If scroll motion becomes theatrical rather than informative, say so.

## Anti-patterns
- over-synchronising everything to scroll
- using progress sync for ordinary section reveals that should just trigger once
- unclear target/container ownership
- no reduced-motion path
- scroll motion that introduces jitter or reading resistance

Reference official `onScroll()` notes in `../animejs-core-api/references/official-docs-map.md`.
