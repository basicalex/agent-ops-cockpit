---
name: animejs-react-integration
description: Integrate Anime.js safely into React components using refs, createScope(), cleanup via revert(), method registration, and responsive rebuild patterns.
---

Use this skill when Anime.js code lives inside React.

## Official baseline pattern
Anime.js official React guidance uses:
- `useRef()` for a root element
- `useRef()` for the scope instance
- `createScope({ root })`
- `.add(self => { ... })`
- `return () => scope.current.revert()` in `useEffect()` cleanup
- `self.add('methodName', fn)` to expose callable methods outside the setup body

## Core rules
1. Prefer refs and scoped selectors over global selectors.
2. Wrap Anime.js work in `createScope({ root })`.
3. Always define a cleanup story with `scope.current.revert()`.
4. If behavior changes with media queries, consider scope media query features.
5. Use `addOnce()` and `keepTime()` only in the intended way; do not call them conditionally.

## When to recommend `keepTime()`
Use `keepTime()` when a responsive/media-query change should rebuild parameters without losing playback state.

## When to recommend `addOnce()`
Use `addOnce()` when one scoped setup should happen only once and should not be reverted between media-query changes.

## Output format
- Integration strategy
- Root / ref model
- Scope lifecycle
- Public methods if needed
- Code
- Cleanup and rerender notes

## Anti-patterns
- direct global selectors in component code without scope
- no unmount cleanup
- conditionally calling `addOnce()` or `keepTime()`
- using effect reruns to recreate motion without lifecycle reasoning
- ignoring reduced motion in component-level animation systems

Reference official notes in `../animejs-core-api/references/official-docs-map.md`.
