# PRD: Integrate Mermaid renderer with phased rollout

## Metadata
- Task ID: 76
- Tag: mermaid
- Status: pending
- Priority: high

## Problem
Plan and implement Mermaid renderer integration across Phase 1 optional binary support and Phase 2 embedded Rust path.

## Goals
- Deliver Mermaid rendering support with a low-risk, optional Phase 1 integration path.
- Provide project-level configurability for render output location with predictable precedence.
- Offer first-class operational UX through install/doctor/status flows in existing AOC control surfaces.
- Prepare a controlled Phase 2 embedded renderer path behind feature flags after Phase 1 validation.

## Non-Goals
- Requiring Mermaid rendering as a hard dependency for all users in Phase 1.
- Reworking Mission Control information architecture for this feature.
- Implementing privileged system package-manager install flows in the initial rollout.

## User Stories
- As a project maintainer, I can configure where Mermaid SVG files are written per project.
- As a user on a new machine, I can discover whether Mermaid rendering is available via doctor/status checks.
- As a user without Mermaid tooling installed, I get a clear, actionable fallback instead of a hard failure.
- As an advanced operator, I can enable embedded rendering in Phase 2 via explicit feature flags.

## Requirements
- Phase 1: Optional external binary integration
  - Integrate support for external `mmdr` (mermaid-rs-renderer) binary when present.
  - Add per-project render output configuration with precedence:
    1) CLI `--out-dir`
    2) Environment override
    3) Project config
    4) Default source-local `renders/`
  - Add Tool Manager controls in `aoc-control` for Mermaid renderer visibility, install, and diagnostics.
  - Implement user-space install path first; defer privileged install paths.
  - Add doctor/status checks that report binary availability, version, and configuration status.
  - Implement graceful fallback with helpful remediation when `mmdr` is missing.
- Phase 2: Optional embedded renderer path
  - Add embedded Rust integration behind feature flags.
  - Keep external-binary path available as fallback during rollout.
  - Define explicit feature gating and migration strategy from Phase 1 to Phase 2.
  - Preserve output parity and configuration semantics established in Phase 1.

## Dependencies
- Upstream `1jehuang/mermaid-rs-renderer` project (MIT license) and version pinning policy.
- Existing `aoc-control` Tool Manager/Settings flows.
- Project config and environment variable plumbing for output-dir precedence.
- CI/bootstrap compatibility with current no-submodule-default workflows.

## Risks and Mitigations
- Submodule/bootstrap fragility risk
  - Mitigation: Phase 1 uses external binary path first; avoid submodule-first dependency.
- UX confusion between Phase 1 and Phase 2 paths
  - Mitigation: clear feature-flag labeling and runtime diagnostics that identify active renderer backend.
- Environment-specific rendering regressions
  - Mitigation: add smoke matrix across session modes and representative environments.

## Security and Licensing
- Preserve attribution/notice requirements for MIT-licensed upstream components.
- Do not introduce privileged install requirements in Phase 1 default path.
- Treat renderer invocation and output paths as untrusted input surfaces; validate and normalize paths.

## Performance
- Rendering latency should remain acceptable for common diagram sizes with no UI hangs.
- Diagnostics and fallback checks should avoid significant startup or command overhead.

## UX
- Keep Mission Control focused on Pulse overview; expose Mermaid operations via `aoc-control` Tool Manager.
- Provide concise, actionable error messages when rendering cannot proceed.
- Keep configuration behavior consistent across CLI, env, and project settings.

## Maintainability
- Preserve backend abstraction so external binary and embedded modes share interfaces.
- Ensure tests cover backend parity to avoid duplicated behavior drift.
- Document feature flags, configuration precedence, and rollout lifecycle.

## Acceptance Criteria
- [ ] Tag `mermaid` contains the integration task and linked PRD.
- [ ] Phase 1 supports external `mmdr` rendering with output-dir precedence and graceful fallback.
- [ ] `aoc-control` exposes Mermaid install/doctor/status workflows for Phase 1.
- [ ] User-space install flow is implemented and documented; privileged flow is explicitly deferred.
- [ ] Phase 2 embedded renderer is implemented behind feature flags with clear backend diagnostics.
- [ ] Rendering outputs and config semantics are equivalent across enabled backends.
- [ ] CI/regression smoke coverage validates both backend modes and fallback behavior.

## Test Strategy
- Unit tests for config precedence resolution and backend selection logic.
- Integration tests for Phase 1 external binary success and missing-binary fallback messaging.
- Integration tests for `aoc-control` install/doctor/status command paths.
- Feature-flag tests confirming Phase 2 embedded backend can be enabled/disabled safely.
- Backend parity tests comparing render output correctness for representative Mermaid fixtures.
- Regression smoke matrix across session modes and key environment variants.

## Success Metrics
- High successful-render rate in supported environments after Phase 1 rollout.
- Reduced setup friction measured by successful doctor/install remediation flows.
- No critical regressions in mission-control flows while Mermaid tooling is introduced.
