//! Mind TUI rendering helpers.
//!
//! Provides pure rendering functions that convert Mind data into
//! `Vec<Line<'static>>` suitable for any Ratatui consumer
//! (Mission Control Mind tab, standalone Mind pane, etc.).
//!
//! Extracted from `aoc-mission-control/src/main.rs` so that Mind
//! rendering lives alongside Mind query runtime in the `aoc-mind` crate.

use aoc_core::mind_observer_feed::{
    MindInjectionPayload, MindObserverFeedEvent, MindObserverFeedProgress, MindObserverFeedStatus,
    MindObserverFeedTriggerKind,
};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// --- Re-exported query types ---
use crate::query::MindArtifactDrilldown;

/// Lane classification for Mind observer events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MindLaneFilter {
    T0,
    T1,
    T2,
    T3,
    All,
}

impl MindLaneFilter {
    pub fn next(self) -> Self {
        match self {
            Self::T0 => Self::T1,
            Self::T1 => Self::T2,
            Self::T2 => Self::T3,
            Self::T3 => Self::All,
            Self::All => Self::T0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::T0 => " t0",
            Self::T1 => "t1",
            Self::T2 => "t2",
            Self::T3 => "t3",
            Self::All => "all",
        }
    }
}

impl std::fmt::Display for MindLaneFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Theme colors used by Mind rendering (mirrors MissionTheme subset).
#[derive(Clone, Copy)]
pub struct MindTheme {
    pub muted: Color,
    pub info: Color,
    pub accent: Color,
    pub warn: Color,
    pub ok: Color,
    pub critical: Color,
    pub text: Color,
    pub title: Color,
}

/// Row in the Mind observer feed.
#[derive(Clone, Debug)]
pub struct MindObserverRow {
    pub agent_id: String,
    pub scope: String,
    pub pane_id: String,
    pub tab_scope: Option<String>,
    pub tab_focused: bool,
    pub event: MindObserverFeedEvent,
    pub source: String,
}

/// Row in the Mind injection feed.
#[derive(Clone, Debug)]
pub struct MindInjectionRow {
    pub scope: String,
    pub pane_id: String,
    pub tab_focused: bool,
    pub payload: MindInjectionPayload,
}

/// Rollup counts across Mind observer rows.
#[derive(Default)]
pub struct MindStatusRollup {
    pub queued: usize,
    pub running: usize,
    pub success: usize,
    pub fallback: usize,
    pub error: usize,
}

// --- Lane classification ---

pub fn mind_event_is_t3(event: &MindObserverFeedEvent) -> bool {
    event
        .runtime
        .as_deref()
        .map(|runtime| {
            let runtime = runtime.to_ascii_lowercase();
            runtime.contains("t3") || runtime.contains("backlog")
        })
        .unwrap_or(false)
        || event
            .reason
            .as_deref()
            .map(|reason| {
                let reason = reason.to_ascii_lowercase();
                reason.contains("t3") || reason.contains("backlog") || reason.contains("canon")
            })
            .unwrap_or(false)
}

pub fn mind_event_is_t2(event: &MindObserverFeedEvent) -> bool {
    event
        .runtime
        .as_deref()
        .map(|runtime| {
            let runtime = runtime.to_ascii_lowercase();
            runtime.contains("t2") || runtime.contains("reflector")
        })
        .unwrap_or(false)
        || event
            .reason
            .as_deref()
            .map(|reason| {
                let reason = reason.to_ascii_lowercase();
                reason.contains("t2") || reason.contains("reflector")
            })
            .unwrap_or(false)
}

pub fn mind_event_is_t0(event: &MindObserverFeedEvent) -> bool {
    if event.progress.is_some() && event.runtime.is_none() {
        return true;
    }
    event
        .reason
        .as_deref()
        .map(|reason| reason.to_ascii_lowercase().contains("t0"))
        .unwrap_or(false)
}

pub fn mind_event_lane(event: &MindObserverFeedEvent) -> MindLaneFilter {
    if mind_event_is_t3(event) {
        MindLaneFilter::T3
    } else if mind_event_is_t2(event) {
        MindLaneFilter::T2
    } else if mind_event_is_t0(event) {
        MindLaneFilter::T0
    } else {
        MindLaneFilter::T1
    }
}

pub fn mind_lane_matches(filter: MindLaneFilter, lane: MindLaneFilter) -> bool {
    match filter {
        MindLaneFilter::All => true,
        _ => filter == lane,
    }
}

pub fn mind_lane_label(lane: MindLaneFilter) -> &'static str {
    lane.label()
}

pub fn mind_lane_color(lane: MindLaneFilter, theme: MindTheme) -> Color {
    match lane {
        MindLaneFilter::T0 => theme.muted,
        MindLaneFilter::T1 => theme.info,
        MindLaneFilter::T2 => theme.accent,
        MindLaneFilter::T3 => theme.warn,
        MindLaneFilter::All => theme.text,
    }
}

// --- Status helpers ---

pub fn mind_status_rollup(rows: &[MindObserverRow]) -> MindStatusRollup {
    let mut rollup = MindStatusRollup::default();
    for row in rows {
        match row.event.status {
            MindObserverFeedStatus::Queued => rollup.queued += 1,
            MindObserverFeedStatus::Running => rollup.running += 1,
            MindObserverFeedStatus::Success => rollup.success += 1,
            MindObserverFeedStatus::Fallback => rollup.fallback += 1,
            MindObserverFeedStatus::Error => rollup.error += 1,
        }
    }
    rollup
}

pub fn mind_lane_rollup(rows: &[MindObserverRow]) -> [usize; 4] {
    let mut lanes = [0usize; 4];
    for row in rows {
        match mind_event_lane(&row.event) {
            MindLaneFilter::T0 => lanes[0] += 1,
            MindLaneFilter::T1 => lanes[1] += 1,
            MindLaneFilter::T2 => lanes[2] += 1,
            MindLaneFilter::T3 => lanes[3] += 1,
            MindLaneFilter::All => {}
        }
    }
    lanes
}

pub fn mind_status_label(status: MindObserverFeedStatus) -> &'static str {
    match status {
        MindObserverFeedStatus::Queued => "queued",
        MindObserverFeedStatus::Running => "running",
        MindObserverFeedStatus::Success => "success",
        MindObserverFeedStatus::Fallback => "fallback",
        MindObserverFeedStatus::Error => "error",
    }
}

pub fn mind_status_color(status: MindObserverFeedStatus, theme: MindTheme) -> Color {
    match status {
        MindObserverFeedStatus::Queued => theme.warn,
        MindObserverFeedStatus::Running => theme.info,
        MindObserverFeedStatus::Success => theme.ok,
        MindObserverFeedStatus::Fallback => theme.warn,
        MindObserverFeedStatus::Error => theme.critical,
    }
}

pub fn mind_trigger_label(trigger: MindObserverFeedTriggerKind) -> &'static str {
    match trigger {
        MindObserverFeedTriggerKind::TokenThreshold => "token",
        MindObserverFeedTriggerKind::TaskCompleted => "task",
        MindObserverFeedTriggerKind::ManualShortcut => "manual",
        MindObserverFeedTriggerKind::Handoff => "handoff",
        MindObserverFeedTriggerKind::Compaction => "compact",
    }
}

pub fn mind_progress_label(progress: &MindObserverFeedProgress) -> String {
    if progress.t1_target_tokens == 0 {
        return format!("t0:{}", progress.t0_estimated_tokens);
    }
    format!(
        "t0:{}/{} next:{}",
        progress.t0_estimated_tokens, progress.t1_target_tokens, progress.tokens_until_next_run
    )
}

pub fn mind_runtime_label(runtime: &str) -> String {
    if runtime.is_empty() {
        "runtime:n/a".to_string()
    } else {
        format!("runtime:{}", runtime)
    }
}

pub fn mind_timestamp_label(value: &str) -> Option<String> {
    // Parse ISO timestamp first — ISO timestamps contain ':' too, try them first
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(dt.format("%H:%M:%S").to_string());
    }
    if let Ok(dt) = value.parse::<chrono::NaiveDateTime>() {
        return Some(dt.format("%H:%M:%S").to_string());
    }
    if let Ok(dt) = value.parse::<chrono::DateTime<chrono::Utc>>() {
        return Some(dt.format("%H:%M:%S").to_string());
    }
    // If value looks like a relative time string already, return as-is
    if value.contains("ago") || value.starts_with('<') {
        return Some(value.to_string());
    }
    // Check for already-formatted time (HH:MM or HH:MM:SS, no date prefix)
    if value
        .chars()
        .all(|c| c.is_ascii_digit() || c == ':' || c == ' ')
    {
        let trimmed = value.trim();
        let colons = trimmed.chars().filter(|c| *c == ':').count();
        let digits = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
        if colons >= 1 && colons <= 2 && digits >= 4 && trimmed.len() <= 12 {
            return Some(trimmed.to_string());
        }
    }
    // Parse as ISO timestamp and format
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(dt.format("%H:%M:%S").to_string());
    }
    // Try date with space separator
    if let Ok(dt) = value.parse::<chrono::NaiveDateTime>() {
        return Some(dt.format("%H:%M:%S").to_string());
    }
    if let Ok(dt) = value.parse::<chrono::DateTime<chrono::Utc>>() {
        return Some(dt.format("%H:%M:%S").to_string());
    }
    None
}

pub fn mind_event_sort_ms(value: Option<&str>) -> Option<i64> {
    let v = value?;
    if let Some(label) = mind_timestamp_label(v) {
        if label.contains(':') {
            let parts: Vec<&str> = label.split(':').collect();
            if parts.len() == 3 {
                if let (Ok(h), Ok(m), Ok(s)) = (
                    parts[0].parse::<i64>(),
                    parts[1].parse::<i64>(),
                    parts[2].parse::<i64>(),
                ) {
                    return Some(h * 3600 + m * 60 + s);
                }
            }
        }
    }
    None
}

// --- Search ---

// collect_mind_search_hits lives in query.rs — use `aoc_mind::collect_mind_search_hits`

// --- Rendering ---

fn ellipsize(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

/// Render the header/status line of the Mind view.
pub fn render_mind_header_lines(
    lane_label: &str,
    scope_label: &str,
    project_label: &str,
    lane_rollup: [usize; 4],
    status_rollup: &MindStatusRollup,
    compact: bool,
    theme: MindTheme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let header = vec![
        Span::styled("lane:", Style::default().fg(theme.muted)),
        Span::styled(
            lane_label.to_string(),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("scope:", Style::default().fg(theme.muted)),
        Span::styled(
            scope_label.to_string(),
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
    ]));

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

    lines
}

/// Render observer activity rows.
pub fn render_mind_observer_rows(
    rows: &[MindObserverRow],
    theme: MindTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "No observer activity yet for current lane/scope.",
            Style::default().fg(theme.muted),
        )));
        lines.push(Line::from(Span::styled(
            "Try: o (T1), O (T1->T2 chain), b (bootstrap dry-run), t/v (filters).",
            Style::default().fg(theme.muted),
        )));
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
            .or_else(|| {
                row.event
                    .conversation_id
                    .clone()
                    .map(|id| format!("conv:{id}"))
            })
            .unwrap_or_else(|| {
                format!(
                    "source:{} tab:{} agent:{}",
                    row.source,
                    row.tab_scope
                        .as_deref()
                        .filter(|v| !v.trim().is_empty())
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

    lines
}

/// Render the injection rollup line.
pub fn render_mind_injection_rollup_line(
    rows: &[MindInjectionRow],
    theme: MindTheme,
    _compact: bool,
) -> Option<Line<'static>> {
    if rows.is_empty() {
        return None;
    }
    let total = rows.len();
    let status_groups: std::collections::HashMap<&str, usize> = rows
        .iter()
        .map(|r| r.payload.status.as_str())
        .fold(std::collections::HashMap::new(), |mut acc, s| {
            *acc.entry(s).or_insert(0) += 1;
            acc
        });
    let parts: Vec<Span<'static>> = vec![
        Span::styled("injections:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("{total}"),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
    ]
    .into_iter()
    .chain(status_groups.into_iter().flat_map(|(status, count)| {
        let color = mind_injection_status_color(status, theme);
        vec![
            Span::raw(" "),
            Span::styled(format!("{status}:{count}"), Style::default().fg(color)),
        ]
    }))
    .collect();
    Some(Line::from(parts))
}

fn mind_injection_status_color(status: &str, theme: MindTheme) -> Color {
    match status {
        "queued" => theme.warn,
        "running" => theme.info,
        "success" => theme.ok,
        "error" => theme.critical,
        _ => theme.muted,
    }
}

/// Render search results.
pub fn render_mind_search_lines(
    snapshot: &MindArtifactDrilldown,
    query: &str,
    is_editing: bool,
    selected: usize,
    theme: MindTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let prompt = if is_editing { "/search>" } else { "search:" };
    let query_display = if query.is_empty() && !is_editing {
        "press / to search artifacts"
    } else {
        query
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{prompt} "),
            Style::default()
                .fg(if is_editing {
                    theme.accent
                } else {
                    theme.muted
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            ellipsize(query_display, if compact { 60 } else { 100 }),
            Style::default().fg(if query.is_empty() {
                theme.muted
            } else {
                theme.text
            }),
        ),
    ]));

    if query.is_empty() {
        return lines;
    }

    let hits = crate::query::collect_mind_search_hits(snapshot, query);
    if hits.is_empty() {
        lines.push(Line::from(Span::styled(
            "no results",
            Style::default().fg(theme.muted),
        )));
        return lines;
    }

    let display_count = if compact { 4 } else { 8 };
    let max = display_count.min(hits.len());
    lines.push(Line::from(vec![Span::styled(
        format!("{} results:", hits.len()),
        Style::default().fg(theme.muted),
    )]));

    for (i, hit) in hits.iter().enumerate().take(max) {
        let prefix = if i == selected { " ▶ " } else { "   " };
        let kind_style = match hit.kind {
            "handshake" => Style::default().fg(theme.info),
            "canon" => Style::default().fg(theme.accent),
            _ => Style::default().fg(theme.muted),
        };
        lines.push(Line::from(vec![
            Span::styled(
                prefix,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("[{}] ", hit.kind),
                kind_style.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                ellipsize(&hit.title, if compact { 40 } else { 80 }),
                Style::default().fg(theme.text),
            ),
        ]));
    }
    if hits.len() > display_count {
        lines.push(Line::from(Span::styled(
            format!("  ... and {} more", hits.len() - display_count),
            Style::default().fg(theme.muted),
        )));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> MindTheme {
        MindTheme {
            muted: Color::DarkGray,
            info: Color::Blue,
            accent: Color::Yellow,
            warn: Color::Magenta,
            ok: Color::Green,
            critical: Color::Red,
            text: Color::White,
            title: Color::Cyan,
        }
    }

    fn make_event(
        status: MindObserverFeedStatus,
        runtime: Option<String>,
    ) -> MindObserverFeedEvent {
        MindObserverFeedEvent {
            status,
            trigger: MindObserverFeedTriggerKind::TokenThreshold,
            conversation_id: None,
            runtime,
            attempt_count: None,
            latency_ms: None,
            reason: None,
            failure_kind: None,
            enqueued_at: None,
            started_at: None,
            completed_at: None,
            progress: None,
        }
    }

    #[test]
    fn mind_lane_filter_cycles() {
        assert_eq!(MindLaneFilter::T0.next(), MindLaneFilter::T1);
        assert_eq!(MindLaneFilter::T1.next(), MindLaneFilter::T2);
        assert_eq!(MindLaneFilter::T2.next(), MindLaneFilter::T3);
        assert_eq!(MindLaneFilter::T3.next(), MindLaneFilter::All);
        assert_eq!(MindLaneFilter::All.next(), MindLaneFilter::T0);
    }

    #[test]
    fn mind_event_is_t3_detects_t3() {
        let event = make_event(
            MindObserverFeedStatus::Success,
            Some("t3_worker".to_string()),
        );
        assert!(mind_event_is_t3(&event));
        assert_eq!(mind_event_lane(&event), MindLaneFilter::T3);
    }

    #[test]
    fn mind_event_is_t2_detects_t2() {
        let event = make_event(
            MindObserverFeedStatus::Running,
            Some("reflector".to_string()),
        );
        assert!(mind_event_is_t2(&event));
        assert_eq!(mind_event_lane(&event), MindLaneFilter::T2);
    }

    #[test]
    fn mind_status_rollup_counts() {
        let theme = test_theme();
        let ev_queued = make_event(MindObserverFeedStatus::Queued, None);
        let ev_ok = make_event(MindObserverFeedStatus::Success, None);
        let ev_err = make_event(MindObserverFeedStatus::Error, None);
        let rows = vec![
            MindObserverRow {
                agent_id: "a1".into(),
                scope: "s".into(),
                pane_id: "1".into(),
                tab_scope: None,
                tab_focused: false,
                event: ev_queued,
                source: "".into(),
            },
            MindObserverRow {
                agent_id: "a2".into(),
                scope: "s".into(),
                pane_id: "2".into(),
                tab_scope: None,
                tab_focused: true,
                event: ev_ok,
                source: "".into(),
            },
            MindObserverRow {
                agent_id: "a3".into(),
                scope: "s".into(),
                pane_id: "3".into(),
                tab_scope: None,
                tab_focused: false,
                event: ev_err,
                source: "".into(),
            },
        ];
        let rollup = mind_status_rollup(&rows);
        assert_eq!(rollup.queued, 1);
        assert_eq!(rollup.success, 1);
        assert_eq!(rollup.error, 1);

        let _lines = render_mind_header_lines(
            "t1",
            "active-tab",
            "test-project",
            mind_lane_rollup(&rows),
            &rollup,
            false,
            theme,
        );
    }

    #[test]
    fn mind_lane_matches_filters() {
        assert!(mind_lane_matches(MindLaneFilter::All, MindLaneFilter::T0));
        assert!(mind_lane_matches(MindLaneFilter::T1, MindLaneFilter::T1));
        assert!(!mind_lane_matches(MindLaneFilter::T2, MindLaneFilter::T1));
    }
}

// --- Insight Detached Rendering ---
// Extracted from main.rs for reuse across Mission Control and Mind panes.

use aoc_core::insight_contracts::{
    InsightDetachedJob, InsightDetachedJobStatus, InsightDetachedOwnerPlane,
    InsightDetachedWorkerKind,
};
use chrono::TimeZone;

pub fn detached_job_status_label(status: InsightDetachedJobStatus) -> &'static str {
    match status {
        InsightDetachedJobStatus::Queued => "queued",
        InsightDetachedJobStatus::Running => "running",
        InsightDetachedJobStatus::Success => "success",
        InsightDetachedJobStatus::Fallback => "fallback",
        InsightDetachedJobStatus::Error => "error",
        InsightDetachedJobStatus::Cancelled => "cancelled",
        InsightDetachedJobStatus::Stale => "stale",
    }
}

pub fn detached_job_status_color(status: InsightDetachedJobStatus, theme: MindTheme) -> Color {
    match status {
        InsightDetachedJobStatus::Queued => theme.warn,
        InsightDetachedJobStatus::Running => theme.info,
        InsightDetachedJobStatus::Success => theme.ok,
        InsightDetachedJobStatus::Fallback => theme.warn,
        InsightDetachedJobStatus::Error => theme.critical,
        InsightDetachedJobStatus::Cancelled | InsightDetachedJobStatus::Stale => theme.muted,
    }
}

pub fn detached_owner_plane_label(owner_plane: InsightDetachedOwnerPlane) -> &'static str {
    match owner_plane {
        InsightDetachedOwnerPlane::Delegated => "delegated",
        InsightDetachedOwnerPlane::Mind => "mind",
    }
}

pub fn detached_worker_kind_label(worker_kind: Option<InsightDetachedWorkerKind>) -> &'static str {
    match worker_kind {
        Some(InsightDetachedWorkerKind::Specialist) => "specialist",
        Some(InsightDetachedWorkerKind::ChainStep) => "chain",
        Some(InsightDetachedWorkerKind::TeamFanout) => "fanout",
        Some(InsightDetachedWorkerKind::T1) => "t1",
        Some(InsightDetachedWorkerKind::T2) => "t2",
        Some(InsightDetachedWorkerKind::T3) => "t3",
        None => "unknown",
    }
}

pub fn detached_worker_kind_display(
    owner_plane: InsightDetachedOwnerPlane,
    worker_kind: Option<InsightDetachedWorkerKind>,
) -> &'static str {
    match (owner_plane, worker_kind) {
        (InsightDetachedOwnerPlane::Mind, Some(InsightDetachedWorkerKind::T2)) => "t2-reflector",
        (InsightDetachedOwnerPlane::Mind, Some(InsightDetachedWorkerKind::T3)) => "t3-runtime",
        _ => detached_worker_kind_label(worker_kind),
    }
}

pub fn detached_job_attention_label(job: &InsightDetachedJob) -> Option<String> {
    match job.status {
        InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running => {
            if job.fallback_used {
                Some("fallback-used".to_string())
            } else {
                None
            }
        }
        InsightDetachedJobStatus::Success => {
            if job.fallback_used {
                Some("fallback-used".to_string())
            } else {
                None
            }
        }
        InsightDetachedJobStatus::Fallback => Some(
            match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => "inline-fallback",
                InsightDetachedOwnerPlane::Delegated => "fallback",
            }
            .to_string(),
        ),
        InsightDetachedJobStatus::Error => Some("error".to_string()),
        InsightDetachedJobStatus::Cancelled => Some("cancelled".to_string()),
        InsightDetachedJobStatus::Stale => Some(
            match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => "lease-lost",
                InsightDetachedOwnerPlane::Delegated => "stale",
            }
            .to_string(),
        ),
    }
}

/// Render the insight detached job rollup line.
/// Uses a generic theme accessor pattern to decouple from MissionTheme.
pub fn render_insight_detached_rollup_line(
    jobs: &[InsightDetachedJob],
    theme: MindTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let latest = jobs.first()?;
    let mut queued = 0usize;
    let mut running = 0usize;
    let mut success = 0usize;
    let mut fallback = 0usize;
    let mut error = 0usize;
    let mut cancelled = 0usize;
    let mut stale = 0usize;
    for job in jobs {
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
    let delegated = jobs
        .iter()
        .filter(|job| matches!(job.owner_plane, InsightDetachedOwnerPlane::Delegated))
        .count();
    let mind = jobs.len().saturating_sub(delegated);
    let label = latest
        .agent
        .as_deref()
        .or(latest.chain.as_deref())
        .or(latest.team.as_deref())
        .unwrap_or("detached-job");
    let when = latest
        .finished_at_ms
        .or(latest.started_at_ms)
        .unwrap_or(latest.created_at_ms);
    let when = chrono::Utc
        .timestamp_millis_opt(when)
        .single()
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".to_string());
    let mut detail_fields = vec![
        format!("q:{queued}"),
        format!("run:{running}"),
        format!("ok:{success}"),
        format!("fb:{fallback}"),
        format!("err:{error}"),
    ];
    if cancelled > 0 {
        detail_fields.push(format!("cx:{cancelled}"));
    }
    if stale > 0 {
        detail_fields.push(format!("stale:{stale}"));
    }
    detail_fields.push(format!("pl:d{}|m{}", delegated, mind));
    detail_fields.push(format!(
        "kind:{}",
        detached_worker_kind_display(latest.owner_plane, latest.worker_kind)
    ));
    if let Some(step_count) = latest.step_count {
        detail_fields.push(format!("steps:{step_count}"));
    }
    let detail = fit_fields(&detail_fields, if compact { 42 } else { 76 });
    let summary = latest
        .output_excerpt
        .as_deref()
        .or(latest.error.as_deref())
        .map(|value| ellipsize(value, if compact { 38 } else { 72 }))
        .unwrap_or_else(|| "detached runtime idle".to_string());
    Some(Line::from(vec![
        Span::styled("subagents:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", detached_job_status_label(latest.status)),
            Style::default()
                .fg(detached_job_status_color(latest.status, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "{}:{}:{}",
                detached_owner_plane_label(latest.owner_plane),
                detached_worker_kind_display(latest.owner_plane, latest.worker_kind),
                ellipsize(label, if compact { 14 } else { 20 })
            ),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(detail, Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::styled(format!("@{when}"), Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(summary, Style::default().fg(theme.muted)),
    ]))
}

// NOTE: fit_fields and ellipsize are internal helpers defined above in the render module.

fn fit_fields(fields: &[String], max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut output = String::new();
    for field in fields {
        if field.trim().is_empty() {
            continue;
        }
        let candidate = if output.is_empty() {
            field.clone()
        } else {
            format!("{output} | {field}")
        };
        if candidate.chars().count() <= max {
            output = candidate;
            continue;
        }
        if output.is_empty() {
            return ellipsize(field, max);
        }
        break;
    }
    output
}

pub fn detached_job_attention_color(job: &InsightDetachedJob, theme: MindTheme) -> Color {
    match job.status {
        InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running => {
            if job.fallback_used {
                theme.warn
            } else {
                theme.muted
            }
        }
        InsightDetachedJobStatus::Success => {
            if job.fallback_used {
                theme.warn
            } else {
                theme.muted
            }
        }
        InsightDetachedJobStatus::Fallback => theme.warn,
        InsightDetachedJobStatus::Error => theme.critical,
        InsightDetachedJobStatus::Cancelled | InsightDetachedJobStatus::Stale => theme.muted,
    }
}

pub fn detached_job_recovery_guidance(job: &InsightDetachedJob) -> Vec<String> {
    let mut steps = Vec::new();
    let kind = detached_worker_kind_display(job.owner_plane, job.worker_kind);
    match job.status {
        InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running => {
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => format!(
                    "Mind {kind} is active under the detached coordinator; wait for completion or cancel with x if the run is no longer useful"
                ),
                InsightDetachedOwnerPlane::Delegated => {
                    "job is active; wait, inspect, or cancel with x if it is no longer useful".to_string()
                }
            });
        }
        InsightDetachedJobStatus::Success => {
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => format!(
                    "inspect the {kind} result in Fleet/Mind before treating it as the latest project-scoped synthesis"
                ),
                InsightDetachedOwnerPlane::Delegated => {
                    "inspect or handoff the selected result into a follow-up tab if operator review is needed".to_string()
                }
            });
        }
        InsightDetachedJobStatus::Fallback => {
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => format!(
                    "Mind {kind} completed via degraded inline fallback; inspect the summary/error before trusting the result"
                ),
                InsightDetachedOwnerPlane::Delegated => {
                    "job completed with degraded execution; inspect the brief/error before trusting the result".to_string()
                }
            });
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => {
                    "if the result is insufficient, rerun the upstream workflow that feeds this Mind worker rather than assuming detached coordination is healthy".to_string()
                }
                InsightDetachedOwnerPlane::Delegated => {
                    "if the result is insufficient, rerun the specialist from the owning Pi session with a narrower prompt".to_string()
                }
            });
        }
        InsightDetachedJobStatus::Error => {
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => format!(
                    "Mind {kind} failed; inspect summary/error context, then compare against adjacent Mind jobs in the same project group"
                ),
                InsightDetachedOwnerPlane::Delegated => {
                    "job failed; inspect stderr/error context and rerun from the owning Pi session after correcting scope or environment".to_string()
                }
            });
            steps.push("compare against other recent jobs in this group to see whether the failure is isolated or systemic".to_string());
        }
        InsightDetachedJobStatus::Cancelled => {
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => format!(
                    "Mind {kind} was cancelled; confirm whether upstream queue pressure still warrants a rerun before restarting work"
                ),
                InsightDetachedOwnerPlane::Delegated => {
                    "job was cancelled; relaunch only if the work is still needed".to_string()
                }
            });
        }
        InsightDetachedJobStatus::Stale => {
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => format!(
                    "Mind {kind} lost lease or restart continuity; treat it as interrupted, not successful"
                ),
                InsightDetachedOwnerPlane::Delegated => {
                    "job lost live ownership or wrapper continuity; treat it as interrupted, not successful".to_string()
                }
            });
            steps.push(match job.owner_plane {
                InsightDetachedOwnerPlane::Mind => {
                    "inspect any partial output, then verify coordinator health before rerunning the upstream workflow".to_string()
                }
                InsightDetachedOwnerPlane::Delegated => {
                    "inspect any partial output, then rerun from the owning session if you still need a complete result".to_string()
                }
            });
        }
    }
    if job.fallback_used
        && !matches!(
            job.status,
            InsightDetachedJobStatus::Fallback | InsightDetachedJobStatus::Stale
        )
    {
        steps.push("fallback behavior was recorded; verify the result before using it as authoritative evidence".to_string());
    }
    steps
}

// --- Shared render utilities ---

use chrono::DateTime;

pub fn format_age(age: Option<i64>) -> String {
    age.map(|value| format!("{value}s"))
        .unwrap_or_else(|| "n/a".to_string())
}

pub fn age_meter(age: Option<i64>, online: bool) -> &'static str {
    const HUB_STALE_SECS: i64 = 45;
    if !online {
        return "!!!!!";
    }
    match age {
        Some(secs) if secs <= 8 => "|||||",
        Some(secs) if secs <= 20 => "||||.",
        Some(secs) if secs <= HUB_STALE_SECS => "|||..",
        Some(_) => "!!...",
        None => ".....",
    }
}

pub fn age_color(age: Option<i64>, online: bool, theme: MindTheme) -> Color {
    const HUB_STALE_SECS: i64 = 45;
    if !online {
        return theme.critical;
    }
    match age {
        Some(secs) if secs <= 20 => theme.ok,
        Some(secs) if secs <= HUB_STALE_SECS => theme.warn,
        Some(_) => theme.critical,
        None => theme.muted,
    }
}

pub fn normalize_lifecycle(raw: &str) -> String {
    let normalized = raw.trim().to_ascii_lowercase().replace('_', "-");
    if normalized.is_empty() {
        "running".to_string()
    } else {
        normalized
    }
}

pub fn lifecycle_color(lifecycle: &str, online: bool, theme: MindTheme) -> Color {
    if !online {
        return theme.critical;
    }
    match normalize_lifecycle(lifecycle).as_str() {
        "error" => theme.critical,
        "needs_input" | "blocked" => theme.warn,
        "busy" => theme.info,
        "idle" => theme.muted,
        _ => theme.ok,
    }
}

pub fn ms_to_datetime(value: i64) -> Option<DateTime<chrono::Utc>> {
    chrono::Utc.timestamp_millis_opt(value).single()
}
