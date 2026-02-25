#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
context_pack_script="$repo_root/scripts/opencode/context-pack.sh"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local message="$3"
  case "$haystack" in
    *"$needle"*)
      ;;
    *)
      fail "$message (missing: $needle)"
      ;;
  esac
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-context-pack-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

project_root="$tmp_root/project"
mkdir -p "$project_root/.aoc/stm/archive" "$project_root/.aoc/insight"

cat > "$project_root/.aoc/memory.md" <<'EOF'
# Memory
MEM_TOKEN_ALPHA
MEM_TOKEN_BRAVO
MEM_TOKEN_CHARLIE
MEM_TOKEN_DELTA
MEM_TOKEN_ECHO
EOF

cat > "$project_root/.aoc/stm/current.md" <<'EOF'
STM_TOKEN_ALPHA
STM_TOKEN_BRAVO
STM_TOKEN_CHARLIE
EOF

cat > "$project_root/.aoc/insight/current.md" <<'EOF'
MIND_TOKEN_ALPHA
MIND_TOKEN_BRAVO
EOF

output_one="$(bash "$context_pack_script" \
  --project-root "$project_root" \
  --max-chars 1800 \
  --mem-max-lines 3 \
  --stm-max-lines 2 \
  --mind-max-lines 1 \
  --mem-max-chars 300 \
  --stm-max-chars 300 \
  --mind-max-chars 300)"

output_two="$(bash "$context_pack_script" \
  --project-root "$project_root" \
  --max-chars 1800 \
  --mem-max-lines 3 \
  --stm-max-lines 2 \
  --mind-max-lines 1 \
  --mem-max-chars 300 \
  --stm-max-chars 300 \
  --mind-max-chars 300)"

if [[ "$output_one" != "$output_two" ]]; then
  fail "context-pack output must be deterministic for same inputs"
fi

assert_contains "$output_one" "## aoc-mem" "must include aoc-mem section"
assert_contains "$output_one" "## aoc-stm" "must include aoc-stm section"
assert_contains "$output_one" "## aoc-mind" "must include aoc-mind section"
assert_contains "$output_one" "source: $project_root/.aoc/memory.md" "must cite memory path"
assert_contains "$output_one" "source: $project_root/.aoc/stm/current.md" "must cite stm current path"
assert_contains "$output_one" "source: $project_root/.aoc/insight/current.md" "must cite mind summary path"

assert_contains "$output_one" "MEM_TOKEN_ALPHA" "must include mem content"
assert_contains "$output_one" "STM_TOKEN_ALPHA" "must include stm content"
assert_contains "$output_one" "MIND_TOKEN_ALPHA" "must include mind content"

python3 - "$output_one" <<'PY'
import sys

text = sys.argv[1]

mem_idx = text.find("## aoc-mem")
stm_idx = text.find("## aoc-stm")
mind_idx = text.find("## aoc-mind")

if mem_idx < 0 or stm_idx < 0 or mind_idx < 0:
    raise SystemExit("missing section header")
if not (mem_idx < stm_idx < mind_idx):
    raise SystemExit("section ordering must be mem -> stm -> mind")
PY

cat > "$project_root/.aoc/stm/current.md" <<'EOF'

EOF
cat > "$project_root/.aoc/stm/archive/20260222-000000Z-test.md" <<'EOF'
STM_ARCHIVE_TOKEN
EOF

archive_output="$(bash "$context_pack_script" --project-root "$project_root" --max-chars 1800)"
assert_contains "$archive_output" "STM_ARCHIVE_TOKEN" "must fallback to archived stm when current is empty"
assert_contains "$archive_output" "mode: archive" "stm mode should report archive fallback"

rm -f "$project_root/.aoc/stm/archive/20260222-000000Z-test.md"
cat > "$project_root/.aoc/stm/current.md" <<'EOF'
STM_CURRENT_ONLY_TOKEN
EOF

draft_output="$(bash "$context_pack_script" --project-root "$project_root" --max-chars 1800)"
assert_contains "$draft_output" "STM_CURRENT_ONLY_TOKEN" "must fallback to current draft when no archive exists"
assert_contains "$draft_output" "mode: current-draft" "stm mode should report current-draft fallback"

small_output="$(bash "$context_pack_script" --project-root "$project_root" --max-chars 450 --mem-max-lines 2 --stm-max-lines 2 --mind-max-lines 1)"
small_len=${#small_output}
if ((small_len > 520)); then
  fail "small output should stay bounded (got $small_len chars)"
fi

echo "All context-pack tests passed."
