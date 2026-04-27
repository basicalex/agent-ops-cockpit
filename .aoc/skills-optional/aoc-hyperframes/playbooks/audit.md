# Audit HyperFrames Workspace

Use for structure, asset, composition, and render readiness reviews.

## Check
- Workspace config: `hyperframes.json`, `meta.json`, `index.html`.
- Folder contract: missing canonical directories.
- Docs: inventory, motion brief, message matrix, export naming, shotlists, retrospectives.
- Assets: unclear names, duplicates, missing provenance, large binaries in wrong folders.
- Compositions: reusable vs campaign-specific placement, missing media refs, format variants.
- Renders: versioned names, correct folders, documented commands.

## Output
Return concise sections:
- Healthy
- Missing
- Risky/unclear
- Recommended next actions

Run `npx hyperframes lint` only when user asks for render-readiness or code validation.
