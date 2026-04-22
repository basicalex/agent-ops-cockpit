#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

ingest="$repo_root/.pi/extensions/mind-ingest.ts"
ops="$repo_root/.pi/extensions/mind-ops.ts"
ctx_ext="$repo_root/.pi/extensions/mind-context.ts"
focus="$repo_root/.pi/extensions/mind-focus.ts"
lib="$repo_root/.pi/extensions/lib/mind.ts"
wrap="$repo_root/bin/aoc-agent-wrap"
docs_agents="$repo_root/docs/agents.md"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq -- "$needle" "$file" || fail "Expected $file to contain: $needle"
}

assert_exists() {
  local file="$1"
  [[ -e "$file" ]] || fail "Expected path to exist: $file"
}

assert_exists "$ingest"
assert_exists "$ops"
assert_exists "$ctx_ext"
assert_exists "$focus"
assert_exists "$lib"

assert_contains "$ingest" 'pi.on("message_end", async (event, ctx) => {'
assert_contains "$ingest" 'pi.on("session_compact", async (event, ctx) => {'
assert_contains "$ops" 'pi.registerCommand("mind-status", {'
assert_contains "$ops" 'pi.registerCommand("aoc-status", {'
assert_contains "$ops" 'pi.registerCommand("mind-finalize", {'
assert_contains "$ctx_ext" 'pi.registerCommand("mind-pack", {'
assert_contains "$ctx_ext" 'pi.registerCommand("mind-pack-expanded", {'
assert_contains "$focus" 'pi.registerCommand("mind-focus", {'
assert_contains "$lib" 'attrs: Record<string, unknown> = {'
assert_contains "$lib" 'file_paths: focus.filePaths'
assert_contains "$lib" 'task_ids: focus.taskIds'
assert_contains "$lib" 'export function resolveMindStorePath(ctx?: ExtensionContext): string {'
assert_contains "$wrap" 'export AOC_MIND_STORE_PATH="$project_root/.aoc/mind/project.sqlite"'
assert_contains "$docs_agents" '- `mind-ingest.ts` — native Pi→Mind ingest + compaction checkpoints'

echo "Mind native extension checks passed."
