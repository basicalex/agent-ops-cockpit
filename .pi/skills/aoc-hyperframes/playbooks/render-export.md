# Render and Export Workflow

Use when linting, previewing, rendering, or packaging outputs.

## Before render
1. Confirm exact composition(s), duration, format, and output target.
2. Run `npx hyperframes lint` when source was changed.
3. Prefer preview handoff before final render.
4. Ensure filenames are versioned and will not overwrite prior exports.

## Naming
Use `project-audience-channel-duration-concept-vN.ext`.

Examples:
- `voyager-business-meta-15s-qr-demo-v1.mp4`
- `voyager-business-reel-6s-multilingual-hook-v1.mp4`
- `voyager-landing-hero-loop-v1.webm`

## After render
- Put output under the correct `hyperframes/renders/**` folder.
- Document command, source composition, output path, and notable warnings.
- Update retrospective or asset inventory if new artifacts become canonical.
