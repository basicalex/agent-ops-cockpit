#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
verify_script="$repo_root/scripts/opencode/verify-omo.sh"
policy_template="$repo_root/config/opencode/oh-my-opencode.policy.jsonc"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-verify-omo-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
export XDG_STATE_HOME="$tmp_root/state"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME"

project_root="$tmp_root/project"
mkdir -p "$project_root"
policy_file="$tmp_root/policy.jsonc"
cp "$policy_template" "$policy_file"

bash "$verify_script" task-authority --policy "$policy_file" --project-root "$project_root" >/dev/null
bash "$verify_script" control-flags --policy "$policy_file" --project-root "$project_root" >/dev/null
bash "$verify_script" profile-isolation --profile sandbox >/dev/null
bash "$verify_script" context-pack --project-root "$project_root" --max-chars 2000 >/dev/null
bash "$verify_script" regression --policy "$policy_file" --project-root "$project_root" --profile sandbox --max-chars 2000 >/dev/null
bash "$verify_script" all --policy "$policy_file" --project-root "$project_root" --profile sandbox >/dev/null

mkdir -p "$project_root/.sisyphus/tasks"
if bash "$verify_script" task-authority --policy "$policy_file" --project-root "$project_root" >/dev/null 2>&1; then
  fail "task-authority should fail when .sisyphus/tasks exists"
fi
rm -rf "$project_root/.sisyphus"

invalid_policy_task="$tmp_root/invalid-task-system.jsonc"
python3 - "$policy_file" "$invalid_policy_task" <<'PY'
import json
import sys
from pathlib import Path

src = Path(sys.argv[1])
dst = Path(sys.argv[2])
payload = json.loads(src.read_text(encoding="utf-8"))
payload["experimental"]["task_system"] = True
dst.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

if bash "$verify_script" task-authority --policy "$invalid_policy_task" --project-root "$project_root" >/dev/null 2>&1; then
  fail "task-authority should fail when experimental.task_system=true"
fi

invalid_policy_prompt="$tmp_root/invalid-prompt.jsonc"
python3 - "$policy_file" "$invalid_policy_prompt" <<'PY'
import json
import sys
from pathlib import Path

src = Path(sys.argv[1])
dst = Path(sys.argv[2])
payload = json.loads(src.read_text(encoding="utf-8"))
payload["agents"]["metis"]["prompt_append"] = "Use whichever task system works best."
dst.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

if bash "$verify_script" task-authority --policy "$invalid_policy_prompt" --project-root "$project_root" >/dev/null 2>&1; then
  fail "task-authority should fail when task governance guidance is weakened"
fi

echo "All OmO governance verification tests passed."
