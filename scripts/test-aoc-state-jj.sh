#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
aoc_state_bin="$repo_root/bin/aoc-state"
aoc_bin="$repo_root/bin/aoc"
state_extension="$repo_root/.omp/extensions/aoc-state.ts"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local needle="$1"
  local file="$2"
  grep -Fq -- "$needle" "$file" || fail "Expected '$needle' in $file"
}

assert_not_contains() {
  local needle="$1"
  local file="$2"
  if grep -Fq -- "$needle" "$file"; then
    fail "Did not expect '$needle' in $file"
  fi
}

assert_executable() {
  local path="$1"
  [[ -x "$path" ]] || fail "Expected executable: $path"
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-state-jj-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

fake_bin="$tmp_root/bin"
project_root="$tmp_root/project"
log_file="$tmp_root/commands.log"
mkdir -p "$fake_bin" "$project_root/.git" "$project_root/.jj" \
  "$project_root/.aoc/logs" "$project_root/.aoc/mind" \
  "$project_root/.taskmaster/docs/specs" "$project_root/.pi/tmp" \
  "$project_root/.omp/extensions" "$project_root/.omp/agents"

cat > "$fake_bin/aoc-handshake" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" != "--json" ]]; then
  echo "unsupported fake aoc-handshake: $*" >&2
  exit 2
fi
cat <<'JSON'
{"vcs":{"kind":"jj","preferredTool":"jj"}}
JSON
FAKE
chmod +x "$fake_bin/aoc-handshake"

cat > "$fake_bin/jj" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
printf 'jj %s\n' "$*" >> "${AOC_STATE_TEST_LOG:?}"
case "${1:-}" in
  status)
    echo 'Working copy changes:'
    echo 'A .omp/extensions/aoc-state.ts'
    ;;
  diff)
    if [[ "${2:-}" == "--summary" ]]; then
      echo 'A .omp/extensions/aoc-state.ts'
    elif [[ "${2:-}" == "--stat" ]]; then
      echo '.omp/extensions/aoc-state.ts | 10 ++++++++++'
    fi
    ;;
  file)
    if [[ "${2:-}" == "list" ]]; then
      echo '.aoc/context.md'
      echo '.omp/extensions/aoc-state.ts'
    fi
    ;;
  *)
    ;;
esac
FAKE
chmod +x "$fake_bin/jj"

cat > "$fake_bin/git" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
printf 'git %s\n' "$*" >> "${AOC_STATE_TEST_LOG:?}"
case "${1:-}" in
  check-ignore)
    exit 1
    ;;
  status|diff)
    exit 0
    ;;
esac
exit 0
FAKE
chmod +x "$fake_bin/git"

export PATH="$fake_bin:$repo_root/bin:$PATH"
export AOC_STATE_TEST_LOG="$log_file"

printf 'context\n' > "$project_root/.aoc/context.md"
printf 'runtime log\n' > "$project_root/.aoc/logs/runtime.log"
printf 'mind db\n' > "$project_root/.aoc/mind/project.sqlite"
printf 'task\n' > "$project_root/.taskmaster/docs/specs/spec.md"
printf 'tmp\n' > "$project_root/.pi/tmp/session.json"
printf 'extension\n' > "$project_root/.omp/extensions/aoc-state.ts"
printf 'agent\n' > "$project_root/.omp/agents/reviewer.md"
printf 'rules\n' > "$project_root/AGENTS.md"
printf 'design\n' > "$project_root/DESIGN.md"

status_output="$tmp_root/status.out"
(
  cd "$project_root"
  "$aoc_bin" state status > "$status_output"
)

assert_contains 'VCS: jj (preferred: jj)' "$status_output"
assert_contains '.omp/extensions' "$status_output"
assert_contains '.omp/agents' "$status_output"
assert_contains 'Runtime/churn artifacts excluded by policy:' "$status_output"
assert_contains 'No unsafe state candidates found by name/pattern scan.' "$status_output"
assert_contains 'No unexpectedly ignored project-state files found.' "$status_output"
assert_contains 'State audit result: project state is clean or trackable.' "$status_output"
assert_contains 'jj status' "$log_file"
assert_contains 'jj diff --summary -- .aoc .taskmaster .pi .omp/extensions .omp/agents AGENTS.md DESIGN.md' "$log_file"
assert_contains 'jj diff --stat -- .aoc .taskmaster .pi .omp/extensions .omp/agents AGENTS.md DESIGN.md' "$log_file"

unsafe_output="$tmp_root/unsafe.out"
printf 'OPENAI_API_KEY=bad\n' > "$project_root/.aoc/bad.env"
if (
  cd "$project_root"
  "$aoc_state_bin" status > "$unsafe_output"
); then
  fail "Expected unsafe .env audit to fail"
fi
assert_contains 'BLOCKED unsafe candidate: .aoc/bad.env' "$unsafe_output"

assert_executable "$aoc_state_bin"
assert_contains 'registerCommand("state-status"' "$state_extension"
assert_contains 'registerCommand("state-commit"' "$state_extension"
assert_contains 'registerCommand("state-push"' "$state_extension"
assert_contains 'jj git push --bookmark <bookmark> --remote <remote>' "$state_extension"
assert_contains 'Never run \`git push\` in a Jujutsu repo.' "$state_extension"
assert_contains 'Never run \`jj git push\` during /state-commit.' "$state_extension"
assert_not_contains 'Prefer \`git push' "$state_extension"

echo "PASS: aoc-state jj audit and prompts"
