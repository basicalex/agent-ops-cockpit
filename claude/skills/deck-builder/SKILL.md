---
name: deck-builder
description: Build a polished, natively editable PPTX deck (plus PDF and per-slide PNG renders) from a content brief, using ppt-master's Quick Generate route with hand-authored SVG pages, lieflat-charts for data pages, and Playwright renders for visual QA. Use whenever the user asks for a presentation, deck, slides, or PPTX for any project, or to revise a deck built with this skill.
argument-hint: "[deck slug | path to an existing deck workspace]"
---

# deck-builder

Turns a content brief into a PPTX where every shape and line of text stays editable in PowerPoint, with speaker notes, a PDF twin, and PNG renders of every slide. Proven on an 18-slide client delivery deck in Croatian (2026-09-03).

The main session owns the words, the checks, and the verdict. Subagents author SVG pages. Nothing ships until you have looked at every rendered slide.

## Setup (once per machine, idempotent)

```bash
bash ~/.claude/skills/deck-builder/setup.sh
```

Installs into `~/dev/tools/`: `ppt-master` (clone), `lieflat-charts` (clone), `ppt-venv` (uv venv, Python 3.12, ppt-master requirements), and Playwright Chromium for this skill's renderer. Re-run to update the clones. All paths below assume that layout; override with `DECK_TOOLS=<dir>`.

## Workspace layout

```
<workspace>/                 # design-lab/decks/<slug>/ inside a repo, else ~/dev/decks/<slug>/
  BRIEF.md                   # audience, brand, paths, rules for workers
  content.md                 # authoritative per-slide copy + layout intent + notes
  images/                    # screenshots, logos (PNG; exact filenames referenced in content.md)
  charts/                    # lieflat-charts HTML + rendered PNG/SVG + data JSON
  project/<slug>_ppt169_<date>/   # ppt-master project (svg_output/, notes/, images/, validation/)
  render/  render-2x/        # per-slide PNGs from the SVG pages (1x review, 2x for the PDF)
  dist/                      # <Name>.pptx, <Name>.pdf
```

Create it with:

```bash
bash ~/.claude/skills/deck-builder/bin/deck-init.sh <workspace> <slug> [ppt169|ppt43|a4]
```

## Workflow

### 1. Content first, in the main session

Write `content.md` with the user before any SVG exists. Copy the template's per-slide block: eyebrow, title, body lines, visual (which image or native element, where), notes. Every claim on a slide must be true in the code or the data today; check the ones you are not sure of before the deck, not after. Placeholders the owner must fill stay visible as `⟨…⟩`.

Decide with the user: language and register, canvas (16:9 default), fonts (Georgia titles + Arial body travel safely to Windows PowerPoint and PDF), whether slides carry speaker notes, the accent palette. Record those in `BRIEF.md`.

### 2. Images and charts

- Screenshots: capture with Playwright at deviceScaleFactor 2 into `images/` under the exact filenames `content.md` uses. Desktop 1440×900, phone 390×844. Browser closed in `finally`, script self-times-out (macOS has no `timeout` binary).
- Charts: for data pages, prefer native SVG drawn in the page (tick rows, bars, dot grids) from a `charts/*.json` data file. A lieflat-charts render placed as a picture only works when the chart is large on the slide; a full report card shrunk to slide size is unreadable. If you do use lieflat-charts, render the card with Playwright (screenshot the card node at deviceScaleFactor 2, then extract the inner `<svg>` with animation state and dash offsets removed).
- No AI image generation and no web image search unless the user asks.

### 3. Author the SVG pages (delegate)

Dispatch one Opus subagent per deck (or per half for decks over ~15 slides) with `templates/DECK-PACKET.md` filled in. It hand-authors `svg_output/NN_slug.svg`, one file per slide, on the canonical structure in `templates/page-content.svg` and `templates/page-cover.svg`. The packet binds: roster from `content.md`, visual system, type scale, image treatment, and the rule that the worker never rewrites copy.

Read `reference/svg-authoring.md` before writing the packet; paste its rules section into the packet. The worker must read ppt-master's `SKILL.md`, then only `workflows/profiles/quick-generate.md` and the references it names. Quick route: no Strategist, no Confirm UI, no design spec or lock.

If subagents die (API 500s happened four times in a row on 2026-09-03), author the pages yourself; the templates make that fast.

### 4. Gate, export, render (main session)

```bash
bash ~/.claude/skills/deck-builder/bin/deck-export.sh <workspace> "<Output Name>"
```

Runs the final quality gate (blocking errors fail the script and are printed), splits `notes/total.md` into per-slide notes when present, exports `dist/<Output Name>.pptx` with notes, renders every page at 1× and 2× with Chromium, and builds `dist/<Output Name>.pdf` from the 2× renders. LibreOffice is not used; it hangs on this Mac.

Gate only:

```bash
bash ~/.claude/skills/deck-builder/bin/deck-gate.sh <workspace>
```

### 5. Look at every slide

Read each `render/slide-NN.png`. Check overflow, overlaps, clipped text, wrong image ratios, missing diacritics, images covering each other, page numbers. Fix the SVG, re-run step 4. Do not report done until the renders are clean and the export line reads `quality_gate=passed`.

### 6. Deliver

Copy `dist/` and `render/` to a durable place (`~/dev/artifact-library/<slug>/` on this machine, with `content.md` beside them) because scratchpad workspaces disappear. Send the PDF with SendUserFile; the PPTX usually exceeds its 30-second upload window, so give its path instead.

## Revisions

Edit `content.md` first, then the matching SVG, then re-run step 4 and step 5. Renumber page footers if slides are added or parked. Keep parked slides in `project/.../parked/` with their notes so they can return.

## Reference

- `reference/svg-authoring.md` – page structure, bounds, text wrapping, type scale, image treatment, native charts.
- `reference/gotchas.md` – every failure hit so far and its fix. Read before debugging the pipeline.
- ppt-master docs: `~/dev/tools/ppt-master/skills/ppt-master/workflows/profiles/quick-generate.md`, `scripts/docs/svg-pipeline.md`.
- lieflat-charts: `~/dev/tools/lieflat-charts/README.en.md`, `catalog.md`.
