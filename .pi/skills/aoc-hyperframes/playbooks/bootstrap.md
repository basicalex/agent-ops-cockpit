# Bootstrap HyperFrames Campaign Factory

Use when HyperFrames exists but needs AOC production structure, catalog, workbench, components, docs, and source/output git policy.

## Steps
1. Confirm `hyperframes/` exists. If not, stop and route to `aoc-hyperframes init`.
2. Run `aoc-hyperframes bootstrap-asset-system --dir hyperframes` to create missing directories, docs, catalog, workbench, playgrounds, and reusable component stubs without overwriting existing source.
3. Confirm root `DESIGN.md` exists; if missing, run `aoc-init` before final composition authoring.
4. Ensure `docs/DESIGN.md` exists and extends root `../../DESIGN.md` as the media/campaign visual gate.
5. Ensure `.gitignore` tracks HyperFrames source/docs/assets while ignoring `renders/**`, `.hyperframes/**`, `.cache/**`, and `node_modules/**`.
6. Run `aoc-hyperframes seed-assets --dry-run` to list candidate assets from `public/`, `apps/*/public/`, `src/assets/`, and `assets/`; use `--copy` or `--symlink` only after review.
7. Run `aoc-hyperframes catalog --write` after adding/moving compositions.
8. Run `aoc-hyperframes check` and fix reported errors before handoff/render.
9. Summarize next required collection: screenshots, screen recordings, map/route imagery, venue/product photos, audio, copy.

## Done when
- Folder contract exists, including `_playgrounds/`, `components/`, `assets/maps/`, docs, and render dirs.
- Required docs exist, including root `DESIGN.md`, `docs/DESIGN.md`, and `docs/composition-catalog.md`.
- `index.html` is either a managed AOC workbench or an explicitly preserved custom file.
- `.gitignore` does not hide all HyperFrames source.
- Visual identity is documented or clearly marked incomplete before composition work, with subsystem notes traceable to root `DESIGN.md`.
- Missing asset checklist is clear.
- `aoc-hyperframes check` passes or remaining warnings are documented.
- No source assets were deleted or overwritten.
