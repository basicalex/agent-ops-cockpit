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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum InsightCommand {
    InsightStatus,
    InsightDispatch(InsightDispatchRequest),
    InsightBootstrap(InsightBootstrapRequest),
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
}
