# SVG authoring rules that produced a clean gate and a good-looking deck

Distilled from an 18-page client deck that passed the gate with zero blocking errors (2026-09-03). ppt-master's own references remain the authority for grammar; this file is what mattered in practice.

## Page skeleton

- Root: `<svg xmlns viewBox="0 0 1280 720" width="1280" height="720" font-family="Arial" data-pptx-page-role="…">`. Roles used: `cover`, `content`, `section` (divider), `ending`.
- First child after `<defs>`: `<rect data-pptx-role="background" …>` filling the canvas. A full-bleed cover photo is a second element with `data-pptx-role="background"`; a scrim over it is `data-pptx-role="decoration"`.
- Bounded root groups never overlap one another (blocking). Layer only inside one group.
- Every visible group carries `data-pptx-bounds="x y w h"`. The bounds are the PowerPoint shape box; text that spills past by more than 5 % is a blocking error. Give bounds room and grow them when you add a line. Overflow errors name the group and the percentage.
- Footer: `<g id="chrome" data-pptx-role="footer" data-pptx-bounds="72 674 1136 18">` with deck name left and `NN / TOTAL` right at 13 px, ink 45 %. Not on cover, section, ending pages.
- Images: `href="../images/<file>.png"` relative to `svg_output/`. Copy every image into the project's `images/` before the gate; the exporter embeds from there.

## Text

- Wrap with `<tspan x="<same x as the text>" dy="28">` per line for 20 px body (dy = 1.4 × font size). Never shrink the font to fit; shorten the copy or add a line and grow the bounds.
- Title 44 px Georgia at y ≈ 142 with the eyebrow at y 84 and a hairline at y 176 reads well in a projected room. Two-line titles: 40 px with dy 48, bounds height ≥ 160.
- Secondary text is ink at 62–72 % opacity via `fill-opacity`, not a grey hex.
- `letter-spacing="2.3"` on 13 px bold caps eyebrows (≈ 0.18 em).
- Diacritics pass through untouched; check the export once with `python -c "from pptx import Presentation; …"` if the language has them.
- When renumbering or bulk-editing with `sed`, target attributes exactly. A loose regex once rewrote every `dy="28"` to `dy="-70"` and scrambled continuation lines.

## Images

- Frame = `clipPath` rect with `rx=16`, a white rect with the drop-shadow filter under the image, a 1 px ink-12 % stroke rect over it.
- `preserveAspectRatio="xMidYMid meet"` when the whole capture must show; `xMidYMin slice` to fill a frame and crop the bottom. To hide a band at the top of a capture, shift the image `y` up inside the clip; never change the ratio.
- Two screenshots side by side: equal frames, 24 px gutter, neither overlapping the other. If content.md says an element must stay visible, check it in the render.
- Phone captures at 390:844 come out about 214 × 463 at slide scale; desktop at 16:10 fits 556 × 347 or 600 × 375.

## Native data pages

- Grids of dots (topic × language), tick rows, stacked layer lists, and simple bars drawn as SVG read far better than a placed chart image and stay editable. Compute positions from a JSON file, do not eyeball.
- A lieflat-charts card is designed for a full page at 940+ px width; shrunk to a third of a slide its labels drop below 10 px. Use lieflat when the chart is the slide, or take its palette and line weights and draw natively.
- Dot: `<circle r="7">`, cell pitch 40–48 px, row labels 18 px, column headers 13 px caps.

## Motif and colour discipline

- One accent colour for eyebrows, rules, checkmarks, numerals, process lines. A second "official" colour only for contract or money marks, named in the brief.
- Hairlines: ink at 12 % alpha, 1 px. No black outlines, no full-contrast borders.
- Shadow: `feDropShadow dx=0 dy=8 stdDeviation=12 flood-opacity=0.18`, filter region padded (`x=-14% y=-16% width=128% height=136%`) so it is not clipped.
- Motif line: one smooth `<path>` with cubic segments reused on cover, divider and close with different crops. Never a jagged polyline.

## Notes

- `notes/total.md`: `# NN_slug` heading matching the SVG filename, note body, `---` between slides. `total_md_split.py` writes `notes/NN_slug.md`; the exporter's `--with-notes` reads those.
- A note is what to say, not a repeat of the slide. One to five sentences.
