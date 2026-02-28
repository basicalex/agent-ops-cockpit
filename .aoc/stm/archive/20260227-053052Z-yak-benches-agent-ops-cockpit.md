- [2026-02-26 08:06] [2026-02-26 07:xx] Follow-up cleanup per user request: removed PI-R from active runtime/installer/control surfaces (aoc-agent/aoc-agent-run/aoc-agent-install/aoc-control), deleted bin/aoc-pi-r, removed PI-R consent/licensing hooks from install.sh, updated docs/env references to PI-only npm runtime, updated smoke tests to assert pi-r rejection/absence, and rebuilt aoc-control for Alt+C parity.
- [2026-02-26 08:17] [2026-02-26 08:xx] Completed zero-string cleanup for PI-R tokens in active source/docs/scripts: removed remaining pi-r/AOC_PIR references outside AOC memory/history, updated smoke tests to use generic unsupported agents, and validated via bash -n + PI smoke scripts.
- [2026-02-26 09:48] [2026-02-26 08:xx] Added PI-first extensibility docs: new docs/agent-extensibility.md with BYO wrapper flow via AOC_AGENT_CMD+aoc-agent-wrap; linked from README/agents/config; corrected mission-control-ops stale codex fallback docs; hardened aoc-agent-wrap env hint generation for custom agent IDs by sanitizing to valid env var tokens.
- [2026-02-27 06:30] Mind flow handoff (T0/T1/T2) — session distillation

Current architecture state
- T0/T1/T2 core logic exists and is implemented in Rust libs with tests.
- UI/feed plumbing for mind observer is integrated (Mission Control + Pi footer).
- Runtime orchestration is not yet fully wired into the live AOC session process path.

T0 (compaction/ingestion)
- T0 compaction contracts and storage are in place (`compact_events_t0`, checkpoints).
- OpenCode adapter normalizes lineage metadata and enforces strict canonical lineage contract.
- Branch-aware lineage migration/schema updates are in place (conversation tree support).
- Authoritative progress payload now includes T0-derived token estimate fields for feed/UI.

T1 (semantic observer)
- Semantic observer runtime/sidecar logic is implemented in `aoc-mind` with fail-open behavior.
- Guardrails and provenance semantics are enforced (fallback/error reasons, attempts, latency metadata).
- Feed event mapping now includes optional progress payload:
  - `t0_estimated_tokens`
  - `t1_target_tokens`
  - `t1_hard_cap_tokens`
  - `tokens_until_next_run`
- Mission Control and Pi extension parse/render this progress safely and prefer authoritative feed values.

T2 (reflector)
- Reflector runtime and storage schema are implemented:
  - lease table for singleton processing
  - T2 jobs queue table
  - claim/complete/fail/retry flow with lease heartbeat
- Detached reflector worker behavior and lock/lease semantics are tested.

Critical truth from this session
- Despite implementation completeness in libraries/tests, T0/T1/T2 are not yet proven as fully running in normal live AOC sessions.
- `aoc-mind` runtime is not yet clearly wired into active hub/agent-wrap execution loop in production path.
- Current live pulse mind events include queued/synthetic events; full end-to-end semantic execution loop still needs explicit runtime integration.

UI/UX decisions finalized
- Mind icon standardized to `✦` (Pi footer and Mission Control Mind surfaces).
- Pi footer center block cleaned: no ETA suffix, percent only.
- Footer layout adjusted to keep bottom-right context meter stable.

Operational follow-up
1) Wire live runtime orchestration:
   - bind mind store path/session scope
   - schedule/trigger T0 ingestion + T1 observer runs
   - enqueue/drive T2 reflector worker loop
   - publish resulting mind feed events end-to-end.
2) Add runtime-level verification that demonstrates real T0/T1/T2 execution in-session.
3) Keep fail-open + provenance accounting guarantees unchanged.

AOC init/default extensions follow-up captured
- `aoc-init` now contains extension seeding logic for `.pi/extensions/minimal.ts` + `themeMap.ts`.
- Real-world install issue observed: seed source dirs absent on installed system (`~/.config/aoc/pi/extensions` and script-relative fallback), so files were not seeded in another repo.
- Packaging/install path must ship these templates (and/or add built-in fallback in `aoc-init`).
