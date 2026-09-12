# AOC Opt-in Search Stack with Managed SearXNG PRD (RPG)

## Problem Statement
AOC now has a strong browser automation primitive via `agent-browser`, but it still lacks a first-class programmatic web discovery layer for agents. In practice this means agents can interact with known pages well, yet broad research and documentation lookup still fall back to ad hoc CLI fetching, direct site guessing, or manual browser-first discovery.

Current gaps:
- `Alt+C -> Settings -> Tools` can install Agent Browser, but it cannot optionally provision a local search backend alongside it.
- There is no AOC-owned search capability that exposes a stable `status/start/query` contract to agents and subagents.
- Search availability is not encoded as managed capability metadata, so agents cannot reliably tell whether search is installed, configured, stopped, or healthy.
- Research-oriented agent flows are forced to choose between direct browser navigation and shell-based web scraping instead of doing the more efficient pattern: search first, browse second.
- Future subagents such as a scout/research specialist do not yet have a canonical way to auto-start local search or guide the developer to enable it through `Alt+C`.

We need an opt-in, AOC-managed local search stack based on SearXNG that integrates cleanly with existing Agent Browser setup, starts lazily when needed, and gives Pi/AOC agents a predictable structured-search primitive before they escalate into browser interaction.

## Target Users
- AOC developers who want a simple `Alt+C` flow to enable web agency beyond browser-only automation.
- Pi agents and future detached/scout subagents that need reliable structured search before opening pages.
- Maintainers extending AOC tool integrations, skills, and operator documentation.
- Developers who prefer local/self-hosted search over external proprietary search APIs.

## Success Metrics
- Developers can enable SearXNG during Agent Browser setup in `Alt+C` with an opt-in flow requiring no manual compose authoring.
- AOC persists enough search metadata for agents to determine whether search is installed, managed, stopped, or healthy.
- Pi/AOC agents can start a stopped managed SearXNG service on demand and reach a healthy query endpoint without manual Docker commands.
- AOC exposes a stable search wrapper/CLI that returns normalized structured results suitable for agent use.
- Research workflows prefer `search -> shortlist -> browse` when search is available, and degrade gracefully when it is not.
- Documentation clearly explains install, health, auto-start behavior, and how future research/scout agents consume the capability.

---

## Architectural Framing
This PRD treats **browser interaction** and **search discovery** as separate but complementary capabilities:

- `agent-browser` remains the browser automation primitive for navigation, DOM interaction, extraction, and testing.
- Managed SearXNG becomes the local search primitive for query-based web discovery.
- A higher-level research workflow may later orchestrate both, but phase 1 should not overload the existing browser skill with all search semantics.

This design keeps AOC modular:
- setup flows stay simple,
- runtime behavior stays predictable,
- agents get a clean AOC-owned contract,
- future providers can be added behind the same search interface later if needed.

## Capability Tree

### Capability: Opt-in Tooling Setup
Extend `Alt+C` tool integrations so developers can enable local search alongside Agent Browser.

#### Feature: Agent Browser + Search setup flow
- **Description**: Add an opt-in SearXNG install/provisioning path to the existing Agent Browser tooling experience.
- **Inputs**: current tool status, developer selection, Docker availability, optional install preferences.
- **Outputs**: configured local search capability or explicit skipped/not-configured state.
- **Behavior**: present search as optional, preserve browser-only installs, and keep the flow simple enough for routine developer setup.

#### Feature: Capability mode selection
- **Description**: Let developers choose how managed search starts and whether it should be treated as AOC-managed infra.
- **Inputs**: install choice, startup preference, local environment checks.
- **Outputs**: persisted mode such as `managed=true`, `auto_start=true`, `provider=searxng`.
- **Behavior**: default to lazy on-demand startup while allowing manual-only or immediate-start variants if AOC exposes them.

### Capability: Managed Search Infrastructure
Create and manage a minimal AOC-specific local SearXNG deployment.

#### Feature: Compose/env scaffolding
- **Description**: Generate an AOC-owned Docker Compose bundle and any companion env/config files for a minimal SearXNG instance.
- **Inputs**: target paths, chosen ports, provider defaults, AOC conventions.
- **Outputs**: compose/config artifacts under an AOC-managed project path.
- **Behavior**: produce reproducible files, avoid overwriting unrelated user infra, and keep the instance tuned for agentic structured search rather than general browsing UX.

#### Feature: Health and lifecycle management
- **Description**: Start, stop, and verify the managed search service reliably.
- **Inputs**: compose path, service name, base URL, health endpoint.
- **Outputs**: running/stopped/failed/healthy status.
- **Behavior**: use Docker Compose as the real control plane, treat lazydocker as optional operator visibility only, and surface actionable errors when the service cannot start.

### Capability: Search Capability Metadata
Persist enough information for agents and operators to reason about search availability.

#### Feature: Search config persistence
- **Description**: Store canonical AOC metadata describing the installed search provider and runtime contract.
- **Inputs**: provider choice, base URL, compose path, auto-start preference, health endpoint.
- **Outputs**: durable config entries discoverable by AOC tools and agents.
- **Behavior**: act as the source of truth for whether search is configured, managed, and eligible for lazy startup.

#### Feature: Search status inspection
- **Description**: Report whether search is disabled, unconfigured, stopped, starting, healthy, or degraded.
- **Inputs**: stored config, container state, health checks.
- **Outputs**: compact operator- and agent-readable status payload.
- **Behavior**: distinguish clearly between “not installed”, “installed but stopped”, and “running but unhealthy”.

### Capability: AOC Search Runtime Interface
Provide a stable agent-facing wrapper instead of exposing raw SearXNG and Docker details in prompts.

#### Feature: Search wrapper commands
- **Description**: Expose AOC-level `status`, `start`, and `query` operations for the search capability.
- **Inputs**: config metadata, query string, optional mode/filter arguments.
- **Outputs**: normalized status envelopes and structured result sets.
- **Behavior**: hide Docker/URL details, auto-start when allowed, and normalize SearXNG response fields for agent use.

#### Feature: Query normalization
- **Description**: Convert provider-specific response payloads into a stable AOC search schema.
- **Inputs**: SearXNG JSON search response.
- **Outputs**: objects containing at least title, URL, snippet/content, source/engine, and ranking/order metadata where available.
- **Behavior**: keep output compact, deterministic, and suitable for downstream browser follow-up.

### Capability: Agent and Skill Integration
Teach AOC skills and future subagents how to use search-first workflows.

#### Feature: Research workflow guidance
- **Description**: Document and encode the preferred `search -> browse -> extract` behavior for research tasks.
- **Inputs**: search availability, user request, result candidates.
- **Outputs**: guidance and/or skill logic for efficient research behavior.
- **Behavior**: prefer search when available, then use `agent-browser` to inspect shortlisted pages.

#### Feature: Auto-start and fallback rules
- **Description**: Allow Pi/AOC agents to start the managed search service when needed or guide the developer to enable it.
- **Inputs**: search config, runtime status, auto-start flag, task intent.
- **Outputs**: started service, user prompt, or graceful fallback path.
- **Behavior**: if configured and stopped, auto-start when allowed; if not configured, prompt the developer toward `Alt+C`; if unavailable, continue with browser/manual discovery when necessary.

#### Feature: Future scout/subagent compatibility
- **Description**: Ensure future research/scout specialists can depend on the same search capability contract.
- **Inputs**: search wrapper, capability metadata, runtime status.
- **Outputs**: reusable behavior for detached or specialized agents.
- **Behavior**: avoid embedding provider-specific details directly into specialist prompts or agent definitions.

---

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-control/
│   │   └── src/main.rs                     # Alt+C tool integration and install UX updates
│   ├── aoc-cli/
│   │   └── src/                            # optional search wrapper command plumbing if added in Rust
│   └── aoc-core/
│       └── src/                            # shared search config/status/result types if needed
├── bin/
│   ├── aoc-agent-install                   # existing installer surface patterns
│   ├── aoc-search                          # new wrapper for status/start/query (proposed)
│   └── aoc-research                        # optional later orchestration helper (future)
├── .aoc/
│   └── services/
│       └── searxng/
│           ├── docker-compose.yml          # managed local search service
│           ├── .env                        # generated runtime vars if needed
│           └── searxng-settings.yml        # minimal agent-oriented provider settings if needed
├── .pi/
│   └── skills/
│       ├── agent-browser/SKILL.md          # browser skill remains browser-focused; references search usage where helpful
│       └── research-workflow/SKILL.md      # new optional orchestration skill (proposed)
├── docs/
│   ├── configuration.md                    # config/env/Alt+C docs
│   ├── installation.md                     # setup flow docs
│   └── research/
│       └── web-agency-stack.md             # new operator/reference docs
└── .taskmaster/docs/prds/
    └── task-175_aoc_search_stack_prd_rpg.md
```

## Module Definitions

### Module: `crates/aoc-control/src/main.rs`
- **Maps to capability**: Opt-in Tooling Setup
- **Responsibility**: extend `Alt+C -> Settings -> Tools` so developers can install/configure search alongside Agent Browser.
- **Exports**:
  - tool integration menu entries
  - setup prompts and status summaries
  - action handlers for install/start/verify flows

### Module: `.aoc/services/searxng/*`
- **Maps to capability**: Managed Search Infrastructure
- **Responsibility**: AOC-owned local SearXNG deployment contract.
- **Exports**:
  - compose/service definition
  - provider/runtime configuration
  - health endpoint details

### Module: `bin/aoc-search` (or equivalent Rust CLI surface)
- **Maps to capability**: AOC Search Runtime Interface
- **Responsibility**: expose `status`, `start`, and `query` behavior to agents/operators.
- **Exports**:
  - `aoc-search status`
  - `aoc-search start`
  - `aoc-search query <text>`

### Module: search config/state handling (`crates/aoc-core` and/or supporting scripts)
- **Maps to capability**: Search Capability Metadata
- **Responsibility**: persist and parse canonical search provider/runtime metadata.
- **Exports**:
  - config readers/writers
  - typed status/result envelopes
  - health check helpers

### Module: `.pi/skills/agent-browser/SKILL.md`
- **Maps to capability**: Agent and Skill Integration
- **Responsibility**: remain the browser primitive while clarifying how search complements browser workflows.
- **Exports**:
  - browser automation guidance
  - optional references to search-first behavior for research tasks

### Module: `.pi/skills/research-workflow/SKILL.md` (new)
- **Maps to capability**: Research workflow guidance + future scout compatibility
- **Responsibility**: teach search-first, browse-second orchestration across search and browser capabilities.
- **Exports**:
  - research playbooks
  - fallback behavior when search is unavailable
  - future specialist/subagent conventions

---

## Dependency Chain

### Foundation Layer (Phase 0)
No dependencies - these are built first.

- **search-capability-contract**: canonical config, status vocabulary, query result schema, and health semantics.
- **managed-searxng-layout**: target file locations, compose contract, env/config conventions, and startup policy.

### Tooling Setup Layer (Phase 1)
- **altc-search-setup**: Depends on [search-capability-contract, managed-searxng-layout]
- **search-config-persistence**: Depends on [search-capability-contract, managed-searxng-layout]

### Runtime Interface Layer (Phase 2)
- **search-lifecycle-wrapper**: Depends on [search-capability-contract, managed-searxng-layout, search-config-persistence]
- **query-normalization**: Depends on [search-capability-contract, search-lifecycle-wrapper]

### Agent Integration Layer (Phase 3)
- **browser-skill-integration**: Depends on [query-normalization]
- **research-skill-orchestration**: Depends on [query-normalization, browser-skill-integration]
- **subagent-auto-start-rules**: Depends on [search-lifecycle-wrapper, research-skill-orchestration]

### Documentation Layer (Phase 4)
- **operator-and-dev-docs**: Depends on [altc-search-setup, search-lifecycle-wrapper, research-skill-orchestration]

---

## Development Phases

### Phase 0: Search Contract Foundation
**Goal**: define the canonical AOC search capability shape before wiring UI or agents to it.

**Entry Criteria**: task approved; current Agent Browser/Alt+C patterns understood.

**Tasks**:
- [ ] Define canonical search config/status/result schema (depends on: none)
  - Acceptance criteria: a single source of truth exists for provider, base URL, compose path, auto-start policy, and result fields.
  - Test strategy: unit tests for config parsing/serialization and status derivation.
- [ ] Define managed SearXNG deployment contract (depends on: none)
  - Acceptance criteria: compose path, service name, health endpoint, and minimal settings are specified.
  - Test strategy: fixture-based validation of generated compose/config artifacts.

**Exit Criteria**: AOC has a stable search contract independent of UI prompts or skill text.

**Delivers**: maintainers can reason about configured/unconfigured/stopped/healthy search deterministically.

---

### Phase 1: Alt+C Provisioning and Config Persistence
**Goal**: let developers opt into managed local search during the existing tools setup flow.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Extend `Alt+C -> Settings -> Tools` with optional SearXNG setup under Agent Browser/web tooling (depends on: [Define canonical search config/status/result schema, Define managed SearXNG deployment contract])
  - Acceptance criteria: the setup path supports browser-only and browser+search outcomes.
  - Test strategy: UI/action flow tests or targeted handler tests for each branch.
- [ ] Generate and persist managed search config/artifacts (depends on: [Define canonical search config/status/result schema, Define managed SearXNG deployment contract])
  - Acceptance criteria: AOC writes reproducible compose/config metadata without requiring manual authoring.
  - Test strategy: golden-file or fixture tests for generated output and saved config.

**Exit Criteria**: developers can enable managed SearXNG and AOC remembers the capability.

**Delivers**: a repository-local or user-scoped managed search installation state ready for runtime use.

---

### Phase 2: Runtime Start/Status/Query Interface
**Goal**: provide a stable AOC-owned wrapper so agents can consume search without raw Docker/SearXNG knowledge.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Implement `aoc-search status/start` lifecycle wrapper (depends on: [Generate and persist managed search config/artifacts])
  - Acceptance criteria: stopped managed services can be started, unhealthy services are reported clearly, and status distinguishes missing vs stopped vs healthy.
  - Test strategy: integration tests with mocked or local compose state plus health probe assertions.
- [ ] Implement normalized `aoc-search query` result output (depends on: [Implement `aoc-search status/start` lifecycle wrapper, Define canonical search config/status/result schema])
  - Acceptance criteria: structured result objects are returned from SearXNG JSON with stable field names and compact output.
  - Test strategy: fixture-based parsing/normalization tests and error-path coverage.

**Exit Criteria**: agents/operators can check status, start search, and run queries through a single AOC surface.

**Delivers**: a reusable search runtime interface suitable for skills and future subagents.

---

### Phase 3: Skill and Subagent Integration
**Goal**: teach research-capable agents to prefer search-first flows while preserving graceful degradation.

**Entry Criteria**: Phase 2 complete.

**Tasks**:
- [ ] Update `agent-browser` guidance to position search as complementary discovery, not a browser replacement (depends on: [Implement normalized `aoc-search query` result output])
  - Acceptance criteria: browser skill docs remain browser-focused but reference search-first research patterns where useful.
  - Test strategy: doc/skill review against desired scope boundaries.
- [ ] Add a new research workflow skill that uses `aoc-search` then `agent-browser` (depends on: [Implement normalized `aoc-search query` result output])
  - Acceptance criteria: the skill instructs agents to search, shortlist, browse, and fall back gracefully.
  - Test strategy: prompt/skill validation through representative research scenarios.
- [ ] Define scout/subagent auto-start and fallback behavior (depends on: [Implement `aoc-search status/start` lifecycle wrapper, Add a new research workflow skill that uses `aoc-search` then `agent-browser`])
  - Acceptance criteria: future specialists know when to auto-start managed search and when to prompt users toward `Alt+C`.
  - Test strategy: scenario tests for configured+stopped, unconfigured, and unhealthy cases.

**Exit Criteria**: AOC research workflows and future scout-style agents can rely on the shared search capability contract.

**Delivers**: search-first agentic research behavior without overloading the browser primitive.

---

### Phase 4: Documentation and Operator Guidance
**Goal**: document the new web agency stack clearly for installation, runtime operations, and future extensibility.

**Entry Criteria**: Phases 1-3 complete.

**Tasks**:
- [ ] Update installation/configuration docs for Alt+C search setup and environment/config overrides (depends on: [Extend `Alt+C -> Settings -> Tools` with optional SearXNG setup under Agent Browser/web tooling, Implement `aoc-search status/start` lifecycle wrapper])
  - Acceptance criteria: docs explain opt-in install, startup policy, and troubleshooting.
  - Test strategy: manual doc walkthrough against a fresh install flow.
- [ ] Add dedicated web research stack/operator docs (depends on: [Add a new research workflow skill that uses `aoc-search` then `agent-browser`, Define scout/subagent auto-start and fallback behavior])
  - Acceptance criteria: docs explain search-first vs browser-first behavior, lazy startup, and future scout usage.
  - Test strategy: documentation review for operator clarity and cross-link correctness.

**Exit Criteria**: contributors and users can install, operate, and extend the search stack without reverse-engineering code paths.

**Delivers**: complete operator-facing and contributor-facing guidance.

---

## Test Strategy

## Test Pyramid

```text
        /\
       /E2E\       ← 10% (Alt+C happy path, managed startup, end-to-end query)
      /------\
     /Integration\ ← 30% (compose lifecycle, health checks, query normalization, config persistence)
    /------------\
   /  Unit Tests  \ ← 60% (schema parsing, status derivation, result normalization, path generation)
  /----------------\
```

## Coverage Requirements
- Line coverage: 85% minimum for new search/runtime modules
- Branch coverage: 80% minimum
- Function coverage: 90% minimum
- Statement coverage: 85% minimum

## Critical Test Scenarios

### Search setup flow
**Happy path**:
- Developer opts into SearXNG during tools setup and AOC writes compose/config metadata.
- Expected: managed search becomes configured and discoverable.

**Edge cases**:
- Developer keeps browser-only install.
- Expected: no search artifacts are created and skills continue to work without search.

**Error cases**:
- Docker/Compose unavailable during setup.
- Expected: AOC surfaces a clear actionable message and does not leave partial/broken state.

**Integration points**:
- Alt+C action handlers, config persistence, generated compose layout.
- Expected: status surfaces reflect saved configuration correctly.

### Search lifecycle wrapper
**Happy path**:
- Managed search is configured but stopped; `aoc-search start` launches it and health passes.
- Expected: status transitions to healthy.

**Edge cases**:
- Search already running.
- Expected: start is idempotent and returns healthy without duplicate errors.

**Error cases**:
- Container starts but health endpoint fails.
- Expected: degraded/unhealthy state with troubleshooting details.

**Integration points**:
- Docker Compose execution, health checks, config-derived URLs.
- Expected: wrapper behavior remains deterministic.

### Query normalization
**Happy path**:
- SearXNG JSON returns regular search results.
- Expected: normalized AOC result objects include title, URL, snippet/content, and source metadata.

**Edge cases**:
- Empty result sets or missing snippet/source fields.
- Expected: output remains valid and compact.

**Error cases**:
- Non-JSON/HTTP failure from backend.
- Expected: explicit failure envelope rather than malformed output.

**Integration points**:
- runtime wrapper, downstream skill consumption.
- Expected: browser workflows can safely consume returned URLs.

### Skill/subagent behavior
**Happy path**:
- Research skill finds results through `aoc-search` and opens shortlisted URLs via `agent-browser`.
- Expected: search-first workflow is followed.

**Edge cases**:
- Search is unconfigured.
- Expected: skill prompts toward `Alt+C` or falls back gracefully.

**Error cases**:
- Search configured but unhealthy.
- Expected: agent reports degraded search and can continue with limited browser/manual discovery.

**Integration points**:
- search wrapper, agent-browser skill, future scout/subagent prompts.
- Expected: no provider-specific Docker details leak into normal agent prompts.

## Test Generation Guidelines
- Prefer fast unit coverage for config/status/result schema logic.
- Use integration fixtures for generated compose/config artifacts instead of brittle full-container tests where possible.
- Reserve end-to-end container startup tests for the main happy path and one failure path.
- Validate output shape stability because agents and future subagents depend on deterministic fields.

---

## Architecture

## System Components
- **Alt+C tooling integration**: setup and operator control surface for optional SearXNG provisioning.
- **Managed SearXNG service**: minimal local search backend running under AOC-managed Docker Compose.
- **Search config/state layer**: canonical metadata describing provider, health endpoint, and startup policy.
- **AOC search wrapper**: operator/agent-facing `status/start/query` contract.
- **Skill layer**: browser and research skills consuming the wrapper without provider-specific knowledge.

## Data Models
- **Search capability config**: `{ enabled, provider, managed, autoStart, baseUrl, composeFile, serviceName, healthcheckUrl }`
- **Search status envelope**: `{ configured, managed, runtimeStatus, healthy, message }`
- **Normalized search result**: `{ title, url, snippet, source, rank }`
- **Query response envelope**: `{ query, provider, results[], warnings[] }`

## Canonical Contract Proposal

### Config File
Primary phase-1 config lives at `.aoc/search.toml`.

Example:

```toml
version = 1

[search]
enabled = true
provider = "searxng"
managed = true
auto_start = true

[search.runtime]
base_url = "http://127.0.0.1:8888"
healthcheck_url = "http://127.0.0.1:8888/search?q=aoc+health&format=json"
compose_file = ".aoc/services/searxng/docker-compose.yml"
service_name = "searxng"

[search.query_defaults]
format = "json"
language = "en"
categories = "general"
safe_search = 0

[search.agent_policy]
allow_auto_start = true
prompt_when_missing = true
prompt_when_unhealthy = true
```

Semantics:
- Missing file means search is unconfigured.
- `enabled = false` means intentionally disabled.
- `managed = true` means AOC owns lifecycle via Docker Compose.
- `auto_start = true` means agents may lazily start the service before querying.

### Runtime Status Vocabulary
AOC should derive and expose one of the following states:
- `unconfigured`
- `disabled`
- `stopped`
- `starting`
- `healthy`
- `unhealthy`
- `error`

Example status envelope:

```json
{
  "configured": true,
  "enabled": true,
  "managed": true,
  "provider": "searxng",
  "runtimeStatus": "healthy",
  "healthy": true,
  "autoStart": true,
  "baseUrl": "http://127.0.0.1:8888",
  "message": "Managed SearXNG is running and healthy."
}
```

### Normalized Query Output
Agents must consume normalized AOC search results rather than raw provider payloads.

Example:

```json
{
  "query": "react useeffect docs",
  "provider": "searxng",
  "status": "ok",
  "results": [
    {
      "title": "Using the Effect Hook – React",
      "url": "https://react.dev/...",
      "snippet": "The Effect Hook lets you perform side effects...",
      "source": "google",
      "rank": 1
    }
  ],
  "warnings": []
}
```

Required result fields:
- `title`
- `url`
- `snippet`
- `source`
- `rank`

### Command Surface
Phase-1 wrapper surface should be exposed via `bin/aoc-search`.

Commands:
- `aoc-search status`
- `aoc-search status --json`
- `aoc-search start`
- `aoc-search start --wait`
- `aoc-search stop`
- `aoc-search health`
- `aoc-search query "<text>"`
- `aoc-search query --json "<text>"`
- `aoc-search query --mode docs --limit 5 "<text>"`

Expected behavior:
- `status` reads config and resolves runtime health.
- `start` launches managed search if configured.
- `query` checks config, auto-starts if allowed, verifies health, calls SearXNG JSON, and returns normalized results.

### Managed Deployment Layout
Phase-1 managed deployment files should live under `.aoc/services/searxng/`.

Proposed files:
- `.aoc/services/searxng/docker-compose.yml`
- `.aoc/services/searxng/settings.yml`
- `.aoc/services/searxng/.env` (optional)

Expected deployment defaults:
- bind locally on `127.0.0.1`
- default port `8888`
- minimal agent-oriented settings
- Docker Compose as the lifecycle primitive
- lazydocker as optional operator visibility only, not a required control plane

### Agent Auto-start and Fallback Rules
When an agent or subagent needs search:
1. Run `aoc-search status`.
2. If state is `healthy`, query immediately.
3. If state is `stopped` and auto-start is allowed, run `aoc-search start` then query.
4. If state is `unconfigured`, prompt the developer toward `Alt+C` setup.
5. If state is `unhealthy` or `error`, report degraded search and fall back to browser/manual discovery when needed.

## Technology Stack
- Rust for `aoc-control`/CLI integration where existing flows already live.
- Shell/bin wrappers where that keeps AOC command ergonomics simple.
- Docker Compose for local managed service lifecycle.
- SearXNG as the initial self-hosted structured search provider.
- PI skills for workflow guidance and future subagent orchestration.

**Decision: SearXNG as phase-1 provider**
- **Rationale**: mature enough, self-hostable, familiar operational model, and suitable JSON search interface for AOC-managed local use.
- **Trade-offs**: adds Docker/service lifecycle complexity and introduces another optional local dependency.
- **Alternatives considered**: OmniSearch as a possible future backend; browser-only/manual discovery; proprietary search APIs.

**Decision: Search as a separate capability from agent-browser**
- **Rationale**: keeps browser automation and search discovery modular while allowing orchestration at the skill layer.
- **Trade-offs**: introduces another wrapper/config surface instead of one monolithic browser skill.
- **Alternatives considered**: folding all research behavior directly into `agent-browser`.

**Decision: Lazy on-demand startup by default**
- **Rationale**: minimizes background resource usage while still preserving agent autonomy.
- **Trade-offs**: first-query latency is slightly higher than always-on service mode.
- **Alternatives considered**: always-on background service; manual-only startup.

---

## Risks

## Technical Risks
**Risk**: SearXNG container/config defaults may be too broad or noisy for agentic research.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: ship a minimal opinionated configuration and normalize output aggressively at the AOC layer.
- **Fallback**: keep provider internals behind `aoc-search` so defaults can be revised without changing agent prompts.

**Risk**: Alt+C setup becomes too complex if search options are overloaded.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: keep search opt-in and default to a single recommended managed mode.
- **Fallback**: move advanced settings behind a secondary screen or env/config overrides.

**Risk**: Agent auto-start behavior creates confusing Docker failures in constrained environments.
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: make status messaging explicit and separate configured/stopped/unhealthy states clearly.
- **Fallback**: disable auto-start and instruct users to start search manually.

## Dependency Risks
- Docker/Compose availability varies by machine and may block the managed setup flow.
- SearXNG upstream image/config changes could require AOC fixture updates.
- Future subagents depend on a stable wrapper contract, so schema churn is costly.

## Scope Risks
- Scope could expand from “managed local search + wrapper” into a full research orchestration product too early.
- There is a temptation to fold search directly into `agent-browser`, which would blur responsibilities.
- Multi-provider support could distract from getting a strong default SearXNG integration working first.

---

## Appendix
## References
- `docs/configuration.md` (Alt+C tools integrations and Agent Browser config)
- `docs/installation.md` (tool installer positioning)
- `docs/research/aoc-search-contract.md` (phase-1 concrete contract)
- `docs/research/aoc-search-altc-plan.md` (Alt+C integration plan)
- `docs/research/aoc-search-runtime-plan.md` (runtime lifecycle and health plan)
- `docs/research/aoc-search-cli-plan.md` (CLI and normalized query plan)
- `.pi/skills/agent-browser/SKILL.md` (current browser capability)
- SearXNG project documentation and JSON query interface

## Glossary
- **Managed search**: an AOC-owned local search backend whose lifecycle and config are controlled by AOC.
- **Lazy startup**: starting the managed search service only when an agent or operator needs it.
- **Search-first workflow**: querying search before opening pages in a browser.

## Open Questions
- Should search config live in project-local `.aoc` files, user-global config, or both with precedence rules?
- Should `aoc-search` be a shell wrapper in `bin/` or a Rust subcommand under `aoc-cli`?
- Should Phase 1 include an immediate “verify query” step after setup completes?
- Do we want a dedicated research skill immediately, or only after the search wrapper stabilizes?
