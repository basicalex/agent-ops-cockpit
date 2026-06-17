# Anime.js official docs map — distilled for AOC/Pi

Researched from official Anime.js documentation (`animejs.com/documentation`) on 2026-04-20.

## High-value official findings

### Modules-first API
Official docs emphasize flexible imports from either:
- main module: `animejs`
- subpaths such as:
  - `animejs/animation`
  - `animejs/timeline`
  - `animejs/utils`
  - `animejs/text`
  - `animejs/svg`
  - `animejs/events`
  - `animejs/scope`
  - `animejs/draggable`
  - `animejs/waapi`

### React / component integration
Official React guidance uses:
- `useRef()` for root/scope refs
- `createScope({ root })`
- `scope.current = createScope(...).add(self => { ... })`
- `return () => scope.current.revert()` in cleanup
- `self.add('methodName', fn)` to expose methods outside the scope

### Timeline model
Official guidance distinguishes:
- `add()` = creates and adds animation directly into the timeline, allowing tween value composition with existing children
- `sync()` = synchronises an existing animation/timeline into another one
- labels and relative offsets are first-class orchestration tools

### Function-based values
Official signature:
`(target, index, length) => value`

Use case:
- per-target values
- data-driven values
- controlled randomness
- recalculation with `animation.refresh()`

### Text splitting
Official docs describe `splitText()` as:
- lightweight
- responsive
- accessible
- able to split lines / words / chars
- configurable with settings like `lines`, `words`, `chars`, `includeSpaces`, `accessible`, `debug`
- includes methods such as `revert()`, `addEffect()`, `refresh()`

### SVG motion
Official SVG tools:
- `svg.morphTo(target, precision)`
- `svg.createDrawable(target)` exposing `draw`
- `svg.createMotionPath(path, offset)` returning `translateX`, `translateY`, `rotate`

Important perf note from docs:
- animating `createDrawable()` on SVG using `vector-effect: non-scaling-stroke` can be slow because path scale factor must be recalculated every tick

### Scope behavior
Official docs position `createScope()` as the component/responsive orchestration primitive.

Important methods:
- `add()`
- `addOnce()`
- `keepTime()`
- `revert()`

Important rule from docs:
- `addOnce()` and `keepTime()` should not be conditional

### Scroll orchestration
Official `onScroll()` docs position it as a way to trigger or synchronise:
- timers
- animations
- timelines

High-value official concepts:
- `container`
- `target`
- `enter`
- `leave`
- `sync`
- `debug`
- scroll callbacks such as `onEnter`, `onLeave`, `onUpdate`, `onSyncComplete`

Threshold defaults:
- enter default: `'end start'`
- leave default: `'start end'`

Sync modes include progress-synced and smoothed/eased modes.

### WAAPI wrapper differences
Official docs highlight Anime.js WAAPI improvements:
- `ease` instead of native `easing`
- accepts Anime.js easing functions and springs
- default easing is `'out(2)'`, not `'linear'`
- multi-target and function-based ergonomics are improved relative to raw WAAPI

### Engine knobs
Official engine docs highlight:
- `engine.fps` default `120`
- `engine.precision` default `4`
- `engine.pauseOnDocumentHidden` default `true`

Use carefully. These are global levers, not casual per-component tweaks.

## Source URLs used
- https://animejs.com/documentation/
- https://animejs.com/documentation/getting-started/module-imports/
- https://animejs.com/documentation/getting-started/using-with-react/
- https://animejs.com/documentation/timeline/
- https://animejs.com/documentation/timeline/add-animations/
- https://animejs.com/documentation/timeline/sync-timelines/
- https://animejs.com/documentation/animation/targets/
- https://animejs.com/documentation/animation/tween-value-types/function-based/
- https://animejs.com/documentation/utilities/stagger/
- https://animejs.com/documentation/utilities/get/
- https://animejs.com/documentation/utilities/set/
- https://animejs.com/documentation/text/splittext/
- https://animejs.com/documentation/text/splittext/textsplitter-settings/
- https://animejs.com/documentation/text/splittext/textsplitter-methods/
- https://animejs.com/documentation/svg/
- https://animejs.com/documentation/svg/morphto/
- https://animejs.com/documentation/svg/createdrawable/
- https://animejs.com/documentation/svg/createmotionpath/
- https://animejs.com/documentation/easings/spring/
- https://animejs.com/documentation/scope/
- https://animejs.com/documentation/scope/scope-methods/addonce/
- https://animejs.com/documentation/scope/scope-methods/keeptime/
- https://animejs.com/documentation/events/onscroll/
- https://animejs.com/documentation/events/onscroll/scrollobserver-thresholds/
- https://animejs.com/documentation/events/onscroll/scrollobserver-synchronisation-modes/
- https://animejs.com/documentation/events/onscroll/scrollobserver-callbacks/
- https://animejs.com/documentation/web-animation-api/api-differences-with-native-waapi/easing/
- https://animejs.com/documentation/web-animation-api/improvements-to-the-web-animation-api/spring-and-custom-easings/
- https://animejs.com/documentation/engine/engine-parameters/fps/
- https://animejs.com/documentation/engine/engine-parameters/precision/
- https://animejs.com/documentation/engine/engine-parameters/pauseondocumenthidden/
- https://animejs.com/documentation/timeline/timeline-methods/revert/
- https://animejs.com/documentation/animation/animation-methods/refresh/
