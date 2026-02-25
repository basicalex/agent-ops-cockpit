<context>
# Overview

Task 108 defines the semantic observational-memory runtime for AOC Mind's background memory layer.

Current state:
- T0 compaction is deterministic and policy-driven.
- T1/T2 runtime exists with deterministic, non-LLM synthesis for correctness and fallback.
- Task/tag attribution and segment routing foundations are implemented.

Gap:
- The current T1/T2 outputs are structurally stable but not semantically inferred.
- To align with observational-memory architecture (Observer + Reflector), T1 and T2 must support model-based semantic inference while preserving deterministic fail-open behavior.

Primary objective for Task 108:
- Add a semantic background agentic layer (Observer and Reflector) using the OpenCode Zen inference provider, with strict guardrails and deterministic fallback.

Scope decision (explicit):
- In scope: Zen-backed semantic T1/T2.
- Out of scope for this task: Roam and Ouros adapters.

# Research Summary (Mastra OM + AOC adaptation)

Key findings from Mastra's public docs and source implementation:

1) Observer and Reflector are semantic model calls, not deterministic parsing.
2) Observation and reflection are token-threshold driven (message threshold for Observer, observation threshold for Reflector).
3) They use strict output contracts and parsing layers, with retry handling for malformed/degenerate outputs.
4) They include buffering/activation mechanics so context does not stall while long memory is processed.
5) Reflection has compression validation and escalating retry guidance.
6) The architecture keeps a stable, append-oriented memory prefix for better continuity and cacheability.

What this means for AOC Mind:
- Preserve AOC's deterministic T0 compaction and provenance.
- Introduce semantic inference only at T1/T2 via pluggable adapter contracts.
- Keep deterministic fallback as baseline system of record when provider path is unavailable or rejected by validation.

# Core Features

## Feature 1: Semantic Observer (T1)

What it does:
- Converts per-conversation T0 chunks into semantically inferred T1 observations.

Why it matters:
- Improves signal density and continuity quality over extractive deterministic text packing.

How it works (high-level):
- Input remains single-conversation only.
- Planner respects parser budgets (target and hard cap).
- Prompted Observer model returns structured T1 payloads.
- Output is validated, bounded, normalized, persisted with full provenance.
- On validation/provider failure, deterministic T1 synthesis is used for that batch.

## Feature 2: Semantic Reflector (T2)

What it does:
- Condenses and restructures T1 observations into higher-order reflections.

Why it matters:
- Prevents T1 accumulation from becoming noisy and enables better long-horizon recall.

How it works (high-level):
- T2 batches are grouped by active tag/workstream policy.
- Reflector inference runs with explicit compression targets and bounded output.
- Compression/quality checks decide accept/retry/fallback.

## Feature 3: Zen Provider Runtime Selection

What it does:
- Adds runtime-selectable Zen model profiles for Observer and Reflector.

Why it matters:
- Allows low-cost default operation and easy model swaps without code changes.

How it works (high-level):
- Default profile is low-cost (for example Minimax M2.5 class).
- Configuration can override model IDs and runtime limits per environment/tag.

## Feature 4: Guardrails + Fail-Open Fallback

What it does:
- Enforces timeout/token/cost/retry bounds and deterministic fallback behavior.

Why it matters:
- Prevents semantic mode from blocking ingestion and preserves operational reliability.

How it works (high-level):
- Bounded retries with deterministic retry policy.
- Validation gate for output schema and size.
- Immediate fallback to deterministic runtime for failed batches.
- Persist failure reason and provenance for observability.

# User Experience

Personas:
- Solo power developer using AOC Mind for long-running coding sessions.
- Maintainer/operator responsible for quality and reliability of background memory.

Key flows:
1) Conversation ingestion -> T0 compaction -> semantic T1 observation.
2) T1 accumulation over threshold -> semantic T2 reflection.
3) Provider fault or invalid output -> deterministic fallback without blocking.
4) Operator changes model profile -> runtime uses new profile on next cycle.

UX constraints:
- User-facing continuity must not degrade when provider path is unstable.
- Observations/reflections remain bounded and traceable.

</context>

<PRD>
# Technical Architecture

## System Components

1) `aoc-opencode-adapter`
- Source of local conversation events.
- Produces deterministic T0 lane and context snapshots.

2) `aoc-mind`
- Owns distillation orchestration (planner + runtime).
- Introduces provider interface and semantic execution path.

3) `aoc-task-attribution`
- Links generated artifacts to tasks with confidence/provenance.

4) `aoc-segment-routing`
- Routes artifacts to segments after generation.

5) Zen provider adapter (Task 108)
- Implements semantic Observer and Reflector calls.
- Applies provider-specific transport, timeout, and retry rules.

6) `aoc-storage`
- Persists artifacts, links, context, routes, and provider provenance metadata.

## Adapter Contracts (new)

Add provider-agnostic interfaces in `aoc-mind`:

- `ObserverAdapter::observe_t1(input) -> T1Result`
- `ReflectorAdapter::reflect_t2(input) -> T2Result`

Required properties:
- Deterministic input canonicalization and hashable payloads.
- Structured output contract (strict schema parse).
- Explicit failure kinds (`timeout`, `invalid_output`, `budget_exceeded`, `provider_error`).

## Runtime Mode Selection

Distillation runtime mode:
- `deterministic_only`
- `semantic_with_fallback` (default for Task 108)

Model profile config (example shape):
- observer model id
- reflector model id
- max input tokens
- max output tokens
- timeout ms
- max retries
- cost guardrails

Default profile:
- low-cost Zen model profile (e.g., Minimax M2.5 class), configurable.

## Data Model Extensions

Persist provider provenance per artifact:
- `provider_name` (e.g., `zen`)
- `model_id`
- `prompt_version`
- `input_hash`
- `output_hash`
- `latency_ms`
- `attempt_count`
- `fallback_used` (bool)
- `fallback_reason` (nullable string)

Notes:
- Raw and T0 remain authoritative provenance lanes.
- T1/T2 store semantic output plus trace IDs to source T0/T1 inputs.

## Inference and Validation Pipeline

1) Planner selects batch/chunk (single conversation for T1).
2) Canonicalize input and compute input hash.
3) Execute adapter with guardrails.
4) Parse and validate structured output.
5) Enforce output bounds and compression quality checks.
6) Persist semantic artifact with provenance.
7) If any step fails and mode allows fallback -> run deterministic synthesis and persist fallback metadata.

## Policy Rules (must hold)

- T1 never mixes multiple conversations.
- T1 under target budget runs single pass.
- T1 over target uses deterministic chunk ordering within the same conversation only.
- T2 may aggregate across conversations only when same active tag/workstream policy allows.
- Cross-tag T2 mixing remains disabled by default.

## Security and Privacy

- Apply existing redaction policies before provider call.
- Never send raw unbounded tool outputs when policy says stripped.
- Log provider metadata, not sensitive raw payload content.

## Performance Requirements

- Provider path must be bounded by timeout and retry caps.
- Fallback path must keep end-to-end progress without operator intervention.
- No pipeline stall if provider is unavailable.

# Development Roadmap

## Phase 108.1 - Interface + Contracts

- Define `ObserverAdapter` and `ReflectorAdapter`.
- Add strict input/output DTOs and validation.
- Add runtime switch: deterministic vs semantic-with-fallback.

Deliverable:
- Semantic contract layer in `aoc-mind` with deterministic compatibility.

## Phase 108.2 - Zen Adapter Implementation

- Implement Zen-backed Observer and Reflector adapter.
- Add prompt templates and versioning.
- Implement canonical request/response normalization.

Deliverable:
- Runnable semantic T1/T2 via Zen for controlled fixtures.

## Phase 108.3 - Model Profile and Configuration

- Add config/env-driven model selection.
- Set low-cost default profile.
- Support separate Observer and Reflector model IDs.

Deliverable:
- Zero-code model swap capability.

## Phase 108.4 - Guardrails + Fallback

- Add timeout, retry, token and cost limits.
- Add output validation failure handling.
- Route failures to deterministic fallback and persist reason.

Deliverable:
- Safe fail-open semantic pipeline.

## Phase 108.5 - Test and Verification

- Add mock Zen fixtures for deterministic testability.
- Add golden tests for output bounds and schema.
- Add regression tests for no cross-conversation T1 mixing and deterministic chunk ordering.

Deliverable:
- Reliable semantic OM test suite with fallback assertions.

# Logical Dependency Chain

1) Contracts first (108.1)
2) Provider implementation (108.2)
3) Runtime model selection (108.3)
4) Guardrails/fallback (108.4)
5) Full fixture matrix (108.5)

Reasoning:
- This sequence prevents provider-specific coupling and keeps fallback safe from day one.

# Requirements

## Functional Requirements

FR-1: System SHALL run semantic T1 Observer via Zen when mode is semantic-with-fallback.

FR-2: System SHALL run semantic T2 Reflector via Zen when trigger policy is met.

FR-3: System SHALL validate semantic outputs against strict parseable schema before persist.

FR-4: System SHALL fall back to deterministic T1/T2 when semantic path fails validation or provider guardrails.

FR-5: System SHALL persist provenance and fallback metadata per generated artifact.

FR-6: System SHALL keep T1 one-conversation-per-pass invariant.

FR-7: System SHALL keep T2 tag/workstream boundary policy as configured.

## Non-Functional Requirements

NFR-1: Semantic calls SHALL respect configured timeout and max retry limits.

NFR-2: Distillation pipeline SHALL remain non-blocking at system level (fail-open).

NFR-3: Output size SHALL remain bounded by configured max chars/tokens.

NFR-4: All persisted artifacts SHALL include reproducibility metadata.

# Acceptance Criteria

1) Semantic Observer and Reflector both execute through Zen adapter in integration tests.
2) Invalid or degenerate semantic output triggers deterministic fallback and records reason.
3) Under-budget conversation produces single semantic T1 pass.
4) Over-budget conversation produces deterministic intra-conversation chunk sequence.
5) T2 reflection respects tag/workstream policy and remains bounded.
6) Model profile can be changed via config/env without code modification.
7) Provenance fields (`provider/model/prompt/input-hash/output-hash/fallback`) are persisted and queryable.

# Test Strategy

Unit:
- Prompt/input canonicalization hash stability.
- Schema parser/validator acceptance and rejection cases.
- Guardrail decision logic (retry/fallback conditions).

Integration:
- `ingest -> T0 -> semantic T1 -> semantic T2 -> attribution -> routing` fixture flow.
- Provider failure paths (`timeout`, malformed output, empty output).
- Deterministic fallback equivalence checks.

Regression:
- No cross-conversation T1 mixing.
- Deterministic chunk ordering remains stable across reruns.
- Cross-tag T2 disabled by default unless policy toggled.

Operational checks:
- Verify latency budget compliance.
- Verify fallback rate metric emission and persistence.

# Success Metrics

- Semantic path success rate above target in normal conditions.
- Fallback path success at 100% for forced provider-failure fixtures.
- No data-loss incidents in T1/T2 artifact generation.
- Deterministic rerun stability for planning/chunking invariants.

# Risks and Mitigations

Risk 1: Provider output drift or malformed payloads.
- Mitigation: strict parser, bounded retries, deterministic fallback.

Risk 2: Cost or latency spikes under heavy load.
- Mitigation: low-cost default model, hard timeouts, per-run budgets, profile overrides.

Risk 3: Semantic over-compression removing important details.
- Mitigation: validation checks, compression targets, prompt versioning, test fixtures with expected retention.

Risk 4: Policy violations (cross-conversation T1 or cross-tag T2).
- Mitigation: enforce planner invariants before inference and before persist.

# Appendix

## References

- Mastra OM docs: https://mastra.ai/docs/memory/observational-memory
- Mastra research article: https://mastra.ai/research/observational-memory
- Mastra source (observer):
  https://raw.githubusercontent.com/mastra-ai/mastra/main/packages/memory/src/processors/observational-memory/observer-agent.ts
- Mastra source (reflector):
  https://raw.githubusercontent.com/mastra-ai/mastra/main/packages/memory/src/processors/observational-memory/reflector-agent.ts
- Mastra source (runtime orchestration):
  https://raw.githubusercontent.com/mastra-ai/mastra/main/packages/memory/src/processors/observational-memory/observational-memory.ts

## Explicit Task-108 Deferrals

- Roam adapter integration (deferred).
- Ouros adapter integration (deferred).

</PRD>
