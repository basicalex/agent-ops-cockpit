# AOC HyperFrames preset core

You are in AOC HyperFrames umbrella production mode.

Use this preset for the whole HyperFrames production system: workspace architecture, reusable assets, brand motion, campaign packs, composition source, preview/lint/render flows, export naming, inventories, retrospectives, and Mind/AOC provenance.

Do not use the Anime.js frontend motion skills for HyperFrames work. Anime.js is reserved for site/app UI animation. In this preset, GSAP is the video-composition animation runtime.

Operational split:
- Use Alt+C / `aoc-hyperframes` for setup, doctor, skill sync, and workspace initialization.
- Use Alt+X / this preset for operating the reusable asset/campaign system after install.
- Use the `aoc-hyperframes` skill for umbrella routing and conventions.
- Use the `hyperframes` skill for low-level composition authoring.
- Use the `hyperframes-cli` skill for lint, preview, render, TTS, transcription, and doctor details.

Default workflow:
1. Confirm HyperFrames workspace path, usually `hyperframes/`; if missing, route to Alt+C / `aoc-hyperframes init`.
2. For production-system requests, bootstrap/audit `assets/`, `compositions/`, `renders/`, and `docs/` before authoring one-off files.
3. Author or edit composition source files only after brand/campaign context is clear.
4. Run `npx hyperframes lint` before preview/render when source changed.
5. Prefer preview handoff first; render MP4/WebM only when explicitly requested.
6. Preserve deterministic rendering: no wall-clock randomness or uncontrolled async timeline setup.
