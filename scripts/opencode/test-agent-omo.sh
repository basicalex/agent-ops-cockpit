#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local message="$3"
  if [[ "$expected" != "$actual" ]]; then
    fail "$message (expected '$expected', got '$actual')"
  fi
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-agent-omo-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_STATE_HOME="$tmp_root/state"
export XDG_CONFIG_HOME="$tmp_root/config"
mkdir -p "$HOME" "$XDG_STATE_HOME" "$XDG_CONFIG_HOME"

project_root="$tmp_root/project"
mkdir -p "$project_root/.aoc/skills/demo"

cat > "$project_root/.aoc/skills/demo/SKILL.md" <<'EOF'
---
name: demo
description: Demo skill for sync checks.
---

# Demo
EOF

bash "$repo_root/bin/aoc-agent" --set omo >/dev/null
current_agent="$(bash "$repo_root/bin/aoc-agent" --current)"
assert_eq "omo" "$current_agent" "default agent should be set to omo"

main_profile="$(bash "$repo_root/bin/aoc-opencode-profile" resolve main)"
sandbox_profile="$(AOC_OMO_PROFILE=sandbox bash "$repo_root/bin/aoc-omo" --print-config-dir)"

expected_sandbox="$XDG_CONFIG_HOME/aoc/opencode/profiles/sandbox"
assert_eq "$expected_sandbox" "$sandbox_profile" "omo should resolve sandbox profile by default"

if [[ "$sandbox_profile" == "$main_profile" ]]; then
  fail "sandbox profile must not equal main profile"
fi

main_profile_via_omo="$(AOC_OMO_PROFILE=main bash "$repo_root/bin/aoc-omo" --print-config-dir)"
assert_eq "$main_profile" "$main_profile_via_omo" "omo main profile override should resolve main path"

AOC_SMOKE_TEST=1 AOC_AGENT_ID=omo bash "$repo_root/bin/aoc-agent-run" >/dev/null
AOC_SMOKE_TEST=1 AOC_AGENT_ID=oc bash "$repo_root/bin/aoc-agent-run" >/dev/null

bash "$repo_root/bin/aoc-skill" sync --agent omo --root "$project_root" --quiet

if [[ ! -L "$project_root/.opencode/skills/demo" ]]; then
  fail "aoc-skill should sync omo alias into OpenCode skill path"
fi

echo "All OmO agent selection tests passed."
