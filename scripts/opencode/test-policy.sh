#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
verify_script="$repo_root/scripts/opencode/verify-omo-policy.sh"
policy_file="$repo_root/config/opencode/oh-my-opencode.policy.jsonc"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

bash "$verify_script" "$policy_file" >/dev/null

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/aoc-policy-test.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

invalid_task_policy="$tmp_dir/invalid-task-system.jsonc"
python3 - "$policy_file" "$invalid_task_policy" <<'PY'
import json
import sys
from pathlib import Path

src = Path(sys.argv[1])
dst = Path(sys.argv[2])
payload = json.loads(src.read_text(encoding="utf-8"))
payload["experimental"]["task_system"] = True
dst.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

if bash "$verify_script" "$invalid_task_policy" >/dev/null 2>&1; then
  fail "verify should fail when task_system is true"
fi

invalid_hook_policy="$tmp_dir/invalid-hooks.jsonc"
python3 - "$policy_file" "$invalid_hook_policy" <<'PY'
import json
import sys
from pathlib import Path

src = Path(sys.argv[1])
dst = Path(sys.argv[2])
payload = json.loads(src.read_text(encoding="utf-8"))
payload["disabled_hooks"] = [h for h in payload.get("disabled_hooks", []) if h != "auto-slash-command"]
dst.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

if bash "$verify_script" "$invalid_hook_policy" >/dev/null 2>&1; then
  fail "verify should fail when required disabled hook is missing"
fi

echo "All OmO policy tests passed."
