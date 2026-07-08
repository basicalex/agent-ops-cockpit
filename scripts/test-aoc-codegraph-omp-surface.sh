#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
omp_extension="$repo_root/.omp/extensions/aoc-codegraph.ts"
init="$repo_root/bin/aoc-init"
rust_fixture="$repo_root/crates/aoc-cli/src/main.rs"
agent_doc="$repo_root/docs/agents.md"
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
[[ -f "$rust_fixture" ]] || fail "Missing kept Rust source fixture: $rust_fixture"
[[ -f "$agent_doc" ]] || fail "Missing agent docs: $agent_doc"
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
assert_contains "$omp_extension" 'resolveRepoCommand(projectRoot, "bin/codegraph", "codegraph")'
assert_contains "$omp_extension" 'scopedCwd(projectRoot, params.cwd)'
assert_contains "$omp_extension" '"--path", projectRoot'
assert_contains "$omp_extension" 'Use aoc_codegraph before broad grep/read scans'
assert_not_contains "$omp_extension" '"node"'
assert_contains "$init" 'Installs AOC OMP extensions selected by active profiles'
assert_contains "$rust_fixture" 'Agent Ops Cockpit CLI'
assert_contains "$rust_fixture" 'Commands::Map'
assert_contains "$agent_doc" 'AOC includes an OMP `aoc_codegraph` tool'
assert_contains "$agent_doc" 'codegraph sync /path/to/project'

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
