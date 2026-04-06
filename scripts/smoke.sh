#!/usr/bin/env bash
set -euo pipefail

export AOC_SMOKE_TEST=1

echo "Running shell integration smoke tests..."

scripts=(
  bin/aoc-launch
  bin/aoc-new-tab
  bin/aoc-hub
  bin/aoc-mission-control-toggle
  bin/aoc-agent-wrap
  bin/aoc-pi
  bin/aoc-agent-run
)

for script in "${scripts[@]}"; do
  echo "Smoke testing $script..."
  if ! bash "$script" >/dev/null; then
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

echo "Smoke testing bin/aoc-agent current..."
if ! bash bin/aoc-agent --current >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-agent --current"
  exit 1
fi

echo "Smoke testing bin/aoc-agent-install status..."
if ! bash bin/aoc-agent-install status pi >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-agent-install status pi"
  exit 1
fi

echo "Smoke testing bin/aoc-zellij-plugin status..."
if ! bash bin/aoc-zellij-plugin status >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-zellij-plugin status"
  exit 1
fi

echo "Smoke testing bin/aoc-tab-metadata status..."
if ! bash bin/aoc-tab-metadata status >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-tab-metadata status"
  exit 1
fi

echo "Smoke testing custom layout create/edit flow..."
layout_smoke_name="smoke-layout-$$"
if ! VISUAL=true EDITOR=true bash bin/aoc-layout --create "$layout_smoke_name" --scope global >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-layout --create"
  rm -f "$HOME/.config/zellij/layouts/$layout_smoke_name.kdl"
  exit 1
fi
if ! test -f "$HOME/.config/zellij/layouts/$layout_smoke_name.kdl"; then
  echo "ERROR: Smoke layout file was not created"
  exit 1
fi
if ! VISUAL=true EDITOR=true bash bin/aoc-layout --edit "$layout_smoke_name" --scope auto >/dev/null; then
  echo "ERROR: Smoke test failed for bin/aoc-layout --edit"
  rm -f "$HOME/.config/zellij/layouts/$layout_smoke_name.kdl"
  exit 1
fi
rm -f "$HOME/.config/zellij/layouts/$layout_smoke_name.kdl"

echo "Smoke testing non-PI agent rejection..."
if bash bin/aoc-agent-install status codex >/dev/null 2>&1; then
  echo "ERROR: non-PI agent status unexpectedly succeeded"
  exit 1
fi

echo "All shell integration smoke tests passed successfully."
