# html-video

html-video is the motion/storyboard engine from the Open Design team. In AOC it sits between brand/campaign planning and HyperFrames rendering.

## Boundary

| Layer | Owns | Does not own |
| --- | --- | --- |
| Open Design | Visual/design exploration and GUI iteration | Campaign provenance or rendering internals |
| AOC | Strategy, approvals, manifests, project-local assets, OMP workflow | Silently installing html-video or owning its engine internals |
| html-video | ContentGraph/storyboard, templates, studio preview, local MP4 export | AOC task/spec/Mind state |
| HyperFrames | Default HTML/CSS/GSAP render/source engine | Brand approval decisions |
| Prism | System-of-record import of AOC creative manifests, status, reviews, and audit state | First-pass rendering implementation or html-video internals |

## Install policy

AOC never installs html-video implicitly. Configure one of:

```bash
AOC_HTML_VIDEO_BIN=/path/to/html-video
AOC_HTML_VIDEO_HOME=/path/to/html-video-checkout
```

For a checkout, AOC expects the built CLI at:

```text
$AOC_HTML_VIDEO_HOME/packages/cli/dist/bin.js
```

Build html-video explicitly from its own repo before pointing AOC at it.

## Commands

```bash
aoc-html-video status
aoc-html-video doctor
aoc-html-video studio --open
aoc-html-video project create --from hyperframes/docs/content-campaign-plan.md
aoc-html-video project add-assets <project-id> --from hyperframes/docs/svg-asset-manifest.md
aoc-html-video project preview <project-id>
aoc-html-video project render <project-id>
```

`status` and `doctor` are read-only and non-installing. `project create` generates an AOC manifest under `hyperframes/generated/html-video/<project-id>/project.json` even when the html-video CLI is unavailable. `aoc-hyperframes brand export` references those project manifests from `hyperframes/generated/brand-content/manifest.json` for Prism ingestion.

## ContentGraph mapping

AOC maps rows in `hyperframes/docs/content-campaign-plan.md` into html-video ContentGraph nodes and `sequence`, `contrast`, or `dependency` edges. Approved SVG/image/copy assets remain sourced from AOC manifests:

- `hyperframes/docs/brand-strategy.md`
- `hyperframes/docs/image-review-board.md`
- `hyperframes/docs/svg-asset-manifest.md`
- `hyperframes/docs/content-campaign-plan.md`

The Prism-facing bundle is exported separately:

```bash
aoc-hyperframes brand export
```

That bundle stores metadata, paths, ContentGraph node/edge summaries, and operator gate status. It does not inline generated media or make Prism parse campaign markdown as a durable API.

Use html-video for multi-frame storyboard/template work. Use direct HyperFrames HTML/GSAP when custom motion or hand-authored source control matters more than template speed.

## Output policy

Generated html-video manifests, previews, and renders live under:

```text
hyperframes/generated/html-video/
```

Do not commit generated exports by default. Record selected export paths back into campaign docs only after operator review.
