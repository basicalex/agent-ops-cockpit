#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
install_script="$repo_root/scripts/opencode/install-omo.sh"
profile_bin="$repo_root/bin/aoc-opencode-profile"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains_token() {
  local content="$1"
  local token="$2"
  local message="$3"
  case " $content " in
    *" $token "*)
      ;;
    *)
      fail "$message (missing token: $token)"
      ;;
  esac
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-install-omo-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
export XDG_STATE_HOME="$tmp_root/state"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME"

main_path="$($profile_bin resolve main)"
mkdir -p "$main_path"
printf '{"sentinel":true}\n' > "$main_path/opencode.json"

sandbox_path="$($profile_bin init sandbox)"
cat > "$sandbox_path/opencode.json" <<'EOF'
{
  "plugin": ["opencode-antigravity-auth@latest", "custom-plugin"],
  "providers": {
    "google": {
      "mode": "antigravity",
      "token": "keep-this"
    },
    "custom": {
      "endpoint": "https://example.invalid"
    }
  }
}
EOF

args_file="$tmp_root/installer-args.txt"
fake_installer="$tmp_root/fake-omo-installer"
cat > "$fake_installer" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

: "${OPENCODE_CONFIG_DIR:?missing OPENCODE_CONFIG_DIR}"
: "${AOC_TEST_INSTALLER_ARGS_FILE:?missing args file}"

printf '%s\n' "$*" > "$AOC_TEST_INSTALLER_ARGS_FILE"

mkdir -p "$OPENCODE_CONFIG_DIR"
cat > "$OPENCODE_CONFIG_DIR/opencode.json" <<'JSON'
{
  "plugin": ["oh-my-opencode"],
  "providers": {
    "google": {
      "mode": "overwritten",
      "token": "new-token"
    },
    "openai": {
      "enabled": true
    }
  }
}
JSON

cat > "$OPENCODE_CONFIG_DIR/oh-my-opencode.jsonc" <<'JSON'
{
  "sisyphus_agent": {
    "disabled": false
  }
}
JSON
EOF
chmod +x "$fake_installer"

AOC_TEST_INSTALLER_ARGS_FILE="$args_file" \
  bash "$install_script" install \
    --profile sandbox \
    --installer-cmd "$fake_installer" \
    --claude max20 \
    --openai yes \
    --gemini no \
    --copilot yes \
    --opencode-zen no \
    --zai-coding-plan yes >/dev/null

AOC_TEST_INSTALLER_ARGS_FILE="$args_file" \
  bash "$install_script" install \
    --profile sandbox \
    --installer-cmd "$fake_installer" \
    --claude max20 \
    --openai yes \
    --gemini no \
    --copilot yes \
    --opencode-zen no \
    --zai-coding-plan yes >/dev/null

captured_args="$(<"$args_file")"
assert_contains_token "$captured_args" "install" "installer command should include install"
assert_contains_token "$captured_args" "--no-tui" "installer command should include --no-tui"
assert_contains_token "$captured_args" "--claude=max20" "claude flag should be forwarded"
assert_contains_token "$captured_args" "--openai=yes" "openai flag should be forwarded"
assert_contains_token "$captured_args" "--gemini=no" "gemini flag should be forwarded"
assert_contains_token "$captured_args" "--copilot=yes" "copilot flag should be forwarded"
assert_contains_token "$captured_args" "--opencode-zen=no" "opencode-zen flag should be forwarded"
assert_contains_token "$captured_args" "--zai-coding-plan=yes" "zai flag should be forwarded"

python3 - "$sandbox_path/opencode.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
payload = json.loads(path.read_text(encoding="utf-8"))

plugins = payload.get("plugin", [])
assert "oh-my-opencode" in plugins, "oh-my-opencode must be present"
assert "opencode-antigravity-auth@latest" in plugins, "antigravity plugin must be preserved"
assert "custom-plugin" in plugins, "custom plugin must be preserved"
assert len(plugins) == len(set(plugins)), "plugin entries must remain deduplicated"

providers = payload.get("providers", {})
assert providers["google"]["mode"] == "antigravity", "existing provider values must be preserved"
assert providers["google"]["token"] == "keep-this", "existing provider token must be preserved"
assert providers["custom"]["endpoint"] == "https://example.invalid", "custom provider must be preserved"
assert providers["openai"]["enabled"] is True, "new installer provider should be merged"
PY

bash "$install_script" verify --profile sandbox >/dev/null

main_content="$(<"$main_path/opencode.json")"
case "$main_content" in
  *'"sentinel":true'*)
    ;;
  *)
    fail "main profile should remain unchanged"
    ;;
esac

missing_policy_installer="$tmp_root/fake-omo-installer-missing-policy"
cat > "$missing_policy_installer" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${OPENCODE_CONFIG_DIR:?missing OPENCODE_CONFIG_DIR}"
mkdir -p "$OPENCODE_CONFIG_DIR"
cat > "$OPENCODE_CONFIG_DIR/opencode.json" <<'JSON'
{
  "plugin": ["oh-my-opencode"]
}
JSON
EOF
chmod +x "$missing_policy_installer"

if bash "$install_script" install --profile qa-profile --installer-cmd "$missing_policy_installer" >/dev/null 2>&1; then
  fail "install should fail when OmO policy file is missing"
fi

echo "All install-omo tests passed."
