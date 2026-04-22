<context>
# Overview

Task 129 introduces a PI-first specialist-role interface with explicit developer control.

Background decisions:
- Background automation is limited to AOC Mind memory runtime (Observer/Reflector).
- Delivery/implementation specialists must be invoked actively by the developer.
- Goal is higher quality and throughput without autonomous swarm behavior.

Role set:
- Scout
- Planner
- Builder
- Reviewer
- Documenter
- Red Team

Guiding principle:
- **Developer is always pilot.**

</context>

<PRD>
# Technical Architecture

## Pi 0.62 Alignment Delta

Task 129 should now explicitly align role dispatch policy with Pi-native provenance and rendering support:
- use `sourceInfo` to classify whether a role-invoked tool/action came from Pi built-ins, project-local runtime extensions, or other extension surfaces
- use that provenance to drive trust tiers, allowlists, approval gates, and escalation rules
- treat `renderCall` / `renderResult` as compact operator UX improvements for role status/result surfaces, not as a replacement for the underlying response contract

## Capabilities

1) Active Role Dispatch
- Explicit commands/tools to invoke a role for a defined task.
- No autonomous background fan-out.

2) Role Contracts
- Per-role intent, scope, input format, output schema, and completion criteria.
- Role prompts/versioning in project-local `.pi/agents/`.

3) Policy Enforcement
- Per-role tool allowlists.
- Approval gate for writes/destructive actions/escalations.
- Budget/time limits per invocation.
- Provenance-aware trust policy using Pi `sourceInfo` to distinguish built-in, project-local, and extension-provided capabilities.

4) Context Integration
- Role invocation receives deterministic context pack slices from task/tag scope.
- Outputs include citations/provenance to artifacts/files.

5) Observability
- UI state for active role, status, elapsed time, usage/cost, and result summary.
- Transcript and run metadata persisted for auditability.

## Runtime Rules

- Dispatch is always explicit and user-visible.
- One invocation id per run; retries are explicit.
- Red Team and Reviewer default to read-first behavior.
- Builder write scope can be constrained to allowlisted paths.

## Output Contract

Each role response must include:
- `summary`
- `actions_taken`
- `risks`
- `next_recommended_step`
- `citations` (files/artifacts)

## Safety Model

- Approval checkpoints before write/destructive operations.
- Policy violations return structured refusal/escalation responses.
- Token/time budget overrun returns bounded partial result and status.

# Acceptance Criteria

1) All six roles are invokable explicitly through PI interface.
2) Role tool scopes are enforced by policy.
3) Write/destructive actions require approval gates.
4) Context pack slices are attached to invocations with deterministic bounds.
5) Operator telemetry (status/timing/usage) is visible during runs.
6) Role outputs satisfy response contract with citations.
7) Provenance-aware policy decisions can distinguish Pi built-ins from project-local/extension tools using `sourceInfo`.
8) Compact role status/result surfaces may use `renderCall` / `renderResult` without weakening the structured output contract.

# Test Strategy

Unit:
- Role contract validation and output schema checks.
- Policy gate/allowlist enforcement.

Integration:
- End-to-end dispatch for each role with realistic tasks.
- Approval gate scenarios (approve/deny/escalate).
- Budget timeout and cancellation behavior.

Regression:
- Ensure no autonomous dispatch occurs without explicit user action.
- Ensure role scope boundaries remain intact across updates.

# Dependencies

- Task 108 (semantic memory runtime)
- Task 109 (context-pack composer)

</PRD>
