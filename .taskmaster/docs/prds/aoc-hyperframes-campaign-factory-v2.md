# AOC HyperFrames Campaign Factory v2 PRD

## Summary

Make HyperFrames an AOC-native campaign asset factory instead of a one-off composition folder. AOC should initialize, validate, organize, and operate a repeatable creative production workspace across projects.

## Goals

- Track HyperFrames source/docs/assets by default while ignoring generated output/cache.
- Seed a richer canonical workspace: design docs, catalogs, workbench, playgrounds, reusable components, campaign docs, and render folders.
- Provide CLI helpers for check/catalog/workbench/campaign/seed-assets/render workflows.
- Keep all writes idempotent and non-destructive by default.
- Make agent presets enforce design-contract-first, catalog-first, lint/check-before-render workflow.

## Non-goals

- Replace HyperFrames CLI internals.
- Automatically commit generated media.
- Blindly copy large/raw/private assets into repos.
- Overwrite custom project compositions or docs.

## Product contract

AOC HyperFrames workspaces use layered source of truth:

```txt
DESIGN.md
  -> hyperframes/docs/DESIGN.md
  -> hyperframes/docs/brand-motion-brief.md
  -> hyperframes/docs/campaign-message-matrix.md
  -> hyperframes/docs/composition-catalog.md
  -> hyperframes/compositions/**
```

`hyperframes/index.html` is the active workbench, not the only composition. `_playgrounds/` contains campaign/system boards. `components/` contains reusable previewable primitives. `renders/` and cache directories are generated output.

## Canonical structure

```txt
hyperframes/
  index.html
  hyperframes.json
  meta.json
  assets/
    README.md
    audio/
    brand/
    captions/
    copy/
    maps/
    photo/
    screens/
    ui/
  compositions/
    _playgrounds/
      system-board.html
    ads/
    brand/
    campaigns/
    components/
      signal-pulse.html
      route-draw.html
      marker-activate.html
      qr-scan-resolve.html
      cta-end-card.html
    landing/
    social/
  docs/
    DESIGN.md
    asset-inventory.md
    brand-motion-brief.md
    campaign-message-matrix.md
    composition-catalog.md
    export-naming.md
    shotlists/
    retrospectives/
  renders/
    ads/
    brand/
    exports/
    landing/
    social/
```

## CLI requirements

### `aoc-hyperframes init` / `bootstrap-asset-system`

- Create/repair canonical structure.
- Seed missing docs/templates only.
- Seed workbench/index and system board only when missing.
- Seed reusable component stubs only when missing.
- Apply git policy that tracks source/docs/assets and ignores output/cache.
- Run a project check after bootstrap when possible.

### `aoc-hyperframes check`

Validate:

- root `DESIGN.md` exists.
- `hyperframes/docs/DESIGN.md` exists and references root design contract.
- required docs/dirs exist.
- `.gitignore` does not ignore the whole HyperFrames workspace without negation.
- render/cache/node_modules paths are ignored.
- `index.html` points to an existing composition when it declares an AOC workbench target.
- HTML compositions include `data-composition-id` and timeline registration.
- ads/social/landing compositions have matching shotlists.
- `npx hyperframes lint` passes when toolchain is available.

### `aoc-hyperframes catalog [--write]`

- List compositions by path/id/status.
- With `--write`, update `docs/composition-catalog.md` inside a managed block.

### `aoc-hyperframes workbench set <composition>`

- Update/create `index.html` as active workbench.
- Use managed AOC markup.
- Refuse missing target.
- Preserve custom file unless managed block exists or file is missing.

### `aoc-hyperframes campaign create <slug>`

Options:

```txt
--audience NAME
--channels meta,reel
--durations 15s,6s
--concept NAME
```

- Generate channel-specific composition stubs.
- Generate playground campaign board.
- Generate matching shotlists.
- Update catalog.
- Never overwrite existing files.

### `aoc-hyperframes seed-assets`

- Default to `--dry-run`.
- Scan common asset roots.
- Classify logos/screens/maps/ui/copy/audio/photo.
- Copy or symlink only with explicit `--copy` or `--symlink`.
- Enforce size cap.
- Never overwrite.
- Append inventory rows with provenance.

### `aoc-hyperframes render <composition>`

- Run check before render unless `--skip-check`.
- Resolve source composition.
- Build export name using project-audience-channel-duration-concept-vN convention.
- Output under `renders/<type>/`.
- Delegate to `npx hyperframes render` when available.

## Agent workflow requirements

AOC HyperFrames skill/preset must instruct agents to:

- Read root `DESIGN.md` and `hyperframes/docs/DESIGN.md` first.
- Use `index.html` as active workbench.
- Use `_playgrounds/` for boards/reviews.
- Update asset inventory when assets change.
- Update catalog when comps move/add.
- Create shotlists for ads/social/landing.
- Run `aoc-hyperframes check` before handoff/render.

## Acceptance criteria

- Fresh `aoc-hyperframes init` creates the v2 factory structure.
- Existing docs/compositions are preserved.
- Gitignore policy no longer hides all HyperFrames source.
- `check`, `catalog --write`, `workbench set`, `campaign create`, `seed-assets --dry-run`, and render dry path work on a temp repo.
- Voyager can be refreshed with `aoc-init` and `aoc-hyperframes bootstrap-asset-system` without destructive changes.
