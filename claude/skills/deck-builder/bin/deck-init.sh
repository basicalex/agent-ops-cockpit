#!/usr/bin/env bash
# deck-init.sh <workspace> <slug> [format]   — create a deck workspace and its ppt-master project.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
WS=${1:?workspace dir}; SLUG=${2:?slug}; FORMAT=${3:-ppt169}
WS=$(mkdir -p "$WS" && cd "$WS" && pwd)
mkdir -p "$WS/images" "$WS/charts" "$WS/project" "$WS/render" "$WS/render-2x" "$WS/dist" "$WS/qa"
for f in BRIEF.md content.md DECK-PACKET.md; do
  [ -e "$WS/$f" ] || cp "$SKILL_DIR/templates/$f" "$WS/$f"
done
if [ -z "$(find "$WS/project" -mindepth 1 -maxdepth 1 -type d)" ]; then
  (cd "$PPT_ROOT" && "$PY" "$PPT_SKILL/scripts/project_manager.py" init "$SLUG" --dir "$WS/project" --format "$FORMAT" --quick-generate)
fi
PROJ=$(find_project "$WS")
mkdir -p "$PROJ/svg_output" "$PROJ/notes" "$PROJ/images"
cat <<MSG
workspace : $WS
project   : $PROJ
next      : fill $WS/content.md and $WS/BRIEF.md, drop images into $WS/images/,
            then write the worker packet from $WS/DECK-PACKET.md
MSG
