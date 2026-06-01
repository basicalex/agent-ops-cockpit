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
