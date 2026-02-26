use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MindObserverFeedStatus {
    Queued,
    Running,
    Success,
    Fallback,
    Error,
}

impl MindObserverFeedStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Success => "success",
            Self::Fallback => "fallback",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MindObserverFeedTriggerKind {
    TokenThreshold,
    TaskCompleted,
    ManualShortcut,
}

impl MindObserverFeedTriggerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TokenThreshold => "token_threshold",
            Self::TaskCompleted => "task_completed",
            Self::ManualShortcut => "manual_shortcut",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindObserverFeedProgress {
    pub t0_estimated_tokens: u32,
    pub t1_target_tokens: u32,
    pub t1_hard_cap_tokens: u32,
    pub tokens_until_next_run: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindObserverFeedEvent {
    pub status: MindObserverFeedStatus,
    pub trigger: MindObserverFeedTriggerKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_count: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enqueued_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<MindObserverFeedProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MindObserverFeedPayload {
    #[serde(default)]
    pub events: Vec<MindObserverFeedEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<i64>,
}
