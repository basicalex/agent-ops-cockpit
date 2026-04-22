# AOC Control PI Compaction Presets PRD (RPG)

## Problem Statement
PI supports auto-compaction tuning through `compaction.enabled`, `compaction.reserveTokens`, and `compaction.keepRecentTokens`, but today operators must remember token math and manually edit `~/.pi/agent/settings.json` or run manual `/compact` when sessions become heavy. In this repo's real workflow, multiple parallel PI sessions can cause noticeable local CPU/UI slowdown well before PI's default late compaction threshold. We need a first-class Alt+C control-plane flow that makes earlier compaction policies easy to apply globally without hand-editing JSON.

## Target Users
- Developers running multiple parallel PI sessions in AOC/Zellij.
- Operators who want lighter long-running sessions without manually monitoring context growth.
- Repo maintainers who need a discoverable control-plane entry instead of undocumented local tweaks.

## Success Metrics
- A user can apply a global PI compaction preset from `Alt+C -> Settings -> Tools` in one modal flow.
- No manual editing of `~/.pi/agent/settings.json` is required for common presets.
- Existing unrelated PI settings remain preserved when compaction policy is updated.
- The control pane surfaces when the current repo has a `.pi/settings.json` compaction override that will supersede the global preset.
- `cargo check --manifest-path crates/Cargo.toml -p aoc-control` passes after the feature lands.

---

## Capability Tree

### Capability: Global PI Compaction Policy Management
Provide safe read/write access to PI's global auto-compaction settings.

#### Feature: Global settings inspection
- **Description**: Read the effective global compaction policy from `~/.pi/agent/settings.json`.
- **Inputs**: PI global settings file, default PI compaction values.
- **Outputs**: TUI-readable status summary.
- **Behavior**: Load JSON if present, fall back to PI defaults when absent, and tolerate missing files.

#### Feature: Global settings persistence
- **Description**: Persist selected preset values into the global PI settings file.
- **Inputs**: Preset selection, existing settings JSON.
- **Outputs**: Updated `compaction` block in global settings.
- **Behavior**: Preserve unrelated root keys while updating only PI compaction fields.

#### Feature: Project override detection
- **Description**: Detect whether the current repo has a project-level PI compaction override.
- **Inputs**: Current project root, `.pi/settings.json`.
- **Outputs**: Warning state in control-pane summary/detail views.
- **Behavior**: Mark when a local `compaction` object exists because project settings take precedence over globals.

### Capability: Alt+C Preset UX
Expose compaction control in the existing Settings -> Tools structure.

#### Feature: Tools menu entry
- **Description**: Add a `PI compaction` row to the Alt+C Tools section.
- **Inputs**: Current compaction status.
- **Outputs**: Discoverable list entry with live summary.
- **Behavior**: Render summary text inline with existing RTK and installer rows.

#### Feature: Preset action modal
- **Description**: Offer predefined compaction profiles in a modal picker.
- **Inputs**: Preset catalog.
- **Outputs**: Chosen preset written globally.
- **Behavior**: Support open/apply/refresh/close interactions following existing RTK and installer patterns.

#### Feature: Detail-pane guidance
- **Description**: Explain what the selected row does and where it writes settings.
- **Inputs**: Selected row and current status.
- **Outputs**: Operator guidance in the right-hand details pane.
- **Behavior**: Show write target, override warning, and intended use of presets for multi-session ergonomics.

### Capability: Documentation Alignment
Keep operator docs aligned with the shipped Alt+C flow.

#### Feature: Configuration docs update
- **Description**: Document the new Alt+C PI compaction presets and write target.
- **Inputs**: Final UX behavior and preset catalog.
- **Outputs**: Updated README/configuration docs.
- **Behavior**: Explain that presets update PI auto-compaction policy rather than the model context window itself.

---

## Repository Structure

```text
agent-ops-cockpit/
├── crates/
│   └── aoc-control/
│       ├── Cargo.toml                 # Add JSON support for PI settings editing
│       └── src/main.rs                # Alt+C Tools entry, modal, status, JSON read/write
├── docs/
│   └── configuration.md              # Alt+C PI compaction preset docs
├── README.md                         # High-level Alt+C tools summary
└── .taskmaster/docs/prds/
    └── pi_compaction_control_prd_rpg.md
```

## Module Definitions

### Module: `crates/aoc-control/src/main.rs`
- **Maps to capability**: Global PI Compaction Policy Management, Alt+C Preset UX
- **Responsibility**: Read/write PI global compaction settings, render status, detect project overrides, and expose preset actions in the TUI.
- **Exports/behaviors**:
  - Settings -> Tools row for PI compaction
  - Nested compaction section in the left pane
  - Preset modal and refresh action
  - Status/detail summaries and project-override warning

### Module: `crates/aoc-control/Cargo.toml`
- **Maps to capability**: Global settings persistence
- **Responsibility**: Add the JSON dependency required to safely merge/update PI settings.

### Module: `docs/configuration.md`
- **Maps to capability**: Documentation Alignment
- **Responsibility**: Explain the Alt+C PI compaction flow, preset intent, and precedence warning.

### Module: `README.md`
- **Maps to capability**: Documentation Alignment
- **Responsibility**: Keep the top-level Alt+C tools summary accurate.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **PI settings read/write helpers**: parse and persist `~/.pi/agent/settings.json` safely.
- **Compaction status model**: normalize defaults, summary text, and project-override detection.

### UX Layer (Phase 1)
- **Tools menu integration**: depends on [PI settings read/write helpers, compaction status model].
- **Preset modal flow**: depends on [PI settings read/write helpers, compaction status model].

### Docs Layer (Phase 2)
- **README/configuration updates**: depends on [Tools menu integration, preset modal flow].

---

## Development Phases

### Phase 0: Settings Contract
**Goal**: Safely model PI compaction status and persistence.

**Tasks**:
- [ ] Add JSON dependency for `aoc-control`.
- [ ] Implement helpers to read/write the PI global compaction object.
- [ ] Detect current-project `.pi/settings.json` compaction overrides.

**Exit Criteria**:
- Control code can load defaults, read existing settings, and persist compaction updates without dropping unrelated keys.

### Phase 1: Alt+C Control Flow
**Goal**: Add a first-class compaction section and preset modal in `aoc-control`.

**Tasks**:
- [ ] Add `PI compaction` to `Settings -> Tools` with live summary text.
- [ ] Add nested `Settings -> Tools -> PI compaction` actions.
- [ ] Add a modal with predefined presets: default, balanced, aggressive, max-throughput, disable.
- [ ] Show override warnings and write target guidance in details.

**Exit Criteria**:
- User can apply a preset entirely from Alt+C and immediately see updated status.

### Phase 2: Documentation
**Goal**: Align docs with the shipped control-plane feature.

**Tasks**:
- [ ] Update `docs/configuration.md` with PI compaction preset flow and precedence note.
- [ ] Update README Alt+C tools summary.

**Exit Criteria**:
- Docs explain what the presets change and where the global settings are written.

---

## Test Strategy

## Test Pyramid
- Unit/static compile: 80%
- Manual TUI verification: 20%

## Coverage Requirements
- `aoc-control` feature compiles cleanly with the new JSON dependency.
- Preset application preserves unrelated global settings keys in manual verification.

## Critical Scenarios
- Missing `~/.pi/agent/settings.json` => applying a preset creates the file and compaction object.
- Existing global settings with unrelated keys => applying a preset updates only `compaction`.
- Current repo has `.pi/settings.json.compaction` => control pane shows override warning.
- Selecting `Disable auto-compaction` => global settings reflect `compaction.enabled = false`.

## Validation Commands
```bash
cargo check --manifest-path crates/Cargo.toml -p aoc-control
```

---

## Risks & Mitigations
- **Risk**: Operators confuse compaction policy with model context window size.
  - **Mitigation**: Use explicit wording: PI compaction policy / auto-compaction presets / global settings.
- **Risk**: Writing settings could wipe unrelated PI config.
  - **Mitigation**: Merge into the existing root JSON object and replace only the `compaction` object.
- **Risk**: Global preset appears ineffective in repos with project overrides.
  - **Mitigation**: Detect `.pi/settings.json` compaction overrides and surface a warning in the UI.

---

## Taskmaster Mapping Notes
- Recommended tag: `pi-compaction-ui`
- Initial implementation task: add Alt+C PI compaction presets and global settings writer in `aoc-control`.
