#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

compact="$(AOC_OMP_CONTEXT_LEVEL=compact bin/aoc-omp-context)"
min="$(AOC_OMP_CONTEXT_LEVEL=min bin/aoc-omp-context)"
full="$(AOC_OMP_CONTEXT_LEVEL=full bin/aoc-omp-context)"

if [[ "$compact" != *"Mode: metadata-only startup capsule"* ]]; then
  echo "ERROR: compact AOC OMP context did not include compact metadata marker" >&2
  exit 1
fi

if [[ "$compact" == *"## Context policy"* ]]; then
  echo "ERROR: compact AOC OMP context included verbose sections" >&2
  exit 1
fi

if [[ "$min" != *"Mode: metadata-only startup capsule"* ]]; then
  echo "ERROR: min AOC OMP context did not include metadata-only marker" >&2
  exit 1
fi

if [[ "$min" == *"## Context policy"* ]]; then
  echo "ERROR: min AOC OMP context included verbose sections" >&2
  exit 1
fi

if [[ "$full" != *"## Context policy"* || "$full" != *"## Slash commands"* ]]; then
  echo "ERROR: full AOC OMP context did not preserve verbose debug capsule" >&2
  exit 1
fi

if AOC_OMP_CONTEXT_LEVEL=invalid bin/aoc-omp-context >/dev/null 2>&1; then
  echo "ERROR: invalid AOC_OMP_CONTEXT_LEVEL unexpectedly succeeded" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT
git_fixture="$tmp_dir/git-only"
mkdir -p "$git_fixture/.git"

git_min="$(AOC_OMP_CONTEXT_LEVEL=min bin/aoc-omp-context "$git_fixture")"
git_compact="$(AOC_OMP_CONTEXT_LEVEL=compact bin/aoc-omp-context "$git_fixture")"

if [[ "$git_min" != *"VCS: git; preferred=git"* ]]; then
  echo "ERROR: git-only fixture did not report git VCS in min context" >&2
  exit 1
fi

if [[ "$git_compact" != *"VCS: git; preferred=git"* ]]; then
  echo "ERROR: git-only fixture did not report git VCS in compact context" >&2
  exit 1
fi


for required in \
  .omp/extensions/aoc-codegraph.ts \
  .omp/extensions/aoc-commit.ts \
  .omp/extensions/aoc-brand-content.ts \
  .omp/extensions/aoc-web-search.ts \
  .omp/extensions/aoc-style.ts \
  .omp/extensions/aoc-profile.ts \
  .omp/agents/brand-strategy.md \
  .omp/agents/brand-concept.md \
  .omp/agents/svg-asset.md \
  .omp/agents/hyperframes-content.md \
  .omp/skills/aoc-dox-cartography/SKILL.md \
  .omp/skills/aoc-init-ops/SKILL.md \
  .omp/skills/ponytail-workflows/SKILL.md; do
  if [[ ! -f "$required" ]]; then
    echo "ERROR: missing repo-tracked OMP runtime asset: $required" >&2
    exit 1
  fi
done

style_state="$tmp_dir/style-state.json"
raw_omp="$tmp_dir/omp raw"
raw_log="$tmp_dir/raw-omp-args"
cat >"$raw_omp" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
: "${RAW_OMP_LOG:?}"
>"$RAW_OMP_LOG"
for arg in "$@"; do
  printf '<%s>\n' "$arg" >>"$RAW_OMP_LOG"
done
SH
chmod +x "$raw_omp"

run_aoc_omp() {
  env \
    AOC_RAW_OMP_BIN="$raw_omp" \
    RAW_OMP_LOG="$raw_log" \
    AOC_PROFILE_STATE_FILE="$tmp_dir/profile-state.json" \
    "$@"
}

run_aoc_omp bin/aoc-omp "prompt with spaces"
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" != *"<--skills>"* || "$raw_args" != *"<aoc-understand,ponytail-workflows>"* ]]; then
  echo "ERROR: default aoc-omp launch did not forward lean skill allowlist" >&2
  exit 1
fi
if [[ "$raw_args" != *"<prompt with spaces>"* ]]; then
  echo "ERROR: aoc-omp did not preserve arguments containing spaces" >&2
  exit 1
fi

run_aoc_omp AOC_OMP_PROFILES=core,hyperframes bin/aoc-omp run
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" != *"<aoc-understand,ponytail-workflows,aoc-hyperframes,hyperframes,hyperframes-cli,website-to-hyperframes,gsap>"* ]]; then
  echo "ERROR: combined profile launch did not forward all active skills once" >&2
  exit 1
fi

printf '{"version":1,"projects":{"%s":{"enabled":[],"disabled":["core","unknown-profile"]}}}\n' "$root" >"$tmp_dir/profile-state.json"
empty_profile_err="$tmp_dir/empty-profile-stderr"
run_aoc_omp bin/aoc-omp run 2>"$empty_profile_err"
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" != *"<--no-skills>"* || "$raw_args" == *"<--skills>"* || "$raw_args" == *"warning:"* || "$raw_args" == *"unknown-profile"* ]]; then
  echo "ERROR: empty active profile set did not forward clean --no-skills" >&2
  exit 1
fi
if [[ -s "$empty_profile_err" ]]; then
  echo "ERROR: successful profile warnings leaked to wrapper stderr" >&2
  exit 1
fi

printf 'not-json\n' >"$tmp_dir/profile-state.json"
failure_profile_err="$tmp_dir/failure-profile-stderr"
if run_aoc_omp bin/aoc-omp run >/dev/null 2>"$failure_profile_err"; then
  echo "ERROR: invalid profile state did not fail wrapper launch" >&2
  exit 1
fi
failure_profile_text="$(cat "$failure_profile_err")"
if [[ "$failure_profile_text" != aoc-omp:\ failed\ to\ resolve\ active\ skills:*invalid\ profile\ state* ]]; then
  echo "ERROR: failed profile resolution did not retain concise diagnostic" >&2
  exit 1
fi

run_aoc_omp bin/aoc-omp --skills custom-one,custom-two run
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" == *"<aoc-understand>"* || "$raw_args" != *"<--skills>"* || "$raw_args" != *"<custom-one,custom-two>"* ]]; then
  echo "ERROR: explicit --skills did not suppress generated allowlist" >&2
  exit 1
fi

run_aoc_omp bin/aoc-omp --skills=custom-eq run
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" == *"<aoc-understand>"* || "$raw_args" != *"<--skills=custom-eq>"* ]]; then
  echo "ERROR: explicit --skills=VALUE did not suppress generated allowlist" >&2
  exit 1
fi

run_aoc_omp bin/aoc-omp --no-skills run
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" == *"<aoc-understand>"* || "$raw_args" != *"<--no-skills>"* ]]; then
  echo "ERROR: --no-skills did not suppress generated allowlist" >&2
  exit 1
fi

run_aoc_omp bin/aoc-omp --version
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" != "<--version>" ]]; then
  echo "ERROR: version command was not passed directly to raw OMP" >&2
  exit 1
fi

run_aoc_omp bin/aoc-omp config get skills.enableClaudeUser
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" != $'<config>\n<get>\n<skills.enableClaudeUser>' ]]; then
  echo "ERROR: OMP subcommand was not passed directly to raw OMP" >&2
  exit 1
fi
for management_command in wt q __complete help; do
  run_aoc_omp bin/aoc-omp "$management_command" probe
  raw_args="$(cat "$raw_log")"
  if [[ "$raw_args" != "<$management_command>"$'\n''<probe>' ]]; then
    echo "ERROR: OMP management command was not passed directly to raw OMP: $management_command" >&2
    exit 1
  fi
done


run_aoc_omp bin/aoc-omp update
raw_args="$(cat "$raw_log")"
if [[ "$raw_args" != "<update>" ]]; then
  echo "ERROR: update command was not passed directly to raw OMP" >&2
  exit 1
fi

printf '%s\n' '{"version":1,"ponytail":"lite","caveman":"ultra","updatedAt":"test"}' >"$style_state"
style_context="$(AOC_STYLE_STATE_FILE="$style_state" AOC_OMP_CONTEXT_LEVEL=compact bin/aoc-omp-context)"
for required in \
  "# AOC Host Style Hooks" \
  "Ponytail engineering mode: lite." \
  "Caveman output mode: ultra." \
  "Ultra: use arrows and common prose abbreviations only when they cannot alter technical meaning."; do
  if [[ "$style_context" != *"$required"* ]]; then
    echo "ERROR: compact context did not include style hook line: $required" >&2
    exit 1
  fi
done

compact_bytes=$(printf '%s' "$compact" | wc -c | tr -d ' ')
full_bytes=$(printf '%s' "$full" | wc -c | tr -d ' ')
if (( compact_bytes >= full_bytes )); then
  echo "ERROR: compact context (${compact_bytes} bytes) is not smaller than full context (${full_bytes} bytes)" >&2
  exit 1
fi

printf 'AOC OMP context levels passed (min=%s compact=%s full=%s bytes)\n' \
  "$(printf '%s' "$min" | wc -c | tr -d ' ')" \
  "$compact_bytes" \
  "$full_bytes"
