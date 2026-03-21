# Yazi Preview Runtime for Alacritty/Kitty and Terminal-Agnostic Fallbacks PRD (RPG)

## Problem Statement
AOC recently restored native Yazi preview behavior, but the real user workflow still has a major gap: SVG and image previews are unreliable or low-quality across the actual terminal stacks developers use.

Current pain points:
- Native Yazi preview depends on renderer/backend combinations (`resvg`, `ueberzugpp`, Kitty/kitten) that are inconsistent across Ubuntu, Alacritty, tmux, and Zellij.
- Ubuntu setup is especially fragile because `ueberzugpp` is often unavailable in default apt repos and the correct install path is non-obvious.
- Native previews in Kitty can appear too small or pixelated in the AOC 3-column Yazi layout, making the “supported backend” path technically functional but not ergonomically acceptable.
- Alacritty remains a preferred terminal for the AOC workflow, but it does not offer a straightforward native Yazi image protocol path, so “just use native Yazi” does not fully solve the user problem.
- AOC currently lacks a deliberate preview runtime strategy that chooses among native Yazi preview, a high-quality custom Linux renderer, and graceful text fallbacks based on environment capability and user preference.

We need a single AOC-owned preview architecture that preserves good default behavior, works well in the real AOC pane layout, supports SVG/image/PDF/media preview quality targets, and gives users a predictable experience across Kitty, Alacritty, tmux, and future Linux terminal setups.

## Target Users
- AOC users who browse files in the left Yazi pane and expect large, legible previews in the expanded 3-column layout.
- Ubuntu/Linux developers using Alacritty, Kitty, tmux, or Zellij who need a stable preview experience without manually understanding every terminal image protocol.
- Maintainers responsible for AOC installer, doctor, Yazi config, and operator docs.
- Future contributors who need a clear policy for when AOC should rely on native Yazi preview versus a custom preview runtime.

## Success Metrics
- AOC can detect whether high-quality native Yazi preview is viable in the current environment and explain why when it is not.
- AOC provides at least one high-quality preview path that works well in the expanded Yazi pane on Ubuntu/Linux even when native Yazi preview in Alacritty is not viable.
- SVG previews render reliably and legibly in the supported AOC path, without requiring users to debug `resvg`/backend combinations manually.
- Preview quality in the AOC-expanded Yazi pane is explicitly tuned and validated for images, SVGs, PDFs, and representative media assets.
- `aoc-doctor`, installer/docs, and runtime config all describe the same preview capability model.
- Users can choose or accept an automatically selected preview mode such as `native`, `custom-overlay`, or `text-fallback`, with clear degradation behavior.

---

## Architectural Framing
This PRD treats previewing as an AOC runtime capability rather than a single hardcoded Yazi implementation detail.

Three preview modes must be considered explicitly:
- **Native Yazi mode**: Use Yazi’s built-in preview behavior when the terminal/backend stack is truly compatible and quality is acceptable.
- **AOC custom high-quality mode**: Use an AOC-owned preview runtime for Linux terminals where native Yazi is unavailable or visually poor, ideally based on real image rendering rather than ANSI approximation.
- **Fallback text mode**: Use metadata/text previews when neither high-quality image path is available.

The core product decision is not merely “native vs custom,” but rather:
- how AOC detects capability,
- how it selects a preview mode,
- how it preserves quality in the actual AOC pane geometry,
- and how it documents/installs the required pieces.

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
Provide an AOC-owned rendering path for Linux terminals where native Yazi preview is unavailable or poor.

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

### Phase 1 — Detect and explain environment capability correctly
**Goal**: Make runtime diagnostics trustworthy.

**Deliverables**:
- Capability probe covering Ubuntu/Linux terminals, tmux/Zellij, Kitty, `ueberzugpp`, and fallback-only states.
- `aoc-doctor` output that reports expected preview experience, not just missing binaries.

**Exit Criteria**:
- Doctor distinguishes “native available but poor candidate”, “custom overlay available”, and “fallback only”.
- Ubuntu guidance no longer requires users to reverse-engineer backend feasibility.

### Phase 2 — Build the custom high-quality preview runtime
**Goal**: Provide a deliberate Linux preview path that is not constrained by native Yazi limitations in Alacritty.

**Deliverables**:
- AOC preview runtime entrypoint.
- Rasterization pipeline for SVG/PDF/media.
- Overlay rendering backend integration with geometry control and cleanup.

**Exit Criteria**:
- SVG/image/PDF previews render legibly in the expanded Yazi pane through the supported custom path.
- Cursor movement updates previews cleanly without stale artifacts.

### Phase 3 — Integrate mode selection with Yazi
**Goal**: Make Yazi use the right preview strategy automatically or by explicit user choice.

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
- Ubuntu + Alacritty + Zellij
- Ubuntu + Kitty + Zellij
- Ubuntu + Kitty + tmux/Zellij combinations where supported
- Ubuntu fallback-only environment with no viable image backend

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
