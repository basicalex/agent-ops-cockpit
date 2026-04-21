//! Fleet surface rendering.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn render_fleet_lines(
    app: &App,
    theme: MissionTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let rows = app.detached_fleet_rows();
    let mut lines = Vec::new();

    if rows.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "Detached fleet",
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("hub data unavailable", Style::default().fg(theme.muted)),
        ]));
        lines.push(Line::from(Span::styled(
            "No detached job snapshots published yet. Launch detached specialists or reconnect Mission Control to the session hub.",
            Style::default().fg(theme.muted),
        )));
        return lines;
    }

    let total_jobs: usize = rows.iter().map(|row| row.jobs.len()).sum();
    let delegated_jobs: usize = rows
        .iter()
        .filter(|row| matches!(row.owner_plane, InsightDetachedOwnerPlane::Delegated))
        .map(|row| row.jobs.len())
        .sum();
    let mind_jobs = total_jobs.saturating_sub(delegated_jobs);
    let selected = app.selected_fleet_index_for_rows(&rows);
    lines.push(Line::from(vec![
        Span::styled("groups:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("{}", rows.len()),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("jobs:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("{}", total_jobs),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("plane:{}", app.fleet_plane_filter.label()),
            Style::default().fg(theme.accent),
        ),
        Span::raw("  "),
        Span::styled(
            if app.fleet_active_only {
                "scope:active"
            } else {
                "scope:all"
            },
            Style::default().fg(theme.accent),
        ),
        Span::raw("  "),
        Span::styled(
            format!("sort:{}", app.fleet_sort_mode.label()),
            Style::default().fg(theme.accent),
        ),
        Span::raw("  "),
        Span::styled(
            format!("delegated:{}", delegated_jobs),
            Style::default().fg(theme.accent),
        ),
        Span::raw(" "),
        Span::styled(
            format!("mind:{}", mind_jobs),
            Style::default().fg(theme.accent),
        ),
    ]));

    for (index, row) in rows.iter().enumerate() {
        let latest = match row.jobs.first() {
            Some(job) => job,
            None => continue,
        };
        let is_selected = index == selected;
        let mut queued = 0usize;
        let mut running = 0usize;
        let mut success = 0usize;
        let mut fallback = 0usize;
        let mut error = 0usize;
        let mut cancelled = 0usize;
        let mut stale = 0usize;
        for job in &row.jobs {
            match job.status {
                InsightDetachedJobStatus::Queued => queued += 1,
                InsightDetachedJobStatus::Running => running += 1,
                InsightDetachedJobStatus::Success => success += 1,
                InsightDetachedJobStatus::Fallback => fallback += 1,
                InsightDetachedJobStatus::Error => error += 1,
                InsightDetachedJobStatus::Cancelled => cancelled += 1,
                InsightDetachedJobStatus::Stale => stale += 1,
            }
        }
        let when = latest
            .finished_at_ms
            .or(latest.started_at_ms)
            .unwrap_or(latest.created_at_ms);
        let when = Utc
            .timestamp_millis_opt(when)
            .single()
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "--:--:--".to_string());
        let mut summary_fields = vec![
            format!("jobs:{}", row.jobs.len()),
            format!("q:{queued}"),
            format!("run:{running}"),
            format!("ok:{success}"),
            format!("fb:{fallback}"),
            format!("err:{error}"),
        ];
        if cancelled > 0 {
            summary_fields.push(format!("cx:{cancelled}"));
        }
        if stale > 0 {
            summary_fields.push(format!("stale:{stale}"));
        }
        let summary = fit_fields(&summary_fields, if compact { 42 } else { 76 });
        let latest_label = latest
            .agent
            .as_deref()
            .or(latest.chain.as_deref())
            .or(latest.team.as_deref())
            .unwrap_or("detached-job");
        lines.push(Line::from(vec![
            Span::styled(
                if is_selected { ">>" } else { "  " },
                Style::default().fg(if is_selected {
                    theme.accent
                } else {
                    theme.muted
                }),
            ),
            Span::raw(" "),
            Span::styled(
                detached_owner_plane_label(row.owner_plane),
                Style::default()
                    .fg(if is_selected {
                        theme.accent
                    } else {
                        theme.info
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                ellipsize(&row.project_root, if compact { 28 } else { 52 }),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(summary, Style::default().fg(theme.accent)),
            Span::raw(" "),
            Span::styled(format!("@{when}"), Style::default().fg(theme.muted)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   latest: "),
            Span::styled(
                format!("[{}]", detached_job_status_label(latest.status)),
                Style::default()
                    .fg(detached_job_status_color(latest.status, theme))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                ellipsize(latest_label, if compact { 18 } else { 28 }),
                Style::default().fg(theme.info),
            ),
            Span::raw(" "),
            Span::styled(
                detached_worker_kind_display(latest.owner_plane, latest.worker_kind),
                Style::default().fg(theme.muted),
            ),
            Span::raw(" "),
            Span::styled(
                detached_job_attention_label(latest)
                    .map(|label| format!("[{label}]"))
                    .unwrap_or_else(|| "[steady]".to_string()),
                Style::default().fg(detached_job_attention_color(latest, theme)),
            ),
        ]));
    }

    if let Some(row) = rows.get(selected) {
        let selected_job_index = app.selected_fleet_job_index_for_row(row);
        let selected_job = row.jobs.get(selected_job_index);
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "Drilldown",
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{} · {} jobs", row.project_root, row.jobs.len()),
                Style::default().fg(theme.info),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "job:{}/{}",
                    selected_job_index.saturating_add(1),
                    row.jobs.len()
                ),
                Style::default().fg(theme.accent),
            ),
        ]));
        if let Some(job) = selected_job {
            let target = job
                .agent
                .as_deref()
                .or(job.chain.as_deref())
                .or(job.team.as_deref())
                .unwrap_or("detached-job");
            lines.push(Line::from(vec![
                Span::raw("  selected: "),
                Span::styled(job.job_id.clone(), Style::default().fg(theme.accent)),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", detached_job_status_label(job.status)),
                    Style::default()
                        .fg(detached_job_status_color(job.status, theme))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(target.to_string(), Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  plane/kind: "),
                Span::styled(
                    format!(
                        "{} / {}",
                        detached_owner_plane_label(job.owner_plane),
                        detached_worker_kind_display(job.owner_plane, job.worker_kind)
                    ),
                    Style::default().fg(theme.muted),
                ),
                Span::raw("  "),
                Span::raw("attention: "),
                Span::styled(
                    detached_job_attention_label(job).unwrap_or_else(|| "steady".to_string()),
                    Style::default().fg(detached_job_attention_color(job, theme)),
                ),
                Span::raw("  "),
                Span::raw("steps: "),
                Span::styled(
                    match (job.current_step_index, job.step_count) {
                        (Some(current), Some(total)) => format!("{current}/{total}"),
                        (_, Some(total)) => format!("?/{total}"),
                        _ => "n/a".to_string(),
                    },
                    Style::default().fg(theme.muted),
                ),
                Span::raw("  "),
                Span::raw("fallback: "),
                Span::styled(
                    if job.fallback_used { "yes" } else { "no" },
                    Style::default().fg(if job.fallback_used {
                        theme.warn
                    } else {
                        theme.ok
                    }),
                ),
            ]));
            if let Some(detail) = job
                .output_excerpt
                .as_deref()
                .or(job.error.as_deref())
                .map(|value| ellipsize(value, if compact { 56 } else { 104 }))
            {
                lines.push(Line::from(vec![
                    Span::raw("  summary: "),
                    Span::styled(detail, Style::default().fg(theme.muted)),
                ]));
            }
            lines.push(Line::from(Span::styled(
                "  recovery:",
                Style::default().fg(theme.title),
            )));
            for guidance in detached_job_recovery_guidance(job)
                .into_iter()
                .take(if compact { 2 } else { 3 })
            {
                lines.push(Line::from(vec![
                    Span::raw("    - "),
                    Span::styled(
                        ellipsize(&guidance, if compact { 64 } else { 116 }),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
            lines.push(Line::from(Span::styled(
                "  recent jobs:",
                Style::default().fg(theme.title),
            )));
            for (index, job) in row
                .jobs
                .iter()
                .take(if compact { 3 } else { 5 })
                .enumerate()
            {
                let label = job
                    .agent
                    .as_deref()
                    .or(job.chain.as_deref())
                    .or(job.team.as_deref())
                    .unwrap_or("detached-job");
                let is_selected_job = index == selected_job_index;
                lines.push(Line::from(vec![
                    Span::styled(
                        if is_selected_job { "    > " } else { "    - " },
                        Style::default().fg(if is_selected_job {
                            theme.accent
                        } else {
                            theme.muted
                        }),
                    ),
                    Span::styled(
                        job.job_id.clone(),
                        Style::default().fg(if is_selected_job {
                            theme.accent
                        } else {
                            theme.info
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("[{}]", detached_job_status_label(job.status)),
                        Style::default().fg(detached_job_status_color(job.status, theme)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        ellipsize(label, if compact { 16 } else { 28 }),
                        Style::default().fg(theme.text),
                    ),
                ]));
            }
        }
    }

    lines
}
