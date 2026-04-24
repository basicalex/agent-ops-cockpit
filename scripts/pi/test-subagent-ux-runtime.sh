#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
extension="$repo_root/.pi/extensions/subagent.ts"
artifacts="$repo_root/.pi/extensions/subagent/artifacts.ts"
manifests="$repo_root/.pi/extensions/subagent/manifests.ts"
registry="$repo_root/.pi/extensions/subagent/registry.ts"
shared="$repo_root/.pi/extensions/subagent/shared.ts"
mission="$repo_root/crates/aoc-mission-control/src/main.rs"
mission_app="$repo_root/crates/aoc-mission-control/src/app.rs"
mission_config="$repo_root/crates/aoc-mission-control/src/config.rs"
mission_tests="$repo_root/crates/aoc-mission-control/src/tests.rs"

python3 - "$extension" "$artifacts" "$manifests" "$registry" "$shared" "$mission" "$mission_app" "$mission_config" "$mission_tests" <<'PY'
import re
import sys
from pathlib import Path

extension_src = Path(sys.argv[1]).read_text(encoding='utf-8')
artifacts_src = Path(sys.argv[2]).read_text(encoding='utf-8')
manifests_src = Path(sys.argv[3]).read_text(encoding='utf-8')
registry_src = Path(sys.argv[4]).read_text(encoding='utf-8')
shared_src = Path(sys.argv[5]).read_text(encoding='utf-8')
mission_src = Path(sys.argv[6]).read_text(encoding='utf-8')
mission_app_src = Path(sys.argv[7]).read_text(encoding='utf-8')
mission_config_src = Path(sys.argv[8]).read_text(encoding='utf-8')
mission_tests_src = Path(sys.argv[9]).read_text(encoding='utf-8')
mission_contract_src = "\n".join([mission_src, mission_app_src, mission_config_src, mission_tests_src])


def block(src: str, name: str) -> str:
    pattern = re.compile(rf'(?:export\s+)?(?:async\s+)?function {re.escape(name)}\([^)]*\)[^{{]*\{{(.*?)\n\}}', re.S)
    m = pattern.search(src)
    if not m:
        raise SystemExit(f'missing function block: {name}')
    return m.group(1)

artifact_block = block(artifacts_src, 'persistArtifactBundle')
for needle in [
    'const enriched = withArtifactRefs(root, job);',
    'ensureArtifactDir(root, enriched);',
    'writeArtifactFile(root, enriched.promptPath, renderPromptArtifact(enriched, options.prompt, options.agent));',
    'appendArtifactFile(root, enriched.eventsPath, options.appendEvent.endsWith("\\n") ? options.appendEvent : `${options.appendEvent}\\n`);',
    'appendArtifactFile(root, enriched.stderrPath, options.appendStderr);',
    'writeArtifactFile(root, enriched.reportPath, renderReportArtifact(enriched, options?.fullOutput, helpers.summarizeToolPolicies));',
    'writeArtifactFile(root, enriched.metaPath, JSON.stringify({',
    'fail open: artifact persistence should not break detached execution or recovery',
]:
    if needle not in artifact_block:
        raise SystemExit(f'persistArtifactBundle missing: {needle}')
for needle in [
    'if (job.stepResults?.length) {',
    '## Step Results',
]:
    if needle not in artifacts_src:
        raise SystemExit(f'artifact report step-results support missing: {needle}')

manifest_block = block(manifests_src, 'loadManifestBundle')
if 'export function agentAvailability(root: string, agent: AgentConfig): AgentAvailability {' not in manifests_src:
    raise SystemExit('agentAvailability must be exported for subagent extension runtime use')

for needle in [
    'const key = manifestCacheKey(root);',
    'const cached = manifestCache.get(root);',
    'if (cached && cached.key === key) return cached.bundle;',
    'manifestCache.set(root, { key, bundle });',
]:
    if needle not in manifest_block:
        raise SystemExit(f'loadManifestBundle caching missing: {needle}')

refresh_block = block(extension_src, 'refreshRegistryJobs')
for needle in [
    'if (prior?.task && !mapped.task) mapped.task = prior.task;',
    'if (prior?.cwd && mapped.cwd === root) mapped.cwd = prior.cwd;',
    'if (prior?.model && !mapped.model) mapped.model = prior.model;',
    'if (prior?.executionMode) mapped.executionMode = prior.executionMode;',
    'if (prior?.artifactDir && !mapped.artifactDir) mapped.artifactDir = prior.artifactDir;',
    'if (prior?.stepResults?.length && !mapped.stepResults?.length) mapped.stepResults = prior.stepResults;',
    'const enriched = persistArtifactBundle(root, job);',
    'return true;',
    'return false;',
]:
    if needle not in refresh_block:
        raise SystemExit(f'refreshRegistryJobs missing: {needle}')

spawn_block = block(extension_src, 'spawnDetachedStep')
for needle in [
    'if (agent.model) args.push("--model", agent.model);',
    'const enriched = persistArtifactBundle(root, currentBeforeSpawn, { prompt: task, agent });',
    'AOC_SUBAGENT_PARENT_JOB_ID: currentDelegatedParentJobId() ?? "",',
    'AOC_SUBAGENT_DEPTH: String(currentSubagentNestingDepth() + 1),',
    'const enriched = persistArtifactBundle(root, current, { appendEvent: line });',
    'const enriched = persistArtifactBundle(root, current, { appendStderr: text });',
    'const enriched = persistArtifactBundle(root, updated, { fullOutput: latestAssistantText || undefined });',
]:
    if needle not in spawn_block:
        raise SystemExit(f'spawnDetachedStep missing: {needle}')

dispatch_registry = re.search(r'async function startDetachedDispatchViaRegistry\((.*?)\n\}', extension_src, re.S)
if not dispatch_registry:
    raise SystemExit('missing startDetachedDispatchViaRegistry block')
if 'step_results?: Array<' not in registry_src:
    raise SystemExit('registry step_results contract missing')
if 'stepResults: job.step_results?.map((step) => ({' not in registry_src:
    raise SystemExit('registry durable step_results mapping missing')
for needle in [
    'job.task = task;',
    'job.cwd = cwd;',
    'job = persistArtifactBundle(root, job, { prompt: task, agent });',
    'job = persistArtifactBundle(root, job, { prompt: task });',
]:
    if needle not in dispatch_registry.group(0):
        raise SystemExit(f'startDetachedDispatchViaRegistry missing: {needle}')

team_fallback = re.search(r'function startDetachedTeam\((.*?)\n\}', extension_src, re.S)
if not team_fallback:
    raise SystemExit('missing startDetachedTeam block')
for needle in [
    'stepResults: [],',
    'stepResults: settled.map((entry) => ({',
    'stepResults: state.jobs.get(jobId)?.stepResults ?? [],',
]:
    if needle not in team_fallback.group(0):
        raise SystemExit(f'startDetachedTeam missing: {needle}')

for helper_name, fallback in [('launchAgentJob', 'startDetachedDispatch'), ('launchTeamJob', 'startDetachedTeam'), ('launchChainJob', 'startDetachedChain')]:
    helper_block = re.search(rf'async function {helper_name}\((.*?)\n\}}', extension_src, re.S)
    if not helper_block:
        raise SystemExit(f'missing {helper_name} block')
    body = helper_block.group(0)
    for needle in [
        'assertDispatchGuardrails();',
        'const manifestBundle = bundle ?? loadManifestBundle(root);',
        'catch(() => undefined)',
        fallback,
    ]:
        if needle not in body:
            raise SystemExit(f'{helper_name} missing: {needle}')

role_block = re.search(r'async function dispatchSpecialistRole\((.*?)\n\}', extension_src, re.S)
if not role_block:
    raise SystemExit('missing dispatchSpecialistRole block')
for needle in [
    'const contextPack = await fetchMindContextPack(role.role, `specialist dispatch: ${role.role}`);',
    'const contextPrelude = renderContextPackPrelude(contextPack);',
    'let job = await launchAgentJob(pi, ctx, role.agent, preface, cwdArg, executionMode, bundle);',
    'const rootWithArtifacts = ctx.cwd ?? process.cwd();',
    'job = persistArtifactBundle(rootWithArtifacts, job, { prompt: preface, agent });',
]:
    if needle not in role_block.group(0):
        raise SystemExit(f'dispatchSpecialistRole missing: {needle}')

background_refresh = re.search(r'function refreshRegistryJobsInBackground\((.*?)\n\}', extension_src, re.S)
if not background_refresh:
    raise SystemExit('missing refreshRegistryJobsInBackground block')
for needle in [
    'const availability = detachedRegistryAvailability();',
    'setInspectorRefreshState(true, "refreshing detached status…");',
    'void refreshRegistryJobs(ctx, undefined, pi)',
    'setInspectorRefreshState(false, ok ? undefined : "detached registry unavailable; showing cached jobs");',
]:
    if needle not in background_refresh.group(0):
        raise SystemExit(f'refreshRegistryJobsInBackground missing: {needle}')

clarify_block = re.search(r'async function showClarifyLaunchDialog\((.*?)\n\}', extension_src, re.S)
if not clarify_block:
    raise SystemExit('missing showClarifyLaunchDialog block')
for needle in [
    'const requiresApproval = request.kind === "role" && request.role.requiresWriteApproval;',
    'const contextStatus = request.kind === "role" ? formatContextPackStatus(request.contextPack) : undefined;',
    'const initialExecutionMode = request.initialExecutionMode ?? "background";',
    'let executionMode = initialExecutionMode;',
    'let approveWrite = request.kind === "role" ? Boolean(request.initialApproveWrite) : false;',
    'if (activeField() === "mode") {',
    'executionMode = nextExecutionMode(executionMode);',
    'approveWrite = !approveWrite;',
    'done({ task, cwdArg: cwdText.trim() || undefined, executionMode, approveWrite: requiresApproval ? approveWrite : undefined });',
]:
    if needle not in clarify_block.group(0):
        raise SystemExit(f'showClarifyLaunchDialog missing: {needle}')

manager_block = re.search(r'class SubagentInspector \{(.*?)\n\}', extension_src, re.S)
if not manager_block:
    raise SystemExit('missing SubagentInspector block')
open_manager = re.search(r'async function openSubagentManager\((.*?)\n\}', extension_src, re.S)
if not open_manager:
    raise SystemExit('missing openSubagentManager block')
for needle in [
    'await ensureInitialized(pi, ctx);',
    'if (!state.inspectorOpen) refreshRegistryJobsInBackground(pi, ctx);',
    'await showSubagentInspector(pi, ctx);',
]:
    if needle not in open_manager.group(0):
        raise SystemExit(f'openSubagentManager missing: {needle}')

dispatch_clarified = re.search(r'async function dispatchClarifiedLaunch\((.*?)\n\}', extension_src, re.S)
if not dispatch_clarified:
    raise SystemExit('missing dispatchClarifiedLaunch block')
for needle in [
    'const clarified = await showClarifyLaunchDialog(ctx, request);',
    'const dispatched = await dispatchSpecialistRole(pi, ctx, request.role.role, clarified.task, clarified.cwdArg, clarified.executionMode, clarified.approveWrite);',
    'const job = await launchTeamJob(pi, ctx, request.teamName, clarified.task, clarified.cwdArg, clarified.executionMode, bundle);',
    'const job = await launchChainJob(pi, ctx, request.chainName, clarified.task, clarified.cwdArg, clarified.executionMode, bundle);',
    'const job = await launchAgentJob(pi, ctx, request.agent.name, clarified.task, clarified.cwdArg, clarified.executionMode, bundle);',
    'const feedback = await resolveLaunchFeedback(pi, ctx, job, `Detached subagent queued: ${job.jobId}`);',
]:
    if needle not in dispatch_clarified.group(0):
        raise SystemExit(f'dispatchClarifiedLaunch missing: {needle}')

cancel_block = re.search(r'async function cancelJob\((.*?)\n\}', extension_src, re.S)
if not cancel_block:
    raise SystemExit('missing cancelJob block')
for needle in [
    'if (!job) {',
    'await refreshRegistryJobs(ctx, jobId, pi);',
    'job = state.jobs.get(jobId) ?? state.registryJobs.get(jobId);',
]:
    if needle not in cancel_block.group(0):
        raise SystemExit(f'cancelJob missing: {needle}')

for needle in [
    'private sectionIndex = 0;',
    'private selected: Record<ManagerSection, number> = {',
    'return MANAGER_SECTIONS[((this.sectionIndex % MANAGER_SECTIONS.length) + MANAGER_SECTIONS.length) % MANAGER_SECTIONS.length]!;',
    'if (section === "recent" && data === "r") {',
    'void this.rerunSelectedJob();',
    'if (section === "recent" && data === "c") {',
    'void this.cancelSelectedJob();',
    'if (section === "recent" && data === "f") {',
    'this.selectLatestFailure();',
    'if (section === "teams" && data === "r") {',
    'this.rerunSelectedTeam();',
    'if (section === "chains" && data === "r") {',
    'this.rerunSelectedChain();',
    'history: ${terminalCount} terminal   attention: ${failures.length}',
    'detail: ${truncateToWidth(`/subagent-team-detail ${name}`',
    'detail: ${truncateToWidth(`/subagent-chain-detail ${name}`',
    'Enter/i inspect • h handoff • r rerun via clarify • f latest failure • c cancel',
    'Enter opens clarify-before-run • r reruns latest team via clarify • l shows the raw command',
    'Enter opens clarify-before-run • r reruns latest chain via clarify • l shows the raw command',
    'this.openClarifyFlow({ kind: "agent", agent: current });',
    'this.openClarifyFlow({ kind: "team", teamName: current[0], members: current[1] });',
    'this.openClarifyFlow({ kind: "chain", chainName: current[0], chain: current[1] });',
    'this.openClarifyFlow({ kind: "role", role: current });',
]:
    if needle not in manager_block.group(0):
        raise SystemExit(f'SubagentInspector missing: {needle}')

for needle in [
    'export type JobStepResult = {',
    'export type ExecutionMode = "background" | "inline_wait" | "inline_summary";',
    'export function normalizeExecutionMode(value: string | undefined): ExecutionMode {',
    'export const INLINE_WAIT_TIMEOUT_MS = 45_000;',
]:
    if needle not in shared_src:
        raise SystemExit(f'shared execution-mode integration missing: {needle}')

for needle in [
    'async function waitForTerminalJob(pi: ExtensionAPI, ctx: ExtensionContext, jobId: string, timeoutMs = INLINE_WAIT_TIMEOUT_MS): Promise<JobRecord | undefined> {',
    'function summarizeStepResults(job: JobRecord): string | undefined {',
    'function formatStepResultLines(job: JobRecord, prefix = "  ", limit = 6): string[] {',
    'function needsAttentionStatus(status: JobStatus): boolean {',
    'const MAX_SUBAGENT_NESTING_DEPTH = 1;',
    'function assertSupportedSessionMode(sessionMode: string | undefined): void {',
    'function assertNoUnsupportedSessionModeFlags(raw: string): void {',
    'function assertDelegatedRecursionAllowed(): void {',
    'function assertDispatchGuardrails(sessionMode?: string): void {',
    'function formatFailureJobs(limit = 5): string {',
    'function recentJobsForTeam(teamName: string, limit = 3): JobRecord[] {',
    'function formatTeamDetail(root: string, name: string, members: string[]): string {',
    'const stepSummary = summarizeStepResults(job);',
    'if (stepSummary) lines.push(`  ${stepSummary}`);',
    'function recentJobsForChain(chainName: string, limit = 3): JobRecord[] {',
    'function formatChainDetail(root: string, name: string, chain: ChainDefinition): string {',
    'function launchRequestFromJob(root: string, job: JobRecord): LaunchDialogRequest | undefined {',
    'async function rerunJob(pi: ExtensionAPI, ctx: ExtensionContext, jobId: string, mode: "clarify" | "as_is" = "clarify"): Promise<JobRecord | undefined> {',
    'const executionMode = normalizeExecutionMode(params.executionMode);',
    'const parsed = parseLeadingExecutionMode(args?.trim() || "");',
    'pi.registerCommand("subagent-history", {',
    'pi.registerCommand("subagent-team-detail", {',
    'pi.registerCommand("subagent-chain-detail", {',
    'pi.registerCommand("subagent-failures", {',
    'pi.registerCommand("subagent-rerun", {',
]:
    if needle not in extension_src:
        raise SystemExit(f'subagent execution-mode integration missing: {needle}')

for needle in [
    'start_view: Option<Mode>,',
    'fleet_plane_filter: FleetPlaneFilter,',
    'config.start_view.unwrap_or(default_mode)',
    'let fleet_plane_filter = config.fleet_plane_filter;',
    'fleet_plane_filter,',
    'fn parse_start_view(value: &str) -> Option<Mode> {',
    '"fleet" | "detached" | "subagents" => Some(Mode::Fleet),',
    'fn parse_fleet_plane_filter(value: &str) -> Option<FleetPlaneFilter> {',
    '"delegated" | "specialist" | "subagents" => Some(FleetPlaneFilter::Delegated),',
    'fn resolve_start_view() -> Option<Mode> {',
    'std::env::var("AOC_MISSION_CONTROL_START_VIEW")',
    'fn resolve_fleet_plane_filter() -> FleetPlaneFilter {',
    'std::env::var("AOC_MISSION_CONTROL_FLEET_PLANE")',
    'cfg.start_view = Some(Mode::Fleet);',
    'cfg.fleet_plane_filter = FleetPlaneFilter::Delegated;',
]:
    if needle not in mission_contract_src:
        raise SystemExit(f'mission-control integration missing: {needle}')

print('Subagent UX runtime checks passed.')
PY
