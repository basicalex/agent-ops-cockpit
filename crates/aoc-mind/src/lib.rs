mod compatibility_queries;
mod ingest;
mod observer_runtime;
mod query;
mod reflector_runtime;
pub mod render;
mod runtime;
mod standalone;
mod t1;
mod t3_runtime;

// Ingest exports
pub use ingest::{
    estimate_compact_tokens, ingest_raw_event, mind_progress_for_conversation, T0IngestConfig,
    T0IngestError, T0IngestReport,
};
pub use t1::{evaluate_t1_token_threshold, T1ThresholdDecision, T1ThresholdError};

// Runtime exports
pub use compatibility_queries::{
    compile_mind_context_pack, compile_mind_provenance_export, compile_mind_provenance_graph,
    mind_context_pack_mode_for_trigger, MindContextPack, MindContextPackCitation,
    MindContextPackMode, MindContextPackProfile, MindContextPackRequest, MindContextPackSection,
    MindContextPackSourceOverrides,
};
pub use observer_runtime::{
    ClaimedObserverRun, ObserverQueueConfig, ObserverTrigger, ObserverTriggerKind,
    ObserverTriggerPriority, SessionObserverQueue,
};
pub use reflector_runtime::{
    DetachedReflectorWorker, ReflectorRuntimeConfig, ReflectorRuntimeError, ReflectorTickReport,
};
pub use runtime::{MindFinalizeDrainOutcome, MindRuntimeConfig, MindRuntimeCore};
pub use t3_runtime::{DetachedT3Worker, T3RuntimeConfig, T3RuntimeError, T3TickReport};

// Query exports
pub use query::{
    canon_key, canon_stale_entries, collect_mind_search_hits, compaction_rebuildable_from_attrs,
    load_mind_artifact_drilldown, mind_store_path, parse_handshake_entries,
    parse_project_canon_entries, project_scope_key, MindArtifactDrilldown, MindCanonEntry,
    MindHandshakeEntry, MindSearchHit, MindSessionExportManifest,
};
pub use standalone::{
    default_pi_session_root, discover_latest_pi_session_file, latest_pi_session_file,
    legacy_mind_store_path, mind_runtime_root, mind_store_path_with_override, open_project_store,
    read_mind_service_health_snapshot, read_mind_service_lease, reflector_dispatch_lock_path,
    reflector_lock_path_with_override, sync_latest_pi_session_into_project_store,
    sync_session_file_into_project_store, t3_dispatch_lock_path, t3_lock_path_with_override,
    write_mind_service_health_snapshot, MindProjectPaths, MindServiceHealthSnapshot,
    MindServiceLease, MindServiceLeaseGuard, OpenedMindProjectStore, StandaloneMindError,
    StandalonePiSyncReport,
};

pub fn canonical_mind_command_name(command: &str) -> Option<&'static str> {
    match command {
        "mind_compaction_checkpoint" => Some("mind_compaction_checkpoint"),
        "mind_ingest_event" | "insight_ingest" => Some("mind_ingest_event"),
        "mind_handoff" | "insight_handoff" => Some("mind_handoff"),
        "mind_resume" | "insight_resume" => Some("mind_resume"),
        "mind_finalize" | "mind_finalize_session" => Some("mind_finalize_session"),
        "mind_compaction_rebuild" => Some("mind_compaction_rebuild"),
        "mind_t3_requeue" => Some("mind_t3_requeue"),
        "mind_handshake_rebuild" => Some("mind_handshake_rebuild"),
        "mind_context_pack" => Some("mind_context_pack"),
        "mind_provenance_query" => Some("mind_provenance_query"),
        _ => None,
    }
}

// Render exports
pub use render::{
    age_color, age_meter, detached_job_attention_color, detached_job_attention_label,
    detached_job_recovery_guidance, detached_job_status_color, detached_job_status_label,
    detached_owner_plane_label, detached_worker_kind_display, detached_worker_kind_label,
    format_age, lifecycle_color, mind_event_is_t0, mind_event_is_t2, mind_event_is_t3,
    mind_event_lane, mind_event_sort_ms, mind_lane_color, mind_lane_label, mind_lane_matches,
    mind_lane_rollup, mind_progress_label, mind_runtime_label, mind_status_color,
    mind_status_label, mind_status_rollup, mind_timestamp_label, mind_trigger_label,
    ms_to_datetime, normalize_lifecycle, render_insight_detached_rollup_line,
    render_mind_header_lines, render_mind_injection_rollup_line, render_mind_observer_rows,
    render_mind_search_lines, MindInjectionRow, MindLaneFilter, MindObserverRow, MindStatusRollup,
    MindTheme,
};

use aoc_core::{
    mind_contracts::{
        build_compaction_t0_slice, build_t2_workstream_batch, canonical_json,
        canonical_payload_hash, validate_t1_scope, ConversationRole, MindContractError,
        ObservationRef, ObserverAdapter, ObserverInput, ObserverOutput, SemanticAdapterError,
        SemanticFailureKind, SemanticGuardrails, SemanticModelProfile, SemanticProvenance,
        SemanticRuntime, SemanticRuntimeMode, SemanticStage, T1Batch, T1_PARSER_HARD_CAP_TOKENS,
        T1_PARSER_TARGET_TOKENS,
    },
    mind_observer_feed::{
        MindInjectionTriggerKind, MindObserverFeedEvent, MindObserverFeedProgress,
        MindObserverFeedStatus, MindObserverFeedTriggerKind,
    },
};
use aoc_storage::{
    CanonEntryRevision, CanonRevisionState, ConversationContextState, MindStore, ProjectWatermark,
    ReflectorJob, StorageError, StoredArtifact, StoredCompactEvent, T3BacklogJob,
};
use aoc_task_attribution::{AttributionConfig, AttributionError, TaskAttributionEngine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

const DEFAULT_T1_OUTPUT_MAX_CHARS: usize = 1_200;
const DEFAULT_T2_OUTPUT_MAX_CHARS: usize = 1_400;
const DEFAULT_T2_TRIGGER_TOKENS: u32 = 2_400;
const DEFAULT_PI_OBSERVER_PROVIDER: &str = "pi";
const DEFAULT_PI_OBSERVER_MODEL: &str = "gpt-5.4-mini";
const DEFAULT_PI_OBSERVER_PROMPT_VERSION: &str = "pi.observer.v1";
const DEFAULT_PI_REFLECTOR_PROVIDER: &str = "pi";
const DEFAULT_PI_REFLECTOR_MODEL: &str = "gpt-5.4-mini";
const DEFAULT_PI_REFLECTOR_PROMPT_VERSION: &str = "pi.reflector.v1";
const DEFAULT_SEMANTIC_COST_MICROS_PER_TOKEN: u64 = 100;

#[derive(Debug, Error)]
pub enum ReflectorJobError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum T3BacklogJobError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
    #[error("export error: {0}")]
    Export(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub const MIND_T3_CANON_SUMMARY_MAX_CHARS: usize = 280;
pub const MIND_T3_CANON_STALE_AFTER_DAYS: i64 = 14;
pub const MIND_T3_HANDSHAKE_TOKEN_BUDGET: u32 = 500;
pub const MIND_T3_HANDSHAKE_MAX_ITEMS: usize = 12;

pub fn process_reflector_job(
    store: &MindStore,
    job: &ReflectorJob,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), ReflectorJobError> {
    let observations = load_reflector_job_observations(store, job)?;
    if observations.is_empty() {
        return Err(ReflectorJobError::Internal(
            "no matching observations found for reflector job".to_string(),
        ));
    }

    let text = synthesize_reflector_job_text(&job.active_tag, &observations, usize::MAX);
    let input_hash = canonical_payload_hash(&(
        &job.active_tag,
        &job.observation_ids,
        &job.conversation_ids,
        job.estimated_tokens,
    ))?;
    let output_hash = canonical_payload_hash(&text)?;
    let artifact_id = format!("ref:auto:{}", &input_hash[..16]);
    let conversation_id = observations
        .first()
        .map(|artifact| artifact.conversation_id.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let trace_ids = observations
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();

    store.insert_reflection(&artifact_id, &conversation_id, now, &text, &trace_ids)?;
    persist_deterministic_provenance(
        store,
        &artifact_id,
        SemanticStage::T2Reflector,
        "deterministic.reflector.runtime.v1",
        input_hash,
        Some(output_hash),
        now,
    )
    .map_err(|err| match err {
        DistillationError::Storage(err) => ReflectorJobError::Storage(err),
        DistillationError::Contract(err) => ReflectorJobError::Contract(err),
        DistillationError::Attribution(err) => ReflectorJobError::Internal(err.to_string()),
        DistillationError::Internal(err) => ReflectorJobError::Internal(err),
    })?;

    Ok(())
}

pub fn process_t3_backlog_job<F>(
    store: &MindStore,
    job: &T3BacklogJob,
    now: chrono::DateTime<chrono::Utc>,
    export_writer: F,
) -> Result<(), T3BacklogJobError>
where
    F: FnOnce(&MindStore, &str, Option<&str>, chrono::DateTime<chrono::Utc>) -> Result<(), String>,
{
    let mut artifacts = Vec::new();
    for artifact_id in &job.artifact_refs {
        if let Some(artifact) = store.artifact_by_id(artifact_id)? {
            artifacts.push(artifact);
        }
    }

    if artifacts.is_empty() {
        return Err(T3BacklogJobError::Internal(format!(
            "t3 backlog job {} has no resolvable artifacts",
            job.job_id
        )));
    }

    artifacts.sort_by(|left, right| {
        left.ts
            .cmp(&right.ts)
            .then(left.artifact_id.cmp(&right.artifact_id))
    });
    artifacts.dedup_by(|left, right| left.artifact_id == right.artifact_id);

    let watermark_scope = t3_scope_id_for_project_root(&job.project_root);
    let watermark = store.project_watermark(&watermark_scope)?;
    let delta = artifacts
        .into_iter()
        .filter(|artifact| artifact_after_watermark(artifact, watermark.as_ref()))
        .collect::<Vec<_>>();

    if delta.is_empty() {
        return Ok(());
    }

    let topic = job
        .active_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let latest_by_entry = latest_t3_artifact_by_entry(&job.project_root, topic.as_deref(), &delta)?;
    let mut touched_entry_ids = Vec::new();
    let mut entry_ids = latest_by_entry.keys().cloned().collect::<Vec<_>>();
    entry_ids.sort();

    for entry_id in entry_ids {
        let artifact = latest_by_entry.get(&entry_id).ok_or_else(|| {
            T3BacklogJobError::Internal(format!("missing t3 artifact for entry {entry_id}"))
        })?;
        let summary = project_canon_summary(artifact);
        let evidence_refs = project_canon_evidence_refs(store, artifact)?;
        let confidence_bps = project_canon_confidence_bps(now, artifact, evidence_refs.len());
        let freshness_score = project_canon_freshness_score(now, artifact.ts);

        let revision = store.upsert_canon_entry_revision(
            &entry_id,
            topic.as_deref(),
            &summary,
            confidence_bps,
            freshness_score,
            None,
            &evidence_refs,
            now,
        )?;
        touched_entry_ids.push(revision.entry_id);
    }

    let stale_before = now - chrono::Duration::days(MIND_T3_CANON_STALE_AFTER_DAYS);
    store.mark_active_canon_entries_stale(topic.as_deref(), stale_before, &touched_entry_ids)?;

    export_writer(store, &job.project_root, topic.as_deref(), now)
        .map_err(T3BacklogJobError::Export)?;

    let last = delta.last().expect("delta is non-empty");
    store.advance_project_watermark(
        &watermark_scope,
        Some(last.ts),
        Some(&last.artifact_id),
        now,
    )?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum DistillationError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
    #[error("attribution error: {0}")]
    Attribution(#[from] AttributionError),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct FinalizeArtifactPlan {
    pub conversation_ids: Vec<String>,
    pub active_tag: Option<String>,
    pub delta_artifacts: Vec<StoredArtifact>,
    pub t1_artifacts: Vec<StoredArtifact>,
    pub t2_artifacts: Vec<StoredArtifact>,
    pub slice_start_id: String,
    pub slice_end_id: String,
    pub slice_hash: String,
    pub artifact_ids: Vec<String>,
    pub last_artifact_ts: chrono::DateTime<chrono::Utc>,
    pub watermark_scope: String,
}

#[derive(Debug, Clone)]
pub enum FinalizeArtifactSelection {
    NoNewArtifacts,
    NoExportableArtifacts {
        conversation_ids: Vec<String>,
        active_tag: Option<String>,
        delta_artifacts: Vec<StoredArtifact>,
    },
    Ready(FinalizeArtifactPlan),
}

#[derive(Debug, Clone)]
pub enum SessionFinalizePlanOutcome {
    Skip {
        observer_reason: &'static str,
        outcome_reason_suffix: &'static str,
    },
    Ready(FinalizeArtifactPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleFinalizeDecision {
    NoLastIngest,
    Disabled,
    WaitingForIdleTimeout,
    Throttled,
    Finalize { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizeDrainDecision {
    Continue,
    Settled,
    TimedOut { observer_reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFinalizeMessageSet {
    pub status: &'static str,
    pub reason: String,
    pub observer_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindCommandObserverEventPlan {
    pub status: MindObserverFeedStatus,
    pub trigger: MindObserverFeedTriggerKind,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindCommandResultPolicy {
    pub status: &'static str,
    pub reason: String,
    pub error_code: Option<&'static str>,
    pub error_message: Option<String>,
    pub observer_event: Option<MindCommandObserverEventPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MindCommandFollowupPlan {
    pub queue_reason: Option<String>,
    pub injection_trigger: Option<MindInjectionTriggerKind>,
    pub injection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindCommandExecutionPlan {
    pub result: MindCommandResultPolicy,
    pub followup: MindCommandFollowupPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManualCompactionRebuildOutcome {
    QueueObserver {
        conversation_id: String,
        queue_reason: String,
        result: MindCommandResultPolicy,
    },
    Complete {
        result: MindCommandResultPolicy,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualT3RequeueOutcome {
    pub result: MindCommandResultPolicy,
    pub pending_jobs: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedHandshakeRebuild {
    pub scope: &'static str,
    pub scope_key: String,
    pub bundle: HandshakeExportBundle,
    pub policy: MindCommandExecutionPlan,
}

#[derive(Debug, Error)]
pub enum FinalizeArtifactPlanError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionExportManifest {
    pub schema_version: u32,
    pub session_id: String,
    pub pane_id: String,
    pub project_root: String,
    pub active_tag: Option<String>,
    pub conversation_ids: Vec<String>,
    pub export_dir: String,
    pub t1_count: usize,
    pub t2_count: usize,
    pub t1_artifact_ids: Vec<String>,
    pub t2_artifact_ids: Vec<String>,
    pub slice_start_id: String,
    pub slice_end_id: String,
    pub slice_hash: String,
    pub exported_at: String,
    pub last_artifact_ts: String,
    pub watermark_scope: String,
    pub t3_job_id: String,
    pub t3_job_inserted: bool,
}

#[derive(Debug, Clone)]
pub struct SessionExportBundle {
    pub manifest: SessionExportManifest,
    pub manifest_json: String,
    pub t1_markdown: String,
    pub t2_markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionExportFileIntent {
    pub file_name: &'static str,
    pub contents: String,
    pub safety_label: &'static str,
    pub write_stage: &'static str,
}

#[derive(Debug, Clone)]
pub struct SessionFinalizeHostPlan {
    pub manifest: SessionExportManifest,
    pub export_files: Vec<SessionExportFileIntent>,
    pub watermark_ts: chrono::DateTime<chrono::Utc>,
    pub watermark_artifact_id: String,
    pub success: SessionFinalizeMessageSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFinalizeExportLocation {
    pub export_dir_name: String,
    pub export_dir: String,
}

#[derive(Debug, Error)]
pub enum SessionExportBundleError {
    #[error("serialization error: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HandshakeTaskCounts {
    pub total: u32,
    pub pending: u32,
    pub in_progress: u32,
    pub blocked: u32,
    pub done: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeWorkstreamSummary {
    pub tag: String,
    pub counts: HandshakeTaskCounts,
    pub prd_backed_open: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeTaskSummary {
    pub id: String,
    pub tag: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub prd_source: Option<&'static str>,
    pub active_agent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HandshakeProjectSnapshot {
    pub workstreams: Vec<HandshakeWorkstreamSummary>,
    pub priority_tasks: Vec<HandshakeTaskSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeExportBundle {
    pub payload: String,
    pub payload_hash: String,
    pub token_estimate: u32,
}

#[derive(Debug, Error)]
pub enum T3ExportError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
}

pub fn build_session_export_bundle(
    plan: &FinalizeArtifactPlan,
    session_id: &str,
    pane_id: &str,
    project_root: &str,
    export_dir: &str,
    t3_job_id: &str,
    t3_job_inserted: bool,
) -> Result<SessionExportBundle, SessionExportBundleError> {
    let t1_markdown = render_artifact_markdown("t1", &plan.t1_artifacts);
    let t2_markdown = render_artifact_markdown("t2", &plan.t2_artifacts);
    let manifest = SessionExportManifest {
        schema_version: 1,
        session_id: session_id.to_string(),
        pane_id: pane_id.to_string(),
        project_root: project_root.to_string(),
        active_tag: plan.active_tag.clone(),
        conversation_ids: plan.conversation_ids.clone(),
        export_dir: export_dir.to_string(),
        t1_count: plan.t1_artifacts.len(),
        t2_count: plan.t2_artifacts.len(),
        t1_artifact_ids: plan
            .t1_artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        t2_artifact_ids: plan
            .t2_artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        slice_start_id: plan.slice_start_id.clone(),
        slice_end_id: plan.slice_end_id.clone(),
        slice_hash: plan.slice_hash.clone(),
        exported_at: plan.last_artifact_ts.to_rfc3339(),
        last_artifact_ts: plan.last_artifact_ts.to_rfc3339(),
        watermark_scope: plan.watermark_scope.clone(),
        t3_job_id: t3_job_id.to_string(),
        t3_job_inserted,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|err| SessionExportBundleError::Serialization(err.to_string()))?;
    Ok(SessionExportBundle {
        manifest,
        manifest_json,
        t1_markdown,
        t2_markdown,
    })
}

pub fn evaluate_idle_finalize(
    last_ingest_at: Option<chrono::DateTime<chrono::Utc>>,
    last_check_at: Option<chrono::DateTime<chrono::Utc>>,
    now: chrono::DateTime<chrono::Utc>,
    idle_timeout_ms: i64,
    idle_check_interval_ms: i64,
) -> IdleFinalizeDecision {
    let Some(last_ingest_at) = last_ingest_at else {
        return IdleFinalizeDecision::NoLastIngest;
    };

    if idle_timeout_ms <= 0 {
        return IdleFinalizeDecision::Disabled;
    }

    if now < last_ingest_at + chrono::Duration::milliseconds(idle_timeout_ms) {
        return IdleFinalizeDecision::WaitingForIdleTimeout;
    }

    if let Some(last_check_at) = last_check_at {
        if now < last_check_at + chrono::Duration::milliseconds(idle_check_interval_ms) {
            return IdleFinalizeDecision::Throttled;
        }
    }

    IdleFinalizeDecision::Finalize {
        reason: "idle timeout finalize",
    }
}

pub fn evaluate_finalize_drain(
    observer_idle: bool,
    reflector_pending: i64,
    now: chrono::DateTime<chrono::Utc>,
    deadline: chrono::DateTime<chrono::Utc>,
) -> FinalizeDrainDecision {
    if observer_idle && reflector_pending <= 0 {
        FinalizeDrainDecision::Settled
    } else if now >= deadline {
        FinalizeDrainDecision::TimedOut {
            observer_reason: "finalize drain timeout reached; exporting current slice",
        }
    } else {
        FinalizeDrainDecision::Continue
    }
}

pub fn session_finalize_error(
    stage: &str,
    err: impl std::fmt::Display,
) -> SessionFinalizeMessageSet {
    let err = err.to_string();
    let (observer_prefix, reason_prefix) = match stage {
        "planning" => (
            "finalize planning failed",
            "finalize failed: planning error",
        ),
        "t3_enqueue" => (
            "finalize t3 enqueue failed",
            "finalize failed: t3 enqueue error",
        ),
        "export_bundle" => (
            "finalize export bundle failed",
            "finalize failed: export bundle error",
        ),
        "t1_export_write" => (
            "finalize write t1.md failed",
            "finalize failed: t1 export error",
        ),
        "t2_export_write" => (
            "finalize write t2.md failed",
            "finalize failed: t2 export error",
        ),
        "manifest_write" => (
            "finalize write manifest failed",
            "finalize failed: manifest write error",
        ),
        "watermark_write" => (
            "finalize watermark advance failed",
            "finalize failed: watermark write error",
        ),
        _ => ("finalize failed", "finalize failed"),
    };

    SessionFinalizeMessageSet {
        status: "error",
        reason: format!("{}: {}", reason_prefix, err),
        observer_reason: format!("{}: {}", observer_prefix, err),
    }
}

pub fn session_finalize_success(
    finalize_reason: &str,
    export_dir: &str,
    manifest: &SessionExportManifest,
) -> SessionFinalizeMessageSet {
    SessionFinalizeMessageSet {
        status: "ok",
        reason: format!(
            "{}: session export finalized at {}",
            finalize_reason, export_dir
        ),
        observer_reason: format!(
            "{}: session export finalized: t1={} t2={} t3_job_inserted={}",
            finalize_reason, manifest.t1_count, manifest.t2_count, manifest.t3_job_inserted
        ),
    }
}

fn sanitize_export_component(input: &str) -> String {
    let mut value = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    while value.contains("__") {
        value = value.replace("__", "_");
    }
    value.trim_matches('_').to_string()
}

fn mind_command_success(reason: impl Into<String>) -> MindCommandResultPolicy {
    MindCommandResultPolicy {
        status: "ok",
        reason: reason.into(),
        error_code: None,
        error_message: None,
        observer_event: None,
    }
}

fn mind_command_error(
    reason: impl Into<String>,
    code: &'static str,
    message: impl Into<String>,
) -> MindCommandResultPolicy {
    MindCommandResultPolicy {
        status: "error",
        reason: reason.into(),
        error_code: Some(code),
        error_message: Some(message.into()),
        observer_event: None,
    }
}

pub fn prepare_handoff_resume_command(
    normalized_command: &str,
    reason: Option<&str>,
) -> MindCommandExecutionPlan {
    let queue_reason = reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(|reason| reason.to_string())
        .unwrap_or_else(|| "stm handoff".to_string());
    let injection_trigger = if normalized_command == "mind_resume"
        || queue_reason.to_ascii_lowercase().contains("resume")
    {
        MindInjectionTriggerKind::Resume
    } else {
        MindInjectionTriggerKind::Handoff
    };
    MindCommandExecutionPlan {
        result: mind_command_success("handoff/resume observer trigger queued"),
        followup: MindCommandFollowupPlan {
            queue_reason: Some(queue_reason.clone()),
            injection_trigger: Some(injection_trigger),
            injection_reason: Some(queue_reason),
        },
    }
}

pub fn mind_handoff_resume_conversation_missing() -> MindCommandResultPolicy {
    mind_command_error(
        "conversation unavailable",
        "conversation_missing",
        "no conversation available for handoff observer trigger",
    )
}

pub fn mind_compaction_rebuild_checkpoint_missing() -> MindCommandResultPolicy {
    let mut policy = mind_command_error(
        "no compaction checkpoint found",
        "mind_compaction_checkpoint_missing",
        "no compaction checkpoint found for session",
    );
    policy.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Error,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: "compaction rebuild failed: no compaction checkpoint found".to_string(),
    });
    policy
}

pub fn mind_compaction_rebuild_checkpoint_lookup_failed(
    err: impl std::fmt::Display,
) -> MindCommandResultPolicy {
    let err = err.to_string();
    let mut policy = mind_command_error(
        "compaction checkpoint lookup failed",
        "mind_compaction_checkpoint_lookup_failed",
        err.clone(),
    );
    policy.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Error,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: format!("compaction rebuild failed: {err}"),
    });
    policy
}

pub fn prepare_compaction_rebuild_success(
    checkpoint_id: &str,
    reason: &str,
) -> MindCommandExecutionPlan {
    let mut result = mind_command_success(format!(
        "compaction rebuilt and requeued: {}",
        checkpoint_id
    ));
    result.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Success,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: format!("compaction slice rebuilt: {}", checkpoint_id),
    });
    MindCommandExecutionPlan {
        result,
        followup: MindCommandFollowupPlan {
            queue_reason: Some(format!(
                "compaction rebuild requested ({reason}): {}",
                checkpoint_id
            )),
            ..Default::default()
        },
    }
}

pub fn mind_compaction_rebuild_unavailable() -> MindCommandResultPolicy {
    let mut policy = mind_command_error(
        "compaction rebuild unavailable",
        "mind_compaction_rebuild_unavailable",
        "checkpoint marker provenance unavailable for rebuild",
    );
    policy.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Error,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: "compaction rebuild unavailable: marker provenance missing".to_string(),
    });
    policy
}

pub fn mind_compaction_rebuild_failed(err: impl std::fmt::Display) -> MindCommandResultPolicy {
    let err = err.to_string();
    let mut policy = mind_command_error(
        "compaction rebuild failed",
        "mind_compaction_rebuild_failed",
        err.clone(),
    );
    policy.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Error,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: format!("compaction rebuild failed: {err}"),
    });
    policy
}

pub fn mind_t3_requeue_success(
    job_id: &str,
    inserted: bool,
    reason: &str,
) -> MindCommandResultPolicy {
    let inserted_label = if inserted { "inserted" } else { "existing" };
    let mut policy = mind_command_success(format!("t3 requeue {} ({})", job_id, inserted_label));
    policy.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Queued,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: format!(
            "t3 requeue requested ({reason}): {} ({})",
            job_id, inserted_label
        ),
    });
    policy
}

pub fn mind_t3_requeue_failed(err: impl std::fmt::Display) -> MindCommandResultPolicy {
    let err = err.to_string();
    let mut policy = mind_command_error("t3 requeue failed", "mind_t3_requeue_failed", err.clone());
    policy.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Error,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: format!("t3 requeue failed: {err}"),
    });
    policy
}

pub fn prepare_handshake_rebuild_success() -> MindCommandExecutionPlan {
    let mut result = mind_command_success("handshake baseline rebuilt");
    result.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Success,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: "handshake baseline rebuilt".to_string(),
    });
    MindCommandExecutionPlan {
        result,
        followup: MindCommandFollowupPlan {
            injection_trigger: Some(MindInjectionTriggerKind::Startup),
            injection_reason: Some("handshake rebuild".to_string()),
            ..Default::default()
        },
    }
}

pub fn mind_handshake_rebuild_failed(err: impl std::fmt::Display) -> MindCommandResultPolicy {
    let err = err.to_string();
    let mut policy = mind_command_error(
        "handshake rebuild failed",
        "mind_handshake_rebuild_failed",
        err.clone(),
    );
    policy.observer_event = Some(MindCommandObserverEventPlan {
        status: MindObserverFeedStatus::Error,
        trigger: MindObserverFeedTriggerKind::ManualShortcut,
        reason: format!("handshake rebuild failed: {err}"),
    });
    policy
}

fn string_list_attr(
    attrs: &std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Vec<String> {
    attrs
        .get(key)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value: &serde_json::Value| value.as_str().map(|s| s.trim().to_string()))
        .filter(|value: &String| !value.is_empty())
        .collect()
}

pub fn rebuild_compaction_t0_slice_from_checkpoint(
    store: &MindStore,
    checkpoint: &aoc_storage::CompactionCheckpoint,
) -> Result<Option<aoc_core::mind_contracts::CompactionT0Slice>, String> {
    let Some(marker_event_id) = checkpoint.marker_event_id.as_deref() else {
        return Ok(None);
    };
    let Some(marker_event) = store
        .raw_event_by_id(marker_event_id)
        .map_err(|err| format!("load compaction marker failed: {err}"))?
    else {
        return Ok(None);
    };

    let read_files = string_list_attr(&marker_event.attrs, "pi_detail_read_files");
    let modified_files = {
        let live = string_list_attr(&marker_event.attrs, "mind_compaction_modified_files");
        if live.is_empty() {
            string_list_attr(&marker_event.attrs, "pi_detail_modified_files")
        } else {
            live
        }
    };
    let slice = build_compaction_t0_slice(
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
        &[marker_event.event_id],
        &read_files,
        &modified_files,
        Some(&checkpoint.checkpoint_id),
        "t0.compaction.v1",
    )
    .map_err(|err| format!("rebuild compaction slice failed: {err}"))?;

    Ok(Some(slice))
}

pub fn execute_manual_compaction_rebuild(
    store: &MindStore,
    session_id: &str,
    reason: &str,
) -> ManualCompactionRebuildOutcome {
    let checkpoint = match store.latest_compaction_checkpoint_for_session(session_id) {
        Ok(Some(checkpoint)) => checkpoint,
        Ok(None) => {
            return ManualCompactionRebuildOutcome::Complete {
                result: mind_compaction_rebuild_checkpoint_missing(),
            };
        }
        Err(err) => {
            return ManualCompactionRebuildOutcome::Complete {
                result: mind_compaction_rebuild_checkpoint_lookup_failed(err),
            };
        }
    };

    match rebuild_compaction_t0_slice_from_checkpoint(store, &checkpoint) {
        Ok(Some(slice)) => {
            if let Err(err) = store.upsert_compaction_t0_slice(&slice) {
                return ManualCompactionRebuildOutcome::Complete {
                    result: mind_compaction_rebuild_failed(err),
                };
            }
            let plan = prepare_compaction_rebuild_success(&checkpoint.checkpoint_id, reason);
            ManualCompactionRebuildOutcome::QueueObserver {
                conversation_id: checkpoint.conversation_id,
                queue_reason: plan.followup.queue_reason.unwrap_or_default(),
                result: plan.result,
            }
        }
        Ok(None) => ManualCompactionRebuildOutcome::Complete {
            result: mind_compaction_rebuild_unavailable(),
        },
        Err(err) => ManualCompactionRebuildOutcome::Complete {
            result: mind_compaction_rebuild_failed(err),
        },
    }
}

fn load_session_export_manifests(
    project_root: &str,
    limit: usize,
) -> Result<Vec<SessionExportManifest>, String> {
    let insight_root = std::path::PathBuf::from(project_root)
        .join(".aoc")
        .join("mind")
        .join("insight");
    let entries = std::fs::read_dir(&insight_root)
        .map_err(|err| format!("read insight export dir failed: {err}"))?;

    let mut manifests = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .filter_map(|dir| {
            let manifest_path = dir.join("manifest.json");
            let payload = std::fs::read_to_string(&manifest_path).ok()?;
            let manifest = serde_json::from_str::<SessionExportManifest>(&payload).ok()?;
            Some((dir, manifest))
        })
        .collect::<Vec<_>>();

    manifests.sort_by(|left, right| {
        right
            .1
            .exported_at
            .cmp(&left.1.exported_at)
            .then_with(|| right.0.cmp(&left.0))
    });

    Ok(manifests
        .into_iter()
        .take(limit.max(1))
        .map(|(_, manifest)| manifest)
        .collect())
}

pub fn load_latest_session_export_manifest(
    project_root: &str,
) -> Result<SessionExportManifest, String> {
    load_session_export_manifests(project_root, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| "no session exports found to requeue".to_string())
}

pub fn execute_manual_t3_requeue_from_manifest(
    store: &MindStore,
    project_root: &str,
    reason: &str,
) -> ManualT3RequeueOutcome {
    let manifest = match load_latest_session_export_manifest(project_root) {
        Ok(manifest) => manifest,
        Err(err) => {
            return ManualT3RequeueOutcome {
                result: mind_t3_requeue_failed(err),
                pending_jobs: None,
            };
        }
    };
    let mut artifact_ids = manifest.t1_artifact_ids.clone();
    artifact_ids.extend(manifest.t2_artifact_ids.clone());
    artifact_ids.sort();
    artifact_ids.dedup();

    if artifact_ids.is_empty() {
        return ManualT3RequeueOutcome {
            result: mind_t3_requeue_failed("latest session export has no t1/t2 artifact ids"),
            pending_jobs: None,
        };
    }

    let slice_start = manifest.slice_start_id.trim();
    let slice_end = manifest.slice_end_id.trim();
    match store.enqueue_t3_backlog_job(
        project_root,
        &manifest.session_id,
        &manifest.pane_id,
        manifest.active_tag.as_deref(),
        if slice_start.is_empty() {
            None
        } else {
            Some(slice_start)
        },
        if slice_end.is_empty() {
            None
        } else {
            Some(slice_end)
        },
        &artifact_ids,
        Utc::now(),
    ) {
        Ok((job_id, inserted)) => ManualT3RequeueOutcome {
            result: mind_t3_requeue_success(&job_id, inserted, reason),
            pending_jobs: store.pending_t3_backlog_jobs().ok(),
        },
        Err(err) => ManualT3RequeueOutcome {
            result: mind_t3_requeue_failed(format!("enqueue t3 backlog job failed: {err}")),
            pending_jobs: None,
        },
    }
}

pub fn prepare_handshake_rebuild(
    store: &MindStore,
    project_root: &str,
    project_snapshot: Option<&HandshakeProjectSnapshot>,
    active_tag: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<PreparedHandshakeRebuild, String> {
    let bundle = build_handshake_export(store, project_snapshot, active_tag, now)
        .map_err(|err| format!("build handshake export failed: {err}"))?;
    Ok(PreparedHandshakeRebuild {
        scope: "project",
        scope_key: project_scope_key(std::path::Path::new(project_root)),
        bundle,
        policy: prepare_handshake_rebuild_success(),
    })
}

pub fn persist_handshake_rebuild_snapshot(
    store: &MindStore,
    prepared: &PreparedHandshakeRebuild,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let _ = store
        .upsert_handshake_snapshot(
            prepared.scope,
            &prepared.scope_key,
            &prepared.bundle.payload,
            &prepared.bundle.payload_hash,
            prepared.bundle.token_estimate,
            now,
        )
        .map_err(|err| format!("persist handshake snapshot failed: {err}"))?;
    Ok(())
}

pub fn prepare_session_finalize_export_location(
    project_root: &str,
    session_id: &str,
    plan: &FinalizeArtifactPlan,
) -> SessionFinalizeExportLocation {
    let safe_session = sanitize_export_component(session_id);
    let ts = plan.last_artifact_ts.format("%Y%m%dT%H%M%SZ");
    let hash_prefix = plan.slice_hash.chars().take(12).collect::<String>();
    let export_dir_name = format!("{}_{}_{}", safe_session, ts, hash_prefix);
    let export_dir = std::path::Path::new(project_root)
        .join(".aoc")
        .join("mind")
        .join("insight")
        .join(&export_dir_name)
        .to_string_lossy()
        .to_string();
    SessionFinalizeExportLocation {
        export_dir_name,
        export_dir,
    }
}

pub fn prepare_session_finalize_host_plan(
    plan: &FinalizeArtifactPlan,
    session_id: &str,
    pane_id: &str,
    project_root: &str,
    export_dir: &str,
    t3_job_id: &str,
    t3_job_inserted: bool,
    finalize_reason: &str,
) -> Result<SessionFinalizeHostPlan, SessionExportBundleError> {
    let bundle = build_session_export_bundle(
        plan,
        session_id,
        pane_id,
        project_root,
        export_dir,
        t3_job_id,
        t3_job_inserted,
    )?;
    let success = session_finalize_success(finalize_reason, export_dir, &bundle.manifest);
    Ok(SessionFinalizeHostPlan {
        manifest: bundle.manifest.clone(),
        export_files: vec![
            SessionExportFileIntent {
                file_name: "t1.md",
                contents: bundle.t1_markdown,
                safety_label: "t1 export",
                write_stage: "t1_export_write",
            },
            SessionExportFileIntent {
                file_name: "t2.md",
                contents: bundle.t2_markdown,
                safety_label: "t2 export",
                write_stage: "t2_export_write",
            },
            SessionExportFileIntent {
                file_name: "manifest.json",
                contents: bundle.manifest_json,
                safety_label: "manifest export",
                write_stage: "manifest_write",
            },
        ],
        watermark_ts: plan.last_artifact_ts,
        watermark_artifact_id: plan.slice_end_id.clone(),
        success,
    })
}

pub fn prepare_session_finalize_plan(
    store: &MindStore,
    session_id: &str,
    pane_id: &str,
    latest_conversation_id: Option<&str>,
    watermark_scope: &str,
) -> Result<SessionFinalizePlanOutcome, FinalizeArtifactPlanError> {
    match plan_session_finalize_artifacts(
        store,
        session_id,
        pane_id,
        latest_conversation_id,
        watermark_scope,
    )? {
        FinalizeArtifactSelection::NoNewArtifacts => Ok(SessionFinalizePlanOutcome::Skip {
            observer_reason: "finalize skipped: no new artifacts",
            outcome_reason_suffix: "no new finalized artifacts",
        }),
        FinalizeArtifactSelection::NoExportableArtifacts { .. } => {
            Ok(SessionFinalizePlanOutcome::Skip {
                observer_reason: "finalize skipped: no t1/t2 artifacts",
                outcome_reason_suffix: "no t1/t2 artifacts available",
            })
        }
        FinalizeArtifactSelection::Ready(plan) => Ok(SessionFinalizePlanOutcome::Ready(plan)),
    }
}

pub fn build_project_mind_export(
    store: &MindStore,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<String, T3ExportError> {
    let active_entries = store.active_canon_entries(None)?;
    let stale_entries = store.canon_entries_by_state(CanonRevisionState::Stale, None)?;
    Ok(render_project_mind_markdown(
        &active_entries,
        &stale_entries,
        now,
    ))
}

pub fn build_handshake_export(
    store: &MindStore,
    project_snapshot: Option<&HandshakeProjectSnapshot>,
    active_tag: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<HandshakeExportBundle, T3ExportError> {
    let mut entries = Vec::new();
    if let Some(tag) = active_tag.filter(|value| !value.trim().is_empty()) {
        let mut tagged = store.active_canon_entries(Some(tag))?;
        entries.append(&mut tagged);
    }

    let all_active = store.active_canon_entries(None)?;
    for entry in all_active {
        if entries
            .iter()
            .any(|existing| existing.entry_id == entry.entry_id)
        {
            continue;
        }
        entries.push(entry);
    }

    if entries.len() > MIND_T3_HANDSHAKE_MAX_ITEMS {
        entries.truncate(MIND_T3_HANDSHAKE_MAX_ITEMS);
    }

    let mut selected = Vec::new();
    let mut payload = render_handshake_markdown(&selected, project_snapshot, active_tag, now);
    let mut token_estimate = estimate_text_tokens(&payload);

    for entry in entries {
        let mut candidate = selected.clone();
        candidate.push(entry);
        let candidate_payload =
            render_handshake_markdown(&candidate, project_snapshot, active_tag, now);
        let candidate_tokens = estimate_text_tokens(&candidate_payload);
        if selected.is_empty() || candidate_tokens <= MIND_T3_HANDSHAKE_TOKEN_BUDGET {
            selected = candidate;
            payload = candidate_payload;
            token_estimate = candidate_tokens;
        } else {
            break;
        }
    }

    let payload_hash = canonical_payload_hash(&payload)?;
    Ok(HandshakeExportBundle {
        payload,
        payload_hash,
        token_estimate,
    })
}

pub fn plan_session_finalize_artifacts(
    store: &MindStore,
    session_id: &str,
    pane_id: &str,
    latest_conversation_id: Option<&str>,
    watermark_scope: &str,
) -> Result<FinalizeArtifactSelection, FinalizeArtifactPlanError> {
    let watermark = store.project_watermark(watermark_scope)?;
    let (conversation_ids, delta_artifacts) = collect_delta_artifacts(
        store,
        session_id,
        latest_conversation_id,
        watermark.as_ref(),
    )?;

    if delta_artifacts.is_empty() {
        return Ok(FinalizeArtifactSelection::NoNewArtifacts);
    }

    let active_tag = resolve_session_active_tag(store, &conversation_ids);
    let t1_artifacts = delta_artifacts
        .iter()
        .filter(|artifact| artifact.kind == "t1")
        .cloned()
        .collect::<Vec<_>>();
    let t2_artifacts = delta_artifacts
        .iter()
        .filter(|artifact| artifact.kind == "t2")
        .cloned()
        .collect::<Vec<_>>();

    if t1_artifacts.is_empty() && t2_artifacts.is_empty() {
        return Ok(FinalizeArtifactSelection::NoExportableArtifacts {
            conversation_ids,
            active_tag,
            delta_artifacts,
        });
    }

    let first = delta_artifacts.first().expect("non-empty delta artifacts");
    let last = delta_artifacts.last().expect("non-empty delta artifacts");
    let last_artifact_ts = last.ts;
    let slice_start_id = first.artifact_id.clone();
    let slice_end_id = last.artifact_id.clone();
    let artifact_ids = delta_artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let slice_hash = canonical_payload_hash(&(
        session_id,
        pane_id,
        &slice_start_id,
        &slice_end_id,
        &artifact_ids,
    ))?;

    Ok(FinalizeArtifactSelection::Ready(FinalizeArtifactPlan {
        conversation_ids,
        active_tag,
        delta_artifacts,
        t1_artifacts,
        t2_artifacts,
        slice_start_id,
        slice_end_id,
        slice_hash,
        artifact_ids,
        last_artifact_ts,
        watermark_scope: watermark_scope.to_string(),
    }))
}

#[derive(Debug, Clone)]
pub struct DistillationConfig {
    pub t1_target_tokens: u32,
    pub t1_hard_cap_tokens: u32,
    pub t2_trigger_tokens: u32,
    pub t1_output_max_chars: usize,
    pub t2_output_max_chars: usize,
    pub enable_attribution: bool,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            t1_target_tokens: T1_PARSER_TARGET_TOKENS,
            t1_hard_cap_tokens: T1_PARSER_HARD_CAP_TOKENS,
            t2_trigger_tokens: DEFAULT_T2_TRIGGER_TOKENS,
            t1_output_max_chars: DEFAULT_T1_OUTPUT_MAX_CHARS,
            t2_output_max_chars: DEFAULT_T2_OUTPUT_MAX_CHARS,
            enable_attribution: true,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DistillationReport {
    pub t0_events_processed: usize,
    pub t1_batches_planned: usize,
    pub t1_artifacts_written: usize,
    pub t2_artifacts_written: usize,
    pub chunked_t1: bool,
    pub attribution_links_written: usize,
}

#[derive(Debug, Clone)]
struct ProducedObservation {
    artifact_id: String,
    ts: chrono::DateTime<chrono::Utc>,
    active_tag: String,
    text: String,
    estimated_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct SemanticObserverConfig {
    pub mode: SemanticRuntimeMode,
    pub profile: SemanticModelProfile,
    pub guardrails: SemanticGuardrails,
}

impl Default for SemanticObserverConfig {
    fn default() -> Self {
        Self {
            mode: SemanticRuntimeMode::SemanticWithFallback,
            profile: default_pi_observer_profile(),
            guardrails: SemanticGuardrails::default(),
        }
    }
}

pub fn default_pi_observer_profile() -> SemanticModelProfile {
    SemanticModelProfile {
        provider_name: DEFAULT_PI_OBSERVER_PROVIDER.to_string(),
        model_id: DEFAULT_PI_OBSERVER_MODEL.to_string(),
        prompt_version: DEFAULT_PI_OBSERVER_PROMPT_VERSION.to_string(),
        max_input_tokens: 4_096,
        max_output_tokens: 768,
    }
}

pub fn default_pi_reflector_profile() -> SemanticModelProfile {
    SemanticModelProfile {
        provider_name: DEFAULT_PI_REFLECTOR_PROVIDER.to_string(),
        model_id: DEFAULT_PI_REFLECTOR_MODEL.to_string(),
        prompt_version: DEFAULT_PI_REFLECTOR_PROMPT_VERSION.to_string(),
        max_input_tokens: 8_192,
        max_output_tokens: 1_024,
    }
}

pub trait PiObserverInvoker {
    fn invoke_observer(
        &self,
        canonical_input_json: &str,
        profile: &SemanticModelProfile,
        guardrails: &SemanticGuardrails,
    ) -> Result<String, SemanticAdapterError>;
}

#[derive(Debug, Default)]
pub struct NoopPiObserverInvoker;

impl PiObserverInvoker for NoopPiObserverInvoker {
    fn invoke_observer(
        &self,
        _canonical_input_json: &str,
        _profile: &SemanticModelProfile,
        _guardrails: &SemanticGuardrails,
    ) -> Result<String, SemanticAdapterError> {
        Err(SemanticAdapterError::new(
            SemanticFailureKind::ProviderError,
            "pi observer runtime is not configured",
        ))
    }
}

#[derive(Debug)]
pub struct PiObserverAdapter<I = NoopPiObserverInvoker> {
    invoker: I,
}

impl Default for PiObserverAdapter<NoopPiObserverInvoker> {
    fn default() -> Self {
        Self {
            invoker: NoopPiObserverInvoker,
        }
    }
}

impl<I> PiObserverAdapter<I> {
    pub fn new(invoker: I) -> Self {
        Self { invoker }
    }
}

impl<I> ObserverAdapter for PiObserverAdapter<I>
where
    I: PiObserverInvoker,
{
    fn observe_t1(
        &self,
        input: &ObserverInput,
        profile: &SemanticModelProfile,
        guardrails: &SemanticGuardrails,
    ) -> Result<ObserverOutput, SemanticAdapterError> {
        let canonical_input_json = canonical_json(input).map_err(|err| {
            SemanticAdapterError::new(
                SemanticFailureKind::InvalidOutput,
                format!("failed to serialize observer input: {err}"),
            )
        })?;

        let raw = self
            .invoker
            .invoke_observer(&canonical_input_json, profile, guardrails)?;

        ObserverOutput::parse_json(&raw).map_err(|err| {
            SemanticAdapterError::new(
                SemanticFailureKind::InvalidOutput,
                format!("failed to parse observer output: {err}"),
            )
        })
    }
}

pub struct SemanticObserverDistiller<A: ObserverAdapter> {
    config: DistillationConfig,
    semantic: SemanticObserverConfig,
    adapter: A,
}

impl<A: ObserverAdapter> SemanticObserverDistiller<A> {
    pub fn new(config: DistillationConfig, semantic: SemanticObserverConfig, adapter: A) -> Self {
        Self {
            config,
            semantic,
            adapter,
        }
    }

    pub fn distill_conversation(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<DistillationReport, DistillationError> {
        if self.semantic.mode == SemanticRuntimeMode::DeterministicOnly {
            return DeterministicDistiller::new(self.config.clone())
                .distill_conversation(store, conversation_id);
        }

        self.distill_with_semantic_t1(store, conversation_id)
    }

    fn observe_t1_with_guardrails(
        &self,
        input: &ObserverInput,
    ) -> (
        Result<ObserverOutput, SemanticAdapterError>,
        u16,
        Option<u64>,
    ) {
        let max_attempts = u16::from(self.semantic.guardrails.max_retries)
            .saturating_add(1)
            .max(1);
        let mut attempt = 1_u16;

        loop {
            if let Err(error) = enforce_observer_budget_guardrails(
                input.estimated_tokens,
                None,
                &self.semantic.profile,
                &self.semantic.guardrails,
            ) {
                return (Err(error), attempt, None);
            }

            let started_at = Utc::now();
            let observed =
                self.adapter
                    .observe_t1(input, &self.semantic.profile, &self.semantic.guardrails);
            let latency_ms = (Utc::now() - started_at).num_milliseconds().max(0) as u64;

            let guarded = observed.and_then(|output| {
                enforce_observer_budget_guardrails(
                    input.estimated_tokens,
                    Some(estimate_observer_output_tokens(&output)),
                    &self.semantic.profile,
                    &self.semantic.guardrails,
                )?;

                if self.semantic.guardrails.timeout_ms > 0
                    && latency_ms > self.semantic.guardrails.timeout_ms
                {
                    return Err(SemanticAdapterError::new(
                        SemanticFailureKind::Timeout,
                        format!(
                            "semantic observer exceeded timeout guardrail: {latency_ms}ms > {}ms",
                            self.semantic.guardrails.timeout_ms
                        ),
                    ));
                }

                Ok(output)
            });

            match guarded {
                Ok(output) => return (Ok(output), attempt, Some(latency_ms)),
                Err(error) => {
                    let retryable = matches!(
                        error.kind,
                        SemanticFailureKind::Timeout
                            | SemanticFailureKind::ProviderError
                            | SemanticFailureKind::LockConflict
                    );

                    if retryable && attempt < max_attempts {
                        attempt = attempt.saturating_add(1);
                        continue;
                    }

                    return (Err(error), attempt, Some(latency_ms));
                }
            }
        }
    }

    fn distill_with_semantic_t1(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<DistillationReport, DistillationError> {
        let t0_events = store.t0_events_for_conversation(conversation_id)?;
        if t0_events.is_empty() {
            return Ok(DistillationReport::default());
        }
        let context_states = store.context_states(conversation_id)?;

        let semantic_input_limit = self.semantic.profile.max_input_tokens.max(1);
        let t1_target_tokens = self.config.t1_target_tokens.min(semantic_input_limit);
        let t1_hard_cap_tokens = self.config.t1_hard_cap_tokens.min(semantic_input_limit);
        let batches = plan_t1_batches(&t0_events, t1_target_tokens, t1_hard_cap_tokens)?;
        let event_lookup = t0_events
            .iter()
            .map(|event| (event.compact_id.clone(), event))
            .collect::<BTreeMap<_, _>>();

        let mut report = DistillationReport {
            t0_events_processed: t0_events.len(),
            t1_batches_planned: batches.len(),
            chunked_t1: batches.len() > 1,
            ..DistillationReport::default()
        };

        let mut observations = Vec::new();

        for (batch_index, batch) in batches.iter().enumerate() {
            let mut batch_events = Vec::with_capacity(batch.compact_event_ids.len());
            for compact_id in &batch.compact_event_ids {
                let event = event_lookup.get(compact_id).ok_or_else(|| {
                    DistillationError::Internal(format!("missing compact event: {compact_id}"))
                })?;
                batch_events.push(*event);
            }

            let ts = batch_events
                .last()
                .map(|event| event.ts)
                .ok_or_else(|| DistillationError::Internal("empty T1 batch".to_string()))?;
            let active_tag = active_tag_for_ts(&context_states, ts)
                .unwrap_or_else(|| "global".to_string())
                .to_lowercase();
            let artifact_id = deterministic_artifact_id(
                "obs",
                conversation_id,
                &batch.compact_event_ids,
                self.config.t1_output_max_chars as u64,
            );

            let observer_input = ObserverInput::new(
                conversation_id,
                active_tag.clone(),
                batch.compact_event_ids.clone(),
                observer_payload_lines(&batch_events),
                batch.estimated_tokens,
                self.semantic.profile.prompt_version.clone(),
            )?;

            let (semantic_result, semantic_attempts, latency_ms) =
                self.observe_t1_with_guardrails(&observer_input);

            match semantic_result {
                Ok(output) => {
                    let text = synthesize_semantic_observation_text(
                        &output,
                        self.config.t1_output_max_chars,
                    );
                    store.insert_observation(
                        &artifact_id,
                        conversation_id,
                        ts,
                        &text,
                        &batch.compact_event_ids,
                    )?;

                    store.upsert_semantic_provenance(&SemanticProvenance {
                        artifact_id: artifact_id.clone(),
                        stage: SemanticStage::T1Observer,
                        runtime: SemanticRuntime::PiSemantic,
                        provider_name: Some(self.semantic.profile.provider_name.clone()),
                        model_id: Some(self.semantic.profile.model_id.clone()),
                        prompt_version: self.semantic.profile.prompt_version.clone(),
                        input_hash: observer_input.input_hash,
                        output_hash: Some(canonical_payload_hash(&output)?),
                        latency_ms,
                        attempt_count: semantic_attempts,
                        fallback_used: false,
                        fallback_reason: None,
                        failure_kind: None,
                        created_at: ts,
                    })?;

                    observations.push(ProducedObservation {
                        artifact_id,
                        ts,
                        active_tag,
                        estimated_tokens: estimate_tokens(&text),
                        text,
                    });
                }
                Err(error) => {
                    let fallback_text = synthesize_observation_text(
                        conversation_id,
                        batch_index + 1,
                        batches.len(),
                        batch,
                        &batch_events,
                        self.config.t1_output_max_chars,
                    );
                    store.insert_observation(
                        &artifact_id,
                        conversation_id,
                        ts,
                        &fallback_text,
                        &batch.compact_event_ids,
                    )?;

                    store.upsert_semantic_provenance(&SemanticProvenance {
                        artifact_id: artifact_id.clone(),
                        stage: SemanticStage::T1Observer,
                        runtime: SemanticRuntime::PiSemantic,
                        provider_name: Some(self.semantic.profile.provider_name.clone()),
                        model_id: Some(self.semantic.profile.model_id.clone()),
                        prompt_version: self.semantic.profile.prompt_version.clone(),
                        input_hash: observer_input.input_hash.clone(),
                        output_hash: None,
                        latency_ms,
                        attempt_count: semantic_attempts,
                        fallback_used: true,
                        fallback_reason: Some(error.message.clone()),
                        failure_kind: Some(error.kind),
                        created_at: ts,
                    })?;

                    store.upsert_semantic_provenance(&SemanticProvenance {
                        artifact_id: artifact_id.clone(),
                        stage: SemanticStage::T1Observer,
                        runtime: SemanticRuntime::Deterministic,
                        provider_name: None,
                        model_id: None,
                        prompt_version: "deterministic.observer.v1".to_string(),
                        input_hash: canonical_payload_hash(&(
                            conversation_id,
                            &batch.compact_event_ids,
                            batch.estimated_tokens,
                        ))?,
                        output_hash: Some(canonical_payload_hash(&fallback_text)?),
                        latency_ms: None,
                        attempt_count: semantic_attempts.saturating_add(1),
                        fallback_used: true,
                        fallback_reason: Some(format!(
                            "semantic observer failed ({})",
                            error.kind.as_str()
                        )),
                        failure_kind: Some(error.kind),
                        created_at: ts,
                    })?;

                    observations.push(ProducedObservation {
                        artifact_id,
                        ts,
                        active_tag,
                        estimated_tokens: estimate_tokens(&fallback_text),
                        text: fallback_text,
                    });
                }
            }

            report.t1_artifacts_written += 1;
        }

        let deterministic = DeterministicDistiller::new(self.config.clone());
        report.t2_artifacts_written = deterministic.emit_reflections(
            store,
            conversation_id,
            &observations,
            self.config.t2_trigger_tokens,
            self.config.t2_output_max_chars,
        )?;

        if self.config.enable_attribution {
            let attribution = TaskAttributionEngine::new(AttributionConfig::default())
                .attribute_conversation(store, conversation_id)?;
            report.attribution_links_written = attribution.links_written;
        }

        Ok(report)
    }
}

#[derive(Debug)]
pub struct SessionObserverRunOutcome {
    pub session_id: String,
    pub conversation_id: String,
    pub trigger: ObserverTrigger,
    pub enqueued_at: chrono::DateTime<chrono::Utc>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub report: Result<DistillationReport, DistillationError>,
    pub progress: Option<MindObserverFeedProgress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserverDrainState {
    pub events: Vec<MindObserverFeedEvent>,
    pub pending_count: usize,
    pub has_active_run: bool,
}

impl ObserverDrainState {
    pub fn is_idle(&self) -> bool {
        self.pending_count == 0 && !self.has_active_run
    }
}

pub fn observer_feed_event_from_outcome(
    store: &MindStore,
    outcome: &SessionObserverRunOutcome,
    completed_at: chrono::DateTime<chrono::Utc>,
) -> MindObserverFeedEvent {
    let mut event = MindObserverFeedEvent {
        status: MindObserverFeedStatus::Success,
        trigger: observer_feed_trigger(outcome.trigger.kind),
        conversation_id: Some(outcome.conversation_id.clone()),
        runtime: None,
        attempt_count: None,
        latency_ms: None,
        reason: None,
        failure_kind: None,
        enqueued_at: Some(outcome.enqueued_at.to_rfc3339()),
        started_at: Some(outcome.started_at.to_rfc3339()),
        completed_at: Some(completed_at.to_rfc3339()),
        progress: outcome.progress.clone(),
    };

    match &outcome.report {
        Err(error) => {
            event.status = MindObserverFeedStatus::Error;
            event.reason = Some(error.to_string());
            event
        }
        Ok(report) => {
            if report.t1_artifacts_written == 0 {
                event.reason = Some("observer run produced no t1 artifacts".to_string());
                return event;
            }

            let Some((runtime, attempt_count, latency_ms, fallback_used, reason, failure_kind)) =
                latest_t1_provenance_summary(store, &outcome.conversation_id)
            else {
                return event;
            };

            event.runtime = Some(runtime);
            event.attempt_count = Some(attempt_count);
            event.latency_ms = latency_ms;
            event.reason = reason;
            event.failure_kind = failure_kind;
            event.status = if fallback_used {
                MindObserverFeedStatus::Fallback
            } else {
                MindObserverFeedStatus::Success
            };
            event
        }
    }
}

pub fn enqueue_observer_and_run_events<A: ObserverAdapter>(
    sidecar: &mut SessionObserverSidecar<A>,
    store: &MindStore,
    session_id: &str,
    conversation_id: &str,
    trigger: MindObserverFeedTriggerKind,
    reason: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
    debounce_run_ms: i64,
) -> Vec<MindObserverFeedEvent> {
    match trigger {
        MindObserverFeedTriggerKind::TokenThreshold => {
            sidecar.enqueue_token_threshold(session_id, conversation_id, now)
        }
        MindObserverFeedTriggerKind::TaskCompleted => {
            sidecar.enqueue_task_completed(session_id, conversation_id, now)
        }
        MindObserverFeedTriggerKind::ManualShortcut => {
            sidecar.enqueue_manual(session_id, conversation_id, now)
        }
        MindObserverFeedTriggerKind::Handoff => {
            sidecar.enqueue_handoff(session_id, conversation_id, now)
        }
        MindObserverFeedTriggerKind::Compaction => {
            sidecar.enqueue_compaction(session_id, conversation_id, now)
        }
    }

    let progress = observer_feed_progress(store, conversation_id, &sidecar.distiller.config);
    let mut events = vec![MindObserverFeedEvent {
        status: MindObserverFeedStatus::Queued,
        trigger,
        conversation_id: Some(conversation_id.to_string()),
        runtime: None,
        attempt_count: None,
        latency_ms: None,
        reason,
        failure_kind: None,
        enqueued_at: Some(now.to_rfc3339()),
        started_at: None,
        completed_at: None,
        progress,
    }];

    let run_at = if trigger == MindObserverFeedTriggerKind::TokenThreshold {
        now + chrono::Duration::milliseconds(debounce_run_ms)
    } else {
        now
    };
    events.extend(run_observer_ready_events(
        sidecar,
        store,
        run_at,
        Utc::now(),
    ));
    events
}

pub fn run_observer_ready_events<A: ObserverAdapter>(
    sidecar: &mut SessionObserverSidecar<A>,
    store: &MindStore,
    run_at: chrono::DateTime<chrono::Utc>,
    completed_at: chrono::DateTime<chrono::Utc>,
) -> Vec<MindObserverFeedEvent> {
    drain_observer_state(sidecar, store, None, run_at, completed_at).events
}

pub fn drain_observer_state<A: ObserverAdapter>(
    sidecar: &mut SessionObserverSidecar<A>,
    store: &MindStore,
    session_id: Option<&str>,
    run_at: chrono::DateTime<chrono::Utc>,
    completed_at: chrono::DateTime<chrono::Utc>,
) -> ObserverDrainState {
    let mut events = Vec::new();
    let outcomes = sidecar.run_ready(store, run_at);
    for outcome in outcomes {
        events.push(MindObserverFeedEvent {
            status: MindObserverFeedStatus::Running,
            trigger: observer_feed_trigger(outcome.trigger.kind),
            conversation_id: Some(outcome.conversation_id.clone()),
            runtime: None,
            attempt_count: None,
            latency_ms: None,
            reason: None,
            failure_kind: None,
            enqueued_at: Some(outcome.enqueued_at.to_rfc3339()),
            started_at: Some(outcome.started_at.to_rfc3339()),
            completed_at: None,
            progress: outcome.progress.clone(),
        });
        events.push(observer_feed_event_from_outcome(
            store,
            &outcome,
            completed_at,
        ));
    }

    let pending_count = session_id
        .map(|id| sidecar.queue().pending_count(id))
        .unwrap_or(0);
    let has_active_run = session_id
        .map(|id| sidecar.queue().has_active_run(id))
        .unwrap_or(false);

    ObserverDrainState {
        events,
        pending_count,
        has_active_run,
    }
}

fn observer_feed_progress(
    store: &MindStore,
    conversation_id: &str,
    config: &DistillationConfig,
) -> Option<MindObserverFeedProgress> {
    let t0_events = store.t0_events_for_conversation(conversation_id).ok()?;
    let t0_estimated_tokens = t0_events.iter().fold(0_u32, |total, event| {
        total.saturating_add(estimate_t0_event_tokens(event))
    });

    Some(MindObserverFeedProgress {
        t0_estimated_tokens,
        t1_target_tokens: config.t1_target_tokens,
        t1_hard_cap_tokens: config.t1_hard_cap_tokens,
        tokens_until_next_run: config.t1_target_tokens.saturating_sub(t0_estimated_tokens),
    })
}

fn latest_t3_artifact_by_entry(
    project_root: &str,
    topic: Option<&str>,
    delta: &[StoredArtifact],
) -> Result<BTreeMap<String, StoredArtifact>, T3BacklogJobError> {
    let mut latest_by_entry: BTreeMap<String, StoredArtifact> = BTreeMap::new();
    for artifact in delta.iter().cloned() {
        let entry_id = project_canon_entry_id_for_artifact(project_root, topic, &artifact)?;
        if let Some(current) = latest_by_entry.get(&entry_id) {
            let should_replace = artifact.ts > current.ts
                || (artifact.ts == current.ts && artifact.artifact_id > current.artifact_id);
            if !should_replace {
                continue;
            }
        }
        latest_by_entry.insert(entry_id, artifact);
    }
    Ok(latest_by_entry)
}

fn project_canon_entry_id_for_artifact(
    project_root: &str,
    topic: Option<&str>,
    artifact: &StoredArtifact,
) -> Result<String, T3BacklogJobError> {
    let digest = canonical_payload_hash(&(
        project_root,
        topic,
        artifact.conversation_id.as_str(),
        artifact.kind.as_str(),
    ))?;
    Ok(format!("canon:{}", &digest[..16]))
}

fn project_canon_summary(artifact: &StoredArtifact) -> String {
    let heading = if artifact.kind == "t2" {
        "Reflection"
    } else {
        "Observation"
    };
    let preview = truncate_chars(
        normalize_text(&artifact.text),
        MIND_T3_CANON_SUMMARY_MAX_CHARS,
    );
    format!(
        "{heading} for {} in {}: {}",
        artifact.kind, artifact.conversation_id, preview
    )
}

fn project_canon_confidence_bps(
    now: chrono::DateTime<chrono::Utc>,
    artifact: &StoredArtifact,
    evidence_count: usize,
) -> u16 {
    let base = if artifact.kind == "t2" {
        8_200u16
    } else {
        7_100u16
    };
    let evidence_boost = ((evidence_count.saturating_sub(1) as u16).saturating_mul(220)).min(1_200);
    let recency_boost = if now - artifact.ts <= chrono::Duration::days(1) {
        300u16
    } else if now - artifact.ts <= chrono::Duration::days(7) {
        120u16
    } else {
        0u16
    };
    base.saturating_add(evidence_boost)
        .saturating_add(recency_boost)
        .min(10_000)
}

fn project_canon_freshness_score(
    now: chrono::DateTime<chrono::Utc>,
    artifact_ts: chrono::DateTime<chrono::Utc>,
) -> u16 {
    if artifact_ts >= now {
        return 10_000;
    }

    let age_hours = (now - artifact_ts).num_hours().max(0) as u16;
    let decay = age_hours.saturating_mul(12).min(10_000);
    10_000u16.saturating_sub(decay)
}

fn project_canon_evidence_refs(
    store: &MindStore,
    artifact: &StoredArtifact,
) -> Result<Vec<String>, T3BacklogJobError> {
    let mut evidence_refs = vec![artifact.artifact_id.clone()];

    for trace_id in &artifact.trace_ids {
        if trace_id == &artifact.artifact_id {
            continue;
        }
        let resolvable = store.artifact_by_id(trace_id)?.is_some();
        if resolvable {
            evidence_refs.push(trace_id.clone());
        }
    }

    evidence_refs.sort();
    evidence_refs.dedup();
    if evidence_refs.is_empty() {
        return Err(T3BacklogJobError::Internal(format!(
            "canon evidence set is empty for artifact {}",
            artifact.artifact_id
        )));
    }

    Ok(evidence_refs)
}

fn t3_scope_id_for_project_root(project_root: &str) -> String {
    format!("project:{}", project_root.to_ascii_lowercase())
}

fn collect_delta_artifacts(
    store: &MindStore,
    session_id: &str,
    latest_conversation_id: Option<&str>,
    watermark: Option<&ProjectWatermark>,
) -> Result<(Vec<String>, Vec<StoredArtifact>), StorageError> {
    let mut conversation_ids = store.conversation_ids_for_session(session_id)?;
    if let Some(conversation_id) = latest_conversation_id {
        conversation_ids.push(conversation_id.to_string());
    }
    conversation_ids.sort();
    conversation_ids.dedup();

    let mut artifacts = Vec::new();
    for conversation_id in &conversation_ids {
        let mut rows = store.artifacts_for_conversation(conversation_id)?;
        artifacts.append(&mut rows);
    }

    artifacts.sort_by(|left, right| {
        left.ts
            .cmp(&right.ts)
            .then(left.artifact_id.cmp(&right.artifact_id))
    });
    artifacts.dedup_by(|left, right| left.artifact_id == right.artifact_id);

    let delta = artifacts
        .into_iter()
        .filter(|artifact| artifact.kind == "t1" || artifact.kind == "t2")
        .filter(|artifact| artifact_after_watermark(artifact, watermark))
        .collect::<Vec<_>>();

    Ok((conversation_ids, delta))
}

fn resolve_session_active_tag(store: &MindStore, conversation_ids: &[String]) -> Option<String> {
    let mut latest: Option<(chrono::DateTime<chrono::Utc>, String)> = None;
    for conversation_id in conversation_ids {
        let states = match store.context_states(conversation_id) {
            Ok(value) => value,
            Err(_) => continue,
        };
        for state in states {
            let Some(tag) = state
                .active_tag
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
            else {
                continue;
            };

            let should_update = latest
                .as_ref()
                .map(|(ts, _)| state.ts > *ts)
                .unwrap_or(true);
            if should_update {
                latest = Some((state.ts, tag));
            }
        }
    }

    latest.map(|(_, tag)| tag)
}

fn artifact_after_watermark(
    artifact: &StoredArtifact,
    watermark: Option<&ProjectWatermark>,
) -> bool {
    let Some(watermark) = watermark else {
        return true;
    };

    match (
        watermark.last_artifact_ts,
        watermark.last_artifact_id.as_ref(),
    ) {
        (Some(last_ts), Some(last_id)) => {
            artifact.ts > last_ts || (artifact.ts == last_ts && artifact.artifact_id > *last_id)
        }
        (Some(last_ts), None) => artifact.ts > last_ts,
        (None, Some(last_id)) => artifact.artifact_id > *last_id,
        (None, None) => true,
    }
}

fn latest_t1_provenance_summary(
    store: &MindStore,
    conversation_id: &str,
) -> Option<(
    String,
    u16,
    Option<u64>,
    bool,
    Option<String>,
    Option<String>,
)> {
    let artifacts = store.artifacts_for_conversation(conversation_id).ok()?;
    let artifact_id = artifacts
        .iter()
        .rev()
        .find(|artifact| artifact.kind == "t1")?
        .artifact_id
        .clone();
    let provenance = store.semantic_provenance_for_artifact(&artifact_id).ok()?;
    if provenance.is_empty() {
        return None;
    }

    let attempt_count = provenance.last().map(|row| row.attempt_count).unwrap_or(1);
    let runtime = provenance
        .last()
        .map(|row| row.runtime.as_str().to_string())
        .unwrap_or_else(|| SemanticRuntime::Deterministic.as_str().to_string());
    let latency_ms = provenance.iter().rev().find_map(|row| row.latency_ms);
    let fallback_used = provenance.iter().any(|row| row.fallback_used);
    let reason = provenance
        .iter()
        .rev()
        .find_map(|row| row.fallback_reason.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let failure_kind = provenance
        .iter()
        .rev()
        .find_map(|row| row.failure_kind.map(|kind| kind.as_str().to_string()));

    Some((
        runtime,
        attempt_count,
        latency_ms,
        fallback_used,
        reason,
        failure_kind,
    ))
}

fn observer_feed_trigger(kind: ObserverTriggerKind) -> MindObserverFeedTriggerKind {
    match kind {
        ObserverTriggerKind::TokenThreshold => MindObserverFeedTriggerKind::TokenThreshold,
        ObserverTriggerKind::TaskCompleted => MindObserverFeedTriggerKind::TaskCompleted,
        ObserverTriggerKind::ManualShortcut => MindObserverFeedTriggerKind::ManualShortcut,
        ObserverTriggerKind::Handoff => MindObserverFeedTriggerKind::Handoff,
        ObserverTriggerKind::Compaction => MindObserverFeedTriggerKind::Compaction,
    }
}

pub struct SessionObserverSidecar<A: ObserverAdapter> {
    queue: SessionObserverQueue,
    distiller: SemanticObserverDistiller<A>,
}

impl<A: ObserverAdapter> SessionObserverSidecar<A> {
    pub fn new(config: DistillationConfig, semantic: SemanticObserverConfig, adapter: A) -> Self {
        let queue = SessionObserverQueue::new(ObserverQueueConfig {
            debounce_ms: semantic.guardrails.queue_debounce_ms,
        });
        let distiller = SemanticObserverDistiller::new(config, semantic, adapter);
        Self { queue, distiller }
    }

    pub fn enqueue_turn(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.enqueue_token_threshold(session_id, conversation_id, now);
    }

    pub fn enqueue_token_threshold(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::token_threshold(),
            now,
        );
    }

    pub fn enqueue_task_completed(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::task_completed(),
            now,
        );
    }

    pub fn enqueue_manual(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::manual_shortcut(),
            now,
        );
    }

    pub fn enqueue_handoff(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::handoff(),
            now,
        );
    }

    pub fn enqueue_compaction(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::compaction(),
            now,
        );
    }

    pub fn run_ready(
        &mut self,
        store: &MindStore,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Vec<SessionObserverRunOutcome> {
        let mut outcomes = Vec::new();

        while let Some(run) = self.queue.claim_ready(now) {
            let progress =
                observer_feed_progress(store, &run.conversation_id, &self.distiller.config);
            let report = self
                .distiller
                .distill_conversation(store, &run.conversation_id);
            self.queue.complete_run(&run, now);
            self.enqueue_branch_backfill(store, &run.session_id, &run.conversation_id, now);
            outcomes.push(SessionObserverRunOutcome {
                session_id: run.session_id,
                conversation_id: run.conversation_id,
                trigger: run.trigger,
                enqueued_at: run.enqueued_at,
                started_at: run.started_at,
                report,
                progress,
            });
        }

        outcomes
    }

    fn enqueue_branch_backfill(
        &mut self,
        store: &MindStore,
        session_id: &str,
        conversation_id: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        let conversation_ids = match store.session_tree_conversations(session_id, conversation_id) {
            Ok(ids) => ids,
            Err(_) => return,
        };

        for candidate in conversation_ids {
            if candidate == conversation_id {
                continue;
            }
            match store.conversation_needs_observer_run(&candidate) {
                Ok(true) => self.queue.enqueue_with_trigger(
                    session_id.to_string(),
                    candidate,
                    ObserverTrigger {
                        kind: ObserverTriggerKind::TokenThreshold,
                        priority: ObserverTriggerPriority::Normal,
                        bypass_debounce: true,
                    },
                    now,
                ),
                Ok(false) | Err(_) => {}
            }
        }
    }

    pub fn queue(&self) -> &SessionObserverQueue {
        &self.queue
    }
}

pub struct DeterministicDistiller {
    config: DistillationConfig,
}

impl DeterministicDistiller {
    pub fn new(config: DistillationConfig) -> Self {
        Self { config }
    }

    pub fn distill_conversation(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<DistillationReport, DistillationError> {
        let t0_events = store.t0_events_for_conversation(conversation_id)?;
        if t0_events.is_empty() {
            return Ok(DistillationReport::default());
        }
        let context_states = store.context_states(conversation_id)?;

        let batches = plan_t1_batches(
            &t0_events,
            self.config.t1_target_tokens,
            self.config.t1_hard_cap_tokens,
        )?;
        let event_lookup = t0_events
            .iter()
            .map(|event| (event.compact_id.clone(), event))
            .collect::<BTreeMap<_, _>>();

        let mut report = DistillationReport {
            t0_events_processed: t0_events.len(),
            t1_batches_planned: batches.len(),
            chunked_t1: batches.len() > 1,
            ..DistillationReport::default()
        };

        let mut observations = Vec::new();

        for (batch_index, batch) in batches.iter().enumerate() {
            let mut batch_events = Vec::with_capacity(batch.compact_event_ids.len());
            for compact_id in &batch.compact_event_ids {
                let event = event_lookup.get(compact_id).ok_or_else(|| {
                    DistillationError::Internal(format!("missing compact event: {compact_id}"))
                })?;
                batch_events.push(*event);
            }

            let text = synthesize_observation_text(
                conversation_id,
                batch_index + 1,
                batches.len(),
                batch,
                &batch_events,
                self.config.t1_output_max_chars,
            );
            let artifact_id = deterministic_artifact_id(
                "obs",
                conversation_id,
                &batch.compact_event_ids,
                self.config.t1_output_max_chars as u64,
            );
            let ts = batch_events
                .last()
                .map(|event| event.ts)
                .ok_or_else(|| DistillationError::Internal("empty T1 batch".to_string()))?;

            store.insert_observation(
                &artifact_id,
                conversation_id,
                ts,
                &text,
                &batch.compact_event_ids,
            )?;
            persist_deterministic_provenance(
                store,
                &artifact_id,
                SemanticStage::T1Observer,
                "deterministic.observer.v1",
                canonical_payload_hash(&(
                    conversation_id,
                    &batch.compact_event_ids,
                    batch.estimated_tokens,
                ))?,
                Some(canonical_payload_hash(&text)?),
                ts,
            )?;

            let active_tag = active_tag_for_ts(&context_states, ts)
                .unwrap_or_else(|| "global".to_string())
                .to_lowercase();

            observations.push(ProducedObservation {
                artifact_id,
                ts,
                active_tag,
                estimated_tokens: estimate_tokens(&text),
                text,
            });
            report.t1_artifacts_written += 1;
        }

        report.t2_artifacts_written = self.emit_reflections(
            store,
            conversation_id,
            &observations,
            self.config.t2_trigger_tokens,
            self.config.t2_output_max_chars,
        )?;

        if self.config.enable_attribution {
            let attribution = TaskAttributionEngine::new(AttributionConfig::default())
                .attribute_conversation(store, conversation_id)?;
            report.attribution_links_written = attribution.links_written;
        }

        Ok(report)
    }

    fn emit_reflections(
        &self,
        store: &MindStore,
        conversation_id: &str,
        observations: &[ProducedObservation],
        trigger_tokens: u32,
        max_chars: usize,
    ) -> Result<usize, DistillationError> {
        if observations.is_empty() || trigger_tokens == 0 {
            return Ok(0);
        }

        let mut by_tag = BTreeMap::<String, Vec<&ProducedObservation>>::new();
        for observation in observations {
            by_tag
                .entry(observation.active_tag.clone())
                .or_default()
                .push(observation);
        }

        let mut created = 0usize;
        for (tag, tagged_observations) in by_tag {
            let refs = tagged_observations
                .iter()
                .map(|observation| ObservationRef {
                    artifact_id: observation.artifact_id.clone(),
                    conversation_id: conversation_id.to_string(),
                    active_tag: tag.clone(),
                })
                .collect::<Vec<_>>();
            let _ = build_t2_workstream_batch(&tag, &refs)?;

            let total_tokens = tagged_observations
                .iter()
                .map(|observation| observation.estimated_tokens)
                .sum::<u32>();
            if total_tokens < trigger_tokens {
                continue;
            }

            let chunks = chunk_observations_for_t2(&tagged_observations, trigger_tokens);
            for (chunk_index, chunk) in chunks.iter().enumerate() {
                let obs_ids = chunk
                    .iter()
                    .map(|observation| observation.artifact_id.clone())
                    .collect::<Vec<_>>();
                let text = synthesize_reflection_text(
                    &tag,
                    chunk_index + 1,
                    chunks.len(),
                    chunk,
                    max_chars,
                );
                let ts = chunk
                    .last()
                    .map(|observation| observation.ts)
                    .ok_or_else(|| DistillationError::Internal("empty T2 chunk".to_string()))?;
                let artifact_id =
                    deterministic_artifact_id("ref", conversation_id, &obs_ids, max_chars as u64);

                store.insert_reflection(&artifact_id, conversation_id, ts, &text, &obs_ids)?;
                persist_deterministic_provenance(
                    store,
                    &artifact_id,
                    SemanticStage::T2Reflector,
                    "deterministic.reflector.v1",
                    canonical_payload_hash(&(conversation_id, &tag, &obs_ids))?,
                    Some(canonical_payload_hash(&text)?),
                    ts,
                )?;
                created += 1;
            }
        }

        Ok(created)
    }
}

fn plan_t1_batches(
    t0_events: &[StoredCompactEvent],
    target_tokens: u32,
    hard_cap_tokens: u32,
) -> Result<Vec<T1Batch>, DistillationError> {
    if t0_events.is_empty() {
        return Ok(Vec::new());
    }

    let first_conversation = t0_events[0].conversation_id.clone();
    let mut total_tokens = 0_u32;
    let mut compact_ids = Vec::with_capacity(t0_events.len());

    for event in t0_events {
        if event.conversation_id != first_conversation {
            return Err(DistillationError::Contract(
                MindContractError::T1CrossConversation {
                    expected: first_conversation.clone(),
                    found: event.conversation_id.clone(),
                },
            ));
        }
        let event_tokens = estimate_t0_event_tokens(event);
        total_tokens = total_tokens.saturating_add(event_tokens);
        compact_ids.push(event.compact_id.clone());
    }
    if total_tokens <= target_tokens {
        let batch = T1Batch {
            conversation_id: first_conversation,
            compact_event_ids: compact_ids,
            estimated_tokens: total_tokens,
        };
        batch.validate_hard_cap(hard_cap_tokens)?;
        return Ok(vec![batch]);
    }

    let mut batches = Vec::new();
    let mut current_ids = Vec::new();
    let mut current_tokens = 0_u32;

    for event in t0_events {
        let event_tokens = estimate_t0_event_tokens(event);
        if event_tokens > hard_cap_tokens {
            return Err(DistillationError::Contract(
                MindContractError::T1OverHardCap {
                    estimated_tokens: event_tokens,
                    hard_cap: hard_cap_tokens,
                },
            ));
        }

        if !current_ids.is_empty() && current_tokens.saturating_add(event_tokens) > target_tokens {
            batches.push(T1Batch {
                conversation_id: event.conversation_id.clone(),
                compact_event_ids: current_ids,
                estimated_tokens: current_tokens,
            });
            current_ids = Vec::new();
            current_tokens = 0;
        }

        current_ids.push(event.compact_id.clone());
        current_tokens = current_tokens.saturating_add(event_tokens);
    }

    if !current_ids.is_empty() {
        batches.push(T1Batch {
            conversation_id: t0_events[0].conversation_id.clone(),
            compact_event_ids: current_ids,
            estimated_tokens: current_tokens,
        });
    }

    validate_t1_scope(&batches)?;
    for batch in &batches {
        batch.validate_hard_cap(hard_cap_tokens)?;
    }
    Ok(batches)
}

fn chunk_observations_for_t2<'a>(
    observations: &'a [&'a ProducedObservation],
    trigger_tokens: u32,
) -> Vec<Vec<&'a ProducedObservation>> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_tokens = 0_u32;

    for observation in observations {
        let tokens = observation.estimated_tokens.max(1);
        if !current.is_empty() && current_tokens.saturating_add(tokens) > trigger_tokens {
            chunks.push(current);
            current = Vec::new();
            current_tokens = 0;
        }

        current.push(*observation);
        current_tokens = current_tokens.saturating_add(tokens);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn synthesize_observation_text(
    conversation_id: &str,
    chunk_index: usize,
    chunk_count: usize,
    batch: &T1Batch,
    events: &[&StoredCompactEvent],
    max_chars: usize,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "T1 observation {chunk_index}/{chunk_count} for {conversation_id}; tokens={} events={}",
        batch.estimated_tokens,
        events.len()
    ));

    for event in events {
        if let Some(text) = event.text.as_deref() {
            let role = event.role.map(role_label).unwrap_or("message");
            lines.push(format!("{role}: {}", normalize_text(text)));
            continue;
        }

        if let Some(tool_meta) = event.tool_meta.as_ref() {
            lines.push(format!(
                "tool:{} status={:?} latency_ms={} exit_code={} output_bytes={} redacted={}",
                tool_meta.tool_name,
                tool_meta.status,
                tool_meta
                    .latency_ms
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta
                    .exit_code
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta.output_bytes,
                tool_meta.redacted
            ));
        }
    }

    truncate_chars(lines.join("\n"), max_chars)
}

fn observer_payload_lines(events: &[&StoredCompactEvent]) -> Vec<String> {
    let mut lines = Vec::new();

    for event in events {
        if let Some(text) = event.text.as_deref() {
            let role = event.role.map(role_label).unwrap_or("message");
            lines.push(format!("{role}: {}", normalize_text(text)));
            continue;
        }

        if let Some(tool_meta) = event.tool_meta.as_ref() {
            lines.push(format!(
                "tool:{} status={:?} latency_ms={} exit_code={} output_bytes={} redacted={}",
                tool_meta.tool_name,
                tool_meta.status,
                tool_meta
                    .latency_ms
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta
                    .exit_code
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta.output_bytes,
                tool_meta.redacted
            ));
        }
    }

    lines
}

fn synthesize_semantic_observation_text(output: &ObserverOutput, max_chars: usize) -> String {
    let mut lines = Vec::new();
    lines.push(output.summary.trim().to_string());

    for point in &output.key_points {
        lines.push(format!("- {}", point.trim()));
    }

    if !output.citations.is_empty() {
        lines.push(format!("citations: {}", output.citations.join(", ")));
    }

    truncate_chars(lines.join("\n"), max_chars)
}

fn estimate_observer_output_tokens(output: &ObserverOutput) -> u32 {
    let mut chars = output.summary.chars().count();
    chars += output
        .key_points
        .iter()
        .map(|point| point.chars().count() + 2)
        .sum::<usize>();
    chars += output
        .citations
        .iter()
        .map(|citation| citation.chars().count() + 2)
        .sum::<usize>();
    ((chars as u32) / 4).max(1)
}

fn estimate_semantic_cost_micros(tokens: u32) -> u64 {
    u64::from(tokens).saturating_mul(DEFAULT_SEMANTIC_COST_MICROS_PER_TOKEN)
}

fn enforce_observer_budget_guardrails(
    input_tokens: u32,
    output_tokens: Option<u32>,
    profile: &SemanticModelProfile,
    guardrails: &SemanticGuardrails,
) -> Result<(), SemanticAdapterError> {
    if input_tokens > profile.max_input_tokens {
        return Err(SemanticAdapterError::new(
            SemanticFailureKind::BudgetExceeded,
            format!(
                "observer input tokens exceed profile limit: {input_tokens} > {}",
                profile.max_input_tokens
            ),
        ));
    }

    if let Some(output_tokens) = output_tokens {
        if output_tokens > profile.max_output_tokens {
            return Err(SemanticAdapterError::new(
                SemanticFailureKind::BudgetExceeded,
                format!(
                    "observer output tokens exceed profile limit: {output_tokens} > {}",
                    profile.max_output_tokens
                ),
            ));
        }
    }

    let budget_tokens = if guardrails.max_budget_tokens == 0 {
        u32::MAX
    } else {
        guardrails.max_budget_tokens
    };

    let observed_tokens = input_tokens.saturating_add(output_tokens.unwrap_or(0));
    if observed_tokens > budget_tokens {
        return Err(SemanticAdapterError::new(
            SemanticFailureKind::BudgetExceeded,
            format!("observer token budget exceeded: {observed_tokens} > {budget_tokens}"),
        ));
    }

    if guardrails.max_budget_cost_micros > 0 {
        let projected_output = output_tokens.unwrap_or(profile.max_output_tokens);
        let projected_tokens = input_tokens.saturating_add(projected_output);
        let projected_cost = estimate_semantic_cost_micros(projected_tokens);
        if projected_cost > guardrails.max_budget_cost_micros {
            return Err(SemanticAdapterError::new(
                SemanticFailureKind::BudgetExceeded,
                format!(
                    "observer cost budget exceeded: {projected_cost} > {} micros",
                    guardrails.max_budget_cost_micros
                ),
            ));
        }
    }

    Ok(())
}

fn load_reflector_job_observations(
    store: &MindStore,
    job: &ReflectorJob,
) -> Result<Vec<StoredArtifact>, StorageError> {
    let mut observations = Vec::new();
    for conversation_id in &job.conversation_ids {
        let artifacts = store.artifacts_for_conversation(conversation_id)?;
        for artifact in artifacts {
            if artifact.kind == "t1" && job.observation_ids.contains(&artifact.artifact_id) {
                observations.push(artifact);
            }
        }
    }
    observations.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    observations.dedup_by(|left, right| left.artifact_id == right.artifact_id);
    Ok(observations)
}

fn synthesize_reflector_job_text(
    tag: &str,
    observations: &[StoredArtifact],
    max_chars: usize,
) -> String {
    let mut lines = vec![format!(
        "T2 runtime reflection for tag={tag} observations={}",
        observations.len()
    )];
    for observation in observations {
        let preview = truncate_chars(normalize_text(&observation.text), 180);
        lines.push(format!("{}: {}", observation.artifact_id, preview));
    }
    truncate_chars(lines.join("\n"), max_chars)
}

fn synthesize_reflection_text(
    tag: &str,
    chunk_index: usize,
    chunk_count: usize,
    observations: &[&ProducedObservation],
    max_chars: usize,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "T2 reflection {chunk_index}/{chunk_count} for tag={tag}; observations={}",
        observations.len()
    ));

    for observation in observations {
        let preview = truncate_chars(normalize_text(&observation.text), 180);
        lines.push(format!("{}: {}", observation.artifact_id, preview));
    }

    truncate_chars(lines.join("\n"), max_chars)
}

fn deterministic_artifact_id(
    prefix: &str,
    conversation_id: &str,
    trace_ids: &[String],
    budget: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(conversation_id.as_bytes());
    hasher.update(b"|");
    hasher.update(prefix.as_bytes());
    hasher.update(b"|");
    hasher.update(budget.to_string().as_bytes());

    let mut sorted = trace_ids.to_vec();
    sorted.sort();
    for trace_id in sorted {
        hasher.update(b"|");
        hasher.update(trace_id.as_bytes());
    }

    let digest = hasher.finalize();
    format!("{prefix}:{}", hex_prefix(&digest, 16))
}

fn hex_prefix(digest: &[u8], bytes: usize) -> String {
    let mut output = String::with_capacity(bytes * 2);
    for byte in digest.iter().take(bytes) {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn persist_deterministic_provenance(
    store: &MindStore,
    artifact_id: &str,
    stage: SemanticStage,
    prompt_version: &str,
    input_hash: String,
    output_hash: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), DistillationError> {
    store.upsert_semantic_provenance(&SemanticProvenance {
        artifact_id: artifact_id.to_string(),
        stage,
        runtime: SemanticRuntime::Deterministic,
        provider_name: None,
        model_id: None,
        prompt_version: prompt_version.to_string(),
        input_hash,
        output_hash,
        latency_ms: None,
        attempt_count: 1,
        fallback_used: false,
        fallback_reason: None,
        failure_kind: None,
        created_at,
    })?;
    Ok(())
}

fn role_label(role: ConversationRole) -> &'static str {
    match role {
        ConversationRole::System => "system",
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    }
}

fn estimate_t0_event_tokens(event: &StoredCompactEvent) -> u32 {
    let text_tokens = event.text.as_deref().map_or(0, estimate_tokens);
    let tool_tokens = event
        .tool_meta
        .as_ref()
        .map_or(0, |meta| 14 + ((meta.output_bytes as u32) / 180));
    (text_tokens + tool_tokens).max(1)
}

fn estimate_tokens(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    (chars / 4).max(1)
}

pub fn render_project_mind_markdown(
    active_entries: &[CanonEntryRevision],
    stale_entries: &[CanonEntryRevision],
    generated_at: chrono::DateTime<chrono::Utc>,
) -> String {
    let mut lines = vec![
        "# Project Mind Canon".to_string(),
        String::new(),
        format!("_generated_at: {}_", generated_at.to_rfc3339()),
        String::new(),
        format!("active_entries: {}", active_entries.len()),
        format!("stale_entries: {}", stale_entries.len()),
        String::new(),
        "## Active canon".to_string(),
        String::new(),
    ];

    if active_entries.is_empty() {
        lines.push("(none)".to_string());
        lines.push(String::new());
    } else {
        for entry in active_entries {
            lines.push(format!("### {} r{}", entry.entry_id, entry.revision));
            if let Some(topic) = entry.topic.as_deref() {
                lines.push(format!("- topic: {topic}"));
            }
            lines.push(format!("- confidence_bps: {}", entry.confidence_bps));
            lines.push(format!("- freshness_score: {}", entry.freshness_score));
            if let Some(supersedes) = entry.supersedes_entry_id.as_deref() {
                lines.push(format!("- supersedes_entry_id: {supersedes}"));
            }
            if !entry.evidence_refs.is_empty() {
                lines.push(format!(
                    "- evidence_refs: {}",
                    entry.evidence_refs.join(", ")
                ));
            }
            lines.push(String::new());
            lines.push(entry.summary.trim().to_string());
            lines.push(String::new());
        }
    }

    lines.push("## Stale canon".to_string());
    lines.push(String::new());
    if stale_entries.is_empty() {
        lines.push("(none)".to_string());
        lines.push(String::new());
    } else {
        for entry in stale_entries {
            lines.push(format!("### {} r{}", entry.entry_id, entry.revision));
            if let Some(topic) = entry.topic.as_deref() {
                lines.push(format!("- topic: {topic}"));
            }
            lines.push(format!("- confidence_bps: {}", entry.confidence_bps));
            lines.push(format!("- freshness_score: {}", entry.freshness_score));
            if !entry.evidence_refs.is_empty() {
                lines.push(format!(
                    "- evidence_refs: {}",
                    entry.evidence_refs.join(", ")
                ));
            }
            lines.push(String::new());
            lines.push(entry.summary.trim().to_string());
            lines.push(String::new());
        }
    }

    lines.join("\n") + "\n"
}

fn normalized_handshake_tag(active_tag: Option<&str>) -> Option<String> {
    active_tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn handshake_entry_matches_tag(entry: &CanonEntryRevision, active_tag: Option<&str>) -> bool {
    let Some(tag) = normalized_handshake_tag(active_tag) else {
        return false;
    };
    entry
        .topic
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|topic| topic.eq_ignore_ascii_case(&tag))
        .unwrap_or(false)
}

fn ranked_handshake_entries<'a>(
    entries: &'a [CanonEntryRevision],
    active_tag: Option<&str>,
) -> Vec<&'a CanonEntryRevision> {
    let mut ranked = entries.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        handshake_entry_matches_tag(right, active_tag)
            .cmp(&handshake_entry_matches_tag(left, active_tag))
            .then_with(|| right.freshness_score.cmp(&left.freshness_score))
            .then_with(|| right.confidence_bps.cmp(&left.confidence_bps))
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| left.entry_id.cmp(&right.entry_id))
    });
    ranked
}

fn handshake_entry_brief(entry: &CanonEntryRevision, max_chars: usize) -> String {
    let topic = entry.topic.as_deref().unwrap_or("global");
    let summary = truncate_chars(normalize_text(&entry.summary), max_chars);
    format!(
        "[{} r{}] topic={} :: {}",
        entry.entry_id, entry.revision, topic, summary
    )
}

fn handshake_entry_looks_unresolved(entry: &CanonEntryRevision) -> bool {
    let mut haystack = entry
        .topic
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !haystack.is_empty() {
        haystack.push(' ');
    }
    haystack.push_str(&normalize_text(&entry.summary).to_ascii_lowercase());
    [
        "todo",
        "remaining",
        "follow-up",
        "follow up",
        "next step",
        "next:",
        "pending",
        "blocked",
        "risk",
        "gap",
        "unresolved",
        "needs",
        "missing",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

pub fn render_handshake_markdown(
    entries: &[CanonEntryRevision],
    project_snapshot: Option<&HandshakeProjectSnapshot>,
    active_tag: Option<&str>,
    generated_at: chrono::DateTime<chrono::Utc>,
) -> String {
    let active_tag = normalized_handshake_tag(active_tag);
    let ranked = ranked_handshake_entries(entries, active_tag.as_deref());
    let focus_entry = ranked.first().copied();
    let task_focus = project_snapshot.and_then(|snapshot| snapshot.priority_tasks.first());
    let task_focus_matches_active_tag = active_tag
        .as_deref()
        .zip(task_focus)
        .map(|(tag, task)| task.tag.eq_ignore_ascii_case(tag))
        .unwrap_or(false);
    let canon_status = if focus_entry.is_some() {
        "available"
    } else {
        "missing"
    };
    let task_state_status = if project_snapshot.is_some() {
        "available"
    } else {
        "unavailable"
    };
    let focus_source = if focus_entry.is_some() && active_tag.is_some() {
        "project-active-tag"
    } else if focus_entry.is_some() {
        "inferred-project-canon"
    } else if task_focus_matches_active_tag {
        "project-active-tag-task-state"
    } else if task_focus.is_some() {
        "inferred-project-task-state"
    } else {
        "none"
    };
    let focus_brief = if let Some(entry) = focus_entry {
        if let Some(tag) = active_tag.as_deref() {
            format!("tag {} :: {}", tag, handshake_entry_brief(entry, 120))
        } else {
            handshake_entry_brief(entry, 120)
        }
    } else if let Some(task) = task_focus {
        let prd = task.prd_source.unwrap_or("no-prd");
        format!(
            "task [{}] ({}/{}) {} — {} [{}]",
            task.id, task.status, task.priority, task.tag, task.title, prd
        )
    } else if let Some(tag) = active_tag.as_deref() {
        if project_snapshot.is_some() {
            format!(
                "tag {} has no active canon entries and no open task-backed focus",
                tag
            )
        } else {
            format!(
                "tag {} has no active canon entries; task state unavailable",
                tag
            )
        }
    } else if project_snapshot.is_some() {
        "no active canon entries and no open task-backed focus".to_string()
    } else {
        "no active canon entries; task state unavailable".to_string()
    };

    let mut lines = vec![
        "# Mind Handshake Baseline".to_string(),
        String::new(),
        "version: 1".to_string(),
        format!("generated_at: {}", generated_at.to_rfc3339()),
        format!("active_tag: {}", active_tag.as_deref().unwrap_or("none")),
        String::new(),
        "## Focus briefing".to_string(),
        String::new(),
        format!("- source: {}", focus_source),
        format!("- current_focus: {}", focus_brief),
        format!(
            "- fallback_status: canon={} task_state={}",
            canon_status, task_state_status
        ),
        "- scope_note: project baseline; tab-local focus may differ".to_string(),
        String::new(),
        "## High-value work".to_string(),
        String::new(),
    ];

    if let Some(snapshot) = project_snapshot {
        if snapshot.priority_tasks.is_empty() {
            lines.push("- (no open PRD-backed or active tasks detected)".to_string());
        } else {
            for task in snapshot.priority_tasks.iter().take(4) {
                let prd = task.prd_source.unwrap_or("no-prd");
                let active = if task.active_agent {
                    " active-agent"
                } else {
                    ""
                };
                lines.push(format!(
                    "- [{}] ({}/{}) {} — {} [{}{}]",
                    task.id, task.status, task.priority, task.tag, task.title, prd, active
                ));
            }
        }
    } else {
        lines.push("- (task state unavailable)".to_string());
    }
    lines.push(String::new());
    lines.push("## Workstream health".to_string());
    lines.push(String::new());
    if let Some(snapshot) = project_snapshot {
        if snapshot.workstreams.is_empty() {
            lines.push("- (no active workstreams detected)".to_string());
        } else {
            for stream in snapshot.workstreams.iter().take(4) {
                lines.push(format!(
                    "- {} :: in-progress={} blocked={} pending={} prd_open={}",
                    stream.tag,
                    stream.counts.in_progress,
                    stream.counts.blocked,
                    stream.counts.pending,
                    stream.prd_backed_open,
                ));
            }
        }
    } else {
        lines.push("- (task state unavailable)".to_string());
    }
    lines.push(String::new());
    lines.push("## Recent developments".to_string());
    lines.push(String::new());

    if ranked.is_empty() {
        lines.push("- (no active canon entries yet)".to_string());
        lines.push(String::new());
        lines.push("## Open fronts".to_string());
        lines.push(String::new());
        lines.push("- (no unresolved fronts detected yet)".to_string());
        lines.push(String::new());
        lines.push("## Priority canon".to_string());
        lines.push(String::new());
        lines.push("- (no active canon entries yet)".to_string());
        lines.push(String::new());
        return lines.join("\n") + "\n";
    }

    for entry in ranked.iter().take(3) {
        lines.push(format!("- {}", handshake_entry_brief(entry, 140)));
    }
    lines.push(String::new());
    lines.push("## Open fronts".to_string());
    lines.push(String::new());

    let unresolved = ranked
        .iter()
        .copied()
        .filter(|entry| handshake_entry_looks_unresolved(entry))
        .take(3)
        .collect::<Vec<_>>();
    if unresolved.is_empty() {
        lines.push("- (no unresolved fronts detected yet)".to_string());
    } else {
        for entry in unresolved {
            lines.push(format!("- {}", handshake_entry_brief(entry, 140)));
        }
    }
    lines.push(String::new());
    lines.push("## Priority canon".to_string());
    lines.push(String::new());

    for entry in ranked {
        let topic = entry.topic.as_deref().unwrap_or("global");
        let summary = truncate_chars(normalize_text(&entry.summary), 180);
        lines.push(format!(
            "- [{} r{}] topic={} confidence={} freshness={} :: {}",
            entry.entry_id,
            entry.revision,
            topic,
            entry.confidence_bps,
            entry.freshness_score,
            summary
        ));
    }
    lines.push(String::new());

    lines.join("\n") + "\n"
}

fn render_artifact_markdown(kind: &str, artifacts: &[StoredArtifact]) -> String {
    let mut lines = vec![format!("# {} export", kind.to_uppercase())];

    if artifacts.is_empty() {
        lines.push("(empty)".to_string());
        return lines.join("\n") + "\n";
    }

    for artifact in artifacts {
        lines.push(format!(
            "## {} [{}] ({})",
            artifact.artifact_id,
            artifact.conversation_id,
            artifact.ts.to_rfc3339()
        ));
        lines.push(artifact.text.trim().to_string());
        lines.push(String::new());
    }

    lines.join("\n")
}

fn estimate_text_tokens(text: &str) -> u32 {
    (text.chars().count() as u32 / 4).max(1)
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: String, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    if text.chars().count() <= max_chars {
        return text;
    }

    let mut out = text
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn active_tag_for_ts(
    context_states: &[ConversationContextState],
    ts: chrono::DateTime<chrono::Utc>,
) -> Option<String> {
    let mut active_tag = None;
    for context in context_states {
        if context.ts > ts {
            break;
        }
        if let Some(tag) = context
            .active_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            active_tag = Some(tag.to_string());
        }
    }
    active_tag
}

#[cfg(test)]
mod tests;
