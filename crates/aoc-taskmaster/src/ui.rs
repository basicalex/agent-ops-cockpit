use crate::state::{App, FocusMode, ALL_TAG_VIEW};
use crate::theme::{self, icons};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Wrap},
    Frame,
};
use std::collections::HashMap;

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.size();

    if app.show_help || app.show_detail || app.show_tag_selector {
        let main = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        app.update_layout(main[0], Some(main[1]));
        render_main(f, app, main[0]);

        if app.show_tag_selector {
            render_tag_selector(f, app, main[1]);
        } else if app.show_help {
            render_help(f, main[1]);
        } else {
            render_details(f, app, main[1]);
        }
    } else {
        app.update_layout(area, None);
        render_main(f, app, area);
    }
}

fn render_help(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Help")
        .border_style(Style::default().fg(Color::Yellow));
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("j / Down", Color::Cyan),
            Span::raw("   Next task"),
        ]),
        Line::from(vec![
            Span::styled("k / Up", Color::Cyan),
            Span::raw("     Previous task"),
        ]),
        Line::from(vec![
            Span::styled("x", Color::Cyan),
            Span::raw("          Toggle done"),
        ]),
        Line::from(vec![
            Span::styled("a", Color::Cyan),
            Span::raw("          Toggle active agent"),
        ]),
        Line::from(vec![
            Span::styled("Space", Color::Cyan),
            Span::raw("      Expand/collapse subtasks"),
        ]),
        Line::from(vec![
            Span::styled("Enter", Color::Cyan),
            Span::raw("      Toggle details pane"),
        ]),
        Line::from(vec![
            Span::styled("Tab", Color::Cyan),
            Span::raw("        Switch focus"),
        ]),
        Line::from(vec![
            Span::styled("r", Color::Cyan),
            Span::raw("          Refresh"),
        ]),
        Line::from(vec![
            Span::styled("f", Color::Cyan),
            Span::raw("          Cycle filter"),
        ]),
        Line::from(vec![
            Span::styled("s", Color::Cyan),
            Span::raw("          Cycle sort (task#/tag)"),
        ]),
        Line::from(vec![
            Span::styled("t", Color::Cyan),
            Span::raw("          Cycle tag (includes all)"),
        ]),
        Line::from(vec![
            Span::styled("T", Color::Cyan),
            Span::raw("          Tag selector"),
        ]),
        Line::from(vec![
            Span::styled("?", Color::Cyan),
            Span::raw("          Toggle help"),
        ]),
        Line::from(vec![
            Span::styled("q", Color::Cyan),
            Span::raw("          Quit"),
        ]),
    ];

    let p = Paragraph::new(text).wrap(Wrap { trim: true });
    f.render_widget(p, inner_area);
}

fn render_main(f: &mut Frame, app: &mut App, area: Rect) {
    if app.display_rows.is_empty() {
        let border_style = if app.focus == FocusMode::List {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let message = if let Some(error) = &app.last_error {
            error.clone()
        } else {
            "No tasks found".to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Tasks")
            .border_style(border_style);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let text = vec![
            Line::from(Span::styled(&message, Color::Yellow)),
            Line::from(""),
            Line::from(format!("root: {}", app.root.display())),
            Line::from(format!("tasks_path: {}", app.tasks_path.display())),
            Line::from(""),
            Line::from("Press r to retry, q to quit."),
        ];
        let p = Paragraph::new(text).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
        return;
    }

    let mut tag_tone_index: HashMap<String, usize> = HashMap::new();
    let mut next_tag_tone: usize = 0;
    let rows: Vec<Row> = app
        .display_rows
        .iter()
        .enumerate()
        .map(|(visual_idx, row)| {
            let task = &app.tasks[row.task_idx];

            let (title, status, priority, is_subtask) =
                if let Some(sub_idx) = row.subtask_path.first() {
                    if *sub_idx < task.subtasks.len() {
                        let sub = &task.subtasks[*sub_idx];
                        (
                            sub.title.clone(),
                            sub.status.clone(),
                            task.priority.clone(),
                            true,
                        )
                    } else {
                        (
                            task.title.clone(),
                            task.status.clone(),
                            task.priority.clone(),
                            false,
                        )
                    }
                } else {
                    (
                        task.title.clone(),
                        task.status.clone(),
                        task.priority.clone(),
                        false,
                    )
                };

            let status_str = status.as_str();
            let s_icon = match status_str {
                "done" => icons::CHECK,
                "in-progress" => icons::IN_PROGRESS,
                "blocked" => icons::BLOCKED,
                "review" => icons::REVIEW,
                "cancelled" => icons::BLOCKED,
                _ => icons::PENDING,
            };
            let s_color = theme::status_color(status_str);

            let prio_str = priority.as_str();
            let p_icon = if is_subtask {
                ""
            } else {
                match prio_str {
                    "high" => icons::PRIORITY_HIGH,
                    "low" => icons::PRIORITY_LOW,
                    _ => icons::PRIORITY_MED,
                }
            };
            let p_color = theme::priority_color(prio_str);

            let mut title_spans = Vec::new();
            let indent_width = row.depth * 2;
            if indent_width > 0 {
                title_spans.push(Span::raw(" ".repeat(indent_width)));
                title_spans.push(Span::raw("- "));
            }

            if !is_subtask && !task.subtasks.is_empty() {
                let icon = if app.expanded_tasks.contains(&task.id) {
                    icons::EXPANDED
                } else {
                    icons::COLLAPSED
                };
                title_spans.push(Span::styled(format!("{} ", icon), Color::Blue));
            }

            if !is_subtask && task.active_agent {
                title_spans.push(Span::styled(format!("{} ", icons::AGENT), Color::Magenta));
            }

            title_spans.push(Span::raw(title));

            let id_cell = if is_subtask { "" } else { task.id.as_str() };
            let tag_cell = if is_subtask {
                Span::raw("")
            } else {
                let tag_label = if row.tag_name == ALL_TAG_VIEW {
                    "all"
                } else {
                    row.tag_name.as_str()
                };
                Span::styled(
                    format!("[{}]", tag_label),
                    theme::tag_badge_style(tag_label, app.tag_color_seed),
                )
            };

            let row_style = if app.current_tag == ALL_TAG_VIEW {
                let tone = *tag_tone_index
                    .entry(row.tag_name.clone())
                    .or_insert_with(|| {
                        let assigned = next_tag_tone;
                        next_tag_tone += 1;
                        assigned
                    });
                theme::zebra_row_style(tone % 2)
            } else {
                theme::zebra_row_style(visual_idx)
            };

            Row::new(vec![
                Cell::from(Span::raw(id_cell)),
                Cell::from(tag_cell),
                Cell::from(Span::styled(s_icon, s_color)),
                Cell::from(Span::styled(p_icon, p_color)),
                Cell::from(Line::from(title_spans)),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Length(16),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(10),
    ];

    let border_style = if app.focus == FocusMode::List {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let table = Table::new(rows, widths)
        .header(Row::new(vec!["ID", "Tag", "S", "P", "Title"]).style(theme::HEADER_STYLE))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Tasks")
                .border_style(border_style),
        )
        .highlight_style(theme::SELECTED_STYLE);

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_details(f: &mut Frame, app: &mut App, area: Rect) {
    let border_style = if app.focus == FocusMode::Details {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Details")
        .border_style(border_style);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let idx = app.table_state.selected().unwrap_or(0);
    if idx >= app.display_rows.len() {
        return;
    }
    let row = &app.display_rows[idx];
    let task = &app.tasks[row.task_idx];
    let task_tag = if row.tag_name == ALL_TAG_VIEW {
        "all".to_string()
    } else {
        row.tag_name.clone()
    };

    let mut lines = Vec::new();
    if let Some(sub_idx) = row.subtask_path.first() {
        if *sub_idx < task.subtasks.len() {
            let sub = &task.subtasks[*sub_idx];
            lines.push(Line::from(vec![
                Span::styled("Parent: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{} {}", task.id, task.title)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Tag: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    task_tag,
                    theme::tag_badge_style(&row.tag_name, app.tag_color_seed),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Subtask: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{} ({})", sub.title, sub.status)),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Description:",
                Style::default().fg(Color::Blue),
            )));
            if sub.description.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No description.",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(sub.description.clone()));
            }
            if !sub.dependencies.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Dependencies:",
                    Style::default().fg(Color::Yellow),
                )));
                for dep in &sub.dependencies {
                    lines.push(Line::from(format!("- {}", dep)));
                }
            }
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::raw(task.id.as_str()),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", task.status),
                theme::status_color(task.status.as_str()),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Tag: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                task_tag,
                theme::tag_badge_style(&row.tag_name, app.tag_color_seed),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            &task.title,
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Priority: {}", task.priority),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "Active Agent: {}",
                if task.active_agent { "yes" } else { "no" }
            ),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Description:",
            Style::default().fg(Color::Blue),
        )));
        if task.description.is_empty() {
            lines.push(Line::from(Span::styled(
                "No description.",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            lines.push(Line::from(task.description.clone()));
        }

        if !task.details.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Details:",
                Style::default().fg(Color::Blue),
            )));
            lines.push(Line::from(task.details.clone()));
        }

        if !task.test_strategy.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Test Strategy:",
                Style::default().fg(Color::Blue),
            )));
            lines.push(Line::from(task.test_strategy.clone()));
        }

        let tag_prd = app
            .project
            .as_ref()
            .and_then(|project| project.tags.get(&row.tag_name))
            .and_then(|tag_ctx| tag_ctx.tag_prd());

        if let Some(prd) = &task.aoc_prd {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "PRD (task):",
                Style::default().fg(Color::Blue),
            )));
            lines.push(Line::from(prd.path.clone()));
        } else if let Some(prd) = tag_prd {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "PRD (tag default):",
                Style::default().fg(Color::Blue),
            )));
            lines.push(Line::from(prd.path));
        }

        if !task.dependencies.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Dependencies:",
                Style::default().fg(Color::Yellow),
            )));
            for dep in &task.dependencies {
                lines.push(Line::from(format!("- {}", dep)));
            }
        }

        if !task.subtasks.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Subtasks:",
                Style::default().fg(Color::Blue),
            )));
            for sub in &task.subtasks {
                let icon = if matches!(sub.status, aoc_core::TaskStatus::Done) {
                    icons::CHECK
                } else {
                    icons::PENDING
                };
                lines.push(Line::from(format!("{} {}", icon, sub.title)));
            }
        }
    }

    let total_height = wrapped_height(&lines, inner_area.width);
    let max_scroll = total_height.saturating_sub(inner_area.height);
    app.details_max_scroll = max_scroll;
    if app.details_scroll > app.details_max_scroll {
        app.details_scroll = app.details_max_scroll;
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .scroll((app.details_scroll, 0));
    f.render_widget(p, inner_area);
}

fn render_tag_selector(f: &mut Frame, app: &mut App, area: Rect) {
    let border_style = if app.focus == FocusMode::Tags {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Tags")
        .border_style(border_style);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if app.tag_items.is_empty() {
        let p = Paragraph::new("No tags found").wrap(Wrap { trim: true });
        f.render_widget(p, inner_area);
        return;
    }

    let active_tag = app.current_tag_or_default();

    let items: Vec<ListItem> = app
        .tag_items
        .iter()
        .map(|item| {
            let marker = if item.name == active_tag { "* " } else { "  " };
            let shown_name = if item.name == ALL_TAG_VIEW {
                "all"
            } else {
                item.name.as_str()
            };
            let label = format!("{}{}  ({}/{})", marker, shown_name, item.done, item.total);
            ListItem::new(Line::from(Span::raw(label)))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(theme::SELECTED_STYLE)
        .highlight_symbol("> ");
    f.render_stateful_widget(list, inner_area, &mut app.tag_list_state);
}

fn wrapped_height(lines: &[Line<'_>], width: u16) -> u16 {
    let width = width.max(1) as usize;
    let mut total: usize = 0;
    for line in lines {
        let line_width = line.width();
        if line_width == 0 {
            total += 1;
        } else {
            total += (line_width + width - 1) / width;
        }
    }
    total as u16
}
