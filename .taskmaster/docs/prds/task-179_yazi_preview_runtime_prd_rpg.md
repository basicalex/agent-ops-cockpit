# Kitty-First Yazi Preview Quality and Runtime PRD (RPG)

## Problem Statement
AOC recently restored native Yazi preview behavior, and the team has now chosen Kitty as the primary terminal direction. However, the actual in-pane Yazi preview experience still fails the UX bar in the expanded 3-column layout.

Current pain points:
- Native Yazi preview in Kitty is functionally rendering, but the preview image is still too small and visibly pixelated inside the third column.
- The preview column itself is large enough; the remaining issue is image placement/scaling quality within that column.
- SVG/image/PDF preview quality is not yet tuned for the actual AOC pane geometry and needs explicit validation criteria.
- The current task framing still overemphasizes terminal-agnostic breadth before proving that Kitty-first inline preview can meet the quality bar.
- AOC still needs a deliberate fallback/runtime strategy if native Kitty preview cannot be tuned to fill the third column with acceptable quality.

We need a Kitty-first preview strategy that prioritizes high-quality, inline, full-usable-width Yazi previews in the expanded third column, while preserving a clear path to custom runtime fallback only if native Kitty behavior cannot be made acceptable.

## Target Users
- AOC users running Kitty who browse files in the left Yazi pane and expect large, legible previews in the expanded 3-column layout.
- Ubuntu/Linux developers using Kitty with tmux/Zellij who need predictable inline preview quality without protocol guesswork.
- Maintainers responsible for AOC installer, doctor, Yazi config, and operator docs.
- Future contributors who need a clear policy for when AOC should stay on native Kitty preview versus escalate to a custom preview runtime.

## Success Metrics
- AOC can detect and report whether native Kitty preview is available and whether it meets the quality bar for the expanded Yazi pane.
- Native Yazi preview in Kitty is either tuned to render at acceptable size/clarity in the third column, or explicitly rejected in favor of a Kitty-first custom inline runtime.
- SVG previews render reliably and legibly in the supported Kitty-first path, without requiring users to debug `resvg`/backend combinations manually.
- Preview quality in the AOC-expanded Yazi pane is explicitly tuned and validated for images, SVGs, PDFs, and representative media assets.
- `aoc-doctor`, installer/docs, and runtime config all describe the same Kitty-first capability model.
- Fallback/custom runtime work is clearly sequenced after native Kitty validation rather than treated as phase 1 by default.

---

## Architectural Framing
This PRD now treats previewing as a **Kitty-first inline Yazi quality problem first**, and only secondarily as a broader runtime abstraction problem.

Three preview modes still matter, but their sequencing changes:
- **Native Kitty/Yazi mode**: First choice. Use Yazi’s built-in preview behavior when Kitty-native rendering is available and can be tuned to meet the quality bar.
- **AOC custom Kitty-first inline mode**: Second choice. If native Kitty preview remains too small or pixelated, use an AOC-owned inline preview runtime that still renders directly in Yazi’s third column.
- **Fallback text mode**: Last resort. Use metadata/text previews when high-quality inline image rendering is unavailable.

The immediate product decision is therefore:
- whether native Kitty preview can be tuned to meet acceptance criteria,
- how AOC detects and reports that quality outcome,
- how AOC escalates to a Kitty-first custom inline runtime if native quality fails,
- and only after that, how broader fallback/portability concerns are documented.

## Capability Tree

### Capability: Preview Capability Detection
Determine what preview strategies are viable in the current runtime.

#### Feature: Environment capability probing
- **Description**: Inspect terminal, multiplexer, OS, and installed tools to determine whether native Yazi or a custom renderer can provide high-quality previews.
- **Inputs**: environment variables, terminal identifiers, tool availability, backend binaries, OS/distribution hints.
- **Outputs**: normalized preview capability report.
- **Behavior**: distinguish among native-ready, custom-overlay-ready, and fallback-only environments.

#### Feature: Quality suitability heuristics
- **Description**: Evaluate whether a technically available backend is actually suitable for the AOC expanded Yazi pane.
- **Inputs**: current pane geometry, preview mode, terminal/backend type, optional user overrides.
- **Outputs**: suitability score or boolean recommendation.
- **Behavior**: allow AOC to reject low-quality-but-available paths such as overly pixelated or undersized rendering.

### Capability: Preview Mode Selection
Choose and persist which preview strategy AOC should use.

#### Feature: Automatic mode selection
- **Description**: Select the best preview mode from available capabilities using deterministic priority rules.
- **Inputs**: capability report, platform defaults, user preference, session context.
- **Outputs**: active preview mode.
- **Behavior**: prefer the highest-quality supported mode and degrade cleanly.

#### Feature: User override and persistence
- **Description**: Let users pin a preferred mode such as native, custom-overlay, or fallback.
- **Inputs**: user config, runtime status, optional per-project/per-user settings.
- **Outputs**: persisted preview policy.
- **Behavior**: respect explicit overrides while still surfacing warnings when the selected mode is unavailable.

### Capability: High-Quality Custom Preview Runtime
Provide an AOC-owned Kitty-first inline rendering path when native Kitty/Yazi preview is unavailable or fails the quality bar.

#### Feature: Bitmap/image overlay rendering
- **Description**: Render images, SVGs, and rasterized documents through a Linux-capable overlay/backend path instead of ANSI-only output.
- **Inputs**: target file, pane geometry, backend choice, style/scale preferences.
- **Outputs**: visible preview in the Yazi preview area.
- **Behavior**: support high-quality scaling and placement, especially in the 3-column expanded mode.

#### Feature: Format rasterization pipeline
- **Description**: Convert SVGs, PDFs, and media into previewable bitmaps before overlay rendering.
- **Inputs**: source file path, format-specific tools (`resvg`, `pdftoppm`, `ffmpeg`), geometry targets.
- **Outputs**: temporary raster assets suitable for preview display.
- **Behavior**: normalize diverse formats into a consistent preview pipeline with clear error handling.

#### Feature: Lifecycle and cleanup
- **Description**: Manage preview process lifetime, temp artifacts, and redraw/clear behavior as the cursor changes.
- **Inputs**: Yazi preview events, pane focus/resize state, preview cache keys.
- **Outputs**: clean updates with no stale overlays or leaked temp files.
- **Behavior**: avoid flicker, stale overlays, and orphaned backend processes.

### Capability: Native Yazi Integration Policy
Define when native Yazi preview remains the preferred path.

#### Feature: Native backend validation
- **Description**: Validate native Yazi preview prerequisites including renderer binaries and terminal protocol support.
- **Inputs**: installed tools, terminal identity, Yazi config, runtime environment.
- **Outputs**: pass/fail reason set for native mode.
- **Behavior**: explain missing dependencies and avoid implying that unsupported terminals should work.

#### Feature: Native-mode quality tuning
- **Description**: Tune native mode where viable by adjusting geometry, pane behavior, and config defaults.
- **Inputs**: Yazi preview config, pane ratios, terminal metrics.
- **Outputs**: improved native preview ergonomics.
- **Behavior**: improve quality without forking behavior unnecessarily when native mode can be made acceptable.

### Capability: Tooling, Docs, and Diagnostics
Make preview behavior understandable and maintainable.

#### Feature: Installer guidance
- **Description**: Install or recommend the correct dependencies per platform and preview mode.
- **Inputs**: OS/distro detection, desired preview modes, package manager availability.
- **Outputs**: actionable install steps and optional automation.
- **Behavior**: avoid false package-manager guidance and clearly distinguish required vs optional components.

#### Feature: Doctor diagnostics
- **Description**: Report preview capabilities, active mode, blockers, and recommended next steps.
- **Inputs**: runtime probing, installed tools, config state.
- **Outputs**: compact capability diagnosis for users and maintainers.
- **Behavior**: explain not just what is missing, but what preview experience the user should expect.

#### Feature: Operator documentation
- **Description**: Document preview mode architecture, terminal compatibility, Ubuntu caveats, and quality tradeoffs.
- **Inputs**: final architecture, install paths, supported environments.
- **Outputs**: updated README/installation/troubleshooting guidance.
- **Behavior**: align docs with real runtime behavior and accepted tradeoffs.

---

## Repository Structure

```text
project-root/
├── yazi/
│   ├── yazi.toml                                # preview mode integration points and native defaults
│   ├── init.lua                                 # status/help surfacing for preview mode where useful
│   └── plugins/
│       └── [preview-runtime integration plugin] # optional mode bridge between Yazi and AOC runtime
├── bin/
│   ├── aoc-doctor                               # capability detection and diagnostics
│   ├── aoc-yazi-preview                         # new AOC preview runtime entrypoint (proposed)
│   ├── aoc-yazi-preview-clear                   # optional cleanup helper (proposed)
│   └── aoc-widget                               # keep media rendering conventions aligned where sensible
├── lib/
│   └── [preview runtime support]                # optional shared shell/python helpers for probing/rendering
├── docs/
│   ├── installation.md                          # install matrix and backend guidance
│   └── yazi-preview.md                          # new dedicated preview architecture/operator doc (proposed)
├── README.md                                    # user-facing summary and troubleshooting
├── install.sh                                   # dependency/install guidance for preview modes
└── .taskmaster/docs/prds/
    └── task-179_yazi_preview_runtime_prd_rpg.md
```

## Module Definitions

### Module: `bin/aoc-yazi-preview` (new)
- **Maps to capability**: High-Quality Custom Preview Runtime
- **Responsibility**: own capability probing, mode selection hooks, rendering dispatch, and preview lifecycle integration.
- **Exports**:
  - preview render entrypoint
  - backend selection logic
  - format dispatcher for images/SVG/PDF/media/text

### Module: `yazi/yazi.toml` and optional Yazi plugin integration
- **Maps to capability**: Preview Mode Selection + Native Yazi Integration Policy
- **Responsibility**: connect Yazi preview events and config to the selected AOC preview strategy.
- **Exports**:
  - preview-related config
  - optional plugin hooks for runtime mode coordination

### Module: `bin/aoc-doctor`
- **Maps to capability**: Preview Capability Detection + Tooling/Diagnostics
- **Responsibility**: report what preview modes are supported and why.
- **Exports**:
  - preview dependency checks
  - terminal/backend diagnostics
  - user-facing remediation guidance

### Module: `install.sh`
- **Maps to capability**: Installer Guidance
- **Responsibility**: install or recommend preview dependencies appropriately by platform.
- **Exports**:
  - dependency install attempts
  - platform-specific warnings and fallbacks

### Module: `docs/installation.md`, `README.md`, and `docs/yazi-preview.md` (new)
- **Maps to capability**: Operator Documentation
- **Responsibility**: document supported preview modes, tradeoffs, and platform caveats.
- **Exports**:
  - install instructions
  - troubleshooting matrix
  - mode-selection explanation

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies - these are built first.

- **preview-capability-model**: canonical vocabulary for `native`, `custom-overlay`, and `fallback` modes plus blocker reasons.
- **preview-quality-policy**: explicit acceptance criteria for what counts as “good enough” in the AOC expanded Yazi pane.
- **preview-config-contract**: user/project config keys for mode selection, overrides, and defaults.

### Detection Layer (Phase 1)
- **environment-prober**: Depends on [preview-capability-model, preview-config-contract]
- **doctor-preview-reporting**: Depends on [preview-capability-model, environment-prober]

### Rendering Layer (Phase 2)
- **format-rasterization-pipeline**: Depends on [preview-capability-model, preview-quality-policy]
- **custom-overlay-backend**: Depends on [preview-capability-model, preview-quality-policy, format-rasterization-pipeline]
- **native-preview-validator**: Depends on [preview-capability-model, environment-prober, preview-quality-policy]

### Integration Layer (Phase 3)
- **preview-mode-selector**: Depends on [environment-prober, custom-overlay-backend, native-preview-validator, preview-config-contract]
- **yazi-preview-bridge**: Depends on [preview-mode-selector, custom-overlay-backend, native-preview-validator]
- **preview-lifecycle-cleanup**: Depends on [custom-overlay-backend, yazi-preview-bridge]

### Productization Layer (Phase 4)
- **installer-preview-guidance**: Depends on [environment-prober, preview-mode-selector]
- **docs-and-troubleshooting**: Depends on [doctor-preview-reporting, installer-preview-guidance, yazi-preview-bridge]
- **acceptance-fixture-matrix**: Depends on [custom-overlay-backend, native-preview-validator, preview-lifecycle-cleanup]

---

## Implementation Roadmap

### Phase 0 — Define the preview architecture contract
**Goal**: Stop treating preview as ad hoc backend detection and define a first-class capability model.

**Deliverables**:
- Canonical preview modes and blocker vocabulary.
- Explicit quality criteria for the AOC expanded Yazi pane.
- Config contract for auto vs forced mode selection.

**Exit Criteria**:
- Maintainers can answer “what preview mode should run here and why?” from one documented contract.
- There is no ambiguity about whether AOC is allowed to prefer custom preview over native Yazi on Linux.

### Phase 1 — Validate and diagnose native Kitty preview quality
**Goal**: Make runtime diagnostics trustworthy and explicitly measure whether native Kitty/Yazi preview meets the UX bar.

**Deliverables**:
- Capability probe covering Kitty, tmux/Zellij context, and current native-preview readiness.
- `aoc-doctor` output that reports expected native Kitty preview quality and blockers, not just missing binaries.
- Native Yazi tuning experiments documented against the expanded 3-column pane.

**Exit Criteria**:
- Doctor distinguishes “native Kitty available and acceptable”, “native Kitty available but fails quality bar”, and “fallback/custom runtime required”.
- Maintainers can clearly explain why the current native preview is or is not acceptable.

### Phase 2 — Build the custom high-quality Kitty-first inline preview runtime
**Goal**: Provide a deliberate inline preview path when native Kitty preview remains too small or pixelated.

**Deliverables**:
- AOC preview runtime entrypoint.
- Rasterization pipeline for SVG/PDF/media.
- Overlay rendering backend integration with geometry control and cleanup.

**Exit Criteria**:
- SVG/image/PDF previews render legibly in the expanded Yazi pane through the supported custom path.
- Cursor movement updates previews cleanly without stale artifacts.

### Phase 3 — Integrate Kitty-first mode selection with Yazi
**Goal**: Make Yazi use the right Kitty-first preview strategy automatically or by explicit user choice.

**Deliverables**:
- Mode selector with deterministic priority rules.
- Yazi bridge/config integration.
- Optional override toggles/settings.

**Exit Criteria**:
- Users do not have to manually rewire Yazi to switch between native and custom modes.
- Preview mode selection is stable across sessions.

### Phase 4 — Finish install/docs/test coverage
**Goal**: Make preview support maintainable and user-comprehensible.

**Deliverables**:
- Installer and docs aligned with actual preview modes.
- Fixture-based acceptance matrix for representative file types and terminal modes.
- Troubleshooting guidance for Ubuntu/Alacritty/Kitty/tmux combinations.

**Exit Criteria**:
- A new user can understand which preview mode they are getting and how to improve it.
- Maintainers have a repeatable verification matrix for regressions.

---

## Test Strategy

### Environment Matrix
Validate at minimum:
- Ubuntu + Kitty + Zellij
- Ubuntu + Kitty + tmux/Zellij combinations where supported
- Ubuntu + Kitty native-preview quality-fail case versus tuned/fixed case
- Ubuntu fallback-only environment with no viable high-quality inline image path

### Asset Matrix
Validate previews for:
- PNG/JPEG/WebP/GIF
- SVG
- PDF first page
- representative video thumbnail path
- text/code/json/csv fallback behavior

### Quality Validation
For each supported mode, verify:
- preview is large enough to be useful in expanded 3-column Yazi mode
- SVG is not silently broken or missing
- rendered output is not unacceptably pixelated under the supported path
- geometry updates correctly on pane expansion/collapse

### Lifecycle Validation
Verify:
- preview clears when leaving the file or pane
- temp files are cleaned up
- stale overlays do not remain after mode switches or pane resizes
- backend processes do not accumulate over repeated navigation

### Diagnostic Validation
Verify:
- `aoc-doctor` reports the correct active/possible preview modes
- install guidance matches actual platform/package availability
- troubleshooting docs resolve common Ubuntu backend failures

---

## Risks and Tradeoffs
- Native Yazi preview may remain impossible or poor in some terminal/protocol combinations; the product must explicitly accept that reality.
- A custom overlay runtime increases maintenance burden and event/lifecycle complexity compared with pure native Yazi behavior.
- `ueberzugpp` availability remains uneven across distros, so installation guidance must not overpromise package-manager simplicity.
- Kitty may offer easier compatibility but may still fail the UX bar if preview quality remains too small/pixelated in AOC’s pane geometry.
- tmux/Zellij interaction can complicate image backends and may require constrained support statements.

## Open Questions
- Should AOC default to a custom Linux preview runtime whenever available, or only when native mode fails capability/quality checks?
- Should preview mode be globally configured, project-scoped, or session-scoped?
- Is `ueberzugpp` the only custom Linux backend AOC should support initially, or should the abstraction leave room for Kitty-native and future backends?
- How much of pane geometry should AOC control directly to optimize preview quality?

## Recommendation
Treat this as a productized preview-runtime decision, not a one-off Yazi tweak. The preferred implementation direction should be:
1. define a first-class preview capability model,
2. implement a high-quality custom Linux preview path,
3. keep native Yazi as a validated mode rather than the only mode,
4. align doctor/install/docs around the same policy.
