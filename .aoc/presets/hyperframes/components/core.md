# AOC HyperFrames preset core

You are in AOC HyperFrames umbrella production mode.

Use this preset for the whole HyperFrames production system: workspace architecture, reusable assets, brand motion, campaign packs, composition source, preview/lint/render flows, export naming, inventories, retrospectives, and Mind/AOC provenance.

Do not use the Anime.js frontend motion skills for HyperFrames work. Anime.js is reserved for site/app UI animation. In this preset, GSAP is the video-composition animation runtime.

Operational split:
- Use Alt+C / `aoc-hyperframes` for setup, doctor, skill sync, and workspace initialization.
- Use Alt+X / this preset for operating the reusable campaign factory after install.
- Use `aoc-hyperframes check`, `catalog --write`, `workbench set`, `campaign create`, `seed-assets`, and `render` for AOC-native production flows.
- Use the `aoc-hyperframes` skill for umbrella routing and conventions.
- Use the `hyperframes` skill for low-level composition authoring.
- Use the `hyperframes-cli` skill for preview, render, TTS, transcription, and doctor details.

Default workflow:
1. Confirm HyperFrames workspace path, usually `hyperframes/`; if missing, route to Alt+C / `aoc-hyperframes init`.
2. Read root `DESIGN.md` and `hyperframes/docs/DESIGN.md` before media/composition work.
3. For production-system requests, bootstrap/audit `assets/`, `compositions/`, `renders/`, docs, `_playgrounds/`, and `docs/composition-catalog.md` before authoring one-off files.
4. Keep `index.html` as active workbench; use `aoc-hyperframes workbench set <composition>` rather than hand-editing when possible.
5. Author or edit composition source files only after brand/campaign context is clear.
6. Run `aoc-hyperframes check` before handoff/render when source changed.
7. Prefer preview handoff first; render MP4/WebM only when explicitly requested.
8. Preserve deterministic rendering: no wall-clock randomness or uncontrolled async timeline setup.
