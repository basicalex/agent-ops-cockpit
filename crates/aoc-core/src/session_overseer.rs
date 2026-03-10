use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const OVERSEER_SNAPSHOT_TOPIC: &str = "observer_snapshot";
pub const OVERSEER_DELTA_TOPIC: &str = "observer_delta";
pub const OVERSEER_TIMELINE_TOPIC: &str = "observer_timeline";
pub const OVERSEER_COMMAND_CAPABILITY: &str = "overseer_command";
pub const OVERSEER_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    #[default]
    Idle,
    Active,
    Blocked,
    NeedsInput,
    Paused,
    Done,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProgressPhase {
    #[default]
    Unknown,
    Planning,
    Implementation,
    Validation,
    Handoff,
    Complete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttentionLevel {
    #[default]
    None,
    Info,
    Warn,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanAlignment {
    #[default]
    Unknown,
    High,
    Medium,
    Low,
    Unassigned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DriftRisk {
    #[default]
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverseerSourceKind {
    #[default]
    Wrapper,
    Hub,
    Mind,
    Manager,
    LocalFallback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ObserverEventKind {
    #[default]
    ProgressUpdate,
    StatusRefresh,
    Blocked,
    Milestone,
    CommandRequested,
    CommandResult,
    HandoffReady,
    TaskCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProgressPosition {
    #[serde(default)]
    pub phase: ProgressPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkerAssignment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AttentionSignal {
    #[serde(default)]
    pub level: AttentionLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DuplicateWorkSignal {
    #[serde(default)]
    pub overlapping_files: Vec<String>,
    #[serde(default)]
    pub overlapping_task_ids: Vec<String>,
    #[serde(default)]
    pub other_agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkerSnapshot {
    pub session_id: String,
    pub agent_id: String,
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default)]
    pub status: WorkerStatus,
    #[serde(default)]
    pub progress: ProgressPosition,
    #[serde(default)]
    pub assignment: WorkerAssignment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
    #[serde(default)]
    pub files_touched: Vec<String>,
    #[serde(default)]
    pub plan_alignment: PlanAlignment,
    #[serde(default)]
    pub drift_risk: DriftRisk,
    #[serde(default)]
    pub attention: AttentionSignal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_work: Option<DuplicateWorkSignal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_meaningful_progress_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_after_ms: Option<u64>,
    #[serde(default)]
    pub source: OverseerSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ObserverEvent {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    #[serde(default)]
    pub kind: ObserverEventKind,
    pub session_id: String,
    pub agent_id: String,
    pub pane_id: String,
    #[serde(default)]
    pub source: OverseerSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<WorkerSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<ManagerCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_result: Option<ManagerCommandResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emitted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ObserverTimelineEntry {
    pub event_id: String,
    pub session_id: String,
    pub agent_id: String,
    #[serde(default)]
    pub kind: ObserverEventKind,
    #[serde(default)]
    pub source: OverseerSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention: Option<AttentionSignal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emitted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ObserverSnapshot {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub session_id: String,
    #[serde(default)]
    pub generated_at_ms: Option<i64>,
    #[serde(default)]
    pub workers: Vec<WorkerSnapshot>,
    #[serde(default)]
    pub timeline: Vec<ObserverTimelineEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagerCommandKind {
    RequestStatusUpdate,
    RequestHandoff,
    PauseAndSummarize,
    RunValidation,
    SwitchFocus,
    FinalizeAndReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SwitchFocusArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", content = "args", rename_all = "snake_case")]
pub enum ManagerCommand {
    RequestStatusUpdate,
    RequestHandoff,
    PauseAndSummarize,
    RunValidation,
    SwitchFocus(SwitchFocusArgs),
    FinalizeAndReport,
}

impl ManagerCommand {
    pub fn kind(&self) -> ManagerCommandKind {
        match self {
            Self::RequestStatusUpdate => ManagerCommandKind::RequestStatusUpdate,
            Self::RequestHandoff => ManagerCommandKind::RequestHandoff,
            Self::PauseAndSummarize => ManagerCommandKind::PauseAndSummarize,
            Self::RunValidation => ManagerCommandKind::RunValidation,
            Self::SwitchFocus(_) => ManagerCommandKind::SwitchFocus,
            Self::FinalizeAndReport => ManagerCommandKind::FinalizeAndReport,
        }
    }

    pub fn parse(command: &str, args: Value) -> Result<Self, String> {
        match command {
            "request_status_update" => Ok(Self::RequestStatusUpdate),
            "request_handoff" => Ok(Self::RequestHandoff),
            "pause_and_summarize" => Ok(Self::PauseAndSummarize),
            "run_validation" => Ok(Self::RunValidation),
            "switch_focus" => serde_json::from_value(args)
                .map(Self::SwitchFocus)
                .map_err(|err| format!("invalid switch_focus args: {err}")),
            "finalize_and_report" => Ok(Self::FinalizeAndReport),
            other => Err(format!("unsupported overseer command: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ManagerCommandStatus {
    #[default]
    Accepted,
    Completed,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ManagerCommandError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ManagerCommandResult {
    #[serde(default)]
    pub status: ManagerCommandStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ManagerCommandError>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandPolicyDecision {
    #[default]
    Allow,
    ConfirmRequired,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandPolicyOutcome {
    pub decision: CommandPolicyDecision,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverseerRetentionPolicy {
    pub max_timeline_entries: usize,
    pub worker_stale_after_ms: u64,
    pub command_result_ttl_ms: u64,
}

impl Default for OverseerRetentionPolicy {
    fn default() -> Self {
        Self {
            max_timeline_entries: 250,
            worker_stale_after_ms: 5 * 60 * 1000,
            command_result_ttl_ms: 10 * 60 * 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyContext {
    #[serde(default)]
    pub worker_status: WorkerStatus,
    #[serde(default)]
    pub has_blocker: bool,
    #[serde(default)]
    pub human_confirmed: bool,
}

pub fn evaluate_command_policy(
    command: &ManagerCommand,
    context: &PolicyContext,
) -> CommandPolicyOutcome {
    match command {
        ManagerCommand::RequestStatusUpdate
        | ManagerCommand::RequestHandoff
        | ManagerCommand::RunValidation => CommandPolicyOutcome {
            decision: CommandPolicyDecision::Allow,
            reason: "safe readback/validation command",
        },
        ManagerCommand::PauseAndSummarize => {
            if context.human_confirmed {
                CommandPolicyOutcome {
                    decision: CommandPolicyDecision::Allow,
                    reason: "human confirmed interruption",
                }
            } else {
                CommandPolicyOutcome {
                    decision: CommandPolicyDecision::ConfirmRequired,
                    reason: "interrupts active worker flow",
                }
            }
        }
        ManagerCommand::SwitchFocus(_) => {
            if context.worker_status == WorkerStatus::Blocked || context.human_confirmed {
                CommandPolicyOutcome {
                    decision: CommandPolicyDecision::Allow,
                    reason: "worker blocked or human confirmed retargeting",
                }
            } else {
                CommandPolicyOutcome {
                    decision: CommandPolicyDecision::ConfirmRequired,
                    reason: "retargets worker away from current task",
                }
            }
        }
        ManagerCommand::FinalizeAndReport => {
            if context.human_confirmed {
                CommandPolicyOutcome {
                    decision: CommandPolicyDecision::Allow,
                    reason: "human confirmed finalize action",
                }
            } else {
                CommandPolicyOutcome {
                    decision: CommandPolicyDecision::ConfirmRequired,
                    reason: "finalizes worker activity and requires review",
                }
            }
        }
    }
}

const fn default_schema_version() -> u16 {
    OVERSEER_SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_switch_focus_command() {
        let command = ManagerCommand::parse(
            "switch_focus",
            serde_json::json!({"task_id": "149.3", "summary": "move to hub cache"}),
        )
        .expect("parse switch_focus");

        let ManagerCommand::SwitchFocus(args) = command else {
            panic!("expected switch focus")
        };
        assert_eq!(args.task_id.as_deref(), Some("149.3"));
        assert_eq!(args.summary.as_deref(), Some("move to hub cache"));
    }

    #[test]
    fn observer_snapshot_defaults_backward_compat() {
        let snapshot: ObserverSnapshot = serde_json::from_value(serde_json::json!({
            "session_id": "s1"
        }))
        .expect("snapshot parse");

        assert_eq!(snapshot.schema_version, OVERSEER_SCHEMA_VERSION);
        assert_eq!(snapshot.session_id, "s1");
        assert!(snapshot.workers.is_empty());
        assert!(snapshot.timeline.is_empty());
        assert!(snapshot.degraded_reason.is_none());
    }

    #[test]
    fn worker_snapshot_defaults_attention_and_plan_fields() {
        let worker: WorkerSnapshot = serde_json::from_value(serde_json::json!({
            "session_id": "s1",
            "agent_id": "s1::12",
            "pane_id": "12"
        }))
        .expect("worker parse");

        assert_eq!(worker.status, WorkerStatus::Idle);
        assert_eq!(worker.progress.phase, ProgressPhase::Unknown);
        assert_eq!(worker.plan_alignment, PlanAlignment::Unknown);
        assert_eq!(worker.drift_risk, DriftRisk::Unknown);
        assert_eq!(worker.attention.level, AttentionLevel::None);
        assert_eq!(worker.source, OverseerSourceKind::Wrapper);
    }

    #[test]
    fn policy_allows_safe_commands() {
        let outcome = evaluate_command_policy(
            &ManagerCommand::RequestStatusUpdate,
            &PolicyContext::default(),
        );
        assert_eq!(outcome.decision, CommandPolicyDecision::Allow);
    }

    #[test]
    fn policy_requires_confirmation_for_focus_switch_when_active() {
        let outcome = evaluate_command_policy(
            &ManagerCommand::SwitchFocus(SwitchFocusArgs::default()),
            &PolicyContext {
                worker_status: WorkerStatus::Active,
                has_blocker: false,
                human_confirmed: false,
            },
        );
        assert_eq!(outcome.decision, CommandPolicyDecision::ConfirmRequired);
    }

    #[test]
    fn policy_allows_focus_switch_for_blocked_worker() {
        let outcome = evaluate_command_policy(
            &ManagerCommand::SwitchFocus(SwitchFocusArgs::default()),
            &PolicyContext {
                worker_status: WorkerStatus::Blocked,
                has_blocker: true,
                human_confirmed: false,
            },
        );
        assert_eq!(outcome.decision, CommandPolicyDecision::Allow);
    }
}
