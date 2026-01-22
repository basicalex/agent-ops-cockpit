use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    #[serde(flatten)]
    pub tags: HashMap<String, TagContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagContext {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
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
    pub priority: TaskPriority,
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(default, rename = "activeAgent")]
    pub active_agent: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    pub id: u32,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default, deserialize_with = "deserialize_deps")]
    pub dependencies: Vec<String>,
}

// Memory System Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub timestamp: String,
    pub content: String,
    pub tags: Vec<String>,
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
