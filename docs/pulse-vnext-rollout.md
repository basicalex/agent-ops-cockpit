# Pulse vNext Rollout and Observability

This document defines rollout controls and the minimum observability signals for Pulse vNext.

## Feature Flag and Rollback

- `AOC_PULSE_VNEXT_ENABLED=1` (default): enables Pulse UDS in `aoc-hub-rs` and Pulse subscriber mode in `aoc-mission-control`.
- `AOC_PULSE_VNEXT_ENABLED=0`: disables Pulse vNext paths and keeps prior behavior available:
  - `aoc-hub-rs` runs websocket hub only (no Pulse UDS task).
  - `aoc-mission-control` stays in local fallback mode.
- `AOC_PULSE_OVERVIEW_ENABLED=0` (default): keeps Pulse Overview deprecated/off in mission-control and runs Work/Diff/Health as primary modes.
- `AOC_PULSE_OVERVIEW_ENABLED=1`: opt-in re-enable of Pulse Overview for bake/testing.

Rollback is immediate: set `AOC_PULSE_VNEXT_ENABLED=0` and restart hub/mission-control.

## Overview Deprecation Decision (2026-02-09)

- Decision: deprecate Overview by default due to low operator utility relative to perceived latency/noise.
- Scope: keep Overview/sidecar implementation in-repo behind `AOC_PULSE_OVERVIEW_ENABLED` for later phase.
- Expected benefit now: less polling churn, less UI noise, clearer operator focus on Work/Diff/Health.

Re-enable criteria for a future phase:

1. Push-fidelity is demonstrably realtime for operator-critical status transitions.
2. Overview rows provide actionable context (not only transport/heartbeat diagnostics).
3. UX gains are visible in day-to-day operation (faster triage and tab jumps).

## Structured Observability Events

- End-to-end latency
  - `pulse_end_to_end_latency` with `stage=delta_ingest|heartbeat_ingest|hub_ingest|render`
  - Includes `agent_id`, sample id, and millisecond latency fields.
- Queue drops and backpressure
  - `pulse_queue_drop` with `reason` and running drop totals.
  - `pulse_send_backpressure` with running backpressure totals.
- Parser confidence transitions
  - `pulse_parser_confidence_transition` from `aoc-agent-wrap-rs`.
  - Includes previous/next lifecycle state and confidence values.
- Layout watcher health
  - `pulse_layout_watcher_health` every watcher interval window.
  - Includes active panes, failure streak, slow-cycle count, churn totals, and queue health counters.

## Rollout Stages

1. Canary: enable Pulse vNext for one session and verify latency, queue, and watcher-health events.
2. Limited rollout: 10-20 active sessions with normal tab churn.
3. Broad rollout: enable by default and continue monitoring warning-rate thresholds.

## Suggested Alert Thresholds

- `pulse_end_to_end_latency` warning if `latency_ms >= 1500`.
- queue drops/backpressure warning if any sustained growth over 5 minutes.
- layout watcher warning if `failure_streak > 0` for consecutive health windows.
