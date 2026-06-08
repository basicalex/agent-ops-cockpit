# SVG Asset Manifest

Image review source: `hyperframes/docs/image-review-board.md`

## Assets

| Asset ID | Region ID | Source image | Target path | Status | Owner | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| SVG-001 | REG-001 | `hyperframes/assets/generated/approved/IMG-001.png` | `hyperframes/assets/generated/svg/SVG-001.svg` | draft | svg-asset | |

Allowed status: `draft`, `operator-approved`, `needs-revision`, `retired`.

## SVG quality contract

- Clean paths/shapes; no embedded raster images unless explicitly approved.
- Use accessible `<title>` and `<desc>` when the SVG is semantically meaningful.
- Preserve brand colors from `DESIGN.md`/`brand-strategy.md`.
- Keep viewBox, dimensions, and intended usage explicit.
- Prefer reusable symbols/groups over duplicated geometry when it improves maintainability.

## Prism-compatible manifest seam

```yaml
schema: aoc.brand_content.svg_manifest.v1
strategy: hyperframes/docs/brand-strategy.md
human_review: hyperframes/docs/image-review-board.md
assets_directory: hyperframes/assets/generated/svg
assets: []
```
