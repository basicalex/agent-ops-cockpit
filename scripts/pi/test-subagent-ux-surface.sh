#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
extension="$repo_root/.pi/extensions/subagent.ts"
shared="$repo_root/.pi/extensions/subagent/shared.ts"
artifacts="$repo_root/.pi/extensions/subagent/artifacts.ts"
manifests="$repo_root/.pi/extensions/subagent/manifests.ts"
registry="$repo_root/.pi/extensions/subagent/registry.ts"
doc="$repo_root/docs/subagent-runtime.md"
mission_doc="$repo_root/docs/mission-control.md"
mission_ops_doc="$repo_root/docs/mission-control-ops.md"
supervision_toggle="$repo_root/bin/aoc-subagent-supervision-toggle"
supervision_cmd="$repo_root/bin/aoc-subagent-supervision"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local needle="$2"
  grep -Fq "$needle" "$file" || fail "Expected $file to contain: $needle"
}

node - <<'NODE' "$extension" "$shared" "$artifacts" "$manifests" "$registry"
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

for file in "$doc" "$mission_doc" "$mission_ops_doc" "$supervision_toggle" "$supervision_cmd" "$shared" "$artifacts" "$manifests" "$registry"
do
  [[ -f "$file" ]] || fail "Missing expected file: $file"
done

assert_contains "$extension" 'import { persistArtifactBundle as persistArtifactBundleImpl'
assert_contains "$extension" 'import { availableAgents, availableChains, availableTeams, assertAgentAvailable, assertChainAvailable, assertTeamAvailable, loadManifestBundle }'
assert_contains "$extension" 'import {'
assert_contains "$extension" 'from "./subagent/registry.ts";'
assert_contains "$extension" 'async function launchAgentJob('
assert_contains "$extension" 'async function launchChainJob('
assert_contains "$extension" 'function persistArtifactBundle(root: string, job: JobRecord, options?: ArtifactPersistenceOptions): JobRecord {'
assert_contains "$extension" 'const MANAGER_SECTIONS: ManagerSection[] = ["recent", "agents", "teams", "chains", "roles"];'
assert_contains "$extension" 'Subagent Manager'
assert_contains "$extension" 'function showClarifyLaunchDialog(ctx: ExtensionContext, request: LaunchDialogRequest): Promise<LaunchDialogResult | undefined> {'
assert_contains "$extension" 'function resolveLaunchFeedback(pi: ExtensionAPI, ctx: ExtensionContext, job: JobRecord, queuedPrefix: string): Promise<{ job: JobRecord; notice: string; level: "info" | "warning" }> {'
assert_contains "$extension" 'function dispatchClarifiedLaunch(pi: ExtensionAPI, ctx: ExtensionContext, request: LaunchDialogRequest): Promise<JobRecord | undefined> {'
assert_contains "$extension" 'async function openSubagentManager(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {'
assert_contains "$extension" 'refreshing detached status…'
assert_contains "$extension" 'renderField("execution_mode", `${executionMode} · ${modeHint}`, currentField === "mode")'
assert_contains "$extension" 'inline mode timed out after ${Math.round(INLINE_WAIT_TIMEOUT_MS / 1000)}s; continuing in background'
assert_contains "$extension" 'parseLeadingExecutionMode(raw: string): { executionMode: ExecutionMode; rest: string }'
assert_contains "$extension" 'pi.registerCommand("subagent-manager", {'
assert_contains "$extension" 'Open the manager-lite overlay for agents, teams, chains, roles, and recent jobs'
assert_contains "$extension" 'function summarizeStepResults(job: JobRecord): string | undefined {'
assert_contains "$extension" 'function formatStepResultLines(job: JobRecord, prefix = "  ", limit = 6): string[] {'
assert_contains "$extension" 'function recentJobsForTeam(teamName: string, limit = 3): JobRecord[] {'
assert_contains "$extension" 'function formatTeamDetail(root: string, name: string, members: string[]): string {'
assert_contains "$extension" 'function needsAttentionStatus(status: JobStatus): boolean {'
assert_contains "$extension" 'const MAX_SUBAGENT_NESTING_DEPTH = 1;'
assert_contains "$extension" 'function assertSupportedSessionMode(sessionMode: string | undefined): void {'
assert_contains "$extension" 'function assertNoUnsupportedSessionModeFlags(raw: string): void {'
assert_contains "$extension" 'function assertDelegatedRecursionAllowed(): void {'
assert_contains "$extension" 'function assertDispatchGuardrails(sessionMode?: string): void {'
assert_contains "$extension" 'function formatFailureJobs(limit = 5): string {'
assert_contains "$extension" 'history: ${terminalCount} terminal   attention: ${failures.length}'
assert_contains "$extension" 'Enter/i inspect • h handoff • r rerun via clarify • f latest failure • c cancel'
assert_contains "$extension" 'function recentJobsForChain(chainName: string, limit = 3): JobRecord[] {'
assert_contains "$extension" 'function formatChainDetail(root: string, name: string, chain: ChainDefinition): string {'
assert_contains "$extension" 'detail: ${truncateToWidth(`/subagent-team-detail ${name}`'
assert_contains "$extension" 'detail: ${truncateToWidth(`/subagent-chain-detail ${name}`'
assert_contains "$extension" 'Enter opens clarify-before-run • r reruns latest team via clarify • l shows the raw command'
assert_contains "$extension" 'Enter opens clarify-before-run • r reruns latest chain via clarify • l shows the raw command'
assert_contains "$extension" 'async function rerunJob(pi: ExtensionAPI, ctx: ExtensionContext, jobId: string, mode: "clarify" | "as_is" = "clarify"): Promise<JobRecord | undefined> {'
assert_contains "$extension" 'pi.registerCommand("subagent-history", {'
assert_contains "$extension" 'Show recent detached subagent run history. Usage: /subagent-history [count]'
assert_contains "$extension" 'pi.registerCommand("subagent-team-detail", {'
assert_contains "$extension" 'Show team detail, members, and recent runs. Usage: /subagent-team-detail <team>'
assert_contains "$extension" 'pi.registerCommand("subagent-chain-detail", {'
assert_contains "$extension" 'Show chain detail, steps, and recent runs. Usage: /subagent-chain-detail <chain>'
assert_contains "$extension" 'pi.registerCommand("subagent-failures", {'
assert_contains "$extension" 'Show recent detached subagent failures needing attention. Usage: /subagent-failures [count]'
assert_contains "$extension" 'pi.registerCommand("subagent-rerun", {'
assert_contains "$extension" 'Rerun a detached subagent job from preserved metadata. Usage: /subagent-rerun [--as-is] <job-id>'
assert_contains "$extension" 'pi.registerCommand("subagent-team", {'
assert_contains "$extension" 'Dispatch one detached canonical team fanout. Usage: /subagent-team [--wait|--summary|--background] <team> :: <task>'
assert_contains "$extension" 'AOC_SUBAGENT_PARENT_JOB_ID: currentDelegatedParentJobId() ?? "",'
assert_contains "$extension" 'AOC_SUBAGENT_DEPTH: String(currentSubagentNestingDepth() + 1),'
assert_contains "$extension" 'artifacts: ${job.artifactDir}'
assert_contains "$extension" 'report: ${job.reportPath}'

assert_contains "$shared" 'export type JobStepResult = {'
assert_contains "$shared" 'export type ExecutionMode = "background" | "inline_wait" | "inline_summary";'
assert_contains "$shared" 'export const ARTIFACTS_DIR = path.join(".pi", "tmp", "subagents");'
assert_contains "$shared" 'export const REPORT_FILENAME = "report.md";'
assert_contains "$shared" 'export const META_FILENAME = "meta.json";'
assert_contains "$shared" 'export const EVENTS_FILENAME = "events.jsonl";'
assert_contains "$shared" 'export const PROMPT_FILENAME = "prompt.md";'
assert_contains "$shared" 'export const STDERR_FILENAME = "stderr.log";'
assert_contains "$shared" 'executionMode: ExecutionMode;'
assert_contains "$shared" 'artifactDir?: string;'
assert_contains "$shared" 'reportPath?: string;'
assert_contains "$shared" 'metaPath?: string;'
assert_contains "$shared" 'eventsPath?: string;'
assert_contains "$shared" 'promptPath?: string;'
assert_contains "$shared" 'stderrPath?: string;'
assert_contains "$shared" 'export const INLINE_WAIT_TIMEOUT_MS = 45_000;'

assert_contains "$artifacts" 'export function persistArtifactBundle('
assert_contains "$artifacts" 'if (job.stepResults?.length) {'
assert_contains "$artifacts" '## Step Results'
assert_contains "$artifacts" 'writeArtifactFile(root, enriched.reportPath, renderReportArtifact(enriched, options?.fullOutput, helpers.summarizeToolPolicies));'
assert_contains "$artifacts" 'job: helpers.snapshotJob(enriched),'
assert_contains "$artifacts" 'fail open: artifact persistence should not break detached execution or recovery'

assert_contains "$manifests" 'const manifestCache = new Map<string, { key: string; bundle: ManifestBundle }>();'
assert_contains "$manifests" 'function manifestCacheKey(root: string): string {'
assert_contains "$manifests" 'if (cached && cached.key === key) return cached.bundle;'
assert_contains "$manifests" 'manifestCache.set(root, { key, bundle });'

assert_contains "$registry" 'export async function sendPulseCommand(command: string, args: Record<string, unknown>): Promise<PulseCommandResultPayload> {'
assert_contains "$registry" 'export async function fetchMindContextPack(role: string, reason: string): Promise<MindContextPackPayload | undefined> {'
assert_contains "$registry" 'export function renderContextPackPrelude(pack: MindContextPackPayload | undefined): string | undefined {'
assert_contains "$registry" 'step_results?: Array<'
assert_contains "$registry" 'export function mapDurableJob(job: DurableDetachedJob, root: string): JobRecord {'
assert_contains "$registry" 'stepResults: job.step_results?.map((step) => ({'

assert_contains "$doc" '.pi/tmp/subagents/<job-id>/report.md'
assert_contains "$doc" '.pi/tmp/subagents/<job-id>/meta.json'
assert_contains "$doc" '.pi/tmp/subagents/<job-id>/events.jsonl'
assert_contains "$doc" '.pi/tmp/subagents/<job-id>/prompt.md'
assert_contains "$doc" '.pi/tmp/subagents/<job-id>/stderr.log'
assert_contains "$doc" 'aoc-subagent-supervision-toggle'
assert_contains "$doc" '`background`, `inline_wait`, `inline_summary`'
assert_contains "$doc" '/subagent-run [--wait|--summary|--background] <agent> :: <task>'
assert_contains "$doc" '/subagent-team [--wait|--summary|--background] <team> :: <task>'
assert_contains "$doc" '/specialist-run [--wait|--summary|--background] <role> :: <task> [:: approve-write]'
assert_contains "$doc" '/subagent-history [count]'
assert_contains "$doc" '/subagent-failures [count]'
assert_contains "$doc" '/subagent-team-detail <team>'
assert_contains "$doc" '/subagent-chain-detail <chain>'
assert_contains "$doc" '/subagent-rerun [--as-is] <job-id>'
assert_contains "$doc" '`/subagent-failures` shows recent non-success jobs needing attention'
assert_contains "$doc" '`/subagent-team-detail <team>` shows team members plus recent team runs'
assert_contains "$doc" 'shows team membership previews and latest team-run context in the teams tab'
assert_contains "$doc" 'shows chain step previews, latest chain-run context, and `r` rerun-via-clarify from the chains tab'
assert_contains "$doc" '`/subagent-rerun --as-is <job-id>` replays the preserved launch shape directly'
assert_contains "$doc" 'delegated launches only support a fresh detached session mode today'
assert_contains "$doc" 'nested delegated dispatch from inside an already-detached delegated run is blocked'
assert_contains "$doc" 'supports `f` in the recent tab to jump directly to the latest attention-needed run'
assert_contains "$doc" 'delegated plane preselected'
assert_contains "$mission_doc" 'aoc-subagent-supervision-toggle'
assert_contains "$mission_ops_doc" 'Delegated subagent supervision fast path'
assert_contains "$mission_ops_doc" 'Subagent Supervision'
assert_contains "$supervision_cmd" 'AOC_MISSION_CONTROL_START_VIEW="${AOC_MISSION_CONTROL_START_VIEW:-fleet}"'
assert_contains "$supervision_cmd" 'AOC_MISSION_CONTROL_FLEET_PLANE="${AOC_MISSION_CONTROL_FLEET_PLANE:-delegated}"'
assert_contains "$supervision_toggle" 'AOC_MISSION_CONTROL_CMD="${AOC_SUBAGENT_SUPERVISION_CMD:-$script_dir/aoc-subagent-supervision}"'
assert_contains "$supervision_toggle" 'AOC_MISSION_CONTROL_PANE_NAME="${AOC_SUBAGENT_SUPERVISION_PANE_NAME:-Subagent Supervision}"'

echo "Subagent UX surface checks passed."
