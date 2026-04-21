//! Mind host render helpers.
//!
//! Host-side render adapters for Mind search, injection, and activity bridge
//! that use Mission Control's MissionTheme rather than aoc-mind's canonical
//! MindTheme. These preserve the richer Mission Control rendering format.

use super::*;

pub(crate) fn render_mind_search_lines(
    snapshot: &MindArtifactDrilldown,
    query: &str,
    editing: bool,
    selected: usize,
    theme: MissionTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Retrieval / search",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            if editing {
                "[editing]"
            } else {
                "[/ edit · n/N browse]"
            },
            Style::default().fg(theme.muted),
        ),
    ])];
    let prompt = if query.trim().is_empty() {
        ""
    } else {
        query.trim()
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("query:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            if prompt.is_empty() {
                "(empty)".to_string()
            } else if editing {
                format!("> {prompt}_")
            } else {
                format!("> {prompt}")
            },
            Style::default().fg(if prompt.is_empty() {
                theme.muted
            } else {
                theme.accent
            }),
        ),
    ]));
    if prompt.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "Type / then a query to search handshake, canon, and recent export summaries.",
                Style::default().fg(theme.muted),
            ),
        ]));
        return lines;
    }

    let hits = collect_mind_search_hits(snapshot, prompt);
    if hits.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "No local project Mind hits.",
                Style::default().fg(theme.warn),
            ),
        ]));
        return lines;
    }

    let selected = selected.min(hits.len().saturating_sub(1));
    lines.push(Line::from(vec![
        Span::raw("  -> "),
        Span::styled(
            format!("{} local hits", hits.len()),
            Style::default().fg(theme.muted),
        ),
        Span::raw(" "),
        Span::styled(
            format!("selected:{}/{}", selected + 1, hits.len()),
            Style::default().fg(theme.info),
        ),
    ]));
    for (index, hit) in hits.iter().enumerate() {
        let is_selected = index == selected;
        lines.push(Line::from(vec![
            Span::styled(
                if is_selected { "  >> " } else { "  • " },
                Style::default().fg(if is_selected {
                    theme.accent
                } else {
                    theme.muted
                }),
            ),
            Span::styled(
                format!("[{}]", hit.kind),
                Style::default().fg(if is_selected {
                    theme.accent
                } else {
                    theme.info
                }),
            ),
            Span::raw(" "),
            Span::styled(
                hit.title.clone(),
                Style::default().fg(if is_selected {
                    theme.text
                } else {
                    theme.accent
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!("score:{}", hit.score),
                Style::default().fg(theme.muted),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("     "),
            Span::styled(
                ellipsize(&hit.summary, if compact { 68 } else { 108 }),
                Style::default().fg(theme.muted),
            ),
        ]));
    }

    if let Some(hit) = hits.get(selected) {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("selected:", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(format!("[{}]", hit.kind), Style::default().fg(theme.info)),
            Span::raw(" "),
            Span::styled(hit.title.clone(), Style::default().fg(theme.accent)),
        ]));
        for detail in &hit.detail {
            lines.push(Line::from(vec![
                Span::raw("    - "),
                Span::styled(
                    ellipsize(detail, if compact { 68 } else { 108 }),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }
    }
    lines
}

pub(crate) fn render_mind_injection_rollup_line(
    rows: &[MindInjectionRow],
    theme: MissionTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let latest = rows.first()?;
    let trigger = latest.payload.trigger.as_str();
    let status = latest.payload.status.trim();
    let when =
        mind_timestamp_label(&latest.payload.queued_at).unwrap_or_else(|| "--:--:--".to_string());
    let mut detail_fields = Vec::new();
    if let Some(tag) = latest.payload.active_tag.as_deref() {
        detail_fields.push(format!("tag:{tag}"));
    }
    if let Some(tokens) = latest.payload.token_estimate {
        detail_fields.push(format!("tokens:{tokens}"));
    }
    if let Some(snapshot_id) = latest.payload.snapshot_id.as_deref() {
        detail_fields.push(format!("hs:{}", ellipsize(snapshot_id, 18)));
    }
    detail_fields.push(format!("scope:{}", latest.payload.scope));
    detail_fields.push(format!("pane:{}", latest.pane_id));
    if rows.len() > 1 {
        detail_fields.push(format!("agents:{}", rows.len()));
    }
    let detail = fit_fields(&detail_fields, if compact { 44 } else { 84 });
    let reason = latest
        .payload
        .reason
        .as_deref()
        .map(|value| ellipsize(value, if compact { 42 } else { 76 }))
        .unwrap_or_else(|| "awaiting bounded context injection".to_string());
    Some(Line::from(vec![
        Span::styled("inject:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", trigger),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", status),
            Style::default()
                .fg(mind_injection_status_color(status, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(detail, Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::styled(format!("@{when}"), Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(reason, Style::default().fg(theme.muted)),
    ]))
}

pub(crate) fn mind_injection_status_color(status: &str, theme: MissionTheme) -> Color {
    match status.trim() {
        "pending" => theme.info,
        "skipped-cooldown" | "skipped-duplicate" => theme.warn,
        "suppressed-pressure" => theme.critical,
        _ => theme.muted,
    }
}

pub(crate) fn render_mind_activity_bridge_lines(
    rows: &[MindObserverRow],
    injections: &[MindInjectionRow],
    jobs: &[InsightDetachedJob],
    snapshot: &MindArtifactDrilldown,
    theme: MissionTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Activity summary",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("[project-local]", Style::default().fg(theme.muted)),
    ])];

    let latest_event = rows.first().and_then(|row| {
        row.event
            .completed_at
            .as_deref()
            .or(row.event.started_at.as_deref())
            .or(row.event.enqueued_at.as_deref())
            .and_then(mind_timestamp_label)
            .map(|when| (mind_lane_label(mind_event_lane(&row.event)), when))
    });
    let latest_injection = injections.first().and_then(|row| {
        mind_timestamp_label(&row.payload.queued_at).map(|when| (row.payload.status.clone(), when))
    });
    let latest_job = jobs.first().and_then(|job| {
        let when = job
            .finished_at_ms
            .or(job.started_at_ms)
            .unwrap_or(job.created_at_ms);
        Utc.timestamp_millis_opt(when).single().map(|dt| {
            (
                detached_job_status_label(job.status).to_string(),
                dt.format("%H:%M:%S").to_string(),
            )
        })
    });

    let mut summary_fields = vec![
        format!("events:{}", rows.len()),
        format!("inject:{}", injections.len()),
        format!("detached:{}", jobs.len()),
        format!("handshake:{}", snapshot.handshake_entries.len()),
        format!("canon:{}", snapshot.active_canon_entries.len()),
    ];
    if let Some((lane, when)) = latest_event.as_ref() {
        summary_fields.push(format!("latest-event:{lane}@{when}"));
    }
    if let Some((status, when)) = latest_injection.as_ref() {
        summary_fields.push(format!("latest-inject:{status}@{when}"));
    }
    if let Some((status, when)) = latest_job.as_ref() {
        summary_fields.push(format!("latest-detached:{status}@{when}"));
    }

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            fit_fields(&summary_fields, if compact { 72 } else { 120 }),
            Style::default().fg(theme.muted),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "Mission Control bridge",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("[global follow-up]", Style::default().fg(theme.muted)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  -> "),
        Span::styled(
            "Stay here for project knowledge review; switch to Fleet for detached runtime drilldown and cancellation.",
            Style::default().fg(theme.muted),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  -> "),
        Span::styled(
            "Use Overview/Overseer when you need pane focus, worker follow, or session-level triage.",
            Style::default().fg(theme.muted),
        ),
    ]));
    let followup_hint = if jobs.is_empty() {
        "No detached project jobs yet; use o / O / b here, then Fleet if background work appears."
    } else {
        "Detached project jobs present; press 4 for Fleet to inspect or cancel them without leaving Mission Control."
    };
    lines.push(Line::from(vec![
        Span::raw("  -> "),
        Span::styled(followup_hint, Style::default().fg(theme.accent)),
    ]));
    lines
}

pub(crate) fn task_bar_spans(
    counts: &TaskCounts,
    width: usize,
    theme: MissionTheme,
) -> Vec<Span<'static>> {
    let width = width.max(6);
    let total = counts.total.max(1) as usize;
    let done_w = (counts.done as usize * width) / total;
    let in_progress_w = (counts.in_progress as usize * width) / total;
    let mut blocked_w = (counts.blocked as usize * width) / total;
    if done_w + in_progress_w + blocked_w > width {
        blocked_w = blocked_w.saturating_sub((done_w + in_progress_w + blocked_w) - width);
    }
    let used = done_w + in_progress_w + blocked_w;
    let pending_w = width.saturating_sub(used);

    let mut spans = vec![Span::styled("[", Style::default().fg(theme.muted))];
    if done_w > 0 {
        spans.push(Span::styled(
            "#".repeat(done_w),
            Style::default().fg(theme.ok),
        ));
    }
    if in_progress_w > 0 {
        spans.push(Span::styled(
            "=".repeat(in_progress_w),
            Style::default().fg(theme.info),
        ));
    }
    if blocked_w > 0 {
        spans.push(Span::styled(
            "!".repeat(blocked_w),
            Style::default().fg(theme.critical),
        ));
    }
    if pending_w > 0 {
        spans.push(Span::styled(
            "-".repeat(pending_w),
            Style::default().fg(theme.muted),
        ));
    }
    spans.push(Span::styled("]", Style::default().fg(theme.muted)));
    spans
}
