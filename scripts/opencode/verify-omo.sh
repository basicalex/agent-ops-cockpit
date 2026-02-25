#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat <<'EOF'
Usage: verify-omo.sh <command> [options]

Commands:
  task-authority      Verify Taskmaster-only OmO governance
  control-flags       Verify control-first OmO policy defaults
  profile-isolation   Verify sandbox/main profile paths remain isolated
  context-pack        Verify context-pack precedence and bounds
  regression          Run combined OmO regression checks
  all                 Run all checks
  help                Show this help

Options:
  --policy <path>       Policy file path (default: config/opencode/oh-my-opencode.policy.jsonc)
  --project-root <path> Project root for task authority checks (default: cwd)
  --profile <name>      Profile for isolation check (default: sandbox)
  --max-chars <n>       Max chars for context-pack verify (default: 12000)
  --run-lint            Run scripts/lint.sh as part of regression check
  --rust-check          Run cargo checks (opt-in) as part of regression check
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

warn() {
  echo "Warning: $*" >&2
}

default_policy_path() {
  printf '%s' "$repo_root/config/opencode/oh-my-opencode.policy.jsonc"
}

profile_tool_path() {
  if command -v aoc-opencode-profile >/dev/null 2>&1; then
    command -v aoc-opencode-profile
    return
  fi
  if [[ -x "$repo_root/bin/aoc-opencode-profile" ]]; then
    printf '%s' "$repo_root/bin/aoc-opencode-profile"
    return
  fi
  die "aoc-opencode-profile not found in PATH or repo bin/"
}

python_parse_and_check() {
  local mode="$1"
  local policy_path="$2"
  local project_root="$3"

  python3 - "$mode" "$policy_path" "$project_root" <<'PY'
import json
import sys
from pathlib import Path


def strip_json_comments(text: str) -> str:
    out = []
    i = 0
    in_string = False
    in_line_comment = False
    in_block_comment = False
    escaped = False

    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""

        if in_line_comment:
            if ch == "\n":
                in_line_comment = False
                out.append(ch)
            i += 1
            continue

        if in_block_comment:
            if ch == "*" and nxt == "/":
                in_block_comment = False
                i += 2
            else:
                i += 1
            continue

        if in_string:
            out.append(ch)
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
            i += 1
            continue

        if ch == '"':
            in_string = True
            out.append(ch)
            i += 1
            continue

        if ch == "/" and nxt == "/":
            in_line_comment = True
            i += 2
            continue

        if ch == "/" and nxt == "*":
            in_block_comment = True
            i += 2
            continue

        out.append(ch)
        i += 1

    return "".join(out)


def load_json_or_jsonc(path: Path) -> dict:
    if not path.exists():
        raise SystemExit(f"policy file not found: {path}")
    raw = path.read_text(encoding="utf-8")
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        payload = json.loads(strip_json_comments(raw))
    if not isinstance(payload, dict):
        raise SystemExit("policy root must be an object")
    return payload


def check_task_authority(policy: dict, project_root: Path) -> None:
    errors = []

    experimental = policy.get("experimental")
    if not isinstance(experimental, dict) or experimental.get("task_system") is not False:
        errors.append("experimental.task_system must be false")

    agents = policy.get("agents")
    if not isinstance(agents, dict):
        errors.append("agents block is required")
    else:
        required_agents = ("sisyphus", "prometheus", "metis")
        for agent_name in required_agents:
            agent_cfg = agents.get(agent_name)
            if not isinstance(agent_cfg, dict):
                errors.append(f"agents.{agent_name} block is required")
                continue
            prompt = str(agent_cfg.get("prompt_append", "")).lower()
            if "taskmaster" not in prompt:
                errors.append(f"agents.{agent_name}.prompt_append must reference Taskmaster")
            if "tm" not in prompt and "aoc-task" not in prompt:
                errors.append(f"agents.{agent_name}.prompt_append must reference tm or aoc-task")
            if ".sisyphus/tasks" not in prompt:
                errors.append(f"agents.{agent_name}.prompt_append must forbid .sisyphus/tasks")

    conflict_path = project_root / ".sisyphus" / "tasks"
    if conflict_path.exists():
        errors.append(f"conflicting task store detected: {conflict_path}")

    if errors:
        raise SystemExit("task-authority check failed: " + "; ".join(errors))


def check_control_flags(policy: dict) -> None:
    errors = []

    runtime = policy.get("runtime_fallback")
    if not isinstance(runtime, dict) or runtime.get("enabled") is not False:
        errors.append("runtime_fallback.enabled must be false")

    background = policy.get("background_task")
    if not isinstance(background, dict):
        errors.append("background_task block is required")
    else:
        default_concurrency = int(background.get("defaultConcurrency", 999))
        if default_concurrency > 4:
            errors.append("background_task.defaultConcurrency must be <= 4")

    disabled_hooks = policy.get("disabled_hooks")
    if not isinstance(disabled_hooks, list):
        errors.append("disabled_hooks must be an array")
    else:
        required_hooks = {
            "keyword-detector",
            "auto-slash-command",
            "ralph-loop",
            "todo-continuation-enforcer",
        }
        missing = sorted(h for h in required_hooks if h not in disabled_hooks)
        if missing:
            errors.append("missing required disabled hooks: " + ", ".join(missing))

    if errors:
        raise SystemExit("control-flags check failed: " + "; ".join(errors))


mode = sys.argv[1]
policy_path = Path(sys.argv[2])
project_root = Path(sys.argv[3])

policy = load_json_or_jsonc(policy_path)

if mode == "task-authority":
    check_task_authority(policy, project_root)
elif mode == "control-flags":
    check_control_flags(policy)
else:
    raise SystemExit(f"unknown mode: {mode}")

print(f"{mode} OK")
PY
}

check_profile_isolation() {
  local profile_name="$1"
  local profile_tool main_path target_path

  profile_tool="$(profile_tool_path)"
  main_path="$($profile_tool resolve main)"
  target_path="$($profile_tool resolve "$profile_name")"

  [[ "$main_path" != "$target_path" ]] || die "profile '$profile_name' resolves to main profile path"
  printf '%s\n' "profile-isolation OK"
}

check_context_pack() {
  local project_root="$1"
  local max_chars="$2"
  local context_pack_script="$repo_root/scripts/opencode/context-pack.sh"

  [[ -f "$context_pack_script" ]] || die "context-pack script not found: $context_pack_script"
  [[ "$max_chars" =~ ^[0-9]+$ ]] || die "--max-chars must be numeric"

  local output
  output="$(bash "$context_pack_script" \
    --project-root "$project_root" \
    --max-chars "$max_chars" \
    --mem-max-lines 24 \
    --stm-max-lines 24 \
    --mind-max-lines 24 \
    --mem-max-chars 1100 \
    --stm-max-chars 1100 \
    --mind-max-chars 1100)"

  python3 - "$output" "$max_chars" <<'PY'
import sys

text = sys.argv[1]
max_chars = int(sys.argv[2])

if len(text) > max_chars:
    raise SystemExit(f"context-pack exceeds max chars: {len(text)} > {max_chars}")

mem = text.find("## aoc-mem")
stm = text.find("## aoc-stm")
mind = text.find("## aoc-mind")

if mem < 0 or stm < 0 or mind < 0:
    raise SystemExit("context-pack missing required section")
if not (mem < stm < mind):
    raise SystemExit("context-pack section order is invalid")
PY

  printf '%s\n' "context-pack OK"
}

check_shell_syntax() {
  local -a files=()
  local file=""

  files+=("$repo_root/bin/aoc-opencode-profile")
  files+=("$repo_root/bin/aoc-omo")
  files+=("$repo_root/bin/aoc-init")
  files+=("$repo_root/install.sh")
  files+=("$repo_root/scripts/smoke.sh")

  shopt -s nullglob
  for file in "$repo_root/scripts/opencode/"*.sh; do
    files+=("$file")
  done
  shopt -u nullglob

  for file in "${files[@]}"; do
    [[ -f "$file" ]] || continue
    bash -n "$file"
  done

  printf '%s\n' "shell-syntax OK"
}

run_rust_checks() {
  local manifest_path="$repo_root/crates/Cargo.toml"

  if ! command -v cargo >/dev/null 2>&1; then
    warn "cargo not found; skipping rust-check"
    return
  fi

  if [[ ! -f "$manifest_path" ]]; then
    warn "Rust workspace not found at $manifest_path; skipping rust-check"
    return
  fi

  cargo check --manifest-path "$manifest_path" -p aoc-cli -p aoc-taskmaster >/dev/null
  printf '%s\n' "rust-check OK"
}

run_regression() {
  local policy_path="$1"
  local project_root="$2"
  local profile_name="$3"
  local max_chars="$4"
  local run_lint="$5"
  local rust_check="$6"

  python_parse_and_check "task-authority" "$policy_path" "$project_root"
  python_parse_and_check "control-flags" "$policy_path" "$project_root"
  check_profile_isolation "$profile_name"
  check_context_pack "$project_root" "$max_chars"
  check_shell_syntax

  if [[ "$run_lint" == "1" ]]; then
    bash "$repo_root/scripts/lint.sh" >/dev/null
    printf '%s\n' "shell-lint OK"
  fi

  if [[ "$rust_check" == "1" ]]; then
    run_rust_checks
  fi

  printf '%s\n' "regression OK"
}

main() {
  local cmd="${1:-all}"
  shift || true

  local policy_path
  local project_root="$PWD"
  local profile_name="sandbox"
  local max_chars="12000"
  local run_lint="0"
  local rust_check="0"
  policy_path="$(default_policy_path)"

  while (($# > 0)); do
    case "$1" in
      --policy)
        [[ $# -ge 2 ]] || die "--policy requires a value"
        policy_path="$2"
        shift 2
        ;;
      --project-root)
        [[ $# -ge 2 ]] || die "--project-root requires a value"
        project_root="$2"
        shift 2
        ;;
      --profile)
        [[ $# -ge 2 ]] || die "--profile requires a value"
        profile_name="$2"
        shift 2
        ;;
      --max-chars)
        [[ $# -ge 2 ]] || die "--max-chars requires a value"
        max_chars="$2"
        shift 2
        ;;
      --run-lint)
        run_lint="1"
        shift
        ;;
      --rust-check)
        rust_check="1"
        shift
        ;;
      -h|--help)
        usage
        return 0
        ;;
      *)
        die "unknown option: $1"
        ;;
    esac
  done

  case "$cmd" in
    task-authority)
      python_parse_and_check "task-authority" "$policy_path" "$project_root"
      ;;
    control-flags)
      python_parse_and_check "control-flags" "$policy_path" "$project_root"
      ;;
    profile-isolation)
      check_profile_isolation "$profile_name"
      ;;
    context-pack)
      check_context_pack "$project_root" "$max_chars"
      ;;
    regression)
      run_regression "$policy_path" "$project_root" "$profile_name" "$max_chars" "$run_lint" "$rust_check"
      ;;
    all)
      python_parse_and_check "task-authority" "$policy_path" "$project_root"
      python_parse_and_check "control-flags" "$policy_path" "$project_root"
      check_profile_isolation "$profile_name"
      check_context_pack "$project_root" "$max_chars"
      ;;
    help)
      usage
      ;;
    *)
      die "unknown command: $cmd"
      ;;
  esac
}

main "$@"
