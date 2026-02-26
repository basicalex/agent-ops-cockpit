---
name: rlm-analysis
description: Analyze large codebases with aoc-rlm scan, peek, and chunk.
---

## Steps
1. Measure scale: `aoc-rlm scan`
2. Find anchors: `aoc-rlm peek "<term>"`
3. Chunk large areas: `aoc-rlm chunk --pattern "src/**/*.rs"`
4. Use chunks to guide targeted reads and edits.

## Notes
- Prefer chunking before deep file reads in very large repos.
