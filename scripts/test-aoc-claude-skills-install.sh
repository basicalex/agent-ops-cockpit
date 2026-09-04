#!/usr/bin/env bash
set -euo pipefail

# aoc-claude-install must copy every seeded Claude skill recursively (bin/,
# templates/, reference/ beside SKILL.md), keep executable bits, and leave
# node_modules and installer backups behind.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

src="$tmp/src"
mkdir -p "$src/claude/skills/nested-skill/bin" "$src/claude/skills/nested-skill/templates/sub" \
  "$src/claude/skills/nested-skill/node_modules/pkg" "$src/config/codex"
printf -- '---\nname: nested-skill\n---\n' >"$src/claude/skills/nested-skill/SKILL.md"
printf '#!/usr/bin/env bash\necho ok\n' >"$src/claude/skills/nested-skill/bin/run.sh"
chmod +x "$src/claude/skills/nested-skill/bin/run.sh"
printf 'template\n' >"$src/claude/skills/nested-skill/templates/sub/page.svg"
printf 'dep\n' >"$src/claude/skills/nested-skill/node_modules/pkg/index.js"
printf 'stale\n' >"$src/claude/skills/nested-skill/SKILL.md.bak.20260101000000"
printf 'global\n' >"$src/claude/CLAUDE.global.md"
printf 'agents\n' >"$src/claude/AGENTS.global.md"
printf '[codex]\n' >"$src/config/codex/config.toml"

claude_dir="$tmp/claude"
mkdir -p "$tmp/home"
run_install() {
  AOC_SOURCE_ROOT="$src" AOC_CLAUDE_DIR="$claude_dir" AOC_CODEX_DIR="$tmp/codex" \
    XDG_CONFIG_HOME="$tmp/xdg" HOME="$tmp/home" "$root/bin/aoc-claude-install" >"$tmp/install.log" 2>&1 \
    || { cat "$tmp/install.log" >&2; exit 1; }
}
run_install

dst="$claude_dir/skills/nested-skill"
[[ -f "$dst/SKILL.md" ]]
[[ -f "$dst/bin/run.sh" && -x "$dst/bin/run.sh" ]]
[[ -f "$dst/templates/sub/page.svg" ]]
[[ ! -e "$dst/node_modules" ]]
[[ ! -e "$dst/SKILL.md.bak.20260101000000" ]]

# Re-running with unchanged sources creates no backups.
run_install
[[ -z "$(find "$dst" -name '*.bak.*')" ]]

# The real deck-builder seed carries its runnable parts.
for f in SKILL.md setup.sh package.json bin/deck-init.sh bin/deck-gate.sh bin/deck-export.sh \
  bin/render-svg.mjs bin/build-pdf.py templates/page-content.svg templates/page-cover.svg \
  templates/content.md templates/BRIEF.md templates/DECK-PACKET.md reference/gotchas.md reference/svg-authoring.md; do
  [[ -f "$root/claude/skills/deck-builder/$f" ]] || { echo "missing deck-builder seed file: $f" >&2; exit 1; }
done
[[ ! -e "$root/claude/skills/deck-builder/node_modules" ]]

echo "test-aoc-claude-skills-install: ok"
