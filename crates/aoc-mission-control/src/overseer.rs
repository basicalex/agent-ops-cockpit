//! Overseer surface rendering and consultation helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn render_overseer_lines(
    app: &App,
    theme: MissionTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let workers = app.overseer_workers();
    let timeline = app.overseer_timeline();
    let generated_at = app
        .overseer_snapshot()
        .and_then(|snapshot| snapshot.generated_at_ms)
        .and_then(ms_to_datetime);
    let artifact_drilldown =
        load_mind_artifact_drilldown(&app.config.project_root, &app.config.session_id);
    let checkpoint = artifact_drilldown.latest_compaction_checkpoint.as_ref();

    if workers.is_empty() && timeline.is_empty() {
        return vec![
            Line::from(Span::styled(
                "No overseer snapshot received yet.",
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                "Waiting for hub observer_snapshot / observer_timeline topics.",
                Style::default().fg(theme.muted),
            )),
        ];
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Workers ",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}", workers.len()),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · timeline "),
        Span::styled(
            format!("{}", timeline.len()),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · generated "),
        Span::styled(
            generated_at
                .map(|value| value.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            Style::default().fg(theme.muted),
        ),
    ])];

    if let Some(snapshot) = app
        .overseer_snapshot()
        .and_then(|snapshot| snapshot.degraded_reason.as_ref())
    {
        lines.push(Line::from(Span::styled(
            format!("degraded: {snapshot}"),
            Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Workers",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));

    for worker in workers.iter().take(if compact { 8 } else { 12 }) {
        let mind_event = app.overseer_mind_event(&worker.agent_id);
        let consultation_packet =
            derive_overseer_consultation_packet(worker, checkpoint, mind_event);
        lines.push(Line::from(render_overseer_worker_line(
            worker, mind_event, theme, compact,
        )));
        if let Some(summary) = worker
            .summary
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(Line::from(Span::styled(
                format!(
                    "    {}",
                    truncate_text(summary, if compact { 72 } else { 110 })
                ),
                Style::default().fg(theme.muted),
            )));
        }
        if should_render_overseer_attention_reason(worker) {
            if let Some(reason) = worker
                .attention
                .reason
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                lines.push(Line::from(Span::styled(
                    format!(
                        "    attention: {}",
                        truncate_text(reason, if compact { 68 } else { 104 })
                    ),
                    Style::default().fg(theme.warn),
                )));
            }
        }
        if let Some(event) = mind_event {
            if let Some(line) = render_overseer_mind_line(event, theme, compact) {
                lines.push(line);
            }
        }
        if should_render_overseer_consultation_line(&consultation_packet, worker) {
            if let Some(line) =
                render_overseer_consultation_line(&consultation_packet, worker, theme, compact)
            {
                lines.push(line);
            }
        }
    }

    let tools = app.orchestrator_tools();
    if !tools.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Mission Control tools",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        for tool in tools.iter().take(if compact { 5 } else { 8 }) {
            lines.push(render_orchestrator_tool_line(tool, theme, compact));
        }

        let graph = app.orchestration_graph_ir();
        if !graph.compile_paths.is_empty() {
            lines.push(render_orchestration_graph_summary_line(&graph, theme));
            lines.push(Line::from(Span::styled(
                "Reviewable compile",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )));
            for path in graph.compile_paths.iter().take(if compact { 2 } else { 6 }) {
                lines.push(render_orchestration_compile_line(path, theme, compact));
            }
        }
    }

    if !timeline.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recent timeline",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        for entry in timeline.iter().take(if compact { 6 } else { 10 }) {
            lines.push(Line::from(render_overseer_timeline_line(entry, theme)));
        }
    }

    lines
}

fn render_overseer_worker_line(
    worker: &WorkerSnapshot,
    mind_event: Option<&MindObserverFeedEvent>,
    theme: MissionTheme,
    compact: bool,
) -> Vec<Span<'static>> {
    let scope = worker
        .role
        .clone()
        .unwrap_or_else(|| extract_label(&worker.agent_id));
    let task = worker
        .assignment
        .task_id
        .clone()
        .or_else(|| worker.assignment.tag.clone())
        .unwrap_or_else(|| "unassigned".to_string());
    let progress = worker
        .progress
        .percent
        .map(|value| format!("{}%", value))
        .unwrap_or_else(|| format!("{:?}", worker.progress.phase).to_ascii_lowercase());
    let align = format!("{:?}", worker.plan_alignment).to_ascii_lowercase();
    let drift = format!("{:?}", worker.drift_risk).to_ascii_lowercase();
    let status = format!("{:?}", worker.status).to_ascii_lowercase();
    let mut spans = vec![
        Span::styled(
            format!("[{}]", overseer_attention_label(worker.attention.level)),
            Style::default()
                .fg(overseer_attention_color(worker.attention.level, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            scope,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" ({}) ", worker.pane_id)),
        Span::styled(
            status,
            Style::default().fg(lifecycle_color_for_worker(worker, theme)),
        ),
        Span::raw(" · "),
        Span::styled(task, Style::default().fg(theme.info)),
        Span::raw(" · "),
        Span::styled(progress, Style::default().fg(theme.text)),
        Span::raw(" · "),
        Span::styled(
            format!("align:{align}"),
            Style::default().fg(overseer_plan_alignment_color(worker.plan_alignment, theme)),
        ),
        Span::raw(" "),
        Span::styled(
            format!("drift:{drift}"),
            Style::default().fg(overseer_drift_color(worker.drift_risk, theme)),
        ),
    ];
    if let Some(duplicate) = worker.duplicate_work.as_ref() {
        let overlap_count =
            duplicate.overlapping_task_ids.len() + duplicate.overlapping_files.len();
        if overlap_count > 0 {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("dup:{overlap_count}"),
                Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
            ));
        }
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        overseer_provenance_label(worker, mind_event),
        Style::default().fg(overseer_provenance_color(mind_event, theme)),
    ));
    if !compact {
        if let Some(branch) = worker
            .branch
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            spans.push(Span::raw(" · "));
            spans.push(Span::styled(
                branch.clone(),
                Style::default().fg(theme.muted),
            ));
        }
    }
    spans
}

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
        degraded_reason: (!degraded_inputs.is_empty()).then(|| {
            format!(
                "packet derived with partial inputs: {}",
                degraded_inputs.join(", ")
            )
        }),
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

fn should_render_overseer_attention_reason(worker: &WorkerSnapshot) -> bool {
    if worker
        .attention
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return false;
    }

    if worker.attention.kind.as_deref() == Some("duplicate_work") {
        return false;
    }

    !matches!(worker.attention.level, AttentionLevel::None)
}

fn should_render_overseer_consultation_line(
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

fn render_orchestrator_tool_line(
    tool: &OrchestratorTool,
    theme: MissionTheme,
    compact: bool,
) -> Line<'static> {
    let status_style = match tool.status {
        OrchestratorToolStatus::Ready => Style::default().fg(theme.ok),
        OrchestratorToolStatus::Unavailable => Style::default().fg(theme.warn),
    }
    .add_modifier(Modifier::BOLD);
    let status_label = match tool.status {
        OrchestratorToolStatus::Ready => "ready",
        OrchestratorToolStatus::Unavailable => "blocked",
    };
    let mut spans = vec![
        Span::styled("    tool ", Style::default().fg(theme.muted)),
        Span::styled(format!("[{status_label}]"), status_style),
        Span::raw(" "),
        Span::styled(tool.label.to_string(), Style::default().fg(theme.info)),
    ];
    if let Some(shortcut) = tool.shortcut {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("key:{shortcut}"),
            Style::default().fg(theme.accent),
        ));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        truncate_text(&tool.summary, if compact { 36 } else { 72 }),
        Style::default().fg(theme.text),
    ));
    if let Some(reason) = tool.reason.as_ref() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            truncate_text(reason, if compact { 20 } else { 36 }),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans)
}

fn render_orchestration_graph_summary_line(
    graph: &OrchestrationGraphIr,
    theme: MissionTheme,
) -> Line<'static> {
    Line::from(vec![
        Span::styled("    graph ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{} nodes", graph.nodes.len()),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("{} edges", graph.edges.len()),
            Style::default().fg(theme.info),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("{} review paths", graph.compile_paths.len()),
            Style::default().fg(theme.text),
        ),
    ])
}

fn render_orchestration_compile_line(
    path: &OrchestrationCompilePath,
    theme: MissionTheme,
    compact: bool,
) -> Line<'static> {
    let status_style = match path.status {
        OrchestratorToolStatus::Ready => Style::default().fg(theme.ok),
        OrchestratorToolStatus::Unavailable => Style::default().fg(theme.warn),
    }
    .add_modifier(Modifier::BOLD);
    let status_label = match path.status {
        OrchestratorToolStatus::Ready => "ready",
        OrchestratorToolStatus::Unavailable => "blocked",
    };
    let preview = truncate_text(&path.steps.join(" -> "), if compact { 52 } else { 96 });
    Line::from(vec![
        Span::styled("    plan ", Style::default().fg(theme.muted)),
        Span::styled(format!("[{status_label}]"), status_style),
        Span::raw(" "),
        Span::styled(path.review_label.clone(), Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::styled(preview, Style::default().fg(theme.text)),
    ])
}

fn render_overseer_consultation_line(
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

fn render_overseer_mind_line(
    event: &MindObserverFeedEvent,
    theme: MissionTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let lane = mind_event_lane(event);
    let mut detail = event
        .reason
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| {
            event
                .failure_kind
                .as_ref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("failure:{value}"))
        })
        .or_else(|| {
            event.progress.as_ref().map(|progress| {
                format!(
                    "tokens:{}→{} next:{}",
                    progress.t0_estimated_tokens,
                    progress.t1_target_tokens,
                    progress.tokens_until_next_run
                )
            })
        })?;
    detail = truncate_text(&detail, if compact { 58 } else { 92 });
    Some(Line::from(vec![
        Span::styled("    semantic ", Style::default().fg(theme.muted)),
        Span::styled(
            format!(
                "[{}:{}]",
                mind_lane_label(lane),
                mind_status_label(event.status)
            ),
            Style::default()
                .fg(mind_status_color(event.status, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(detail, Style::default().fg(theme.info)),
    ]))
}

fn render_overseer_timeline_line(
    entry: &ObserverTimelineEntry,
    theme: MissionTheme,
) -> Vec<Span<'static>> {
    let when = entry
        .emitted_at_ms
        .and_then(ms_to_datetime)
        .map(|value| value.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".to_string());
    let kind = format!("{:?}", entry.kind).to_ascii_lowercase();
    let mut spans = vec![
        Span::styled(when, Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(entry.agent_id.clone(), Style::default().fg(theme.info)),
        Span::raw(" "),
        Span::styled(
            kind,
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(summary) = entry
        .summary
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(
            summary.clone(),
            Style::default().fg(theme.text),
        ));
    }
    if let Some(reason) = entry
        .reason
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(
            reason.clone(),
            Style::default().fg(theme.muted),
        ));
    }
    spans
}

pub(crate) fn overseer_attention_rank(level: &AttentionLevel) -> usize {
    match level {
        AttentionLevel::Critical => 4,
        AttentionLevel::Warn => 3,
        AttentionLevel::Info => 2,
        AttentionLevel::None => 1,
    }
}

pub(crate) fn overseer_drift_rank(risk: &DriftRisk) -> usize {
    match risk {
        DriftRisk::High => 4,
        DriftRisk::Medium => 3,
        DriftRisk::Low => 2,
        DriftRisk::Unknown => 1,
    }
}

fn overseer_attention_label(level: AttentionLevel) -> &'static str {
    match level {
        AttentionLevel::Critical => "critical",
        AttentionLevel::Warn => "warn",
        AttentionLevel::Info => "info",
        AttentionLevel::None => "ok",
    }
}

fn overseer_provenance_label(
    worker: &WorkerSnapshot,
    mind_event: Option<&MindObserverFeedEvent>,
) -> String {
    let base = worker
        .provenance
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(match worker.source {
            aoc_core::session_overseer::OverseerSourceKind::Wrapper => "heuristic:wrapper",
            aoc_core::session_overseer::OverseerSourceKind::Hub => "heuristic:hub",
            aoc_core::session_overseer::OverseerSourceKind::Mind => "semantic:mind",
            aoc_core::session_overseer::OverseerSourceKind::Manager => "heuristic:manager",
            aoc_core::session_overseer::OverseerSourceKind::LocalFallback => "heuristic:local",
        });
    if let Some(event) = mind_event {
        format!(
            "[prov:{}+mind:{}:{}]",
            base,
            mind_lane_label(mind_event_lane(event)),
            mind_status_label(event.status)
        )
    } else {
        format!("[prov:{base}]")
    }
}

fn overseer_provenance_color(
    mind_event: Option<&MindObserverFeedEvent>,
    theme: MissionTheme,
) -> Color {
    if let Some(event) = mind_event {
        mind_status_color(event.status, theme)
    } else {
        theme.muted
    }
}

fn overseer_attention_color(level: AttentionLevel, theme: MissionTheme) -> Color {
    match level {
        AttentionLevel::Critical => theme.critical,
        AttentionLevel::Warn => theme.warn,
        AttentionLevel::Info => theme.info,
        AttentionLevel::None => theme.ok,
    }
}

fn overseer_plan_alignment_color(level: PlanAlignment, theme: MissionTheme) -> Color {
    match level {
        PlanAlignment::High => theme.ok,
        PlanAlignment::Medium => theme.info,
        PlanAlignment::Low => theme.warn,
        PlanAlignment::Unassigned => theme.warn,
        PlanAlignment::Unknown => theme.muted,
    }
}

fn overseer_drift_color(level: DriftRisk, theme: MissionTheme) -> Color {
    match level {
        DriftRisk::High => theme.critical,
        DriftRisk::Medium => theme.warn,
        DriftRisk::Low => theme.ok,
        DriftRisk::Unknown => theme.muted,
    }
}

fn lifecycle_color_for_worker(worker: &WorkerSnapshot, theme: MissionTheme) -> Color {
    match worker.status {
        WorkerStatus::Done => theme.ok,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput => theme.warn,
        WorkerStatus::Offline => theme.muted,
        WorkerStatus::Active => theme.info,
        WorkerStatus::Paused | WorkerStatus::Idle => theme.muted,
    }
}
