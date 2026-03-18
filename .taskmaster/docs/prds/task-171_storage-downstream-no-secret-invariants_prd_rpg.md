# Storage and Downstream No-Secret Invariants PRD (RPG)

## Problem Statement
Even with ingress sanitization, Mind remains unsafe if any caller can persist secret-bearing payloads directly into storage or if derived layers can reconstruct unsafe text. Today `insert_raw_event` serializes and writes event payloads without enforcing a hard no-secret invariant, which means one bypass can repopulate `raw_events`, then propagate into compaction, observations, reflections, and exported artifacts.

We need storage-backed invariants that reject or sanitize unsafe content and prove that all downstream layers remain secret-free.

## Target Users
- **Mind/storage maintainers** responsible for durability guarantees.
- **Operators** who need assurance that a single missed call site cannot reintroduce leaks.
- **Security and incident responders** who need reliable verification surfaces.

## Success Metrics
- Unsafe crafted raw events cannot be durably inserted without sanitization or rejection.
- `raw_events`, `compact_events_t0`, `observations_t1`, `reflections_t2`, handshake exports, and project-mind exports all test clean for known secret patterns.
- Storage bypass attempts fail loudly with actionable error semantics.
- End-to-end test coverage proves secret-free invariants across persistence and derived artifacts.

---

## Capability Tree

### Capability: Storage Boundary Enforcement
Turn storage into a last-chance security gate.

#### Feature: Raw event validation before write
- **Description**: Validate serialized raw event payloads before insertion.
- **Inputs**: `RawEvent` destined for `insert_raw_event`.
- **Outputs**: Successful insert only if payload satisfies no-secret invariant.
- **Behavior**: Reject or sanitize payloads that still contain secret markers.

#### Feature: Security failure reporting
- **Description**: Surface why a payload was blocked.
- **Inputs**: Validation failures.
- **Outputs**: Structured storage/security error.
- **Behavior**: Fail closed with clear diagnostics that do not echo secret text.

### Capability: Derived Artifact Safety
Prevent propagation of unsafe text into higher-order Mind artifacts.

#### Feature: T0 compaction safety
- **Description**: Ensure compacted events and snippets never contain unsafe strings.
- **Inputs**: Sanitized or validated raw events.
- **Outputs**: Secret-free T0 rows.
- **Behavior**: Preserve tool metadata while withholding unsafe output text.

#### Feature: T1/T2 derivation safety
- **Description**: Ensure observations and reflections cannot include raw secret-bearing content.
- **Inputs**: T0 inputs and semantic/distillation outputs.
- **Outputs**: Secret-free T1/T2 artifacts.
- **Behavior**: Validate derived text prior to persistence or export.

#### Feature: Export safety
- **Description**: Ensure handshake and project-mind markdown exports are secret-free.
- **Inputs**: Derived canon, reflections, and provenance content.
- **Outputs**: Safe markdown artifacts.
- **Behavior**: Validate/export only sanitized text and fail closed on violation.

---

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-storage/
│   │   └── src/lib.rs
│   ├── aoc-core/
│   │   └── src/mind_contracts.rs or security helper module
│   ├── aoc-mind/
│   │   └── src/lib.rs
│   └── aoc-agent-wrap-rs/
│       └── src/main.rs
└── tests/ or crate-local tests
```

## Module Definitions

### Module: `crates/aoc-storage/src/lib.rs`
- **Maps to capability**: Storage Boundary Enforcement
- **Responsibility**: Validate raw event payloads before SQLite persistence; return explicit security failures on violation.
- **Exports**:
  - `insert_raw_event(...)` with no-secret enforcement
  - `StorageError::SecurityViolation` or equivalent

### Module: `crates/aoc-mind/src/lib.rs`
- **Maps to capability**: T1/T2 derivation safety
- **Responsibility**: Ensure distillation and semantic outputs cannot persist unsafe strings.

### Module: `crates/aoc-agent-wrap-rs/src/main.rs`
- **Maps to capability**: Export safety
- **Responsibility**: Validate handshake/project-mind exports before writing them to disk.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **No-secret validator contract**: Shared validator semantics and failure model.

### Storage Enforcement Layer (Phase 1)
- **Raw event storage guard**: Depends on [No-secret validator contract]

### Derived Artifact Layer (Phase 2)
- **T0/T1/T2 invariant enforcement**: Depends on [Raw event storage guard]
- **Export invariant enforcement**: Depends on [Raw event storage guard]

### Verification Layer (Phase 3)
- **Cross-table and export scans**: Depends on [T0/T1/T2 invariant enforcement, Export invariant enforcement]

---

## Development Phases

### Phase 0: Validation Contract
- Define what constitutes a storage violation.
- Decide reject-vs-sanitize behavior for last-chance enforcement.

### Phase 1: Storage Guard
- Patch `insert_raw_event` to validate serialized event bodies/attrs before write.
- Ensure errors do not echo sensitive data.

### Phase 2: Derived and Export Validation
- Validate T0/T1/T2 output text and export text before persistence/write.
- Fail closed on invariant violations.

### Phase 3: End-to-End Verification
- Add tests that seed risky inputs and confirm all durable surfaces remain clean.
- Add scan helpers for known compromise patterns.

---

## Test Strategy
- Storage-level tests attempting to insert crafted secret-bearing raw events directly.
- T0/T1/T2 derivation tests confirming no secret patterns appear in derived rows.
- Export tests asserting handshake/project-mind markdown never includes known secret markers.
- End-to-end scan tests covering `raw_events`, `compact_events_t0`, `observations_t1`, `reflections_t2`, and exported files.

---

## Risks and Mitigations
- **Risk**: False positives block valid content.
  - **Mitigation**: Use targeted rules with provider-specific fixtures and clear redaction semantics.
- **Risk**: Performance cost from repeated validation.
  - **Mitigation**: Reuse canonical validator logic and scope scans to text-bearing fields only.
- **Risk**: One derived writer path is missed.
  - **Mitigation**: Add exhaustive test coverage over every durable artifact class.
