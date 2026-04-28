# Campaign Pack Workflow

Use when creating a reusable campaign package.

## Inputs
- Campaign name and audience
- Message families/hooks
- Required formats and durations
- Source assets available/missing
- CTA and landing destination

## Create

Prefer CLI scaffold:

```bash
aoc-hyperframes campaign create <campaign> \
  --audience <audience> \
  --channels meta,reel \
  --durations 15s,6s \
  --concept <concept>
```

Expected source layout:

```txt
hyperframes/compositions/_playgrounds/<campaign>-board.html
hyperframes/compositions/ads/<audience>/<channel>-<duration>-<concept>-v1.html
hyperframes/compositions/social/<audience>/<channel>-<duration>-<concept>-v1.html
hyperframes/compositions/landing/<audience>/<concept>-v1.html
hyperframes/assets/copy/<campaign>/
hyperframes/docs/shotlists/<composition-name>.md
hyperframes/docs/retrospectives/
```

## Plan
- 3-5 video concepts
- Required screenshots/photos/audio
- Reusable components needed
- Render targets and naming
- Measurement/retrospective fields

## Rules
- Read root `DESIGN.md` and `hyperframes/docs/DESIGN.md` before writing HTML.
- Keep brand-level components in `compositions/components/` and previewable.
- Keep campaign boards in `_playgrounds/`.
- Keep channel variants under `ads/<audience>/`, `social/<audience>/`, and `landing/<audience>/`.
- Update `docs/composition-catalog.md` with `aoc-hyperframes catalog --write`.
- Create/maintain shotlists for ads/social/landing comps.
- Run `aoc-hyperframes check` before handoff.
- Do not render unless requested.
