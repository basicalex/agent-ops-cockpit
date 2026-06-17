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

if [[ "$compact" == *"Focused commands:"* ]]; then
  echo "ERROR: compact AOC OMP context included verbose focused command list" >&2
  exit 1
fi

if [[ "$min" != *"policy=metadata-only"* ]]; then
  echo "ERROR: min AOC OMP context did not include minimal policy line" >&2
  exit 1
fi

if [[ "$min" == *"## Mind / recall policy"* ]]; then
  echo "ERROR: min AOC OMP context included verbose sections" >&2
  exit 1
fi

if [[ "$full" != *"## Mind / recall policy"* || "$full" != *"Focused commands:"* ]]; then
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
jj_fixture="$tmp_dir/jj-colocated"
mkdir -p "$git_fixture/.git" "$jj_fixture/.git" "$jj_fixture/.jj"

git_min="$(AOC_OMP_CONTEXT_LEVEL=min bin/aoc-omp-context "$git_fixture")"
git_compact="$(AOC_OMP_CONTEXT_LEVEL=compact bin/aoc-omp-context "$git_fixture")"
jj_min="$(AOC_OMP_CONTEXT_LEVEL=min bin/aoc-omp-context "$jj_fixture")"
jj_full="$(AOC_OMP_CONTEXT_LEVEL=full bin/aoc-omp-context "$jj_fixture")"

if [[ "$git_min" != *"vcs=git; preferred=git"* ]]; then
  echo "ERROR: git-only fixture did not report git VCS in min context" >&2
  exit 1
fi

if [[ "$git_compact" != *"VCS: git; preferred=git"* ]]; then
  echo "ERROR: git-only fixture did not report git VCS in compact context" >&2
  exit 1
fi

if [[ "$jj_min" != *"vcs=jj; preferred=jj"* ]]; then
  echo "ERROR: colocated Jujutsu fixture did not report jj VCS in min context" >&2
  exit 1
fi

if [[ "$jj_full" != *"## VCS policy"* || "$jj_full" != *"not a Git staging area"* || "$jj_full" != *"jj split"* ]]; then
  echo "ERROR: colocated Jujutsu fixture did not include full Jujutsu safety guidance" >&2
  exit 1
fi

for required in \
  .omp/extensions/aoc-codegraph.ts \
  .omp/extensions/aoc-commit.ts \
  .omp/extensions/aoc-jj-init.ts \
  .omp/extensions/aoc-brand-content.ts \
  .omp/extensions/aoc-web-search.ts \
  .omp/agents/brand-strategy.md \
  .omp/agents/brand-concept.md \
  .omp/agents/svg-asset.md \
  .omp/agents/hyperframes-content.md \
  .omp/skills/aoc-dox-cartography/SKILL.md \
  .omp/skills/aoc-init-ops/SKILL.md; do
  if [[ ! -f "$required" ]]; then
    echo "ERROR: missing repo-tracked OMP runtime asset: $required" >&2
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
