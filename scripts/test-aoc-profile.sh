#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

state_file="$tmp/profiles.json"

assert_lines_equal() {
  local actual_file="$1"
  local expected_file="$2"
  if ! diff -u "$expected_file" "$actual_file"; then
    echo "ERROR: line output mismatch" >&2
    exit 1
  fi
}

AOC_PROFILE_STATE_FILE="$state_file" bash "$root/bin/aoc-profile" active --kind extensions --root "$root" --manifest "$root/.omp/manifest.toml" >"$tmp/default-extensions.out"
cat >"$tmp/default-extensions.expected" <<'EOF'
aoc-profile.ts
aoc-codegraph.ts
aoc-mind.ts
aoc-dox.ts
aoc-style.ts
EOF
assert_lines_equal "$tmp/default-extensions.out" "$tmp/default-extensions.expected"

if grep -Fxq 'aoc-master.ts' "$tmp/default-extensions.out" || \
   grep -Fxq 'aoc-brand-content.ts' "$tmp/default-extensions.out" || \
   grep -Fxq 'aoc-web-search.ts' "$tmp/default-extensions.out"; then
  echo "ERROR: default core extensions included gated profile assets" >&2
  exit 1
fi

AOC_OMP_PROFILES=core,hyperframes bash "$root/bin/aoc-profile" active --kind agents --root "$root" --manifest "$root/.omp/manifest.toml" >"$tmp/hyperframes-agents.out"
cat >"$tmp/hyperframes-agents.expected" <<'EOF'
brand-strategy.md
brand-concept.md
svg-asset.md
hyperframes-content.md
EOF
assert_lines_equal "$tmp/hyperframes-agents.out" "$tmp/hyperframes-agents.expected"

if AOC_OMP_PROFILES=does-not-exist bash "$root/bin/aoc-profile" active --kind extensions --root "$root" --manifest "$root/.omp/manifest.toml" >"$tmp/unknown.out" 2>"$tmp/unknown.err"; then
  echo "ERROR: unknown AOC_OMP_PROFILES unexpectedly succeeded" >&2
  exit 1
fi
if [[ -s "$tmp/unknown.out" ]]; then
  echo "ERROR: unknown profile command printed stdout" >&2
  exit 1
fi
if ! grep -Fq 'unknown profile: does-not-exist' "$tmp/unknown.err"; then
  echo "ERROR: unknown profile stderr did not explain failure" >&2
  exit 1
fi

python3 - "$root" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
profile = (root / '.omp/extensions/aoc-profile.ts').read_text(encoding='utf-8')
style = (root / '.omp/extensions/aoc-style.ts').read_text(encoding='utf-8')
checks = [
    ('aoc-profile.ts', profile, 'registerCommand("profile"'),
    ('aoc-style.ts', style, 'registerCommand("ponytail"'),
    ('aoc-style.ts', style, 'registerCommand("caveman"'),
    ('aoc-style.ts', style, 'before_agent_start'),
    ('aoc-style.ts', style, 'ponytail-workflows'),
]
for name, text, needle in checks:
    if needle not in text:
        raise SystemExit(f'ERROR: {name} missing {needle}')
for removed in (
    'registerCommand("ponytail-review"',
    'registerCommand("ponytail-audit"',
    'registerCommand("ponytail-debt"',
    'registerCommand("ponytail-help"',
):
    if removed in style:
        raise SystemExit(f'ERROR: aoc-style.ts still contains removed command registration {removed}')
PY

if node -e 'require.resolve("typescript")' >/dev/null 2>&1; then
  node - "$root" <<'NODE'
const fs = require('fs');
const path = require('path');
const ts = require('typescript');
const root = process.argv[2];
for (const rel of ['.omp/extensions/aoc-profile.ts', '.omp/extensions/aoc-style.ts']) {
  const file = path.join(root, rel);
  const source = fs.readFileSync(file, 'utf8');
  const result = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2020,
      esModuleInterop: true,
      skipLibCheck: true,
    },
    reportDiagnostics: true,
    fileName: file,
  });
  const diagnostics = result.diagnostics || [];
  const failures = diagnostics.filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error);
  if (failures.length) {
    const host = {
      getCanonicalFileName: (name) => name,
      getCurrentDirectory: () => root,
      getNewLine: () => '\n',
    };
    console.error(ts.formatDiagnosticsWithColorAndContext(failures, host));
    process.exit(1);
  }
}
NODE
else
  echo "SKIP: local typescript module unavailable; static checks still passed"
fi

printf 'AOC profile resolver/static extension checks passed\n'
