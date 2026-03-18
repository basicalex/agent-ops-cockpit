# Mind Ingress Secret Redaction PRD (RPG)

## Problem Statement
Mind currently accepts raw tool output and other event payload text before applying any durable secret-safety boundary. This allows env dumps, bearer tokens, provider API keys, and other secret-bearing strings to enter `RawEvent` formation and be persisted downstream. The immediate failure mode is credential compromise; the longer-term failure mode is loss of trust in the provenance and reflection pipeline.

We need a single ingress redaction layer that makes secret persistence impossible by default, especially for tool output and shell-derived content.

## Target Users
- **Operators/developers** who need Mind features without risking credential leakage.
- **Mind pipeline maintainers** who need one canonical sanitization path instead of per-adapter patchwork.
- **Security reviewers** who need deterministic guarantees about secret handling.

## Success Metrics
- 0 known secret markers persist to `raw_events.payload_json` from Pi, Opencode, or direct Mind ingest flows.
- 100% of tool-result ingestion paths pass through one shared sanitizer.
- Tool-result outputs are dropped or redacted by default unless explicitly safe-listed.
- Regression tests cover known patterns such as `ANTHROPIC_AUTH_TOKEN=`, `ANTHROPIC_API_KEY=`, `Authorization: Bearer`, and `sk-or-v1-`.

---

## Capability Tree

### Capability: Canonical Ingress Sanitization
Own the single decision point for what text may enter Mind persistence.

#### Feature: Secret pattern detection
- **Description**: Detect env-style secrets, auth headers, private tokens, and other credential markers in strings.
- **Inputs**: Candidate text from message/tool-result/other payload fields.
- **Outputs**: Redaction decision plus sanitized text and reason metadata.
- **Behavior**: Apply deterministic provider-specific and generic secret rules with safe defaults.

#### Feature: Tool output default-drop policy
- **Description**: Prevent raw tool stdout/stderr from entering persistent Mind payloads unless explicitly allowed.
- **Inputs**: Tool result payload, tool name, output text.
- **Outputs**: Sanitized `ToolResultEvent` with output removed or redacted.
- **Behavior**: Drop risky outputs by default; preserve metadata such as tool name, status, exit code, and redaction marker.

### Capability: Recursive Event Scrubbing
Protect non-tool content paths that still carry free-form strings.

#### Feature: Message text redaction
- **Description**: Scrub secrets from user/assistant/system text before raw event persistence.
- **Inputs**: `MessageEvent.text`.
- **Outputs**: Sanitized text plus redaction provenance.
- **Behavior**: Redact only secret-bearing substrings while preserving as much semantic value as safe.

#### Feature: Generic payload traversal
- **Description**: Recursively sanitize `Other { payload }` and relevant attrs string values.
- **Inputs**: JSON payloads and string attrs.
- **Outputs**: Sanitized payload tree.
- **Behavior**: Traverse nested JSON and redact unsafe strings without breaking shape invariants.

### Capability: Redaction Provenance
Make secret handling observable without exposing the secret.

#### Feature: Redaction markers and reasons
- **Description**: Record whether content was dropped or redacted and why.
- **Inputs**: Sanitization decision.
- **Outputs**: `redacted=true` plus attrs/reason markers.
- **Behavior**: Preserve debugging value without storing sensitive source text.

---

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-core/
│   │   └── src/mind_contracts.rs or src/secret_sanitizer.rs
│   ├── aoc-pi-adapter/
│   │   └── src/lib.rs
│   ├── aoc-opencode-adapter/
│   │   └── src/lib.rs
│   └── aoc-agent-wrap-rs/
│       └── src/main.rs
└── crates/aoc-storage/
    └── src/lib.rs
```

## Module Definitions

### Module: shared secret sanitizer (`aoc-core` or equivalent)
- **Maps to capability**: Canonical Ingress Sanitization + Recursive Event Scrubbing
- **Responsibility**: Provide the canonical API for sanitizing raw events before persistence.
- **Exports**:
  - `sanitize_raw_event_for_mind(...)`
  - `sanitize_string_secret_content(...)`
  - `SecretRedactionDecision`

### Module: `crates/aoc-pi-adapter/src/lib.rs`
- **Maps to capability**: Tool output default-drop policy
- **Responsibility**: Route all normalized Pi events through the shared sanitizer before insert/compaction.

### Module: `crates/aoc-opencode-adapter/src/lib.rs`
- **Maps to capability**: Tool output default-drop policy
- **Responsibility**: Route Opencode parsed tool results through the shared sanitizer.

### Module: `crates/aoc-agent-wrap-rs/src/main.rs`
- **Maps to capability**: Redaction provenance
- **Responsibility**: Sanitize direct `mind_ingest_event` payloads server-side and never trust caller-supplied `redacted` flags.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **Secret rule set**: Canonical provider-specific and generic secret detection rules.
- **Sanitizer contract**: Stable API for sanitizing raw events and annotating redaction reasons.

### Adapter Integration Layer (Phase 1)
- **Pi ingestion wiring**: Depends on [Secret rule set, Sanitizer contract]
- **Opencode ingestion wiring**: Depends on [Secret rule set, Sanitizer contract]
- **Direct ingest wiring**: Depends on [Secret rule set, Sanitizer contract]

### Verification Layer (Phase 2)
- **Ingress regression coverage**: Depends on [Pi ingestion wiring, Opencode ingestion wiring, Direct ingest wiring]

---

## Development Phases

### Phase 0: Secret Sanitizer Contract
- Define the shared sanitizer API and redaction semantics.
- Lock the default policy that tool output is dropped/redacted unless safe-listed.

### Phase 1: Ingress Wiring
- Patch Pi adapter, Opencode adapter, and direct Mind ingest to sanitize before storage and compaction.
- Remove any trust in caller-provided redaction state.

### Phase 2: Regression Validation
- Add targeted tests for known secret markers and nested payload cases.
- Verify sanitized payloads preserve structure but not secret values.

---

## Test Strategy
- Unit tests for secret detection patterns and recursive JSON string scrubbing.
- Adapter tests showing secret-bearing session/tool output never survives into persisted raw events.
- Direct ingest tests proving caller-supplied `redacted=false` cannot bypass sanitization.
- Negative tests ensuring non-secret operational text remains usable after sanitization.

---

## Risks and Mitigations
- **Risk**: Over-redaction harms Mind usefulness.
  - **Mitigation**: Drop raw tool output by default but retain structured metadata and reason markers.
- **Risk**: New ingress path bypasses sanitizer.
  - **Mitigation**: Central API plus code search coverage and storage-layer backstop in companion tasks.
- **Risk**: Provider-specific token formats evolve.
  - **Mitigation**: Support both provider-specific and generic secret classes with extensible rules.
