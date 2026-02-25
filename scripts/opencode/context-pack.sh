#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat <<'EOF'
Usage: context-pack.sh [options]

Compose bounded AOC context in deterministic precedence order:
  aoc-mem -> aoc-stm -> aoc-mind

Options:
  --project-root <path>      Project root containing .aoc (default: cwd)
  --max-chars <n>            Total max output chars (default: 12000)
  --mem-max-lines <n>        Max lines from aoc-mem section (default: 120)
  --stm-max-lines <n>        Max lines from aoc-stm section (default: 120)
  --mind-max-lines <n>       Max lines from aoc-mind section (default: 80)
  --mem-max-chars <n>        Max chars from aoc-mem section (default: 5000)
  --stm-max-chars <n>        Max chars from aoc-stm section (default: 4000)
  --mind-max-chars <n>       Max chars from aoc-mind section (default: 2500)
  --help                     Show this help

Environment overrides:
  AOC_CONTEXT_PACK_MAX_CHARS
  AOC_CONTEXT_PACK_MEM_MAX_LINES
  AOC_CONTEXT_PACK_STM_MAX_LINES
  AOC_CONTEXT_PACK_MIND_MAX_LINES
  AOC_CONTEXT_PACK_MEM_MAX_CHARS
  AOC_CONTEXT_PACK_STM_MAX_CHARS
  AOC_CONTEXT_PACK_MIND_MAX_CHARS
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

is_number() {
  [[ "$1" =~ ^[0-9]+$ ]]
}

run_in_project() {
  local project_root="$1"
  shift
  (
    cd "$project_root"
    "$@"
  )
}

resolve_mind_file() {
  local project_root="$1"
  local candidate=""
  local -a candidates=(
    "$project_root/.aoc/mind/current.md"
    "$project_root/.aoc/mind/latest.md"
    "$project_root/.aoc/insight/current.md"
    "$project_root/.aoc/insight/index.md"
  )

  for candidate in "${candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
      printf '%s' "$candidate"
      return
    fi
  done
}

main() {
  local project_root="$PWD"
  local max_chars="${AOC_CONTEXT_PACK_MAX_CHARS:-12000}"
  local mem_max_lines="${AOC_CONTEXT_PACK_MEM_MAX_LINES:-120}"
  local stm_max_lines="${AOC_CONTEXT_PACK_STM_MAX_LINES:-120}"
  local mind_max_lines="${AOC_CONTEXT_PACK_MIND_MAX_LINES:-80}"
  local mem_max_chars="${AOC_CONTEXT_PACK_MEM_MAX_CHARS:-5000}"
  local stm_max_chars="${AOC_CONTEXT_PACK_STM_MAX_CHARS:-4000}"
  local mind_max_chars="${AOC_CONTEXT_PACK_MIND_MAX_CHARS:-2500}"

  while (($# > 0)); do
    case "$1" in
      --project-root)
        [[ $# -ge 2 ]] || die "--project-root requires a value"
        project_root="$2"
        shift 2
        ;;
      --max-chars)
        [[ $# -ge 2 ]] || die "--max-chars requires a value"
        max_chars="$2"
        shift 2
        ;;
      --mem-max-lines)
        [[ $# -ge 2 ]] || die "--mem-max-lines requires a value"
        mem_max_lines="$2"
        shift 2
        ;;
      --stm-max-lines)
        [[ $# -ge 2 ]] || die "--stm-max-lines requires a value"
        stm_max_lines="$2"
        shift 2
        ;;
      --mind-max-lines)
        [[ $# -ge 2 ]] || die "--mind-max-lines requires a value"
        mind_max_lines="$2"
        shift 2
        ;;
      --mem-max-chars)
        [[ $# -ge 2 ]] || die "--mem-max-chars requires a value"
        mem_max_chars="$2"
        shift 2
        ;;
      --stm-max-chars)
        [[ $# -ge 2 ]] || die "--stm-max-chars requires a value"
        stm_max_chars="$2"
        shift 2
        ;;
      --mind-max-chars)
        [[ $# -ge 2 ]] || die "--mind-max-chars requires a value"
        mind_max_chars="$2"
        shift 2
        ;;
      -h|--help)
        usage
        return 0
        ;;
      *)
        die "unknown option: $1"
        ;;
    esac
  done

  [[ -d "$project_root" ]] || die "project root not found: $project_root"

  is_number "$max_chars" || die "--max-chars must be numeric"
  is_number "$mem_max_lines" || die "--mem-max-lines must be numeric"
  is_number "$stm_max_lines" || die "--stm-max-lines must be numeric"
  is_number "$mind_max_lines" || die "--mind-max-lines must be numeric"
  is_number "$mem_max_chars" || die "--mem-max-chars must be numeric"
  is_number "$stm_max_chars" || die "--stm-max-chars must be numeric"
  is_number "$mind_max_chars" || die "--mind-max-chars must be numeric"

  local tmp_root
  tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-context-pack.XXXXXX")"
  trap "rm -rf '$tmp_root'" EXIT

  local mem_source stm_source mind_source stm_mode
  local mem_content_file stm_content_file mind_content_file
  mem_content_file="$tmp_root/mem.txt"
  stm_content_file="$tmp_root/stm.txt"
  mind_content_file="$tmp_root/mind.txt"
  : > "$mem_content_file"
  : > "$stm_content_file"
  : > "$mind_content_file"

  mem_source="$project_root/.aoc/memory.md"
  run_in_project "$project_root" env "AOC_MEMORY_FILE=$mem_source" aoc-mem read > "$mem_content_file" 2>/dev/null || true

  stm_mode="archive"
  stm_source="$(run_in_project "$project_root" aoc-stm path 2>/dev/null || true)"
  run_in_project "$project_root" aoc-stm resume > "$stm_content_file" 2>/dev/null || true
  if ! grep -q '[^[:space:]]' "$stm_content_file"; then
    stm_mode="current-draft"
    stm_source="$(run_in_project "$project_root" aoc-stm path current 2>/dev/null || printf '%s/.aoc/stm/current.md' "$project_root")"
    run_in_project "$project_root" aoc-stm read-current > "$stm_content_file" 2>/dev/null || true
    if ! grep -q '[^[:space:]]' "$stm_content_file"; then
      stm_mode="none"
      stm_source="(none)"
    fi
  fi

  mind_source="$(resolve_mind_file "$project_root" || true)"
  if [[ -n "$mind_source" ]]; then
    cp "$mind_source" "$mind_content_file"
  else
    mind_source="(none)"
  fi

  python3 - "$project_root" "$max_chars" "$mem_max_lines" "$stm_max_lines" "$mind_max_lines" "$mem_max_chars" "$stm_max_chars" "$mind_max_chars" "$mem_source" "$stm_source" "$stm_mode" "$mind_source" "$mem_content_file" "$stm_content_file" "$mind_content_file" <<'PY'
import sys
from pathlib import Path


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except Exception:
        return ""


def trim_lines_and_chars(text: str, max_lines: int, max_chars: int, label: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines = lines[:-1]

    line_truncated = False
    if len(lines) > max_lines:
        lines = lines[:max_lines]
        line_truncated = True

    out = "\n".join(lines)
    char_truncated = False
    if len(out) > max_chars:
        out = out[:max_chars]
        char_truncated = True

    notes = []
    if line_truncated:
        notes.append(f"[truncated {label}: line budget {max_lines}]")
    if char_truncated:
        notes.append(f"[truncated {label}: char budget {max_chars}]")
    if notes:
        if out:
            out += "\n"
        out += "\n".join(notes)

    return out


def section(name: str, source: str, mode: str, body: str) -> str:
    lines = [f"## {name}", f"source: {source}"]
    if mode:
        lines.append(f"mode: {mode}")
    lines.append("content:")
    if body.strip():
        lines.append(body)
    else:
        lines.append("(no content)")
    return "\n".join(lines)


project_root = Path(sys.argv[1])
max_chars = int(sys.argv[2])
mem_max_lines = int(sys.argv[3])
stm_max_lines = int(sys.argv[4])
mind_max_lines = int(sys.argv[5])
mem_max_chars = int(sys.argv[6])
stm_max_chars = int(sys.argv[7])
mind_max_chars = int(sys.argv[8])

mem_source = sys.argv[9]
stm_source = sys.argv[10]
stm_mode = sys.argv[11]
mind_source = sys.argv[12]

mem_text = read_text(Path(sys.argv[13]))
stm_text = read_text(Path(sys.argv[14]))
mind_text = read_text(Path(sys.argv[15]))

mem_body = trim_lines_and_chars(mem_text, mem_max_lines, mem_max_chars, "aoc-mem")
stm_body = trim_lines_and_chars(stm_text, stm_max_lines, stm_max_chars, "aoc-stm")
mind_body = trim_lines_and_chars(mind_text, mind_max_lines, mind_max_chars, "aoc-mind")

header = "\n".join(
    [
        "# AOC Context Pack",
        f"project_root: {project_root}",
        "precedence: aoc-mem -> aoc-stm -> aoc-mind",
    ]
)

parts = [
    header,
    section("aoc-mem", mem_source, "read", mem_body),
    section("aoc-stm", stm_source, stm_mode, stm_body),
    section("aoc-mind", mind_source, "summary", mind_body),
]
final = "\n\n".join(parts).rstrip() + "\n"

if len(final) > max_chars:
    marker = "\n[truncated context-pack: total char budget reached]\n"
    if max_chars <= len(marker):
        final = marker[:max_chars]
    else:
        final = final[: max_chars - len(marker)] + marker

print(final, end="")
PY
}

main "$@"
