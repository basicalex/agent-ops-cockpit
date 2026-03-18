# Mind Runtime Environment Isolation PRD (RPG)

## Problem Statement
Mind’s recent leak was caused by persisted tool output, but the broader risk remains: Mind-related processes and subprocesses may inherit the ambient shell environment, including provider credentials and other sensitive operator state. If those values are visible to children by default, any future bug, debug print, or command invocation can convert ambient secrets into durable compromise.

We need least-privilege environment handling so Mind and allied workers receive only explicitly required non-secret variables.

## Target Users
- **Operators** who run AOC/Pi with provider credentials in their shell.
- **Runtime maintainers** who need deterministic child-process behavior.
- **Security reviewers** who need a clear answer to whether Mind can access ambient credentials.

## Success Metrics
- Mind-related subprocess launches use allowlisted environment passing by default.
- Provider credentials and unrelated ambient env vars are absent from child-process environments in tests.
- Any required inherited vars are documented, minimized, and justified.
- Runtime behavior remains functional under the reduced env model.

---

## Capability Tree

### Capability: Least-Privilege Child Process Spawning
Control what environment reaches Mind-related children.

#### Feature: Env allowlist policy
- **Description**: Define the minimal set of variables required by Mind workers and helpers.
- **Inputs**: Runtime/session/process requirements.
- **Outputs**: Explicit allowlist contract.
- **Behavior**: Exclude provider secrets and unrelated ambient vars by default.

#### Feature: Env-cleared spawning
- **Description**: Clear inherited environment before adding approved vars.
- **Inputs**: Spawn requests for Mind-related subprocesses.
- **Outputs**: Child processes launched with deterministic env.
- **Behavior**: Use `env_clear()`/equivalent then add allowlisted keys only.

### Capability: Runtime Compatibility Preservation
Keep the system working while reducing ambient exposure.

#### Feature: Required-variable documentation
- **Description**: Record which env vars remain necessary and why.
- **Inputs**: Spawn-path audit results.
- **Outputs**: Maintainer docs and code comments.
- **Behavior**: Make exceptions explicit and reviewable.

#### Feature: Fallback and diagnostics
- **Description**: Provide clear failures when a needed variable is missing.
- **Inputs**: Child startup/runtime validation.
- **Outputs**: Actionable errors without secret disclosure.
- **Behavior**: Fail fast with remediation hints.

### Capability: Env Exposure Verification
Prove the reduced env model is real.

#### Feature: Spawn-path env assertions
- **Description**: Test child-process env contents.
- **Inputs**: Representative spawn paths and injected ambient secrets.
- **Outputs**: Passing assertions that secrets are not inherited.
- **Behavior**: Verify both presence of required allowlisted vars and absence of blocked vars.

---

## Repository Structure

```text
project-root/
├── crates/
│   ├── aoc-agent-wrap-rs/
│   │   └── src/main.rs
│   ├── aoc-mission-control/
│   │   └── src/main.rs
│   ├── aoc-control/
│   │   └── src/main.rs
│   └── aoc-mind/
│       └── src/{reflector_runtime.rs,t3_runtime.rs}
└── docs/
    └── env-hardening.md (or related runtime docs)
```

## Module Definitions

### Module: spawn-path audit and helpers
- **Maps to capability**: Env allowlist policy + Env-cleared spawning
- **Responsibility**: Centralize env allowlist construction and child-process launch behavior.
- **Exports**:
  - `build_mind_env_allowlist(...)`
  - helper wrappers for secure child spawning

### Module: runtime call sites (`aoc-agent-wrap-rs`, `aoc-mission-control`, `aoc-control`)
- **Maps to capability**: Runtime Compatibility Preservation
- **Responsibility**: Apply the allowlist helper everywhere Mind-adjacent children are launched.

### Module: docs
- **Maps to capability**: Required-variable documentation
- **Responsibility**: Document the approved env contract and exception handling.

---

## Dependency Chain

### Foundation Layer (Phase 0)
- **Spawn-path inventory**: Identify all Mind-related process launches and current env inheritance behavior.
- **Allowlist contract**: Define the approved runtime env surface.

### Runtime Hardening Layer (Phase 1)
- **Secure spawn helpers**: Depends on [Spawn-path inventory, Allowlist contract]

### Integration Layer (Phase 2)
- **Call-site migration**: Depends on [Secure spawn helpers]

### Verification Layer (Phase 3)
- **Env inheritance tests and docs**: Depends on [Call-site migration]

---

## Development Phases

### Phase 0: Inventory and Policy
- Audit every Mind-related spawn path.
- Define the minimal allowlist and explicit exceptions.

### Phase 1: Secure Spawn Helpers
- Implement helper APIs that clear env and add only approved vars.
- Standardize diagnostics for missing required env.

### Phase 2: Call-Site Adoption
- Migrate identified spawn paths to the secure helper.
- Remove ad-hoc ambient inheritance.

### Phase 3: Verification and Documentation
- Add tests that inject ambient provider secrets and confirm they do not reach children.
- Document remaining required vars and rationale.

---

## Test Strategy
- Unit tests for allowlist construction.
- Spawn integration tests proving ambient credentials are absent in child environments.
- Regression tests covering representative Mind-related launch paths.
- Documentation validation ensuring exceptions remain explicit and narrow.

---

## Risks and Mitigations
- **Risk**: Breaking runtime flows that depended on ambient env.
  - **Mitigation**: Inventory first, adopt helper incrementally, add precise diagnostics.
- **Risk**: Hidden spawn paths remain unpatched.
  - **Mitigation**: Use code search plus targeted tests for every known launch site.
- **Risk**: Overly broad allowlist regresses security.
  - **Mitigation**: Require justification and docs for each retained variable.
