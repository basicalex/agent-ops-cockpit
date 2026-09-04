# Pipeline gotchas (each one cost time on 2026-09-03)

## ppt-master

- `svg_quality_checker.py … --stage draft` is rejected. Use `--stage final` for the gate; the early gate the profile mentions runs with the same flags after the first five pages.
- `--json` writes `validation/svg_quality_report.json`; stdout stays a text summary. Parse stdout with grep (`[ERROR]`, `With errors:`), not `json.load`.
- `svg_to_pptx.py` refuses to export unless the final report in `validation/` passed. Re-run the checker after every SVG edit before exporting.
- Module bounds overflow above 5 % is blocking. The message names the group; widen the `data-pptx-bounds` height or shorten the text.
- "Noncanonical compact authoring" warnings (explicit fill on footer text etc.) are advisory. Ignore or normalise with `compact_svg_styles.py --inplace`, then re-gate.
- Run tool scripts with cwd = the ppt-master repo root and pass absolute project paths; never `cd` into the project.
- `project_manager.py init <slug> --dir <workspace>/project --format ppt169 --quick-generate` puts the project under the workspace instead of the tool clone. `projects/*` in the clone is gitignored anyway.

- Bounded root groups (`<g data-pptx-bounds>`) must not overlap each other beyond 1 px; that is blocking. Full-bleed imagery and scrims are root elements with `data-pptx-role="background"` / `"decoration"`, not a bounded group. Anything you want to overlay on an image goes inside the same group as that image.

## Rendering

- LibreOffice `--convert-to pdf` hangs on this Mac (two 5-minute timeouts, exit 143), even with a fresh `-env:UserInstallation` profile, and a stale `soffice --version` process held the lock. Do not use it. The PDF is built from Chromium renders of the SVG pages with PyMuPDF (`bin/build-pdf.py`).
- bun keeps the event loop alive after `browser.close()` with Playwright 1.62; every render script ends with `process.exit(0)` or the pipeline hangs after writing its PNGs.
- PyMuPDF: `import pymupdf as fitz` (the `fitz` alias import is deprecated).
- The renderer reads the viewBox of the first page, so ppt43 and a4 decks render at their own size.
- The rendered PNGs are of the SVG, not the PPTX. PowerPoint's rendering of Georgia/Arial and shadows is close but not identical; open the PPTX once in PowerPoint or Keynote before a client meeting.

## macOS shell

- No `timeout` binary. Give every Playwright script an internal `setTimeout(() => process.exit(2), 600000)` and close the browser in `finally`.
- Environment variables set with `X=…` before a heredoc are not exported into a Python `os.environ`. Use `export` or hardcode the path.

## Delivery

- SendUserFile times out at 30 s; a 7–8 MB PPTX fails, a 5 MB PDF passes. Send the PDF and give the PPTX path.
- Scratchpad workspaces are per session and disappear. Copy `dist/`, `render/` and `content.md` to `~/dev/artifact-library/<slug>/` before ending the session.

## Subagents

- Four Opus subagents in a row died with `API Error: 500` while authoring. When that happens, author directly from the templates rather than retrying blind; the SVG pages are small.
- Workers never export or render; the orchestrator does, so the gate result and the visual check stay in one place.

## Capturing app screenshots

- Chat/embed APIs may 403 headless user agents. Override per page with CDP `Network.setUserAgentOverride` before navigating.
- Hide framework dev overlays (`nextjs-portal{display:none!important}`) and dismiss consent banners before shooting.
- A curl probe that returns 429 from an embed bootstrap is usually a bot filter on missing browser headers, not a rate limit.
