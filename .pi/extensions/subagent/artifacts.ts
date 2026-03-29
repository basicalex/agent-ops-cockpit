import * as fs from "node:fs";
import * as path from "node:path";
import {
	ARTIFACTS_DIR,
	EVENTS_FILENAME,
	META_FILENAME,
	PROMPT_FILENAME,
	REPORT_FILENAME,
	STDERR_FILENAME,
	relative,
	resolveScopedCwd,
	sanitizeSlug,
	type AgentConfig,
	type JobRecord,
	type PersistedJobRecord,
	type ToolPolicyRecord,
} from "./shared.ts";

export type ArtifactPersistenceOptions = {
	prompt?: string;
	agent?: AgentConfig;
	appendEvent?: string;
	appendStderr?: string;
	fullOutput?: string;
};

export type ArtifactPersistenceHelpers = {
	snapshotJob: (job: JobRecord) => PersistedJobRecord;
	summarizeToolPolicies: (toolPolicies: ToolPolicyRecord[] | undefined) => string | undefined;
};

export function artifactRefs(root: string, jobId: string): Pick<JobRecord, "artifactDir" | "reportPath" | "metaPath" | "eventsPath" | "promptPath" | "stderrPath"> {
	const dir = path.join(root, ARTIFACTS_DIR, sanitizeSlug(jobId));
	return {
		artifactDir: relative(root, dir),
		reportPath: relative(root, path.join(dir, REPORT_FILENAME)),
		metaPath: relative(root, path.join(dir, META_FILENAME)),
		eventsPath: relative(root, path.join(dir, EVENTS_FILENAME)),
		promptPath: relative(root, path.join(dir, PROMPT_FILENAME)),
		stderrPath: relative(root, path.join(dir, STDERR_FILENAME)),
	};
}

export function withArtifactRefs(root: string, job: JobRecord): JobRecord {
	if (job.artifactDir && job.reportPath && job.metaPath && job.eventsPath && job.promptPath && job.stderrPath) return job;
	return { ...job, ...artifactRefs(root, job.jobId) };
}

export function resolveArtifactPath(root: string, filePath: string | undefined): string | undefined {
	if (!filePath) return undefined;
	return path.isAbsolute(filePath) ? filePath : path.join(root, filePath);
}

export function ensureArtifactDir(root: string, job: JobRecord): void {
	const dir = resolveArtifactPath(root, job.artifactDir);
	if (!dir) return;
	fs.mkdirSync(dir, { recursive: true });
}

export function writeArtifactFile(root: string, filePath: string | undefined, content: string): void {
	const resolved = resolveArtifactPath(root, filePath);
	if (!resolved) return;
	fs.writeFileSync(resolved, content, "utf8");
}

export function appendArtifactFile(root: string, filePath: string | undefined, content: string): void {
	const resolved = resolveArtifactPath(root, filePath);
	if (!resolved) return;
	fs.appendFileSync(resolved, content, "utf8");
}

export function renderPromptArtifact(job: JobRecord, prompt: string, agent?: AgentConfig): string {
	const lines = [
		"# Detached subagent prompt",
		"",
		`- job: ${job.jobId}`,
		`- agent: ${job.agent}`,
		`- mode: ${job.mode}`,
		`- execution_mode: ${job.executionMode}`,
		`- cwd: ${job.cwd}`,
		`- created_at: ${new Date(job.createdAt).toISOString()}`,
	];
	if (job.specialistRole) lines.push(`- specialist_role: ${job.specialistRole}`);
	if (job.teamName) lines.push(`- team: ${job.teamName}`);
	if (job.chainName) lines.push(`- chain: ${job.chainName}`);
	if (typeof job.chainStepIndex === "number" && typeof job.chainStepCount === "number") {
		lines.push(`- chain_step: ${job.chainStepIndex + 1}/${job.chainStepCount}`);
	}
	if (typeof job.contextPackUsed === "boolean") lines.push(`- context_pack: ${job.contextPackUsed ? "mind-v2-attached" : "unavailable"}`);
	if (typeof job.writeApproved === "boolean") lines.push(`- write_approval: ${job.writeApproved ? "approved" : "read-first"}`);
	if (agent?.model) lines.push(`- model: ${agent.model}`);
	if (agent?.tools?.length) lines.push(`- tools: ${agent.tools.join(",")}`);
	lines.push("", "## Task", "", prompt || "(none)");
	if (agent?.systemPrompt) lines.push("", "## System Prompt", "", agent.systemPrompt);
	return lines.join("\n") + "\n";
}

export function renderReportArtifact(
	job: JobRecord,
	fullOutput: string | undefined,
	summarizeToolPolicies: (toolPolicies: ToolPolicyRecord[] | undefined) => string | undefined,
): string {
	const lines = [
		"# Detached subagent report",
		"",
		`- job: ${job.jobId}`,
		`- agent: ${job.agent}`,
		`- mode: ${job.mode}`,
		`- execution_mode: ${job.executionMode}`,
		`- status: ${job.status}`,
		`- cwd: ${job.cwd}`,
		`- created_at: ${new Date(job.createdAt).toISOString()}`,
	];
	if (job.specialistRole) lines.push(`- specialist_role: ${job.specialistRole}`);
	if (job.teamName) lines.push(`- team: ${job.teamName}`);
	if (typeof job.contextPackUsed === "boolean") lines.push(`- context_pack: ${job.contextPackUsed ? "mind-v2-attached" : "unavailable"}`);
	if (typeof job.writeApproved === "boolean") lines.push(`- write_approval: ${job.writeApproved ? "approved" : "read-first"}`);
	if (job.startedAt) lines.push(`- started_at: ${new Date(job.startedAt).toISOString()}`);
	if (job.finishedAt) lines.push(`- finished_at: ${new Date(job.finishedAt).toISOString()}`);
	if (typeof job.exitCode === "number") lines.push(`- exit_code: ${job.exitCode}`);
	if (job.chainName) lines.push(`- chain: ${job.chainName}`);
	if (typeof job.chainStepIndex === "number" && typeof job.chainStepCount === "number") {
		lines.push(`- chain_step: ${job.chainStepIndex + 1}/${job.chainStepCount}`);
	}
	const toolSummary = summarizeToolPolicies(job.toolPolicies);
	if (toolSummary) lines.push(`- tool_provenance: ${toolSummary}`);
	if (job.reportPath) lines.push(`- report_path: ${job.reportPath}`);
	lines.push("", "## Task", "", job.task || "(none)", "", "## Result", "", fullOutput || job.outputExcerpt || "(no output recorded)");
	if (job.stepResults?.length) {
		lines.push("", "## Step Results", "");
		for (const [index, step] of job.stepResults.entries()) {
			lines.push(`- step ${index + 1}: ${step.agent} · ${step.status}`);
			if (step.outputExcerpt) lines.push(`  output: ${step.outputExcerpt}`);
			if (step.error) lines.push(`  error: ${step.error}`);
			if (step.stderrExcerpt) lines.push(`  stderr: ${step.stderrExcerpt}`);
		}
	}
	if (job.error) lines.push("", "## Error", "", job.error);
	if (job.stderrExcerpt) lines.push("", "## Stderr Excerpt", "", job.stderrExcerpt);
	if (job.manifestErrors.length > 0) lines.push("", "## Manifest Warnings", "", ...job.manifestErrors.map((item) => `- ${item}`));
	return lines.join("\n") + "\n";
}

export function persistArtifactBundle(root: string, job: JobRecord, helpers: ArtifactPersistenceHelpers, options?: ArtifactPersistenceOptions): JobRecord {
	const enriched = withArtifactRefs(root, job);
	try {
		ensureArtifactDir(root, enriched);
		if (typeof options?.prompt === "string") {
			writeArtifactFile(root, enriched.promptPath, renderPromptArtifact(enriched, options.prompt, options.agent));
		}
		if (options?.appendEvent) {
			appendArtifactFile(root, enriched.eventsPath, options.appendEvent.endsWith("\n") ? options.appendEvent : `${options.appendEvent}\n`);
		}
		if (options?.appendStderr) {
			appendArtifactFile(root, enriched.stderrPath, options.appendStderr);
		}
		writeArtifactFile(root, enriched.reportPath, renderReportArtifact(enriched, options?.fullOutput, helpers.summarizeToolPolicies));
		writeArtifactFile(root, enriched.metaPath, JSON.stringify({
			version: 1,
			updatedAt: new Date().toISOString(),
			job: helpers.snapshotJob(enriched),
			fullOutputChars: options?.fullOutput?.length ?? undefined,
		}, null, 2) + "\n");
	} catch {
		// fail open: artifact persistence should not break detached execution or recovery
	}
	return enriched;
}
