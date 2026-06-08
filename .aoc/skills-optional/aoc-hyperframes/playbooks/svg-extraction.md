# SVG Extraction Playbook

Use when asking OMP specialist agents to convert approved image regions into clean SVG specs/code.

## Procedure

1. Read `image-review-board.md`, `svg-asset-manifest.md`, and brand strategy.
2. Dispatch `svg-asset` with the specific region row, source path, target path, bounds, and style constraints.
3. Specialist output must include exact SVG code/spec and target path for primary-agent/operator application.
4. Reject embedded raster fallbacks unless explicitly approved.
5. Record final status and notes in `svg-asset-manifest.md`.

## Output

Approved SVGs live under `hyperframes/assets/generated/svg/`; the manifest remains the source of truth.
