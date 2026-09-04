# Deck-builder packet — <Deck>

You are the worker. Never delegate, never spawn agents. Report per ~/.config/aoc/communication-contract.md: plain language, findings not narrative, no filler.

## Inputs (all absolute)
- WS = <workspace path>
- Slide content (authoritative copy, per-slide layout intent, speaker notes): WS/content.md
- Brand + rules: WS/BRIEF.md
- Images: WS/images/*.png (filenames referenced in content.md; see IMAGE-STATUS)
- Charts / data: WS/charts/ (JSON data for native charts; PNG/SVG only if content.md says "as picture")

## Tool: ppt-master, Quick Generate route
- SKILL_DIR = ~/dev/tools/ppt-master/skills/ppt-master
- Python: ~/dev/tools/ppt-venv/bin/python for every `python3` the skill mentions. Run tool scripts with cwd = ~/dev/tools/ppt-master and pass the ABSOLUTE project path.
- Project (already initialised): PROJ = <WS/project/<slug>_ppt169_<date>>. Author pages into PROJ/svg_output/, notes into PROJ/notes/total.md, copy images into PROJ/images/.
- Load order: SKILL_DIR/SKILL.md, run attribution_guard.py if it asks, workflows/routing.md, then ONLY workflows/profiles/quick-generate.md and the references it names (plan-core, canvas-formats, shared-standards-core, executor-base, semantic-svg, preset-shape-vocabulary, executor-image + image layout files, executor-notes). Explicit Quick request: no Strategist, no Confirm UI, no design_spec/lock. Decide everything unspecified yourself.
- Canvas: ppt169 (viewBox 0 0 1280 720). Free design, flat structure. Reading mode: <presentation|document>. Speaker notes <ENABLED: use each slide's "Notes" line | OFF>.
- Fonts: titles Georgia, body Arial. Calibrate with text_measure.py as the skill requires.
- Language: <…>; diacritics must survive into the PPTX.
- Images: copy PNGs into PROJ/images/ as Existing (user-supplied) resources. No AI image generation, no web image search.
- No transitions or animations beyond the skill default; no narration.

## Design contract (binding)
- Roster = exactly the slides in content.md, in order, with the given titles, eyebrows and copy. Do not add slides, do not rewrite copy (fix only an obvious typo and list it in the report). Placeholders in ⟨…⟩ stay visible verbatim.
- Page skeletons: start every page from ~/.claude/skills/deck-builder/templates/page-content.svg or page-cover.svg. Keep `data-pptx-page-role`, the `data-pptx-role="background"` rect, `data-pptx-bounds` on every group, and the footer group.
- Visual system: <paste from BRIEF.md: colours with their single use each, hairlines at ink 12 % alpha, no black outlines, no heavy shadows, no card grids for their own sake, no emoji, icons only where a topic needs one>.
- Type scale (presentation mode): eyebrow 13 px Arial bold caps letter-spacing 2.3; title 44–52 px Georgia; section titles 26 px Georgia; body 20–26 px Arial at ink 72 %; captions 16–18 px. Left-aligned. Margins ≥ 72 px sides. Body lines wrap with `<tspan x="<same x>" dy="28">`, never by narrowing the font.
- Every group's `data-pptx-bounds` must contain its content; text that spills past its bounds by more than 5 % is a blocking gate error. Size bounds generously and grow them when you add lines.
- Screenshot treatment: rounded rect radius 16 via clipPath, 1 px ink 12 % stroke, soft warm shadow (ink 18 %, blur ≈ 24, offset y 8). Phone shots keep 390:844 (about 210–230 px wide on a 720 canvas); desktop shots keep their ratio. Never stretch. Crop only by clipping inside the frame.
- Two images on one slide never overlap unless content.md says so; the element content.md names as "must stay visible" must stay visible.
- Native charts: draw them from WS/charts/*.json as SVG rows/bars/dots with labels in the type scale. Do not place a shrunken chart image.
- Recurring motif (optional): <one thin accent path, same curve on cover/divider/close>.
- Footer on content pages: "<Deck short name>" left, "NN / TOTAL" right, 13 px ink 45 %. None on cover, dividers, close.
- If an image listed in content.md is missing on disk: draw a dashed ink-25 % rounded rect with the filename in 14 px monospace at the same position and continue. Never substitute another image.

## Delivery
1. Author all SVGs in PROJ/svg_output/ (NN_slug.svg, two-digit prefix, order = content.md). Run the early gate after the first five pages as the profile says.
2. Final gate: `bash ~/.claude/skills/deck-builder/bin/deck-gate.sh WS` — fix every blocking error; warnings about noncanonical styles are advisory.
3. Notes: write PROJ/notes/total.md (`# NN_slug` heading per slide, `---` separators) from the Notes lines.
4. Do NOT export or render; the orchestrator runs deck-export.sh and reviews the renders.
5. Report: gate result, every deviation from content.md, every missing image, anything you decided that the packet left open.

## IMAGE-STATUS (verified <date>)
<list each image in content.md: exists / missing / quirks such as bands to clip>
