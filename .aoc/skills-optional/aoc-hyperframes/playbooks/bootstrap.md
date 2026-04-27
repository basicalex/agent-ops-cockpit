# Bootstrap HyperFrames Asset System

Use when HyperFrames exists but needs AOC production structure.

## Steps
1. Confirm `hyperframes/` exists. If not, stop and route to `aoc-hyperframes init`.
2. Create missing directories from the canonical workspace structure.
3. Create missing docs from templates without overwriting existing docs.
4. Ensure `docs/DESIGN.md` exists and treat it as the visual identity gate before final composition authoring.
5. Inspect likely brand/source paths narrowly (`public/`, `apps/*/public/`, `src/components/`, `apps/*/src/components/`) and list candidate assets; do not move them unless asked.
6. Update `docs/asset-inventory.md` with existing/missing assets and provenance.
7. Summarize next required collection: screenshots, screen recordings, map/route imagery, venue/product photos, audio, copy.

## Done when
- Folder contract exists.
- Required docs exist, including `docs/DESIGN.md`.
- Visual identity is documented or clearly marked incomplete before composition work.
- Missing asset checklist is clear.
- No source assets were deleted or overwritten.
