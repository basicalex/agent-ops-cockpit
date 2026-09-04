#!/usr/bin/env bash
# One-time (idempotent) setup for deck-builder: tool clones, Python venv, Playwright Chromium.
set -euo pipefail
TOOLS="${DECK_TOOLS:-$HOME/dev/tools}"
SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$TOOLS"

clone_or_pull() {
  local url=$1 dir=$2
  if [ -d "$dir/.git" ]; then
    git -C "$dir" pull --ff-only -q && echo "updated  $dir"
  else
    git clone -q "$url" "$dir" && echo "cloned   $dir"
  fi
}
clone_or_pull https://github.com/hugohe3/ppt-master "$TOOLS/ppt-master"
clone_or_pull https://github.com/larashero3-dotcom/lieflat-charts "$TOOLS/lieflat-charts"

VENV="$TOOLS/ppt-venv"
if [ ! -x "$VENV/bin/python" ]; then
  uv venv --python 3.12 -q "$VENV"
  echo "created  $VENV"
fi
uv pip install -q --python "$VENV/bin/python" -r "$TOOLS/ppt-master/skills/ppt-master/requirements.txt"
echo "python   $("$VENV/bin/python" --version) at $VENV"

(cd "$SKILL_DIR" && bun install --silent)
"$SKILL_DIR/node_modules/.bin/playwright" install chromium >/dev/null
echo "playwright chromium ready"

"$VENV/bin/python" -c "import pptx, pymupdf, pathops, uharfbuzz; print('venv deps ok')"
echo "tools root: $TOOLS"
