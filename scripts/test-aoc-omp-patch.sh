#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
patcher="$repo_root/bin/aoc-omp-patch"

fail() { echo "FAIL: $*" >&2; exit 1; }
assert_contains() { grep -Fq -- "$1" "$2" || fail "Expected '$1' in $2"; }
assert_not_contains() { if grep -Fq -- "$1" "$2"; then fail "Did not expect '$1' in $2"; fi; }

write_cli_fixture() {
  local file="$1"
  mkdir -p "$(dirname "$file")"
  cat > "$file" <<'EOF'
import*as fs1 from"fs";import*as path1 from"path";import{stripVTControlCharacters as stripVTControlCharacters2}from"util";class FooterComponent{#cachedBranch;#onBranchChange;#getCurrentBranch(){if(this.#cachedBranch!==void 0)return this.#cachedBranch;let headState=head.resolveSync(getProjectDir());return this.#cachedBranch=headState===null?null:headState.kind==="ref"?headState.branchName??headState.ref:"detached",this.#cachedBranch}#shouldLookupPr(branch){return branch === "detached" || this.#isDefaultBranch(branch) || this.#prLookupInFlight}}
EOF
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-omp-patch-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT
export HOME="$tmp_root/home"
mkdir -p "$HOME/.cache/.bun/bin" "$HOME/.cache/.bun/install/global/node_modules/@oh-my-pi/pi-coding-agent/dist"

cache="$tmp_root/cache"
cli="$cache/@oh-my-pi/pi-coding-agent@15.10.8@@@1/dist/cli.js"
write_cli_fixture "$cli"

"$patcher" --cache-root "$cache"
assert_contains 'function aocFindJjRepoDirAsync' "$cli"
assert_contains 'aocResolveJjSummaryAsync' "$cli"
assert_contains 'jj?' "$cli"
assert_contains 'aocCountAddedRemovedAsync' "$cli"
assert_not_contains 'spawnSync("jj"' "$cli"

cp "$cli" "$tmp_root/patched.once"
"$patcher" --cache-root "$cache" --quiet
cmp -s "$cli" "$tmp_root/patched.once" || fail "Expected second patch run to be byte-identical"

echo "PASS: aoc-omp-patch"
