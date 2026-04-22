//! Overseer worker and mind-signal rendering helpers.

use super::*;

pub(crate) fn render_overseer_worker_line(
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
            spans.push(Span::styled(branch.clone(), Style::default().fg(theme.muted)));
        }
    }
    spans
}

pub(crate) fn should_render_overseer_attention_reason(worker: &WorkerSnapshot) -> bool {
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

pub(crate) fn render_overseer_mind_line(
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
