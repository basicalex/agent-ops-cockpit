# Mind Runtime Live Validation

This runbook captures the exact checks for task `167`:

- fresh AOC/Pi session wiring on current repo binaries
- Pulse UDS bootstrap health
- live `mind_*` command roundtrips
- durable Mind artifacts + provenance evidence
- operator-visible success/failure signals in Pi UI, Mission Control, and logs

## One-command validator

Run:

```bash
bash scripts/pi/validate-mind-runtime-live.sh
```

What it does:

1. starts a fresh temp project root
2. starts `bin/aoc-hub`
3. starts a fresh wrapped Pi session via `bin/aoc-agent-wrap`
4. verifies Pulse snapshot state arrives over the UDS socket
5. sends live runtime commands through Pulse:
   - `mind_ingest_event`
   - `run_observer`
   - `mind_handoff`
   - `mind_finalize_session`
   - `mind_provenance_query`
   - `insight_detached_status` with `owner_plane=mind`
6. verifies:
   - snapshot state exists
   - Mind fields are visible over Pulse
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

## Pi launch-mode expectation

Pi now prefers `aoc-agent-wrap-rs` by default when that binary is available.
This keeps live Pulse + Mind runtime activation aligned with the documented
session model.

Override only when needed:

- `AOC_PI_USE_WRAP_RS=1` — force wrapper mode
- `AOC_PI_USE_WRAP_RS=0` — explicit legacy direct-exec fallback
- unset / `auto` — prefer wrapper when available, otherwise fall back direct

## Operator-visible signals

### Pi footer / commands

Defined in `.pi/extensions/minimal.ts`:

- footer shows AOC Mind observer state:
  - `idle`
  - `queued`
  - `running`
  - `success`
  - `fallback`
  - `error`
- `Alt+M` / `/mind` toggles the project Mind floating UI
- `/mind-observer-run` queues a manual observer run
- notification strings include:
  - `Project Mind toggled`
  - `Observer run queued`
  - `Observer run unavailable (Pulse disconnected)`
  - `Mind UI unavailable: ...`

### Mission Control

Mission Control shows live Mind health from wrapper snapshots, including:

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

- The validator is intentionally non-interactive so it can run in a normal shell.
- It validates the same Pulse command surfaces used by the live Pi/Mission
  Control path, even though it uses a bounded synthetic session/task payload.
- If the validator fails, rerun with `AOC_KEEP_MIND_RUNTIME_LIVE_TMP=1` and
  inspect the preserved workspace, especially `.aoc/logs/` and `.aoc/mind/`.
