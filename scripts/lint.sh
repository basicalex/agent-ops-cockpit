#!/usr/bin/env bash
set -euo pipefail

if ! command -v shellcheck >/dev/null 2>&1; then
  echo "shellcheck not found; running bash -n syntax fallback."
  for f in bin/* install.sh install/bootstrap.sh legacy/opencode/scripts/*.sh; do
    if [ -f "$f" ] && head -n 1 "$f" | grep -q 'bash'; then
      bash -n "$f"
    fi
  done
  exit 0
fi

files=(
  bin/*
  install.sh
  install/bootstrap.sh
  legacy/opencode/scripts/*.sh
)

# Filter out non-files or directories just in case
check_files=()
for f in "${files[@]}"; do
  if [ -f "$f" ]; then
    check_files+=("$f")
  fi
done

shellcheck -S error -x "${check_files[@]}"
