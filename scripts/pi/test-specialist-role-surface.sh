#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
extension="$repo_root/.pi/extensions/subagent.ts"
shared="$repo_root/.pi/extensions/subagent/shared.ts"
registry="$repo_root/.pi/extensions/subagent/registry.ts"
artifacts="$repo_root/.pi/extensions/subagent/artifacts.ts"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq "$needle" "$file" || fail "Expected $file to contain: $needle"
}

node - <<'NODE' "$extension" "$shared" "$registry" "$artifacts"
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

for manifest in \
  "$repo_root/.pi/agents/planner-agent.md" \
  "$repo_root/.pi/agents/builder-agent.md" \
  "$repo_root/.pi/agents/documenter-agent.md" \
  "$repo_root/.pi/agents/red-team-agent.md"
do
  [[ -f "$manifest" ]] || fail "Missing manifest: $manifest"
  assert_contains "$manifest" "tools: read,bash"
done

assert_contains "$extension" 'const SPECIALIST_ROLES: Record<SpecialistRoleName, SpecialistRoleConfig>'
assert_contains "$extension" 'name: "aoc_specialist_role"'
assert_contains "$extension" 'pi.registerCommand("specialist-run"'
assert_contains "$extension" 'pi.registerCommand("specialist-roles"'
assert_contains "$registry" 'sendPulseCommand("mind_context_pack"'
assert_contains "$extension" 'approveWrite=true'
assert_contains "$extension" 'approve-write'
assert_contains "$extension" 'assertSupportedSessionMode(params.sessionMode);'
assert_contains "$extension" 'const executionMode = normalizeExecutionMode(params.executionMode);'
assert_contains "$extension" 'const lines = [feedback.notice, `role: ${dispatched.role.role}`, `execution_mode: ${feedback.job.executionMode}`];'
assert_contains "$extension" 'const approveWrite = rest.some((part) => /^approve-write$/i.test(part));'
assert_contains "$extension" 'assertRoleToolPolicies(toolPolicies, role);'
assert_contains "$extension" 'const contextPack = await fetchMindContextPack(role.role, `specialist dispatch: ${role.role}`);'
assert_contains "$extension" 'const MAX_SUBAGENT_NESTING_DEPTH = 1;'
assert_contains "$extension" 'job.specialistRole = role.role;'
assert_contains "$extension" 'job.writeApproved = role.requiresWriteApproval ? Boolean(approveWrite) : false;'
assert_contains "$extension" 'job.contextPackUsed = Boolean(contextPrelude);'
assert_contains "$artifacts" 'specialist_role: ${job.specialistRole}'
assert_contains "$artifacts" 'write_approval: ${job.writeApproved ? "approved" : "read-first"}'
assert_contains "$extension" 'details: { action: params.action, jobId: params.jobId, role: job?.specialistRole, job }'
assert_contains "$extension" 'details: { action: params.action, role: job.specialistRole, job }'

python3 - "$extension" <<'PY'
import re
import sys
from pathlib import Path

src = Path(sys.argv[1]).read_text(encoding='utf-8')
checks = {
    'scout': ('explorer-agent', False),
    'planner': ('planner-agent', False),
    'builder': ('builder-agent', True),
    'reviewer': ('code-review-agent', False),
    'documenter': ('documenter-agent', False),
    'red-team': ('red-team-agent', True),
}
for role, (agent, requires_approval) in checks.items():
    role_key = f'"{role}"' if '-' in role else role
    pattern = re.compile(rf'{re.escape(role_key)}\s*:\s*\{{(.*?)\n\t\}},', re.S)
    match = pattern.search(src)
    if not match:
        raise SystemExit(f'missing role block for {role}')
    body = match.group(1)
    if f'agent: "{agent}"' not in body:
        raise SystemExit(f'role {role} missing agent {agent}')
    expected = f'requiresWriteApproval: {str(requires_approval).lower()}'
    if expected not in body:
        raise SystemExit(f'role {role} missing {expected}')
PY

echo "Specialist role surface checks passed."
