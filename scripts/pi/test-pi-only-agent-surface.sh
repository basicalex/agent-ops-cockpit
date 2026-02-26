#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_fails_with() {
  local expected="$1"
  shift
  local log_file
  log_file="$(mktemp "${TMPDIR:-/tmp}/aoc-pi-only-fail.XXXXXX")"
  if "$@" >"$log_file" 2>&1; then
    cat "$log_file" >&2
    rm -f "$log_file"
    fail "Expected command to fail: $*"
  fi
  grep -Fq "$expected" "$log_file" || {
    cat "$log_file" >&2
    rm -f "$log_file"
    fail "Expected failure output to contain: $expected"
  }
  rm -f "$log_file"
}

assert_status_shape() {
  local output="$1"
  case "$output" in
    installed|missing) ;;
    *) fail "Unexpected install status output: $output" ;;
  esac
}

assert_absent_wrapper() {
  local rel_path="$1"
  if [[ -e "$repo_root/$rel_path" ]]; then
    fail "Legacy wrapper still present: $rel_path"
  fi
}

export PATH="$repo_root/bin:$PATH"
state_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-pi-only-state.XXXXXX")"
trap 'rm -rf "$state_root"' EXIT
export XDG_STATE_HOME="$state_root/state"
mkdir -p "$XDG_STATE_HOME"

# Legacy non-PI wrappers are removed from the shipped bin surface.
for wrapper in \
  bin/aoc-codex \
  bin/aoc-gemini \
  bin/aoc-cc \
  bin/aoc-oc \
  bin/aoc-kimi \
  bin/aoc-omo \
  bin/aoc-codex-tab \
  bin/codex \
  bin/gemini \
  bin/claude \
  bin/opencode \
  bin/kimi \
  bin/aoc-opencode-profile
  do
    assert_absent_wrapper "$wrapper"
  done

assert_fails_with "Unknown agent" aoc-agent --set codex
assert_fails_with "Unknown agent" aoc-agent --set rust-agent
assert_fails_with "Unsupported agent: codex" aoc-agent-install status codex
assert_fails_with "Unsupported agent: rust-agent" aoc-agent-install status rust-agent

# PI selector remains valid.
aoc-agent --set pi >/dev/null
[[ "$(aoc-agent --current)" == "pi" ]] || fail "Expected current default agent to be pi"

pi_status="$(aoc-agent-install status pi)"
assert_status_shape "$pi_status"

echo "PI-only agent surface checks passed."
