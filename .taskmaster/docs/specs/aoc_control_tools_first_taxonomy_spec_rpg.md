# AOC Control Tools-First Taxonomy Spec (RPG)

## Problem Statement
The Alt+C control TUI still presents AOC as a settings/configuration surface: Tools are nested under Settings, while Theme and custom Layout utilities are promoted despite being low-value or deprecated in current Omarchy/Zellij-driven workflows. Operators most often open Alt+C to run tools such as AOC Understand, Agent Browser + Search, HyperFrames, Vercel, RTK, and compaction. Success means the fastest visible path is `Alt+C -> Tools`, with theme/custom layout utilities demoted to an Advanced/Legacy area.

## Target Users
- AOC operators launching project tools from a terminal-first cockpit.
- Developers using AOC Understand, Agent Browser/Search, HyperFrames, and runtime maintenance flows frequently.
- Maintainers who still need legacy theme/layout hooks without making them prominent.

## Success Metrics
- Top nav shows Tools as the first/default control surface.
- Tool detail titles and docs use `Alt+C -> Tools -> ...` instead of `Alt+C -> Settings -> Tools -> ...`.
- Theme and custom layout creation/editing are labeled deprecated/legacy and reachable only through Advanced.
- Existing tool actions continue to work and tests pass.

## Capability Tree

### Capability: Tools-first navigation
- **Description**: Promote operational tools to the primary Alt+C destination.
- **Inputs**: Existing control pane tab/section state, tool option lists.
- **Outputs**: Top-level Tools nav and direct tool detail sections.
- **Behavior**: Keep existing tool actions while replacing Settings-first labeling with Tools-first labeling.

### Capability: Advanced legacy utilities
- **Description**: Preserve low-priority configuration utilities without presenting them as primary workflows.
- **Inputs**: Existing theme, background, layout, and default layout handlers.
- **Outputs**: Advanced menu with deprecation labels for AOC theme manager and custom layout utilities.
- **Behavior**: Existing code paths remain available but clearly marked legacy/deprecated.

### Capability: Documentation alignment
- **Description**: Update human docs to match the new taxonomy.
- **Inputs**: README/control pane guide references.
- **Outputs**: Updated paths and deprecation guidance.
- **Behavior**: Replace Settings -> Tools paths with Tools paths and describe Omarchy/theme ownership.

## Repository Structure

```text
crates/aoc-control/src/main.rs   # TUI taxonomy, labels, navigation behavior, tests
docs/control-pane.md             # Operator guide for Alt+C taxonomy
README.md                        # Short common setup paths
```

## Module Definitions

### Module: control-navigation
- **Maps to capability**: Tools-first navigation
- **Responsibility**: Top-level nav labels and section transitions.
- **Files**: `crates/aoc-control/src/main.rs`
- **Exports**: Existing binary behavior; no public API change.

### Module: legacy-advanced-section
- **Maps to capability**: Advanced legacy utilities
- **Responsibility**: Label and route theme/layout/background/default utilities as Advanced/Legacy.
- **Files**: `crates/aoc-control/src/main.rs`
- **Exports**: Existing keyboard actions remain.

### Module: control-docs
- **Maps to capability**: Documentation alignment
- **Responsibility**: Update user-facing paths and deprecation rationale.
- **Files**: `docs/control-pane.md`, `README.md`

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies.
- control-navigation: Uses current TUI state and tool action implementations.

### Presentation Layer (Phase 1)
Depends on Foundation.
- legacy-advanced-section: Depends on control-navigation section model.

### Documentation Layer (Phase 2)
Depends on Presentation.
- control-docs: Depends on final labels/paths.

No circular dependencies.

## Implementation Phases

### Phase 1: Promote Tools
- Entry: Existing Tools menu/actions verified in current code.
- Work: Make Tools the first/default top-level nav item and update titles from Settings · Tools to Tools.
- Exit: AOC Understand and related tool tests still pass.

### Phase 2: Demote legacy settings
- Entry: Tools-first path works.
- Work: Rename Settings root to Advanced, label Theme as deprecated/Omarchy-owned, label custom layout utilities as legacy.
- Exit: Advanced still exposes default/background/layout utilities without primary prominence.

### Phase 3: Docs and validation
- Entry: TUI labels settled.
- Work: Update docs/control-pane.md and README paths.
- Exit: cargo test/check and targeted rg checks pass.

## Acceptance Criteria
- `Alt+C` opens with Tools as the first/default nav choice.
- Tool paths are one level shorter: `Alt+C -> Tools -> AOC Understand`.
- Theme manager copy says AOC theme management is deprecated/legacy because Omarchy owns theming.
- Custom layout creation/editing copy says legacy/advanced, not primary workflow.
- Existing tool actions and background job log behavior are not regressed.

## Test Strategy
- `cargo test --manifest-path crates/Cargo.toml -p aoc-control`
- `cargo check --manifest-path crates/Cargo.toml -p aoc-control`
- Targeted `rg` checks for `Alt+C -> Tools`, deprecated theme copy, and legacy layout labels.

## Risks
- **Medium**: Existing code names still use `Defaults`/`SettingsSection`; this spec prefers small label/route changes over a broad rename to reduce risk.
- **Low**: Deprecated utilities remain available, so users who need them are not blocked.
