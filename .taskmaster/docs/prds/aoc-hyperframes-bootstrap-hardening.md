# PRD: AOC HyperFrames Bootstrap Hardening

## Summary

Harden the AOC HyperFrames bootstrap so a freshly initialized HyperFrames workspace becomes filesystem-real and composition-ready, not merely conceptually configured.

The immediate trigger is a Voyager architecture review: HyperFrames was initialized, but the production folders and visual identity source of truth were missing. AOC should make this first production step repeatable, idempotent, and non-destructive.

## Problem

`aoc-hyperframes init` can create a HyperFrames starter workspace, but production work still needs a reusable asset/campaign structure. The current `bootstrap-asset-system` command creates most core folders and docs, but it lacks two important readiness pieces:

- `hyperframes/assets/maps/` for map, route, destination, and geography source assets.
- `hyperframes/docs/DESIGN.md` as the composition-authoring visual identity gate expected by HyperFrames workflows.

Without those, downstream agents can jump into generic composition work before brand tokens, semantic colors, route/marker usage, and CTA/end-card rules are frozen.

## Goals

- Add `assets/maps` to the canonical HyperFrames production folder contract.
- Seed `docs/DESIGN.md` non-destructively during bootstrap.
- Document that visual identity must be filled before final ad/site compositions are created.
- Keep bootstrap idempotent: existing docs/assets are never overwritten.
- Preserve the umbrella architecture: `aoc-hyperframes` handles production system operations; low-level composition stays with the `hyperframes` skill.

## Non-goals

- Do not build Voyager-specific ad compositions in AOC core.
- Do not move/copy real brand assets automatically.
- Do not render videos as part of bootstrap.
- Do not make `assets/maps` mandatory for every campaign; it is a standard available slot.

## User experience

After running:

```bash
aoc-hyperframes bootstrap-asset-system --root <repo> --dir hyperframes
```

A project should have:

```txt
hyperframes/assets/maps/
hyperframes/docs/DESIGN.md
hyperframes/docs/brand-motion-brief.md
hyperframes/docs/asset-inventory.md
hyperframes/docs/campaign-message-matrix.md
hyperframes/docs/export-naming.md
```

The command should print concise next steps telling the operator to fill `docs/DESIGN.md`, inventory assets, then build reusable components before final campaign comps.

## Acceptance criteria

1. `bootstrap-asset-system` creates `hyperframes/assets/maps/`.
2. `bootstrap-asset-system` seeds `hyperframes/docs/DESIGN.md` if missing.
3. Existing `docs/DESIGN.md` is preserved on subsequent runs.
4. `.aoc/skills-optional/aoc-hyperframes/templates/DESIGN.md` exists.
5. `.pi/skills/aoc-hyperframes/templates/DESIGN.md` exists.
6. AOC HyperFrames skill docs list `DESIGN.md` in canonical structure and required docs.
7. Bootstrap playbook explicitly gates composition work on visual identity docs.
8. Temporary workspace validation proves first-run creation and second-run preservation.

## Validation plan

- `bash -n bin/aoc-hyperframes`
- Run `./bin/aoc-hyperframes init` against a temp root.
- Run `./bin/aoc-hyperframes bootstrap-asset-system` against the temp root.
- Assert `hyperframes/assets/maps` exists.
- Assert `hyperframes/docs/DESIGN.md` exists.
- Modify `DESIGN.md`, rerun bootstrap, assert modified content remains.
- Run `./bin/aoc-skill validate --root .`.

## Taskmaster

Primary task: `197` — Harden AOC HyperFrames bootstrap for design docs and map assets.
