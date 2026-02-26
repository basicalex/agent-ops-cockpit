<context>
# Overview

Task 108 defines the semantic observational-memory runtime for AOC Mind's background layer.

Baseline already delivered:
- Deterministic T0 compaction and ingest checkpoints.
- Deterministic T1/T2 fallback distiller.
- Task/tag attribution and segment routing contracts.

Re-scoped strategy for production:
- Use **Pi-native inference runtime** (existing Pi OAuth/provider setup) instead of making an external provider stack the primary path.
- Run **one T1 Observer sidecar per active Pi session**.
- Run **one singleton T2 Reflector worker per project/session scope** (detached), triggered by observation thresholds.
- Keep deterministic distillation as authoritative fail-open baseline.

This keeps quality scaling while preserving reliability and operator control.

## Scope

In scope for Task 108:
1. Session-scoped semantic T1 observer sidecars.
2. Singleton semantic T2 reflector runtime.
3. Lock/lease coordination preventing duplicate reflection passes.
4. Pi-provider model profile selection (low-cost defaults).
5. Timeout/budget/retry guardrails + deterministic fallback.
6. Provenance and failure metadata persistence.

Out of scope for Task 108:
- Autonomous multi-agent swarms.
- Multiple concurrent reflector workers writing the same workstream.
- Mandatory external provider dependencies.
- Specialist role UX (Scout/Planner/Builder/Reviewer/Documenter/Red Team) orchestration logic beyond memory runtime contracts.

## Product/UX Intent

- Users continue normal Pi sessions.
- T1 observer runs in background with bounded cadence and no interaction blocking.
- T2 reflector runs detached and only when threshold policy triggers.
- If semantic path fails, users still get deterministic artifacts and uninterrupted workflow.

</context>

<PRD>
# Technical Architecture

## System Components

1) `aoc-opencode-adapter`
- Source of local conversation events.
- Produces deterministic T0 compact lane and checkpoints.

2) `aoc-mind`
- Distillation planner + runtime orchestration.
- Owns Observer/Reflector adapter contracts.
- Manages queueing and trigger policy.

3) `pi` background observer sidecar (new runtime path)
- Spawned from active Pi sessions.
- Performs semantic T1 inference on eligible conversation chunks.

4) singleton reflector worker (new runtime path)
- Detached process.
- Claims lock/lease and executes semantic T2 reflection jobs.

5) `aoc-storage`
- Persists artifacts, attribution links, context state, checkpoints.
- Persists provenance metadata and lock/lease/job state for recovery.

6) Optional enhancer adapters (deferred priority)
- External adapters may remain additive later, but are not required for this phase.

## Runtime Topology

### T1 Observer (per active Pi session)
- Trigger source: session events (`turn_end`/batch thresholds).
- Scope: one conversation per pass (same invariants as deterministic planner).
- Execution: low-cost model profile via Pi runtime.
- Behavior: debounced queue; max one active observer execution per session.

### T2 Reflector (singleton detached worker)
- Trigger source: accumulated T1 observations crossing threshold per tag/workstream.
- Scope: can aggregate multiple conversations only within same tag/workstream policy.
- Execution: detached worker process with lease heartbeats.
- Behavior: only one active reflector owner for a project/session scope at a time.

## Locking & Coordination Model

Use a two-layer safety model:

1. **Advisory file lock**
- Path: `.aoc/mind/locks/reflector.lock`
- Contains owner id, pid, started/heartbeat timestamps, TTL.
- Fast path to prevent accidental duplicate startup.

2. **Durable DB lease/job claiming**
- Lease record confirms runtime ownership with heartbeat/expiry.
- Reflection jobs are atomically claimed (single consumer semantics).
- Stale owner takeover allowed only after TTL expiry.

This combination prevents duplicate reflection writes even if one lock layer fails.

## Adapter Contracts

Provider-agnostic interfaces in `aoc-mind`:

- `ObserverAdapter::observe_t1(input) -> T1Result`
- `ReflectorAdapter::reflect_t2(input) -> T2Result`

Required properties:
- Deterministic canonicalization of inputs.
- Stable input hashes.
- Strict output schema parse/validation.
- Explicit failure kinds (`timeout`, `invalid_output`, `budget_exceeded`, `provider_error`, `lock_conflict`).

## Runtime Mode Selection

Modes:
- `deterministic_only`
- `semantic_with_fallback` (default target mode)

Semantic provider selection:
- Primary runtime: Pi provider/model config (inherits existing OAuth/provider setup).
- Per-stage profiles: separate observer and reflector model ids.
- Default profile: low-cost model class for background cadence.

Config knobs:
- max input/output tokens
- timeout ms
- max retries
- max budget tokens/cost
- queue debounce interval
- reflector lease TTL

## Data Model Extensions

Persist semantic provenance and runtime outcomes per artifact:
- runtime (`deterministic` | `pi-semantic` | `external-semantic`)
- provider_name
- model_id
- prompt_version
- input_hash
- output_hash
- latency_ms
- attempt_count
- fallback_used
- fallback_reason

Persist coordination state (lease/job metadata) for singleton behavior and crash recovery.

## Processing Pipeline

1. Ingest events and produce T0 compact transcript.
2. Planner determines T1 batch/chunk boundaries.
3. Session observer executes semantic T1 with guardrails.
4. Validate, bound, and persist T1 artifacts + provenance.
5. If threshold reached, enqueue T2 workstream jobs.
6. Singleton reflector claims lease + job, executes semantic T2.
7. Validate, bound, and persist T2 artifacts + provenance.
8. On any semantic failure, fallback to deterministic output and persist fallback reason.

## Invariants and Policy Rules

- T1 never mixes multiple conversations.
- T1 under target budget runs single pass.
- T1 over budget uses deterministic intra-conversation chunk ordering.
- T2 cross-conversation synthesis only within same tag/workstream policy.
- Cross-tag T2 mixing remains disabled by default.
- At most one reflector owner active for a given project scope.

## Security & Privacy

- Apply redaction before semantic calls.
- Never send stripped raw tool output unless allowlisted by policy.
- Store metadata/provenance; avoid persisting sensitive raw provider payloads.

## Performance & Reliability

- Observer sidecars must be non-blocking for interactive user turns.
- Reflector worker bounded by timeout/retry/budget limits.
- Fail-open fallback guarantees progress without operator intervention.
- Crash/restart recovery preserves idempotence via hashes and leases.

# Development Roadmap

## Phase 108.1 - Contracts + Canonicalization
- Finalize Observer/Reflector adapter DTOs.
- Finalize canonical payload hashing.
- Keep deterministic compatibility path.

## Phase 108.2 - Session Observer Sidecar (T1)
- Implement per-session observer queue + debounce.
- Spawn Pi-based semantic T1 runtime with low-cost default model profile.
- Enforce one active observer execution per session.

## Phase 108.3 - Singleton Reflector Worker (T2)
- Implement detached reflector process.
- Add file lock + DB lease + atomic job claim.
- Add stale-lease recovery and takeover rules.

## Phase 108.4 - Guardrails + Fallback + Provenance
- Add timeout/retry/token/cost limits.
- Add strict output validation and rejection handling.
- Persist fallback reasons and runtime provenance.

## Phase 108.5 - End-to-End Verification
- Add multi-session concurrency fixtures.
- Add lock-contention and stale-lease recovery tests.
- Add semantic failure fixtures verifying deterministic fallback.

# Requirements

## Functional Requirements

FR-1: System SHALL run semantic T1 Observer per active Pi session in semantic mode.

FR-2: System SHALL run semantic T2 Reflector through a singleton detached worker.

FR-3: System SHALL prevent concurrent reflector ownership for the same project scope.

FR-4: System SHALL validate semantic outputs before persistence.

FR-5: System SHALL fall back to deterministic runtime on semantic failure.

FR-6: System SHALL persist provenance/fallback metadata for every semantic attempt.

FR-7: System SHALL preserve T1/T2 scope invariants (no cross-conversation T1 mixing, policy-bounded T2 aggregation).

## Non-Functional Requirements

NFR-1: Background semantic processing SHALL not block interactive Pi turns.

NFR-2: Runtime SHALL enforce configured timeout/retry/budget limits.

NFR-3: Lock/lease coordination SHALL recover from crashed owners via TTL expiry.

NFR-4: Reruns SHALL remain deterministic for planner/chunking invariants and idempotent artifact persistence.

# Acceptance Criteria

1) T1 semantic observer runs from active Pi sessions with bounded queueing.
2) T2 semantic reflector runs as singleton; duplicate concurrent runs are prevented.
3) Lock contention tests prove no double-processing of the same reflection job.
4) Provider or schema failures trigger deterministic fallback with persisted reason.
5) Under-budget T1 remains single-pass; over-budget T1 chunking remains deterministic.
6) Model/profile selection uses Pi provider configuration without code edits.
7) Provenance fields are queryable for both success and fallback artifacts.

# Test Strategy

Unit:
- Input canonicalization and hash stability.
- Lock/lease TTL behavior and stale-owner takeover.
- Guardrail decision logic and failure classification.

Integration:
- `ingest -> T0 -> semantic T1 -> semantic T2 -> attribution -> routing` across multiple simulated sessions.
- Reflector singleton acquisition with forced contention.
- Provider failures (`timeout`, malformed output, budget exceed) with deterministic fallback assertions.

Regression:
- No cross-conversation T1 mixing.
- Deterministic chunk ordering for over-budget conversations.
- Cross-tag T2 disabled unless policy explicitly enables it.

Operational:
- Background queue latency within configured budget.
- Reflector lock health and recovery path validated after forced crash.

# Success Metrics

- Semantic path success rate meets target under normal conditions.
- Forced-failure fixtures achieve 100% deterministic fallback completion.
- Zero duplicate reflection writes under lock-contention tests.
- No interactive-turn blocking incidents from observer sidecars.

# Risks and Mitigations

Risk: Too many concurrent observer sidecars increase local resource contention.
- Mitigation: per-session debounce + max concurrency controls + low-cost model defaults.

Risk: Lock drift causes orphaned reflector ownership.
- Mitigation: heartbeat TTL + stale-owner takeover + durable job claiming.

Risk: Semantic over-compression reduces retrieval quality.
- Mitigation: schema checks, compression bounds, prompt versioning, golden fixtures.

Risk: Provider instability/cost spikes.
- Mitigation: hard budgets/timeouts and deterministic fallback baseline.

# Explicit Deferrals (Task 108)

- Multi-reflector parallel fan-out per project.
- Autonomous specialist-team orchestration.
- Mandatory external provider stack.

</PRD>
