# AOC Lexicon

Canonical human-agent project language for Agent Ops Cockpit.

Use this file as a semantic consistency layer, not a requirements source or memory dump. Keep entries concise. Add only grounded AOC-specific terms, aliases, conflicts, or relationships that improve future agent work.

## Governance

Authority order for implementation work:
1. Explicit current user instruction.
2. Active spec / PRD / task acceptance criteria.
3. Current repo behavior.
4. Existing AOC lexicon.
5. Existing memory.
6. Agent inference.

Agents may update this file only for high-confidence, low-risk terminology changes. Propose instead of editing when a term is ambiguous, broad, political, disruptive, or conflicts with existing usage.

## Entry format

```md
### Canonical Term

Definition: Concise definition.
Aliases: accepted alternate terms, if any.
Avoid: confusing or deprecated terms, if any.
Relationships: related commands, artifacts, workflows, states, owners, or lifecycle notes.
Evidence: path, command, task, spec, or implemented behavior that grounds the entry.
```

## Core terms

### AOC Implementation Journey

Definition: Standard workflow that moves from source-of-truth resolution through task decomposition, implementation, verification, lexicon delta, and completion marking.
Aliases: implementation journey.
Avoid: delivery journey, agent journey, classic journey when used ambiguously.
Relationships: `/implement`; source → semantics → task → subtasks → implement → test → lexicon delta → mark complete.
Evidence: `.pi/prompts/implement.md`.

### AOC Lexicon

Definition: Project-local canonical terminology file for AOC-specific concepts, workflows, commands, artifacts, and user-facing language.
Aliases: lexicon.
Avoid: memory dump, glossary when implying exhaustive documentation.
Relationships: `.aoc/lexicon.md`; Lexicon Preflight; Lexicon Delta; AOC Lexicon Journey.
Evidence: `.pi/prompts/implement.md`; `.pi/prompts/lexicon.md`.

### AOC Lexicon Journey

Definition: Focused workflow for reviewing, proposing, and applying governed updates to AOC terminology without expanding into unrelated implementation.
Aliases: lexicon journey.
Avoid: full documentation audit when only terminology is in scope.
Relationships: `/lexicon`; proposal/report mode by default; apply mode only when explicitly requested.
Evidence: `.pi/prompts/lexicon.md`.

### Lexicon Preflight

Definition: Lightweight optional check during implementation planning that maps task/spec/conversation terms to canonical AOC terms when language matters.
Aliases: semantics preflight.
Avoid: mandatory glossary review.
Relationships: occurs after source-of-truth resolution; should be skipped for generic implementation work with no terminology impact.
Evidence: `.pi/prompts/implement.md`.

### Lexicon Delta

Definition: Non-blocking post-verification check that updates or proposes changes to AOC terminology when implementation clarified language.
Aliases: semantics delta.
Avoid: automatic vocabulary churn.
Relationships: occurs after verification and before completion marking; direct edits require high confidence and low risk.
Evidence: `.pi/prompts/implement.md`.

### Directed Handoff Packet

Definition: Deliberate continuation artifact created when another agent or future session needs to resume incomplete work.
Aliases: directed handoff.
Avoid: handoff summary, continuation note when used casually.
Relationships: created through `aoc-stm`; not a replacement for durable memory.
Evidence: `.pi/prompts/implement.md`; AGENTS.md effective contract.

### Durable Decision

Definition: Persistent project decision, discovery, constraint, or history that should survive across sessions.
Aliases: memory-worthy decision.
Avoid: lexicon entry when the content is historical rather than terminological.
Relationships: stored through `aoc-mem add`; separate from AOC Lexicon.
Evidence: AGENTS.md effective contract.

## Product surfaces

### Agent Ops Cockpit

Definition: Pi-first terminal workspace for AI-assisted development with project context, tasks, memory, operator controls, and Zellij/Pi integration.
Aliases: AOC.
Avoid: generic agent wrapper when referring to the full product.
Relationships: `aoc`; `aoc-init`; `aoc-doctor`; Pi Runtime; Taskmaster; Mind; Control Pane; Mission Control.
Evidence: `README.md`; `AOC.md`.

### Pi Runtime

Definition: Canonical agent runtime for AOC projects.
Aliases: Pi coding agent.
Avoid: OpenCode/runtime-neutral phrasing for current AOC default.
Relationships: `.pi/`; `.pi/prompts/`; `.pi/skills/`; `.pi/extensions/`; `aoc`.
Evidence: `docs/agents.md`; `AOC.md`.

### Zellij Workspace

Definition: Persistent terminal workspace AOC uses for panes, tabs, task views, shells, and Pi sessions.
Aliases: Zellij session.
Avoid: terminal window when referring to the managed AOC pane/tab model.
Relationships: AOC launch; layouts; Mission Control tab; floating panes.
Evidence: `README.md`; `AOC.md`; `docs/layouts.md`.

### Control Pane

Definition: `Alt+C` operator surface for tools, setup, integrations, logs, health checks, and background jobs.
Aliases: Alt+C control pane; `aoc-control`.
Avoid: settings-only surface.
Relationships: Tools-first taxonomy; background jobs; AOC Understand; Agent Browser + Search; RTK Routing; HyperFrames.
Evidence: `docs/control-pane.md`; `.taskmaster/docs/specs/aoc_control_tools_first_taxonomy_spec_rpg.md`.

### Mission Control

Definition: Global operator surface for fleet, session, worker, overseer, detached-job, health, diff, and orchestration visibility.
Aliases: dedicated Mission Control tab; `aoc-mission-control`.
Avoid: Pulse Pane as current product identity.
Relationships: Overseer; delegated jobs; Mind view host; Pulse; detached fleet.
Evidence: `docs/reference/architecture.md`; `docs/reference/mission-control-architecture.md`.

### Mind

Definition: Project-scoped knowledge and runtime-state surface backed by a project store.
Aliases: AOC Mind; Pi Mind when referring to Pi session-derived Mind flow.
Avoid: broad memory; generic STM; Pulse-coupled Mind.
Relationships: `aoc-mind-service`; `.aoc/mind/project.sqlite`; context packs; provenance; T0/T1/T2/T3.
Evidence: `docs/reference/architecture.md`; `docs/tasks-memory.md`.

### Pulse

Definition: Transport, IPC, and telemetry substrate for AOC runtime communication.
Aliases: Pulse IPC.
Avoid: product identity for Mind or Mission Control.
Relationships: `pulse.sock`; `AOC_PULSE_SOCK`; hub/client transport; compatibility path.
Evidence: `docs/reference/architecture.md`; `docs/reference/pulse-ipc-protocol.md`.

## Task, context, and memory

### Taskmaster

Definition: Project task tracking and execution ledger used by AOC through `tm` and `aoc-task`.
Aliases: task ledger; Taskmaster task state.
Avoid: memory store; raw log store.
Relationships: tasks; specs; subtasks; readiness explanations; Taskmaster Execution Cockpit.
Evidence: `docs/tasks-memory.md`; `.taskmaster/docs/specs/task-232_taskmaster_execution_cockpit_spec.md`.

### Spec

Definition: Linked planning, architecture, implementation, recovery, rollout, or operational document used to ground Taskmaster and implementation work.
Aliases: RPG spec; task spec.
Avoid: PRD as generic current term.
Relationships: `.taskmaster/docs/specs/`; `aocPrd` legacy metadata; Taskmaster.
Evidence: `.taskmaster/docs/specs/task-207_prd_to_spec_refactor_spec_rpg.md`.

### Taskmaster Execution Cockpit

Definition: Taskmaster enhancement model where tasks hold compact execution-state indexes: what happened, what was proven, what remains uncertain, and what is actionable.
Aliases: execution cockpit.
Avoid: replacement for Mind, STM, Git, or raw logs.
Relationships: `tm ready`; task-scoped execution index; Taskmaster readiness explanations.
Evidence: `.taskmaster/docs/specs/task-232_taskmaster_execution_cockpit_spec.md`; `.taskmaster/docs/specs/task-237_taskmaster_ready_explain_spec.md`.

### STM Directed Handoff Layer

Definition: Short-term handoff system for deliberate in-progress packets between agents or sessions.
Aliases: STM; handoff layer.
Avoid: durable memory; mailbox; generic work log.
Relationships: `aoc-stm`; `/handoff`; `/rresume`; Directed Handoff Packet.
Evidence: `docs/tasks-memory.md`.

### Context Router

Definition: AOC routing layer that classifies context sources before agent startup or intent-triggered loading.
Aliases: context loading router.
Avoid: startup dumping.
Relationships: startup kernel; `.aoc/effective-agent-contract.md`; `aoc-handshake --json`; lazy loading.
Evidence: `docs/reference/contextualization-architecture-plan.md`.

### Startup Kernel

Definition: Compact default startup context containing effective agent contract, project snapshot, handshake metadata, active tag/preset metadata, and context routing policy.
Aliases: kernel-first startup.
Avoid: broad startup context.
Relationships: Context Router; `aoc-handshake --json`; `.aoc/context.md`; `.aoc/effective-agent-contract.md`.
Evidence: `docs/reference/contextualization-architecture-plan.md`.

## Agents and workers

### Delegated Specialist

Definition: Explicit operator-invoked detached specialist agent running through AOC/Pi subagent controls.
Aliases: delegated subagent; specialist role.
Avoid: Mind Worker when referring to user-launched specialist UX.
Relationships: `.pi/agents/`; `aoc_subagent`; `aoc_specialist_role`; `/subagent-run`; `/specialist-run`.
Evidence: `docs/reference/subagent-runtime.md`.

### Mind Worker

Definition: Project-scoped detached worker plane used by Mind/runtime systems, separate from delegated specialist UX.
Aliases: T0/T1/T2/T3 worker when tier-specific.
Avoid: Delegated Specialist unless operator-launched through subagent UX.
Relationships: Mind; Mission Control fleet; detached lifecycle/provenance contracts.
Evidence: `docs/reference/subagent-runtime.md`; `docs/reference/architecture.md`.

## Modes, mapping, design, and media

### AOC Preset

Definition: Project-local session mode that coordinates prompt components, Pi runtime state, slash commands, and active/recommended skill routing.
Aliases: preset; mode.
Avoid: nested-skill runtime; replacement for layouts; replacement for Mind.
Relationships: `/preset`; `Alt+X`; `.aoc/presets/`; active skill; recommended skill.
Evidence: `docs/presets.md`.

### AOC Understand

Definition: AOC wrapper around Understand-Anything for repository knowledge graphs, guided tours, explain/chat/onboard/diff/domain flows, dashboard, and AOC Map sync.
Aliases: Understand wrapper.
Avoid: teach workflow for new repo-understanding work.
Relationships: `aoc-understand`; Understand-Anything; AOC Map.
Evidence: `docs/understand.md`; `.taskmaster/docs/specs/aoc_understand_integration_spec_rpg.md`.

### AOC Map

Definition: Canonical project-local graph microsite under `.aoc/map/` for curated repo maps, Mermaid diagrams, generated pages, and local browsing.
Aliases: map microsite.
Avoid: AOC See; `.aoc/see/`; `.aoc/diagrams/` as current terms.
Relationships: `aoc-map`; `aoc map`; Mermaid; AOC Understand map sync.
Evidence: `docs/aoc-map.md`.

### Open Design

Definition: Optional GUI-first design studio bridge for visual iteration, design systems, prototypes, decks, templates, previews, and artifact import/export.
Aliases: OD; Open Design studio.
Avoid: treating Open Design as AOC task/provenance owner.
Relationships: `aoc-od`; `.aoc/open-design/`; `.od/artifacts/`; `DESIGN.md`; HyperFrames.
Evidence: `docs/open-design.md`; `.taskmaster/docs/specs/aoc_open_design_global_integration_rpg.md`.

### HyperFrames

Definition: Project-local video and campaign factory for reusable media assets, campaign compositions, renders, and shotlists.
Aliases: AOC HyperFrames.
Avoid: generic design iteration surface.
Relationships: `aoc-hyperframes`; `aoc-hf`; Open Design; design artifacts.
Evidence: `docs/hyperframes.md`; `README.md`.

## Runtime utilities

### RTK Routing

Definition: Command-routing and output-condensing layer for noisy routine commands, intended to preserve context health.
Aliases: RTK.
Avoid: security sandbox.
Relationships: `aoc-rtk`; `.aoc/rtk.toml`; allowlisted commands; denylisted commands; fail-open behavior.
Evidence: `docs/reference/rtk-routing.md`.

### Managed Asset

Definition: AOC-authored project asset seeded or refreshed into projects by `aoc-init` and tracked through marker files and `.aoc/managed-assets.json`.
Aliases: AOC managed asset.
Avoid: project-authored artifact.
Relationships: `.aoc-managed`; `.aoc/managed-assets.json`; `aoc-init --check-managed`.
Evidence: `docs/managed-assets.md`.

## Legacy aliases to avoid

### PRD

Definition: Legacy generic planning-document term retained in compatibility metadata.
Aliases: `aocPrd` metadata.
Avoid: PRD as the current generic term for specs.
Relationships: Spec; Taskmaster; legacy-compatible metadata.
Evidence: `.taskmaster/docs/specs/task-207_prd_to_spec_refactor_spec_rpg.md`.

### Teach Mode

Definition: Deprecated prompt-only repository-understanding workflow that wrote Markdown notes under `.aoc/insight/`.
Aliases: teach workflow; legacy teach prompts.
Avoid: using teach for new repository-understanding work.
Relationships: AOC Understand replaces teach for new repo understanding.
Evidence: `docs/understand.md`.

### Pulse Pane

Definition: Legacy compatibility label that now degrades to normal Mission Control behavior.
Aliases: `pulse-pane`; `pulse_pane`; `pulse`.
Avoid: current product-surface identity.
Relationships: Mission Control; Pulse.
Evidence: `docs/reference/mission-control-architecture.md`; `docs/reference/architecture.md`.

### AOC See

Definition: Legacy map surface name retained only for transition/migration compatibility.
Aliases: `.aoc/see/`; old `.aoc/diagrams/` workspace.
Avoid: current map terminology.
Relationships: AOC Map; `aoc-map`; `aoc map`.
Evidence: `docs/aoc-map.md`.

### Retired Zellij Top Bar

Definition: Removed AOC status-bar experiment superseded by the Herdr/OMP-first workspace model.
Aliases: old AOC tab bar; retired top-bar plugin.
Avoid: reintroducing status-bar plugin assets or installer seeding.
Relationships: Herdr Workspace; OMP.
Evidence: Herdr/OMP cutover; `docs/herdr-workspace.md`; `docs/aoc-feature-inventory.md`.
