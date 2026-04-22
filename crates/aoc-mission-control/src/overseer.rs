//! Overseer surface rendering and consultation helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;
use crate::overseer_consultation::{
    derive_overseer_consultation_packet, render_overseer_consultation_line,
    should_render_overseer_consultation_line,
};
use crate::overseer_ops_render::{
    render_orchestration_compile_line, render_orchestration_graph_summary_line,
    render_orchestrator_tool_line, render_overseer_timeline_line,
};
use crate::overseer_worker_render::{
    render_overseer_mind_line, render_overseer_worker_line,
    should_render_overseer_attention_reason,
};

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

