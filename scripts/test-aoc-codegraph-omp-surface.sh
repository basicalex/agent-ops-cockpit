#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
omp_extension="$repo_root/.omp/extensions/aoc-codegraph.ts"
init="$repo_root/bin/aoc-init"
control="$repo_root/crates/aoc-control/src/main.rs"
control_doc="$repo_root/docs/control-pane.md"
agents_contract="$repo_root/AGENTS.md"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq "$needle" "$file" || fail "Expected $file to contain: $needle"
}

assert_not_contains() {
  local file="$1"
  local needle="$2"
  if grep -Fq "$needle" "$file"; then
    fail "Expected $file not to contain: $needle"
  fi
}

[[ -f "$omp_extension" ]] || fail "Missing OMP CodeGraph extension: $omp_extension"
[[ -f "$init" ]] || fail "Missing aoc-init: $init"
[[ -f "$control" ]] || fail "Missing aoc-control source: $control"
[[ -f "$control_doc" ]] || fail "Missing control pane docs: $control_doc"
[[ -f "$agents_contract" ]] || fail "Missing agent contract: $agents_contract"

if node -e "require('typescript')" >/dev/null 2>&1; then
  node - <<'NODE' "$omp_extension"
const ts=require('typescript');
const fs=require('fs');
for (const file of process.argv.slice(2)) {
  const src=fs.readFileSync(file,'utf8');
  const out=ts.transpileModule(src,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}});
  if(out.diagnostics?.length){
    console.error('FILE', file);
    console.error(ts.formatDiagnosticsWithColorAndContext(out.diagnostics,{getCurrentDirectory:()=>process.cwd(),getCanonicalFileName:f=>f,getNewLine:()=>"\n"}));
    process.exit(1);
  }
}
NODE
else
  echo "typescript module unavailable; skipping transpile and running static surface checks only."
fi

assert_contains "$omp_extension" 'name: "aoc_codegraph"'
assert_contains "$omp_extension" '"status", "files", "search", "context", "callers", "callees", "impact", "affected"'
assert_contains "$omp_extension" 'spawn("codegraph", args'
assert_contains "$omp_extension" 'cwd escapes project root'
assert_contains "$omp_extension" '"--path", projectRoot'
assert_contains "$omp_extension" 'Use aoc_codegraph before broad grep/read scans'
assert_not_contains "$omp_extension" '"node"'
assert_contains "$init" 'aoc-codegraph.ts'
assert_contains "$control" 'CodeGraph agent index'
assert_contains "$control" 'pnpm add -g @colbymchenry/codegraph'
assert_contains "$control" 'SettingsSection::ToolsCodeGraph'
assert_contains "$control_doc" 'Alt+C -> Tools -> CodeGraph agent index'
assert_contains "$control_doc" 'pnpm add -g @colbymchenry/codegraph'

if command -v codegraph >/dev/null 2>&1; then
  codegraph --help >/dev/null || fail "codegraph is on PATH but --help failed"
  codegraph files --help >/dev/null || fail "codegraph files --help failed"
  codegraph query --help >/dev/null || fail "codegraph query --help failed"
  codegraph context --help >/dev/null || fail "codegraph context --help failed"
  codegraph affected --help >/dev/null || fail "codegraph affected --help failed"
  if [[ -d "$repo_root/.codegraph" ]]; then
    codegraph status "$repo_root" >/dev/null || fail "codegraph status failed for existing index"
  fi
  echo "CodeGraph OMP tool surface checks passed (codegraph CLI present)."
else
  echo "CodeGraph OMP tool surface checks passed (codegraph CLI absent; tool fallback is expected)."
fi
