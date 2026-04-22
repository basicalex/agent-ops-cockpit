#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
wrap_script="$repo_root/bin/aoc-agent-wrap"
rtk_proxy_script="$repo_root/bin/aoc-rtk-proxy"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq "$needle" "$file" || fail "Expected $file to contain: $needle"
}

assert_not_contains() {
  local file="$1"
  local needle="$2"
  if grep -Fq "$needle" "$file"; then
    fail "Did not expect $file to contain: $needle"
  fi
}

make_stub_bins() {
  local dir="$1"
  mkdir -p "$dir"

  cat >"$dir/aoc-agent-wrap-rs" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${AOC_PI_WRAP_RS_TEST_LOG:?}"
printf 'wrap-rs %s\n' "$*" >>"$AOC_PI_WRAP_RS_TEST_LOG"
printf 'env AOC_AGENT_RUN=%s AOC_AGENT_PTY=%s AOC_PROJECT_ROOT=%s AOC_AGENT_LAUNCH_MODE=%s AOC_AGENT_WRAP_MODE=%s AOC_AGENT_TMUX_ACTIVE=%s AOC_AGENT_BOOTLOADER_ACTIVE=%s\n' "${AOC_AGENT_RUN:-}" "${AOC_AGENT_PTY:-}" "${AOC_PROJECT_ROOT:-}" "${AOC_AGENT_LAUNCH_MODE:-}" "${AOC_AGENT_WRAP_MODE:-}" "${AOC_AGENT_TMUX_ACTIVE:-}" "${AOC_AGENT_BOOTLOADER_ACTIVE:-}" >>"$AOC_PI_WRAP_RS_TEST_LOG"
exit 0
EOF
  chmod +x "$dir/aoc-agent-wrap-rs"

  cat >"$dir/tmux" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${AOC_PI_WRAP_RS_TEST_LOG:?}"
printf 'tmux %s\n' "$*" >>"$AOC_PI_WRAP_RS_TEST_LOG"
exit 97
EOF
  chmod +x "$dir/tmux"

  cat >"$dir/pi" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${AOC_PI_WRAP_RS_TEST_LOG:?}"
printf 'pi %s\n' "$*" >>"$AOC_PI_WRAP_RS_TEST_LOG"
exit 0
EOF
  chmod +x "$dir/pi"
}

run_default_prefers_wrap_rs_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  local stub_bin="$tmp/bin"
  local log="$tmp/default.log"
  make_stub_bins "$stub_bin"

  env -u AOC_PI_USE_WRAP_RS -u AOC_PI_USE_PTY -u AOC_AGENT_PTY \
  PATH="$stub_bin:$PATH" \
  ZELLIJ_SESSION_NAME="" \
  AOC_AGENT_RUN=1 \
  AOC_AGENT_WRAP_RS_BIN="$stub_bin/aoc-agent-wrap-rs" \
  AOC_RTK_BYPASS=1 \
  AOC_HANDSHAKE_MODE=off \
  AOC_PI_WRAP_RS_TEST_LOG="$log" \
  AOC_PROJECT_ROOT="$repo_root" \
  AOC_SESSION_ID="pi-wrap-default" \
  AOC_PANE_ID="11" \
  bash "$wrap_script" pi pi "PI Agent" --version

  assert_contains "$log" 'wrap-rs --session pi-wrap-default --pane-id 11 --agent-id pi'
  assert_contains "$log" 'env AOC_AGENT_RUN=1 AOC_AGENT_PTY=1 AOC_PROJECT_ROOT='
  assert_contains "$log" 'AOC_AGENT_LAUNCH_MODE=managed AOC_AGENT_WRAP_MODE=wrap-rs AOC_AGENT_TMUX_ACTIVE=0 AOC_AGENT_BOOTLOADER_ACTIVE=0'
  assert_not_contains "$log" '/tmp/aoc-bootloader-'
  assert_not_contains "$log" 'tmux '
}

run_explicit_direct_exec_opt_out_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  local stub_bin="$tmp/bin"
  local log="$tmp/direct.log"
  make_stub_bins "$stub_bin"

  mkdir -p "$tmp/project"
  (
    cd "$tmp"
    env -u AOC_PI_USE_PTY -u AOC_AGENT_PTY \
    PATH="$stub_bin:$PATH" \
    ZELLIJ_SESSION_NAME="" \
    AOC_AGENT_WRAP_RS_BIN="$stub_bin/aoc-agent-wrap-rs" \
    AOC_NO_TMUX=1 \
    AOC_RTK_BYPASS=1 \
    AOC_HANDSHAKE_MODE=off \
    AOC_PI_USE_WRAP_RS=0 \
    AOC_PI_WRAP_RS_TEST_LOG="$log" \
    AOC_PROJECT_ROOT="$tmp/project" \
    AOC_SESSION_ID="pi-wrap-direct" \
    AOC_PANE_ID="12" \
    bash "$wrap_script" pi pi "PI Agent" --version
  )

  assert_contains "$log" 'pi --version'
}

run_rtk_shim_exports_dir_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  local stub_bin="$tmp/bin"
  local log="$tmp/rtk.log"
  make_stub_bins "$stub_bin"

  env -u AOC_PI_USE_WRAP_RS -u AOC_PI_USE_PTY -u AOC_AGENT_PTY \
  PATH="$stub_bin:$PATH" \
  XDG_STATE_HOME="$tmp/state" \
  ZELLIJ_SESSION_NAME="" \
  AOC_AGENT_RUN=1 \
  AOC_RTK_MODE=on \
  AOC_RTK_ALLOWLIST='rg' \
  AOC_RTK_FAIL_OPEN=1 \
  AOC_HANDSHAKE_MODE=off \
  AOC_PI_USE_WRAP_RS=0 \
  AOC_PI_WRAP_RS_TEST_LOG="$log" \
  AOC_PROJECT_ROOT="$repo_root" \
  AOC_SESSION_ID="pi-wrap-rtk" \
  AOC_PANE_ID="14" \
  bash "$wrap_script" pi pi "PI Agent" --version

  local shim="$tmp/state/aoc/rtk-shims/pi-wrap-rtk/14/rg"
  [[ -f "$shim" ]] || fail "Expected RTK shim to be created at $shim"
  assert_contains "$shim" 'export AOC_RTK_SHIM_DIR='
}

run_missing_wrap_rs_falls_back_direct_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  local stub_bin="$tmp/bin"
  local log="$tmp/fallback.log"
  local copied_bin_dir="$tmp/copied-bin"
  mkdir -p "$stub_bin" "$copied_bin_dir"

  cp "$repo_root/bin/aoc-agent-wrap" "$copied_bin_dir/aoc-agent-wrap"
  cp "$repo_root/bin/aoc-utils.sh" "$copied_bin_dir/aoc-utils.sh"

  cat >"$stub_bin/pi" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${AOC_PI_WRAP_RS_TEST_LOG:?}"
printf 'pi %s\n' "$*" >>"$AOC_PI_WRAP_RS_TEST_LOG"
exit 0
EOF
  chmod +x "$stub_bin/pi" "$copied_bin_dir/aoc-agent-wrap"

  mkdir -p "$tmp/no-wrap-project"
  (
    cd "$tmp"
    env -u AOC_PI_USE_WRAP_RS -u AOC_PI_USE_PTY -u AOC_AGENT_PTY \
    PATH="$stub_bin:/usr/bin:/bin" \
    ZELLIJ_SESSION_NAME="" \
    AOC_NO_TMUX=1 \
    AOC_RTK_BYPASS=1 \
    AOC_HANDSHAKE_MODE=off \
    AOC_PI_WRAP_RS_TEST_LOG="$log" \
    AOC_PROJECT_ROOT="$tmp/no-wrap-project" \
    AOC_SESSION_ID="pi-wrap-fallback" \
    AOC_PANE_ID="13" \
    bash "$copied_bin_dir/aoc-agent-wrap" pi pi "PI Agent" --version
  )

  assert_contains "$log" 'pi --version'
}

bash -n "$wrap_script" "$rtk_proxy_script"
assert_contains "$rtk_proxy_script" '/aoc/rtk-shims/'
run_default_prefers_wrap_rs_test
run_explicit_direct_exec_opt_out_test
run_rtk_shim_exports_dir_test
run_missing_wrap_rs_falls_back_direct_test

echo "Pi wrap-rs default checks passed."
