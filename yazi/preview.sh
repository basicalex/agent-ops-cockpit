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
  local f="$1"
  if [[ -z "$f" ]]; then f="$file"; fi
  
  cmd="$(bat_cmd)"
  if [[ -n "$cmd" ]]; then
    # We ignore errors from bat (like Broken pipe)
    "$cmd" --style=plain --color=always --line-range=:200 "$f" 2>/dev/null || true
  else
    head -n 200 "$f"
  fi
}

archive_preview() {
  echo "Archive Contents:"
  echo "-----------------"
  if have bsdtar; then
    bsdtar -tf "$file" | head -n 50
  elif have unzip && [[ "$file" == *.zip ]]; then
    unzip -l "$file" | head -n 50
  elif have tar && [[ "$file" == *.tar* || "$file" == *.tgz ]]; then
    tar -tf "$file" | head -n 50
  elif have 7z; then
    7z l "$file" | head -n 50
  else
    echo "No suitable archive tool found (bsdtar, unzip, tar, 7z)."
  fi
}

json_preview() {
  if have jq; then
    jq -C . "$file" | head -n 200
  else
    text_preview
  fi
}

csv_preview() {
  if have csvlook; then
    csvlook "$file" | head -n 50
  elif have column; then
    column -s, -t "$file" | head -n 50
  else
    text_preview
  fi
}

# Create a safe temporary directory that cleans itself up
tmpdir=""
cleanup() {
  if [[ -n "$tmpdir" && -d "$tmpdir" ]]; then
    rm -rf "$tmpdir"
  fi
}
trap cleanup EXIT

case "${file,,}" in
  *.png|*.jpg|*.jpeg|*.webp|*.gif)
    if have chafa; then
      chafa --size="${w}x${h}" "$file"
    else
      text_preview
    fi
    ;;
  *.svg)
    if have rsvg-convert && have chafa; then
      tmpdir="$(mktemp -d 2>/dev/null || mktemp -d -t 'yazi_svg')"
      tmp="$tmpdir/out.png"
      rsvg-convert "$file" -o "$tmp" >/dev/null 2>&1 || true
      if [[ -f "$tmp" ]]; then
        chafa --size="${w}x${h}" "$tmp"
      else
        text_preview
      fi
    else
      text_preview
    fi
    ;;
  *.pdf)
    if have pdftoppm && have chafa; then
      tmpdir="$(mktemp -d 2>/dev/null || mktemp -d -t 'yazi_pdf')"
      pdftoppm -f 1 -singlefile -png "$file" "$tmpdir/out" >/dev/null 2>&1 || true
      if [[ -f "$tmpdir/out.png" ]]; then
        chafa --size="${w}x${h}" "$tmpdir/out.png"
      else
        text_preview
      fi
    else
      text_preview
    fi
    ;;
  *.tex)
    if have tectonic && have pdftoppm && have chafa; then
      tmpdir="$(mktemp -d 2>/dev/null || mktemp -d -t 'yazi_tex')"
      # Tectonic writes to the same dir as source by default or outdir
      # Using --outdir avoids cluttering the project
      tectonic "$file" --outdir "$tmpdir" >/dev/null 2>&1 || true
      pdf="$(find "$tmpdir" -name "*.pdf" | head -n1 || true)"
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
    else
      text_preview
    fi
    ;;
  *.zip|*.tar|*.tar.gz|*.tgz|*.rar|*.7z|*.jar|*.war|*.ear)
    archive_preview
    ;;
  *.json)
    json_preview
    ;;
  *.csv)
    csv_preview
    ;;
  *)
    if have file; then
      kind="$(file -Lb --mime-type "$file" || true)"
      case "$kind" in
        text/*|application/xml|application/javascript|application/x-sh)
          text_preview
          ;;
        application/json)
          json_preview
          ;;
        application/zip|application/x-tar|application/gzip|application/x-7z-compressed)
          archive_preview
          ;;
        image/*)
          if have chafa; then
             chafa --size="${w}x${h}" "$file"
          else
             text_preview
          fi
          ;;
        *)
          # Fallback: try text preview, if it looks binary 'bat' might warn or show hex
          # Just show file info first
          echo "File Type: $kind"
          echo "-----------------"
          text_preview
          ;;
      esac
    else
      text_preview
    fi
    ;;
esac
