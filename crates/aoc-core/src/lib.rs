use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    #[serde(flatten)]
    pub tags: HashMap<String, TagContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagContext {
    pub tasks: Vec<Task>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub details: String,
    #[serde(default, rename = "testStrategy")]
    pub test_strategy: String,
    pub status: TaskStatus,
    #[serde(default, deserialize_with = "deserialize_deps")]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub priority: TaskPriority,
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
    #[serde(default, rename = "aocPrd")]
    pub aoc_prd: Option<TaskPrd>,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(default, rename = "activeAgent")]
    pub active_agent: bool,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPrd {
    pub path: String,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Done,
    Cancelled,
    Deferred,
    Review,
    Blocked,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in-progress",
            TaskStatus::Done => "done",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Deferred => "deferred",
            TaskStatus::Review => "review",
            TaskStatus::Blocked => "blocked",
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self, TaskStatus::Done | TaskStatus::Cancelled)
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TaskStatus {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let normalized = input.trim().to_lowercase();
        match normalized.as_str() {
            "pending" => Ok(TaskStatus::Pending),
            "in-progress" | "in_progress" | "inprogress" => Ok(TaskStatus::InProgress),
            "done" => Ok(TaskStatus::Done),
            "cancelled" | "canceled" => Ok(TaskStatus::Cancelled),
            "deferred" => Ok(TaskStatus::Deferred),
            "review" => Ok(TaskStatus::Review),
            "blocked" => Ok(TaskStatus::Blocked),
            other => Err(format!("Unknown status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    High,
    Medium,
    Low,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Medium
    }
}

impl TaskPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskPriority::High => "high",
            TaskPriority::Medium => "medium",
            TaskPriority::Low => "low",
        }
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TaskPriority {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let normalized = input.trim().to_lowercase();
        match normalized.as_str() {
            "high" => Ok(TaskPriority::High),
            "medium" => Ok(TaskPriority::Medium),
            "low" => Ok(TaskPriority::Low),
            other => Err(format!("Unknown priority: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    #[serde(deserialize_with = "deserialize_id_u32")]
    pub id: u32,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default, deserialize_with = "deserialize_deps")]
    pub dependencies: Vec<String>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

// Memory System Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub timestamp: String,
    pub content: String,
    pub tags: Vec<String>,
}

/// Deserialize an ID that can be either a string or a number into a String
fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val: serde_json::Value = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom("expected string or number for id")),
    }
}

/// Deserialize an ID that can be either a string or a number into a u32
fn deserialize_id_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let val: serde_json::Value = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::String(s) => s.parse::<u32>().map_err(serde::de::Error::custom),
        serde_json::Value::Number(n) => n
            .as_u64()
            .and_then(|u| u32::try_from(u).ok())
            .ok_or_else(|| serde::de::Error::custom("invalid u32")),
        _ => Err(serde::de::Error::custom("expected string or number for id")),
    }
}

fn deserialize_deps<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
    let mut deps = Vec::new();
    for val in v {
        if let Some(s) = val.as_str() {
            deps.push(s.to_string());
        } else if let Some(i) = val.as_i64() {
            deps.push(i.to_string());
        } else if let Some(u) = val.as_u64() {
            deps.push(u.to_string());
        }
    }
    Ok(deps)
}
