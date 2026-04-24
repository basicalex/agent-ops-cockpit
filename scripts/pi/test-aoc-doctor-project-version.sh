#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
aoc_init_bin="$repo_root/bin/aoc-init"
aoc_doctor_bin="$repo_root/bin/aoc-doctor"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local needle="$1"
  local file="$2"
  grep -Fq "$needle" "$file" || fail "Expected '$needle' in $file"
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-doctor-project-version-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$tmp_root/bin"

cat > "$tmp_root/bin/pi" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat > "$tmp_root/bin/zellij" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" ]]; then
  echo "zellij 0.44.0"
  exit 0
fi
exit 0
EOF
cat > "$tmp_root/bin/yazi" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat > "$tmp_root/bin/fzf" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$tmp_root/bin/pi" "$tmp_root/bin/zellij" "$tmp_root/bin/yazi" "$tmp_root/bin/fzf"
export PATH="$tmp_root/bin:$PATH"

project_ok="$tmp_root/project-ok"
mkdir -p "$project_ok/.git"
AOC_INIT_SKIP_BUILD=1 bash "$aoc_init_bin" "$project_ok" >/dev/null 2>&1
ok_log="$tmp_root/doctor-ok.log"
(
  cd "$project_ok"
  bash "$aoc_doctor_bin" >"$ok_log" 2>&1
)
assert_contains 'Project AOC version: 2' "$ok_log"
assert_contains '[ok] PI runtime package wiring' "$ok_log"

project_broken="$tmp_root/project-broken"
mkdir -p "$project_broken/.git" "$project_broken/.aoc" "$project_broken/.pi"
cat > "$project_broken/.aoc/init-state.json" <<'EOF'
{
  "schemaVersion": 1,
  "projectAocVersion": 2
}
EOF
cat > "$project_broken/.pi/settings.json" <<'EOF'
{
  "packages": [
    "./packages/pi-multi-auth-aoc"
  ]
}
EOF
broken_log="$tmp_root/doctor-broken.log"
set +e
(
  cd "$project_broken"
  bash "$aoc_doctor_bin" >"$broken_log" 2>&1
)
status=$?
set -e
[[ "$status" -eq 1 ]] || fail "Expected aoc-doctor to fail for broken project runtime, got $status"
assert_contains 'Project AOC version: 2' "$broken_log"
assert_contains '.pi/settings.json references ./packages/pi-multi-auth-aoc but the package directory is missing' "$broken_log"
assert_contains "Run 'aoc-init' to repair PI runtime package seeding and .pi/settings.json wiring." "$broken_log"
assert_contains "Inspect project state with 'aoc-init --status'." "$broken_log"

echo "aoc-doctor project version/runtime checks passed."
