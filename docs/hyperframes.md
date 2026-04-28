# HyperFrames

HyperFrames is AOC's optional video and campaign factory workflow.

Use it for:

- HTML/CSS/GSAP video compositions
- reusable brand motion components
- campaign packs for ads, social, landing pages, and product demos
- asset inventories, shotlists, catalogs, previews, linting, and renders

## Setup

From an AOC project:

```bash
aoc-hyperframes init
```

Or in AOC:

```text
Alt+C -> Settings -> Tools -> HyperFrames -> Init workspace + campaign factory
```

This seeds the full production workspace:

- `hyperframes/` workspace
- Pi HyperFrames skills and prompt
- source-tracking `.gitignore` policy
- `hyperframes/docs/DESIGN.md`
- asset inventory and brand/campaign docs
- composition catalog
- reusable component stubs
- `_playgrounds/system-board.html`
- shotlists for existing campaign compositions

## Git policy

AOC tracks HyperFrames source/docs/seed assets and ignores generated/heavy outputs.

Expected policy:

```gitignore
# AOC HyperFrames: track source/docs/seed assets; ignore generated/heavy outputs
!/hyperframes/
!/hyperframes/**
/hyperframes/renders/**
/hyperframes/.hyperframes/**
/hyperframes/.cache/**
/hyperframes/node_modules/**
```

Do not ignore `hyperframes/` wholesale.

## Daily commands

```bash
aoc-hyperframes check --dir hyperframes
aoc-hyperframes catalog --dir hyperframes --write
aoc-hyperframes workbench set compositions/_playgrounds/system-board.html
aoc-hyperframes seed-assets --dir hyperframes --dry-run
```

## Create campaign compositions

```bash
aoc-hyperframes campaign create business-first \
  --audience business \
  --channels meta,reel \
  --durations 15s,6s \
  --concept qr-demo
```

This creates campaign compositions, shotlists, and catalog entries without overwriting existing files.

## Preview

Inside AOC:

```text
Alt+C -> Settings -> Tools -> HyperFrames -> Start preview pane
```

Manual:

```bash
cd hyperframes
npx hyperframes preview
```

## Render

```bash
aoc-hyperframes render compositions/ads/business/meta-15s-qr-demo-v1.html
```

The wrapper runs checks, sets the workbench target, and writes output under `hyperframes/renders/`.

Do not commit render batches by default. Promote selected exports only when explicitly needed.

## Preset surface

Use:

```text
Alt+X -> AOC HyperFrames
```

Use this after setup when asking the Pi agent to build, review, or render campaign compositions.

## Requirements

- Node.js `>= 22`
- FFmpeg

Check environment:

```bash
aoc-hyperframes doctor
```

Check project readiness:

```bash
aoc-hyperframes check --dir hyperframes
```
