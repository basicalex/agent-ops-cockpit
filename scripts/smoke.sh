#!/usr/bin/env bash
set -euo pipefail

export AOC_SMOKE_TEST=1

scripts=(
  bin/aoc-launch
  bin/aoc-new-tab
  bin/aoc-hub
  bin/aoc-mission-control-toggle
  bin/aoc-agent-wrap
  bin/aoc-omo
  bin/aoc-pi
  bin/aoc-pi-r
)

echo "Running shell integration smoke tests..."

for script in "${scripts[@]}"; do
  echo "Smoke testing $script..."
  if ! bash "$script"; then
    echo "ERROR: Smoke test failed for $script"
    exit 1
  fi
done

echo "Smoke testing bin/aoc-rtk status..."
if ! bash bin/aoc-rtk status --shell >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-rtk status"
  exit 1
fi

echo "Smoke testing bin/aoc-rtk manual route..."
tmp_dir="$(mktemp -d)"
cat <<'EOF' > "$tmp_dir/rtk.toml"
mode = "on"
fail_open = true
gain_mode = "double-dash"
binary = "missing-rtk"
allowlist = ["echo"]
denylist = []
install_url = ""
install_sha256 = ""
EOF
if ! AOC_RTK_CONFIG="$tmp_dir/rtk.toml" bash bin/aoc-rtk echo smoke-test >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-rtk manual route"
  rm -rf "$tmp_dir"
  exit 1
fi
rm -rf "$tmp_dir"

echo "Smoke testing bin/aoc-agent-install status..."
if ! bash bin/aoc-agent-install status codex >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-agent-install status"
  exit 1
fi

echo "Smoke testing bin/aoc-agent-install PI Rust consent gate..."
if AOC_PI_CONSENT_FILE="$(mktemp -u)" AOC_PIR_INSTALL_CMD="printf 'noop'" bash bin/aoc-agent-install install pi-r >/dev/null 2>&1; then
  echo "ERROR: PI Rust consent gate smoke test unexpectedly passed"
  exit 1
fi

if [[ -f "scripts/opencode/verify-omo.sh" && -f "config/opencode/oh-my-opencode.policy.jsonc" ]]; then
  echo "Smoke testing OmO governance checks..."
  if ! bash scripts/opencode/verify-omo.sh regression --policy "config/opencode/oh-my-opencode.policy.jsonc" --project-root "$PWD" --profile sandbox --max-chars 4096 >/dev/null; then
    echo "ERROR: Smoke test failed for OmO governance checks"
    exit 1
  fi
fi

echo "All shell integration smoke tests passed successfully."
