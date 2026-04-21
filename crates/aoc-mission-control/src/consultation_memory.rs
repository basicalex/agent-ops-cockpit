//! Consultation persistence / memory glue.
//!
//! Mission Control-specific helpers for persisting consultation outcomes into
//! the Mind store. Pure host concerns that belong outside aoc-mind.

use super::mind_artifact_drilldown::parse_rfc3339_utc;
use super::*;

pub(crate) fn persist_consultation_outcome(
    project_root: &Path,
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
    kind: ConsultationPacketKind,
) -> Result<String, String> {
    let store_path = mind_store_path(project_root);
    if let Some(parent) = store_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create mind store directory failed: {err}"))?;
    }
    let store = aoc_storage::MindStore::open(&store_path)
        .map_err(|err| format!("open mind store failed: {err}"))?;
    let ts = payload
        .packet
        .as_ref()
        .and_then(|packet| packet.freshness.packet_generated_at.as_deref())
        .and_then(parse_rfc3339_utc)
        .unwrap_or_else(Utc::now);
    let artifact_id = format!("consult:{}", payload.consultation_id);
    let conversation_id = consultation_memory_conversation_id(request_packet, payload);
    let trace_ids = consultation_memory_trace_ids(request_packet, payload, kind);
    let text = render_consultation_outcome_markdown(request_packet, payload, kind, ts);
    let input_hash =
        canonical_payload_hash(&(request_packet, payload, consultation_kind_slug(kind)))
            .map_err(|err| format!("hash consultation outcome failed: {err}"))?;

    store
        .insert_reflection(&artifact_id, &conversation_id, ts, &text, &trace_ids)
        .map_err(|err| format!("persist consultation reflection failed: {err}"))?;
    store
        .upsert_semantic_provenance(&SemanticProvenance {
            artifact_id: artifact_id.clone(),
            stage: SemanticStage::T2Reflector,
            runtime: SemanticRuntime::Deterministic,
            provider_name: None,
            model_id: None,
            prompt_version: "mission-control.consultation-memory.v1".to_string(),
            input_hash,
            output_hash: None,
            latency_ms: None,
            attempt_count: 1,
            fallback_used: false,
            fallback_reason: None,
            failure_kind: None,
            created_at: ts,
        })
        .map_err(|err| format!("persist consultation provenance failed: {err}"))?;

    let task_ids = consultation_memory_task_ids(request_packet, payload);
    for task_id in task_ids {
        let link = ArtifactTaskLink::new(
            artifact_id.clone(),
            task_id,
            ArtifactTaskRelation::Mentioned,
            800,
            Vec::new(),
            "mission-control.consultation-memory".to_string(),
            ts,
            None,
        )
        .map_err(|err| format!("build consultation task link failed: {err}"))?;
        store
            .upsert_artifact_task_link(&link)
            .map_err(|err| format!("persist consultation task link failed: {err}"))?;
    }

    for evidence in consultation_memory_evidence_refs(request_packet, payload) {
        if let Some(path) = evidence
            .path
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            store
                .upsert_artifact_file_link(&aoc_storage::ArtifactFileLink {
                    artifact_id: artifact_id.clone(),
                    path: path.clone(),
                    relation: evidence
                        .relation
                        .clone()
                        .unwrap_or_else(|| "consultation_evidence".to_string()),
                    source: "mission-control.consultation-memory".to_string(),
                    additions: None,
                    deletions: None,
                    staged: false,
                    untracked: false,
                    created_at: ts,
                    updated_at: ts,
                })
                .map_err(|err| format!("persist consultation file link failed: {err}"))?;
        }
    }

    Ok(artifact_id)
}

pub(crate) fn consultation_kind_slug(kind: ConsultationPacketKind) -> &'static str {
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

pub(crate) fn consultation_memory_conversation_id(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
) -> String {
    request_packet
        .identity
        .conversation_id
        .clone()
        .or_else(|| {
            payload
                .packet
                .as_ref()
                .and_then(|packet| packet.identity.conversation_id.clone())
        })
        .unwrap_or_else(|| format!("consultation:{}", request_packet.identity.session_id))
}

pub(crate) fn consultation_memory_trace_ids(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
    kind: ConsultationPacketKind,
) -> Vec<String> {
    let mut trace_ids = vec![
        format!("consultation:{}", payload.consultation_id),
        format!("consultation_kind:{}", consultation_kind_slug(kind)),
        format!("consultation_status:{:?}", payload.status).to_ascii_lowercase(),
        format!("requester:{}", payload.requesting_agent_id),
        format!("responder:{}", payload.responding_agent_id),
    ];
    if !request_packet.packet_id.trim().is_empty() {
        trace_ids.push(format!("request_packet:{}", request_packet.packet_id));
    }
    if let Some(packet) = payload.packet.as_ref() {
        if !packet.packet_id.trim().is_empty() {
            trace_ids.push(format!("response_packet:{}", packet.packet_id));
        }
    }
    trace_ids.sort();
    trace_ids.dedup();
    trace_ids
}

pub(crate) fn consultation_memory_task_ids(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
) -> Vec<String> {
    let mut task_ids = request_packet.task_context.task_ids.clone();
    if let Some(packet) = payload.packet.as_ref() {
        task_ids.extend(packet.task_context.task_ids.iter().cloned());
    }
    task_ids.sort();
    task_ids.dedup();
    task_ids.retain(|value| !value.trim().is_empty());
    task_ids
}

pub(crate) fn consultation_memory_evidence_refs(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
) -> Vec<aoc_core::consultation_contracts::ConsultationEvidenceRef> {
    let mut refs = request_packet.evidence_refs.clone();
    if let Some(packet) = payload.packet.as_ref() {
        refs.extend(packet.evidence_refs.iter().cloned());
    }
    refs.sort_by(|left, right| {
        left.reference
            .cmp(&right.reference)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.relation.cmp(&right.relation))
    });
    refs.dedup_by(|left, right| {
        left.reference == right.reference
            && left.path == right.path
            && left.relation == right.relation
    });
    refs
}

pub(crate) fn render_consultation_outcome_markdown(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
    kind: ConsultationPacketKind,
    ts: DateTime<Utc>,
) -> String {
    let mut lines = vec![
        "# Consultation outcome".to_string(),
        String::new(),
        format!("- consultation_id: {}", payload.consultation_id),
        format!("- kind: {}", consultation_kind_slug(kind)),
        format!(
            "- status: {}",
            format!("{:?}", payload.status).to_ascii_lowercase()
        ),
        format!("- requester: {}", payload.requesting_agent_id),
        format!("- responder: {}", payload.responding_agent_id),
        format!("- recorded_at: {}", ts.to_rfc3339()),
    ];
    if let Some(tag) = request_packet.task_context.active_tag.as_ref() {
        lines.push(format!("- active_tag: {tag}"));
    }
    if !request_packet.task_context.task_ids.is_empty() {
        lines.push(format!(
            "- tasks: {}",
            request_packet.task_context.task_ids.join(", ")
        ));
    }
    lines.push(String::new());
    lines.push("## Request".to_string());
    if let Some(summary) = request_packet.summary.as_ref() {
        lines.push(summary.clone());
    }
    if let Some(help) = request_packet.help_request.as_ref() {
        lines.push(format!("- help_request [{}]: {}", help.kind, help.question));
    }
    if !request_packet.blockers.is_empty() {
        lines.push("- blockers:".to_string());
        for blocker in &request_packet.blockers {
            lines.push(format!("  - {}", blocker.summary));
        }
    }
    if !request_packet.current_plan.is_empty() {
        lines.push("- request_plan:".to_string());
        for item in &request_packet.current_plan {
            lines.push(format!("  - {}", item.title));
        }
    }

    lines.push(String::new());
    lines.push("## Response".to_string());
    if let Some(packet) = payload.packet.as_ref() {
        if let Some(summary) = packet.summary.as_ref() {
            lines.push(summary.clone());
        }
        if !packet.current_plan.is_empty() {
            lines.push("- response_plan:".to_string());
            for item in &packet.current_plan {
                lines.push(format!("  - {}", item.title));
            }
        }
        if !packet.blockers.is_empty() {
            lines.push("- response_blockers:".to_string());
            for blocker in &packet.blockers {
                lines.push(format!("  - {}", blocker.summary));
            }
        }
        if let Some(rationale) = packet.confidence.rationale.as_ref() {
            lines.push(format!("- rationale: {rationale}"));
        }
    } else if let Some(message) = payload.message.as_ref() {
        lines.push(message.clone());
    }
    if let Some(error) = payload.error.as_ref() {
        lines.push(format!("- error [{}]: {}", error.code, error.message));
    }

    let evidence_refs = consultation_memory_evidence_refs(request_packet, payload);
    if !evidence_refs.is_empty() {
        lines.push(String::new());
        lines.push("## Evidence refs".to_string());
        for evidence in evidence_refs {
            let mut line = format!("- {}", evidence.reference);
            if let Some(label) = evidence.label.as_ref() {
                line.push_str(&format!(" — {label}"));
            }
            if let Some(path) = evidence.path.as_ref() {
                line.push_str(&format!(" ({path})"));
            }
            lines.push(line);
        }
    }

    lines.join("\n")
}
