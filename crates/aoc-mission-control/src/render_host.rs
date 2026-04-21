//! Render orchestration host helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn render_ui(frame: &mut ratatui::Frame, app: &App) {
    let size = frame.size();
    let theme = app
        .config
        .mission_custom_theme
        .unwrap_or_else(|| mission_theme(app.config.mission_theme));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(size);
    if app.mode == Mode::Overview && app.config.overview_enabled {
        render_overview_panel(frame, app, theme, layout[0]);
    } else {
        frame.render_widget(render_body(app, theme, size.width), layout[0]);
    }
    if app.help_open {
        render_help_overlay(frame, app, theme);
    }
}

fn render_body(app: &App, theme: MissionTheme, width: u16) -> Paragraph<'static> {
    let compact = is_compact(width);
    let lines = match app.mode {
        Mode::Overview => Vec::new(),
        Mode::Overseer => render_overseer_lines(app, theme, compact),
        Mode::Mind => render_mind_lines(app, theme, compact),
        Mode::Fleet => render_fleet_lines(app, theme, compact),
        Mode::Work => render_work_lines(app, theme, compact),
        Mode::Diff => render_diff_lines(app, theme, compact, width),
        Mode::Health => render_health_lines(app, theme, compact),
    };
    let panel_title = if app.mode == Mode::Mind {
        "✦ Mind / Insight".to_string()
    } else if app.mode == Mode::Fleet {
        "Detached Fleet".to_string()
    } else if app.mode == Mode::Overseer {
        "Session Overseer".to_string()
    } else {
        app.mode.title().to_string()
    };
    Paragraph::new(Text::from(lines))
        .style(Style::default().fg(theme.text).bg(theme.surface))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.surface))
                .title(Span::styled(
                    panel_title,
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .scroll((app.scroll, 0))
}

fn render_help_overlay(frame: &mut ratatui::Frame, app: &App, theme: MissionTheme) {
    let area = centered_rect(78, 72, frame.size());
    let source = app.mode_source();
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "Controls",
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "mode:{} src:{}",
                    app.mode.title().to_ascii_lowercase(),
                    source
                ),
                Style::default().fg(theme.muted),
            ),
        ]),
        Line::from(Span::styled(
            "Mission Control Navigation",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(if app.config.overview_enabled {
            "  1/2/3/4/5/6/7 switch mode (Overview/Overseer/Mind/Fleet/Work/Diff/Health)"
        } else {
            "  2/3/4/5/6/7 switch mode (Overseer/Mind/Fleet/Work/Diff/Health)"
        }),
        Line::from("  Tab      cycle mode"),
        Line::from("  r        refresh local snapshot"),
        Line::from(""),
    ];
    lines.extend(mode_help_lines(app, theme));
    lines.extend([
        Line::from(""),
        Line::from(Span::styled(
            "Session & Exit",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ? or F1  toggle this help"),
        Line::from("  Esc      close help"),
        Line::from("  q        quit"),
    ]);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().fg(theme.text).bg(theme.surface))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
                    .style(Style::default().bg(theme.surface))
                    .title(Span::styled(
                        "Help",
                        Style::default()
                            .fg(theme.title)
                            .add_modifier(Modifier::BOLD),
                    )),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn mode_help_lines(app: &App, theme: MissionTheme) -> Vec<Line<'static>> {
    match app.mode {
        Mode::Overview => {
            if !app.config.overview_enabled {
                return vec![
                    Line::from(Span::styled(
                        "Overview Deprecated",
                        Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
                    )),
                    Line::from("  Overview display and local polling are disabled."),
                    Line::from(
                        "  Use Overseer/Mind/Work/Diff/Health modes for current operations.",
                    ),
                ];
            }
            vec![
                Line::from(Span::styled(
                    "Overview Mode",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from("  j/k      select agent row (>> + reverse)"),
                Line::from("  g        jump to first agent"),
                Line::from("  Enter    focus selected tab; unmapped -> pane note"),
                Line::from("  e        capture selected pane evidence"),
                Line::from("  E        open live pane follow"),
                Line::from("  x        request stop selected agent"),
                Line::from("  a        toggle sort (layout/attention)"),
                Line::from("  o        request manual observer run"),
            ]
        }
        Mode::Overseer => vec![
            Line::from(Span::styled(
                "Session Overseer Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll overseer snapshot and timeline"),
            Line::from("  g        jump to top"),
            Line::from("  Enter    focus selected worker tab"),
            Line::from("  e        capture selected worker pane evidence"),
            Line::from("  E        open live pane follow"),
            Line::from("  x        stop selected worker"),
            Line::from("  c        request peer review for selected worker"),
            Line::from("  u        request peer unblock/help for selected worker"),
            Line::from("  s        spawn a fresh worker tab"),
            Line::from("  d        delegate selected worker into a new tab + brief"),
            Line::from("  o        request fresh observer run for selected worker"),
            Line::from("  r        refresh local snapshot while hub catches up"),
        ],
        Mode::Mind => vec![
            Line::from(Span::styled(
                "✦ Mind/Insight Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll observer timeline"),
            Line::from("  g        jump to top"),
            Line::from("  e        capture focused pane evidence"),
            Line::from("  E        open live pane follow"),
            Line::from("  o        request manual observer run (T1)"),
            Line::from("  O        run insight_dispatch chain (T1->T2)"),
            Line::from("  b / B    bootstrap dry-run / seed enqueue"),
            Line::from("  F        force finalize session"),
            Line::from("  C        rebuild/requeue latest compaction checkpoint"),
            Line::from("  R        requeue latest T3 export slice"),
            Line::from("  H        rebuild handshake baseline"),
            Line::from("  /        edit local project Mind search query"),
            Line::from("  n / N    browse search results next / previous"),
            Line::from("  t        toggle lane (t0/t1/t2/t3/all)"),
            Line::from("  v        toggle scope (active tab/all tabs)"),
            Line::from("  p        toggle provenance drilldown"),
        ],
        Mode::Fleet => vec![
            Line::from(Span::styled(
                "Detached Fleet Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      select fleet group"),
            Line::from("  Left/Right or [/]  select job within the group"),
            Line::from("  g        jump to top of groups + jobs"),
            Line::from("  Enter    focus a live tab for the selected project"),
            Line::from("  i        launch inspect follow-up tab + brief"),
            Line::from("  h        launch handoff follow-up tab + brief"),
            Line::from("  x        cancel selected active detached job"),
            Line::from("  f        toggle plane filter (all/delegated/mind)"),
            Line::from("  S        toggle sort (project/newest/active-first/error-first)"),
            Line::from("  A        toggle active-only groups"),
            Line::from("  grouped  by project root and ownership plane"),
            Line::from("  lower    drilldown shows selected group details + recent jobs"),
        ],
        Mode::Work => vec![
            Line::from(Span::styled(
                "Work Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll work summary"),
            Line::from("  g        jump to top"),
        ],
        Mode::Diff => vec![
            Line::from(Span::styled(
                "Diff Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll diff summary"),
            Line::from("  g        jump to top"),
        ],
        Mode::Health => vec![
            Line::from(Span::styled(
                "Health Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll dependency checks"),
            Line::from("  g        jump to top"),
        ],
    }
}
