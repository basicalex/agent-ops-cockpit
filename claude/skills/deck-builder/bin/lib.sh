# shellcheck shell=bash
# Shared resolution for deck-builder scripts. Source, do not execute.
TOOLS="${DECK_TOOLS:-$HOME/dev/tools}"
PPT_ROOT="$TOOLS/ppt-master"
PPT_SKILL="$PPT_ROOT/skills/ppt-master"
PY="$TOOLS/ppt-venv/bin/python"
SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() { echo "deck-builder: $*" >&2; exit 1; }

[ -x "$PY" ] || die "no venv at $PY; run $SKILL_DIR/setup.sh"
[ -f "$PPT_SKILL/SKILL.md" ] || die "no ppt-master at $PPT_ROOT; run $SKILL_DIR/setup.sh"

# find_project <workspace> -> echoes the single ppt-master project dir under <workspace>/project
find_project() {
  local ws=$1
  local found
  found=$(find "$ws/project" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort)
  [ -n "$found" ] || die "no ppt-master project under $ws/project (run deck-init.sh)"
  [ "$(printf '%s\n' "$found" | wc -l | tr -d ' ')" = 1 ] || die "more than one project under $ws/project; keep one"
  printf '%s\n' "$found"
}
