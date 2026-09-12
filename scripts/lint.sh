#!/usr/bin/env bash
set -euo pipefail

collect_shell_files() {
  local f first_line
  for f in bin/* install.sh install/bootstrap.sh legacy/opencode/scripts/*.sh; do
    [[ -f "$f" ]] || continue
    first_line="$(head -n 1 "$f")"
    if [[ "$first_line" =~ ^#!.*(^|[/[:space:]])(bash|sh|dash|ksh)([[:space:]]|$) ]]; then
      check_files+=("$f")
    fi
  done
}

check_files=()
collect_shell_files

if ! command -v shellcheck >/dev/null 2>&1; then
  echo "shellcheck not found; running bash -n syntax fallback."
  for f in "${check_files[@]}"; do
    bash -n "$f"
  done
  echo "Checked ${#check_files[@]} shell files."
  exit 0
fi

shellcheck -S error -x "${check_files[@]}"
echo "Checked ${#check_files[@]} shell files."
