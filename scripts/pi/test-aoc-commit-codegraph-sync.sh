#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
commit_extension="$repo_root/.omp/extensions/aoc-commit.ts"
commit_docs="$repo_root/docs/commit-intelligence.md"
workspace_docs="$repo_root/docs/herdr-workspace.md"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq "$needle" "$file" || fail "Expected $file to contain: $needle"
}

[[ -f "$commit_extension" ]] || fail "Missing OMP commit extension: $commit_extension"
[[ -f "$commit_docs" ]] || fail "Missing commit intelligence docs: $commit_docs"
[[ -f "$workspace_docs" ]] || fail "Missing Herdr workspace docs: $workspace_docs"

assert_contains "$commit_extension" 'After a successful commit only, if `.codegraph/` exists and `codegraph` is on PATH, run `codegraph sync <repo-root>`'
assert_contains "$commit_extension" 'Never run CodeGraph sync before the commit'
assert_contains "$commit_extension" 'CodeGraph cache status: synced | skipped (no index/CLI) | failed (advisory; reason)'
assert_contains "$commit_docs" 'After a successful commit, agents should refresh CodeGraph only as cache maintenance'
assert_contains "$workspace_docs" 'post-commit advisory cache maintenance'

echo "OK: /commit post-commit CodeGraph sync contract is documented"
