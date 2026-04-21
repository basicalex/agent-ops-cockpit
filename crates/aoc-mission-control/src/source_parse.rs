//! Source/payload parsing helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn ms_to_datetime(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_millis_opt(value).single()
}

pub(crate) fn status_payload_from_state(state: &AgentState) -> AgentStatusPayload {
    let lifecycle = normalize_lifecycle(&state.lifecycle);
    let project_root = source_string_field(&state.source, "project_root")
        .unwrap_or_else(|| "(unknown)".to_string());
    let tab_scope = source_string_field(&state.source, "tab_scope");
    let agent_label = source_string_field(&state.source, "agent_label")
        .or_else(|| source_string_field(&state.source, "label"))
        .or_else(|| Some(extract_label(&state.agent_id)));
    AgentStatusPayload {
        agent_id: state.agent_id.clone(),
        status: lifecycle,
        pane_id: state.pane_id.clone(),
        project_root,
        tab_scope,
        agent_label,
        message: state.snippet.clone(),
        session_title: source_string_field(&state.source, "session_title"),
        chat_title: source_string_field(&state.source, "chat_title"),
    }
}

pub(crate) fn source_string_field(source: &Option<Value>, key: &str) -> Option<String> {
    source_value_field(source, key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

pub(crate) fn canonical_tab_scope(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

pub(crate) fn tab_scope_matches(viewer_scope: Option<&str>, candidate_scope: Option<&str>) -> bool {
    let Some(viewer) = viewer_scope.and_then(canonical_tab_scope) else {
        return false;
    };
    let Some(candidate) = candidate_scope.and_then(canonical_tab_scope) else {
        return false;
    };
    viewer == candidate
}

pub(crate) fn source_value_by_keys<'a>(
    source: &'a Option<Value>,
    keys: &[&str],
) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = source_value_field(source, key) {
            return Some(value);
        }
    }
    None
}

pub(crate) fn source_value_field<'a>(source: &'a Option<Value>, key: &str) -> Option<&'a Value> {
    let root = source.as_ref()?.as_object()?;
    if let Some(value) = root.get(key) {
        return Some(value);
    }
    for nested_key in ["agent_status", "pulse", "telemetry"] {
        if let Some(value) = root
            .get(nested_key)
            .and_then(Value::as_object)
            .and_then(|nested| nested.get(key))
        {
            return Some(value);
        }
    }
    None
}

pub(crate) fn parse_task_summaries_from_source(
    value: &Value,
    fallback_agent_id: &str,
) -> Result<Vec<TaskSummaryPayload>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    if let Some(items) = value.as_array() {
        let mut parsed = Vec::new();
        for item in items {
            parsed.push(parse_task_summary_item(item, fallback_agent_id, "default")?);
        }
        parsed.sort_by(|left, right| left.tag.cmp(&right.tag));
        return Ok(parsed);
    }
    if let Some(map) = value.as_object() {
        if looks_like_task_summary_payload(map) {
            return Ok(vec![parse_task_summary_item(
                value,
                fallback_agent_id,
                "default",
            )?]);
        }
        let mut parsed = Vec::new();
        for (tag, item) in map {
            parsed.push(parse_task_summary_item(item, fallback_agent_id, tag)?);
        }
        parsed.sort_by(|left, right| left.tag.cmp(&right.tag));
        return Ok(parsed);
    }
    Err("unsupported task summary source shape".to_string())
}

pub(crate) fn parse_task_summary_item(
    value: &Value,
    fallback_agent_id: &str,
    fallback_tag: &str,
) -> Result<TaskSummaryPayload, String> {
    let mut payload: TaskSummaryPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    if payload.agent_id.trim().is_empty() {
        payload.agent_id = fallback_agent_id.to_string();
    }
    payload.tag = if payload.tag.trim().is_empty() {
        fallback_tag.to_string()
    } else {
        payload.tag.trim().to_string()
    };
    if payload.counts.total == 0
        && payload.counts.pending == 0
        && payload.counts.in_progress == 0
        && payload.counts.done == 0
        && payload.counts.blocked == 0
    {
        if let Some(map) = value.as_object() {
            if !map.contains_key("counts")
                && (map.contains_key("total")
                    || map.contains_key("pending")
                    || map.contains_key("in_progress")
                    || map.contains_key("done")
                    || map.contains_key("blocked"))
            {
                if let Ok(counts) = serde_json::from_value::<TaskCounts>(value.clone()) {
                    payload.counts = counts;
                }
            }
        }
    }
    if let Some(active_tasks) = payload.active_tasks.as_mut() {
        for task in active_tasks {
            task.status = task.status.trim().to_ascii_lowercase().replace('_', "-");
        }
    }
    Ok(payload)
}

pub(crate) fn looks_like_task_summary_payload(map: &serde_json::Map<String, Value>) -> bool {
    map.contains_key("tag")
        || map.contains_key("counts")
        || map.contains_key("active_tasks")
        || map.contains_key("error")
        || map.contains_key("agent_id")
}

pub(crate) fn parse_diff_summary_from_source(
    value: &Value,
    fallback_agent_id: &str,
    fallback_project_root: &str,
) -> Result<Option<DiffSummaryPayload>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut payload: DiffSummaryPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    if payload.agent_id.trim().is_empty() {
        payload.agent_id = fallback_agent_id.to_string();
    }
    if payload.repo_root.trim().is_empty() {
        payload.repo_root = fallback_project_root.to_string();
    }
    payload.reason = payload
        .reason
        .as_ref()
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty());
    Ok(Some(payload))
}

pub(crate) fn parse_health_from_source(value: &Value) -> Result<Option<HealthSnapshot>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut snapshot: HealthSnapshot =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    if snapshot.taskmaster_status.trim().is_empty() {
        snapshot.taskmaster_status = "unknown".to_string();
    }
    for check in &mut snapshot.checks {
        if check.status.trim().is_empty() {
            check.status = "unknown".to_string();
        }
    }
    Ok(Some(snapshot))
}

pub(crate) fn parse_mind_observer_from_source(
    value: &Value,
) -> Result<Option<MindObserverFeedPayload>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut payload: MindObserverFeedPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    for event in &mut payload.events {
        event.reason = event
            .reason
            .as_ref()
            .map(|reason| reason.trim().to_string())
            .filter(|reason| !reason.is_empty());
        event.runtime = event
            .runtime
            .as_ref()
            .map(|runtime| runtime.trim().to_string())
            .filter(|runtime| !runtime.is_empty());
        event.failure_kind = event
            .failure_kind
            .as_ref()
            .map(|kind| kind.trim().to_string())
            .filter(|kind| !kind.is_empty());
        if let Some(progress) = event.progress.as_mut() {
            if progress.t1_target_tokens == 0 {
                event.progress = None;
                continue;
            }
            if progress.t1_hard_cap_tokens < progress.t1_target_tokens {
                progress.t1_hard_cap_tokens = progress.t1_target_tokens;
            }
            progress.tokens_until_next_run = progress
                .tokens_until_next_run
                .min(progress.t1_target_tokens);
        }
    }
    if payload.events.is_empty() {
        return Ok(None);
    }
    Ok(Some(payload))
}

pub(crate) fn parse_mind_injection_from_source(
    value: &Value,
) -> Result<Option<MindInjectionPayload>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut payload: MindInjectionPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    payload.status = payload.status.trim().to_ascii_lowercase().replace('_', "-");
    payload.scope = payload.scope.trim().to_string();
    payload.scope_key = payload.scope_key.trim().to_string();
    payload.active_tag = payload
        .active_tag
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    payload.reason = payload
        .reason
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    payload.snapshot_id = payload
        .snapshot_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    payload.payload_hash = payload
        .payload_hash
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if payload.status.is_empty() || payload.scope.is_empty() || payload.scope_key.is_empty() {
        return Ok(None);
    }
    Ok(Some(payload))
}

pub(crate) fn parse_insight_runtime_from_source(
    value: &Value,
) -> Result<Option<InsightRuntimeSnapshot>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut snapshot: InsightRuntimeSnapshot =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    snapshot.last_error = snapshot
        .last_error
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if snapshot.queue_depth < 0 {
        snapshot.queue_depth = 0;
    }
    if snapshot.t3_queue_depth < 0 {
        snapshot.t3_queue_depth = 0;
    }
    Ok(Some(snapshot))
}

pub(crate) fn parse_insight_detached_from_source(
    value: &Value,
) -> Result<Option<InsightDetachedStatusResult>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut snapshot: InsightDetachedStatusResult =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    snapshot.jobs.retain(|job| !job.job_id.trim().is_empty());
    snapshot.jobs.sort_by(|a, b| {
        b.created_at_ms
            .cmp(&a.created_at_ms)
            .then_with(|| a.job_id.cmp(&b.job_id))
    });
    snapshot.active_jobs = snapshot
        .jobs
        .iter()
        .filter(|job| {
            matches!(
                job.status,
                InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
            )
        })
        .count();
    if snapshot.status.trim().is_empty() {
        snapshot.status = if snapshot.jobs.is_empty() {
            "idle".to_string()
        } else {
            "ok".to_string()
        };
    }
    Ok(Some(snapshot))
}

pub(crate) fn source_confidence(source: &Option<Value>) -> Option<u8> {
    source
        .as_ref()
        .and_then(|value| source_numeric_field(value, "parser_confidence"))
        .or_else(|| {
            source
                .as_ref()
                .and_then(|value| source_numeric_field(value, "lifecycle_confidence"))
        })
}

pub(crate) fn source_numeric_field(source: &Value, key: &str) -> Option<u8> {
    source_value_field(&Some(source.clone()), key)
        .and_then(Value::as_u64)
        .and_then(|number| u8::try_from(number).ok())
}
