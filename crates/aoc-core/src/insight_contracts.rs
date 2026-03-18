use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightDispatchMode {
    Dispatch,
    Chain,
    Parallel,
}

impl Default for InsightDispatchMode {
    fn default() -> Self {
        Self::Dispatch
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InsightDispatchRequest {
    #[serde(default)]
    pub mode: InsightDispatchMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default)]
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDispatchStepResult {
    pub agent: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDispatchResult {
    pub mode: InsightDispatchMode,
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub steps: Vec<InsightDispatchStepResult>,
    #[serde(default)]
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightBootstrapGapKind {
    MissingImplementation,
    UndocumentedCode,
    DriftRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightBootstrapGap {
    pub gap_id: String,
    pub kind: InsightBootstrapGapKind,
    pub severity: String,
    pub confidence: String,
    pub summary: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightTaskProposal {
    pub title: String,
    pub priority: String,
    pub rationale: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightSeedJob {
    pub seed_id: String,
    pub scope_tag: String,
    #[serde(default)]
    pub source_gap_ids: Vec<String>,
    pub priority: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightBootstrapRequest {
    #[serde(default)]
    pub scope_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tag: Option<String>,
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default = "default_max_gaps")]
    pub max_gaps: usize,
}

fn default_true() -> bool {
    true
}

fn default_max_gaps() -> usize {
    12
}

impl Default for InsightBootstrapRequest {
    fn default() -> Self {
        Self {
            scope_paths: Vec::new(),
            active_tag: None,
            dry_run: true,
            max_gaps: default_max_gaps(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightBootstrapResult {
    pub dry_run: bool,
    #[serde(default)]
    pub gaps: Vec<InsightBootstrapGap>,
    #[serde(default)]
    pub taskmaster_projection: Vec<InsightTaskProposal>,
    #[serde(default)]
    pub seeds: Vec<InsightSeedJob>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightRetrievalScope {
    Session,
    Project,
    Auto,
}

impl Default for InsightRetrievalScope {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightRetrievalMode {
    Brief,
    Refs,
    Snips,
}

impl Default for InsightRetrievalMode {
    fn default() -> Self {
        Self::Brief
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InsightRetrievalRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub scope: InsightRetrievalScope,
    #[serde(default)]
    pub mode: InsightRetrievalMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightRetrievalCitation {
    pub source_id: String,
    pub label: String,
    pub reference: String,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightRetrievalDrilldownRef {
    pub kind: String,
    pub label: String,
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightRetrievalHit {
    pub source_id: String,
    pub scope: InsightRetrievalScope,
    pub label: String,
    pub reference: String,
    pub score: i64,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub citations: Vec<InsightRetrievalCitation>,
    #[serde(default)]
    pub drilldown_refs: Vec<InsightRetrievalDrilldownRef>,
    #[serde(default)]
    pub line_budget: usize,
    #[serde(default)]
    pub lines_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightRetrievalResult {
    pub query: String,
    pub scope: InsightRetrievalScope,
    pub resolved_scope: InsightRetrievalScope,
    pub mode: InsightRetrievalMode,
    pub status: String,
    #[serde(default)]
    pub summary_lines: Vec<String>,
    #[serde(default)]
    pub hits: Vec<InsightRetrievalHit>,
    #[serde(default)]
    pub citations: Vec<InsightRetrievalCitation>,
    #[serde(default)]
    pub fallback_used: bool,
    #[serde(default)]
    pub hit_budget: usize,
    #[serde(default)]
    pub line_budget_per_hit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InsightStatusResult {
    pub queue_depth: i64,
    pub reflector_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_tick_ms: Option<i64>,
    #[serde(default)]
    pub lock_conflicts: u64,
    #[serde(default)]
    pub jobs_completed: u64,
    #[serde(default)]
    pub jobs_failed: u64,
    #[serde(default)]
    pub supervisor_runs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightDetachedMode {
    Dispatch,
    Chain,
    Parallel,
}

impl Default for InsightDetachedMode {
    fn default() -> Self {
        Self::Dispatch
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightDetachedJobStatus {
    Queued,
    Running,
    Success,
    Fallback,
    Error,
    Cancelled,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InsightDetachedDispatchRequest {
    #[serde(default)]
    pub mode: InsightDetachedMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default)]
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDetachedJob {
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_job_id: Option<String>,
    pub mode: InsightDetachedMode,
    pub status: InsightDetachedJobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default)]
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_step_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub step_results: Vec<InsightDispatchStepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDetachedDispatchResult {
    pub job: InsightDetachedJob,
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub accepted: bool,
    #[serde(default)]
    pub fallback_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InsightDetachedStatusRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDetachedStatusResult {
    pub status: String,
    #[serde(default)]
    pub jobs: Vec<InsightDetachedJob>,
    #[serde(default)]
    pub active_jobs: usize,
    #[serde(default)]
    pub fallback_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDetachedCancelRequest {
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InsightDetachedCancelResult {
    pub job_id: String,
    pub status: InsightDetachedJobStatus,
    pub summary: String,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub fallback_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum InsightCommand {
    InsightStatus,
    InsightDispatch(InsightDispatchRequest),
    InsightBootstrap(InsightBootstrapRequest),
    InsightRetrieve(InsightRetrievalRequest),
    InsightDetachedDispatch(InsightDetachedDispatchRequest),
    InsightDetachedStatus(InsightDetachedStatusRequest),
    InsightDetachedCancel(InsightDetachedCancelRequest),
}

impl InsightCommand {
    pub fn parse(command: &str, args: serde_json::Value) -> Result<Self, String> {
        match command {
            "insight_status" => Ok(Self::InsightStatus),
            "insight_dispatch" => serde_json::from_value(args)
                .map(Self::InsightDispatch)
                .map_err(|err| format!("invalid insight_dispatch args: {err}")),
            "insight_bootstrap" => serde_json::from_value(args)
                .map(Self::InsightBootstrap)
                .map_err(|err| format!("invalid insight_bootstrap args: {err}")),
            "insight_retrieve" => serde_json::from_value(args)
                .map(Self::InsightRetrieve)
                .map_err(|err| format!("invalid insight_retrieve args: {err}")),
            "insight_detached_dispatch" => serde_json::from_value(args)
                .map(Self::InsightDetachedDispatch)
                .map_err(|err| format!("invalid insight_detached_dispatch args: {err}")),
            "insight_detached_status" => serde_json::from_value(args)
                .map(Self::InsightDetachedStatus)
                .map_err(|err| format!("invalid insight_detached_status args: {err}")),
            "insight_detached_cancel" => serde_json::from_value(args)
                .map(Self::InsightDetachedCancel)
                .map_err(|err| format!("invalid insight_detached_cancel args: {err}")),
            other => Err(format!("unsupported insight command: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dispatch_defaults_mode() {
        let command = InsightCommand::parse(
            "insight_dispatch",
            serde_json::json!({"input": "summarize current state"}),
        )
        .expect("dispatch parse");

        let InsightCommand::InsightDispatch(request) = command else {
            panic!("expected dispatch")
        };
        assert_eq!(request.mode, InsightDispatchMode::Dispatch);
        assert_eq!(request.input, "summarize current state");
    }

    #[test]
    fn parse_bootstrap_defaults_to_dry_run() {
        let command = InsightCommand::parse("insight_bootstrap", serde_json::json!({}))
            .expect("bootstrap parse");

        let InsightCommand::InsightBootstrap(request) = command else {
            panic!("expected bootstrap")
        };
        assert!(request.dry_run);
        assert_eq!(request.max_gaps, 12);
    }

    #[test]
    fn parse_retrieve_defaults_scope_and_mode() {
        let command = InsightCommand::parse(
            "insight_retrieve",
            serde_json::json!({"query": "canon drift"}),
        )
        .expect("retrieve parse");

        let InsightCommand::InsightRetrieve(request) = command else {
            panic!("expected retrieve")
        };
        assert_eq!(request.scope, InsightRetrievalScope::Auto);
        assert_eq!(request.mode, InsightRetrievalMode::Brief);
        assert_eq!(request.query, "canon drift");
    }

    #[test]
    fn parse_detached_dispatch_defaults_mode() {
        let command = InsightCommand::parse(
            "insight_detached_dispatch",
            serde_json::json!({"input": "run detached observer"}),
        )
        .expect("detached dispatch parse");

        let InsightCommand::InsightDetachedDispatch(request) = command else {
            panic!("expected detached dispatch")
        };
        assert_eq!(request.mode, InsightDetachedMode::Dispatch);
        assert_eq!(request.input, "run detached observer");
        assert_eq!(request.cwd, None);
    }

    #[test]
    fn parse_detached_status_defaults_empty_filter() {
        let command = InsightCommand::parse("insight_detached_status", serde_json::json!({}))
            .expect("detached status parse");

        let InsightCommand::InsightDetachedStatus(request) = command else {
            panic!("expected detached status")
        };
        assert_eq!(request.job_id, None);
        assert_eq!(request.limit, None);
    }

    #[test]
    fn parse_detached_cancel_requires_job_id() {
        let command = InsightCommand::parse(
            "insight_detached_cancel",
            serde_json::json!({"job_id": "sj_123"}),
        )
        .expect("detached cancel parse");

        let InsightCommand::InsightDetachedCancel(request) = command else {
            panic!("expected detached cancel")
        };
        assert_eq!(request.job_id, "sj_123");
        assert_eq!(request.reason, None);
    }
}
