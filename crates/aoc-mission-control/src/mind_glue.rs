//! Mission Control Mind view coordinator.
//!
//! Thin host-side module that composes Mind rendering from dedicated surface
//! modules. Consultation persistence lives in `consultation_memory`. Artifact
//! drilldown and compaction state live in `mind_artifact_drilldown`. Host-side
//! render adapters (search, injection, activity bridge, task bars) live in
//! `mind_host_render`. Summary and observer event sections live in
//! `mind_summary_render`.

use super::*;

pub(crate) fn render_mind_lines(
    app: &App,
    theme: MissionTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    use mind_artifact_drilldown::render_mind_artifact_drilldown_lines;
    use mind_host_render::{
        render_mind_activity_bridge_lines, render_mind_injection_rollup_line,
        render_mind_search_lines,
    };
    use mind_summary_render::{render_mind_observer_lines, render_mind_summary_lines};

    let rows = app.mind_rows();
    let all_rows = app.mind_rows_for_lane(MindLaneFilter::All);
    let detached_jobs = app.insight_detached_jobs();
    let artifact_snapshot =
        load_mind_artifact_drilldown(&app.config.project_root, &app.config.session_id);
    let injection_rows = app.mind_injection_rows();
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

    let mut lines = render_mind_summary_lines(app, &artifact_snapshot, &detached_jobs, theme, compact);

    if let Some(line) = render_mind_injection_rollup_line(&injection_rows, theme, compact) {
        lines.push(line);
    }

    if let Some(line) =
        aoc_mind::render_insight_detached_rollup_line(&detached_jobs, mind_theme(theme), compact)
    {
        lines.push(line);
    }

    lines.extend(render_mind_observer_lines(&rows, theme, compact));
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
