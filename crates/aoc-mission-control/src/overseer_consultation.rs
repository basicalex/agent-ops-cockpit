//! Overseer consultation packet policy and rendering helpers.

use super::*;

pub(crate) fn derive_overseer_consultation_packet(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    mind_event: Option<&MindObserverFeedEvent>,
) -> ConsultationPacket {
    let mut degraded_inputs = Vec::new();
    if checkpoint.is_none() {
        degraded_inputs.push("mind.compaction_checkpoint".to_string());
    }
    if mind_event.is_none() {
        degraded_inputs.push("mind.t1".to_string());
    }
    if worker
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        degraded_inputs.push("overseer.summary".to_string());
    }

    let source_status = if matches!(worker.status, WorkerStatus::Offline) {
        ConsultationSourceStatus::Stale
    } else if !degraded_inputs.is_empty() {
        ConsultationSourceStatus::Partial
    } else {
        ConsultationSourceStatus::Complete
    };

    ConsultationPacket {
        packet_id: format!("mc:{}:{}", worker.session_id, worker.agent_id),
        kind: ConsultationPacketKind::Align,
        identity: ConsultationIdentity {
            session_id: worker.session_id.clone(),
            agent_id: worker.agent_id.clone(),
            pane_id: Some(worker.pane_id.clone()),
            conversation_id: checkpoint.map(|value| value.conversation_id.clone()),
            role: worker.role.clone(),
        },
        task_context: ConsultationTaskContext {
            active_tag: worker.assignment.tag.clone(),
            task_ids: worker.assignment.task_id.iter().cloned().collect(),
            focus_summary: worker.summary.clone().or_else(|| worker.blocker.clone()),
        },
        summary: worker
            .summary
            .clone()
            .or_else(|| worker.blocker.clone())
            .or_else(|| {
                Some(format!(
                    "status={} phase={:?}",
                    format!("{:?}", worker.status).to_ascii_lowercase(),
                    worker.progress.phase
                ))
            }),
        checkpoint: checkpoint.map(|value| ConsultationCheckpointRef {
            checkpoint_id: value.checkpoint_id.clone(),
            conversation_id: Some(value.conversation_id.clone()),
            compaction_entry_id: value.compaction_entry_id.clone(),
            ts: Some(value.ts.to_rfc3339()),
        }),
        freshness: ConsultationFreshness {
            packet_generated_at: Some(Utc::now().to_rfc3339()),
            source_updated_at: worker
                .last_update_at_ms
                .and_then(ms_to_datetime)
                .map(|ts| ts.to_rfc3339()),
            stale_after_ms: worker.stale_after_ms,
            source_status,
            degraded_inputs: degraded_inputs.clone(),
        },
        confidence: ConsultationConfidence {
            overall_bps: Some(overseer_consultation_confidence_bps(
                worker, checkpoint, mind_event,
            )),
            rationale: Some(overseer_consultation_rationale(
                worker, checkpoint, mind_event,
            )),
        },
        help_request: overseer_help_request(worker),
        degraded_reason: (!degraded_inputs.is_empty())
            .then(|| format!("packet derived with partial inputs: {}", degraded_inputs.join(", "))),
        ..Default::default()
    }
    .normalize()
}

fn overseer_help_request(worker: &WorkerSnapshot) -> Option<ConsultationHelpRequest> {
    if matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        return Some(ConsultationHelpRequest {
            kind: if matches!(worker.status, WorkerStatus::Blocked) {
                "blocker_escalation".to_string()
            } else {
                "alignment_request".to_string()
            },
            question: worker
                .blocker
                .clone()
                .unwrap_or_else(|| "need bounded manager guidance".to_string()),
            requested_from: Some("mission_control".to_string()),
            urgency: Some(if matches!(worker.status, WorkerStatus::Blocked) {
                "high".to_string()
            } else {
                "medium".to_string()
            }),
        });
    }
    None
}

pub(crate) fn should_render_overseer_consultation_line(
    packet: &ConsultationPacket,
    worker: &WorkerSnapshot,
) -> bool {
    matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) || matches!(worker.drift_risk, DriftRisk::High)
        || matches!(
            packet.freshness.source_status,
            ConsultationSourceStatus::Partial | ConsultationSourceStatus::Stale
        )
        || worker.assignment.task_id.is_none() && worker.assignment.tag.is_none()
        || packet.help_request.is_some()
}

fn overseer_consultation_confidence_bps(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    mind_event: Option<&MindObserverFeedEvent>,
) -> u16 {
    let mut score = 600u16;
    if checkpoint.is_some() {
        score += 100;
    }
    if mind_event.is_some() {
        score += 100;
    }
    if worker.assignment.task_id.is_some() || worker.assignment.tag.is_some() {
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
    score.min(1000)
}

fn overseer_consultation_rationale(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    mind_event: Option<&MindObserverFeedEvent>,
) -> String {
    let mut parts = Vec::new();
    parts.push(if checkpoint.is_some() {
        "checkpoint linked"
    } else {
        "checkpoint missing"
    });
    parts.push(if mind_event.is_some() {
        "mind signal present"
    } else {
        "mind signal missing"
    });
    parts.push(
        if worker.assignment.task_id.is_some() || worker.assignment.tag.is_some() {
            "task context present"
        } else {
            "task context missing"
        },
    );
    if matches!(worker.drift_risk, DriftRisk::High) {
        parts.push("high drift risk");
    }
    if matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        parts.push("operator input needed");
    }
    parts.join(", ")
}

pub(crate) fn render_overseer_consultation_line(
    packet: &ConsultationPacket,
    worker: &WorkerSnapshot,
    theme: MissionTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let suggestion = if matches!(worker.status, WorkerStatus::Blocked) {
        "ask for unblock plan + evidence-backed next step".to_string()
    } else if matches!(worker.status, WorkerStatus::NeedsInput) {
        "send alignment prompt with explicit decision request".to_string()
    } else if matches!(worker.drift_risk, DriftRisk::High) {
        "request concise alignment + validation plan".to_string()
    } else if worker.assignment.task_id.is_none() && worker.assignment.tag.is_none() {
        "assign task/tag before further implementation".to_string()
    } else if packet.freshness.source_status == ConsultationSourceStatus::Stale {
        "request fresh status update before steering".to_string()
    } else {
        "continue current lane; request validation if milestone reached".to_string()
    };

    let meta = format!(
        "src:{} conf:{}",
        format!("{:?}", packet.freshness.source_status).to_ascii_lowercase(),
        packet.confidence.overall_bps.unwrap_or_default()
    );
    Some(Line::from(vec![
        Span::styled("    mc ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("[{}]", consultation_packet_kind_label(packet.kind)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate_text(&suggestion, if compact { 48 } else { 84 }),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(meta, Style::default().fg(theme.muted)),
    ]))
}

fn consultation_packet_kind_label(kind: ConsultationPacketKind) -> &'static str {
    match kind {
        ConsultationPacketKind::Summary => "summary",
        ConsultationPacketKind::Plan => "plan",
        ConsultationPacketKind::Blockers => "blockers",
        ConsultationPacketKind::Review => "review",
        ConsultationPacketKind::Align => "align",
        ConsultationPacketKind::CheckpointStatus => "checkpoint",
        ConsultationPacketKind::HelpRequest => "help",
    }
}
