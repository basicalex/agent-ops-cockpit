# Mind Runtime Live Validation

This runbook captures the exact checks for task `142`:

- fresh AOC/Pi session wiring on current repo binaries
- Pulse UDS bootstrap health
- live `mind_*` command roundtrips
- durable Mind artifacts + provenance evidence
- operator-visible success/failure signals in Pi UI, Mission Control, and logs

## One-command validators

Quick live validator:

```bash
bash scripts/pi/validate-mind-runtime-live.sh
```

Broader task-142 hardening suite:

```bash
bash scripts/pi/validate-mind-runtime-hardening.sh
```

### Live validator: what it does

1. starts a fresh temp project root
2. starts `bin/aoc-hub`
3. starts a fresh wrapped Pi session via `bin/aoc-agent-wrap`
4. verifies standalone/runtime state is discoverable for the project, including derived service-state honesty (`running` / `degraded` / `stale` / `inactive` / `cold`)
5. exercises live Mind command surfaces across the current runtime split:
   - standalone ingest / sync into `.aoc/mind/project.sqlite`
   - standalone `observer-run`
   - standalone `finalize-session`
   - standalone `context-pack`
   - compatibility/detached status queries where still runtime-owned elsewhere
6. verifies:
   - standalone/runtime state exists
- `aoc-mind-service status --json` returns a canonical `service_status` summary with explicit stale/degraded detection
   - `.aoc/mind/project.sqlite` exists
   - an insight export bundle exists with `t1.md`, `t2.md`, and `manifest.json`
   - provenance export returns `graph.status = ok`
   - Mind-owned detached status queries return a healthy response (`status = ok` or `status = idle` depending on whether Mind-owned jobs are active at that instant)
   - detached rows returned through that query, if any, are all `owner_plane = Mind`
   - hub + wrapper log files were written under `.aoc/logs/`

By default the temp workspace is cleaned up on success.
Set `AOC_KEEP_MIND_RUNTIME_LIVE_TMP=1` to preserve it for inspection.
Set `AOC_VALIDATE_MIND_RUNTIME_USE_CARGO=1` to force current-source `cargo run` instead of any previously built wrapper/hub binaries.
Set `AOC_VALIDATE_MIND_RUNTIME_TIMEOUT_SEC=<n>` to raise command/snapshot wait budgets when validating from source on a cold build.

## Hardening suite coverage

`validate-mind-runtime-hardening.sh` runs the live validator above and then
executes the most important bounded Mind-runtime recovery regressions from
`crates/aoc-agent-wrap-rs/src/main.rs`:

- `mind_startup_reconciles_stale_detached_t2_and_t3_jobs`
- `mind_detached_t2_worker_respects_cancelled_job_state`
- `mind_detached_t3_worker_respects_cancelled_job_state`
- `mind_t2_dispatcher_stamps_detached_jobs_and_falls_back_inline`
- `mind_t3_dispatcher_stamps_detached_jobs_and_falls_back_inline`
- `multi_session_finalize_stress_drains_t3_backlog_without_duplicates`
- `migration_creates_mind_tables`
- `replay_stability_keeps_same_t0_hash_for_same_policy`
- `pulse_mind_compaction_rebuild_replays_latest_checkpoint_and_requeues_observer`
- `compaction_t0_slice_is_deterministic_and_dedupes_lists`

This gives task 142 a single operator/maintainer command that covers:

- live standalone Mind roundtrips
- detached Mind visibility
- stale-lease recovery on startup
- cancel handling for detached T2/T3 workers
- deterministic inline fallback when detached worker spawn fails
- finalization drains and dedupes T3 backlog work
- storage migration safety for the Mind schema
- replay stability for the same T0 policy input
- checkpoint-driven rebuild/requeue safety
- deterministic compaction-derived T0 slice generation

## Pi launch-mode expectation

Pi now prefers `aoc-agent-wrap-rs` by default when that binary is available.
For managed AOC Zellij panes, the hot path is now intentionally thin:

```text
zellij pane
  -> aoc-agent-wrap
    -> aoc-agent-wrap-rs
      -> pi
```

Managed-pane defaults:

- `AOC_PI_USE_WRAP_RS=1`
- `AOC_AGENT_PTY=1`
- `AOC_PI_USE_BOOTLOADER=0`
- `AOC_PI_USE_TMUX=0`

Override only when needed:

- `AOC_PI_USE_WRAP_RS=1` — force wrapper mode
- `AOC_PI_USE_WRAP_RS=0` — explicit legacy direct-exec fallback
- unset / `auto` — prefer wrapper when available, otherwise fall back direct
- `AOC_PI_USE_BOOTLOADER=1` — opt back into the shell handshake path
- `AOC_PI_USE_TMUX=1` — opt back into nested tmux for PI

## Operator-visible signals

### Pi footer / commands

Defined across the native Pi Mind extension stack:

- `.pi/extensions/minimal.ts`
  - footer shows AOC Mind observer state:
    - `idle`
    - `queued`
    - `running`
    - `success`
    - `fallback`
    - `error`
- `.pi/extensions/mind-ops.ts`
  - `Alt+M` / `/mind` toggles the project Mind floating UI
  - `/mind-observer-run` queues a manual observer run
  - `/mind-status` shows ingest/runtime health
  - `/aoc-status` shows managed launch/runtime health
  - `/mind-finalize` requests finalize/export
- `.pi/extensions/mind-context.ts`
  - `/mind-pack`
  - `/mind-pack-expanded`
  - `/mind-resume`
- `.pi/extensions/mind-focus.ts`
  - `/mind-focus`
- notification strings include:
  - `Project Mind toggled`
  - `Observer run queued`
  - `Mind sync failed: ...` (standalone service/runtime warning; inspect the surfaced error text rather than assuming transport failure)
  - `Mind UI unavailable: ...`

### Mission Control

Mission Control currently shows live Mind health from wrapper-driven runtime snapshots, while the architecture is being cut over toward canonical standalone/service-owned health, including:

- observer feed status transitions
- `queue_depth`
- `t3_queue_depth`
- `supervisor_runs`
- `last_error`
- queue summaries like `t2q:<n> t3q:<n>`
- status notes such as:
  - `hub connected`
  - `hub offline; command unavailable`
  - `<command> queued for <target>`
  - `hub delta gap detected; awaiting resync`

For detached Mind work, Fleet also shows ownership-aware rows for the current
shipped detached slice:

- `t2-reflector`
- `t3-runtime`

T1 remains inline/session-scoped in the first detached rollout slice.

## Durable evidence

After `mind_finalize_session`, inspect:

- `.aoc/mind/project.sqlite`
- `.aoc/mind/insight/<export>/t1.md`
- `.aoc/mind/insight/<export>/t2.md`
- `.aoc/mind/insight/<export>/manifest.json`

For logs, inspect:

- `.aoc/logs/aoc-hub-<session>.log`
- `.aoc/logs/aoc-agent-wrap-<session>-<agent>.log`

## Notes

- `validate-mind-runtime-live.sh` is the fastest smoke check.
- `validate-mind-runtime-hardening.sh` is the broader pre-release / rollout
  confidence command.

- The validator is intentionally non-interactive so it can run in a normal shell.
- It validates the current standalone-first Pi Mind command surface plus the remaining bounded compatibility/runtime seams, even though it uses a synthetic session/task payload.
- If the validator fails, rerun with `AOC_KEEP_MIND_RUNTIME_LIVE_TMP=1` and
  inspect the preserved workspace, especially `.aoc/logs/` and `.aoc/mind/`.
