//! Overview surface rendering and row presentation helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn render_overview_panel(
    frame: &mut ratatui::Frame,
    app: &App,
    theme: MissionTheme,
    area: Rect,
) {
    let rows = app.overview_rows();
    let compact = is_compact(area.width);
    let mut state = ListState::default();
    if !rows.is_empty() {
        state.select(Some(
            app.selected_overview.min(rows.len().saturating_sub(1)),
        ));
    }

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| {
            let line = Line::from(overview_row_spans(row, app, theme, compact, area.width));
            ListItem::new(line)
        })
        .collect();

    let title = if app.config.overview_enabled {
        "Overview"
    } else {
        "Overview (disabled)"
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.surface))
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .style(Style::default().fg(theme.text))
        .highlight_style(
            Style::default()
                .bg(theme.surface)
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("» ");
    frame.render_stateful_widget(list, area, &mut state);
}

pub(crate) fn overview_row_spans(
    row: &OverviewRow,
    app: &App,
    theme: MissionTheme,
    compact: bool,
    width: u16,
) -> Vec<Span<'static>> {
    let decorations = OverviewDecorations {
        attention_chip: attention_chip_from_row(row),
        context: app.overview_context_hint(row),
        task_signal: app.overview_task_signal(row),
        git_signal: app.overview_git_signal(row),
    };
    let presenter = overview_row_presenter(row, &decorations, compact, width);
    let lifecycle_color = lifecycle_color(&row.lifecycle, row.online, theme);
    let age_color = age_color(row.age_secs, row.online, theme);
    let badge_color = overview_badge_color(presenter.badge, theme);
    let mut spans = vec![
        Span::styled(
            presenter.badge.bracketed(),
            Style::default()
                .fg(badge_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            presenter.identity,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            presenter.lifecycle_chip,
            Style::default()
                .fg(lifecycle_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            presenter.location_chip,
            Style::default().fg(if row.tab_focused {
                theme.accent
            } else {
                theme.muted
            }),
        ),
        Span::raw(" "),
        Span::styled(presenter.freshness, Style::default().fg(age_color)),
    ];
    if let Some(task_signal) = presenter.task_signal {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(task_signal, Style::default().fg(theme.info)));
    }
    if let Some(git_signal) = presenter.git_signal {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            git_signal.clone(),
            Style::default().fg(if git_signal.contains('+') || git_signal.contains('?') {
                theme.warn
            } else {
                theme.muted
            }),
        ));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        presenter.context,
        Style::default().fg(theme.muted),
    ));
    spans
}

#[derive(Clone, Debug)]
pub(crate) struct OverviewDecorations {
    pub(crate) attention_chip: AttentionChip,
    pub(crate) context: String,
    pub(crate) task_signal: Option<String>,
    pub(crate) git_signal: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttentionChip {
    Err,
    Needs,
    Blocked,
    Stale,
    Drift,
    Ok,
}

impl AttentionChip {
    pub(crate) fn label(self) -> &'static str {
        match self {
            AttentionChip::Err => "ERR",
            AttentionChip::Needs => "NEEDS",
            AttentionChip::Blocked => "BLOCK",
            AttentionChip::Stale => "STALE",
            AttentionChip::Drift => "DRIFT",
            AttentionChip::Ok => "OK",
        }
    }

    pub(crate) fn severity(self) -> u8 {
        match self {
            AttentionChip::Err => 0,
            AttentionChip::Needs => 1,
            AttentionChip::Blocked => 2,
            AttentionChip::Stale => 3,
            AttentionChip::Drift => 4,
            AttentionChip::Ok => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverviewBadge {
    Attention(AttentionChip),
}

impl OverviewBadge {
    pub(crate) fn bracketed(self) -> String {
        match self {
            OverviewBadge::Attention(chip) => format!("[{}]", chip.label()),
        }
    }
}

pub(crate) fn attention_chip_from_row(row: &OverviewRow) -> AttentionChip {
    if !row.online {
        return AttentionChip::Stale;
    }
    match normalize_lifecycle(&row.lifecycle).as_str() {
        "error" => AttentionChip::Err,
        "needs-input" => AttentionChip::Needs,
        "blocked" => AttentionChip::Blocked,
        _ => {
            if source_chip_from_row(&row.source) == SourceChip::Mixed {
                AttentionChip::Drift
            } else {
                AttentionChip::Ok
            }
        }
    }
}

pub(crate) fn attention_chip_color(chip: AttentionChip, theme: MissionTheme) -> Color {
    match chip {
        AttentionChip::Err => theme.critical,
        AttentionChip::Needs => theme.warn,
        AttentionChip::Blocked => theme.warn,
        AttentionChip::Stale => theme.critical,
        AttentionChip::Drift => theme.info,
        AttentionChip::Ok => theme.ok,
    }
}

pub(crate) fn overview_badge_color(chip: OverviewBadge, theme: MissionTheme) -> Color {
    match chip {
        OverviewBadge::Attention(value) => attention_chip_color(value, theme),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceChip {
    Hub,
    Local,
    Mixed,
}

#[derive(Clone, Debug)]
pub(crate) struct OverviewRowPresenter {
    pub(crate) identity: String,
    pub(crate) lifecycle_chip: String,
    pub(crate) location_chip: String,
    pub(crate) badge: OverviewBadge,
    pub(crate) freshness: String,
    pub(crate) context: String,
    pub(crate) task_signal: Option<String>,
    pub(crate) git_signal: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct PresenterBudgets {
    label: usize,
    pane: usize,
    tab_name: usize,
    context: usize,
    include_task_signal: bool,
    include_git_signal: bool,
    include_meter: bool,
}

pub(crate) fn overview_row_presenter(
    row: &OverviewRow,
    decorations: &OverviewDecorations,
    compact: bool,
    width: u16,
) -> OverviewRowPresenter {
    let mut plans = vec![PresenterBudgets {
        label: if compact { 14 } else { 20 },
        pane: if compact { 8 } else { 12 },
        tab_name: if compact { 8 } else { 14 },
        context: if compact { 20 } else { 28 },
        include_task_signal: true,
        include_git_signal: !compact,
        include_meter: !compact,
    }];
    plans.push(PresenterBudgets {
        include_git_signal: false,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        include_task_signal: false,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 6,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 0,
        context: 16,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 0,
        label: 12,
        context: 12,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 0,
        label: 8,
        pane: 6,
        context: 10,
        ..plans[0]
    });

    let max_width = width.saturating_sub(8) as usize;
    for plan in plans {
        let presenter = overview_row_presenter_with_budget(row, decorations, plan);
        if presenter_text_len(&presenter) <= max_width.max(28) {
            return presenter;
        }
    }
    overview_row_presenter_with_budget(
        row,
        decorations,
        PresenterBudgets {
            label: 8,
            pane: 6,
            tab_name: 0,
            context: 8,
            include_task_signal: false,
            include_git_signal: false,
            include_meter: false,
        },
    )
}

fn overview_row_presenter_with_budget(
    row: &OverviewRow,
    decorations: &OverviewDecorations,
    budget: PresenterBudgets,
) -> OverviewRowPresenter {
    let identity = format!(
        "{}::{}",
        ellipsize(&row.label, budget.label.max(4)),
        ellipsize(&row.pane_id, budget.pane.max(4))
    );
    let lifecycle_chip = format!("[{:<5}]", lifecycle_chip_label(&row.lifecycle, row.online));
    let location_chip = overview_location_chip(row, budget.tab_name);
    let badge = OverviewBadge::Attention(decorations.attention_chip);
    let freshness = if budget.include_meter {
        format!(
            "HB:{} {}",
            age_meter(row.age_secs, row.online),
            format_age(row.age_secs)
        )
    } else {
        format_age(row.age_secs)
    };
    let context = format!(
        "M:{}",
        ellipsize(&decorations.context, budget.context.max(8))
    );

    OverviewRowPresenter {
        identity,
        lifecycle_chip,
        location_chip,
        badge,
        freshness,
        context,
        task_signal: budget
            .include_task_signal
            .then(|| decorations.task_signal.clone())
            .flatten(),
        git_signal: budget
            .include_git_signal
            .then(|| decorations.git_signal.clone())
            .flatten(),
    }
}

pub(crate) fn overview_location_chip(row: &OverviewRow, tab_name_budget: usize) -> String {
    let focused_suffix = if row.tab_focused { "*" } else { "" };
    let tab_name = row
        .tab_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(tab_index) = row.tab_index else {
        if let Some(name) = tab_name {
            if tab_name_budget == 0 {
                return format!("T?{focused_suffix}");
            }
            return format!("T?:{}{}", ellipsize(name, tab_name_budget), focused_suffix);
        }
        return "T?:???".to_string();
    };
    if tab_name_budget == 0 {
        return format!("T{tab_index}{focused_suffix}");
    }
    if let Some(tab_name) = tab_name {
        return format!(
            "T{tab_index}:{}{}",
            ellipsize(tab_name, tab_name_budget),
            focused_suffix
        );
    }
    format!("T{tab_index}:???{focused_suffix}")
}

pub(crate) fn lifecycle_chip_label(lifecycle: &str, online: bool) -> &'static str {
    if !online {
        return "OFF";
    }
    match normalize_lifecycle(lifecycle).as_str() {
        "error" => "ERR",
        "blocked" => "BLOCK",
        "needs-input" => "NEEDS",
        "busy" => "BUSY",
        "idle" => "IDLE",
        _ => "RUN",
    }
}

fn source_chip_from_row(source: &str) -> SourceChip {
    let normalized = source.trim().to_ascii_lowercase();
    if normalized == "hub" {
        return SourceChip::Hub;
    }
    if normalized.contains("hub+")
        || normalized.contains("+hub")
        || normalized == "mix"
        || normalized.starts_with("mix+")
    {
        return SourceChip::Mixed;
    }
    SourceChip::Local
}

pub(crate) fn presenter_text_len(presenter: &OverviewRowPresenter) -> usize {
    let mut len = presenter.badge.bracketed().chars().count()
        + 1
        + presenter.identity.chars().count()
        + 1
        + presenter.lifecycle_chip.chars().count()
        + 1
        + presenter.location_chip.chars().count()
        + 1
        + presenter.freshness.chars().count()
        + 1
        + presenter.context.chars().count();
    if let Some(task_signal) = presenter.task_signal.as_ref() {
        len += 1 + task_signal.chars().count();
    }
    if let Some(git_signal) = presenter.git_signal.as_ref() {
        len += 1 + git_signal.chars().count();
    }
    len
}
