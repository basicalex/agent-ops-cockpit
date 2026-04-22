#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
mind_toggle="$repo_root/bin/aoc-mind-toggle"
mission_doc="$repo_root/docs/mission-control.md"
mission_ops_doc="$repo_root/docs/mission-control-ops.md"
mind_ops_ext="$repo_root/.pi/extensions/mind-ops.ts"
mind_ctx_ext="$repo_root/.pi/extensions/mind-context.ts"
mind_focus_ext="$repo_root/.pi/extensions/mind-focus.ts"
mind_ingest_ext="$repo_root/.pi/extensions/mind-ingest.ts"
mind_lib="$repo_root/.pi/extensions/lib/mind.ts"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq "$needle" "$file" || fail "Expected $file to contain: $needle"
}

make_stub_bin() {
  local dir="$1"
  mkdir -p "$dir"

  cat >"$dir/aoc-mission-control" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${AOC_MIND_TEST_LOG:?}"
printf 'mission-control AOC_PROJECT_ROOT=%s AOC_MIND_PROJECT_SCOPED=%s AOC_MISSION_CONTROL_MODE=%s AOC_MISSION_CONTROL_START_VIEW=%s\n' \
  "${AOC_PROJECT_ROOT:-}" "${AOC_MIND_PROJECT_SCOPED:-}" "${AOC_MISSION_CONTROL_MODE:-}" "${AOC_MISSION_CONTROL_START_VIEW:-}" >>"$AOC_MIND_TEST_LOG"
EOF
  chmod +x "$dir/aoc-mission-control"

  cat >"$dir/aoc-align" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--print-root" ]]; then
  printf '%s\n' "${AOC_MIND_TEST_ALIGN_ROOT:-}"
  exit 0
fi
exit 1
EOF
  chmod +x "$dir/aoc-align"

  cat >"$dir/zellij" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${AOC_MIND_TEST_LOG:?}"
cmd="${1:-}"
sub="${2:-}"
case "$cmd $sub" in
  "action current-tab-info")
    if [[ "${3:-}" == "--help" ]]; then exit 0; fi
    if [[ "${3:-}" == "--json" ]]; then
      printf '%s\n' "${AOC_MIND_TEST_CURRENT_TAB_JSON:-{\"tab_id\":1,\"position\":0,\"hide_floating_panes\":false}}"
      exit 0
    fi
    ;;
  "action list-panes")
    if [[ "${3:-}" == "--help" ]]; then exit 0; fi
    if [[ "${3:-}" == "--json" ]]; then
      printf '%s\n' "${AOC_MIND_TEST_PANES_JSON:-[]}"
      exit 0
    fi
    ;;
  "action list-tabs")
    if [[ "${3:-}" == "--help" ]]; then exit 0; fi
    if [[ "${3:-}" == "--json" ]]; then
      printf '%s\n' "${AOC_MIND_TEST_TABS_JSON:-[]}"
      exit 0
    fi
    ;;
  "action show-floating-panes")
    printf 'show-floating-panes %s\n' "$*" >>"$AOC_MIND_TEST_LOG"
    exit 0
    ;;
  "action hide-floating-panes")
    printf 'hide-floating-panes %s\n' "$*" >>"$AOC_MIND_TEST_LOG"
    exit 0
    ;;
  "action toggle-floating-panes")
    printf 'toggle-floating-panes %s\n' "$*" >>"$AOC_MIND_TEST_LOG"
    exit 0
    ;;
  "action new-pane")
    printf 'new-pane %s\n' "$*" >>"$AOC_MIND_TEST_LOG"
    exit 0
    ;;
  "action toggle-pane-pinned")
    printf 'toggle-pane-pinned %s\n' "$*" >>"$AOC_MIND_TEST_LOG"
    exit 0
    ;;
esac
printf 'unexpected-zellij %s\n' "$*" >>"$AOC_MIND_TEST_LOG"
exit 1
EOF
  chmod +x "$dir/zellij"
}

run_outside_zellij_exec_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  local stub_bin="$tmp/bin"
  make_stub_bin "$stub_bin"
  local root="$tmp/project"
  mkdir -p "$root"
  local log="$tmp/out.log"

  PATH="$stub_bin:$PATH" \
  AOC_MIND_TEST_LOG="$log" \
  AOC_PROJECT_ROOT="$root" \
  AOC_MIND_CMD="aoc-mission-control" \
  ZELLIJ_SESSION_NAME="" \
  bash "$mind_toggle"

  assert_contains "$log" "mission-control AOC_PROJECT_ROOT=$root AOC_MIND_PROJECT_SCOPED=1 AOC_MISSION_CONTROL_MODE=mission-control AOC_MISSION_CONTROL_START_VIEW=mind"
}

run_zellij_project_resolution_and_new_pane_test() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  local stub_bin="$tmp/bin"
  make_stub_bin "$stub_bin"
  local tab_project="$tmp/tab-project"
  mkdir -p "$tab_project"
  local log="$tmp/new-pane.log"
  local panes
  panes="[{\"pane_id\":11,\"tab_id\":3,\"name\":\"Agent [repo-a]\",\"cwd\":\"$tab_project\",\"is_focused\":true}]"
  local current='{"tab_id":3,"position":0,"hide_floating_panes":true}'

  assert_contains "$repo_root/bin/aoc-zellij.sh" 'aoc_zellij_current_tab_agent_project_root() {'
  assert_contains "$repo_root/bin/aoc-zellij.sh" 'current_working_directory'

  PATH="$stub_bin:$PATH" \
  AOC_MIND_TEST_LOG="$log" \
  AOC_PROJECT_ROOT="$tab_project" \
  AOC_MIND_CMD="aoc-mission-control" \
  ZELLIJ_SESSION_NAME="sess-b" \
  AOC_MIND_TEST_PANES_JSON="$panes" \
  AOC_MIND_TEST_CURRENT_TAB_JSON="$current" \
  AOC_HUB_ADDR="127.0.0.1:44444" \
  bash "$mind_toggle"

  assert_contains "$log" "new-pane action new-pane --floating --name Project Mind --width 76% --height 78% --x 12% --y 10% -- bash -lc"
  assert_contains "$log" "export AOC_PROJECT_ROOT=$tab_project"
  assert_contains "$log" "export AOC_MIND_PROJECT_SCOPED=1"
  assert_contains "$log" "export AOC_MISSION_CONTROL_START_VIEW=mind"
}

bash -n "$mind_toggle" "$repo_root/bin/aoc-zellij.sh"
run_outside_zellij_exec_test
run_zellij_project_resolution_and_new_pane_test

assert_contains "$mind_toggle" 'if pane_id="$(aoc_zellij_find_current_tab_pane_id_by_name "$pane_name" 2>/dev/null)" && [[ -n "$pane_id" ]]; then'
assert_contains "$mind_toggle" 'aoc_zellij_show_current_tab_floating >/dev/null 2>&1 || true'
assert_contains "$mind_toggle" 'aoc_zellij_hide_current_tab_floating >/dev/null 2>&1 || true'
assert_contains "$mind_ops_ext" 'pi.registerCommand("mind", {'
assert_contains "$mind_ops_ext" 'pi.registerCommand("mind-status", {'
assert_contains "$mind_ops_ext" 'pi.registerCommand("aoc-status", {'
assert_contains "$mind_ops_ext" 'pi.registerCommand("mind-finalize", {'
assert_contains "$mind_ops_ext" 'pi.registerShortcut("alt+m", {'
assert_contains "$mind_ctx_ext" 'pi.registerCommand("mind-pack", {'
assert_contains "$mind_ctx_ext" 'pi.registerCommand("mind-pack-expanded", {'
assert_contains "$mind_focus_ext" 'pi.registerCommand("mind-focus", {'
assert_contains "$mind_ingest_ext" 'pi.on("message_end", async (event, ctx) => {'
assert_contains "$mind_lib" 'export async function ingestMindMessage(message: any, ctx: ExtensionContext): Promise<{ ok: boolean; error?: string }> {'
assert_contains "$mind_lib" 'export async function sendMindCompactionCheckpoint(event: any, ctx: ExtensionContext): Promise<{ ok: boolean; error?: string }> {'
assert_contains "$mission_doc" 'Floating project Mind bootstrap'
assert_contains "$mission_ops_doc" 'Project-scoped floating Mind UI'

echo "Mind floating UI checks passed."
