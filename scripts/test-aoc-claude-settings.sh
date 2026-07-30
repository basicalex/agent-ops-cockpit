#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

settings_dir="$tmp/claude"
run_settings_install() {
  AOC_CLAUDE_DIR="$settings_dir" "$root/bin/aoc-claude-install" --settings-only >/dev/null
}

# Missing settings are created with the required global defaults.
run_settings_install
python3 - "$settings_dir/settings.json" <<'PY'
import json
import sys
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text())
assert data["permissions"]["defaultMode"] == "bypassPermissions"
assert data["skipDangerousModePermissionPrompt"] is True
assert data["skipAutoPermissionPrompt"] is True
PY

# Existing unrelated settings survive while conflicting permission defaults change.
cat >"$settings_dir/settings.json" <<'JSON'
{
  "env": {"EXISTING": "kept"},
  "permissions": {
    "allow": ["Bash(git status)"],
    "defaultMode": "auto"
  },
  "skipDangerousModePermissionPrompt": false,
  "custom": [1, 2, 3]
}
JSON
run_settings_install
python3 - "$settings_dir/settings.json" <<'PY'
import json
import sys
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text())
assert data["env"] == {"EXISTING": "kept"}
assert data["custom"] == [1, 2, 3]
assert data["permissions"]["allow"] == ["Bash(git status)"]
assert data["permissions"]["defaultMode"] == "bypassPermissions"
assert data["skipDangerousModePermissionPrompt"] is True
assert data["skipAutoPermissionPrompt"] is True
PY

# A second merge is content-idempotent.
cp "$settings_dir/settings.json" "$tmp/idempotent.before"
run_settings_install
cmp -s "$tmp/idempotent.before" "$settings_dir/settings.json"

# Malformed settings fail without modifying the source file.
printf '{not json\n' >"$settings_dir/settings.json"
cp "$settings_dir/settings.json" "$tmp/malformed.before"
if run_settings_install 2>/dev/null; then
  echo "ERROR: malformed Claude settings unexpectedly succeeded" >&2
  exit 1
fi
cmp -s "$tmp/malformed.before" "$settings_dir/settings.json"

# --settings-only works from an installed bin directory with no source checkout.
installed_bin="$tmp/installed/bin"
installed_settings="$tmp/installed-claude"
mkdir -p "$installed_bin"
cp "$root/bin/aoc-claude-install" "$installed_bin/aoc-claude-install"
AOC_CLAUDE_DIR="$installed_settings" "$installed_bin/aoc-claude-install" --settings-only >/dev/null
python3 - "$installed_settings/settings.json" <<'PY'
import json
import sys
from pathlib import Path

assert json.loads(Path(sys.argv[1]).read_text())["permissions"]["defaultMode"] == "bypassPermissions"
PY

# Read-only init does not touch global settings; normal init reaches the merge first.
project="$tmp/project"
mkdir -p "$project"
printf '{still invalid\n' >"$settings_dir/settings.json"
AOC_CLAUDE_DIR="$settings_dir" "$root/bin/aoc-init" --status "$project" >/dev/null
if AOC_CLAUDE_DIR="$settings_dir" "$root/bin/aoc-init" "$project" >/dev/null 2>&1; then
  echo "ERROR: normal aoc-init skipped malformed global Claude settings" >&2
  exit 1
fi
[[ ! -e "$project/.aoc" ]]

echo "aoc Claude settings tests passed"
