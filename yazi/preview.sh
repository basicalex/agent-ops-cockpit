#!/usr/bin/env bash
# Yazi preview script.
# Args: $1 = file path, $2 = width, $3 = height
set -euo pipefail

file="${1:-}"
w="${2:-80}"
h="${3:-24}"

have() { command -v "$1" >/dev/null 2>&1; }
bat_cmd() {
  if have bat; then
    echo "bat"
  elif have batcat; then
    echo "batcat"
  else
    echo ""
  fi
}

text_preview() {
  local cmd
  cmd="$(bat_cmd)"
  if [[ -n "$cmd" ]]; then
    "$cmd" --style=plain --color=always --line-range=:200 "$file"
  else
    sed -n '1,200p' "$file"
  fi
}

case "$file" in
  *.png|*.jpg|*.jpeg|*.webp|*.gif)
    if have chafa; then
      chafa --size="${w}x${h}" "$file"
    else
      text_preview
    fi
    ;;
  *.svg)
    if have rsvg-convert && have chafa; then
      tmp="/tmp/yazi_prev_$$.png"
      rsvg-convert "$file" -o "$tmp" >/dev/null 2>&1 || true
      if [[ -f "$tmp" ]]; then
        chafa --size="${w}x${h}" "$tmp"
        rm -f "$tmp"
      else
        text_preview
      fi
    else
      text_preview
    fi
    ;;
  *.pdf)
    if have pdftoppm && have chafa; then
      tmpdir="/tmp/yazi_pdfprev_$$"
      mkdir -p "$tmpdir"
      pdftoppm -f 1 -singlefile -png "$file" "$tmpdir/out" >/dev/null 2>&1 || true
      if [[ -f "$tmpdir/out.png" ]]; then
        chafa --size="${w}x${h}" "$tmpdir/out.png"
      else
        text_preview
      fi
      rm -rf "$tmpdir"
    else
      text_preview
    fi
    ;;
  *.tex)
    if have tectonic && have pdftoppm && have chafa; then
      tmpdir="/tmp/yazi_texprev_$$"
      mkdir -p "$tmpdir"
      tectonic "$file" --outdir "$tmpdir" >/dev/null 2>&1 || true
      pdf="$(ls -1 "$tmpdir"/*.pdf 2>/dev/null | head -n1 || true)"
      if [[ -n "$pdf" ]]; then
        pdftoppm -f 1 -singlefile -png "$pdf" "$tmpdir/out" >/dev/null 2>&1 || true
        if [[ -f "$tmpdir/out.png" ]]; then
          chafa --size="${w}x${h}" "$tmpdir/out.png"
        else
          text_preview
        fi
      else
        text_preview
      fi
      rm -rf "$tmpdir"
    else
      text_preview
    fi
    ;;
  *)
    if have file; then
      kind="$(file -Lb --mime-type "$file" || true)"
      case "$kind" in
        text/*|application/json|application/xml)
          text_preview
          ;;
        *)
          file -Lb "$file" || true
          ;;
      esac
    else
      text_preview
    fi
    ;;
esac
