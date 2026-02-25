#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat <<'EOF'
Usage: install-omo.sh <command> [options]

Commands:
  install    Install Oh-My-OpenCode into a sandbox profile (default)
  verify     Verify sandbox OmO registration and profile health
  help       Show this help

Options:
  --profile <name>              Profile name (default: sandbox)
  --claude <yes|no|max20>       Claude subscription mode (default: no)
  --openai <yes|no>             OpenAI subscription mode (default: no)
  --gemini <yes|no>             Gemini subscription mode (default: no)
  --copilot <yes|no>            GitHub Copilot subscription mode (default: no)
  --opencode-zen <yes|no>       OpenCode Zen mode (default: no)
  --zai-coding-plan <yes|no>    Z.ai coding plan mode (default: no)
  --installer-cmd <cmd>         Override installer command (default auto-detect)
  --no-verify                   Skip post-install verification
  -h, --help                    Show help

Environment:
  AOC_OMO_INSTALLER_CMD         Installer command override
  AOC_OMO_*                     Provider flag defaults (AOC_OMO_CLAUDE, etc.)

Behavior:
  - Forces OPENCODE_CONFIG_DIR to selected profile path
  - Preserves existing plugin/provider entries via merge-safe reconciliation
  - Verifies `oh-my-opencode` registration after install
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

to_lower() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]'
}

validate_yes_no() {
  local key="$1"
  local value="$(to_lower "$2")"
  case "$value" in
    yes|no)
      printf '%s' "$value"
      ;;
    *)
      die "invalid value for $key: '$2' (expected yes|no)"
      ;;
  esac
}

validate_claude() {
  local value="$(to_lower "$1")"
  case "$value" in
    yes|no|max20)
      printf '%s' "$value"
      ;;
    *)
      die "invalid value for --claude: '$1' (expected yes|no|max20)"
      ;;
  esac
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

merge_configs() {
  local before_json="$1"
  local after_json="$2"
  local out_json="$3"

  python3 - "$before_json" "$after_json" "$out_json" <<'PY'
import copy
import json
import sys
from pathlib import Path


def load_json(path: Path) -> dict:
    if not path.exists():
        return {}
    text = path.read_text(encoding="utf-8").strip()
    if not text:
        return {}
    try:
        payload = json.loads(text)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"Invalid JSON in {path}: {exc}")
    if isinstance(payload, dict):
        return payload
    return {}


def list_union(existing, incoming):
    result = []
    for source in (existing, incoming):
        if not isinstance(source, list):
            continue
        for item in source:
            if item not in result:
                result.append(item)
    return result


def preserve_existing(existing, incoming):
    if isinstance(existing, dict) and isinstance(incoming, dict):
        merged = copy.deepcopy(existing)
        for key, value in incoming.items():
            if key not in merged:
                merged[key] = value
            else:
                merged[key] = preserve_existing(merged[key], value)
        return merged
    if isinstance(existing, list) and isinstance(incoming, list):
        return list_union(existing, incoming)
    return copy.deepcopy(existing)


before_path = Path(sys.argv[1])
after_path = Path(sys.argv[2])
out_path = Path(sys.argv[3])

before = load_json(before_path)
after = load_json(after_path)

merged = copy.deepcopy(after)

before_plugins = before.get("plugin")
after_plugins = after.get("plugin")
merged_plugins = list_union(before_plugins, after_plugins)
if not any(isinstance(item, str) and item.startswith("oh-my-opencode") for item in merged_plugins):
    merged_plugins.append("oh-my-opencode")
merged["plugin"] = merged_plugins

for provider_key in ("provider", "providers"):
    if provider_key in before and provider_key in after:
        merged[provider_key] = preserve_existing(before[provider_key], after[provider_key])
    elif provider_key in before and provider_key not in after:
        merged[provider_key] = copy.deepcopy(before[provider_key])

out_path.parent.mkdir(parents=True, exist_ok=True)
out_path.write_text(json.dumps(merged, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
}

verify_profile() {
  local profile_name="$1"
  local profile_tool profile_path opencode_json

  profile_tool="$(profile_tool_path)"
  profile_path="$($profile_tool resolve "$profile_name")"
  opencode_json="$profile_path/opencode.json"

  [[ -d "$profile_path" ]] || die "profile path not found: $profile_path"
  [[ -f "$opencode_json" ]] || die "missing config: $opencode_json"

  python3 - "$opencode_json" <<'PY'
import json
import sys
from pathlib import Path

config_path = Path(sys.argv[1])
try:
    payload = json.loads(config_path.read_text(encoding="utf-8"))
except Exception as exc:
    raise SystemExit(f"Invalid opencode.json: {exc}")

plugins = payload.get("plugin")
if not isinstance(plugins, list):
    raise SystemExit("opencode.json missing plugin array")

if not any(isinstance(item, str) and item.startswith("oh-my-opencode") for item in plugins):
    raise SystemExit("oh-my-opencode plugin is not registered")
PY

  if [[ ! -f "$profile_path/oh-my-opencode.jsonc" && ! -f "$profile_path/oh-my-opencode.json" ]]; then
    die "missing OmO policy file in profile ($profile_path/oh-my-opencode.jsonc or .json)"
  fi

  printf '%s\n' "$profile_path"
}

install_omo() {
  local profile_name="$1"
  local claude="$2"
  local openai="$3"
  local gemini="$4"
  local copilot="$5"
  local opencode_zen="$6"
  local zai_coding_plan="$7"
  local installer_cmd="$8"
  local skip_verify="$9"

  local profile_tool profile_path opencode_json
  local before_file after_file merged_file
  local -a installer_runner

  profile_tool="$(profile_tool_path)"
  profile_path="$($profile_tool init "$profile_name")"
  opencode_json="$profile_path/opencode.json"

  before_file="$(mktemp "${TMPDIR:-/tmp}/aoc-omo-before.XXXXXX")"
  after_file="$(mktemp "${TMPDIR:-/tmp}/aoc-omo-after.XXXXXX")"
  merged_file="$(mktemp "${TMPDIR:-/tmp}/aoc-omo-merged.XXXXXX")"
  trap "rm -f '$before_file' '$after_file' '$merged_file'" RETURN

  if [[ -f "$opencode_json" ]]; then
    cp "$opencode_json" "$before_file"
  else
    printf '{}\n' > "$before_file"
  fi

  if [[ -n "$installer_cmd" ]]; then
    installer_runner=("$installer_cmd")
  elif command -v oh-my-opencode >/dev/null 2>&1; then
    installer_runner=("oh-my-opencode")
  elif command -v bunx >/dev/null 2>&1; then
    installer_runner=("bunx" "oh-my-opencode")
  elif command -v npx >/dev/null 2>&1; then
    installer_runner=("npx" "oh-my-opencode")
  else
    die "no OmO installer found (expected oh-my-opencode, bunx, or npx)"
  fi

  OPENCODE_CONFIG_DIR="$profile_path" "${installer_runner[@]}" install --no-tui \
    --claude="$claude" \
    --openai="$openai" \
    --gemini="$gemini" \
    --copilot="$copilot" \
    --opencode-zen="$opencode_zen" \
    --zai-coding-plan="$zai_coding_plan"

  if [[ -f "$opencode_json" ]]; then
    cp "$opencode_json" "$after_file"
  else
    printf '{}\n' > "$after_file"
  fi

  merge_configs "$before_file" "$after_file" "$merged_file"
  mv "$merged_file" "$opencode_json"

  if [[ "$skip_verify" != "1" ]]; then
    verify_profile "$profile_name" >/dev/null
  fi

  printf '%s\n' "$profile_path"
}

main() {
  local cmd="${1:-install}"
  shift || true

  local profile_name="sandbox"
  local claude="${AOC_OMO_CLAUDE:-no}"
  local openai="${AOC_OMO_OPENAI:-no}"
  local gemini="${AOC_OMO_GEMINI:-no}"
  local copilot="${AOC_OMO_COPILOT:-no}"
  local opencode_zen="${AOC_OMO_OPENCODE_ZEN:-no}"
  local zai_coding_plan="${AOC_OMO_ZAI_CODING_PLAN:-no}"
  local installer_cmd="${AOC_OMO_INSTALLER_CMD:-}"
  local skip_verify="0"

  while (($# > 0)); do
    case "$1" in
      --profile)
        [[ $# -ge 2 ]] || die "--profile requires a value"
        profile_name="$2"
        shift 2
        ;;
      --claude)
        [[ $# -ge 2 ]] || die "--claude requires a value"
        claude="$2"
        shift 2
        ;;
      --openai)
        [[ $# -ge 2 ]] || die "--openai requires a value"
        openai="$2"
        shift 2
        ;;
      --gemini)
        [[ $# -ge 2 ]] || die "--gemini requires a value"
        gemini="$2"
        shift 2
        ;;
      --copilot)
        [[ $# -ge 2 ]] || die "--copilot requires a value"
        copilot="$2"
        shift 2
        ;;
      --opencode-zen)
        [[ $# -ge 2 ]] || die "--opencode-zen requires a value"
        opencode_zen="$2"
        shift 2
        ;;
      --zai-coding-plan)
        [[ $# -ge 2 ]] || die "--zai-coding-plan requires a value"
        zai_coding_plan="$2"
        shift 2
        ;;
      --installer-cmd)
        [[ $# -ge 2 ]] || die "--installer-cmd requires a value"
        installer_cmd="$2"
        shift 2
        ;;
      --no-verify)
        skip_verify="1"
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

  claude="$(validate_claude "$claude")"
  openai="$(validate_yes_no --openai "$openai")"
  gemini="$(validate_yes_no --gemini "$gemini")"
  copilot="$(validate_yes_no --copilot "$copilot")"
  opencode_zen="$(validate_yes_no --opencode-zen "$opencode_zen")"
  zai_coding_plan="$(validate_yes_no --zai-coding-plan "$zai_coding_plan")"

  case "$cmd" in
    install)
      install_omo "$profile_name" "$claude" "$openai" "$gemini" "$copilot" "$opencode_zen" "$zai_coding_plan" "$installer_cmd" "$skip_verify"
      ;;
    verify)
      verify_profile "$profile_name"
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
