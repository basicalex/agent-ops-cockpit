#!/usr/bin/env bash
set -euo pipefail

# Backfill AOC Mind from Pi session history for this project.
# - Ingests every project-local Pi session JSONL via aoc-mind-service sync-pi --all.
# - Runs the Mind observer for conversations that have T0 compact events but no T1 observations.
# - Tool outputs are not persisted by the adapters; Mind keeps message text plus tool metadata only.

PROJECT_ROOT="${1:-$(pwd)}"
AGENT_ID="${AOC_MIND_BACKFILL_AGENT_ID:-aoc-mind-backfill}"
SESSION_ID="${AOC_MIND_BACKFILL_SESSION_ID:-mind-backfill}"
PANE_ID="${AOC_MIND_BACKFILL_PANE_ID:-mind-backfill}"
SERVICE_BIN="${AOC_MIND_SERVICE_BIN:-aoc-mind-service}"
SQLITE_BIN="${SQLITE_BIN:-sqlite3}"
MODE="${AOC_MIND_BACKFILL_MODE:-missing-t1}" # missing-t1 | all
RUN_REFLECTOR="${AOC_MIND_BACKFILL_REFLECTOR:-1}"
RUN_T3="${AOC_MIND_BACKFILL_T3:-1}"
FINALIZE_T3="${AOC_MIND_BACKFILL_FINALIZE_T3:-1}"
MAX_T3_TICKS="${AOC_MIND_BACKFILL_MAX_T3_TICKS:-200}"

if [[ ! -d "$PROJECT_ROOT" ]]; then
  echo "project root not found: $PROJECT_ROOT" >&2
  exit 2
fi

if ! command -v "$SERVICE_BIN" >/dev/null 2>&1; then
  echo "missing $SERVICE_BIN" >&2
  exit 2
fi
if ! command -v "$SQLITE_BIN" >/dev/null 2>&1; then
  echo "missing $SQLITE_BIN" >&2
  exit 2
fi

# Prefer the service-resolved store. AOC_MIND_STORE_PATH may be set by older
# wrappers and can point at a legacy path, while aoc-mind-service status is the
# authoritative project-scoped location.
STORE_PATH="$($SERVICE_BIN status --project-root "$PROJECT_ROOT" 2>/dev/null | awk -F': ' '/^store_path:/ {print $2; exit}')"
if [[ -n "${AOC_MIND_STORE_PATH_OVERRIDE:-}" ]]; then
  STORE_PATH="$AOC_MIND_STORE_PATH_OVERRIDE"
fi
if [[ -z "$STORE_PATH" ]]; then
  runtime_key="$(printf '%s' "$PROJECT_ROOT" | sed 's#/#_#g')"
  STORE_PATH="$HOME/.local/state/aoc/mind/projects/${runtime_key}/project.sqlite"
fi

before_t1=0
if [[ -f "$STORE_PATH" ]]; then
  before_t1="$($SQLITE_BIN "$STORE_PATH" "select count(*) from observations_t1;" 2>/dev/null || echo 0)"
fi

echo "== AOC Mind Pi history backfill =="
echo "project_root: $PROJECT_ROOT"
echo "store_path: $STORE_PATH"
echo "mode: $MODE"
echo

echo "-- sync all Pi session files --"
"$SERVICE_BIN" sync-pi --project-root "$PROJECT_ROOT" --agent-id "$AGENT_ID" --all --json

if [[ ! -f "$STORE_PATH" ]]; then
  echo "Mind store not found after sync: $STORE_PATH" >&2
  exit 1
fi

echo
echo "-- select conversations --"
case "$MODE" in
  all)
    query="select distinct conversation_id from compact_events_t0 order by conversation_id;"
    ;;
  missing-t1)
    query="
      select distinct c.conversation_id
      from compact_events_t0 c
      where not exists (
        select 1 from observations_t1 o where o.conversation_id = c.conversation_id
      )
      order by c.conversation_id;
    "
    ;;
  *)
    echo "unknown AOC_MIND_BACKFILL_MODE=$MODE; expected missing-t1 or all" >&2
    exit 2
    ;;
esac

mapfile -t conversations < <("$SQLITE_BIN" "$STORE_PATH" "$query")
echo "conversations_selected: ${#conversations[@]}"

processed=0
failed=0
for conversation_id in "${conversations[@]}"; do
  [[ -n "$conversation_id" ]] || continue
  echo "observer: $conversation_id"
  if "$SERVICE_BIN" observer-run \
      --project-root "$PROJECT_ROOT" \
      --session-id "$SESSION_ID" \
      --pane-id "$PANE_ID" \
      --conversation-id "$conversation_id" \
      --agent-id "$AGENT_ID" \
      --reason "Pi history backfill" \
      --json >/tmp/aoc-mind-backfill-observer.json; then
    processed=$((processed + 1))
  else
    failed=$((failed + 1))
    cat /tmp/aoc-mind-backfill-observer.json >&2 || true
  fi
done
rm -f /tmp/aoc-mind-backfill-observer.json

if [[ "$RUN_REFLECTOR" == "1" ]]; then
  echo
  echo "-- reflector tick --"
  # observer-run usually emits T2 immediately; this drains any queued T2 jobs.
  while :; do
    reflector_json="$($SERVICE_BIN reflector-run --project-root "$PROJECT_ROOT" --session-id "$SESSION_ID" --pane-id "$PANE_ID" --agent-id "$AGENT_ID" --json || true)"
    echo "$reflector_json"
    claimed="$(printf '%s' "$reflector_json" | jq -r '.report.jobs_claimed // 0' 2>/dev/null || echo 0)"
    [[ "$claimed" != "0" ]] || break
  done
fi

if [[ "$FINALIZE_T3" == "1" ]]; then
  echo
  echo "-- finalize conversations into T3 backlog --"
  mapfile -t finalize_conversations < <("$SQLITE_BIN" "$STORE_PATH" "select distinct conversation_id from (select conversation_id from observations_t1 union select conversation_id from reflections_t2) order by conversation_id;")
  finalized=0
  for conversation_id in "${finalize_conversations[@]}"; do
    [[ -n "$conversation_id" ]] || continue
    # Use a per-conversation pane id so each conversation gets an independent
    # finalize watermark. A single session/pane watermark would skip older
    # conversations after the first high-watermark export.
    safe="$(printf '%s' "$conversation_id" | sha256sum | cut -c1-12)"
    "$SERVICE_BIN" finalize-session \
      --project-root "$PROJECT_ROOT" \
      --session-id "$SESSION_ID" \
      --pane-id "conv-$safe" \
      --conversation-id "$conversation_id" \
      --reason "Pi history backfill finalize" \
      --json >/dev/null || true
    finalized=$((finalized + 1))
  done
  echo "conversations_finalized: $finalized"
fi

if [[ "$RUN_T3" == "1" ]]; then
  echo
  echo "-- t3 ticks --"
  for _ in $(seq 1 "$MAX_T3_TICKS"); do
    t3_json="$($SERVICE_BIN t3-run --project-root "$PROJECT_ROOT" --session-id "$SESSION_ID" --pane-id "$PANE_ID" --agent-id "$AGENT_ID" --json || true)"
    echo "$t3_json"
    claimed="$(printf '%s' "$t3_json" | jq -r '.report.jobs_claimed // 0' 2>/dev/null || echo 0)"
    [[ "$claimed" != "0" ]] || break
  done
fi

after_t1="$($SQLITE_BIN "$STORE_PATH" "select count(*) from observations_t1;" 2>/dev/null || echo 0)"
after_t2="$($SQLITE_BIN "$STORE_PATH" "select count(*) from reflections_t2;" 2>/dev/null || echo 0)"
after_t3_jobs="$($SQLITE_BIN "$STORE_PATH" "select count(*) from t3_backlog_jobs;" 2>/dev/null || echo 0)"
after_canon="$($SQLITE_BIN "$STORE_PATH" "select count(*) from project_canon_revisions;" 2>/dev/null || echo 0)"
non_null_tool_outputs="$($SQLITE_BIN "$STORE_PATH" "select count(*) from raw_events where kind='tool_result' and json_extract(payload_json,'$.output') is not null;" 2>/dev/null || echo unknown)"
t0_tool_snippets="$($SQLITE_BIN "$STORE_PATH" "select count(*) from compact_events_t0 where snippet is not null;" 2>/dev/null || echo unknown)"

echo
echo "-- summary --"
echo "observer_processed: $processed"
echo "observer_failed: $failed"
echo "t1_before: $before_t1"
echo "t1_after: $after_t1"
echo "t2_after: $after_t2"
echo "t3_jobs_after: $after_t3_jobs"
echo "canon_revisions_after: $after_canon"
echo "non_null_tool_outputs: $non_null_tool_outputs"
echo "t0_tool_snippets: $t0_tool_snippets"

echo
echo "-- doctor --"
"$SERVICE_BIN" doctor --project-root "$PROJECT_ROOT" || true

[[ "$failed" == "0" ]]
