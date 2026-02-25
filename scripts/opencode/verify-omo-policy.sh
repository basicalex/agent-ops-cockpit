#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat <<'EOF'
Usage: verify-omo-policy.sh [policy-path]

Validate OmO control policy invariants and basic config compatibility.

Defaults:
  policy-path = config/opencode/oh-my-opencode.policy.jsonc
EOF
}

policy_path="${1:-$repo_root/config/opencode/oh-my-opencode.policy.jsonc}"

if [[ "${policy_path:-}" == "-h" || "${policy_path:-}" == "--help" ]]; then
  usage
  exit 0
fi

[[ -f "$policy_path" ]] || {
  echo "Error: policy file not found: $policy_path" >&2
  exit 1
}

python3 - "$policy_path" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])

try:
    payload = json.loads(path.read_text(encoding="utf-8"))
except json.JSONDecodeError as exc:
    raise SystemExit(f"Invalid JSON/JSONC for policy file: {exc}")

if not isinstance(payload, dict):
    raise SystemExit("Policy root must be an object")

allowed_top_keys = {
    "$schema",
    "agents",
    "categories",
    "background_task",
    "experimental",
    "sisyphus_agent",
    "runtime_fallback",
    "disabled_hooks",
    "disabled_commands",
    "tmux",
    "notification",
    "comment_checker",
    "browser_automation_engine",
    "git_master",
    "skills",
    "lsp",
    "disabled_categories",
    "disabled_agents",
    "disabled_skills",
    "disabled_mcps",
    "disabled_features",
}

unknown = sorted(k for k in payload.keys() if k not in allowed_top_keys)
if unknown:
    raise SystemExit("Unexpected top-level keys: " + ", ".join(unknown))

exp = payload.get("experimental")
if not isinstance(exp, dict):
    raise SystemExit("Missing experimental object")
if exp.get("task_system") is not False:
    raise SystemExit("experimental.task_system must be false")
if exp.get("auto_resume") is not False:
    raise SystemExit("experimental.auto_resume must be false")

runtime_fallback = payload.get("runtime_fallback")
if not isinstance(runtime_fallback, dict):
    raise SystemExit("Missing runtime_fallback object")
if runtime_fallback.get("enabled") is not False:
    raise SystemExit("runtime_fallback.enabled must be false for control-first defaults")
if int(runtime_fallback.get("max_fallback_attempts", 99)) > 2:
    raise SystemExit("runtime_fallback.max_fallback_attempts must be <= 2")

background = payload.get("background_task")
if not isinstance(background, dict):
    raise SystemExit("Missing background_task object")
if int(background.get("defaultConcurrency", 999)) > 4:
    raise SystemExit("background_task.defaultConcurrency must be <= 4")

hooks = payload.get("disabled_hooks")
if not isinstance(hooks, list):
    raise SystemExit("disabled_hooks must be an array")

required_disabled_hooks = {
    "keyword-detector",
    "auto-slash-command",
    "ralph-loop",
    "todo-continuation-enforcer",
}
missing_hooks = sorted(h for h in required_disabled_hooks if h not in hooks)
if missing_hooks:
    raise SystemExit("Required disabled hooks missing: " + ", ".join(missing_hooks))

print("Policy OK:", path)
PY
