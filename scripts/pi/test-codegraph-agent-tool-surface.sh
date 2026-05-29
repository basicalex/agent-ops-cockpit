#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
extension="$repo_root/.pi/extensions/aoc-codegraph.ts"
settings="$repo_root/.pi/settings.json"
explorer="$repo_root/.pi/agents/explorer-agent.md"
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

[[ -f "$extension" ]] || fail "Missing CodeGraph extension: $extension"
[[ -f "$settings" ]] || fail "Missing Pi settings: $settings"
[[ -f "$explorer" ]] || fail "Missing explorer agent manifest: $explorer"
[[ -f "$init" ]] || fail "Missing aoc-init: $init"
[[ -f "$control" ]] || fail "Missing aoc-control source: $control"
[[ -f "$control_doc" ]] || fail "Missing control pane docs: $control_doc"
[[ -f "$agents_contract" ]] || fail "Missing agent contract: $agents_contract"

if node -e "require('typescript')" >/dev/null 2>&1; then
  node - <<'NODE' "$extension"
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

assert_contains "$extension" 'name: "aoc_codegraph"'
assert_contains "$extension" '"status", "files", "search", "context", "node", "callers", "callees", "impact", "affected"'
assert_contains "$extension" 'spawn("codegraph", args'
assert_contains "$extension" 'Alt+C -> Tools -> CodeGraph agent index'
assert_contains "$extension" 'Use aoc_codegraph before broad grep/read scans'
assert_contains "$extension" 'cwd escapes project root'

assert_contains "$settings" '"extensions/aoc-codegraph.ts"'
assert_contains "$init" 'aoc-codegraph.ts'
assert_contains "$explorer" 'tools: read,bash,aoc_codegraph'
assert_contains "$explorer" 'When `aoc_codegraph` is available and `.codegraph/` exists'
assert_contains "$control" 'CodeGraph agent index'
assert_contains "$control" 'pnpm add -g @colbymchenry/codegraph'
assert_contains "$control" 'SettingsSection::ToolsCodeGraph'
assert_contains "$control_doc" 'Alt+C -> Tools -> CodeGraph agent index'
assert_contains "$control_doc" 'pnpm add -g @colbymchenry/codegraph'
assert_contains "$agents_contract" 'aoc_codegraph` is a default read-only main-agent development tool'
assert_contains "$agents_contract" 'Use `aoc_codegraph` before broad grep/read scans'

if command -v codegraph >/dev/null 2>&1; then
  codegraph --help >/dev/null 2>&1 || fail "codegraph is on PATH but --help failed"
  echo "CodeGraph agent tool surface checks passed (codegraph CLI present)."
else
  echo "CodeGraph agent tool surface checks passed (codegraph CLI absent; tool fallback is expected)."
fi
