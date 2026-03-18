#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

secret_pattern='(sk-or-v1-[A-Za-z0-9_-]{8,}|ANTHROPIC_API_KEY=|OPENAI_API_KEY=|Authorization:[[:space:]]*Bearer[[:space:]]+|x-api-key[[:space:]]*[:=][[:space:]]*)'
forbidden_runtime_pattern='(^|/)\.aoc/mind/.*(project\.sqlite|\.sqlite$|\.lock$)'

tracked_runtime_hits="$(git ls-files | grep -E "$forbidden_runtime_pattern" || true)"
if [[ -n "$tracked_runtime_hits" ]]; then
  echo "Forbidden tracked Mind runtime artifacts detected:" >&2
  echo "$tracked_runtime_hits" >&2
  exit 1
fi

export_files="$(find .aoc/mind -type f \( -path '*/t3/*.md' -o -path '*/insight/*.md' -o -path '*/insight/*.json' \) 2>/dev/null | sort || true)"
if [[ -z "$export_files" ]]; then
  echo "Mind runtime safety verification passed."
  exit 0
fi

secret_hits=0
while IFS= read -r path; do
  [[ -z "$path" ]] && continue
  if grep -I -n -E "$secret_pattern" "$path" >/tmp/mind-secret-scan-hit.$$ 2>/dev/null; then
    echo "Potential secret marker detected in Mind export file: $path" >&2
    cat /tmp/mind-secret-scan-hit.$$ >&2
    secret_hits=1
  fi
done <<< "$export_files"
rm -f /tmp/mind-secret-scan-hit.$$ >/dev/null 2>&1 || true

if [[ "$secret_hits" -ne 0 ]]; then
  exit 1
fi

echo "Mind runtime safety verification passed."
