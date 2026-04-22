use anyhow::{anyhow, bail, Context, Result};
use chrono::{TimeZone, Utc};
use clap::{Args, Subcommand, ValueEnum};
use std::{
    env,
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aoc_core::{
    consultation_contracts::{
        ConsultationArtifactRef, ConsultationBlocker, ConsultationCheckpointRef,
        ConsultationConfidence, ConsultationEvidenceRef, ConsultationFreshness,
        ConsultationHelpRequest, ConsultationIdentity, ConsultationPacket, ConsultationPacketKind,
        ConsultationPlanItem, ConsultationSourceStatus, ConsultationTaskContext,
    },
    pulse_ipc::{
        decode_frame, encode_frame, CommandPayload, HelloPayload, ObserverTimelinePayload,
        ProtocolVersion, SubscribePayload, WireEnvelope, WireMsg, CURRENT_PROTOCOL_VERSION,
        DEFAULT_MAX_FRAME_BYTES,
    },
    session_overseer::{
        DriftRisk, ObserverSnapshot, WorkerSnapshot, WorkerStatus, OVERSEER_SNAPSHOT_TOPIC,
        OVERSEER_TIMELINE_TOPIC,
    },
};
use aoc_storage::{CompactionCheckpoint, MindStore};

#[derive(Subcommand, Debug)]
pub enum OverseerCommand {
    /// Read the current overseer snapshot for a session
    Snapshot(OverseerQueryArgs),
    /// Read recent overseer timeline entries for a session
    Timeline(TimelineArgs),
    /// Read a bounded consultation packet for a worker/session
    Consult(ConsultArgs),
    /// Send a manager-style overseer command to a worker
    Command(CommandArgs),
}

#[derive(Args, Debug)]
pub struct OverseerQueryArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct TimelineArgs {
    #[command(flatten)]
    pub query: OverseerQueryArgs,
    /// Maximum number of entries to print.
    #[arg(long, default_value_t = 12)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct ConsultArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Target worker agent id. Optional when exactly one worker is present.
    #[arg(long)]
    pub target_agent_id: Option<String>,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(value_enum, default_value_t = ConsultationPacketKindArg::Summary)]
    pub kind: ConsultationPacketKindArg,
}

#[derive(Args, Debug)]
pub struct CommandArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Target worker agent id.
    #[arg(long)]
    pub target_agent_id: String,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(value_enum)]
    pub kind: OverseerCommandKindArg,
    /// Task id to switch focus to (switch-focus only).
    #[arg(long)]
    pub task_id: Option<String>,
    /// Operator/manager summary for switch-focus.
    #[arg(long)]
    pub summary: Option<String>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ConsultationPacketKindArg {
    Summary,
    Plan,
    Blockers,
    Review,
    Align,
    CheckpointStatus,
    HelpRequest,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OverseerCommandKindArg {
    RequestStatusUpdate,
    RequestHandoff,
    PauseAndSummarize,
    RunValidation,
    SwitchFocus,
    FinalizeAndReport,
}

pub fn handle_overseer_command(command: OverseerCommand) -> Result<()> {
    match command {
        OverseerCommand::Snapshot(args) => handle_snapshot(args),
        OverseerCommand::Timeline(args) => handle_timeline(args),
        OverseerCommand::Consult(args) => handle_consult(args),
        OverseerCommand::Command(args) => handle_command(args),
    }
}

fn handle_snapshot(args: OverseerQueryArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path);
    let snapshot = request_snapshot(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
    )?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }

    println!(
        "session={} workers={} timeline={} generated_at_ms={}",
        snapshot.session_id,
        snapshot.workers.len(),
        snapshot.timeline.len(),
        snapshot.generated_at_ms.unwrap_or_default()
    );
    for worker in snapshot.workers {
        let task = worker
            .assignment
            .task_id
            .or(worker.assignment.tag)
            .unwrap_or_else(|| "unassigned".to_string());
        let attention = format!("{:?}", worker.attention.level).to_ascii_lowercase();
        let drift = format!("{:?}", worker.drift_risk).to_ascii_lowercase();
        let status = format!("{:?}", worker.status).to_ascii_lowercase();
        let summary = worker.summary.unwrap_or_default();
        println!(
            "- {} pane={} status={} task={} attention={} drift={} {}",
            worker.agent_id, worker.pane_id, status, task, attention, drift, summary
        );
    }
    Ok(())
}

fn handle_timeline(args: TimelineArgs) -> Result<()> {
    let session_id = resolve_session_id(args.query.session_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.query.socket_path);
    let payload = request_timeline(
        &session_id,
        &socket_path,
        Duration::from_millis(args.query.timeout_ms),
    )?;
    if args.query.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!(
        "session={} timeline={} generated_at_ms={}",
        payload.session_id,
        payload.entries.len(),
        payload.generated_at_ms.unwrap_or_default()
    );
    for entry in payload.entries.into_iter().take(args.limit) {
        let kind = format!("{:?}", entry.kind).to_ascii_lowercase();
        let summary = entry.summary.unwrap_or_default();
        println!(
            "- {} {} {} {}",
            entry.emitted_at_ms.unwrap_or_default(),
            entry.agent_id,
            kind,
            summary
        );
    }
    Ok(())
}

fn handle_consult(args: ConsultArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path);
    let snapshot = request_snapshot(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
    )?;
    let worker = select_worker(&snapshot, args.target_agent_id.as_deref())?;
    let checkpoint = load_latest_compaction_checkpoint(&session_id);
    let packet = derive_consultation_packet(
        &snapshot,
        worker,
        args.kind.into(),
        checkpoint.as_ref(),
        now_ms() as i64,
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&packet)?);
        return Ok(());
    }

    print_consultation_packet(&packet);
    Ok(())
}

fn handle_command(args: CommandArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id.clone())?;
    let command_args = command_args_json(&args);
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path.clone());
    let request_id = format!("overseer-cli-{}", now_ms());
    let command = command_name(&args.kind);
    let result = request_command_result(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
        &request_id,
        &args.target_agent_id,
        command,
        command_args,
    )?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    println!(
        "command={} target={} status={} code={} message={}",
        result.command,
        result.target_agent_id.unwrap_or_default(),
        result.status,
        result.error_code.unwrap_or_default(),
        result.message.unwrap_or_default()
    );
    Ok(())
}

#[derive(serde::Serialize)]
pub(crate) struct CommandResultView {
    pub(crate) command: String,
    pub(crate) target_agent_id: Option<String>,
    pub(crate) status: String,
    pub(crate) error_code: Option<String>,
    pub(crate) message: Option<String>,
}

fn is_terminal_command_status(status: &str) -> bool {
    !status.eq_ignore_ascii_case("accepted")
}

fn is_session_scoped_snapshot(
    envelope_session_id: &str,
    payload_session_id: &str,
    session_id: &str,
) -> bool {
    envelope_session_id == session_id && payload_session_id == session_id
}

impl From<ConsultationPacketKindArg> for ConsultationPacketKind {
    fn from(value: ConsultationPacketKindArg) -> Self {
        match value {
            ConsultationPacketKindArg::Summary => Self::Summary,
            ConsultationPacketKindArg::Plan => Self::Plan,
            ConsultationPacketKindArg::Blockers => Self::Blockers,
            ConsultationPacketKindArg::Review => Self::Review,
            ConsultationPacketKindArg::Align => Self::Align,
            ConsultationPacketKindArg::CheckpointStatus => Self::CheckpointStatus,
            ConsultationPacketKindArg::HelpRequest => Self::HelpRequest,
        }
    }
}

fn select_worker<'a>(
    snapshot: &'a ObserverSnapshot,
    target: Option<&str>,
) -> Result<&'a WorkerSnapshot> {
    if let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) {
        return snapshot
            .workers
            .iter()
            .find(|worker| worker.agent_id == target)
            .with_context(|| format!("worker not found in snapshot: {target}"));
    }

    match snapshot.workers.as_slice() {
        [worker] => Ok(worker),
        [] => bail!("snapshot has no workers to consult"),
        _ => bail!("multiple workers present; pass --target-agent-id"),
    }
}

fn load_latest_compaction_checkpoint(session_id: &str) -> Option<CompactionCheckpoint> {
    let store_path = mind_store_path()?;
    if !store_path.exists() {
        return None;
    }
    let store = MindStore::open(store_path).ok()?;
    store
        .latest_compaction_checkpoint_for_session(session_id)
        .ok()
        .flatten()
}

fn resolve_aoc_state_home() -> PathBuf {
    if let Ok(value) = env::var("XDG_STATE_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".local/state")
}

fn mind_store_path() -> Option<PathBuf> {
    env::var("AOC_MIND_STORE_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let root = project_root()?;
            Some(
                resolve_aoc_state_home()
                    .join("aoc")
                    .join("mind")
                    .join("projects")
                    .join(root.to_string_lossy().replace(
                        |ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'),
                        "_",
                    ))
                    .join("project.sqlite"),
            )
        })
}

fn project_root() -> Option<PathBuf> {
    env::var("AOC_PROJECT_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
}

fn derive_consultation_packet(
    snapshot: &ObserverSnapshot,
    worker: &WorkerSnapshot,
    kind: ConsultationPacketKind,
    checkpoint: Option<&CompactionCheckpoint>,
    now_ms: i64,
) -> ConsultationPacket {
    let mut degraded_inputs = Vec::new();
    if checkpoint.is_none() {
        degraded_inputs.push("mind.compaction_checkpoint".to_string());
    }
    if worker
        .summary
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        degraded_inputs.push("overseer.summary".to_string());
    }
    if worker.assignment.task_id.is_none() && worker.assignment.tag.is_none() {
        degraded_inputs.push("task.assignment".to_string());
    }

    let source_status = if is_worker_stale(worker, now_ms) {
        ConsultationSourceStatus::Stale
    } else if degraded_inputs.is_empty() {
        ConsultationSourceStatus::Complete
    } else {
        ConsultationSourceStatus::Partial
    };

    let mut summary = consultation_summary(worker, checkpoint, kind);
    if summary
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        summary = Some(synthesize_worker_summary(worker));
    }

    let blockers = consultation_blockers(worker, checkpoint, kind);
    let help_request = consultation_help_request(worker, kind);
    let evidence_refs = consultation_evidence_refs(worker, checkpoint);
    let artifact_refs = consultation_artifact_refs(snapshot, worker, checkpoint);

    ConsultationPacket {
        packet_id: format!(
            "consult:{}:{}:{}:{}",
            consultation_kind_name(kind),
            snapshot.session_id,
            worker.agent_id,
            now_ms
        ),
        kind,
        identity: ConsultationIdentity {
            session_id: worker.session_id.clone(),
            agent_id: worker.agent_id.clone(),
            pane_id: Some(worker.pane_id.clone()),
            conversation_id: checkpoint.map(|value| value.conversation_id.clone()),
            role: worker.role.clone(),
        },
        task_context: ConsultationTaskContext {
            active_tag: worker.assignment.tag.clone(),
            task_ids: worker
                .assignment
                .task_id
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            focus_summary: Some(synthesize_focus_summary(worker)),
        },
        current_plan: consultation_plan(worker),
        summary,
        blockers,
        checkpoint: checkpoint.map(|value| ConsultationCheckpointRef {
            checkpoint_id: value.checkpoint_id.clone(),
            conversation_id: Some(value.conversation_id.clone()),
            compaction_entry_id: value.compaction_entry_id.clone(),
            ts: Some(value.ts.to_rfc3339()),
        }),
        artifact_refs,
        evidence_refs,
        freshness: ConsultationFreshness {
            packet_generated_at: Some(ms_to_rfc3339(now_ms)),
            source_updated_at: consultation_source_updated_at(worker, checkpoint),
            stale_after_ms: worker.stale_after_ms,
            source_status,
            degraded_inputs: degraded_inputs.clone(),
        },
        confidence: ConsultationConfidence {
            overall_bps: consultation_confidence_bps(worker, checkpoint),
            rationale: Some(confidence_rationale(worker, checkpoint)),
        },
        help_request,
        degraded_reason: if degraded_inputs.is_empty() {
            None
        } else {
            Some(format!(
                "packet derived with partial inputs: {}",
                degraded_inputs.join(", ")
            ))
        },
        ..Default::default()
    }
    .normalize()
}

fn consultation_plan(worker: &WorkerSnapshot) -> Vec<ConsultationPlanItem> {
    let mut items = Vec::new();
    let mut title = format!("Continue {:?} work", worker.progress.phase).to_ascii_lowercase();
    title = title.replace('_', " ");
    items.push(ConsultationPlanItem {
        title,
        status: Some(format!("{:?}", worker.status).to_ascii_lowercase()),
        task_id: worker.assignment.task_id.clone(),
        summary: worker.summary.clone(),
        evidence_refs: worker.files_touched.iter().take(4).cloned().collect(),
    });
    if let Some(blocker) = worker
        .blocker
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        items.push(ConsultationPlanItem {
            title: "Resolve blocker".to_string(),
            status: Some("pending".to_string()),
            task_id: worker.assignment.task_id.clone(),
            summary: Some(blocker.to_string()),
            evidence_refs: worker.files_touched.iter().take(2).cloned().collect(),
        });
    }
    items
}

fn consultation_blockers(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    kind: ConsultationPacketKind,
) -> Vec<ConsultationBlocker> {
    let mut blockers = Vec::new();
    if let Some(blocker) = worker
        .blocker
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        blockers.push(ConsultationBlocker {
            summary: blocker.to_string(),
            severity: Some(
                match worker.status {
                    WorkerStatus::Blocked => "high",
                    WorkerStatus::NeedsInput => "medium",
                    _ => "low",
                }
                .to_string(),
            ),
            kind: Some("runtime".to_string()),
            evidence_refs: worker.files_touched.iter().take(4).cloned().collect(),
        });
    }
    if matches!(kind, ConsultationPacketKind::CheckpointStatus) && checkpoint.is_none() {
        blockers.push(ConsultationBlocker {
            summary: "latest compaction checkpoint unavailable".to_string(),
            severity: Some("medium".to_string()),
            kind: Some("checkpoint".to_string()),
            evidence_refs: Vec::new(),
        });
    }
    blockers
}

fn consultation_help_request(
    worker: &WorkerSnapshot,
    kind: ConsultationPacketKind,
) -> Option<ConsultationHelpRequest> {
    if matches!(kind, ConsultationPacketKind::HelpRequest)
        || matches!(
            worker.status,
            WorkerStatus::Blocked | WorkerStatus::NeedsInput
        )
    {
        let question = worker
            .blocker
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "review current progress and recommend next step".to_string());
        return Some(ConsultationHelpRequest {
            kind: if matches!(worker.status, WorkerStatus::Blocked) {
                "blocker_escalation".to_string()
            } else {
                "review_request".to_string()
            },
            question,
            requested_from: Some("mission_control".to_string()),
            urgency: Some(
                if matches!(worker.status, WorkerStatus::Blocked) {
                    "high"
                } else {
                    "medium"
                }
                .to_string(),
            ),
        });
    }
    None
}

fn consultation_evidence_refs(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
) -> Vec<ConsultationEvidenceRef> {
    let mut refs = worker
        .files_touched
        .iter()
        .take(8)
        .map(|path| ConsultationEvidenceRef {
            reference: format!("file:{path}"),
            label: Some(path.clone()),
            path: Some(path.clone()),
            relation: Some("files_touched".to_string()),
        })
        .collect::<Vec<_>>();
    if let Some(checkpoint) = checkpoint {
        refs.push(ConsultationEvidenceRef {
            reference: format!("checkpoint:{}", checkpoint.checkpoint_id),
            label: checkpoint.summary.clone(),
            path: None,
            relation: Some("latest_checkpoint".to_string()),
        });
    }
    refs
}

fn consultation_artifact_refs(
    snapshot: &ObserverSnapshot,
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
) -> Vec<ConsultationArtifactRef> {
    let mut refs = Vec::new();
    if let Some(checkpoint) = checkpoint {
        refs.push(ConsultationArtifactRef {
            artifact_id: checkpoint.checkpoint_id.clone(),
            layer: Some("t0".to_string()),
            kind: Some("pi_compaction_checkpoint".to_string()),
            created_at: Some(checkpoint.ts.to_rfc3339()),
        });
    }
    if snapshot
        .timeline
        .iter()
        .any(|entry| entry.agent_id == worker.agent_id)
    {
        refs.push(ConsultationArtifactRef {
            artifact_id: format!(
                "observer_timeline:{}:{}",
                snapshot.session_id, worker.agent_id
            ),
            layer: Some("t1".to_string()),
            kind: Some("observer_timeline".to_string()),
            created_at: snapshot.generated_at_ms.map(ms_to_rfc3339),
        });
    }
    refs
}

fn consultation_summary(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    kind: ConsultationPacketKind,
) -> Option<String> {
    match kind {
        ConsultationPacketKind::Summary => worker.summary.clone(),
        ConsultationPacketKind::Plan => Some(format!(
            "phase={} status={} focus={}",
            format!("{:?}", worker.progress.phase).to_ascii_lowercase(),
            format!("{:?}", worker.status).to_ascii_lowercase(),
            worker
                .assignment
                .task_id
                .clone()
                .or_else(|| worker.assignment.tag.clone())
                .unwrap_or_else(|| "unassigned".to_string())
        )),
        ConsultationPacketKind::Blockers => {
            worker.blocker.clone().or_else(|| worker.summary.clone())
        }
        ConsultationPacketKind::Review => Some(format!(
            "review request for {} with drift={} attention={}",
            worker.agent_id,
            format!("{:?}", worker.drift_risk).to_ascii_lowercase(),
            format!("{:?}", worker.attention.level).to_ascii_lowercase()
        )),
        ConsultationPacketKind::Align => Some(format!(
            "plan_alignment={} drift_risk={} task={}",
            format!("{:?}", worker.plan_alignment).to_ascii_lowercase(),
            format!("{:?}", worker.drift_risk).to_ascii_lowercase(),
            worker
                .assignment
                .task_id
                .clone()
                .unwrap_or_else(|| "none".to_string())
        )),
        ConsultationPacketKind::CheckpointStatus => checkpoint.and_then(|value| {
            Some(format!(
                "latest checkpoint {} trigger={} tokens_before={} summary={}",
                value.checkpoint_id,
                value.trigger_source,
                value
                    .tokens_before
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                value
                    .summary
                    .clone()
                    .unwrap_or_else(|| "(none)".to_string())
            ))
        }),
        ConsultationPacketKind::HelpRequest => worker
            .blocker
            .clone()
            .or_else(|| Some("requesting bounded review/alignment packet".to_string())),
    }
}

fn consultation_source_updated_at(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
) -> Option<String> {
    let worker_time = worker.last_update_at_ms.map(ms_to_rfc3339);
    match (worker_time, checkpoint) {
        (Some(worker_time), Some(checkpoint)) => {
            if checkpoint.ts.timestamp_millis() > worker.last_update_at_ms.unwrap_or_default() {
                Some(checkpoint.ts.to_rfc3339())
            } else {
                Some(worker_time)
            }
        }
        (Some(worker_time), None) => Some(worker_time),
        (None, Some(checkpoint)) => Some(checkpoint.ts.to_rfc3339()),
        (None, None) => None,
    }
}

fn consultation_confidence_bps(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
) -> Option<u16> {
    let mut score = 550u16;
    if worker
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_some()
    {
        score += 150;
    }
    if worker.assignment.task_id.is_some() {
        score += 100;
    }
    if checkpoint.is_some() {
        score += 100;
    }
    if matches!(worker.drift_risk, DriftRisk::High) {
        score = score.saturating_sub(150);
    }
    if matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        score = score.saturating_sub(100);
    }
    Some(score.min(1000))
}

fn confidence_rationale(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
) -> String {
    let mut parts = Vec::new();
    if worker
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_some()
    {
        parts.push("worker summary present");
    } else {
        parts.push("worker summary missing");
    }
    if worker.assignment.task_id.is_some() || worker.assignment.tag.is_some() {
        parts.push("task context present");
    } else {
        parts.push("task context missing");
    }
    if checkpoint.is_some() {
        parts.push("checkpoint linked");
    } else {
        parts.push("checkpoint unavailable");
    }
    if matches!(worker.drift_risk, DriftRisk::High) {
        parts.push("high drift risk");
    }
    parts.join(", ")
}

fn synthesize_focus_summary(worker: &WorkerSnapshot) -> String {
    worker
        .assignment
        .task_id
        .clone()
        .or_else(|| worker.assignment.tag.clone())
        .or_else(|| worker.summary.clone())
        .unwrap_or_else(|| "unassigned workstream".to_string())
}

fn synthesize_worker_summary(worker: &WorkerSnapshot) -> String {
    format!(
        "status={} phase={} attention={} drift={}",
        format!("{:?}", worker.status).to_ascii_lowercase(),
        format!("{:?}", worker.progress.phase).to_ascii_lowercase(),
        format!("{:?}", worker.attention.level).to_ascii_lowercase(),
        format!("{:?}", worker.drift_risk).to_ascii_lowercase()
    )
}

fn consultation_kind_name(kind: ConsultationPacketKind) -> &'static str {
    match kind {
        ConsultationPacketKind::Summary => "summary",
        ConsultationPacketKind::Plan => "plan",
        ConsultationPacketKind::Blockers => "blockers",
        ConsultationPacketKind::Review => "review",
        ConsultationPacketKind::Align => "align",
        ConsultationPacketKind::CheckpointStatus => "checkpoint_status",
        ConsultationPacketKind::HelpRequest => "help_request",
    }
}

fn is_worker_stale(worker: &WorkerSnapshot, now_ms: i64) -> bool {
    match (worker.last_update_at_ms, worker.stale_after_ms) {
        (Some(last_update_at_ms), Some(stale_after_ms)) => {
            now_ms.saturating_sub(last_update_at_ms) > stale_after_ms as i64
        }
        _ => false,
    }
}

fn ms_to_rfc3339(value: i64) -> String {
    Utc.timestamp_millis_opt(value)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

fn print_consultation_packet(packet: &ConsultationPacket) {
    println!(
        "packet={} kind={} worker={} session={} source_status={} degraded={}",
        packet.packet_id,
        consultation_kind_name(packet.kind),
        packet.identity.agent_id,
        packet.identity.session_id,
        format!("{:?}", packet.freshness.source_status).to_ascii_lowercase(),
        packet.is_degraded()
    );
    if let Some(summary) = packet.summary.as_deref() {
        println!("summary: {summary}");
    }
    if !packet.task_context.task_ids.is_empty() || packet.task_context.active_tag.is_some() {
        println!(
            "task: ids={} tag={}",
            packet.task_context.task_ids.join(","),
            packet
                .task_context
                .active_tag
                .as_deref()
                .unwrap_or("(none)")
        );
    }
    if let Some(checkpoint) = packet.checkpoint.as_ref() {
        println!(
            "checkpoint: {} @{}",
            checkpoint.checkpoint_id,
            checkpoint.ts.as_deref().unwrap_or("unknown")
        );
    }
    for blocker in &packet.blockers {
        println!("blocker: {}", blocker.summary);
    }
    for plan in &packet.current_plan {
        println!("plan: {}", plan.title);
    }
    if let Some(help) = packet.help_request.as_ref() {
        println!("help: {} -> {}", help.kind, help.question);
    }
    if !packet.evidence_refs.is_empty() {
        let refs = packet
            .evidence_refs
            .iter()
            .map(|item| item.reference.as_str())
            .take(4)
            .collect::<Vec<_>>()
            .join(", ");
        println!("evidence: {refs}");
    }
    if packet.is_degraded() {
        println!(
            "degraded: {}",
            packet
                .degraded_reason
                .as_deref()
                .unwrap_or("partial sources")
        );
    }
}

fn request_snapshot(
    session_id: &str,
    socket_path: &PathBuf,
    timeout: Duration,
) -> Result<ObserverSnapshot> {
    let mut stream =
        connect_subscriber(session_id, socket_path, &[OVERSEER_SNAPSHOT_TOPIC], timeout)?;
    loop {
        let envelope = read_wire_envelope(&mut stream, timeout)?;
        match envelope.msg {
            WireMsg::ObserverSnapshot(payload)
                if is_session_scoped_snapshot(
                    &envelope.session_id,
                    &payload.session_id,
                    session_id,
                ) =>
            {
                return Ok(payload);
            }
            WireMsg::ObserverSnapshot(_) => continue,
            WireMsg::Snapshot(_) | WireMsg::Delta(_) | WireMsg::Heartbeat(_) => continue,
            other => {
                bail!("unexpected pulse message while waiting for observer snapshot: {other:?}")
            }
        }
    }
}

fn request_timeline(
    session_id: &str,
    socket_path: &PathBuf,
    timeout: Duration,
) -> Result<ObserverTimelinePayload> {
    let mut stream =
        connect_subscriber(session_id, socket_path, &[OVERSEER_TIMELINE_TOPIC], timeout)?;
    loop {
        let envelope = read_wire_envelope(&mut stream, timeout)?;
        match envelope.msg {
            WireMsg::ObserverTimeline(payload)
                if is_session_scoped_snapshot(
                    &envelope.session_id,
                    &payload.session_id,
                    session_id,
                ) =>
            {
                return Ok(payload);
            }
            WireMsg::ObserverTimeline(_) => continue,
            WireMsg::Snapshot(_) | WireMsg::Delta(_) | WireMsg::Heartbeat(_) => continue,
            other => {
                bail!("unexpected pulse message while waiting for observer timeline: {other:?}")
            }
        }
    }
}

pub(crate) fn request_command_result(
    session_id: &str,
    socket_path: &PathBuf,
    timeout: Duration,
    request_id: &str,
    target_agent_id: &str,
    command: &str,
    args: serde_json::Value,
) -> Result<CommandResultView> {
    let mut stream = connect_subscriber(session_id, socket_path, &["command_result"], timeout)?;
    let envelope = WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: session_id.to_string(),
        sender_id: client_id(),
        timestamp: now_rfc3339(),
        request_id: Some(request_id.to_string()),
        msg: WireMsg::Command(CommandPayload {
            command: command.to_string(),
            target_agent_id: Some(target_agent_id.to_string()),
            args,
        }),
    };
    send_wire_envelope(&mut stream, &envelope)?;

    loop {
        let envelope = read_wire_envelope(&mut stream, timeout)?;
        if envelope.session_id != session_id {
            continue;
        }
        if envelope.request_id.as_deref() != Some(request_id) {
            continue;
        }
        match envelope.msg {
            WireMsg::CommandResult(payload) => {
                if !is_terminal_command_status(&payload.status) {
                    continue;
                }
                return Ok(CommandResultView {
                    command: payload.command,
                    target_agent_id: Some(target_agent_id.to_string()),
                    status: payload.status,
                    error_code: payload.error.as_ref().map(|err| err.code.clone()),
                    message: payload
                        .message
                        .or_else(|| payload.error.as_ref().map(|err| err.message.clone())),
                });
            }
            other => bail!("unexpected pulse message while waiting for command result: {other:?}"),
        }
    }
}

#[cfg(unix)]
fn connect_subscriber(
    session_id: &str,
    socket_path: &PathBuf,
    topics: &[&str],
    timeout: Duration,
) -> Result<std::os::unix::net::UnixStream> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket_path).with_context(|| {
        format!(
            "failed to connect to pulse socket {}",
            socket_path.display()
        )
    })?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    let hello = WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: session_id.to_string(),
        sender_id: client_id(),
        timestamp: now_rfc3339(),
        request_id: None,
        msg: WireMsg::Hello(HelloPayload {
            client_id: client_id(),
            role: "subscriber".to_string(),
            capabilities: vec![
                "snapshot".to_string(),
                "delta".to_string(),
                "command_result".to_string(),
            ],
            agent_id: None,
            pane_id: None,
            project_root: None,
        }),
    };
    send_wire_envelope(&mut stream, &hello)?;

    let subscribe = WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: session_id.to_string(),
        sender_id: client_id(),
        timestamp: now_rfc3339(),
        request_id: None,
        msg: WireMsg::Subscribe(SubscribePayload {
            topics: topics.iter().map(|topic| (*topic).to_string()).collect(),
            since_seq: None,
        }),
    };
    send_wire_envelope(&mut stream, &subscribe)?;
    Ok(stream)
}

#[cfg(not(unix))]
fn connect_subscriber(
    _session_id: &str,
    _socket_path: &PathBuf,
    _topics: &[&str],
    _timeout: Duration,
) -> Result<()> {
    bail!("overseer CLI currently requires unix domain sockets")
}

#[cfg(unix)]
fn send_wire_envelope(
    stream: &mut std::os::unix::net::UnixStream,
    envelope: &WireEnvelope,
) -> Result<()> {
    let frame = encode_frame(envelope, DEFAULT_MAX_FRAME_BYTES)?;
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

#[cfg(unix)]
fn read_wire_envelope(
    stream: &mut std::os::unix::net::UnixStream,
    timeout: Duration,
) -> Result<WireEnvelope> {
    stream.set_read_timeout(Some(timeout))?;
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0u8; 8192];
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            bail!("pulse socket closed before response arrived");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            let frame = buffer.drain(..=newline).collect::<Vec<u8>>();
            return decode_frame::<WireEnvelope>(&frame, DEFAULT_MAX_FRAME_BYTES)
                .map_err(|err| anyhow!(err.to_string()));
        }
    }
}

fn command_name(kind: &OverseerCommandKindArg) -> &'static str {
    match kind {
        OverseerCommandKindArg::RequestStatusUpdate => "request_status_update",
        OverseerCommandKindArg::RequestHandoff => "request_handoff",
        OverseerCommandKindArg::PauseAndSummarize => "pause_and_summarize",
        OverseerCommandKindArg::RunValidation => "run_validation",
        OverseerCommandKindArg::SwitchFocus => "switch_focus",
        OverseerCommandKindArg::FinalizeAndReport => "finalize_and_report",
    }
}

fn command_args_json(args: &CommandArgs) -> serde_json::Value {
    match args.kind {
        OverseerCommandKindArg::SwitchFocus => serde_json::json!({
            "task_id": args.task_id,
            "summary": args.summary,
        }),
        _ => serde_json::json!({}),
    }
}

pub(crate) fn resolve_session_id(value: Option<String>) -> Result<String> {
    if let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(value);
    }
    if let Ok(value) = std::env::var("AOC_SESSION_ID") {
        let value = value.trim();
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }
    bail!("missing session id; pass --session-id or set AOC_SESSION_ID")
}

pub(crate) fn resolve_pulse_socket_path(
    session_id: &str,
    override_path: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = override_path {
        return path;
    }
    if let Ok(value) = std::env::var("AOC_PULSE_SOCK") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    let runtime_dir = if let Ok(value) = std::env::var("XDG_RUNTIME_DIR") {
        if !value.trim().is_empty() {
            PathBuf::from(value)
        } else {
            PathBuf::from("/tmp")
        }
    } else if let Ok(uid) = std::env::var("UID") {
        PathBuf::from(format!("/run/user/{uid}"))
    } else {
        PathBuf::from("/tmp")
    };
    runtime_dir
        .join("aoc")
        .join(session_slug(session_id))
        .join("pulse.sock")
}

fn session_slug(session_id: &str) -> String {
    let mut slug = String::with_capacity(session_id.len());
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            slug.push(ch);
        } else {
            slug.push('-');
        }
    }
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-').to_string();
    let base = if slug.is_empty() {
        "session".to_string()
    } else {
        slug
    };
    let hash = stable_hash_hex(session_id);
    let short = if base.len() > 48 {
        &base[..48]
    } else {
        base.as_str()
    };
    format!("{short}-{hash}")
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in input.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("{hash:08x}")
}

fn client_id() -> String {
    format!("aoc-overseer-cli-{}", std::process::id())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_core::session_overseer::{
        AttentionLevel, AttentionSignal, PlanAlignment, ProgressPhase, ProgressPosition,
        WorkerAssignment,
    };

    fn sample_worker() -> WorkerSnapshot {
        WorkerSnapshot {
            session_id: "sess-1".to_string(),
            agent_id: "sess-1::pane-2".to_string(),
            pane_id: "pane-2".to_string(),
            role: Some("builder".to_string()),
            status: WorkerStatus::Active,
            progress: ProgressPosition {
                phase: ProgressPhase::Implementation,
                percent: Some(60),
            },
            assignment: WorkerAssignment {
                task_id: Some("156".to_string()),
                tag: Some("mind".to_string()),
                epic_id: None,
            },
            summary: Some("implementing consultation packet schema".to_string()),
            blocker: None,
            files_touched: vec![
                "crates/aoc-core/src/consultation_contracts.rs".to_string(),
                "docs/research/consultation-packet-contract.md".to_string(),
            ],
            plan_alignment: PlanAlignment::High,
            drift_risk: DriftRisk::Low,
            attention: AttentionSignal {
                level: AttentionLevel::Info,
                kind: Some("progressing".to_string()),
                reason: Some("recent update".to_string()),
            },
            duplicate_work: None,
            branch: Some("feature/consult".to_string()),
            last_update_at_ms: Some(1_000),
            last_meaningful_progress_at_ms: Some(900),
            stale_after_ms: Some(5_000),
            source: aoc_core::session_overseer::OverseerSourceKind::Wrapper,
            provenance: Some("wrapper".to_string()),
        }
    }

    fn sample_snapshot() -> ObserverSnapshot {
        ObserverSnapshot {
            session_id: "sess-1".to_string(),
            generated_at_ms: Some(1_500),
            workers: vec![sample_worker()],
            ..Default::default()
        }
    }

    fn sample_checkpoint() -> CompactionCheckpoint {
        CompactionCheckpoint {
            checkpoint_id: "cmpchk:sess-1:1".to_string(),
            conversation_id: "pi:sess-1".to_string(),
            session_id: "sess-1".to_string(),
            ts: Utc.timestamp_millis_opt(1_400).single().unwrap(),
            trigger_source: "pi_compact".to_string(),
            reason: Some("threshold compact".to_string()),
            summary: Some("checkpoint summary".to_string()),
            tokens_before: Some(12_000),
            first_kept_entry_id: Some("entry-42".to_string()),
            compaction_entry_id: Some("compact-1".to_string()),
            from_extension: true,
            marker_event_id: Some("evt-1".to_string()),
            schema_version: 1,
            created_at: Utc.timestamp_millis_opt(1_400).single().unwrap(),
            updated_at: Utc.timestamp_millis_opt(1_400).single().unwrap(),
        }
    }

    #[test]
    fn derive_consultation_packet_includes_checkpoint_and_evidence() {
        let snapshot = sample_snapshot();
        let worker = &snapshot.workers[0];
        let checkpoint = sample_checkpoint();
        let packet = derive_consultation_packet(
            &snapshot,
            worker,
            ConsultationPacketKind::Summary,
            Some(&checkpoint),
            2_000,
        );

        assert_eq!(packet.kind, ConsultationPacketKind::Summary);
        assert_eq!(packet.identity.session_id, "sess-1");
        assert_eq!(
            packet.identity.conversation_id.as_deref(),
            Some("pi:sess-1")
        );
        assert_eq!(packet.task_context.task_ids, vec!["156"]);
        assert_eq!(
            packet.freshness.source_status,
            ConsultationSourceStatus::Complete
        );
        assert!(packet.checkpoint.is_some());
        assert!(!packet.evidence_refs.is_empty());
        assert!(packet
            .summary
            .as_deref()
            .unwrap()
            .contains("consultation packet"));
    }

    #[test]
    fn derive_consultation_packet_degrades_when_checkpoint_missing_and_worker_stale() {
        let mut snapshot = sample_snapshot();
        snapshot.workers[0].summary = None;
        snapshot.workers[0].assignment.task_id = None;
        snapshot.workers[0].assignment.tag = None;
        snapshot.workers[0].last_update_at_ms = Some(0);
        snapshot.workers[0].stale_after_ms = Some(100);
        snapshot.workers[0].status = WorkerStatus::Blocked;
        snapshot.workers[0].blocker = Some("need operator guidance".to_string());

        let packet = derive_consultation_packet(
            &snapshot,
            &snapshot.workers[0],
            ConsultationPacketKind::HelpRequest,
            None,
            1_000,
        );

        assert_eq!(
            packet.freshness.source_status,
            ConsultationSourceStatus::Stale
        );
        assert!(packet.is_degraded());
        assert!(packet
            .freshness
            .degraded_inputs
            .contains(&"mind.compaction_checkpoint".to_string()));
        assert!(packet.help_request.is_some());
        assert_eq!(packet.blockers.len(), 1);
        assert!(packet
            .summary
            .as_deref()
            .unwrap()
            .contains("need operator guidance"));
    }

    #[test]
    fn select_worker_requires_target_when_multiple_workers_exist() {
        let mut snapshot = sample_snapshot();
        let mut worker = sample_worker();
        worker.agent_id = "sess-1::pane-3".to_string();
        worker.pane_id = "pane-3".to_string();
        snapshot.workers.push(worker);

        let err = select_worker(&snapshot, None).expect_err("should require explicit target");
        assert!(err.to_string().contains("multiple workers present"));
    }

    #[test]
    fn command_result_terminal_status_helper_treats_accepted_as_non_terminal() {
        assert!(!is_terminal_command_status("accepted"));
        assert!(!is_terminal_command_status("ACCEPTED"));
        assert!(is_terminal_command_status("ok"));
        assert!(is_terminal_command_status("error"));
    }

    #[test]
    fn session_scoped_snapshot_helper_requires_matching_envelope_and_payload_sessions() {
        assert!(is_session_scoped_snapshot("sess-1", "sess-1", "sess-1"));
        assert!(!is_session_scoped_snapshot("sess-2", "sess-1", "sess-1"));
        assert!(!is_session_scoped_snapshot("sess-1", "sess-2", "sess-1"));
    }
}
