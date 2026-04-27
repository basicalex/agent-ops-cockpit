# Campaign Pack Workflow

Use when creating a reusable campaign package.

## Inputs
- Campaign name and audience
- Message families/hooks
- Required formats and durations
- Source assets available/missing
- CTA and landing destination

## Create
```txt
hyperframes/compositions/campaigns/<campaign>/
hyperframes/compositions/ads/<campaign>/
hyperframes/compositions/social/<campaign>/
hyperframes/compositions/landing/<campaign>/
hyperframes/assets/copy/<campaign>/
hyperframes/docs/shotlists/<campaign>.md
hyperframes/docs/retrospectives/
```

## Plan
- 3-5 video concepts
- Required screenshots/photos/audio
- Reusable components needed
- Render targets and naming
- Measurement/retrospective fields

## Rules
- Keep brand-level components in `compositions/components/`.
- Keep campaign-specific variants under campaign folders.
- Do not render unless requested.
