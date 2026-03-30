#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
use_cargo="${AOC_VALIDATE_MIND_RUNTIME_USE_CARGO:-1}"
request_timeout_sec="${AOC_VALIDATE_MIND_RUNTIME_TIMEOUT_SEC:-45}"
keep_tmp="${AOC_KEEP_MIND_RUNTIME_LIVE_TMP:-0}"

run_step() {
  local label="$1"
  shift
  echo "==> $label"
  "$@"
}

run_step \
  "Live Mind runtime validation" \
  env \
    AOC_VALIDATE_MIND_RUNTIME_USE_CARGO="$use_cargo" \
    AOC_VALIDATE_MIND_RUNTIME_TIMEOUT_SEC="$request_timeout_sec" \
    AOC_KEEP_MIND_RUNTIME_LIVE_TMP="$keep_tmp" \
    bash "$repo_root/scripts/pi/validate-mind-runtime-live.sh"

run_step \
  "Mind stale-lease recovery regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-agent-wrap-rs mind_startup_reconciles_stale_detached_t2_and_t3_jobs -- --nocapture"

run_step \
  "Mind T2 cancel regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-agent-wrap-rs mind_detached_t2_worker_respects_cancelled_job_state -- --nocapture"

run_step \
  "Mind T3 cancel regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-agent-wrap-rs mind_detached_t3_worker_respects_cancelled_job_state -- --nocapture"

run_step \
  "Mind T2 inline fallback regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-agent-wrap-rs mind_t2_dispatcher_stamps_detached_jobs_and_falls_back_inline -- --nocapture"

run_step \
  "Mind T3 inline fallback regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-agent-wrap-rs mind_t3_dispatcher_stamps_detached_jobs_and_falls_back_inline -- --nocapture"

run_step \
  "Mind finalization drain/idempotence regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-agent-wrap-rs multi_session_finalize_stress_drains_t3_backlog_without_duplicates -- --nocapture"

run_step \
  "Mind storage migration regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-storage migration_creates_mind_tables -- --nocapture"

run_step \
  "Mind replay stability regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-storage replay_stability_keeps_same_t0_hash_for_same_policy -- --nocapture"

run_step \
  "Mind checkpoint rebuild/requeue regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-agent-wrap-rs pulse_mind_compaction_rebuild_replays_latest_checkpoint_and_requeues_observer -- --nocapture"

run_step \
  "Mind compaction determinism regression" \
  bash -lc "cd \"$repo_root/crates\" && cargo test -q -p aoc-core compaction_t0_slice_is_deterministic_and_dedupes_lists -- --nocapture"

echo "Mind runtime hardening suite passed."
