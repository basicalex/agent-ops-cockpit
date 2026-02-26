- [2026-02-25 15:59] [2026-02-25 15:58] Resumed via STM. Started task 121, authored PI-first ownership contract at .taskmaster/docs/prds/aoc_pi_cleanup_contract.md, updated PRD with contract resolution, marked task 121 done, and recorded decision in aoc-mem.
- [2026-02-25 16:03] [2026-02-25 16:03] Completed task 122. Updated bin/aoc-init for deterministic PI baseline dirs (.pi/prompts,.pi/skills,.pi/extensions), required prompt allowlist seeding with canonical-first source order + legacy fallback, no tmcc prompt seeding, and explicit PI skill sync. Verified with fresh temp repo idempotent rerun and scripts/opencode/test-aoc-init-omo.sh.
- [2026-02-25 16:13] [2026-02-25 16:07] Completed task 123. Updated setup_pi_prompts in bin/aoc-init to canonical-first seed resolution (.config/aoc/pi/prompts, bundled .pi/prompts) with legacy fallback (.aoc/prompts/pi + legacy config path) and explicit compatibility logging. docs/agents.md now states .aoc/prompts/pi is fallback-only during compatibility window. Validated fresh + existing upgrade path (tm-cc restored, tmcc preserved) and OmO regression script.
- [2026-02-25 16:18] HANDOFF — AOC Mind PI-native semantic runtime + specialist role orchestration

Date: 2026-02-25
Repo: /home/ceii/dev/agent-ops-cockpit

What changed this session
- Re-scoped Mind Task 108 away from external-first inference toward PI-native semantic runtime.
- Updated task-level PRD for 108:
  .taskmaster/docs/prds/task-108_semantic-om-background-layer_prd.md
- Updated Taskmaster task/subtasks for 108 to reflect production topology.
- Updated Task 109 details/tests so context packs support active specialist dispatch.
- Added new Task 129 + PRD for explicit human-in-command specialist interface:
  .taskmaster/docs/prds/task-129_pi-specialist-role-interface_prd.md
- Recorded durable memory decision via aoc-mem add.

Core architecture decision (authoritative for next implementation session)
1) Background memory runtime only (AOC Mind)
   - T1 Observer: one background sidecar per active PI session.
   - T2 Reflector: one singleton detached worker per project/session scope.
2) Coordination safety
   - Advisory lock file + durable DB lease/job-claim semantics.
   - Goal: prevent duplicate T2 reflection writes when multiple sessions are active.
3) Reliability model
   - Semantic path is additive quality layer.
   - Deterministic distiller remains fail-open baseline and system-of-record fallback.
4) Human control model
   - Specialist agents are NOT autonomous background swarm.
   - Scout / Planner / Builder / Reviewer / Documenter / Red Team are actively invoked by developer.

Why this changed
- Prior plan emphasized optional external provider adapters (Zen etc.).
- New production direction uses PI-native providers/OAuth/runtime to reduce integration burden and align with session-native orchestration patterns.

Evidence and reference from example repos/docs
- External example checked: https://github.com/disler/pi-vs-claude-code
- Key patterns adopted:
  - Spawn child PI processes for isolated subagents in JSON/print mode.
  - Persist per-agent session files for continuity.
  - Use extension tools/commands for explicit dispatch.
  - Keep dashboard/status visibility for operator control.
- PI docs validated (extensions.md + subagent example):
  - Extensions can spawn subprocesses, register tools/commands, and handle lifecycle events.
  - Session events and tool hooks support background orchestration.

Current Taskmaster state in tag=mind
- 108 pending/high (subtask 1 in-progress)
- 109 pending/high
- 110 pending/medium
- 129 pending/high
- 101–107 done/high foundations already in place.

Task 108 (current structure)
- [1] Define semantic runtime interfaces and canonical payload contracts (in-progress)
- [2] Implement session-scoped T1 Observer sidecar via PI runtime
- [3] Implement singleton detached T2 Reflector with lock/lease coordination
- [4] Add PI model profiles and runtime guardrails
- [5] Implement fail-open deterministic fallback and provenance persistence
- [6] Ship fixture/integration suite for concurrency, locking, and fallback

Task 129 (new)
- Human-in-command specialist interface with six roles.
- Depends on 108 + 109.
- Includes policy gates, role tool scopes, telemetry UI, and validation suite.

Suggested execution order next session
A) Finish 108.1 contracts first
   - Finalize ObserverAdapter/ReflectorAdapter DTOs and failure kinds.
   - Finalize canonical input hashing + schema validation boundaries.
B) Implement 108.2 T1 observer sidecar
   - Per-session queue + debounce + one active run invariant.
   - PI subprocess invocation profile (low-cost default model).
C) Implement 108.3 singleton T2 reflector
   - Lock path proposal: .aoc/mind/locks/reflector.lock
   - Add durable lease + atomic job claim + stale-lease takeover tests.
D) Implement 108.4 + 108.5
   - Guardrails (timeout/token/cost/retry), fallback metadata, provenance persistence.
E) Implement 108.6 test matrix
   - Multi-session contention, lock conflict, stale recovery, fallback correctness.
F) Start 109 if needed for role-ready context slices, then begin 129 role UX.

Critical invariants to preserve
- No cross-conversation mixing in T1.
- Deterministic chunk ordering when over budget.
- T2 aggregation bounded by tag/workstream policy; cross-tag off by default.
- Non-blocking interactive PI UX while background jobs run.
- Explicit approvals for write/destructive specialist actions.

Files of interest (resume quickly)
- PRD: .taskmaster/docs/prds/aoc-mind_prd.md
- Task 108 PRD: .taskmaster/docs/prds/task-108_semantic-om-background-layer_prd.md
- Task 129 PRD: .taskmaster/docs/prds/task-129_pi-specialist-role-interface_prd.md
- Existing deterministic distiller: crates/aoc-mind/src/lib.rs
- Existing ingestion/checkpointing: crates/aoc-opencode-adapter/src/lib.rs
- Storage schema baseline: crates/aoc-storage/migrations/0001_mind_schema.sql

Operational note
- tm tag current currently returns aoc/pi_cleanup; when resuming Mind work, explicitly use --tag mind or switch tag before edits.
