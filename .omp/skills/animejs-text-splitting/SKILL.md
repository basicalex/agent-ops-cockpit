---
name: animejs-text-splitting
description: Plan and implement Anime.js splitText() effects for lines, words, and characters with responsive, accessible, and cleanup-aware behavior.
---

Use this skill when the task involves text reveals or text choreography.

## Official-model rules
`splitText()` is described by Anime.js as lightweight, responsive, and accessible.
It can split:
- lines
- words
- chars

Important settings include:
- `lines`
- `words`
- `chars`
- `includeSpaces`
- `accessible`
- `debug`

Important methods include:
- `revert()`
- `addEffect()`
- `refresh()`

## Decision framework
Choose the split level intentionally:
- **lines** → editorial reveals, strong reading rhythm
- **words** → emphasis and staggered semantic pacing
- **chars** → expressive/logo/hero moments only when legibility survives

## Output format
- Text intent
- Split level choice
- Wrapping / clipping strategy
- Accessibility risk
- Responsive behavior
- Code
- Cleanup / refresh notes

## Guidance
- Prefer lines or words for most UI copy.
- Use chars sparingly; call out legibility risk when appropriate.
- Mention whether spaces should be included.
- If the text can reflow, mention `refresh()`.
- If the effect lifecycle ends, mention `revert()`.
- If the motion should be screen-reader-safe, mention `accessible` behavior explicitly.

## Anti-patterns
- character-level motion for body copy
- ornamental split effects on critical UI labels
- no revert/refresh strategy for responsive layouts
- prioritising spectacle over legibility

Reference official text notes in `../animejs-core-api/references/official-docs-map.md`.
