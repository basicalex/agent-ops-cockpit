//! Overseer orchestration-tool and timeline rendering helpers.

use super::*;

pub(crate) fn render_orchestrator_tool_line(
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

pub(crate) fn render_orchestration_graph_summary_line(
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

pub(crate) fn render_orchestration_compile_line(
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

pub(crate) fn render_overseer_timeline_line(
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
        spans.push(Span::styled(summary.clone(), Style::default().fg(theme.text)));
    }
    if let Some(reason) = entry
        .reason
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(reason.clone(), Style::default().fg(theme.muted)));
    }
    spans
}
