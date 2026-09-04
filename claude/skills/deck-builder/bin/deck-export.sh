#!/usr/bin/env bash
# deck-export.sh <workspace> "<Output Name>" [--no-notes]
# gate -> split notes -> export PPTX -> render 1x/2x -> PDF. Fails on any blocking gate error.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
WS=$(cd "${1:?workspace dir}" && pwd); NAME=${2:?output name}; NOTES=--with-notes
[ "${3:-}" = "--no-notes" ] && NOTES=""
PROJ=$(find_project "$WS")
SVG="$PROJ/svg_output"
[ -n "$(ls "$SVG"/*.svg 2>/dev/null)" ] || die "no pages in $SVG"

echo "== gate"
bash "$SKILL_DIR/bin/deck-gate.sh" "$WS"

if [ -n "$NOTES" ] && [ -f "$PROJ/notes/total.md" ]; then
  echo "== notes"
  (cd "$PPT_ROOT" && "$PY" "$PPT_SKILL/scripts/total_md_split.py" "$PROJ" -q)
fi
if [ -n "$NOTES" ] && [ -z "$(ls "$PROJ"/notes/*.md 2>/dev/null | grep -v total.md)" ]; then
  echo "no per-slide notes found; exporting without notes"; NOTES=""
fi

echo "== export"
mkdir -p "$WS/dist"
PPTX="$WS/dist/$NAME.pptx"
(cd "$PPT_ROOT" && "$PY" "$PPT_SKILL/scripts/svg_to_pptx.py" "$PROJ" --quick-generate $NOTES -o "$PPTX") | grep -E 'POSTFLIGHT|Saved|ERROR|error' || true
[ -f "$PPTX" ] || die "export produced no file"

echo "== render"
bun "$SKILL_DIR/bin/render-svg.mjs" "$SVG" "$WS/render" 1
bun "$SKILL_DIR/bin/render-svg.mjs" "$SVG" "$WS/render-2x" 2

echo "== pdf"
"$PY" "$SKILL_DIR/bin/build-pdf.py" "$WS/render-2x" "$WS/dist/$NAME.pdf"

echo
echo "pptx   : $PPTX ($(du -h "$PPTX" | cut -f1))"
echo "pdf    : $WS/dist/$NAME.pdf"
echo "render : $WS/render/slide-NN.png  (look at every one before reporting done)"
