# AOC Subagent Runtime

This document is the operator/runtime reference for AOC's **delegated specialist subagent** path inside Pi.

It explains what exists today, what the current source of truth is, how delegated specialists relate to Mind workers, and how operators should launch, inspect, recover, and trust detached work.

> This document is about the **delegated specialist** UX and runtime contract.
>
> It does **not** redefine the broader detached control plane owned by:
> - task 169 — detached Pi subagent runtime substrate
> - task 129 — explicit specialist role interface
> - task 178 — Mind detached dispatcher
> - task 149 — Mission Control / fleet / overseer boundary

---

## 1. What this runtime is

AOC runs detached specialist work through a project-local Pi extension:

- `.pi/extensions/subagent.ts`

That extension loads canonical repo-owned manifests from:

- `.pi/agents/*.md`
- `.pi/agents/teams.yaml`
- `.pi/agents/agent-chain.yaml`

and exposes:

- model-facing tools for detached dispatch/status/cancel
- user-facing slash commands for launch, status, inspection, handoff, and cancellation
- a compact Pi status/widget surface
- a session-local manager-lite overlay

The intended product is:

- **delegated specialist helpers** that a developer or main Pi session can invoke explicitly,
- running in isolated detached/background workers,
- with durable lifecycle truth and recoverable status,
- while preserving AOC provenance and ownership boundaries.

---

## 2. What this runtime is not

This path is **not**:

- a replacement for the durable detached registry,
- a replacement for Mission Control fleet summaries,
- a generic third-party subagent package drop-in,
- or a unified UX for all detached workers in every ownership plane.

Most importantly:

> **Mind workers may reuse detached lifecycle/provenance contracts, but they do not inherit delegated-specialist session UX by default.**

That means:

- delegated specialists are a **Pi-session product surface**,
- Mind workers are a **project-scoped detached worker plane**,
- and Mission Control remains the **global fleet/control surface**.

---

## 3. Canonical assets and ownership

### Canonical runtime assets

- `.pi/extensions/subagent.ts`
- `.pi/agents/*.md`
- `.pi/agents/teams.yaml`
- `.pi/agents/agent-chain.yaml`

### Control-plane/runtime integrations

- `crates/aoc-core/src/insight_contracts.rs`
- `crates/aoc-agent-wrap-rs/src/insight_orchestrator.rs`
- `crates/aoc-hub-rs/src/pulse_uds.rs`
- `crates/aoc-mission-control/src/main.rs`

### High-level rule

- `.pi/agents/*` define the delegated specialist identities and chain inputs.
- `.pi/extensions/subagent.ts` is the canonical Pi-side delegated runtime surface.
- the durable detached registry and Pulse-backed status flow remain the lifecycle source of truth when available.

---

## 4. Current operator surfaces

## Tools

### `aoc_subagent`
Detached AOC-native subagent runtime tool.

Supported actions:
- `dispatch`
- `dispatch_team`
- `dispatch_chain`
- `status`
- `cancel`
- `list_agents`

Use it when the main Pi agent needs to:
- launch one canonical project-local detached agent,
- launch one canonical detached team fanout,
- launch one canonical detached chain,
- inspect actual detached job state,
- or cancel a detached job.

### `aoc_specialist_role`
Explicit human-in-command specialist-role tool.

Supported actions:
- `dispatch`
- `status`
- `cancel`
- `list_roles`

Roles currently mapped in the extension:
- `scout`
- `planner`
- `builder`
- `reviewer`
- `documenter`
- `red-team`

`builder` and `red-team` require explicit write approval for write-like/destructive requests.

---

## Slash commands

### Catalog / discovery
- `/subagent-agents`
- `/specialist-roles`

### Launch
- `/subagent-run [--wait|--summary|--background] <agent> :: <task>`
- `/subagent-team [--wait|--summary|--background] <team> :: <task>`
- `/subagent-chain [--wait|--summary|--background] <chain> :: <task>`
- `/specialist-run [--wait|--summary|--background] <role> :: <task> [:: approve-write]`
- `/subagent-explore [--wait|--summary|--background] <task>`
- `/subagent-review [--wait|--summary|--background] <task>`
- `/subagent-test [--wait|--summary|--background] <task>`
- `/subagent-scout [--wait|--summary|--background] <task>`

### Status / recovery / handoff
- `/subagent-status [job-id]`
- `/subagent-recent [count]`
- `/subagent-history [count]`
- `/subagent-failures [count]`
- `/subagent-team-detail <team>`
- `/subagent-chain-detail <chain>`
- `/subagent-rerun [--as-is] <job-id>`
- `/subagent-inspect <job-id>`
- `/subagent-handoff <job-id>`
- `/subagent-cancel <job-id>`

### Overlay
- `/subagent-inspector`
- `/subagent-manager`
- `Alt+A`

---

## 5. Lifecycle model

Detached delegated jobs use an explicit lifecycle model.

### Core terminal/runtime states
- `queued`
- `running`
- `success`
- `fallback`
- `error`
- `cancelled`

### Recovery-specific state
- `stale`

`stale` means AOC found evidence of a previously active detached job after session/extension interruption, but the local session can no longer safely treat it as a live in-memory run.

Operator meaning:
- **not** silent success
- **not** proof of completion
- **requires review or rerun**

### Expected lifecycle behavior
- a job is created with a detached identity and metadata
- status is refreshed from the durable registry when possible
- terminal jobs get a concise handoff summary
- recovery paths preserve degraded state rather than hiding it

### Fail-open rule
If manifests, spawn, provider execution, or detached runtime bridging fail, the system must surface:
- explicit fallback/error state,
- preserved reason/excerpt where possible,
- and operator-reviewable output.

---

## 6. Source of truth and recovery model

## Primary truth
When Pulse / wrapper integration is available, the durable detached registry is the source of truth for:
- job status
- recovery
- cancellation
- cross-session visibility

## Session-local state
The Pi extension also keeps session-visible entries for:
- recent jobs
- handoff notifications
- compact in-session status rendering

These session entries are **not** the authoritative lifecycle source when durable registry state is available.

## Restart behavior
If the extension restores a job that was still `queued` or `running` in local session state but no live ownership is attached anymore, it marks the job `stale` and preserves recovery context for operator review.

### Operator rule of thumb
If a run matters and you see uncertainty:
1. check `/subagent-status <job-id>`
2. inspect `/subagent-inspect <job-id>`
3. treat `stale` as interrupted/needs review
4. rerun when confidence is not sufficient

---

## 7. Specialist roles and approval semantics

AOC exposes two related but distinct surfaces:

### Canonical agents / chains
These are raw delegated specialist assets under `.pi/agents/*`.

Examples already referenced by AOC docs:
- `explorer-agent`
- `code-review-agent`
- `testing-agent`
- `scout-web-agent`

### Explicit specialist roles
These are operator-facing role contracts mapped to backing agents.

Current role mappings in `.pi/extensions/subagent.ts`:
- `scout` -> `explorer-agent`
- `planner` -> `planner-agent`
- `builder` -> `builder-agent`
- `reviewer` -> `code-review-agent`
- `documenter` -> `documenter-agent`
- `red-team` -> `red-team-agent`

### Approval rule
The following roles currently require explicit write approval for write-like requests:
- `builder`
- `red-team`

Approval must be explicit:
- tool path: `approveWrite=true`
- slash path: `:: approve-write`

If approval is not supplied, the run should not silently escalate.

---

## 8. Provenance and trust model

AOC aligns detached delegated execution with Pi 0.62 provenance-aware tool policy.

### Default trust rule
Detached canonical subagents trust only:
- `builtin` tools
- `project-local` tools

By default they do **not** trust:
- sdk-injected tools
- non-project extension tools

unless explicitly enabled through the extension/runtime policy.

### Why this matters
Detached subagents are effectively delegated prompts plus tool access. Repo-controlled manifests are useful, but they are still powerful. Provenance-aware filtering keeps delegated execution aligned with repo trust and operator expectations.

### Operator-visible provenance
Status/handoff views preserve tool provenance summaries so operators can see, at a glance, what trust tier the delegated job ran under.

---

## 9. Mind context packs and delegated-vs-Mind boundary

Some explicit specialist roles may request a bounded Mind v2 context pack during dispatch.

### Current behavior
- role runs attempt to attach bounded Mind context when available
- the dispatch path fails open when Mind context is unavailable
- job metadata records whether context was attached

### Important boundary
This does **not** mean delegated specialist jobs become Mind workers.

The boundary is:
- delegated specialists may **consume** bounded Mind context
- Mind workers may **reuse** detached lifecycle metadata
- but the two are not the same product surface

Mission Control and ownership-aware fleet summaries are the right place for cross-plane/global visibility.

---

## 10. Current Pi-session UX

The current UX is intentionally low-noise.

### Current surfaces
- compact footer/status summary for active/recent jobs
- bounded below-editor widget for active/recent jobs
- manager-lite overlay via `Alt+A` / `/subagent-manager`
- structured status / inspect / handoff commands
- completion notifications for terminal jobs

### Current manager posture
Today's overlay is best understood as:
- **manager-lite**, not a full dashboard
- useful for browsing recent jobs plus agent / team / chain / role catalogs
- intentionally lighter than a persistent multitool dashboard
- opens immediately from `Alt+A` / `/subagent-manager` and hydrates detached status in the background
- includes clarify-before-run launch preflight for agents, teams, chains, roles, and reruns
- exposes explicit execution-mode selection (`background`, `inline_wait`, `inline_summary`)
- exposes role-aware approval toggles and Mind context-pack availability hints before dispatch
- highlights recent non-success jobs needing attention in the recent tab
- supports `f` in the recent tab to jump directly to the latest attention-needed run
- shows team membership previews and latest team-run context in the teams tab
- shows chain step previews, latest chain-run context, and `r` rerun-via-clarify from the chains tab

### Productization follow-on
Task `181` under tag `subagent-ux` is the follow-on productization track for:
- manager-lite overlay
- stable report artifacts
- clarify-before-run launch flow
- explicit execution modes
- chain catalog ergonomics
- recursion/session-mode guardrails

History and recent-failure visibility are now part of the current Pi session surface:
- `/subagent-recent` and `/subagent-history` show compact terminal run history
- `/subagent-failures` shows recent non-success jobs needing attention
- `/subagent-team-detail <team>` shows team members plus recent team runs
- `/subagent-chain-detail <chain>` shows ordered chain steps plus recent chain runs
- `/subagent-rerun [--as-is] <job-id>` reuses preserved metadata so operators do not have to reconstruct launch context manually
- the manager recent tab calls out attention-needed runs and supports `f` to jump to the latest one
- the compact widget/status line surfaces attention counts without turning the session into a dashboard

This lets AOC adopt the strongest UX lessons from `pi-subagents` without giving up AOC-native runtime ownership.

---

## 11. Artifacts and reports

## Current state
The delegated runtime now persists both structured session entries and stable per-job report bundles under:
- `.pi/tmp/subagents/<job-id>/report.md`
- `.pi/tmp/subagents/<job-id>/meta.json`
- `.pi/tmp/subagents/<job-id>/events.jsonl`
- `.pi/tmp/subagents/<job-id>/prompt.md`
- `.pi/tmp/subagents/<job-id>/stderr.log`

Those artifacts complement, but do not replace:
- durable detached registry / Pulse lifecycle truth
- session-local notifications
- recoverable inspect/handoff views

### Why this is important
The design intent is:
- keep inline session UX compact
- move deep drilldown to stable artifact references
- keep durable registry state as lifecycle truth
- avoid turning the main Pi session into a transcript dump

Primary review path:
- `/subagent-status <job-id>` for lifecycle truth
- `/subagent-inspect <job-id>` for a fuller compact review
- `/subagent-handoff <job-id>` for concise operator handoff
- stable `report.md` / `meta.json` / `stderr.log` artifacts for deeper drilldown

For team / parallel jobs specifically:
- inspect and handoff views now summarize per-member outcomes when durable `step_results` are available
- artifact reports include a bounded `Step Results` section so operators can review member-level success/fallback/cancel state without opening raw logs first

---

## 12. Recommended operator workflow

## Launch
Use one of:
- direct commands:
  - `/subagent-explore [--wait|--summary|--background] <task>`
  - `/subagent-review [--wait|--summary|--background] <task>`
  - `/subagent-test [--wait|--summary|--background] <task>`
  - `/subagent-scout [--wait|--summary|--background] <task>`
  - `/subagent-run [--wait|--summary|--background] <agent> :: <task>`
  - `/subagent-team [--wait|--summary|--background] <team> :: <task>`
  - `/subagent-chain [--wait|--summary|--background] <chain> :: <task>`
  - `/specialist-run [--wait|--summary|--background] <role> :: <task> [:: approve-write]`
- manager-assisted clarify flow:
  - `Alt+A` or `/subagent-manager`
  - browse agents / teams / chains / roles
  - press `Enter` to open launch preflight
  - edit task and cwd
  - choose execution mode (`background`, `inline_wait`, or `inline_summary`)
  - toggle builder/red-team approval when applicable
  - review Mind context-pack availability hint for specialist roles
  - confirm detached dispatch

### Execution modes
- `background` — queue detached work immediately and return control to the main session
- `inline_wait` — queue detached work, then wait briefly for terminal status before falling back to background continuation if needed
- `inline_summary` — queue detached work, then wait briefly for terminal status and prefer a concise handoff view when completion arrives in time

All three modes keep the detached registry / Pulse flow as lifecycle truth. Inline modes change parent-session completion behavior, not the underlying detached runtime model.

### Guardrails
The delegated Pi surface intentionally keeps session semantics narrow:
- delegated launches only support a fresh detached session mode today
- requests such as `--session-mode ...`, `--reuse-session`, or `--inherit-session` fail fast instead of silently guessing behavior
- nested delegated dispatch from inside an already-detached delegated run is blocked to prevent runaway recursion and ambiguous context inheritance
- use handoff back to the parent session or Mission Control supervision instead of delegated-in-delegated nesting

## Monitor
Use:
- compact status line/widget for passive awareness, including attention counts for recent failures
- `/subagent-status` for explicit truth
- `/subagent-recent` or `/subagent-history` for compact terminal run history
- `/subagent-failures` for recent detached runs needing attention
- `/subagent-team-detail <team>` for bounded team membership and recent-run drilldown
- `Alt+A` for focused review, launch, rerun preflight, and latest-failure jump within the recent tab
- `bin/aoc-subagent-supervision-toggle` for a Zellij floating-pane fast path into Mission Control Fleet with the delegated plane preselected
- manager open uses cached local state first and refreshes detached registry status in the background when Pulse is reachable

### Fast supervision pane
For lower-latency detached supervision outside the Pi overlay, use:
- `aoc-subagent-supervision-toggle`

Bind that launcher to your preferred Zellij shortcut if you want a one-keystroke supervision path distinct from Pi's in-session `Alt+A` manager overlay.

This opens or focuses one named floating pane that runs Mission Control in:
- `mission-control` runtime mode
- `Fleet` view
- `delegated` plane filter by default

Boundary:
- Pi remains the launch / clarify / approval surface
- the floating pane is the fast observability / drilldown surface
- Pulse / durable detached registry remain the lifecycle source of truth

## Recover / inspect
Use:
- `/subagent-recent`
- `/subagent-history`
- `/subagent-failures`
- `/subagent-team-detail <team>`
- `/subagent-chain-detail <chain>`
- `/subagent-rerun [--as-is] <job-id>`
- `/subagent-inspect <job-id>`
- `/subagent-handoff <job-id>`
- `/subagent-cancel <job-id>`

## When to rerun
Rerun if:
- the job is `stale`
- fallback/error output is insufficient
- role approval was missing for intended write-like work
- bounded Mind context was unavailable and materially needed
- the operator needs a cleaner or narrower prompt/task framing

Rerun paths:
- `/subagent-rerun <job-id>` opens the clarify-before-run flow using preserved metadata
- `/subagent-rerun --as-is <job-id>` replays the preserved launch shape directly
- manager recent tab: `r` reruns the selected prior run via clarify
- manager teams tab: `r` reruns the latest run for the selected team via clarify
- manager chains tab: `r` reruns the latest run for the selected chain via clarify

---

## 13. Troubleshooting

### "Unknown detached subagent job"
Possible causes:
- wrong job id
- older job aged out of session-local visibility
- local session never observed the job and you need registry-backed status refresh

Try:
- `/subagent-recent`
- `/subagent-history`
- `/subagent-failures`
- `/subagent-chain-detail <chain>`
- `/subagent-status`
- rerun from the main session if the work is not recoverable enough

### Job shows `stale`
Meaning:
- the job was previously active
- the extension/session was reloaded or interrupted
- the current session cannot safely keep treating it as an in-memory live run

Action:
- inspect it
- verify registry-backed status if available
- rerun if the result is ambiguous

### Manifest/agent errors
Possible causes:
- malformed `.pi/agents/*.md`
- missing agent referenced by chain/role
- chain references unknown agent names

Action:
- run `/subagent-agents`
- inspect manifest files under `.pi/agents/`
- review validation warnings surfaced in tool results

### Write-oriented role refused to proceed
Likely cause:
- `builder` or `red-team` requested without approval

Action:
- re-run with explicit approval only if the write/destructive request is intended

### Unsupported session mode request
Likely cause:
- a delegated launch tried to request session reuse/inheritance semantics that are not supported yet

Action:
- use the default detached launch flow
- remove `--session-mode`, `--reuse-session`, `--inherit-session`, or similar flags
- if you need cross-plane/global supervision, use Mission Control rather than trying to turn delegated runs into a shared-session product surface

### Nested delegated launch blocked
Likely cause:
- a detached delegated run attempted to dispatch another delegated run from inside itself

Action:
- hand results back to the parent session
- rerun or fan out from the parent operator session instead
- use Mission Control for supervision rather than recursive delegated nesting

### Detached job completed but main session feels too poll-driven
Current best path is:
- completion notification
- `/subagent-inspect`
- `/subagent-handoff`
- `/subagent-history`
- `/subagent-failures`
- `Alt+A` for the manager recent tab, including the latest-failure jump

Task `181` remains the dedicated follow-on for chain ergonomics and guardrails, but history/failure visibility is now part of the current session surface.

---

## 14. Validation and rollout checks

Current relevant validation surfaces include:
- `scripts/pi/test-specialist-role-surface.sh`
- `scripts/pi/test-specialist-role-runtime-guards.sh`
- `scripts/pi/test-subagent-ux-surface.sh`
- `scripts/pi/test-subagent-ux-runtime.sh`
- `scripts/pi/test-pi-only-agent-surface.sh`

Recommended operator smoke checks after subagent/runtime changes:
1. `/subagent-agents`
2. `/specialist-roles`
3. `/subagent-explore <task>`
4. `/subagent-status`
5. `Alt+A`
6. `/subagent-inspect <job-id>` for a completed job
7. builder/red-team approval-path check when relevant

For deeper detached orchestration and fleet validation, continue to rely on:
- Mission Control detached summaries
- Pulse/durable-registry-backed status behavior
- task 169 / 178 / 149 regression coverage

---

## 15. Related docs

- `docs/agents.md`
- `docs/insight-subagent-orchestration.md`
- `.taskmaster/docs/prds/task-169_aoc_detached_pi_subagent_runtime_prd_rpg.md`
- `.taskmaster/docs/prds/task-129_pi-specialist-role-interface_prd.md`
- `.taskmaster/docs/prds/aoc_detached_orchestration_prd_rpg.md`
- `.taskmaster/docs/prds/pi_subagent_ux_alignment_prd_rpg.md`

## Comparative references

- Pi example: `examples/extensions/subagent/README.md`
- Comparative external reference: `https://github.com/nicobailon/pi-subagents`
