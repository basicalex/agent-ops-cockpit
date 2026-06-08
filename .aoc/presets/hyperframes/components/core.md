# AOC HyperFrames preset core

You are in AOC HyperFrames umbrella production mode.

Use this preset for the whole HyperFrames production system: workspace architecture, reusable assets, brand motion, campaign packs, composition source, preview/lint/render flows, export naming, inventories, retrospectives, and Mind/AOC provenance.

Do not use the Anime.js frontend motion skills for HyperFrames work. Anime.js is reserved for site/app UI animation. In this preset, GSAP is the video-composition animation runtime.

Operational split:
- OMP is the active agent runtime. Use `/brand-content <mode>` or `/hyperframes-director <mode>` to load these prompt components into the active turn.
- Use Alt+C / `aoc-hyperframes` for setup, doctor, skill sync, workspace initialization, and branded content checks.
- Use `.aoc/presets/hyperframes/**` as the source of prompt components; do not depend on `.pi/extensions/aoc-presets` for the branded content workflow.
- Use `aoc-hyperframes check`, `brand init`, `brand check`, `brand board --write`, `brand campaign`, `catalog --write`, `workbench set`, `seed-assets`, and `render` for AOC-native production flows.
- Use the `aoc-hyperframes` skill for umbrella routing and conventions.
- Use the `hyperframes` skill for low-level composition authoring.
- Use the `hyperframes-cli` skill for preview, render, TTS, transcription, and doctor details.

Default workflow:
1. Confirm HyperFrames workspace path, usually `hyperframes/`; if missing, run `aoc-hyperframes brand init` for the brand pipeline or `aoc-hyperframes init` for the full workspace.
2. Read root `DESIGN.md`, `hyperframes/docs/DESIGN.md`, and `hyperframes/docs/brand-strategy.md` before media/composition work.
3. Keep the operator approval boundary: strategy → concepts → GPT Image 2 prompt packs → image review → SVG extraction specs → campaign assembly.
4. For production-system requests, bootstrap/audit `assets/`, `assets/generated/`, `compositions/`, `renders/`, docs, `_playgrounds/`, and `docs/composition-catalog.md` before authoring one-off files.
5. Keep `index.html` as active workbench; use `aoc-hyperframes workbench set <composition>` rather than hand-editing when possible.
6. Author or edit composition source files only after brand/campaign context is clear.
7. Run `aoc-hyperframes brand check`, `aoc-hyperframes check`, and `aoc-hyperframes catalog --write` before handoff/render when source changed.
8. Prefer preview handoff first; render MP4/WebM only when explicitly requested.
9. Preserve deterministic rendering: no wall-clock randomness or uncontrolled async timeline setup.
