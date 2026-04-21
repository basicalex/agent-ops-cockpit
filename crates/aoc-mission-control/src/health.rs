//! Health surface rendering.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn render_health_lines(
    app: &App,
    theme: MissionTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            "Hub",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            if app.connected {
                "connected"
            } else {
                "offline (fallback active)"
            },
            Style::default().fg(if app.connected {
                theme.ok
            } else {
                theme.critical
            }),
        ),
    ]));
    let health_rows = app.health_rows();
    for (idx, row) in health_rows.iter().enumerate() {
        if idx > 0 && !compact {
            lines.push(Line::from(""));
        }
        push_health_snapshot_lines(
            &mut lines,
            &row.snapshot,
            &row.scope,
            &row.project_root,
            theme,
            compact,
        );
    }
    lines
}

fn push_health_snapshot_lines(
    lines: &mut Vec<Line<'static>>,
    snapshot: &HealthSnapshot,
    scope: &str,
    project_root: &str,
    theme: MissionTheme,
    compact: bool,
) {
    lines.push(Line::from(vec![
        Span::styled(scope.to_string(), Style::default().fg(theme.title)),
        Span::raw(" "),
        Span::styled(
            format!(
                "@{}",
                short_project(project_root, if compact { 16 } else { 28 })
            ),
            Style::default().fg(theme.muted),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  taskmaster ", Style::default().fg(theme.title)),
        Span::styled(
            ellipsize(&snapshot.taskmaster_status, if compact { 34 } else { 72 }),
            Style::default().fg(if snapshot.taskmaster_status.contains("available") {
                theme.ok
            } else {
                theme.warn
            }),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "  dependencies",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    for dep in &snapshot.dependencies {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "*",
                Style::default().fg(if dep.available {
                    theme.ok
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(dep.name.clone(), Style::default().fg(theme.text)),
            Span::raw(" "),
            Span::styled(
                if dep.available { "ok" } else { "missing" },
                Style::default().fg(if dep.available {
                    theme.ok
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!(
                    "({})",
                    dep.path.clone().unwrap_or_else(|| "not found".to_string())
                ),
                Style::default().fg(theme.muted),
            ),
        ]));
    }
    lines.push(Line::from(Span::styled(
        "  checks",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    for check in &snapshot.checks {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                check.name.clone(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                check.status.clone(),
                Style::default().fg(check_status_color(&check.status, theme)),
            ),
            Span::raw(" "),
            Span::styled(
                check.timestamp.clone().unwrap_or_else(|| "n/a".to_string()),
                Style::default().fg(theme.muted),
            ),
            Span::raw(" "),
            Span::styled(
                ellipsize(
                    check.details.as_deref().unwrap_or(""),
                    if compact { 20 } else { 52 },
                ),
                Style::default().fg(theme.muted),
            ),
        ]));
    }
}

fn check_status_color(status: &str, theme: MissionTheme) -> Color {
    match status.to_ascii_lowercase().as_str() {
        "ok" | "pass" | "passed" | "success" | "done" => theme.ok,
        "fail" | "failed" | "error" => theme.critical,
        "unknown" => theme.muted,
        _ => theme.warn,
    }
}
