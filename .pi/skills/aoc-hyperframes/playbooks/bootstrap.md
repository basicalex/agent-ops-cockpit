# Bootstrap HyperFrames Campaign Factory

Use when HyperFrames exists but needs AOC production structure, catalog, workbench, components, docs, and source/output git policy.

## Steps
1. Confirm `hyperframes/` exists. If not, stop and route to `aoc-hyperframes init`.
2. Run `aoc-hyperframes bootstrap-asset-system --dir hyperframes` to create missing directories, docs, catalog, workbench, playgrounds, reusable component stubs, and `hyperframes/package.json` without overwriting existing source.
3. Confirm `hyperframes/package.json` pins `hyperframes@0.4.33` and `packageManager: pnpm@10.33.2`; install with `pnpm install` before lint/render. `bun install` is acceptable if project prefers bun.
4. Use `aoc-hf` for local preview convenience after install; use `aoc-hf-u [version]` for explicit local HyperFrames dependency updates.
5. Confirm root `DESIGN.md` exists; if missing, run `aoc-init` before final composition authoring.
6. Ensure `docs/DESIGN.md` exists and extends root `../../DESIGN.md` as the media/campaign visual gate.
7. Ensure `.gitignore` tracks HyperFrames source/docs/assets while ignoring `renders/**`, `.hyperframes/**`, `.cache/**`, and `node_modules/**`.
8. Run `aoc-hyperframes seed-assets --dry-run` to list candidate assets from `public/`, `apps/*/public/`, `src/assets/`, and `assets/`; use `--copy` or `--symlink` only after review.
9. Run `aoc-hyperframes catalog --write` after adding/moving compositions.
10. Run `aoc-hyperframes check` and fix reported errors before handoff/render.
11. Summarize next required collection: screenshots, screen recordings, map/route imagery, venue/product photos, audio, copy.

## Done when
- Folder contract exists, including `_playgrounds/`, `components/`, `assets/maps/`, docs, and render dirs.
- Required docs exist, including root `DESIGN.md`, `docs/DESIGN.md`, and `docs/composition-catalog.md`.
- `hyperframes/package.json` exists with pnpm package manager metadata and pinned HyperFrames CLI dependency.
- `aoc-hf` and `aoc-hf-u` are available after AOC install; `aoc-init` confirms aliases when HyperFrames is present.
- `index.html` is either a managed AOC workbench or an explicitly preserved custom file.
- `.gitignore` does not hide all HyperFrames source.
- Visual identity is documented or clearly marked incomplete before composition work, with subsystem notes traceable to root `DESIGN.md`.
- Missing asset checklist is clear.
- `aoc-hyperframes check` passes or remaining warnings are documented.
- No source assets were deleted or overwritten.
