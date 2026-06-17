---
name: aoc-hyperframes
description: Umbrella production mode for operating HyperFrames inside AOC. Use for HyperFrames workspace architecture, reusable asset systems, campaign packs, brand motion systems, render/export conventions, inventories, retrospectives, and Mind/AOC provenance. For low-level composition authoring use the hyperframes skill; for CLI commands use hyperframes-cli.
---

# AOC HyperFrames

Operate HyperFrames as an AOC campaign/media production system.

## Mental model

- **Alt+C / `aoc-hyperframes` control**: install, initialize, repair, doctor, and sync HyperFrames support.
- **Alt+X `aoc-hyperframes` preset**: operate the production system: assets, reusable components, campaigns, renders, docs, and provenance.
- **`hyperframes` skill**: low-level HTML/GSAP composition rules.
- **`hyperframes-cli` skill**: preview, lint, render, TTS, transcription, doctor.

## Startup checks

1. Run/consider `aoc-handshake --json` for AOC status.
2. Locate target repo and HyperFrames workspace, usually `hyperframes/`.
3. If no workspace exists, route to Alt+C / `aoc-hyperframes init` before production work.
4. Inspect `hyperframes/hyperframes.json` when present.
5. Avoid reading binary/image/video assets unless the user explicitly asks to view them.

## Routing

Use these playbooks:

- Bootstrap asset system → `playbooks/bootstrap.md`
- Audit workspace/assets → `playbooks/audit.md`
- Create campaign pack → `playbooks/campaign-pack.md`
- Render/export workflow → `playbooks/render-export.md`
- Retrospective/provenance → `playbooks/retrospective.md`

## Canonical workspace structure

```txt
hyperframes/
  index.html
  hyperframes.json
  meta.json
  compositions/
    _playgrounds/
    components/
    brand/
    campaigns/
    ads/
    social/
    landing/
  assets/
    README.md
    brand/
    screens/
    photo/
    maps/
    ui/
    audio/
    captions/
    copy/
  renders/
    brand/
    ads/
    social/
    landing/
    exports/
  docs/
    DESIGN.md
    asset-inventory.md
    brand-motion-brief.md
    campaign-message-matrix.md
    export-naming.md
    composition-catalog.md
    shotlists/
    retrospectives/
```

## Source-of-truth rules

- Root design contract: project `DESIGN.md` is the upstream visual/product source of truth when present.
- Visual identity gate: `hyperframes/docs/DESIGN.md` must exist and be reviewed before final composition authoring; it extends the root `DESIGN.md` for media/campaign work.
- Workbench: `hyperframes/index.html` points at the active composition under review; it is not the only source composition.
- Source compositions: `hyperframes/compositions/**`.
- Navigation/catalog: `hyperframes/docs/composition-catalog.md` plus path conventions provide project UX when the HyperFrames UI lists every HTML file.
- Reusable assets: `hyperframes/assets/brand/**`, `hyperframes/assets/maps/**`, `hyperframes/compositions/components/**`.
- Campaign-specific source: `hyperframes/compositions/campaigns/**`, `ads/**`, `social/**`, `landing/**`, `assets/copy/**`, docs shotlists.
- Playgrounds: `hyperframes/compositions/_playgrounds/**` are review boards and experiments, not final exports by default.
- Generated outputs: `hyperframes/renders/**`, `.hyperframes/**`, `.cache/**`, `node_modules/**`.
- Do not treat preview server output as a durable artifact.

## AOC CLI helpers

Use `aoc-hyperframes` for factory operations:

- `aoc-hyperframes init` / `bootstrap-asset-system` — create/repair factory structure.
- `aoc-hyperframes check` — validate design docs, git policy, catalog, IDs/timelines, shotlists, and HyperFrames lint.
- `aoc-hyperframes catalog --write` — refresh `docs/composition-catalog.md`.
- `aoc-hyperframes workbench set <composition>` — point `index.html` at active comp.
- `aoc-hyperframes campaign create <slug> --audience <name> --channels meta,reel --durations 15s,6s --concept <slug>` — seed campaign pack.
- `aoc-hyperframes seed-assets --dry-run|--copy|--symlink` — discover/apply app assets with inventory provenance.
- `aoc-hyperframes render <composition>` — check, set workbench, render with AOC export naming.

## Safety rules

- Do not delete or destructively move source assets without explicit confirmation.
- Prefer copy/reference for extracted app/site brand assets; keep provenance in `docs/asset-inventory.md`.
- Never overwrite rendered exports silently; increment versions.
- Preserve original captures/screenshots/photos; put normalized derivatives in clearly named subfolders.
- Ask before rendering long videos or running broad asset transformations.
- Keep responses concise; summarize large inventories.

## Mind/AOC provenance

Record major decisions with `aoc-mem add` when they affect reusable production rules, e.g. workspace contract, export naming, brand motion grammar, campaign message families.

Good artifacts for Mind/project intelligence:

- `hyperframes/docs/DESIGN.md`
- `hyperframes/docs/asset-inventory.md`
- `hyperframes/docs/brand-motion-brief.md`
- `hyperframes/docs/campaign-message-matrix.md`
- `hyperframes/docs/export-naming.md`
- `hyperframes/docs/composition-catalog.md`
- `hyperframes/docs/shotlists/*.md`
- `hyperframes/docs/retrospectives/*.md`
- Final render paths and commands used

## Required docs/templates

Use templates from `templates/` when creating missing docs:

- `DESIGN.md`
- `asset-inventory.md`
- `brand-motion-brief.md`
- `campaign-message-matrix.md`
- `export-naming.md`
- `composition-catalog.md`
- `shotlist.md`
- `retrospective.md`
