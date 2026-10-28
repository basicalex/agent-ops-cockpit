#!/usr/bin/env bash
set -euo pipefail

# A project AGENTS.md with an old "## Agent Ops Cockpit (AOC)" section and no
# markers: the section becomes the managed block, project lines stay, and
# lines about retired tools go.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" XDG_CONFIG_HOME="$tmp/config"
export AOC_OMP_AGENT_DIR="$tmp/omp-agent" AOC_CLAUDE_INSTALL_OFFLINE=1
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$AOC_OMP_AGENT_DIR"

fail() { echo "ERROR: $*" >&2; exit 1; }
start='<!-- aoc:managed:start -->'
end='<!-- aoc:managed:end -->'
specs_line='- `docs/specs/`: spec documents (moved from `.taskmaster/docs` on 2026-10-26).'

project="$tmp/project"
mkdir -p "$project"
agents="$project/AGENTS.md"
cat > "$agents" <<'MD'
# Repository Guidelines

## Project Structure
This is a Bun workspace monorepo.
- `apps/web`: Next.js App Router frontend.

## Testing Guidelines
- `packages/shared` uses Vitest (`vitest run`).

## Agent Ops Cockpit (AOC)
Use the AOC tools for context and memory in this repo.

- **Context:** `.aoc/context.md` (auto-generated project map)
- **Design:** `DESIGN.md` at the project root is the visual/product design contract. Read it before UI, docs-site, marketing, HyperFrames, or other product-facing changes.
- **Memory:** use `aoc-mem read/search/add` via Bash tool (do not edit `.aoc/memory.md` directly)
- **STM directed handoff layer:** use `aoc-stm status` for deliberate in-progress handoffs only.
- `docs/specs/`: spec documents (moved from `.taskmaster/docs` on 2026-10-26).
- **RTK routing (default-on for new AOC repos):** manage with `aoc-rtk status` and `.aoc/rtk.toml`
- **Startup handshake:** use `aoc-handshake --json` for AOC/Mind/VCS availability. Prefer `jj` when the handshake reports Jujutsu.
- **Tasks:** `aoc task list` or `tm next` from Taskmaster.
- **Layouts:** zellij layouts live in `.aoc/layouts`.
- **Search:** codegraph first, then SearXNG.
- **Deploy:** run `bun run deploy:preview` before a release.

## Lightweight Validation
- Prefer OMP `lsp diagnostics` on touched files/globs for edit-loop validation before running build, lint, or typecheck commands.
- Do not run full project build/lint/test as a routine sanity check during active edits.

If AOC files are missing or stale, run `aoc-init` at the repo root.

## Configuration Notes
- Web app requires `NEXT_PUBLIC_CONVEX_URL`.
MD

run_init() {
  local log="$1"
  AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$project" > "$log" 2>&1 || { cat "$log" >&2; fail "aoc-init failed"; }
}

run_init "$tmp/first.log"
grep -Fq 'Replaced the old AOC section in AGENTS.md with a managed block.' "$tmp/first.log" || fail 'old AOC section not converted'
grep -Fq "kept project line: $specs_line" "$tmp/first.log" || fail 'kept section line not reported'
cp "$agents" "$tmp/after-first.md"

[[ "$(head -n 1 "$agents")" == "$start" ]] || fail 'AGENTS.md does not start with the managed block'
[[ "$(grep -Fxc "$start" "$agents")" == 1 && "$(grep -Fxc "$end" "$agents")" == 1 ]] || fail 'managed markers are not unique'
grep -Fxq '## Agent Ops Cockpit (AOC)' "$agents" && fail 'old AOC heading survived'
[[ "$(grep -Fxc '## Lightweight Validation' "$agents")" == 1 ]] || fail 'template section duplicated outside the block'
[[ "$(grep -Fxc '## Project rules' "$agents")" == 1 ]] || fail 'missing a single Project rules section'

# Project lines outside the old section, and project lines inside it, kept once and in order.
project_lines=(
  '# Repository Guidelines'
  '## Project Structure'
  'This is a Bun workspace monorepo.'
  '- `apps/web`: Next.js App Router frontend.'
  '## Testing Guidelines'
  '- `packages/shared` uses Vitest (`vitest run`).'
  '## Project rules'
  "$specs_line"
  '- **Deploy:** run `bun run deploy:preview` before a release.'
  '## Configuration Notes'
  '- Web app requires `NEXT_PUBLIC_CONVEX_URL`.'
)
python3 - "$agents" "$end" "${project_lines[@]}" <<'PY'
import sys
lines = open(sys.argv[1], encoding="utf-8").read().splitlines()
end = lines.index(sys.argv[2])
wanted = sys.argv[3:]
for line in wanted:
    if lines.count(line) != 1:
        raise SystemExit(f"ERROR: project line not kept exactly once: {line}")
positions = [lines.index(line) for line in wanted]
if not all(pos > end for pos in positions):
    raise SystemExit("ERROR: project lines are inside the managed block")
if positions != sorted(positions):
    raise SystemExit("ERROR: project lines changed order")
outside = lines[end + 1:]
for bad in ("aoc-mem", "aoc-stm", "aoc-rtk", "RTK", "Mind", "jj", "Jujutsu", "zellij", "codegraph",
            "Taskmaster", "tm next", "aoc task", "SearXNG", "HyperFrames", "Use the AOC tools",
            "If AOC files are missing"):
    hits = [line for line in outside if bad in line]
    if hits:
        raise SystemExit(f"ERROR: retired or template line survived ({bad}): {hits[0]}")
PY

# Second run: nothing changes.
run_init "$tmp/second.log"
grep -Fq 'AGENTS.md managed block already up to date.' "$tmp/second.log" || fail 'second run rewrote the managed block'
cmp -s "$tmp/after-first.md" "$agents" || fail 'second run changed AGENTS.md'

# The block matches what a fresh project gets.
fresh="$tmp/fresh"
mkdir -p "$fresh"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$fresh" > "$tmp/fresh.log" 2>&1 || { cat "$tmp/fresh.log" >&2; fail 'fresh aoc-init failed'; }
block() { sed -n "\\|^$start\$|,\\|^$end\$|p" "$1"; }
[[ "$(block "$agents")" == "$(block "$fresh/AGENTS.md")" ]] || fail 'converted block differs from the fresh block'

# A project AGENTS.md with no AOC section gets the block once, content intact.
plain="$tmp/plain"
mkdir -p "$plain"
printf '# Plain project\n\n## Build\n- `make`\n' > "$plain/AGENTS.md"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$plain" > "$tmp/plain1.log" 2>&1 || { cat "$tmp/plain1.log" >&2; fail 'plain aoc-init failed'; }
cp "$plain/AGENTS.md" "$tmp/plain-first.md"
AOC_INIT_SKIP_BUILD=1 bash "$root/bin/aoc-init" "$plain" > "$tmp/plain2.log" 2>&1 || { cat "$tmp/plain2.log" >&2; fail 'plain aoc-init failed'; }
cmp -s "$tmp/plain-first.md" "$plain/AGENTS.md" || fail 'second run changed the plain AGENTS.md'
grep -Fxq '## Agent Ops Cockpit (AOC)' "$plain/AGENTS.md" && fail 'old AOC section appended to plain AGENTS.md'
[[ "$(sed -n "\\|^$end\$|,\$p" "$plain/AGENTS.md")" == "$end"$'\n\n# Plain project\n\n## Build\n- `make`' ]] ||
  fail 'plain project content changed'

printf 'PASS: old AOC section converted to a managed block; project lines kept, retired lines dropped\n'
