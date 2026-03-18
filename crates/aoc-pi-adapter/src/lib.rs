use aoc_core::mind_contracts::{
    build_compaction_t0_slice, canonical_json, compact_raw_event_to_t0,
    sanitize_raw_event_for_storage, sha256_hex, ConversationRole, MessageEvent, RawEvent,
    RawEventBody, T0CompactionPolicy, ToolExecutionStatus, ToolResultEvent, LINEAGE_ATTRS_KEY,
};
use aoc_storage::{CompactionCheckpoint, IngestionCheckpoint, MindStore, StorageError};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PiAdapterError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("invalid session header: {0}")]
    InvalidSessionHeader(String),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiSessionSource {
    pub session_id: String,
    pub conversation_id: String,
    pub cwd: Option<String>,
    pub version: Option<u32>,
    pub parent_session: Option<String>,
    pub session_file_path: String,
    pub header_end_cursor: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IngestionReport {
    pub session_id: String,
    pub conversation_id: String,
    pub processed_raw_events: usize,
    pub produced_t0_events: usize,
    pub persisted_compaction_checkpoints: usize,
    pub skipped_corrupt_lines: usize,
    pub deferred_partial_line: bool,
    pub reset_due_to_truncation: bool,
    pub raw_cursor: u64,
    pub t0_cursor: u64,
}

pub struct PiSessionIngestor {
    options: IngestionOptions,
}

impl PiSessionIngestor {
    pub fn new(options: IngestionOptions) -> Self {
        Self { options }
    }

    pub fn inspect_session_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<PiSessionSource, PiAdapterError> {
        let bytes = fs::read(path.as_ref())?;
        parse_session_source(path.as_ref(), &bytes)
    }

    pub fn ingest_session_file(
        &self,
        store: &MindStore,
        agent_id: &str,
        path: impl AsRef<Path>,
    ) -> Result<IngestionReport, PiAdapterError> {
        let path = path.as_ref();
        let bytes = fs::read(path)?;
        let source = parse_session_source(path, &bytes)?;
        let checkpoint = store.checkpoint(&source.conversation_id)?;

        let mut report = IngestionReport {
            session_id: source.session_id.clone(),
            conversation_id: source.conversation_id.clone(),
            ..IngestionReport::default()
        };

        let mut start_cursor = checkpoint
            .as_ref()
            .map_or(source.header_end_cursor, |checkpoint| checkpoint.raw_cursor);

        if start_cursor as usize > bytes.len() {
            start_cursor = source.header_end_cursor;
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

            let event = normalize_entry(parsed, &source, agent_id, line_offset)
                .map_err(|err| PiAdapterError::Serialization(err.to_string()))?;
            let event = sanitize_raw_event_for_storage(&event);

            let inserted = store.insert_raw_event(&event)?;
            if inserted {
                report.processed_raw_events += 1;
            }

            if let Some(compact) = compact_raw_event_to_t0(&event, &self.options.policy)
                .map_err(|err| PiAdapterError::Serialization(err.to_string()))?
            {
                store.upsert_t0_compact_event(&compact)?;
                if inserted {
                    report.produced_t0_events += 1;
                }
            }

            if let Some(checkpoint) = parse_compaction_checkpoint(&event, &source) {
                store.upsert_compaction_checkpoint(&checkpoint)?;
                if let Some(slice) = parse_compaction_t0_slice(&event, &checkpoint) {
                    store.upsert_compaction_t0_slice(&slice)?;
                }
                if inserted {
                    report.persisted_compaction_checkpoints += 1;
                }
            }
        }

        let new_cursor = start_cursor + consumed as u64;
        report.raw_cursor = new_cursor;
        report.t0_cursor = new_cursor;

        store.upsert_checkpoint(&IngestionCheckpoint {
            conversation_id: source.conversation_id.clone(),
            raw_cursor: report.raw_cursor,
            t0_cursor: report.t0_cursor,
            policy_version: self.options.policy.policy_version.clone(),
            updated_at: Utc::now(),
        })?;

        Ok(report)
    }
}

fn parse_session_source(path: &Path, bytes: &[u8]) -> Result<PiSessionSource, PiAdapterError> {
    let Some(newline_index) = bytes.iter().position(|byte| *byte == b'\n') else {
        return Err(PiAdapterError::InvalidSessionHeader(
            "session file missing newline-terminated header".to_string(),
        ));
    };

    let header: Value = serde_json::from_slice(&bytes[..newline_index])
        .map_err(|err| PiAdapterError::InvalidSessionHeader(err.to_string()))?;
    let object = header.as_object().ok_or_else(|| {
        PiAdapterError::InvalidSessionHeader("header must be a JSON object".to_string())
    })?;

    let entry_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if entry_type != "session" {
        return Err(PiAdapterError::InvalidSessionHeader(format!(
            "expected type=session, found {entry_type}"
        )));
    }

    let session_file_path = path.to_string_lossy().to_string();
    let session_id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            let digest = sha256_hex(session_file_path.as_bytes());
            format!("pi-session:{}", &digest[..24])
        });

    Ok(PiSessionSource {
        conversation_id: format!("pi:{session_id}"),
        session_id,
        cwd: object
            .get("cwd")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        version: object
            .get("version")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        parent_session: object
            .get("parentSession")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        session_file_path,
        header_end_cursor: (newline_index + 1) as u64,
    })
}

fn normalize_entry(
    value: Value,
    source: &PiSessionSource,
    agent_id: &str,
    line_offset: usize,
) -> Result<RawEvent, PiAdapterError> {
    let object = value.as_object();
    let event_id = object
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .map(|id| format!("pi:{}", id.trim()))
        .filter(|id| id != "pi:")
        .unwrap_or_else(|| {
            let canonical = canonical_json(&value).unwrap_or_else(|_| "{}".to_string());
            let digest = sha256_hex(
                format!("{}:{}:{canonical}", source.session_file_path, line_offset).as_bytes(),
            );
            format!("pi:evt:{}", &digest[..24])
        });

    let ts = object
        .and_then(|object| object.get("timestamp"))
        .and_then(Value::as_str)
        .and_then(parse_timestamp)
        .unwrap_or_else(|| fallback_ts(line_offset));

    let mut attrs = build_source_attrs(object, source, &event_id);
    let body = normalize_body(object, &value, &mut attrs)?;

    Ok(RawEvent {
        event_id,
        conversation_id: source.conversation_id.clone(),
        agent_id: agent_id.to_string(),
        ts,
        body,
        attrs,
    })
}

fn normalize_body(
    object: Option<&Map<String, Value>>,
    original: &Value,
    attrs: &mut BTreeMap<String, Value>,
) -> Result<RawEventBody, PiAdapterError> {
    let object =
        object.ok_or_else(|| PiAdapterError::Serialization("entry must be object".into()))?;
    let entry_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match entry_type {
        "message" => normalize_message_entry(object, attrs),
        "compaction" => {
            enrich_compaction_attrs(object, attrs);
            Ok(RawEventBody::Other {
                payload: original.clone(),
            })
        }
        "branch_summary" => {
            enrich_branch_summary_attrs(object, attrs);
            Ok(RawEventBody::Other {
                payload: original.clone(),
            })
        }
        "custom" => {
            enrich_custom_entry_attrs(object, attrs);
            Ok(RawEventBody::Other {
                payload: original.clone(),
            })
        }
        "model_change" | "thinking_level_change" => {
            enrich_metadata_entry_attrs(object, attrs);
            Ok(RawEventBody::Other {
                payload: original.clone(),
            })
        }
        _ => Ok(RawEventBody::Other {
            payload: original.clone(),
        }),
    }
}

fn normalize_message_entry(
    object: &Map<String, Value>,
    attrs: &mut BTreeMap<String, Value>,
) -> Result<RawEventBody, PiAdapterError> {
    let message = object
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PiAdapterError::Serialization("message entry missing message object".into())
        })?;
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or_default();
    attrs.insert(
        "pi_message_role".to_string(),
        Value::String(role.to_string()),
    );

    match role {
        "user" => {
            enrich_message_content_attrs(message, attrs);
            Ok(RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: collect_text_content(message.get("content")),
            }))
        }
        "assistant" => {
            enrich_message_content_attrs(message, attrs);
            Ok(RawEventBody::Message(MessageEvent {
                role: ConversationRole::Assistant,
                text: collect_text_blocks_only(message.get("content")),
            }))
        }
        "toolResult" => {
            enrich_message_content_attrs(message, attrs);
            Ok(RawEventBody::ToolResult(ToolResultEvent {
                tool_name: message
                    .get("toolName")
                    .and_then(Value::as_str)
                    .unwrap_or("tool_result")
                    .to_string(),
                status: ToolExecutionStatus::from(
                    !message
                        .get("isError")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                ),
                latency_ms: None,
                exit_code: None,
                output: optional_text(collect_text_content(message.get("content"))),
                redacted: false,
            }))
        }
        "bashExecution" => {
            if let Some(command) = message.get("command").and_then(Value::as_str) {
                attrs.insert("pi_command".to_string(), Value::String(command.to_string()));
            }
            for (key, attr_key) in [
                ("cancelled", "pi_cancelled"),
                ("truncated", "pi_truncated"),
                ("excludeFromContext", "pi_exclude_from_context"),
            ] {
                if let Some(value) = message.get(key).and_then(Value::as_bool) {
                    attrs.insert(attr_key.to_string(), Value::Bool(value));
                }
            }
            if let Some(path) = message.get("fullOutputPath").and_then(Value::as_str) {
                attrs.insert(
                    "pi_full_output_path".to_string(),
                    Value::String(path.to_string()),
                );
            }

            let exit_code = message
                .get("exitCode")
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok());
            let cancelled = message
                .get("cancelled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let status = match (cancelled, exit_code) {
                (true, _) => ToolExecutionStatus::Failure,
                (false, Some(0)) | (false, None) => ToolExecutionStatus::Success,
                (false, Some(_)) => ToolExecutionStatus::Failure,
            };

            Ok(RawEventBody::ToolResult(ToolResultEvent {
                tool_name: "bash_execution".to_string(),
                status,
                latency_ms: None,
                exit_code,
                output: message
                    .get("output")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                redacted: false,
            }))
        }
        "custom" => {
            if let Some(custom_type) = message.get("customType").and_then(Value::as_str) {
                attrs.insert(
                    "pi_custom_message_type".to_string(),
                    Value::String(custom_type.to_string()),
                );
            }
            if let Some(display) = message.get("display") {
                attrs.insert("pi_custom_message_display".to_string(), display.clone());
            }
            if let Some(details) = message.get("details") {
                attrs.insert("pi_custom_message_details".to_string(), details.clone());
            }
            enrich_message_content_attrs(message, attrs);
            Ok(RawEventBody::Other {
                payload: Value::Object(object.clone()),
            })
        }
        _ => Ok(RawEventBody::Other {
            payload: Value::Object(object.clone()),
        }),
    }
}

fn enrich_message_content_attrs(message: &Map<String, Value>, attrs: &mut BTreeMap<String, Value>) {
    let Some(Value::Array(content)) = message.get("content") else {
        if let Some(Value::String(text)) = message.get("content") {
            attrs.insert("pi_text_block_count".to_string(), Value::Number(1.into()));
            attrs.insert(
                "pi_message_text_bytes".to_string(),
                Value::Number((text.len() as u64).into()),
            );
        }
        return;
    };

    let mut text_blocks = 0_u64;
    let mut thinking_blocks = 0_u64;
    let mut tool_call_blocks = 0_u64;
    let mut non_text_blocks = 0_u64;

    for block in content {
        let Some(object) = block.as_object() else {
            non_text_blocks += 1;
            continue;
        };
        match object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text" => text_blocks += 1,
            "thinking" => {
                thinking_blocks += 1;
                non_text_blocks += 1;
            }
            "tool_call" => {
                tool_call_blocks += 1;
                non_text_blocks += 1;
            }
            _ => non_text_blocks += 1,
        }
    }

    attrs.insert(
        "pi_text_block_count".to_string(),
        Value::Number(text_blocks.into()),
    );
    if thinking_blocks > 0 {
        attrs.insert(
            "pi_thinking_block_count".to_string(),
            Value::Number(thinking_blocks.into()),
        );
        attrs.insert("pi_has_thinking_blocks".to_string(), Value::Bool(true));
    }
    if tool_call_blocks > 0 {
        attrs.insert(
            "pi_tool_call_block_count".to_string(),
            Value::Number(tool_call_blocks.into()),
        );
        attrs.insert("pi_has_tool_call_blocks".to_string(), Value::Bool(true));
    }
    if non_text_blocks > 0 {
        attrs.insert(
            "pi_non_text_block_count".to_string(),
            Value::Number(non_text_blocks.into()),
        );
        attrs.insert("pi_has_non_text_blocks".to_string(), Value::Bool(true));
    }
}

fn enrich_compaction_attrs(object: &Map<String, Value>, attrs: &mut BTreeMap<String, Value>) {
    if let Some(summary) = object.get("summary").and_then(Value::as_str) {
        attrs.insert(
            "pi_compaction_summary".to_string(),
            Value::String(summary.to_string()),
        );
    }
    if let Some(first_kept) = object.get("firstKeptEntryId").and_then(Value::as_str) {
        attrs.insert(
            "pi_first_kept_entry_id".to_string(),
            Value::String(first_kept.to_string()),
        );
    }
    if let Some(tokens_before) = object.get("tokensBefore").and_then(Value::as_u64) {
        attrs.insert(
            "pi_tokens_before".to_string(),
            Value::Number(tokens_before.into()),
        );
    }
    if let Some(from_hook) = object.get("fromHook").and_then(Value::as_bool) {
        attrs.insert("pi_from_hook".to_string(), Value::Bool(from_hook));
    }
    preserve_details_attrs(object.get("details"), attrs);
}

fn enrich_branch_summary_attrs(object: &Map<String, Value>, attrs: &mut BTreeMap<String, Value>) {
    if let Some(summary) = object.get("summary").and_then(Value::as_str) {
        attrs.insert(
            "pi_branch_summary".to_string(),
            Value::String(summary.to_string()),
        );
    }
    if let Some(from_id) = object.get("fromId").and_then(Value::as_str) {
        attrs.insert(
            "pi_branch_from_id".to_string(),
            Value::String(from_id.to_string()),
        );
    }
    if let Some(from_hook) = object.get("fromHook").and_then(Value::as_bool) {
        attrs.insert("pi_from_hook".to_string(), Value::Bool(from_hook));
    }
    preserve_details_attrs(object.get("details"), attrs);
}

fn enrich_custom_entry_attrs(object: &Map<String, Value>, attrs: &mut BTreeMap<String, Value>) {
    if let Some(name) = object.get("name").and_then(Value::as_str) {
        attrs.insert(
            "pi_custom_name".to_string(),
            Value::String(name.to_string()),
        );
    }
    if let Some(payload) = object.get("payload") {
        attrs.insert("pi_custom_payload".to_string(), payload.clone());
    }
}

fn enrich_metadata_entry_attrs(object: &Map<String, Value>, attrs: &mut BTreeMap<String, Value>) {
    for key in ["model", "thinkingLevel", "provider", "reason"] {
        if let Some(value) = object.get(key) {
            attrs.insert(format!("pi_{key}"), value.clone());
        }
    }
}

fn preserve_details_attrs(details: Option<&Value>, attrs: &mut BTreeMap<String, Value>) {
    let Some(details) = details.and_then(Value::as_object) else {
        return;
    };

    if let Some(read_files) = string_array(details.get("readFiles")) {
        attrs.insert(
            "pi_detail_read_files".to_string(),
            Value::Array(read_files.clone()),
        );
        attrs.insert(
            "pi_detail_read_file_count".to_string(),
            Value::Number((read_files.len() as u64).into()),
        );
    }

    if let Some(modified_files) = string_array(details.get("modifiedFiles")) {
        attrs.insert(
            "pi_detail_modified_files".to_string(),
            Value::Array(modified_files.clone()),
        );
        attrs.insert(
            "pi_detail_modified_file_count".to_string(),
            Value::Number((modified_files.len() as u64).into()),
        );
    }

    attrs.insert(
        "pi_details".to_string(),
        Value::Object(
            details
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
    );
}

fn string_array(value: Option<&Value>) -> Option<Vec<Value>> {
    let array = value?.as_array()?;
    let values = array
        .iter()
        .filter_map(|value| value.as_str().map(|s| Value::String(s.to_string())))
        .collect::<Vec<_>>();
    Some(values)
}

fn parse_compaction_checkpoint(
    event: &RawEvent,
    source: &PiSessionSource,
) -> Option<CompactionCheckpoint> {
    let RawEventBody::Other { payload } = &event.body else {
        return None;
    };
    let object = payload.as_object()?;
    if object.get("type").and_then(Value::as_str)? != "compaction" {
        return None;
    }

    let compaction_entry_id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let checkpoint_id = compaction_entry_id
        .as_deref()
        .map(|id| format!("cmpchk:{}:{id}", source.conversation_id))
        .unwrap_or_else(|| format!("cmpchk:{}:{}", source.conversation_id, event.ts.timestamp()));

    Some(CompactionCheckpoint {
        checkpoint_id,
        conversation_id: source.conversation_id.clone(),
        session_id: source.session_id.clone(),
        ts: event.ts,
        trigger_source: "pi_compact_import".to_string(),
        reason: Some("pi_session_import".to_string()),
        summary: object
            .get("summary")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        tokens_before: object
            .get("tokensBefore")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        first_kept_entry_id: object
            .get("firstKeptEntryId")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        compaction_entry_id,
        from_extension: object
            .get("fromHook")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        marker_event_id: Some(event.event_id.clone()),
        schema_version: 1,
        created_at: event.ts,
        updated_at: event.ts,
    })
}

fn parse_compaction_t0_slice(
    event: &RawEvent,
    checkpoint: &CompactionCheckpoint,
) -> Option<aoc_core::mind_contracts::CompactionT0Slice> {
    let read_files = string_list_attr(&event.attrs, "pi_detail_read_files");
    let modified_files = string_list_attr(&event.attrs, "pi_detail_modified_files");
    build_compaction_t0_slice(
        &checkpoint.conversation_id,
        &checkpoint.session_id,
        checkpoint.ts,
        &checkpoint.trigger_source,
        checkpoint.reason.as_deref(),
        checkpoint.summary.as_deref(),
        checkpoint.tokens_before,
        checkpoint.first_kept_entry_id.as_deref(),
        checkpoint.compaction_entry_id.as_deref(),
        checkpoint.from_extension,
        "pi_compaction_checkpoint",
        &[event.event_id.clone()],
        &read_files,
        &modified_files,
        Some(&checkpoint.checkpoint_id),
        "t0.compaction.v1",
    )
    .ok()
}

fn string_list_attr(attrs: &BTreeMap<String, Value>, key: &str) -> Vec<String> {
    attrs
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(|s| s.trim().to_string()))
        .filter(|value| !value.is_empty())
        .collect()
}

fn build_source_attrs(
    object: Option<&Map<String, Value>>,
    source: &PiSessionSource,
    event_id: &str,
) -> BTreeMap<String, Value> {
    let mut attrs = BTreeMap::new();
    attrs.insert(
        "pi_session_id".to_string(),
        Value::String(source.session_id.clone()),
    );
    attrs.insert(
        "pi_session_file_path".to_string(),
        Value::String(source.session_file_path.clone()),
    );
    attrs.insert(
        "pi_conversation_source".to_string(),
        Value::String(source.conversation_id.clone()),
    );
    attrs.insert(
        "pi_imported_event_id".to_string(),
        Value::String(event_id.to_string()),
    );

    if let Some(version) = source.version {
        attrs.insert(
            "pi_session_version".to_string(),
            Value::Number(version.into()),
        );
    }
    if let Some(cwd) = &source.cwd {
        attrs.insert("pi_cwd".to_string(), Value::String(cwd.clone()));
    }
    if let Some(parent_session) = &source.parent_session {
        attrs.insert(
            "pi_parent_session".to_string(),
            Value::String(parent_session.clone()),
        );
    }

    if let Some(object) = object {
        if let Some(entry_id) = object.get("id").and_then(Value::as_str) {
            attrs.insert(
                "pi_entry_id".to_string(),
                Value::String(entry_id.to_string()),
            );
        }
        if let Some(parent_id) = object.get("parentId").and_then(Value::as_str) {
            attrs.insert(
                "pi_parent_entry_id".to_string(),
                Value::String(parent_id.to_string()),
            );
        }
        if let Some(entry_type) = object.get("type").and_then(Value::as_str) {
            attrs.insert(
                "pi_entry_type".to_string(),
                Value::String(entry_type.to_string()),
            );
        }
    }

    attrs.insert(
        LINEAGE_ATTRS_KEY.to_string(),
        json!({
            "session_id": source.session_id,
            "root_conversation_id": source.conversation_id,
        }),
    );

    attrs
}

fn collect_text_content(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|value| match value {
                Value::Object(object)
                    if object.get("type").and_then(Value::as_str) == Some("text") =>
                {
                    object
                        .get("text")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn collect_text_blocks_only(value: Option<&Value>) -> String {
    collect_text_content(value)
}

fn optional_text(text: String) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn fallback_ts(line_offset: usize) -> DateTime<Utc> {
    Utc.timestamp_opt(line_offset as i64, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    fn write_session(contents: &str) -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp file");
        fs::write(file.path(), contents).expect("write session");
        file
    }

    fn open_store_file() -> NamedTempFile {
        NamedTempFile::new().expect("temp db")
    }

    fn raw_event_attrs_json(db_path: &Path, event_id: &str) -> Value {
        let conn = Connection::open(db_path).expect("open sqlite");
        let attrs_json: String = conn
            .query_row(
                "SELECT attrs_json FROM raw_events WHERE event_id = ?1",
                [event_id],
                |row| row.get(0),
            )
            .expect("attrs row");
        serde_json::from_str(&attrs_json).expect("attrs json")
    }

    #[test]
    fn inspect_session_file_derives_identity() {
        let file = write_session(
            r#"{"type":"session","version":3,"id":"sess-1","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"m1","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"hello"}}
"#,
        );

        let ingestor = PiSessionIngestor::new(IngestionOptions::default());
        let source = ingestor.inspect_session_file(file.path()).expect("inspect");
        assert_eq!(source.session_id, "sess-1");
        assert_eq!(source.conversation_id, "pi:sess-1");
        assert_eq!(source.cwd.as_deref(), Some("/tmp/proj"));
        assert!(source.header_end_cursor > 0);
    }

    #[test]
    fn ingest_session_file_is_incremental_and_persists_compaction_checkpoint() {
        let file = write_session(
            r#"{"type":"session","version":3,"id":"sess-2","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"u1","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"hello"}}
{"type":"message","id":"a1","parentId":"u1","timestamp":"2024-12-03T14:00:02.000Z","message":{"role":"assistant","content":[{"type":"text","text":"hi"},{"type":"thinking","thinking":"hidden"}]}}
{"type":"compaction","id":"c1","parentId":"a1","timestamp":"2024-12-03T14:10:00.000Z","summary":"summary","firstKeptEntryId":"a1","tokensBefore":123,"details":{"readFiles":["src/lib.rs"],"modifiedFiles":["src/main.rs"]}}
"#,
        );

        let store = MindStore::open_in_memory().expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());

        let first = ingestor
            .ingest_session_file(&store, "agent-1", file.path())
            .expect("first ingest");
        assert_eq!(first.conversation_id, "pi:sess-2");
        assert_eq!(first.processed_raw_events, 3);
        assert_eq!(first.produced_t0_events, 2);
        assert_eq!(first.persisted_compaction_checkpoints, 1);
        assert_eq!(store.raw_event_count("pi:sess-2").expect("raw count"), 3);
        assert_eq!(store.t0_event_count("pi:sess-2").expect("t0 count"), 2);
        assert!(store
            .latest_compaction_checkpoint_for_session("sess-2")
            .expect("latest checkpoint")
            .is_some());
        let slice = store
            .latest_compaction_t0_slice_for_session("sess-2")
            .expect("latest compaction slice")
            .expect("compaction slice exists");
        assert_eq!(slice.compaction_entry_id.as_deref(), Some("c1"));
        assert_eq!(slice.read_files, vec!["src/lib.rs".to_string()]);
        assert_eq!(slice.modified_files, vec!["src/main.rs".to_string()]);
        assert_eq!(slice.source_event_ids, vec!["pi:c1".to_string()]);

        let second = ingestor
            .ingest_session_file(&store, "agent-1", file.path())
            .expect("second ingest");
        assert_eq!(second.processed_raw_events, 0);
        assert_eq!(second.produced_t0_events, 0);
        assert_eq!(second.persisted_compaction_checkpoints, 0);
        assert_eq!(store.raw_event_count("pi:sess-2").expect("raw count"), 3);
    }

    #[test]
    fn tool_output_is_dropped_before_raw_storage() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-secret","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"tool-1","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"toolResult","toolName":"bash","content":"ANTHROPIC_AUTH_TOKEN=super-secret-value"}}
"#,
        );
        let store = MindStore::open_in_memory().expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());
        ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("ingest");

        let raw = store
            .raw_event_by_id("pi:tool-1")
            .expect("query")
            .expect("event exists");
        let RawEventBody::ToolResult(tool) = raw.body else {
            panic!("expected tool result");
        };
        assert_eq!(tool.output, None);
        assert!(tool.redacted);
    }

    #[test]
    fn imported_attrs_preserve_compaction_details_and_message_block_metadata() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-3","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"a2","parentId":null,"timestamp":"2024-12-03T14:00:02.000Z","message":{"role":"assistant","content":[{"type":"text","text":"hi"},{"type":"thinking","thinking":"hidden"},{"type":"tool_call","toolName":"read"}]}}
{"type":"branch_summary","id":"b1","parentId":"a2","timestamp":"2024-12-03T14:05:00.000Z","summary":"branch summary","fromId":"a2","fromHook":true,"details":{"readFiles":["src/lib.rs"],"modifiedFiles":["src/main.rs","README.md"]}}
"#,
        );
        let db_file = open_store_file();
        let store = MindStore::open(db_file.path()).expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());
        ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("ingest");
        drop(store);

        let assistant_attrs = raw_event_attrs_json(db_file.path(), "pi:a2");
        assert_eq!(assistant_attrs["pi_message_role"], "assistant");
        assert_eq!(assistant_attrs["pi_has_thinking_blocks"], true);
        assert_eq!(assistant_attrs["pi_has_tool_call_blocks"], true);
        assert_eq!(assistant_attrs["pi_text_block_count"], 1);

        let branch_attrs = raw_event_attrs_json(db_file.path(), "pi:b1");
        assert_eq!(branch_attrs["pi_branch_summary"], "branch summary");
        assert_eq!(branch_attrs["pi_branch_from_id"], "a2");
        assert_eq!(branch_attrs["pi_detail_read_file_count"], 1);
        assert_eq!(branch_attrs["pi_detail_modified_file_count"], 2);
        assert_eq!(branch_attrs["pi_detail_read_files"][0], "src/lib.rs");
        assert_eq!(branch_attrs["pi_detail_modified_files"][1], "README.md");
    }

    #[test]
    fn imported_compaction_slice_rebuilds_deterministically_from_checkpoint_and_raw_attrs() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-rebuild","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"compaction","id":"c-rebuild","timestamp":"2024-12-03T14:10:00.000Z","summary":"summary","firstKeptEntryId":"u1","tokensBefore":123,"fromHook":true,"details":{"readFiles":["src/lib.rs"],"modifiedFiles":["src/main.rs","README.md"]}}
"#,
        );
        let db_file = open_store_file();
        let store = MindStore::open(db_file.path()).expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());

        ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("ingest");

        let checkpoint = store
            .latest_compaction_checkpoint_for_session("sess-rebuild")
            .expect("latest checkpoint")
            .expect("checkpoint exists");
        let stored_slice = store
            .compaction_t0_slice_for_checkpoint(&checkpoint.checkpoint_id)
            .expect("stored slice lookup")
            .expect("stored slice exists");
        let marker_event = store
            .raw_event_by_id(
                checkpoint
                    .marker_event_id
                    .as_deref()
                    .expect("marker event id"),
            )
            .expect("load marker event")
            .expect("marker event exists");

        let rebuilt = parse_compaction_t0_slice(&marker_event, &checkpoint).expect("rebuild slice");
        assert_eq!(rebuilt.slice_id, stored_slice.slice_id);
        assert_eq!(rebuilt.slice_hash, stored_slice.slice_hash);
        assert_eq!(rebuilt.read_files, stored_slice.read_files);
        assert_eq!(rebuilt.modified_files, stored_slice.modified_files);
        assert_eq!(rebuilt.source_event_ids, stored_slice.source_event_ids);
    }

    #[test]
    fn imported_long_session_compaction_retains_evidence_across_reingest_and_rebuild() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-long","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"u1","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"start long session"}}
{"type":"branch_summary","id":"b1","parentId":"u1","timestamp":"2024-12-03T14:05:00.000Z","summary":"prep","fromId":"u1","details":{"readFiles":["src/lib.rs","docs/plan.md"],"modifiedFiles":["src/main.rs"]}}
{"type":"compaction","id":"c1","parentId":"b1","timestamp":"2024-12-03T14:10:00.000Z","summary":"first compact","firstKeptEntryId":"b1","tokensBefore":100,"fromHook":true,"details":{"readFiles":["src/lib.rs","docs/plan.md"],"modifiedFiles":["src/main.rs","README.md"]}}
"#,
        );
        let store = MindStore::open_in_memory().expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());

        let first = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("first ingest");
        assert_eq!(first.persisted_compaction_checkpoints, 1);

        let appended = format!(
            "{}{}",
            fs::read_to_string(session.path()).expect("read"),
            concat!(
                "{\"type\":\"message\",\"id\":\"u2\",\"parentId\":\"c1\",\"timestamp\":\"2024-12-03T14:15:00.000Z\",\"message\":{\"role\":\"user\",\"content\":\"continue after first compact\"}}\n",
                "{\"type\":\"compaction\",\"id\":\"c2\",\"parentId\":\"u2\",\"timestamp\":\"2024-12-03T14:20:00.000Z\",\"summary\":\"second compact\",\"firstKeptEntryId\":\"u2\",\"tokensBefore\":80,\"fromHook\":true,\"details\":{\"readFiles\":[\"src/lib.rs\",\"docs/plan.md\",\"src/feature.rs\"],\"modifiedFiles\":[\"src/main.rs\",\"README.md\",\"src/feature.rs\"]}}\n"
            )
        );
        fs::write(session.path(), appended).expect("append rewrite");

        let second = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("second ingest");
        assert_eq!(second.persisted_compaction_checkpoints, 1);

        let checkpoint = store
            .latest_compaction_checkpoint_for_session("sess-long")
            .expect("latest checkpoint")
            .expect("checkpoint exists");
        assert_eq!(checkpoint.compaction_entry_id.as_deref(), Some("c2"));

        let stored_slice = store
            .compaction_t0_slice_for_checkpoint(&checkpoint.checkpoint_id)
            .expect("stored slice lookup")
            .expect("stored slice exists");
        assert_eq!(stored_slice.read_files.len(), 3);
        assert_eq!(stored_slice.modified_files.len(), 3);
        assert!(stored_slice
            .read_files
            .iter()
            .any(|path| path == "docs/plan.md"));
        assert!(stored_slice
            .modified_files
            .iter()
            .any(|path| path == "src/feature.rs"));

        let marker_event = store
            .raw_event_by_id(
                checkpoint
                    .marker_event_id
                    .as_deref()
                    .expect("marker event id"),
            )
            .expect("load marker event")
            .expect("marker event exists");
        let rebuilt = parse_compaction_t0_slice(&marker_event, &checkpoint).expect("rebuild slice");
        assert_eq!(rebuilt.slice_hash, stored_slice.slice_hash);
        assert_eq!(rebuilt.read_files, stored_slice.read_files);
        assert_eq!(rebuilt.modified_files, stored_slice.modified_files);
        assert_eq!(rebuilt.source_event_ids, stored_slice.source_event_ids);
    }

    #[test]
    fn repeated_compactions_in_same_session_append_incrementally() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-4","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"u1","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"hello"}}
{"type":"compaction","id":"c1","parentId":"u1","timestamp":"2024-12-03T14:10:00.000Z","summary":"first compact","firstKeptEntryId":"u1","tokensBefore":100}
"#,
        );
        let store = MindStore::open_in_memory().expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());

        let first = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("first ingest");
        assert_eq!(first.processed_raw_events, 2);
        assert_eq!(first.persisted_compaction_checkpoints, 1);

        let appended = format!(
            "{}{}",
            fs::read_to_string(session.path()).expect("read"),
            concat!(
                "{\"type\":\"branch_summary\",\"id\":\"b2\",\"parentId\":\"c1\",\"timestamp\":\"2024-12-03T14:11:00.000Z\",\"summary\":\"branch\",\"fromId\":\"c1\"}\n",
                "{\"type\":\"compaction\",\"id\":\"c2\",\"parentId\":\"b2\",\"timestamp\":\"2024-12-03T14:20:00.000Z\",\"summary\":\"second compact\",\"firstKeptEntryId\":\"b2\",\"tokensBefore\":80}\n"
            )
        );
        fs::write(session.path(), appended).expect("append rewrite");

        let second = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("second ingest");
        assert_eq!(second.processed_raw_events, 2);
        assert_eq!(second.persisted_compaction_checkpoints, 1);
        assert_eq!(store.raw_event_count("pi:sess-4").expect("raw count"), 4);
        let latest = store
            .latest_compaction_checkpoint_for_session("sess-4")
            .expect("latest checkpoint")
            .expect("checkpoint exists");
        assert_eq!(latest.compaction_entry_id.as_deref(), Some("c2"));
        assert_eq!(latest.summary.as_deref(), Some("second compact"));

        let latest_slice = store
            .latest_compaction_t0_slice_for_session("sess-4")
            .expect("latest compaction slice")
            .expect("slice exists");
        assert_eq!(latest_slice.compaction_entry_id.as_deref(), Some("c2"));
        assert_eq!(latest_slice.summary.as_deref(), Some("second compact"));
    }

    #[test]
    fn partial_last_line_is_deferred_until_completed() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-5","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"u1","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"hello"}}
{"type":"compaction","id":"c1","parentId":"u1""#,
        );
        let store = MindStore::open_in_memory().expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());

        let first = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("first ingest");
        assert_eq!(first.processed_raw_events, 1);
        assert!(first.deferred_partial_line);
        assert!(store
            .latest_compaction_checkpoint_for_session("sess-5")
            .expect("latest checkpoint")
            .is_none());

        let completed = format!(
            "{}{}",
            fs::read_to_string(session.path()).expect("read"),
            " ,\"timestamp\":\"2024-12-03T14:10:00.000Z\",\"summary\":\"after completion\",\"firstKeptEntryId\":\"u1\",\"tokensBefore\":50}\n"
        );
        fs::write(session.path(), completed).expect("complete line");

        let second = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("second ingest");
        assert_eq!(second.processed_raw_events, 1);
        assert_eq!(second.persisted_compaction_checkpoints, 1);
        assert!(!second.deferred_partial_line);
        assert_eq!(store.raw_event_count("pi:sess-5").expect("raw count"), 2);
    }

    #[test]
    fn truncation_resets_cursor_and_reconciles_new_tail() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-6","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"u1","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"hello"}}
{"type":"compaction","id":"c1","parentId":"u1","timestamp":"2024-12-03T14:10:00.000Z","summary":"before truncate","firstKeptEntryId":"u1","tokensBefore":100}
"#,
        );
        let store = MindStore::open_in_memory().expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());

        let first = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("first ingest");
        assert_eq!(first.processed_raw_events, 2);

        fs::write(
            session.path(),
            concat!(
                "{\"type\":\"session\",\"id\":\"sess-6\"}\n",
                "{\"type\":\"message\",\"id\":\"u2\",\"timestamp\":\"2024-12-03T14:30:01.000Z\",\"message\":{\"role\":\"user\",\"content\":\"n\"}}\n",
                "{\"type\":\"compaction\",\"id\":\"c2\",\"timestamp\":\"2024-12-03T14:31:00.000Z\",\"summary\":\"t\"}\n"
            ),
        )
        .expect("truncate rewrite");

        let second = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("second ingest");
        assert!(second.reset_due_to_truncation);
        assert_eq!(second.processed_raw_events, 2);
        let latest = store
            .latest_compaction_checkpoint_for_session("sess-6")
            .expect("latest checkpoint")
            .expect("checkpoint exists");
        assert_eq!(latest.compaction_entry_id.as_deref(), Some("c2"));
        assert_eq!(store.raw_event_count("pi:sess-6").expect("raw count"), 4);
    }

    #[test]
    fn corrupt_lines_are_skipped_without_blocking_later_valid_entries() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-7","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"message","id":"u1","timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"hello"}}
{this is not json}
{"type":"compaction","id":"c1","timestamp":"2024-12-03T14:10:00.000Z","summary":"after corrupt","tokensBefore":42}
"#,
        );
        let store = MindStore::open_in_memory().expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());

        let report = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("ingest");
        assert_eq!(report.processed_raw_events, 2);
        assert_eq!(report.produced_t0_events, 1);
        assert_eq!(report.persisted_compaction_checkpoints, 1);
        assert_eq!(report.skipped_corrupt_lines, 1);
        assert_eq!(store.raw_event_count("pi:sess-7").expect("raw count"), 2);
    }

    #[test]
    fn custom_and_metadata_entries_preserve_attrs_without_creating_t0() {
        let session = write_session(
            r#"{"type":"session","version":3,"id":"sess-8","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/tmp/proj"}
{"type":"custom","id":"x1","timestamp":"2024-12-03T14:00:01.000Z","name":"extension_state","payload":{"enabled":true,"mode":"watch"}}
{"type":"model_change","id":"m1","timestamp":"2024-12-03T14:00:02.000Z","model":"gpt-x","provider":"openai","reason":"user_request"}
{"type":"thinking_level_change","id":"t1","timestamp":"2024-12-03T14:00:03.000Z","thinkingLevel":"deep","reason":"complexity_detected"}
"#,
        );
        let db_file = open_store_file();
        let store = MindStore::open(db_file.path()).expect("open store");
        let ingestor = PiSessionIngestor::new(IngestionOptions::default());
        let report = ingestor
            .ingest_session_file(&store, "agent-1", session.path())
            .expect("ingest");
        assert_eq!(report.processed_raw_events, 3);
        assert_eq!(report.produced_t0_events, 0);
        drop(store);

        let custom_attrs = raw_event_attrs_json(db_file.path(), "pi:x1");
        assert_eq!(custom_attrs["pi_custom_name"], "extension_state");
        assert_eq!(custom_attrs["pi_custom_payload"]["enabled"], true);
        assert_eq!(custom_attrs["pi_custom_payload"]["mode"], "watch");

        let model_attrs = raw_event_attrs_json(db_file.path(), "pi:m1");
        assert_eq!(model_attrs["pi_model"], "gpt-x");
        assert_eq!(model_attrs["pi_provider"], "openai");
        assert_eq!(model_attrs["pi_reason"], "user_request");

        let thinking_attrs = raw_event_attrs_json(db_file.path(), "pi:t1");
        assert_eq!(thinking_attrs["pi_thinkingLevel"], "deep");
        assert_eq!(thinking_attrs["pi_reason"], "complexity_detected");
    }
}
