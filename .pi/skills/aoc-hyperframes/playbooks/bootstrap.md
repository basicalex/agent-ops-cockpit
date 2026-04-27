# Bootstrap HyperFrames Asset System

Use when HyperFrames exists but needs AOC production structure.

## Steps
1. Confirm `hyperframes/` exists. If not, stop and route to `aoc-hyperframes init`.
2. Create missing directories from the canonical workspace structure.
3. Create missing docs from templates without overwriting existing docs.
4. Inspect likely brand/source paths narrowly (`public/`, `apps/*/public/`, `src/components/`, `apps/*/src/components/`) and list candidate assets; do not move them unless asked.
5. Update `docs/asset-inventory.md` with existing/missing assets and provenance.
6. Summarize next required collection: screenshots, screen recordings, venue/product photos, audio, copy.

## Done when
- Folder contract exists.
- Required docs exist.
- Missing asset checklist is clear.
- No source assets were deleted or overwritten.
