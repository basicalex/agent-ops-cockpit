use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskRoot {
    #[serde(flatten)]
    pub tags: BTreeMap<String, TaskTag>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskTag {
    #[serde(default)]
    pub tasks: Vec<Task>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TaskId {
    Number(u64),
    Text(String),
}

fn deserialize_task_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let parsed = TaskId::deserialize(deserializer)?;
    Ok(match parsed {
        TaskId::Number(value) => value.to_string(),
        TaskId::Text(value) => value,
    })
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Task {
    #[serde(deserialize_with = "deserialize_task_id")]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub details: String,
    #[serde(default, rename = "testStrategy")]
    pub test_strategy: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default, rename = "activeAgent")]
    pub active_agent: bool,
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Subtask {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
}

impl Task {
    pub fn completion_stats(&self) -> (usize, usize) {
        if self.subtasks.is_empty() {
            let done = if self.status.to_lowercase() == "done" { 1 } else { 0 };
            return (done, 1);
        }
        
        let mut done = 0;
        let mut total = 0;
        for sub in &self.subtasks {
            let (d, t) = sub.completion_stats();
            done += d;
            total += t;
        }
        (done, total)
    }

    pub fn progress(&self) -> f32 {
        let (done, total) = self.completion_stats();
        if total == 0 { 0.0 } else { done as f32 / total as f32 }
    }
}

impl Subtask {
    pub fn completion_stats(&self) -> (usize, usize) {
        if self.subtasks.is_empty() {
            let done = if self.status.to_lowercase() == "done" { 1 } else { 0 };
            return (done, 1);
        }
        
        let mut done = 0;
        let mut total = 0;
        for sub in &self.subtasks {
            let (d, t) = sub.completion_stats();
            done += d;
            total += t;
        }
        (done, total)
    }
}
