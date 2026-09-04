#!/usr/bin/env bash
# deck-gate.sh <workspace>   — run ppt-master's final quality gate; exit non-zero on blocking errors.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
WS=$(cd "${1:?workspace dir}" && pwd)
PROJ=$(find_project "$WS")
LOG=$(mktemp)
set +e
(cd "$PPT_ROOT" && "$PY" "$PPT_SKILL/scripts/svg_quality_checker.py" "$PROJ" --quick-generate --canonical-authoring --stage final --json) >"$LOG" 2>&1
RC=$?
set -e
# stdout is a text summary even with --json; the JSON report lands in validation/svg_quality_report.json
grep -E '^\s*\[(ERROR|SUMMARY)\]|Fully passed|With warnings|With errors|blocking:' "$LOG" || true
ERRS=$(grep -oE 'With errors: [0-9]+' "$LOG" | grep -oE '[0-9]+' | tail -1)
if [ "${ERRS:-1}" != 0 ] || [ $RC -ne 0 ]; then
  echo; echo "--- error detail ---"
  grep -A4 -E '^\s*\[ERROR\] [^ ]+\.svg' "$LOG" | head -80
  echo "gate: FAILED (rc=$RC); full log: $LOG"
  exit 1
fi
echo "gate: passed; full log: $LOG"
