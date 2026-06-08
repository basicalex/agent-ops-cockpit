# Content Campaign Plan

Strategy source: `hyperframes/docs/brand-strategy.md`  
Concept source: `hyperframes/docs/concept-directions.md`  
Review source: `hyperframes/docs/image-review-board.md`  
SVG manifest: `hyperframes/docs/svg-asset-manifest.md`

## Campaign brief

- Campaign slug:
- Audience:
- Approved concept IDs:
- Primary channel(s):
- CTA:
- Required formats:
- Render policy: preview first; render only when explicitly requested

## Content graph / storyboard

| Node ID | Kind | Beat | Copy | Asset refs | Duration | Edge notes |
| --- | --- | --- | --- | --- | ---: | --- |
| N01 | hook | | | | | sequence -> N02 |

Use `sequence`, `contrast`, and `dependency` edges when converting to html-video ContentGraph.

## HyperFrames outputs

| Composition | Purpose | Status | Preview/render |
| --- | --- | --- | --- |
| `hyperframes/compositions/_playgrounds/brand-campaign-board.html` | Review board | draft | |

## html-video outputs

| Project ID | Manifest | Preview | Render | Status |
| --- | --- | --- | --- | --- |
| | `hyperframes/generated/html-video/<project>/project.json` | | | draft |


Run `aoc-hyperframes brand export` after campaign, html-video, SVG, preview, or render changes to refresh `hyperframes/generated/brand-content/manifest.json` for Prism. Prism imports that JSON bundle; it does not parse this markdown as the durable contract.

## Campaign seam fields for AOC export

`aoc-hyperframes brand campaign <slug>` appends one `## Campaign: <slug>` section with these fields:

```yaml
campaign: <slug>
audience: <audience>
concept: <concept>
channels: [meta,reel]
durations: [15s,6s]
brand_strategy: hyperframes/docs/brand-strategy.md
human_review: hyperframes/docs/image-review-board.md
svg_manifest: hyperframes/docs/svg-asset-manifest.md
hyperframes_catalog: hyperframes/docs/composition-catalog.md
html_video_manifest: hyperframes/generated/html-video/<slug>/project.json
```
