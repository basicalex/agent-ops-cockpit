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
    components/
    brand/
    campaigns/
    ads/
    social/
    landing/
  assets/
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
    shotlists/
    retrospectives/
```

## Source-of-truth rules

- Root design contract: project `DESIGN.md` is the upstream visual/product source of truth when present.
- Visual identity gate: `hyperframes/docs/DESIGN.md` must exist and be reviewed before final composition authoring; it extends the root `DESIGN.md` for media/campaign work.
- Source compositions: `hyperframes/index.html`, `hyperframes/compositions/**`.
- Reusable assets: `hyperframes/assets/brand/**`, `hyperframes/assets/maps/**`, `hyperframes/compositions/components/**`.
- Campaign-specific source: `hyperframes/compositions/campaigns/**`, `ads/**`, `social/**`, `landing/**`, `assets/copy/**`, docs shotlists.
- Generated outputs: `hyperframes/renders/**`.
- Do not treat preview server output as a durable artifact.

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
- `shotlist.md`
- `retrospective.md`
