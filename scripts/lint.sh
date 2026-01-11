#!/usr/bin/env bash
set -euo pipefail

if ! command -v shellcheck >/dev/null 2>&1; then
  echo "shellcheck not found. Install it to run lint."
  exit 1
fi

files=(
  bin/aoc-doctor
  bin/aoc-launch
  bin/aoc-star
  bin/aoc-sys
  bin/aoc-taskmaster
  bin/aoc-test
  bin/aoc-uninstall
  bin/aoc-widget
  install.sh
  scripts/build-taskmaster-plugin.sh
  yazi/preview.sh
)

shellcheck -x "${files[@]}"
