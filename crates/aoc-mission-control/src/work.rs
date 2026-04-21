//! Work surface rendering.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn render_work_lines(app: &App, theme: MissionTheme, compact: bool) -> Vec<Line<'static>> {
    let projects = app.work_rows();
    if projects.is_empty() {
        return vec![Line::from(Span::styled(
            "No task data available.",
            Style::default().fg(theme.muted),
        ))];
    }
    let mut lines = Vec::new();
    for project in projects {
        lines.push(Line::from(vec![
            Span::styled(
                format!("Project {}", short_project(&project.project_root, 28)),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", project.scope),
                Style::default().fg(theme.muted),
            ),
        ]));
        for tag in project.tags {
            let mut spans = vec![
                Span::raw("  "),
                Span::styled(
                    format!("{}", ellipsize(&tag.tag, 18)),
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ];
            spans.extend(task_bar_spans(
                &tag.counts,
                if compact { 12 } else { 18 },
                theme,
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("ip:{}", tag.counts.in_progress),
                Style::default().fg(if tag.counts.in_progress > 0 {
                    theme.info
                } else {
                    theme.muted
                }),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("blk:{}", tag.counts.blocked),
                Style::default().fg(if tag.counts.blocked > 0 {
                    theme.critical
                } else {
                    theme.muted
                }),
            ));
            lines.push(Line::from(spans));
            if let Some(item) = tag.in_progress_titles.first() {
                lines.push(Line::from(vec![
                    Span::raw("    -> "),
                    Span::styled(
                        ellipsize(item, if compact { 40 } else { 72 }),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
        }
        if !compact {
            lines.push(Line::from(""));
        }
    }
    lines
}
