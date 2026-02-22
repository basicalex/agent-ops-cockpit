# AOC + Oh-My-OpenCode Integrated Setup (Sandbox-First)

## PRD (Repository Planning Graph / RPG Method)

---

<overview>

## Problem Statement

AOC already has strong project-local context (`.aoc/context.md`, `aoc-mem`, `aoc-stm`, Taskmaster PRDs), custom skills, and controlled workflows. Oh-My-OpenCode (OmO) can add powerful multi-agent orchestration and editing reliability, but direct adoption today has friction and risk:

- setup is manual and profile-specific (plugin/config drift across machines)
- OmO defaults can introduce autonomous loops and token-heavy behavior that reduce engineer control
- OmO has its own task system, which can conflict with Taskmaster unless explicitly governed
- no single reproducible path exists where `./install.sh` + `aoc-init` yields an optimized, safe OmO+AOC stack

The goal is to make OmO an additive execution layer inside AOC without replacing AOC governance. The integrated system must preserve Taskmaster authority, AOC context authority, and engineer-in-the-loop control while remaining one-command reproducible.

## Target Users

1) **Solo AOC power user (primary)**
   - Runs local agent workflows daily and wants faster execution without losing control
   - Needs deterministic setup across laptops/workstations
   - Requires Taskmaster-first planning and AOC memory continuity

2) **AOC maintainers (secondary)**
   - Need a supportable integration path with clear rollback
   - Must prevent context bloat and accidental autonomous behavior regressions

3) **Team adopters (secondary)**
   - Need reproducible onboarding: clone repo, run `./install.sh`, run `aoc-init`
   - Need consistent conventions around context, tasks, and handoffs

## Success Metrics

- **Reproducible setup**: On a clean machine/profile, `./install.sh` then `aoc-init` yields OmO-enabled AOC with zero manual JSON edits.
- **Sandbox safety**: First-run OmO setup defaults to sandbox profile isolation and does not mutate the primary OpenCode profile unexpectedly.
- **Task authority**: 100% of implementation tasks tracked in Taskmaster (`tm`/`aoc-task`); OmO task system remains disabled by default.
- **Control budget**: Default OmO integration disables autonomous loop features (`ralph`/`ulw` continuation patterns) unless explicitly opted-in.
- **Context quality**: Agent context pack remains bounded and AOC-first (decision memory + STM + mind artifacts), with no prompt bloat regression.
- **Operational reliability**: Install and init remain idempotent; reruns do not break existing user auth/config.

</overview>

---

<functional-decomposition>

## Capability Tree

### Capability: Sandbox-First OmO Provisioning

Create a safe, isolated OpenCode profile path to evaluate and operate OmO without destabilizing the user's primary profile.

#### Feature: Profile isolation bootstrap
- **Description**: Provision sandbox config directories using `OPENCODE_CONFIG_DIR` conventions.
- **Inputs**: User home path, AOC config root, optional profile name.
- **Outputs**: Initialized sandbox profile tree with baseline OpenCode config.
- **Behavior**: Create profile if missing, preserve if existing, avoid destructive overwrites.

#### Feature: OmO plugin install into sandbox
- **Description**: Install/register OmO in the sandbox profile with non-interactive flags where possible.
- **Inputs**: Subscription/provider flags, install mode, profile path.
- **Outputs**: Sandbox `opencode.json` with OmO plugin registration.
- **Behavior**: Retry-friendly install, preserve existing plugins (e.g., antigravity auth), fail with actionable diagnostics.

#### Feature: Sandbox promotion/rollback
- **Description**: Support opt-in promotion from sandbox to primary profile and clean rollback.
- **Inputs**: Sandbox profile state, promotion decision.
- **Outputs**: Promoted config or reverted sandbox.
- **Behavior**: Explicit command path; no automatic destructive profile migration.

---

### Capability: OmO Governance for AOC Control

Configure OmO to remain additive, deterministic, and engineer-controlled.

#### Feature: Control-first OmO defaults
- **Description**: Generate OmO config that disables loop-heavy/autonomous features by default.
- **Inputs**: OmO config schema, AOC policy defaults.
- **Outputs**: `.opencode/oh-my-opencode.jsonc` (project-local) and/or profile-level config.
- **Behavior**: Keep high-value capabilities (agents, LSP, safe editing), disable unbounded continuation hooks.

#### Feature: Provider/model compatibility layer
- **Description**: Preserve existing provider setup (native providers, antigravity plugin, fallback chains) during OmO install.
- **Inputs**: Existing `opencode.json` provider/plugin sections.
- **Outputs**: Merged, compatible config.
- **Behavior**: Non-destructive merge; no unexpected provider override unless explicit.

#### Feature: Concurrency/token guardrails
- **Description**: Bound parallelism and fallback behavior to keep spend/control predictable.
- **Inputs**: AOC policy defaults, machine constraints.
- **Outputs**: Background concurrency and fallback settings.
- **Behavior**: Conservative defaults with explicit opt-up.

---

### Capability: Taskmaster-Only Task Governance

Ensure OmO uses Taskmaster conventions rather than parallel task stores.

#### Feature: OmO task-system disablement
- **Description**: Keep OmO experimental task system disabled by default.
- **Inputs**: OmO config flags.
- **Outputs**: `experimental.task_system=false` in effective config.
- **Behavior**: Enforced baseline; opt-in only via explicit operator change.

#### Feature: Task policy prompts/hooks
- **Description**: Inject policy instructions so OmO agents use `tm`/`aoc-task` for planning and status updates.
- **Inputs**: AOC workflow contract, agent prompts.
- **Outputs**: Consistent task behavior across main and delegated agents.
- **Behavior**: Policy text in project-level agent instructions; validation checks in smoke/runbook.

#### Feature: Conflict detection
- **Description**: Detect accidental writes to `.sisyphus/tasks` or competing task stores.
- **Inputs**: Workspace paths, smoke checks.
- **Outputs**: Warning/failure signal in validation pipeline.
- **Behavior**: Fast check in smoke script and documented remediation.

---

### Capability: AOC Context Stack Bridge

Integrate OmO context consumption with AOC memory layers, without bloating prompts.

#### Feature: Context pack composer
- **Description**: Build bounded context payload from AOC layers in precedence order.
- **Inputs**: `aoc-mem read`, `aoc-stm`/`aoc-stm read`, and future `aoc-mind` summaries.
- **Outputs**: Compact context pack for agent startup/injection.
- **Behavior**: Deterministic order and budgets; include citations/paths to durable artifacts.

#### Feature: AGENTS.md harmonization
- **Description**: Keep AGENTS.md procedural and slim while directing durable knowledge to `.aoc` artifacts.
- **Inputs**: AOC agent contract, OmO context hooks.
- **Outputs**: Non-duplicative context injection behavior.
- **Behavior**: Avoid redundant README/AGENTS over-injection; prefer AOC-curated sources.

#### Feature: Handoff continuity policy
- **Description**: Align `aoc-stm` handoffs with OmO execution sessions.
- **Inputs**: STM current/archive, task state.
- **Outputs**: Reliable handoff rhythm and archival flow.
- **Behavior**: Encourage archive checkpoints; promote durable decisions to `aoc-mem`.

---

### Capability: Install and Init Integration

Make OmO setup part of standard AOC bootstrap path.

#### Feature: `install.sh` OmO integration
- **Description**: Add optional/default OmO sandbox provisioning to installer.
- **Inputs**: Install flags, profile mode, provider preferences.
- **Outputs**: OmO-ready config and assets after install.
- **Behavior**: Idempotent reruns, safe defaults, clear logs.

#### Feature: `aoc-init` OmO project seeding
- **Description**: Ensure project-local OmO policy config and guidance are seeded/repaired.
- **Inputs**: Project root, global template paths.
- **Outputs**: `.opencode` policy files and docs references.
- **Behavior**: Non-destructive merge/seed, preserving existing user customizations when valid.

#### Feature: One-command verification readiness
- **Description**: Ensure post-install project is immediately usable with expected policy behavior.
- **Inputs**: Installer/init outputs.
- **Outputs**: Passing validation checklist.
- **Behavior**: Fast smoke checks plus manual acceptance checklist.

---

### Capability: Validation, Observability, and Documentation

Deliver confidence and maintainability for ongoing integration changes.

#### Feature: Automated integration checks
- **Description**: Add shell and crate-level checks covering OmO config and policy invariants.
- **Inputs**: Scripts, config files, binaries.
- **Outputs**: Pass/fail signals in local and CI workflows.
- **Behavior**: Catch regressions early (task conflicts, loop re-enables, missing profile isolation).

#### Feature: Manual acceptance suite
- **Description**: Define operator checks for install/init flow and practical usage.
- **Inputs**: Fresh/sandbox profile, sample tasks.
- **Outputs**: Human-verified acceptance record.
- **Behavior**: Includes rollback rehearsal and profile-switch checks.

#### Feature: Operator runbooks
- **Description**: Document sandbox workflow, promotion, rollback, and troubleshooting.
- **Inputs**: Final implementation behavior.
- **Outputs**: Docs for maintainers and users.
- **Behavior**: Keep concise, scenario-based, and update with each behavioral change.

</functional-decomposition>

---

<structural-decomposition>

## Repository Structure

```
agent-ops-cockpit/
├── install.sh                             # Main installer (extend for OmO sandbox bootstrap)
├── install/
│   └── bootstrap.sh                       # Remote bootstrap (ensure OmO-aware install flags flow)
├── bin/
│   ├── aoc-init                           # Project initializer (seed/repair OmO project policy)
│   └── aoc-opencode-profile               # (new) profile/sandbox helper commands
├── config/
│   └── opencode/
│       ├── opencode.base.json.template    # (new) baseline plugin/provider merge template
│       └── oh-my-opencode.policy.jsonc    # (new) AOC control-first OmO defaults
├── scripts/
│   ├── smoke.sh                           # Extend with OmO integration checks
│   └── opencode/
│       ├── install-omo.sh                 # (new) OmO install orchestration script
│       ├── verify-omo.sh                  # (new) policy/state verification checks
│       └── context-pack.sh                # (new) bounded AOC context pack composer
├── .opencode/
│   ├── agents/                            # Existing project-level OpenCode agents
│   ├── commands/                          # Existing project-level commands
│   └── oh-my-opencode.jsonc               # (new) project-local OmO policy config
├── docs/
│   ├── installation.md                    # Update with OmO path
│   ├── agents.md                          # Update with OmO+Taskmaster governance notes
│   └── omo-integration.md                 # (new) sandbox/promotion/rollback runbook
└── .taskmaster/docs/prds/
    └── omo-integration_prd_rpg.md         # This PRD
```

## Module Definitions

### Module: `aoc-opencode-profile` (new)
- **Maps to capability**: Sandbox-First OmO Provisioning
- **Responsibility**: Manage profile roots (`main`, `sandbox`, custom) and switching helpers.
- **Exports**:
  - `profile_resolve(name)` - resolve profile path
  - `profile_init(name)` - initialize profile safely
  - `profile_promote(source, target)` - explicit promotion path with backup

### Module: `scripts/opencode/install-omo.sh` (new)
- **Maps to capability**: Sandbox-First OmO Provisioning, OmO Governance
- **Responsibility**: Install OmO plugin and apply baseline policy without clobbering provider config.
- **Exports**:
  - `install_omo(profile, flags)`
  - `merge_opencode_plugins(profile)`

### Module: `config/opencode/*` (new)
- **Maps to capability**: OmO Governance for AOC Control
- **Responsibility**: Store reusable config templates for policy-controlled OmO defaults.
- **Exports**:
  - `opencode.base.json.template`
  - `oh-my-opencode.policy.jsonc`

### Module: `scripts/opencode/context-pack.sh` (new)
- **Maps to capability**: AOC Context Stack Bridge
- **Responsibility**: Compose bounded context packet from `aoc-mem`, `aoc-stm`, and `aoc-mind` artifacts.
- **Exports**:
  - `compose_context_pack(project_root)`

### Module: `install.sh` (existing, extended)
- **Maps to capability**: Install and Init Integration
- **Responsibility**: Ensure OmO bootstrap integrates with existing AOC installer idempotently.
- **Exports**:
  - `install_omo_if_enabled()`
  - `seed_project_omo_policy_if_enabled()`

### Module: `bin/aoc-init` (existing, extended)
- **Maps to capability**: Install and Init Integration, AOC Context Stack Bridge
- **Responsibility**: Seed/repair project-local OmO policy files and guidance alongside `.aoc` state.
- **Exports**:
  - `setup_omo_policy()`
  - `ensure_omo_taskmaster_governance()`

### Module: `scripts/smoke.sh` + `scripts/opencode/verify-omo.sh` (extended/new)
- **Maps to capability**: Validation, Observability, and Documentation
- **Responsibility**: Verify OmO policy invariants and integration behavior.
- **Exports**:
  - `verify_omo_task_authority()`
  - `verify_omo_control_flags()`
  - `verify_omo_profile_isolation()`

</structural-decomposition>

---

<dependency-graph>

## Dependency Chain

### Foundation Layer (Phase 0)

No dependencies.

- **`aoc-opencode-profile`**: Profile resolution, sandbox initialization, promotion scaffolding.
- **`config/opencode` templates**: Baseline OmO policy templates and merge contracts.
- **Policy contract updates**: Governance rules for Taskmaster authority and control defaults.

### Sandbox Bring-Up Layer (Phase 1)

- **`scripts/opencode/install-omo.sh`**: Depends on [`aoc-opencode-profile`, `config/opencode` templates].
- **Sandbox verifier checks**: Depends on [`aoc-opencode-profile`].

### Governance Bridge Layer (Phase 2)

- **Taskmaster governance enforcement**: Depends on [`config/opencode` templates, sandbox verifier checks].
- **Context pack composer (`context-pack.sh`)**: Depends on [`aoc-opencode-profile`, policy contract updates].

### Installer/Initializer Integration Layer (Phase 3)

- **`install.sh` OmO integration**: Depends on [`scripts/opencode/install-omo.sh`, Taskmaster governance enforcement, context pack composer].
- **`aoc-init` OmO seeding/repair**: Depends on [`config/opencode` templates, Taskmaster governance enforcement].

### Validation & Documentation Layer (Phase 4)

- **Smoke/regression suite expansion**: Depends on [`install.sh` OmO integration, `aoc-init` OmO seeding/repair].
- **Operator runbooks/docs**: Depends on [all prior layers].

### Release Readiness Layer (Phase 5)

- **One-command acceptance (`./install.sh` + `aoc-init`)**: Depends on [Phase 4 validation + docs completion].

</dependency-graph>

---

<implementation-roadmap>

## Development Phases

### Phase 0: Foundations (Sandbox + Policy Contracts)
**Goal**: Establish profile isolation primitives and policy templates.

**Entry Criteria**: Repo builds; existing installer/init paths are green.

**Tasks**:
- [ ] Implement `aoc-opencode-profile` helper module (depends on: none)
  - Acceptance criteria: can initialize sandbox profile path and resolve active profile deterministically.
  - Test strategy: shell unit tests for path resolution and idempotent profile init.

- [ ] Create OmO policy templates in `config/opencode/` (depends on: none)
  - Acceptance criteria: templates encode control-first defaults, Taskmaster authority, and provider-safe merge points.
  - Test strategy: fixture tests for template rendering and key-preservation merge behavior.

- [ ] Define governance contract docs for OmO inside AOC (depends on: none)
  - Acceptance criteria: explicit statements for disabled loop features and Taskmaster-only task ownership.
  - Test strategy: docs lint + checklist validation in review.

**Exit Criteria**: Sandbox/profile and template contracts are in place and testable.

**Delivers**: Reliable base for additive OmO integration without touching primary profile behavior.

---

### Phase 1: Sandbox OmO Bring-Up
**Goal**: Install and validate OmO in isolated profile with no primary-profile blast radius.

**Entry Criteria**: Phase 0 complete.

**Tasks**:
- [ ] Implement `scripts/opencode/install-omo.sh` install orchestration (depends on: [Phase 0 profile helper, Phase 0 templates])
  - Acceptance criteria: OmO plugin installs into sandbox profile and preserves existing plugin/provider entries.
  - Test strategy: sandbox fixture profile tests + plugin-array verification.

- [ ] Implement sandbox verification checks (depends on: [Phase 0 profile helper])
  - Acceptance criteria: command reports profile isolation status and plugin registration health.
  - Test strategy: shell integration checks against ephemeral profile dirs.

**Exit Criteria**: OmO can be installed and validated in sandbox without manual JSON editing.

**Delivers**: Safe experimentation lane for OmO inside AOC.

---

### Phase 2: Governance Bridges (Taskmaster + Context)
**Goal**: Align OmO runtime behavior with AOC governance.

**Entry Criteria**: Phase 1 complete.

**Tasks**:
- [ ] Enforce Taskmaster-only task flow in OmO policy (depends on: [Phase 1 verifier, Phase 0 templates])
  - Acceptance criteria: OmO task system disabled, task guidance references `tm`/`aoc-task` only.
  - Test strategy: config assertions + smoke check that `.sisyphus/tasks` remains untouched during sample workflow.

- [ ] Implement AOC context pack composer (`context-pack.sh`) (depends on: [Phase 0 profile helper, Phase 0 governance docs])
  - Acceptance criteria: produces bounded context payload with precedence: `aoc-mem` -> `aoc-stm` -> `aoc-mind`.
  - Test strategy: fixture-driven output tests, line/size bounds enforced.

- [ ] Harmonize AGENTS/README injection behavior with OmO (depends on: [Taskmaster governance task, context-pack task])
  - Acceptance criteria: no duplicate bloated context injection; procedural guidance remains in AGENTS.
  - Test strategy: manual transcript review + bounded output checks.

**Exit Criteria**: OmO behaves as additive executor under AOC authority.

**Delivers**: Governed OmO runtime with controlled context and task ownership.

---

### Phase 3: Installer and Initializer Integration
**Goal**: Integrate sandbox and policy setup into standard `install.sh` + `aoc-init` flow.

**Entry Criteria**: Phases 0-2 complete.

**Tasks**:
- [ ] Extend `install.sh` with OmO sandbox provisioning path (depends on: [Phase 1 install orchestration, Phase 2 governance bridges])
  - Acceptance criteria: installer provisions OmO-ready sandbox/profile assets idempotently.
  - Test strategy: repeat install runs in temp HOME; compare before/after file diffs for stability.

- [ ] Extend `aoc-init` to seed/repair project OmO policy config (depends on: [Phase 0 templates, Phase 2 governance bridges])
  - Acceptance criteria: `aoc-init` creates/repairs `.opencode/oh-my-opencode.jsonc` and associated guidance safely.
  - Test strategy: idempotency tests over existing/non-existing project states.

**Exit Criteria**: User can run installer + init and receive policy-correct OmO+AOC setup.

**Delivers**: One-command bootstrap behavior with sandbox-first safety.

---

### Phase 4: Validation, Docs, and Operator Readiness
**Goal**: Lock reliability and handoff quality for maintainers/users.

**Entry Criteria**: Phase 3 complete.

**Tasks**:
- [ ] Add OmO integration checks to smoke pipeline (depends on: [Phase 3 tasks])
  - Acceptance criteria: smoke suite verifies profile isolation, control defaults, Taskmaster authority.
  - Test strategy: `AOC_SMOKE_TEST=1 bash scripts/smoke.sh` plus targeted OmO verify script.

- [ ] Publish runbook docs for sandbox, promotion, rollback (depends on: [Phase 3 tasks])
  - Acceptance criteria: concise operator docs cover first-run, troubleshooting, and recovery.
  - Test strategy: docs walkthrough by maintainer using clean profile.

- [ ] Define release acceptance checklist (depends on: [smoke checks, runbook docs])
  - Acceptance criteria: checklist confirms `./install.sh` + `aoc-init` success from clean environment.
  - Test strategy: manual E2E rehearsal and sign-off record.

**Exit Criteria**: Integration is supportable and repeatable by maintainers and users.

**Delivers**: Production-ready OmO integration path for AOC.

</implementation-roadmap>

---

<test-strategy>

## Test Pyramid

```
        /\
       /E2E\        ← 10% (fresh install/init flows, manual + scripted)
      /------\
     /Integration\  ← 30% (installer/init/profile/config bridge interactions)
    /------------\
   /  Unit Tests  \ ← 60% (script helpers, config rendering, policy assertions)
  /----------------\
```

## Coverage Requirements
- Line coverage: 75% minimum for touched Rust crates; shell logic must include explicit smoke/integration checks.
- Branch coverage: 65% minimum for touched Rust crates.
- Function coverage: 75% minimum for touched Rust crates.
- Statement coverage: 75% minimum for touched Rust crates.

## Critical Test Scenarios

### Profile Isolation (`aoc-opencode-profile`)
**Happy path**:
- initialize sandbox profile and resolve active path
- Expected: deterministic profile directories and no mutation of primary profile

**Edge cases**:
- profile already exists with partial config
- Expected: non-destructive repair and clear log output

**Error cases**:
- missing permissions or invalid path
- Expected: fail with actionable guidance, no partial corruption

### OmO Config/Governance
**Happy path**:
- generate control-first config with task system disabled
- Expected: expected keys present and effective values enforce governance

**Edge cases**:
- existing plugin/provider arrays include antigravity and custom providers
- Expected: merge preserves existing entries and appends OmO safely

**Error cases**:
- malformed existing JSON
- Expected: validation error with remediation path; no blind overwrite

### Taskmaster Authority Bridge
**Happy path**:
- perform sample task workflow with OmO agents
- Expected: updates happen in Taskmaster only (`tm`), not in `.sisyphus/tasks`

**Edge cases**:
- delegated agents and retries
- Expected: governance instructions persist across delegation boundaries

**Error cases**:
- accidental task-system enablement
- Expected: smoke check fails and points to config key to reset

### Install + Init End-to-End
**Integration points**:
- `./install.sh` provisions OmO policy scaffolding
- `aoc-init` seeds/repairs project config
- Expected: idempotent reruns, stable outputs, no conflicting task/context behavior

## Test Generation Guidelines

- Use fixture-driven tests for profile/config merge behavior.
- Keep shell checks deterministic; prefer explicit file/key assertions.
- Add regression checks for the exact governance keys (loop/continuation off, task system off).
- Ensure context pack output is bounded in size and includes deterministic section order.
- Include at least one clean-environment rehearsal script for maintainers.

</test-strategy>

---

<architecture>

## System Components

1) **AOC Installer Layer (`install.sh`, `install/bootstrap.sh`)**
   - Installs binaries/assets and now seeds OmO sandbox-ready baseline.

2) **AOC Initializer Layer (`aoc-init`)**
   - Ensures project-local AOC + OmO policy files are present and current.

3) **OmO Sandbox/Profile Layer**
   - Isolated OpenCode profile(s) managed via `OPENCODE_CONFIG_DIR` policy.

4) **Governance Layer (Taskmaster + Policy Config)**
   - Encodes Taskmaster-only task handling and control-first OmO behavior.

5) **Context Bridge Layer (`context-pack.sh`)**
   - Composes bounded AOC context packs from memory, STM, and mind artifacts.

6) **Validation Layer (`scripts/smoke.sh`, verify scripts)**
   - Verifies profile isolation, policy invariants, and one-command readiness.

## Data Models

### Config Artifacts

- `opencode.json` (profile-scoped): plugin/provider model registry and auth integration.
- `oh-my-opencode.jsonc` (project/profile scoped): OmO runtime policy (agents/hooks/concurrency/task-system).
- `.aoc` artifacts (project-scoped): memory (`memory.md`), STM (`stm/`), mind outputs (`mind/`), context snapshot.

### Governance Invariants

- `task_authority = taskmaster_only`
- `omo_task_system = disabled`
- `continuation_loops = disabled_by_default`
- `context_precedence = [aoc-mem, aoc-stm, aoc-mind]`

## Technology Stack

- Shell scripting (installer/init/orchestration)
- Rust (existing AOC CLIs/TUIs where integration hooks are needed)
- JSON/JSONC policy templates
- Taskmaster CLI (`tm`/`aoc-task`) for task lifecycle

**Decision: Sandbox-first OmO adoption**
- **Rationale**: Prevents primary profile breakage and supports controlled rollout.
- **Trade-offs**: Adds profile management complexity and extra docs.
- **Alternatives considered**: Direct install into primary profile (rejected for safety).

**Decision: Taskmaster as sole task authority**
- **Rationale**: Preserve existing AOC planning/traceability and avoid split-brain task state.
- **Trade-offs**: OmO native task features remain unused by default.
- **Alternatives considered**: dual-task-system operation (rejected due to drift/conflicts).

**Decision: Control-first OmO defaults**
- **Rationale**: Maintain engineer control, predictable spend, and deterministic behavior.
- **Trade-offs**: Less autonomous throughput out of the box.
- **Alternatives considered**: full ultrawork defaults (rejected for this repo’s governance goals).

**Decision: Keep durable context in `.aoc`**
- **Rationale**: Centralizes project memory and avoids tool-specific lock-in.
- **Trade-offs**: Requires explicit bridge logic for external harnesses.
- **Alternatives considered**: OmO-native memory/task storage (rejected as source of truth).

</architecture>

---

<risks>

## Technical Risks

**Risk**: OmO config schema changes across releases
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: Version-pinned template checks + verify script for required keys
- **Fallback**: lock to known-good OmO version and degrade to base OpenCode mode

**Risk**: Hook interactions re-enable token-heavy autonomous behaviors
- **Impact**: High
- **Likelihood**: Medium
- **Mitigation**: explicit disabled hook list + smoke checks for prohibited features
- **Fallback**: emergency policy profile with all OmO continuation features disabled

**Risk**: Provider/plugin merge conflicts (antigravity + OmO + native providers)
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: non-destructive merge tests and profile backups before write
- **Fallback**: restore from backup and retry with minimal plugin set

**Risk**: CLI/runtime instability in certain environments
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: sandbox-first trial gate and explicit fallback commands
- **Fallback**: keep AOC baseline operation without OmO enabled

## Dependency Risks

- OmO installer behavior or flags can change
  - Mitigation: pin tested version in install logic and update runbook on bump

- OpenCode plugin API changes can affect config merge paths
  - Mitigation: isolate profile operations and add compatibility checks in smoke

## Scope Risks

**Risk**: Attempting to productionize all OmO features instead of additive integration
- **Impact**: High
- **Likelihood**: High
- **Mitigation**: keep first release to sandbox + governance + one-command bootstrap
- **Fallback**: defer advanced OmO capabilities to later tag tasks

**Risk**: Underestimating documentation/operator training needs
- **Impact**: Medium
- **Likelihood**: Medium
- **Mitigation**: treat runbook and acceptance checklist as release-blocking deliverables
- **Fallback**: release behind explicit opt-in flag until docs are complete

</risks>

---

<appendix>

## References

- OmO repository: `https://github.com/code-yeongyu/oh-my-opencode`
- OmO installation guide: `https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/refs/heads/master/docs/guide/installation.md`
- OmO config reference: `https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/refs/heads/dev/docs/reference/configuration.md`
- Taskmaster PRD template: `.taskmaster/templates/example_prd_rpg.txt`

## Glossary

- **AOC**: Agent Ops Cockpit repository and workflow system
- **OmO**: Oh-My-OpenCode plugin layer
- **Sandbox profile**: isolated OpenCode config root used for safe integration trials
- **Task authority**: the canonical system where tasks are created/updated
- **Context pack**: bounded payload assembled from AOC memory layers for agent startup

## Open Questions

1) Should OmO install be default-on in `install.sh`, or opt-in via env flag for first release?
2) What exact minimum OmO/OpenCode versions should be pinned for deterministic behavior?
3) Which OmO hooks are explicitly allowed for additive value in AOC v1 integration?
4) How should sandbox promotion be exposed (single command vs documented manual flow)?
5) What final acceptance threshold defines “production-ready” for one-command setup?

</appendix>
