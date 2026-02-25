use aoc_core::mind_contracts::{
    canonical_json, compact_raw_event_to_t0, ConversationRole, MessageEvent, RawEvent,
    RawEventBody, T0CompactionPolicy, TaskSignalEvent, ToolExecutionStatus, ToolResultEvent,
};
use aoc_storage::{ConversationContextState, IngestionCheckpoint, MindStore, StorageError};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("serialization error: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone)]
pub struct IngestionOptions {
    pub policy: T0CompactionPolicy,
}

impl Default for IngestionOptions {
    fn default() -> Self {
        Self {
            policy: T0CompactionPolicy::default(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IngestionReport {
    pub processed_raw_events: usize,
    pub produced_t0_events: usize,
    pub captured_task_signals: usize,
    pub context_state_snapshots: usize,
    pub skipped_corrupt_lines: usize,
    pub deferred_partial_line: bool,
    pub reset_due_to_truncation: bool,
    pub raw_cursor: u64,
    pub t0_cursor: u64,
}

#[derive(Debug, Default, Clone)]
struct AttributionState {
    active_tag: Option<String>,
    active_tasks: BTreeSet<String>,
}

impl AttributionState {
    fn from_snapshot(snapshot: Option<ConversationContextState>) -> Self {
        let Some(snapshot) = snapshot else {
            return Self::default();
        };
        let active_tasks = snapshot.active_tasks.into_iter().collect();
        Self {
            active_tag: snapshot.active_tag,
            active_tasks,
        }
    }

    fn apply_signal(&mut self, signal: &TaskSignalEvent) {
        if let Some(active_tag) = signal
            .active_tag
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            self.active_tag = Some(active_tag.to_string());
        }

        let lifecycle = signal
            .lifecycle
            .as_deref()
            .unwrap_or_default()
            .to_lowercase();
        if lifecycle.contains("clear") || lifecycle.contains("reset") {
            self.active_tasks.clear();
            return;
        }

        for task_id in signal
            .task_ids
            .iter()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
        {
            if lifecycle.contains("done")
                || lifecycle.contains("completed")
                || lifecycle.contains("cancel")
                || lifecycle.contains("closed")
                || lifecycle.contains("remove")
            {
                self.active_tasks.remove(task_id);
            } else {
                self.active_tasks.insert(task_id.to_string());
            }
        }
    }

    fn snapshot(
        &self,
        conversation_id: &str,
        ts: DateTime<Utc>,
        lifecycle: Option<String>,
        signal_task_ids: Vec<String>,
        signal_source: &str,
    ) -> ConversationContextState {
        ConversationContextState {
            conversation_id: conversation_id.to_string(),
            ts,
            active_tag: self.active_tag.clone(),
            active_tasks: self.active_tasks.iter().cloned().collect(),
            lifecycle,
            signal_task_ids,
            signal_source: signal_source.to_string(),
        }
    }
}

pub struct OpenCodeIngestor {
    options: IngestionOptions,
}

impl OpenCodeIngestor {
    pub fn new(options: IngestionOptions) -> Self {
        Self { options }
    }

    pub fn ingest_conversation_file(
        &self,
        store: &MindStore,
        conversation_id: &str,
        agent_id: &str,
        path: impl AsRef<Path>,
    ) -> Result<IngestionReport, AdapterError> {
        let bytes = fs::read(path)?;
        let checkpoint = store.checkpoint(conversation_id)?;
        let mut attribution_state =
            AttributionState::from_snapshot(store.latest_context_state(conversation_id)?);

        let mut report = IngestionReport::default();
        let mut start_cursor = checkpoint
            .as_ref()
            .map_or(0_u64, |checkpoint| checkpoint.raw_cursor);

        if start_cursor as usize > bytes.len() {
            start_cursor = 0;
            report.reset_due_to_truncation = true;
        }

        let mut consumed: usize = 0;
        let pending = &bytes[start_cursor as usize..];

        while consumed < pending.len() {
            let remaining = &pending[consumed..];
            let Some(newline_index) = remaining.iter().position(|byte| *byte == b'\n') else {
                report.deferred_partial_line = !remaining.is_empty();
                break;
            };

            let line_end = consumed + newline_index;
            let line = &pending[consumed..line_end];
            consumed = line_end + 1;

            if line.is_empty() {
                continue;
            }

            let line_offset = start_cursor as usize + (consumed - (newline_index + 1));

            let parsed: Value = match serde_json::from_slice(line) {
                Ok(parsed) => parsed,
                Err(_) => {
                    report.skipped_corrupt_lines += 1;
                    continue;
                }
            };

            let derived_signal = parse_task_signal_event(parsed.as_object());

            let event = normalize_raw_event(parsed, conversation_id, agent_id, line_offset)
                .map_err(|err| AdapterError::Serialization(err.to_string()))?;

            if store.insert_raw_event(&event)? {
                report.processed_raw_events += 1;
            }

            if let Some(compact) = compact_raw_event_to_t0(&event, &self.options.policy)
                .map_err(|err| AdapterError::Serialization(err.to_string()))?
            {
                store.upsert_t0_compact_event(&compact)?;
                report.produced_t0_events += 1;
            }

            if let Some(signal) = derived_signal.or_else(|| match &event.body {
                RawEventBody::TaskSignal(signal) => Some(signal.clone()),
                _ => None,
            }) {
                attribution_state.apply_signal(&signal);
                let source = signal
                    .signal_source
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("task_signal");
                let snapshot = attribution_state.snapshot(
                    conversation_id,
                    event.ts,
                    signal.lifecycle.clone(),
                    signal.task_ids.clone(),
                    source,
                );
                store.append_context_state(&snapshot)?;
                report.captured_task_signals += 1;
                report.context_state_snapshots += 1;
            }
        }

        let new_cursor = start_cursor + consumed as u64;
        report.raw_cursor = new_cursor;
        report.t0_cursor = new_cursor;

        store.upsert_checkpoint(&IngestionCheckpoint {
            conversation_id: conversation_id.to_string(),
            raw_cursor: report.raw_cursor,
            t0_cursor: report.t0_cursor,
            policy_version: self.options.policy.policy_version.clone(),
            updated_at: Utc::now(),
        })?;

        Ok(report)
    }
}

fn normalize_raw_event(
    value: Value,
    conversation_id: &str,
    agent_id: &str,
    line_offset: usize,
) -> Result<RawEvent, AdapterError> {
    let object = value.as_object();

    let event_id = object
        .and_then(|object| object.get("event_id").or_else(|| object.get("id")))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            let canonical = canonical_json(&value).unwrap_or_else(|_| "{}".to_string());
            let digest =
                sha256_hex(format!("{conversation_id}:{line_offset}:{canonical}").as_bytes());
            format!("evt:{}", &digest[..24])
        });

    let ts = object
        .and_then(|object| object.get("ts").or_else(|| object.get("timestamp")))
        .and_then(Value::as_str)
        .and_then(parse_timestamp)
        .unwrap_or_else(|| fallback_ts(line_offset));

    let body = if let Some(message) = parse_message_event(object) {
        RawEventBody::Message(message)
    } else if let Some(tool_result) = parse_tool_result_event(object) {
        RawEventBody::ToolResult(tool_result)
    } else if let Some(task_signal) = parse_task_signal_event(object) {
        RawEventBody::TaskSignal(task_signal)
    } else {
        RawEventBody::Other {
            payload: value.clone(),
        }
    };

    Ok(RawEvent {
        event_id,
        conversation_id: conversation_id.to_string(),
        agent_id: agent_id.to_string(),
        ts,
        body,
        attrs: Default::default(),
    })
}

fn parse_message_event(object: Option<&serde_json::Map<String, Value>>) -> Option<MessageEvent> {
    let object = object?;
    let role = object
        .get("role")
        .and_then(Value::as_str)
        .and_then(parse_role)?;
    let text = object
        .get("text")
        .or_else(|| object.get("content"))
        .and_then(Value::as_str)
        .map(ToString::to_string)?;

    Some(MessageEvent { role, text })
}

fn parse_tool_result_event(
    object: Option<&serde_json::Map<String, Value>>,
) -> Option<ToolResultEvent> {
    let object = object?;
    let tool_name = object
        .get("tool_name")
        .or_else(|| object.get("tool"))
        .and_then(Value::as_str)
        .map(ToString::to_string)?;

    let status = if let Some(success) = object.get("success").and_then(Value::as_bool) {
        ToolExecutionStatus::from(success)
    } else if let Some(status) = object.get("status").and_then(Value::as_str) {
        match status {
            "success" | "ok" | "passed" => ToolExecutionStatus::Success,
            _ => ToolExecutionStatus::Failure,
        }
    } else {
        ToolExecutionStatus::Success
    };

    let latency_ms = object
        .get("latency_ms")
        .or_else(|| object.get("duration_ms"))
        .and_then(Value::as_u64);
    let exit_code = object
        .get("exit_code")
        .or_else(|| object.get("code"))
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let output = object
        .get("output")
        .or_else(|| object.get("result"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let redacted = object
        .get("redacted")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Some(ToolResultEvent {
        tool_name,
        status,
        latency_ms,
        exit_code,
        output,
        redacted,
    })
}

fn parse_task_signal_event(
    object: Option<&serde_json::Map<String, Value>>,
) -> Option<TaskSignalEvent> {
    let object = object?;

    let mut active_tag = first_string(object, &["active_tag", "tag"]);
    if active_tag.is_none() {
        active_tag = first_nested_string(object, &["current_tag"], "tag");
    }

    let mut lifecycle = first_string(object, &["lifecycle", "action"]);
    let mut task_ids = collect_task_ids(object);
    let mut signal_source = None;

    if let Some(command) = first_string(object, &["command", "cmd"]) {
        if is_taskmaster_command(&command) {
            if command_contains_tag_current_json(&command) {
                if active_tag.is_none() {
                    active_tag = parse_tag_from_tm_current_output(object);
                }
                if lifecycle.is_none() {
                    lifecycle = Some("tag_current".to_string());
                }
                signal_source = Some("tm_tag_current_json".to_string());
            }

            if let Some(parsed) = parse_task_lifecycle_from_command(&command) {
                if parsed.active_tag.is_some() {
                    active_tag = parsed.active_tag;
                }
                if !parsed.task_ids.is_empty() {
                    task_ids = parsed.task_ids;
                }
                if parsed.lifecycle.is_some() {
                    lifecycle = parsed.lifecycle;
                }
                signal_source = Some("task_lifecycle_command".to_string());
            }
        }
    }

    if signal_source.is_none()
        && (object.contains_key("counts")
            || payload_object(object)
                .map(|payload| payload.contains_key("counts"))
                .unwrap_or(false))
        && active_tag.is_some()
    {
        signal_source = Some("task_summary".to_string());
        if lifecycle.is_none() {
            lifecycle = Some("task_summary".to_string());
        }
    }

    if signal_source.is_none()
        && (object.contains_key("task")
            || payload_object(object)
                .map(|payload| payload.contains_key("task"))
                .unwrap_or(false))
        && (object.contains_key("action")
            || payload_object(object)
                .map(|payload| payload.contains_key("action"))
                .unwrap_or(false))
    {
        signal_source = Some("task_update".to_string());
    }

    if active_tag.is_none() && lifecycle.is_none() && task_ids.is_empty() {
        return None;
    }

    task_ids.sort();
    task_ids.dedup();

    Some(TaskSignalEvent {
        active_tag,
        task_ids,
        lifecycle,
        signal_source,
    })
}

#[derive(Debug, Default)]
struct ParsedLifecycleSignal {
    active_tag: Option<String>,
    task_ids: Vec<String>,
    lifecycle: Option<String>,
}

fn payload_object<'a>(
    object: &'a serde_json::Map<String, Value>,
) -> Option<&'a serde_json::Map<String, Value>> {
    object.get("payload").and_then(Value::as_object)
}

fn first_string(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = object.get(*key).and_then(Value::as_str) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    if let Some(payload) = payload_object(object) {
        for key in keys {
            if let Some(value) = payload.get(*key).and_then(Value::as_str) {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }

    None
}

fn first_nested_string(
    object: &serde_json::Map<String, Value>,
    nested_keys: &[&str],
    leaf_key: &str,
) -> Option<String> {
    for nested_key in nested_keys {
        if let Some(value) = object
            .get(*nested_key)
            .and_then(Value::as_object)
            .and_then(|nested| nested.get(leaf_key))
            .and_then(Value::as_str)
        {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    if let Some(payload) = payload_object(object) {
        for nested_key in nested_keys {
            if let Some(value) = payload
                .get(*nested_key)
                .and_then(Value::as_object)
                .and_then(|nested| nested.get(leaf_key))
                .and_then(Value::as_str)
            {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }

    None
}

fn collect_task_ids(object: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut task_ids = Vec::new();

    for value in [
        object.get("task_ids"),
        payload_object(object).and_then(|payload| payload.get("task_ids")),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(array) = value.as_array() {
            for entry in array {
                if let Some(task_id) = task_id_from_value(entry) {
                    task_ids.push(task_id);
                }
            }
        }
    }

    for value in [
        object.get("active_tasks"),
        payload_object(object).and_then(|payload| payload.get("active_tasks")),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(array) = value.as_array() {
            for entry in array {
                if let Some(task_id) = task_id_from_value(entry) {
                    task_ids.push(task_id);
                }
            }
        }
    }

    for value in [
        object.get("task"),
        payload_object(object).and_then(|payload| payload.get("task")),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(task_id) = task_id_from_value(value) {
            task_ids.push(task_id);
        }
    }

    task_ids.sort();
    task_ids.dedup();
    task_ids
}

fn task_id_from_value(value: &Value) -> Option<String> {
    if let Some(task_id) = value.as_str() {
        let trimmed = task_id.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if let Some(task_id) = value.as_i64() {
        return Some(task_id.to_string());
    }

    if let Some(task_object) = value.as_object() {
        if let Some(task_id) = task_object.get("id") {
            return task_id_from_value(task_id);
        }
    }

    None
}

fn is_taskmaster_command(command: &str) -> bool {
    let normalized = command.to_lowercase();
    normalized.contains("tm ")
        || normalized.starts_with("tm")
        || normalized.contains("aoc-task")
        || normalized.contains("taskmaster")
}

fn command_contains_tag_current_json(command: &str) -> bool {
    let normalized = command.to_lowercase();
    normalized.contains("tag") && normalized.contains("current") && normalized.contains("--json")
}

fn parse_tag_from_tm_current_output(object: &serde_json::Map<String, Value>) -> Option<String> {
    let output = first_string(object, &["output", "stdout", "result", "text"])?;
    let parsed: Value = if let Ok(parsed) = serde_json::from_str(&output) {
        parsed
    } else {
        extract_json_object_from_text(&output)?
    };
    parsed
        .get("tag")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn extract_json_object_from_text(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

fn parse_task_lifecycle_from_command(command: &str) -> Option<ParsedLifecycleSignal> {
    let normalized = command.replace('"', " ").replace('\'', " ");
    let tokens = normalized
        .split_whitespace()
        .map(|token| token.trim().to_lowercase())
        .collect::<Vec<String>>();

    if tokens.is_empty() {
        return None;
    }

    let mut parsed = ParsedLifecycleSignal::default();

    if let Some(tag_index) = tokens.iter().position(|token| token == "--tag") {
        if let Some(tag) = tokens.get(tag_index + 1) {
            parsed.active_tag = Some(tag.to_string());
        }
    }

    if let Some(status_index) = tokens.iter().position(|token| token == "status") {
        if let Some(task_id) = tokens.get(status_index + 1) {
            parsed.task_ids.push(task_id.to_string());
        }
        if let Some(status) = tokens.get(status_index + 2) {
            parsed.lifecycle = Some(status.to_string());
        }
    }

    if parsed.task_ids.is_empty() {
        if let Some(done_index) = tokens
            .iter()
            .position(|token| token == "done" || token == "complete" || token == "completed")
        {
            if let Some(task_id) = tokens.get(done_index + 1) {
                parsed.task_ids.push(task_id.to_string());
            }
            parsed.lifecycle = Some("done".to_string());
        }
    }

    if parsed.task_ids.is_empty() {
        if let Some(start_index) = tokens
            .iter()
            .position(|token| token == "start" || token == "resume")
        {
            if let Some(task_id) = tokens.get(start_index + 1) {
                parsed.task_ids.push(task_id.to_string());
            }
            parsed.lifecycle = Some("in-progress".to_string());
        }
    }

    if parsed.active_tag.is_none() && parsed.task_ids.is_empty() && parsed.lifecycle.is_none() {
        return None;
    }

    parsed.task_ids.sort();
    parsed.task_ids.dedup();
    Some(parsed)
}

fn parse_role(value: &str) -> Option<ConversationRole> {
    match value {
        "system" => Some(ConversationRole::System),
        "user" => Some(ConversationRole::User),
        "assistant" => Some(ConversationRole::Assistant),
        "tool" => Some(ConversationRole::Tool),
        _ => None,
    }
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.with_timezone(&Utc))
}

fn fallback_ts(line_offset: usize) -> DateTime<Utc> {
    let secs = i64::try_from(line_offset).unwrap_or(i64::MAX);
    Utc.timestamp_opt(secs, 0).single().unwrap_or_else(Utc::now)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn ingest_is_incremental_and_tolerates_corrupt_and_partial_lines() {
        let db_file = NamedTempFile::new().expect("temp db");
        let store = MindStore::open(db_file.path()).expect("open store");

        let mut log = NamedTempFile::new().expect("temp log");
        writeln!(
            log,
            "{{\"event_id\":\"m1\",\"timestamp\":\"2026-02-23T12:00:00Z\",\"role\":\"user\",\"text\":\"hello\"}}"
        )
        .expect("write message");
        writeln!(log, "{{\"bad_json\"").expect("write corrupt");
        write!(
            log,
            "{{\"event_id\":\"t1\",\"timestamp\":\"2026-02-23T12:00:01Z\",\"tool_name\":\"bash\",\"success\":true,\"output\":\"abc\"}}"
        )
        .expect("write partial tool");
        log.flush().expect("flush");

        let ingestor = OpenCodeIngestor::new(IngestionOptions::default());
        let first = ingestor
            .ingest_conversation_file(&store, "conv-1", "agent-1", log.path())
            .expect("ingest first");

        assert_eq!(first.processed_raw_events, 1);
        assert_eq!(first.produced_t0_events, 1);
        assert_eq!(first.skipped_corrupt_lines, 1);
        assert!(first.deferred_partial_line);
        assert!(!first.reset_due_to_truncation);
        assert_eq!(store.raw_event_count("conv-1").expect("raw count"), 1);
        assert_eq!(store.t0_event_count("conv-1").expect("t0 count"), 1);

        writeln!(log).expect("finish partial line");
        log.flush().expect("flush");

        let second = ingestor
            .ingest_conversation_file(&store, "conv-1", "agent-1", log.path())
            .expect("ingest second");

        assert_eq!(second.processed_raw_events, 1);
        assert_eq!(second.produced_t0_events, 1);
        assert_eq!(second.skipped_corrupt_lines, 0);
        assert!(!second.deferred_partial_line);
        assert_eq!(store.raw_event_count("conv-1").expect("raw count"), 2);
        assert_eq!(store.t0_event_count("conv-1").expect("t0 count"), 2);

        let third = ingestor
            .ingest_conversation_file(&store, "conv-1", "agent-1", log.path())
            .expect("ingest third");
        assert_eq!(third.processed_raw_events, 0);
        assert_eq!(third.produced_t0_events, 0);
    }

    #[test]
    fn ingestion_recovers_if_file_truncates_after_checkpoint() {
        let db_file = NamedTempFile::new().expect("temp db");
        let store = MindStore::open(db_file.path()).expect("open store");

        let mut log = NamedTempFile::new().expect("temp log");
        writeln!(
            log,
            "{{\"event_id\":\"m1\",\"timestamp\":\"2026-02-23T12:00:00Z\",\"role\":\"user\",\"text\":\"first\"}}"
        )
        .expect("write");
        log.flush().expect("flush");

        let ingestor = OpenCodeIngestor::new(IngestionOptions::default());
        let first = ingestor
            .ingest_conversation_file(&store, "conv-2", "agent-1", log.path())
            .expect("ingest first");
        assert!(!first.reset_due_to_truncation);

        fs::write(log.path(), "{\"event_id\":\"m2\"}\n").expect("truncate and rewrite");

        let second = ingestor
            .ingest_conversation_file(&store, "conv-2", "agent-1", log.path())
            .expect("ingest second");
        assert!(second.reset_due_to_truncation);
        assert_eq!(second.processed_raw_events, 1);
        assert_eq!(store.raw_event_count("conv-2").expect("raw count"), 2);
    }

    #[test]
    fn ingest_uses_policy_allowlist_for_t0_snippets() {
        let db_file = NamedTempFile::new().expect("temp db");
        let store = MindStore::open(db_file.path()).expect("open store");

        let mut log = NamedTempFile::new().expect("temp log");
        writeln!(
            log,
            "{{\"event_id\":\"t2\",\"timestamp\":\"2026-02-23T12:00:00Z\",\"tool_name\":\"bash\",\"success\":true,\"output\":\"1234567890\"}}"
        )
        .expect("write");
        log.flush().expect("flush");

        let mut policy = T0CompactionPolicy::default();
        policy.tool_snippet_allowlist.insert("bash".to_string(), 4);
        let ingestor = OpenCodeIngestor::new(IngestionOptions { policy });

        let report = ingestor
            .ingest_conversation_file(&store, "conv-3", "agent-1", log.path())
            .expect("ingest");

        assert_eq!(report.processed_raw_events, 1);
        assert_eq!(report.produced_t0_events, 1);
        assert_eq!(store.t0_event_count("conv-3").expect("t0 count"), 1);
    }

    #[test]
    fn parses_tm_tag_current_json_signal_from_command_output() {
        let parsed = serde_json::json!({
            "tool_name": "bash",
            "command": "tm tag current --json",
            "output": "{\n  \"tag\": \"mind\",\n  \"task_count\": 10\n}",
            "success": true
        });

        let signal = parse_task_signal_event(parsed.as_object()).expect("signal expected");
        assert_eq!(signal.active_tag.as_deref(), Some("mind"));
        assert_eq!(signal.lifecycle.as_deref(), Some("tag_current"));
        assert_eq!(signal.signal_source.as_deref(), Some("tm_tag_current_json"));
    }

    #[test]
    fn context_state_carries_forward_across_restart() {
        let db_file = NamedTempFile::new().expect("temp db");
        let mut log = NamedTempFile::new().expect("temp log");
        writeln!(
            log,
            "{{\"event_id\":\"s1\",\"timestamp\":\"2026-02-23T12:00:00Z\",\"tool_name\":\"bash\",\"command\":\"tm tag current --json\",\"output\":\"{{\\\"tag\\\":\\\"mind\\\",\\\"task_count\\\":10}}\"}}"
        )
        .expect("write first signal");
        writeln!(
            log,
            "{{\"event_id\":\"s2\",\"timestamp\":\"2026-02-23T12:00:01Z\",\"tool_name\":\"bash\",\"command\":\"aoc-task status 101 in-progress --tag mind\",\"output\":\"ok\"}}"
        )
        .expect("write second signal");
        log.flush().expect("flush log");

        let ingestor = OpenCodeIngestor::new(IngestionOptions::default());
        let store = MindStore::open(db_file.path()).expect("open store");

        let first = ingestor
            .ingest_conversation_file(&store, "conv-ctx", "agent-1", log.path())
            .expect("first ingest");
        assert_eq!(first.captured_task_signals, 2);

        let snapshot = store
            .latest_context_state("conv-ctx")
            .expect("latest state")
            .expect("snapshot exists");
        assert_eq!(snapshot.active_tag.as_deref(), Some("mind"));
        assert_eq!(snapshot.active_tasks, vec!["101".to_string()]);

        drop(store);

        let mut append = fs::OpenOptions::new()
            .append(true)
            .open(log.path())
            .expect("reopen log");
        writeln!(
            append,
            "{{\"event_id\":\"s3\",\"timestamp\":\"2026-02-23T12:00:02Z\",\"tool_name\":\"bash\",\"command\":\"tm done 101 --tag mind\",\"output\":\"done\"}}"
        )
        .expect("append completion signal");
        append.flush().expect("flush append");

        let restarted_store = MindStore::open(db_file.path()).expect("reopen store");
        let second = ingestor
            .ingest_conversation_file(&restarted_store, "conv-ctx", "agent-1", log.path())
            .expect("second ingest");
        assert_eq!(second.processed_raw_events, 1);
        assert_eq!(second.captured_task_signals, 1);

        let updated_snapshot = restarted_store
            .latest_context_state("conv-ctx")
            .expect("latest state")
            .expect("snapshot exists");
        assert_eq!(updated_snapshot.active_tag.as_deref(), Some("mind"));
        assert!(updated_snapshot.active_tasks.is_empty());
    }
}
