#!/usr/bin/env bash
set -euo pipefail

if ! command -v shellcheck >/dev/null 2>&1; then
  echo "shellcheck not found. Install it to run lint."
  exit 1
fi

files=(
  bin/*
  install.sh
  install/bootstrap.sh
  scripts/opencode/*.sh
  yazi/preview.sh
)

# Filter out non-files or directories just in case
check_files=()
for f in "${files[@]}"; do
  if [ -f "$f" ]; then
    check_files+=("$f")
  fi
done

shellcheck -S error -x "${check_files[@]}"
