# Render and Export Workflow

Use when linting, previewing, rendering, or packaging outputs.

## Before render
1. Confirm exact composition(s), duration, format, and output target.
2. Run `aoc-hyperframes check` after source/docs/assets changed.
3. Use `aoc-hyperframes workbench set <composition>` for preview focus.
4. Prefer preview handoff before final render.
5. Ensure filenames are versioned and will not overwrite prior exports.

## Naming
Use `project-audience-channel-duration-concept-vN.ext`.

Examples:
- `voyager-business-meta-15s-qr-demo-v1.mp4`
- `voyager-business-reel-6s-multilingual-hook-v1.mp4`
- `voyager-landing-hero-loop-v1.webm`

## Render helper

Prefer:

```bash
aoc-hyperframes render <composition> --format mp4 --quality standard
```

This runs/checks the factory contract, points the workbench at the selected composition, and writes to `hyperframes/renders/<type>/` with AOC export naming.

## After render
- Put output under the correct `hyperframes/renders/**` folder.
- Do not commit render batches by default; promote selected exports only with explicit approval.
- Document command, source composition, output path, and notable warnings.
- Update retrospective or asset inventory if new artifacts become canonical.
