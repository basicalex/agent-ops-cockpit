//! Mission Control Mind view coordinator.
//!
//! Thin host-side module that composes Mind rendering from dedicated surface
//! modules. Consultation persistence lives in `consultation_memory`.  Artifact
//! drilldown and compaction state live in `mind_artifact_drilldown`.  Host-side
//! render adapters (search, injection, activity bridge, task bars) live in
//! `mind_host_render`.

use super::*;

pub(crate) fn render_mind_lines(app: &App, theme: MissionTheme, compact: bool) -> Vec<Line<'static>> {
    use mind_artifact_drilldown::render_mind_artifact_drilldown_lines;
    use mind_host_render::{
        render_mind_activity_bridge_lines, render_mind_injection_rollup_line,
        render_mind_search_lines,
    };

    let rows = app.mind_rows();
    let all_rows = app.mind_rows_for_lane(MindLaneFilter::All);
    let detached_jobs = app.insight_detached_jobs();
    let artifact_snapshot =
        load_mind_artifact_drilldown(&app.config.project_root, &app.config.session_id);
    let mut lines = Vec::new();

    let lane_label = app.mind_lane.label().to_ascii_uppercase();
    let scope_label = if app.config.mind_project_scoped {
        "project"
    } else if app.mind_show_all_tabs {
        "all-tabs"
    } else {
        "active-tab"
    };
    let project_label = app.config.project_root.to_string_lossy().to_string();
    let lane_rollup = mind_lane_rollup(&all_rows);
    let mut header = vec![
        Span::styled("lane:", Style::default().fg(theme.muted)),
        Span::styled(
            lane_label,
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("scope:", Style::default().fg(theme.muted)),
        Span::styled(
            scope_label,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!(
                "t0:{} t1:{} t2:{} t3:{}",
                lane_rollup[0], lane_rollup[1], lane_rollup[2], lane_rollup[3]
            ),
            Style::default().fg(theme.muted),
        ),
    ];

    if let Some(runtime) = app.insight_runtime_rollup() {
        header.push(Span::raw("  "));
        header.push(Span::styled(
            format!(
                "t2q:{} done:{} fail:{} lock:{} | t3q:{} done:{} fail:{} rq:{} dlq:{} lock:{}",
                runtime.queue_depth,
                runtime.reflector_jobs_completed,
                runtime.reflector_jobs_failed,
                runtime.reflector_lock_conflicts,
                runtime.t3_queue_depth,
                runtime.t3_jobs_completed,
                runtime.t3_jobs_failed,
                runtime.t3_jobs_requeued,
                runtime.t3_jobs_dead_lettered,
                runtime.t3_lock_conflicts
            ),
            Style::default().fg(theme.muted),
        ));
    }
    lines.push(Line::from(header));
    lines.push(Line::from(vec![
        Span::styled("project:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            ellipsize(&project_label, if compact { 44 } else { 88 }),
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            if app.config.mind_project_scoped {
                "[project-scoped]"
            } else {
                "[session-scoped]"
            },
            Style::default().fg(theme.muted),
        ),
    ]));

    let status_rollup = mind_status_rollup(&rows);
    lines.push(Line::from(vec![
        Span::styled("status:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("q:{}", status_rollup.queued),
            Style::default().fg(theme.warn),
        ),
        Span::raw(" "),
        Span::styled(
            format!("run:{}", status_rollup.running),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(
            format!("ok:{}", status_rollup.success),
            Style::default().fg(theme.ok),
        ),
        Span::raw(" "),
        Span::styled(
            format!("fb:{}", status_rollup.fallback),
            Style::default().fg(theme.warn),
        ),
        Span::raw(" "),
        Span::styled(
            format!("err:{}", status_rollup.error),
            Style::default().fg(theme.critical),
        ),
    ]));

    let export_status = artifact_snapshot
        .latest_export
        .as_ref()
        .map(|manifest| {
            mind_timestamp_label(&manifest.exported_at)
                .map(|label| format!("latest@{label}"))
                .unwrap_or_else(|| "latest:present".to_string())
        })
        .unwrap_or_else(|| "latest:none".to_string());
    let recovery_status = if artifact_snapshot.compaction_rebuildable {
        "recovery:ready"
    } else if artifact_snapshot.compaction_marker_event_available {
        "recovery:partial"
    } else {
        "recovery:none"
    };
    lines.push(Line::from(vec![
        Span::styled("overview:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("handshake:{}", artifact_snapshot.handshake_entries.len()),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(
            format!("canon:{}", artifact_snapshot.active_canon_entries.len()),
            Style::default().fg(theme.accent),
        ),
        Span::raw(" "),
        Span::styled(
            format!("stale:{}", artifact_snapshot.stale_canon_count),
            Style::default().fg(if artifact_snapshot.stale_canon_count > 0 {
                theme.warn
            } else {
                theme.muted
            }),
        ),
        Span::raw(" "),
        Span::styled(export_status, Style::default().fg(theme.ok)),
        Span::raw(" "),
        Span::styled(
            recovery_status,
            Style::default().fg(if artifact_snapshot.compaction_rebuildable {
                theme.ok
            } else if artifact_snapshot.compaction_marker_event_available {
                theme.warn
            } else {
                theme.muted
            }),
        ),
        Span::raw(" "),
        Span::styled(
            format!("detached:{}", detached_jobs.len()),
            Style::default().fg(theme.muted),
        ),
    ]));

    let injection_rows = app.mind_injection_rows();
    if let Some(line) = render_mind_injection_rollup_line(&injection_rows, theme, compact) {
        lines.push(line);
    }

    if let Some(line) =
        aoc_mind::render_insight_detached_rollup_line(&detached_jobs, mind_theme(theme), compact)
    {
        lines.push(line);
    }

    let search_lines = render_mind_search_lines(
        &artifact_snapshot,
        &app.mind_search_query,
        app.mind_search_editing,
        app.mind_search_selected,
        theme,
        compact,
    );
    let activity_bridge_lines = render_mind_activity_bridge_lines(
        &rows,
        &injection_rows,
        &detached_jobs,
        &artifact_snapshot,
        theme,
        compact,
    );
    let artifact_lines = render_mind_artifact_drilldown_lines(
        &app.config.project_root,
        &app.config.session_id,
        theme,
        compact,
        app.mind_show_provenance,
        &all_rows,
        app.insight_runtime_rollup(),
    );

    lines.push(Line::from(vec![
        Span::styled(
            "Observer activity",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{} events]", rows.len()),
            Style::default().fg(theme.muted),
        ),
    ]));

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "No observer activity yet for current lane/scope.",
            Style::default().fg(theme.muted),
        )));
        lines.push(Line::from(Span::styled(
            "Try: o (T1), O (T1->T2 chain), b (bootstrap dry-run), t/v (filters).",
            Style::default().fg(theme.muted),
        )));
        lines.push(Line::from(""));
        lines.extend(search_lines);
        lines.push(Line::from(""));
        lines.extend(activity_bridge_lines);
        if !artifact_lines.is_empty() {
            lines.push(Line::from(""));
            lines.extend(artifact_lines);
        }
        return lines;
    }

    for row in rows {
        let status_label = mind_status_label(row.event.status);
        let status_color = mind_status_color(row.event.status, theme);
        let trigger_label = mind_trigger_label(row.event.trigger);
        let lane = mind_event_lane(&row.event);
        let lane_label = mind_lane_label(lane);
        let runtime_label = row
            .event
            .runtime
            .as_deref()
            .map(mind_runtime_label)
            .unwrap_or("runtime:n/a".to_string());
        let latency = row
            .event
            .latency_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "n/a".to_string());
        let attempts = row
            .event
            .attempt_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        let when = row
            .event
            .completed_at
            .as_deref()
            .or(row.event.started_at.as_deref())
            .or(row.event.enqueued_at.as_deref())
            .and_then(mind_timestamp_label)
            .unwrap_or_else(|| "--:--:--".to_string());

        let mut primary_spans = vec![
            Span::styled("✦", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", lane_label),
                Style::default()
                    .fg(mind_lane_color(lane, theme))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", status_label),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", trigger_label),
                Style::default().fg(theme.info),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", runtime_label),
                Style::default().fg(theme.accent),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{}::{}", row.scope, row.pane_id),
                Style::default()
                    .fg(if row.tab_focused {
                        theme.accent
                    } else {
                        theme.text
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(format!("lat:{latency}"), Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(format!("att:{attempts}"), Style::default().fg(theme.muted)),
        ];
        if let Some(progress) = row.event.progress.as_ref() {
            primary_spans.push(Span::raw(" "));
            primary_spans.push(Span::styled(
                mind_progress_label(progress),
                Style::default().fg(theme.muted),
            ));
        }
        primary_spans.push(Span::raw(" "));
        primary_spans.push(Span::styled(
            format!("@{when}"),
            Style::default().fg(theme.muted),
        ));
        lines.push(Line::from(primary_spans));

        let mut context = row
            .event
            .reason
            .clone()
            .or_else(|| row.event.failure_kind.clone())
            .or_else(|| row.event.conversation_id.map(|id| format!("conv:{id}")))
            .unwrap_or_else(|| {
                format!(
                    "source:{} tab:{} agent:{}",
                    row.source,
                    row.tab_scope
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or("n/a"),
                    row.agent_id
                )
            });
        if compact {
            context = ellipsize(&context, 52);
        }
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(context, Style::default().fg(theme.muted)),
        ]));
    }

    lines.push(Line::from(""));
    lines.extend(search_lines);
    lines.push(Line::from(""));
    lines.extend(activity_bridge_lines);

    if !artifact_lines.is_empty() {
        lines.push(Line::from(""));
        lines.extend(artifact_lines);
    }

    lines
}

pub(crate) use consultation_memory::*;

pub(crate) use mind_host_render::task_bar_spans;
