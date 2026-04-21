//! Diff surface rendering.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn render_diff_lines(
    app: &App,
    theme: MissionTheme,
    compact: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let projects = app.diff_rows();
    if projects.is_empty() {
        return vec![Line::from(Span::styled(
            "No diff data available.",
            Style::default().fg(theme.muted),
        ))];
    }
    let mut lines = Vec::new();
    for project in projects {
        lines.push(Line::from(vec![
            Span::styled(
                format!("Repo {}", short_project(&project.project_root, 28)),
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
        if !project.git_available {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!(
                        "diff unavailable: {}",
                        project.reason.unwrap_or_else(|| "unknown".to_string())
                    ),
                    Style::default().fg(theme.critical),
                ),
            ]));
            if !compact {
                lines.push(Line::from(""));
            }
            continue;
        }
        let dirty = project.summary.staged.files
            + project.summary.unstaged.files
            + project.summary.untracked.files;
        let churn = project.summary.staged.additions
            + project.summary.staged.deletions
            + project.summary.unstaged.additions
            + project.summary.unstaged.deletions;
        let (risk_label, risk_color) = if churn > 200 || dirty > 24 {
            ("risk:high", theme.critical)
        } else if churn > 80 || dirty > 10 {
            ("risk:med", theme.warn)
        } else {
            ("risk:low", theme.ok)
        };
        let summary_line = fit_fields(
            &[
                format!("stg:{}", project.summary.staged.files),
                format!("uns:{}", project.summary.unstaged.files),
                format!("new:{}", project.summary.untracked.files),
                format!("churn:{}", churn),
            ],
            width.saturating_sub(18) as usize,
        );
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                risk_label,
                Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(summary_line, Style::default().fg(theme.muted)),
        ]));
        let file_limit = if compact { 4 } else { MAX_DIFF_FILES };
        let path_max = width.saturating_sub(if compact { 28 } else { 34 }) as usize;
        for file in project.files.iter().take(file_limit) {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{}", short_status(&file.status)),
                    Style::default()
                        .fg(diff_status_color(&file.status, theme))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("+{}", file.additions),
                    Style::default().fg(theme.ok),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("-{}", file.deletions),
                    Style::default().fg(theme.critical),
                ),
                Span::raw(" "),
                Span::styled(
                    ellipsize(&file.path, path_max.max(16)),
                    Style::default().fg(theme.text),
                ),
            ]));
        }
        if !compact {
            lines.push(Line::from(""));
        }
    }
    lines
}

fn diff_status_color(status: &str, theme: MissionTheme) -> Color {
    match status {
        "added" => theme.ok,
        "deleted" => theme.critical,
        "renamed" => theme.accent,
        "untracked" => theme.info,
        _ => theme.warn,
    }
}
