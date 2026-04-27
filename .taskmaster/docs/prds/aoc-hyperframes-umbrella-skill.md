# AOC HyperFrames Umbrella Skill PRD

## Purpose
Create an `aoc-hyperframes` umbrella production mode for projects that use HyperFrames as a reusable campaign/media asset factory.

This mode sits above the HyperFrames CLI/composition skills. It owns durable production conventions: workspace structure, reusable assets, campaign packs, render/export discipline, and provenance notes for Mind/AOC.

## Operating Model
- **Alt+C / `aoc-hyperframes` control** installs, initializes, repairs, doctors, and syncs HyperFrames support into a target repo.
- **Alt+X `aoc-hyperframes` preset** operates the production system after install: asset architecture, reusable components, campaign workflows, renders, inventories, and retrospectives.

## Goals
1. Provide a single operator-facing HyperFrames work system instead of many narrow ad hoc skills.
2. Make HyperFrames work reusable across projects such as Voyager.
3. Standardize source asset, composition, campaign, render, and documentation locations.
4. Produce artifacts Mind can understand: briefs, asset inventories, campaign outputs, decisions, commits, and retrospectives.
5. Keep all flows idempotent and safe: never delete source assets or overwrite exports without explicit confirmation/versioning.

## Non-goals
- Replace the upstream HyperFrames CLI.
- Replace low-level composition authoring rules in the existing `hyperframes` skill.
- Force all projects to use one brand identity.
- Auto-render or publish campaign assets without operator intent.

## Canonical Workspace Contract
Within a target repo, `hyperframes/` SHOULD contain:

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
    asset-inventory.md
    brand-motion-brief.md
    campaign-message-matrix.md
    export-naming.md
    shotlists/
    retrospectives/
```

## Required Umbrella Workflows
### Bootstrap
Detect HyperFrames status, create missing folders/docs, seed template inventories, and summarize missing source assets.

### Audit
Inspect workspace structure, assets, compositions, render outputs, missing references, duplicate/unclear assets, and export naming.

### Asset Intake
Classify incoming assets into brand, screens, photo, UI, audio, captions, and copy while preserving originals and provenance.

### Brand Kit
Create or maintain motion/visual rules and reusable brand component requirements.

### Component Library
Plan and create reusable composition components such as intro cards, route draw, signal pulse, QR scan resolve, CTA end card, testimonial card, and logo outro.

### Campaign Pack
Create campaign-specific folders, message matrix, shotlist, required asset checklist, templates, and render targets.

### Render/Export
Run lint/doctor/render only when requested, use versioned names, place outputs in the correct render folders, and document commands/output artifacts.

### Retrospective
Capture what was rendered, what inputs were missing, what decisions changed, and what should be remembered by AOC/Mind.

## Preset Requirements
- Add/rename an Alt+X preset ID `aoc-hyperframes` with label `AOC HyperFrames`.
- Keep compatibility with the existing `hyperframes` preset where practical.
- Active skills should include `aoc-hyperframes`; low-level recommended skills should include `hyperframes`, `hyperframes-cli`, `website-to-hyperframes`, and `gsap` by mode.
- Preset components should explain the Alt+C/Alt+X split.

## CLI Requirements
`bin/aoc-hyperframes` should continue to handle setup/doctor/sync and should also seed/sync the new umbrella skill and preset assets.

Future optional command:
- `aoc-hyperframes bootstrap-asset-system --root <path> --dir hyperframes`

## Acceptance Criteria
- A new `aoc-hyperframes` PI skill exists with routing, conventions, playbooks, templates, safety rules, and Mind/AOC provenance guidance.
- Optional skill sync seeds `aoc-hyperframes` into target repos.
- Alt+X preset metadata exposes `aoc-hyperframes` as the umbrella mode and documents that Alt+C handles install/init.
- A target repo can run the skill/preset after HyperFrames install and receive deterministic instructions for bootstrapping the asset system.
- Taskmaster contains implementation tasks/subtasks linked to this PRD.
