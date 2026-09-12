# Runtime Artifact Isolation and Continuous Secret Verification PRD (RPG)

## Problem Statement
Even after secret-safe ingestion and storage hardening, the system remains fragile if durable Mind artifacts live in repo-visible paths or if there is no continuous verification that leaks have not returned. The recent incident became materially worse because `.aoc/mind` runtime data was tracked and could be committed. We need operational hardening that makes runtime state non-committable by default and continuously checks that sensitive persistence cannot recur.

## Target Users
- **Operators and maintainers** who need safe runtime defaults.
- **Contributors** who should not accidentally commit live Mind state.
- **Security responders** who need repeatable scan and remediation workflows.

## Success Metrics
- Runtime Mind persistence defaults to a non-committed location or otherwise isolated policy surface.
- `.aoc/mind` remains ignored or contains only intentionally exported safe artifacts.
- Automated checks detect known secret markers and forbidden runtime artifacts before merge/release.
- Incident response guidance exists for rotation, cleanup, and verification after any suspected leak.

---

## Capability Tree

### Capability: Runtime Artifact Isolation
Prevent live Mind state from becoming normal source-controlled content.

#### Feature: Safe storage path policy
- **Description**: Resolve durable Mind storage to a non-repo runtime/state location where feasible.
- **Inputs**: Project root, runtime dir, XDG state settings, overrides.
- **Outputs**: Canonical storage path policy.
- **Behavior**: Prefer runtime/state directories over tracked repo paths; support explicit safe overrides.

#### Feature: Export boundary separation
- **Description**: Distinguish live runtime state from explicit human-readable exports.
- **Inputs**: Runtime state and export generation requests.
- **Outputs**: Clear separation between transient state and safe exports.
- **Behavior**: Keep only deliberate, validated exports in project-visible locations.

### Capability: Commit-Safety Controls
Reduce the chance that runtime artifacts enter git again.

#### Feature: Ignore policy enforcement
- **Description**: Keep runtime artifact paths ignored and documented.
- **Inputs**: Repo policy files and runtime path conventions.
- **Outputs**: Stable ignore rules and path expectations.
- **Behavior**: Treat tracked runtime DB artifacts as policy violations.

#### Feature: Pre-merge verification
- **Description**: Check for forbidden runtime files and secret markers in git-visible surfaces.
- **Inputs**: Working tree, staged diff, generated artifacts.
- **Outputs**: Pass/fail verification result.
- **Behavior**: Fail fast when forbidden artifacts or known secret markers are detected.

### Capability: Incident Remediation Readiness
Provide a repeatable response when compromise is suspected.

#### Feature: Rotation and cleanup playbook
- **Description**: Document what to rotate, purge, and verify after a leak.
- **Inputs**: Known provider/token classes and repo cleanup workflow.
- **Outputs**: Operator-ready incident instructions.
- **Behavior**: Cover token rotation, history cleanup, branch/tag handling, and verification scans.

---

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-agent-wrap-rs/
│   │   └── src/main.rs
│   ├── aoc-cli/
│   │   └── src/overseer.rs
│   └── aoc-mission-control/
│       └── src/main.rs
├── .gitignore
├── docs/
│   └── security/ or runtime-policy docs
└── scripts/ or crate-local test helpers for verification
```

## Module Definitions

### Module: runtime path resolution (`aoc-agent-wrap-rs`, `aoc-cli`, related callers)
- **Maps to capability**: Safe storage path policy
- **Responsibility**: Resolve durable Mind storage outside commit paths where feasible and keep override behavior explicit.

### Module: repo policy (`.gitignore`, verification scripts/tests)
- **Maps to capability**: Ignore policy enforcement + Pre-merge verification
- **Responsibility**: Encode commit-safety rules and automated checks.

### Module: security docs/playbook
- **Maps to capability**: Rotation and cleanup playbook
- **Responsibility**: Provide actionable incident response instructions.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **Runtime path policy**: Canonical decision for where live Mind state belongs.
- **Verification policy**: Define forbidden files and secret marker scan scope.

### Runtime Isolation Layer (Phase 1)
- **Path resolver updates**: Depends on [Runtime path policy]
- **Export boundary rules**: Depends on [Runtime path policy]

### Verification Layer (Phase 2)
- **Ignore and scan enforcement**: Depends on [Verification policy, Path resolver updates]

### Operations Layer (Phase 3)
- **Incident playbook**: Depends on [Ignore and scan enforcement]

---

## Development Phases

### Phase 0: Policy Definition
- Decide where live Mind runtime state should live by default.
- Define which project-visible artifacts are permitted exports versus forbidden runtime state.

### Phase 1: Runtime Isolation
- Update path resolution and export boundaries to align with policy.
- Preserve compatibility where necessary via explicit override mechanisms.

### Phase 2: Continuous Verification
- Add checks for forbidden runtime files and known secret markers.
- Ensure `.gitignore` and related repo policy stay aligned with runtime behavior.

### Phase 3: Incident Readiness
- Write operator-facing remediation guidance for rotation, purge, verification, and cleanup.

---

## Test Strategy
- Path resolution tests for repo-root vs runtime-dir behavior.
- Repo-policy tests ensuring forbidden Mind runtime artifacts are ignored or absent.
- Automated scan tests for known secret markers in git-visible artifacts.
- Playbook validation against the actual cleanup and verification workflow used in this incident.

---

## Risks and Mitigations
- **Risk**: Existing workflows depend on repo-local `.aoc/mind` paths.
  - **Mitigation**: Support explicit overrides and migration behavior while moving default paths to safer locations.
- **Risk**: Verification only covers known patterns.
  - **Mitigation**: Combine marker scans with structural checks for forbidden runtime artifacts.
- **Risk**: Safe exports and runtime state become conflated again.
  - **Mitigation**: Keep path and artifact classes explicit in code and docs.
