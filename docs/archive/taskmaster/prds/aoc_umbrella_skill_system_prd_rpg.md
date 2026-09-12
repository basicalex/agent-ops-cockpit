# AOC Umbrella Skill System — RPG PRD

## Problem Statement
AOC currently has many top-level PI skills that mix true operating modes with narrow specialist playbooks. This creates routing clutter, makes Alt+X/preset selection feel noisy, and encourages agents to pick micro-skills ad hoc instead of operating inside coherent production modes. Design, frontend motion, and HyperFrames are the main examples: their specialist guidance is valuable, but the top-level skill surface is too fragmented.

AOC needs a compact umbrella skill system where top-level skills represent durable modes and specialist guidance lives under each mode as playbooks/templates. This preserves quality while reducing cognitive load.

## Target Users
- AOC operators switching between engineering, design, HyperFrames, and motion work.
- Coding agents that need deterministic routing and fewer ambiguous skill choices.
- Maintainers of `.pi/skills` and `aoc-init` seeding.
- Future mode authors such as `aoc-email` / CRM automation.

## Success Metrics
- Top-level default skill inventory is compact and mode-oriented.
- HyperFrames, design, and frontend motion retain specialist depth through playbooks/templates.
- Obsolete micro-skill directories are removed or no longer seeded after content is folded.
- `docs/skills.md`, preset docs, and init/sync behavior match the actual skill surface.
- Future modes can follow the same convention: `SKILL.md` + `playbooks/` + `templates/`.

---

## Capability Tree

### Capability: Skill Surface Audit
Identifies which skills are core modes, which are specialist playbooks, and which are obsolete aliases.

#### Feature: Inventory classification
- **Description**: Classify current `.pi/skills` entries into core, umbrella, fold, optional, or delete.
- **Inputs**: `.pi/skills/*/SKILL.md`, docs, aoc-init seeding paths.
- **Outputs**: Canonical skill inventory and migration plan.
- **Behavior**: Preserve knowledge; remove only duplicate shells or obsolete aliases.

#### Feature: Drift detection
- **Description**: Detect mismatch between docs, init seeding, and actual `.pi/skills`.
- **Inputs**: `docs/skills.md`, `bin/aoc-init`, `.pi/skills`.
- **Outputs**: Required doc/init updates.
- **Behavior**: Ensure deleted skills do not reappear on sync/init.

### Capability: Umbrella Mode Contract
Defines the shape of every mode-level skill.

#### Feature: Mode structure
- **Description**: Standardize umbrella skills as `SKILL.md`, `playbooks/`, and `templates/`.
- **Inputs**: Existing `aoc-hyperframes` pattern.
- **Outputs**: Reusable convention for design, motion, HyperFrames, and future email mode.
- **Behavior**: Top-level skill routes; playbooks hold specialist workflows; templates hold artifact formats.

#### Feature: Routing semantics
- **Description**: Make top-level skills route intent to internal playbooks.
- **Inputs**: User intent, mode context, playbook list.
- **Outputs**: Clear routing instructions.
- **Behavior**: Avoid loading unrelated specialist content unless needed.

### Capability: HyperFrames Media Factory Integration
Treats `aoc-hyperframes` as a production/media factory mode, not just a simple consolidated skill. The v2 factory CLI now owns workspace contracts, design gates, catalogs, workbench targets, campaign creation, guarded asset seeding, render naming, and check-before-render workflow.

#### Feature: HyperFrames factory mode boundary
- **Description**: Keep `aoc-hyperframes` as the visible Alt+X mode while low-level `hyperframes`, `hyperframes-cli`, `website-to-hyperframes`, and `gsap` remain implementation/reference playbooks or hidden support skills.
- **Inputs**: Existing HyperFrames skills, `.aoc/presets/hyperframes`, `bin/aoc-hyperframes`, campaign factory v2 PRD/task.
- **Outputs**: One operator-facing HyperFrames media factory mode with internal routing for composition, CLI, website capture, GSAP, catalog, campaign, asset, render, and retrospective workflows.
- **Behavior**: Preserve low-level correctness without exposing every support skill as a top-level Alt+X choice.

#### Feature: Alt+X HyperFrames simplification
- **Description**: Slim Alt+X to mode-level choices and hide micro-skills from default visible routing.
- **Inputs**: Preset menu, preset skill filters, HyperFrames preset metadata.
- **Outputs**: A compact HyperFrames mode surface; optional internal modes are compose/site/cli/review or fewer if operator experience remains noisy.
- **Behavior**: Prefer `aoc-hyperframes` as active; support skills should be recommended/internal, not user-facing clutter.

#### Feature: HyperFrames factory compatibility
- **Description**: Ensure umbrella skill cleanup does not break campaign factory v2.
- **Inputs**: `aoc-hyperframes check`, `catalog`, `workbench`, `campaign create`, `seed-assets`, `render`.
- **Outputs**: Cleanup plan that preserves v2 factory behavior and docs.
- **Behavior**: Do not delete support skills or prompt assets until their guidance is available under `aoc-hyperframes` or hidden safely by filters.

### Capability: Design Skill Consolidation
Turns design micro-skills into internal playbooks under `design-director`.

#### Feature: Design playbook migration
- **Description**: Fold `design-diff`, `design-handoff`, `design-premium-ui`, `design-redesign`, `design-review`, `design-spec`, and `design-tokens` into `design-director/playbooks`.
- **Inputs**: Existing design skill content.
- **Outputs**: Design playbooks and templates.
- **Behavior**: Preserve critique/spec/tokens/handoff quality without top-level clutter.

#### Feature: Design umbrella routing
- **Description**: Update `design-director/SKILL.md` to route by intent.
- **Inputs**: User asks for review, redesign, diff, spec, tokens, handoff, premium UI.
- **Outputs**: Correct internal playbook selection.
- **Behavior**: One visible design mode.

### Capability: Frontend Motion Skill Consolidation
Turns Anime.js micro-skills into internal playbooks under `motion-director`.

#### Feature: Motion playbook migration
- **Description**: Fold Anime.js specialist skills into `motion-director/playbooks`.
- **Inputs**: `animejs-*` skill content.
- **Outputs**: Playbooks for API, React, timelines, scroll, SVG, text splitting, perf/a11y, review, scene planning.
- **Behavior**: Preserve implementation depth and API correctness.

#### Feature: Motion umbrella routing
- **Description**: Update `motion-director/SKILL.md` to route by motion task.
- **Inputs**: User motion intent.
- **Outputs**: Correct internal playbook selection.
- **Behavior**: One visible frontend motion mode.

### Capability: Docs, Init, and Sync Alignment
Ensures the compact surface remains stable.

#### Feature: Docs update
- **Description**: Update `docs/skills.md` and preset docs to reflect mode umbrellas.
- **Inputs**: Final inventory.
- **Outputs**: Accurate docs.
- **Behavior**: Document mode/playbook distinction.

#### Feature: Init/sync update
- **Description**: Update seeding logic so folded micro-skills do not return as top-level skills.
- **Inputs**: `bin/aoc-init`, `aoc-skill` sync behavior if relevant.
- **Outputs**: Stable compact `.pi/skills` after init/sync.
- **Behavior**: Additive sync should preserve umbrellas and avoid reseeding obsolete shells.

---

## Repository Structure

```text
.pi/skills/
  aoc-hyperframes/
    SKILL.md
    playbooks/
      audit.md
      bootstrap.md
      campaign-pack.md
      composition-authoring.md
      cli.md
      website-capture.md
      gsap-motion.md
      render-export.md
      retrospective.md
    templates/
      asset-inventory.md
      brand-motion-brief.md
      campaign-message-matrix.md
      export-naming.md
      retrospective.md
      shotlist.md

  design-director/
    SKILL.md
    playbooks/
      diff.md
      handoff.md
      premium-ui.md
      redesign.md
      review.md
      spec.md
      tokens.md
    templates/
      design-spec.md
      screen-review.md
      handoff.md
      token-audit.md

  motion-director/
    SKILL.md
    playbooks/
      animejs-core-api.md
      react-integration.md
      timelines.md
      scroll-interaction.md
      svg-motion.md
      text-splitting.md
      performance-a11y.md
      review.md
      scene-planning.md
    templates/
      motion-plan.md
      motion-review.md
```

Final visible top-level skills should trend toward lean umbrella skills plus prompt commands such as `/commit`:

```text
aoc-hyperframes
aoc-init-ops
aoc-map
agent-browser
custom-layout-ops
design-director
motion-director
prd-rpg-authoring
rlm-analysis
teach-workflow
tm-cc
vercel-cli
web-research
zellij-theme-ops
```

Future mode:

```text
aoc-email
```

---

## Dependency Chain

### Foundation Layer — Phase 0
No dependencies.
- Audit current skill inventory.
- Define umbrella mode contract.
- Define canonical final top-level inventory.

### Migration Layer — Phase 1
Depends on Phase 0.
- Adapt HyperFrames around the v2 media factory: one visible `aoc-hyperframes` mode, support skills hidden/internalized only when safe.
- Fold design micro-skills into `design-director`.
- Fold Anime.js micro-skills into `motion-director`.

### Cleanup Layer — Phase 2
Depends on Phase 1.
- Remove obsolete top-level skill directories after content migration.
- Remove alias-only clutter such as `tmcc` if confirmed unnecessary.

### Alignment Layer — Phase 3
Depends on Phase 2.
- Update docs.
- Update `aoc-init`/sync seeding so cleanup persists.
- Add validation/smoke checks.

No circular dependencies: the contract drives migration, migration drives cleanup, cleanup drives docs/init alignment.

---

## Implementation Phases

### Phase 0: Spec and Tasking
- **Goal**: Capture the umbrella skill system as planned work.
- **Exit criteria**: PRD linked to a Taskmaster task with subtasks.
- **Test strategy**: `aoc-task prd show <id> --tag env-protec` resolves this PRD.

### Phase 1: Inventory and Migration Plan
- **Goal**: Produce exact keep/fold/delete list.
- **Exit criteria**: Documented migration map for every current top-level skill.
- **Test strategy**: Compare map against `find .pi/skills -maxdepth 2 -name SKILL.md`.

### Phase 2: HyperFrames Media Factory Alignment
- **Goal**: Align umbrella cleanup with `aoc-hyperframes` campaign/media factory v2.
- **Exit criteria**: Alt+X exposes `aoc-hyperframes` as the visible media factory mode; low-level HyperFrames support skills are either hidden/internal or deliberately retained as non-default references; campaign factory v2 commands and docs remain intact.
- **Test strategy**: Verify `aoc-hyperframes --help`, `bash -n bin/aoc-hyperframes`, preset metadata, skill filters, and factory docs. Do not remove `hyperframes`, `hyperframes-cli`, `website-to-hyperframes`, or `gsap` until their essential guidance is preserved or hidden safely.

### Phase 3: Design Consolidation
- **Goal**: Make `design-director` the only visible design mode.
- **Exit criteria**: Design specialist content exists under `design-director/playbooks` and top-level design micro-skills are removed or no longer seeded.
- **Test strategy**: Spot-check each design playbook for preserved trigger/workflow guidance.

### Phase 4: Motion Consolidation
- **Goal**: Make `motion-director` the only visible frontend motion mode.
- **Exit criteria**: Anime.js specialist content exists under `motion-director/playbooks` and top-level Anime.js micro-skills are removed or no longer seeded.
- **Test strategy**: Spot-check API correctness, React cleanup, timeline, scroll, SVG, text, and perf/a11y playbooks.

### Phase 5: Docs and Init/Sync Alignment
- **Goal**: Ensure compact mode surface persists.
- **Exit criteria**: Docs and init/sync seeding match final inventory.
- **Test strategy**: Run targeted skill validation/sync check; deleted micro-skills do not reappear.

---

## Risks and Mitigations

- **Risk**: Specialist quality gets lost during consolidation.
  - **Mitigation**: Move content before deleting shells; verify one playbook per specialist workflow.
- **Risk**: `aoc-init` reseeds deleted skills.
  - **Mitigation**: Update seeding source/logic in same task before marking cleanup complete.
- **Risk**: Users lose discoverability of specialist workflows.
  - **Mitigation**: Umbrella `SKILL.md` files must list routing table and playbooks clearly.
- **Risk**: Preset switching becomes too coarse.
  - **Mitigation**: Keep internal routing explicit; top-level mode is coarse, playbook selection remains precise.

## Acceptance Criteria

- Task exists with subtasks for audit, HyperFrames, design, motion, docs/init alignment.
- PRD linked to task.
- Final implementation leaves no top-level micro-skill clutter for folded domains; HyperFrames support skills are hidden/internalized in a way that does not break campaign factory v2.
- Quality content remains available under umbrella playbooks/templates or deliberately retained support references.
- Future `aoc-email` can follow the same mode contract.
