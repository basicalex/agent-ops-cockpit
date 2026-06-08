#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
patcher="$repo_root/bin/aoc-omp-patch"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local needle="$1"
  local file="$2"
  grep -Fq -- "$needle" "$file" || fail "Expected '$needle' in $file"
}

assert_not_contains() {
  local needle="$1"
  local file="$2"
  if grep -Fq -- "$needle" "$file"; then
    fail "Did not expect '$needle' in $file"
  fi
}

write_footer_fixture() {
  local footer="$1"
  mkdir -p "$(dirname "$footer")"
  cat > "$footer" <<'EOF'
import * as fs from "node:fs";
import React from "react";
import { Text } from "ink";
import { getProjectDir } from "../../utils/project";
import * as git from "../../utils/git";

export class FooterComponent extends React.Component {
  #cachedBranch: string | null | undefined = undefined;
  #gitWatcher: fs.FSWatcher | null = null;
  #onBranchChange: (() => void) | null = null;

  invalidate(): void {
    this.#cachedBranch = undefined;
  }

  #getCurrentBranch(): string | null {
    if (this.#cachedBranch !== undefined) {
      return this.#cachedBranch;
    }

    const head = git.head.resolveSync(getProjectDir());
    if (!head) {
      this.#cachedBranch = null;
    } else if (head.name) {
      this.#cachedBranch = head.name;
    } else {
      this.#cachedBranch = "detached";
    }

    return this.#cachedBranch;
  }

  render() {
    let pwd = getProjectDir();
    const branch = this.#getCurrentBranch();
    if (branch) {
      pwd = `${pwd} (${branch})`;
    }
    return <Text>{pwd}</Text>;
  }
}
EOF
}

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/aoc-omp-patch-test.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

export HOME="$tmp_root/home"
export XDG_CONFIG_HOME="$tmp_root/config"
mkdir -p "$HOME/.omp/agent" "$XDG_CONFIG_HOME"
cat > "$HOME/.omp/agent/config.yml" <<'EOF'
lastChangelogVersion: 15.10.8
EOF

cache="$tmp_root/cache"
footer="$cache/@oh-my-pi/pi-coding-agent@15.10.8@@@1/src/modes/components/footer.ts"
write_footer_fixture "$footer"

"$patcher" --cache-root "$cache" --omp-dir "$HOME/.omp/agent"
assert_contains 'AOC-JJ-FOOTER-PATCH-BEGIN' "$footer"
assert_contains '#getCurrentVcsSummary' "$footer"
assert_contains 'spawnSync("jj"' "$footer"
assert_contains 'jj?' "$footer"
assert_contains 'Δ${files} +${added} -${removed} ⇢${bookmarks}' "$footer"
assert_not_contains '#cachedBranch' "$footer"

cp "$footer" "$tmp_root/patched.once"
"$patcher" --cache-root "$cache" --omp-dir "$HOME/.omp/agent" --quiet
cmp -s "$footer" "$tmp_root/patched.once" || fail "Expected second patch run to be byte-identical"

bad_footer="$cache/@oh-my-pi/pi-coding-agent@15.10.9@@@1/src/modes/components/footer.ts"
write_footer_fixture "$bad_footer"
python3 - "$bad_footer" <<'PY'
from pathlib import Path
import sys
path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
text = text.replace('    const branch = this.#getCurrentBranch();\n    if (branch) {\n      pwd = `${pwd} (${branch})`;\n    }\n', '')
path.write_text(text, encoding="utf-8")
PY
cat > "$HOME/.omp/agent/config.yml" <<'EOF'
lastChangelogVersion: 15.10.9
EOF
cp "$bad_footer" "$tmp_root/bad.before"
if "$patcher" --cache-root "$cache" --omp-dir "$HOME/.omp/agent" --quiet 2>"$tmp_root/bad.err"; then
  fail "Expected unsupported layout to fail"
fi
assert_contains 'aoc-omp-patch: unsupported OMP package layout; footer anchors not found' "$tmp_root/bad.err"
cmp -s "$bad_footer" "$tmp_root/bad.before" || fail "Expected unsupported footer to remain byte-identical"

assert_contains 'aoc-omp-patch --quiet' "$repo_root/bin/aoc-omp"

echo "PASS: aoc-omp-patch"
