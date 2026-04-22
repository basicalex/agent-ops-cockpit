#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
extension="$repo_root/.pi/extensions/subagent.ts"
registry="$repo_root/.pi/extensions/subagent/registry.ts"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

python3 - "$extension" "$registry" <<'PY'
import re
import sys
from pathlib import Path

ext_src = Path(sys.argv[1]).read_text(encoding='utf-8')
reg_src = Path(sys.argv[2]).read_text(encoding='utf-8')


def block(src: str, name: str) -> str:
    pattern = re.compile(rf'(?:export\s+)?(?:async\s+)?function {re.escape(name)}\([^)]*\)[:\s\w<>,\[\]\|"\-]*\{{(.*?)\n\}}', re.S)
    m = pattern.search(src)
    if not m:
        raise SystemExit(f'missing function block: {name}')
    return m.group(1)

fetch_block = block(reg_src, 'fetchMindContextPack')
for needle in [
    'sendPulseCommand("mind_context_pack"',
    'mode: "dispatch"',
    'detail: false',
    'if (result.status !== "ok" || !result.message) return undefined;',
]:
    if needle not in fetch_block:
        raise SystemExit(f'fetchMindContextPack missing: {needle}')
if 'catch {' not in fetch_block or 'return undefined;' not in fetch_block:
    raise SystemExit('fetchMindContextPack must fail open to undefined')

approval_block = block(ext_src, 'enforceRoleApproval')
for needle in [
    'if (!role.requiresWriteApproval) return;',
    'if (!taskLooksWriteLike(task) && !taskLooksDestructive(task)) return;',
    'if (approveWrite) return;',
    'requires approveWrite=true for write/destructive requests',
]:
    if needle not in approval_block:
        raise SystemExit(f'enforceRoleApproval missing: {needle}')

session_mode_block = block(ext_src, 'assertSupportedSessionMode')
for needle in [
    'normalized === "fresh"',
    'normalized === "detached"',
    'normalized === "fresh_detached"',
    'Detached delegated subagents currently only support fresh detached session mode.',
]:
    if needle not in session_mode_block:
        raise SystemExit(f'assertSupportedSessionMode missing: {needle}')

recursion_block = block(ext_src, 'assertDelegatedRecursionAllowed')
for needle in [
    'const depth = currentSubagentNestingDepth();',
    'if (depth < MAX_SUBAGENT_NESTING_DEPTH) return;',
    'Nested delegated subagent dispatch is blocked inside detached delegated runs',
]:
    if needle not in recursion_block:
        raise SystemExit(f'assertDelegatedRecursionAllowed missing: {needle}')

guardrail_block = block(ext_src, 'assertDispatchGuardrails')
for needle in [
    'assertSupportedSessionMode(sessionMode);',
    'assertDelegatedRecursionAllowed();',
]:
    if needle not in guardrail_block:
        raise SystemExit(f'assertDispatchGuardrails missing: {needle}')

policy_block = block(ext_src, 'assertRoleToolPolicies')
for needle in [
    'assertAllowedToolPolicies(toolPolicies, role.agent);',
    'role.allowedTrustTiers.includes(policy.trustTier)',
    'references tools outside its trust policy',
]:
    if needle not in policy_block:
        raise SystemExit(f'assertRoleToolPolicies missing: {needle}')

refresh_block = block(ext_src, 'refreshRegistryJobs')
for needle in [
    'if (prior?.specialistRole && !mapped.specialistRole) mapped.specialistRole = prior.specialistRole;',
    'if (typeof prior?.writeApproved === "boolean" && typeof mapped.writeApproved !== "boolean") mapped.writeApproved = prior.writeApproved;',
    'if (typeof prior?.contextPackUsed === "boolean" && typeof mapped.contextPackUsed !== "boolean") mapped.contextPackUsed = prior.contextPackUsed;',
]:
    if needle not in refresh_block:
        raise SystemExit(f'refreshRegistryJobs missing telemetry preservation: {needle}')

snapshot_block = block(ext_src, 'snapshotJob')
if 'const { pid: _pid, ...rest } = job;' not in snapshot_block:
    raise SystemExit('snapshotJob should preserve metadata by stripping only pid')

restore_block = block(ext_src, 'restoreJobs')
for needle in [
    'restored.set(data.jobId, { ...data, executionMode: normalizeExecutionMode((data as { executionMode?: string }).executionMode) });',
    'job.status = "stale";',
    'job.error = job.error ?? "extension reloaded before detached result was observed";',
]:
    if needle not in restore_block:
        raise SystemExit(f'restoreJobs missing persisted telemetry/stale safety: {needle}')

dispatch_match = re.search(r'async function dispatchSpecialistRole\((.*?)\n\}', ext_src, re.S)
if not dispatch_match:
    raise SystemExit('missing dispatchSpecialistRole block')
dispatch_block = dispatch_match.group(0)
for needle in [
    'assertDispatchGuardrails();',
    'const contextPack = await fetchMindContextPack(role.role, `specialist dispatch: ${role.role}`);',
    'const contextPrelude = renderContextPackPrelude(contextPack);',
    'job.specialistRole = role.role;',
    'job.writeApproved = role.requiresWriteApproval ? Boolean(approveWrite) : false;',
    'job.contextPackUsed = Boolean(contextPrelude);',
    'return { role, job, contextPackUsed: Boolean(contextPrelude) };',
]:
    if needle not in dispatch_block:
        raise SystemExit(f'dispatchSpecialistRole missing: {needle}')

print('Specialist role runtime guard checks passed.')
PY
