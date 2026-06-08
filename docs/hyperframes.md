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
- OMP HyperFrames/brand-content commands plus compatible source skills/prompts
- source-tracking `.gitignore` policy
- `hyperframes/package.json` with `hyperframes@0.4.33` and `packageManager: bun@1.3.9`
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

## Branded content pipeline

Use this when a campaign starts from brand strategy and approved generated concepts rather than from an already-authored composition.

```bash
aoc-hyperframes brand init --brand <brand-slug>
```

Operator workflow:

1. In OMP, activate a mode with `/brand-content strategy` or `/hyperframes-director strategy`.
2. Fill and approve `hyperframes/docs/brand-strategy.md`.
3. Generate 3-7 strategy-bound campaign directions in `hyperframes/docs/concept-directions.md`.
4. Use `/brand-content image` to create GPT Image 2 prompt packs in `hyperframes/docs/image-generation-board.md`.
5. Save generated or Open Design-imported concept images under `hyperframes/assets/generated/concepts/`.
6. Review with the operator in `hyperframes/docs/image-review-board.md`; approve, reject, or mark regions for extraction.
7. Dispatch OMP specialists such as `svg-asset` for exact SVG specs from approved regions, then record outputs in `hyperframes/docs/svg-asset-manifest.md` and `hyperframes/assets/generated/svg/`.
8. Assemble short-form slides/posts/stories through either:
   - html-video content graph + template/studio/render flow for multi-frame campaign videos, or
   - direct HyperFrames HTML/GSAP compositions when custom motion is required.
9. Run `aoc-hyperframes brand check --no-lint`, `aoc-hyperframes check --no-lint`, and `aoc-hyperframes catalog --write`.
10. Export the stable Prism import contract with `aoc-hyperframes brand export`.
11. Preview first; render only when explicitly requested.

Useful commands:

```bash
aoc-hyperframes brand check --no-lint
aoc-hyperframes brand board --write
aoc-hyperframes brand campaign <slug> --audience <audience> --concept <concept>
aoc-hyperframes brand export
aoc-hyperframes brand export --output hyperframes/generated/brand-content/manifest.json
aoc-html-video project create --from hyperframes/docs/content-campaign-plan.md
aoc-html-video project add-assets <project-id> --from hyperframes/docs/svg-asset-manifest.md
```

Generated concept images, approved source images, crops, renders, and html-video exports are ignored by default unless the project explicitly promotes them.

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

Convenience alias from anywhere inside the project:

```bash
aoc-hf
```

Pass preview args after `--`:

```bash
aoc-hf -- --port 3001
```

`aoc-hf` uses the project-local `hyperframes/node_modules/.bin/hyperframes` binary. It never installs dependencies. If missing, run:

```bash
cd hyperframes
bun install
```

Manual equivalent:

```bash
cd hyperframes
bun install
bunx hyperframes preview
```

pnpm is also supported if preferred:

```bash
cd hyperframes
pnpm install
pnpm exec hyperframes preview
```

## Update local HyperFrames CLI

Use explicit local updates so dependency changes are visible in `hyperframes/package.json` and lockfiles:

```bash
aoc-hf-u
```

Pinned version:

```bash
aoc-hf-u 0.4.36
```

Package manager detection prefers `packageManager`, then lockfiles, then bun default. After install/update it runs:

```bash
aoc-hyperframes check
```

## Render

```bash
aoc-hyperframes render compositions/ads/business/meta-15s-qr-demo-v1.html
```

The wrapper runs checks, sets the workbench target, and writes output under `hyperframes/renders/`.

Do not commit render batches by default. Promote selected exports only when explicitly needed.

## OMP command surface

Use these OMP slash commands after setup:

```text
/brand-content strategy
/brand-content concepts
/brand-content image
/brand-content review
/brand-content svg
/brand-content campaign
/hyperframes-director campaign
```

They load prompt components from `.aoc/presets/hyperframes/**` into the active OMP turn. They do not require the legacy Pi preset runtime.

## Requirements

- Node.js `>= 22`
- FFmpeg
- bun preferred for workspace install; pnpm is supported

AOC setup confirms `hyperframes/package.json` during `aoc-hyperframes init`, `aoc-hyperframes bootstrap-asset-system`, and `aoc-init`. Install dependencies before lint/render:

```bash
cd hyperframes
bun install
```

If you prefer pnpm:

```bash
cd hyperframes
pnpm install
```

Check environment:

```bash
aoc-hyperframes doctor
```

Check project readiness:

```bash
aoc-hyperframes check --dir hyperframes
```
