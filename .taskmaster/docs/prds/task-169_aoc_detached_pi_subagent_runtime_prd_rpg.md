# AOC Detached Pi Subagent Runtime PRD (RPG)

## Problem Statement
AOC already has seeded specialist definitions under `.pi/agents/*`, team/chain manifests, Mission Control orchestration entry points, and wrapper-side `insight_dispatch` scaffolding. But detached agent execution is not yet a first-class, repo-controlled runtime substrate.

Current gaps:
- No project-local Pi extension actually loads canonical `.pi/agents/*.md` and turns them into executable detached/background workers.
- Current wrapper orchestration is gated behind `AOC_INSIGHT_AGENT_CMD`, so real subprocess execution is configuration-dependent rather than part of the AOC runtime contract.
- There is no durable detached job lifecycle model for queued/running/completed/failed/cancelled sub-agents with AOC-native telemetry.
- Mission Control and Pulse expose some orchestration entry points, but there is no canonical detached sub-agent status surface, cancellation flow, or result pickup contract.
- Third-party subagent packages may offer similar UX, but they do not guarantee alignment with AOC scope boundaries, provenance requirements, fail-open behavior, or canonical `.pi/agents/*` semantics.

We need an AOC-native detached Pi subagent runtime substrate that keeps `.pi/agents/*` as the source of truth for delegated specialist agents, preserves full control over lifecycle and telemetry, and makes detached execution reliable and observable. This runtime should be treated as the shared control-plane foundation for detached agent lifecycles, while delegated specialist subagents remain the first product target and Mind/T1/T2/T3 workers may later reuse selected lifecycle, telemetry, and provenance contracts without collapsing the two operating modes into one UX model.

## Target Users
- AOC maintainers implementing detached agent orchestration and control-plane telemetry inside Pi and Mission Control.
- Operators who need to dispatch delegated specialist agents in detached/background mode while continuing primary session work.
- Advanced contributors who want canonical agent/team/chain manifests to execute consistently across repos without third-party package assumptions.
- AOC runtime contributors who need a reusable detached lifecycle/provenance substrate that Mind workers can later consume without inheriting delegated-subagent UX semantics.

## Success Metrics
- 100% of delegated detached sub-agent runs resolve from canonical `.pi/agents/*.md`, `teams.yaml`, and `agent-chain.yaml`.
- Detached jobs expose explicit lifecycle states: `queued`, `running`, `success`, `fallback`, `error`, `cancelled`.
- Operators can launch, inspect, and cancel detached delegated jobs from at least one canonical surface with no hidden mutation path.
- Sub-agent outputs preserve provenance and agent identity for all completed/fallback runs.
- The detached runtime contract is reusable by non-delegated worker planes for lifecycle/telemetry purposes without forcing those planes into delegated-subagent promotion/report UX.
- Core orchestration continues to fail open deterministically when subprocess spawning, manifests, or provider execution fail.

---

## Architectural Framing
This PRD defines the **detached agent runtime substrate** with an initial and primary product focus on **delegated specialist subagents** such as scout/test/review and explicit expert chains.

Two operating modes are intentionally distinguished:
- **Delegated specialist subagents**: operator- or main-agent-invoked helpers that return reports into the current workstream and may later be promoted into richer operator-visible surfaces.
- **Mind workers**: detached T1/T2/T3 or related cognition-pipeline workers that are primarily queue/store/artifact-oriented and should reuse lifecycle/telemetry concepts where helpful without being forced into delegated-subagent UX or ownership semantics.

This PRD therefore owns:
- detached lifecycle/control-plane primitives,
- canonical delegated specialist orchestration via `.pi/agents/*`, and
- observability patterns for delegated jobs.

It does **not** redefine the full Mind worker product model; Mind-specific trigger semantics, queue ownership, and Mind-surface behavior remain separate concerns even when they reuse contracts from this runtime substrate.

## Capability Tree

### Capability: Canonical Agent Resolution
Load and validate AOC-owned agent definitions and orchestration manifests.

#### Feature: Agent markdown resolution
- **Description**: Resolve agent definitions from `.pi/agents/*.md` using frontmatter and body as canonical runtime input.
- **Inputs**: project-local agent markdown files, frontmatter fields, current project root.
- **Outputs**: validated agent config with name, description, tools, optional model, prompt body, and source path.
- **Behavior**: reject malformed manifests, preserve deterministic ordering, and normalize tool/model metadata for dispatch.

#### Feature: Team and chain manifest resolution
- **Description**: Resolve `teams.yaml` and `agent-chain.yaml` into executable orchestration plans.
- **Inputs**: `.pi/agents/teams.yaml`, `.pi/agents/agent-chain.yaml`.
- **Outputs**: validated team fanout definitions and sequential chain definitions.
- **Behavior**: ensure referenced agents exist, surface validation errors clearly, and preserve AOC chain placeholder semantics.

#### Feature: Scope and policy validation
- **Description**: Enforce AOC dispatch policy before sub-agent execution begins.
- **Inputs**: selected agent/team/chain, active tag/session/project context, runtime policy.
- **Outputs**: allow/deny decision with normalized execution policy.
- **Behavior**: enforce tool bounds, scope/tag boundaries, and repo-local trust assumptions.

### Capability: Detached Subagent Execution
Run isolated Pi subprocess workers under an AOC-owned lifecycle model.

#### Feature: Single detached dispatch
- **Description**: Start one detached worker for a specific agent/task pair.
- **Inputs**: agent config, task prompt, cwd/project root, execution policy.
- **Outputs**: job id, queued/running status, result stream metadata.
- **Behavior**: spawn an isolated `pi` subprocess, capture output incrementally, and return immediately to the caller.

#### Feature: Chain execution
- **Description**: Run sequential detached steps with prior output forwarded into later prompts.
- **Inputs**: chain definition, original input, per-step agent policies.
- **Outputs**: ordered step results plus aggregate job summary.
- **Behavior**: maintain step ordering, support `$INPUT`/`$ORIGINAL` placeholders, and stop or degrade predictably on failure.

#### Feature: Parallel execution
- **Description**: Run multiple detached specialists concurrently and aggregate results.
- **Inputs**: team definition or explicit fanout set, shared input, concurrency limits.
- **Outputs**: per-agent statuses plus all-settled aggregate summary.
- **Behavior**: bound concurrency, tolerate partial failure, and preserve per-agent provenance.

#### Feature: Cancellation and timeout control
- **Description**: Allow operators or runtime policy to stop detached work safely.
- **Inputs**: job id, process handle, timeout/abort request.
- **Outputs**: cancelled lifecycle state and terminal cleanup result.
- **Behavior**: propagate termination to subprocesses, update telemetry, and avoid orphaned workers.

### Capability: Detached Job State and Telemetry
Persist and expose detached job lifecycle and output metadata.

#### Feature: Job registry
- **Description**: Track active and recent detached jobs in a durable, queryable registry.
- **Inputs**: job create/start/update/finish events.
- **Outputs**: stored job records with status, timestamps, agent identity, execution mode, and summary refs.
- **Behavior**: support restart-safe state reconstruction for recent jobs and bounded retention.

#### Feature: Stream and result capture
- **Description**: Collect partial output, terminal summaries, stderr, and usage metadata from each detached worker.
- **Inputs**: subprocess JSON/RPC/print events, tool calls, assistant output, exit status.
- **Outputs**: display-ready stream items and terminal result bundle.
- **Behavior**: truncate safely, preserve important errors, and retain enough detail for Mission Control and Pi UI drilldown.

#### Feature: Provenance envelope
- **Description**: Attach provenance to each detached run and step.
- **Inputs**: agent identity, source manifest path, prompt payload refs, outputs, fallback reasons.
- **Outputs**: provenance-ready metadata for AOC surfaces and later memory/report integration.
- **Behavior**: distinguish success/fallback/error cleanly and never hide degraded execution.

### Capability: AOC Surface Integration
Make detached delegated sub-agents visible and controllable from canonical AOC surfaces, while keeping the underlying lifecycle contract reusable by other detached worker planes.

#### Feature: Pi extension tool and commands
- **Description**: Provide project-local Pi extension entry points for dispatch, status, and cancellation.
- **Inputs**: user or model calls, detached orchestration args.
- **Outputs**: immediate job ack plus readable status/result views.
- **Behavior**: integrate with Pi extension APIs rather than requiring external packages.

#### Feature: Mission Control and Pulse status surfacing
- **Description**: Publish detached job lifecycle into Pulse and Mission Control.
- **Inputs**: job registry updates and result summaries.
- **Outputs**: queue depth, active jobs, latest terminal states, and drilldown refs.
- **Behavior**: keep observability compact, low-noise, and operator-actionable.

#### Feature: Wrapper interoperability
- **Description**: Let existing wrapper-side orchestration surfaces consume the same detached runtime contract.
- **Inputs**: `insight_dispatch` / related wrapper requests.
- **Outputs**: consistent job creation, status lookup, and result retrieval behavior.
- **Behavior**: preserve fail-open compatibility while shifting runtime ownership toward canonical Pi extension behavior.

### Capability: Reliability and Safety
Keep detached sub-agent execution deterministic and policy-safe.

#### Feature: Fail-open fallback contract
- **Description**: Produce explicit fallback results when manifests, subprocesses, or providers fail.
- **Inputs**: validation failures, spawn failures, execution errors, timeouts.
- **Outputs**: structured fallback/error result with actionable explanation.
- **Behavior**: never silently drop jobs or hide degraded execution.

#### Feature: Resource and concurrency guardrails
- **Description**: Bound how many detached jobs can run and what resources they can consume.
- **Inputs**: concurrency settings, active jobs, model/tool policy.
- **Outputs**: admission decisions, queue behavior, throttling diagnostics.
- **Behavior**: prevent runaway fanout and maintain predictable terminal performance.

---

## Repository Structure

```text
project-root/
├── .pi/
│   ├── agents/
│   │   ├── insight-t1-observer.md
│   │   ├── insight-t2-reflector.md
│   │   ├── teams.yaml
│   │   └── agent-chain.yaml
│   └── extensions/
│       ├── minimal.ts
│       ├── themeMap.ts
│       └── subagent.ts                       # new canonical detached runtime extension
├── crates/
│   ├── aoc-core/
│   │   └── src/
│   │       ├── insight_contracts.rs
│   │       └── pulse_ipc.rs
│   ├── aoc-agent-wrap-rs/
│   │   └── src/
│   │       ├── main.rs
│   │       └── insight_orchestrator.rs
│   ├── aoc-hub-rs/
│   │   └── src/pulse_uds.rs
│   └── aoc-mission-control/
│       └── src/main.rs
├── docs/
│   ├── agents.md
│   ├── insight-subagent-orchestration.md
│   └── subagent-runtime.md                  # new operator/runtime reference
└── .taskmaster/docs/prds/
    └── task-169_aoc_detached_pi_subagent_runtime_prd_rpg.md
```

## Module Definitions

### Module: `.pi/extensions/subagent.ts`
- **Maps to capability**: Detached Subagent Execution + Pi extension integration
- **Responsibility**: canonical detached Pi subprocess orchestration, agent manifest loading, lifecycle updates, and user/model-facing commands/tools.
- **Exports**:
  - detached dispatch/status/cancel tools or commands
  - runtime state restoration helpers
  - stream/result rendering helpers

### Module: `.pi/agents/*.md` + `teams.yaml` + `agent-chain.yaml`
- **Maps to capability**: Canonical Agent Resolution
- **Responsibility**: canonical role, team, and chain definitions used by the detached runtime.
- **Exports**:
  - frontmatter + body contract for agents
  - team fanout map
  - chain execution definitions

### Module: `crates/aoc-core/src/insight_contracts.rs`
- **Maps to capability**: Wrapper interoperability + telemetry typing
- **Responsibility**: typed request/result/status contracts for detached orchestration surfaces.
- **Exports**:
  - detached job status/result envelopes
  - command parsing helpers
  - compatibility-safe orchestration enums

### Module: `crates/aoc-agent-wrap-rs/src/insight_orchestrator.rs`
- **Maps to capability**: Wrapper interoperability
- **Responsibility**: consume canonical detached-runtime contracts and preserve fallback behavior for wrapper-triggered orchestration.
- **Exports**:
  - dispatch bridge
  - status/result lookup helpers
  - fallback execution path for non-extension contexts

### Module: `crates/aoc-hub-rs/src/pulse_uds.rs`
- **Maps to capability**: Mission Control and Pulse status surfacing
- **Responsibility**: route detached sub-agent lifecycle topics and command envelopes.
- **Exports**:
  - Pulse routing for detached job state
  - lifecycle topic aggregation

### Module: `crates/aoc-mission-control/src/main.rs`
- **Maps to capability**: AOC surface integration
- **Responsibility**: render detached job queue/state/drilldown and support operator actions.
- **Exports**:
  - status presentation and dispatch actions inside Overseer/Mind-related views

### Module: `docs/subagent-runtime.md`
- **Maps to capability**: Reliability and operator rollout
- **Responsibility**: document runtime contract, lifecycle states, guardrails, and troubleshooting.

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies.
- **Canonical manifest contract**: `.pi/agents/*.md`, `teams.yaml`, `agent-chain.yaml` validation rules.
- **Detached runtime contract types**: typed job state, mode, result, and cancellation/status schemas.

### Extension Runtime Layer (Phase 1)
- **Project-local detached runtime extension**: Depends on [Canonical manifest contract, Detached runtime contract types].
- **Basic single-dispatch subprocess execution**: Depends on [Project-local detached runtime extension].

### Orchestration Modes Layer (Phase 2)
- **Chain execution**: Depends on [Basic single-dispatch subprocess execution, Canonical manifest contract].
- **Parallel execution**: Depends on [Basic single-dispatch subprocess execution, Canonical manifest contract].
- **Cancellation and timeout handling**: Depends on [Basic single-dispatch subprocess execution].

### Telemetry and Integration Layer (Phase 3)
- **Job registry and result capture**: Depends on [Chain execution, Parallel execution, Cancellation and timeout handling].
- **Pulse/Mission Control integration**: Depends on [Job registry and result capture, Detached runtime contract types].
- **Wrapper interoperability bridge**: Depends on [Detached runtime contract types, Job registry and result capture].

### Hardening and Rollout Layer (Phase 4)
- **Fallback and restart recovery**: Depends on [Job registry and result capture, Wrapper interoperability bridge].
- **Docs and seeded runtime rollout**: Depends on [Pulse/Mission Control integration, Fallback and restart recovery].
- **Regression coverage**: Depends on [all prior phases].

---

## Development Phases

### Phase 0: Detached Runtime Contract Baseline
**Goal**: define canonical runtime inputs, states, and outputs.

**Entry Criteria**: existing `.pi/agents/*` assets and current insight orchestration code are available.

**Tasks**:
- [ ] Define canonical agent/team/chain validation rules.
  - Acceptance criteria: malformed/missing references are rejected with actionable errors.
  - Test strategy: validation fixtures cover missing fields, unknown agents, and duplicate names.
- [ ] Define detached job state/result schemas.
  - Acceptance criteria: status, mode, and terminal result shapes are typed and reusable.
  - Test strategy: schema parse/serialize tests cover valid and invalid payloads.

**Exit Criteria**: manifest and lifecycle contracts are stable enough for extension implementation.

**Delivers**: explicit AOC-owned detached sub-agent contract.

---

### Phase 1: Project-Local Detached Runtime Extension
**Goal**: make single detached dispatch work from a canonical Pi extension.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Add `.pi/extensions/subagent.ts` with manifest loading and project-root policy handling.
- [ ] Implement single detached subprocess execution for one agent/task pair.
- [ ] Add immediate ack/status lookup surface for launched jobs.

**Exit Criteria**: one detached sub-agent can be launched, tracked, and queried locally.

**Delivers**: working Option B baseline.

---

### Phase 2: Chain, Parallel, and Control Flows
**Goal**: support full orchestration modes with bounded lifecycle control.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Implement chain execution with ordered step forwarding.
- [ ] Implement bounded parallel execution with all-settled aggregation.
- [ ] Implement cancellation and timeout handling.

**Exit Criteria**: single, chain, and parallel modes all run under one detached runtime model.

**Delivers**: canonical sub-agent orchestration modes.

---

### Phase 3: Telemetry and AOC Integration
**Goal**: expose detached sub-agent state and results across AOC surfaces.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Add job registry and result capture.
- [ ] Publish lifecycle updates to Pulse and Mission Control.
- [ ] Bridge wrapper `insight_dispatch` usage to the detached runtime contract.

**Exit Criteria**: operators can inspect detached job state/results from canonical AOC surfaces.

**Delivers**: observable detached sub-agent system.

---

### Phase 4: Hardening and Rollout
**Goal**: make detached sub-agents safe, restart-tolerant, and documented.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Add fallback and restart recovery behavior.
- [ ] Publish runtime/operator docs and rollout guidance.
- [ ] Add regression coverage for lifecycle, cancellation, and fallback paths.

**Exit Criteria**: detached sub-agent runtime is safe to ship as the canonical AOC path.

**Delivers**: production-ready detached sub-agent runtime.

---

## Test Strategy

## Test Pyramid

```text
        /\
       /E2E\       ← 15%
      /------\
     /Integration\ ← 35%
    /------------\
   /  Unit Tests  \ ← 50%
  /----------------\
```

## Coverage Requirements
- Line coverage: 85% minimum for new extension/runtime code
- Branch coverage: 75% minimum
- Function coverage: 90% minimum for lifecycle helpers
- Statement coverage: 85% minimum

## Critical Test Scenarios

### Canonical manifest resolution
**Happy path**:
- valid agents, teams, and chains load from `.pi/agents/*`.
- Expected: normalized configs resolve deterministically.

**Error cases**:
- unknown agent references, malformed frontmatter, invalid chain steps.
- Expected: actionable validation errors without partial silent execution.

### Detached single dispatch
**Happy path**:
- one detached worker starts, reports running, and completes successfully.
- Expected: job status transitions `queued -> running -> success` and captures output.

**Error cases**:
- spawn failure, bad model/tool config, subprocess non-zero exit.
- Expected: terminal fallback/error state with preserved stderr and provenance.

### Chain and parallel orchestration
**Happy path**:
- chain forwards prior output in order; parallel fanout aggregates multiple results.
- Expected: correct step ordering and all-settled summaries.

**Edge cases**:
- one branch fails while others succeed.
- Expected: aggregate result preserves partial completion and explicit degraded status.

### Cancellation and restart behavior
**Happy path**:
- operator cancels a running detached job.
- Expected: subprocess is terminated and job status becomes `cancelled`.

**Error cases**:
- extension/runtime reload while jobs are active.
- Expected: bounded recovery or explicit stale-job terminal state with no orphaned silent runners.

### AOC surface integration
**Happy path**:
- Mission Control and/or wrapper status views show active/recent detached jobs.
- Expected: queue depth, latest state, and drilldown refs are visible.

**Integration points**:
- wrapper `insight_dispatch` can consume the same detached job contract.
- Expected: no hidden alternate path or incompatible result shapes.

## Test Generation Guidelines
- Prefer deterministic subprocess fixtures or mocked pi child-process wrappers where possible.
- Keep lifecycle assertions explicit: queued, running, success/fallback/error/cancelled.
- Assert provenance fields on degraded runs.
- Add concurrency tests for bounded fanout and cancellation races.

---

## Architecture

## System Components
1. **Canonical Agent Assets**: `.pi/agents/*.md`, `teams.yaml`, `agent-chain.yaml`.
2. **Detached Runtime Extension**: `.pi/extensions/subagent.ts` manages manifests, subprocesses, lifecycle, and user/model entry points.
3. **Typed Contract Layer**: `aoc-core` structures detached job requests/results/status.
4. **Wrapper Bridge**: `aoc-agent-wrap-rs` interoperates with detached runtime contracts and fallback behavior.
5. **Pulse/Mission Control Surfaces**: lifecycle visibility, drilldown, and operator controls.
6. **Operator Docs**: runtime reference and rollout guidance.

## Data Models
- **DetachedSubagentJob**: `job_id`, `mode`, `agent_or_team`, `status`, `created_at`, `started_at`, `finished_at`, `cancelled_at`.
- **DetachedSubagentStepResult**: `job_id`, `step_id`, `agent`, `status`, `output_excerpt`, `stderr_excerpt`, `usage`, `fallback_used`.
- **DetachedSubagentPolicy**: `tools`, `model`, `cwd`, `scope_tag`, `allow_project_agents`, `timeout_ms`.
- **DetachedSubagentProvenance**: `agent_file`, `team_or_chain_manifest`, `prompt_ref`, `result_ref`, `failure_kind`.

## Technology Stack
- **TypeScript** for the Pi extension runtime and UI integration.
- **Rust** for typed contracts, wrapper bridging, and Mission Control/Pulse integration.
- **Pi subprocess execution** for isolation and compatibility with canonical agent prompts/tool limits.

**Decision: make the project-local Pi extension the canonical delegated-subagent runtime owner**
- **Rationale**: this keeps `.pi/agents/*` as the source of truth for delegated specialist agents and avoids third-party package lock-in while still establishing detached-runtime primitives that other worker planes can reuse.
- **Trade-offs**: more implementation work in-repo and more lifecycle code to maintain, plus the need to document where delegated UX ends and Mind-worker semantics begin.
- **Alternatives considered**: installing a third-party subagent package and adapting around it.

**Decision: preserve wrapper interoperability instead of replacing it outright**
- **Rationale**: existing `insight_dispatch` and Mission Control surfaces already exist and should evolve rather than fork.
- **Trade-offs**: dual-surface integration work during transition.
- **Alternatives considered**: deleting wrapper orchestration and moving everything into the extension immediately.

**Decision: require explicit detached lifecycle states and cancellation semantics**
- **Rationale**: background execution without observability is operationally unsafe.
- **Trade-offs**: extra state-management and testing burden.
- **Alternatives considered**: fire-and-forget subprocesses with only terminal summaries.

---

## Risks

## Technical Risks
**Risk**: detached extension lifecycle can diverge from wrapper/Mission Control expectations.
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: shared typed contracts and integration tests across surfaces.
- **Fallback**: retain wrapper fallback path while extension integration stabilizes.

**Risk**: subprocess management causes orphaned or stale detached jobs after reload/restart.
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: explicit job registry, PID tracking, timeout handling, and recovery policy.
- **Fallback**: mark jobs stale and require operator relaunch rather than guessing state.

## Dependency Risks
- Pi extension API behavior may change across Pi upgrades.
- Provider/model quirks can affect non-interactive detached runs.
- Mission Control integration may require incremental UI work beyond the extension baseline.

## Scope Risks
- Detached runtime, UI integration, and wrapper bridge can sprawl if delivered in one slice.
- Mitigation: phase delivery with a usable single-dispatch baseline first.

---

## References
- `docs/insight-subagent-orchestration.md`
- `docs/agents.md`
- Pi docs: `docs/extensions.md`
- Pi example: `examples/extensions/subagent/README.md`
- Current wrapper bridge: `crates/aoc-agent-wrap-rs/src/insight_orchestrator.rs`

## Glossary
- **Detached sub-agent**: a Pi subprocess worker that runs independently of the main agent turn after launch.
- **Canonical agent asset**: repo-owned definition under `.pi/agents/*`.
- **Fail-open**: explicit degraded/fallback result instead of silent failure or hidden mutation.

## Open Questions
- Should the initial detached runtime persist job state only in session/extension state, or also in a Rust-side durable store?
- Which surface should be canonical first for cancellation/status: Pi command/tool, Mission Control, or both?
- How much wrapper-side orchestration should remain once the project-local extension is authoritative?
