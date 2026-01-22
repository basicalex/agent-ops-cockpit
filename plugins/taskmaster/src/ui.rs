use crate::state::{FocusMode, State};
use crate::theme::{self, icons};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, state: &mut State) {
    let area = f.size();

    if state.show_help || state.show_detail {
        let main = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_main(f, state, main[0]);

        if state.show_help {
            render_help(f, state, main[1]);
        } else {
            render_details(f, state, main[1]);
        }
    } else {
        render_main(f, state, area);
    }
}

fn render_help(f: &mut Frame, _state: &State, area: Rect) {
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
            Span::raw("   Select next task"),
        ]),
        Line::from(vec![
            Span::styled("k / Up", Color::Cyan),
            Span::raw("     Select previous task"),
        ]),
        Line::from(vec![
            Span::styled("x", Color::Cyan),
            Span::raw("          Toggle Done/Pending"),
        ]),
        Line::from(vec![
            Span::styled("Space", Color::Cyan),
            Span::raw("      Expand/Collapse subtasks"),
        ]),
        Line::from(vec![
            Span::styled("Enter", Color::Cyan),
            Span::raw("      Toggle Details Pane"),
        ]),
        Line::from(vec![
            Span::styled("Tab", Color::Cyan),
            Span::raw("        Switch focus (List <-> Details)"),
        ]),
        Line::from(vec![
            Span::styled("r", Color::Cyan),
            Span::raw("          Refresh tasks"),
        ]),
        Line::from(vec![
            Span::styled("f", Color::Cyan),
            Span::raw("          Cycle Filter (All/Pending/Done)"),
        ]),
        Line::from(vec![
            Span::styled("t", Color::Cyan),
            Span::raw("          Cycle Tag"),
        ]),
        Line::from(vec![
            Span::styled("?", Color::Cyan),
            Span::raw("          Toggle this Help"),
        ]),
    ];

    let p = Paragraph::new(text).wrap(Wrap { trim: true });
    f.render_widget(p, inner_area);
}

fn render_main(f: &mut Frame, state: &mut State, area: Rect) {
    let rows: Vec<Row> = state
        .display_rows
        .iter()
        .map(|row| {
            let task = &state.tasks[row.task_idx];

            // Resolve subtask if applicable
            let (title, status, priority, is_subtask) =
                if let Some(sub_idx) = row.subtask_path.first() {
                    if *sub_idx < task.subtasks.len() {
                        let sub = &task.subtasks[*sub_idx];
                        (
                            sub.title.clone(),
                            sub.status.clone(),
                            aoc_core::TaskPriority::Medium,
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

            let status_str = format!("{:?}", status).to_lowercase();
            let s_icon = match status_str.as_str() {
                "done" => icons::CHECK,
                "inprogress" | "in-progress" => icons::IN_PROGRESS,
                "blocked" => icons::BLOCKED,
                "review" => icons::REVIEW,
                "cancelled" => icons::BLOCKED,
                _ => icons::PENDING,
            };
            let s_color = theme::status_color(&status_str);

            let prio_str = format!("{:?}", priority).to_lowercase();
            let p_icon = if is_subtask {
                "" // Don't show priority for subtasks
            } else {
                match prio_str.as_str() {
                    "high" => icons::PRIORITY_HIGH,
                    "low" => icons::PRIORITY_LOW,
                    _ => icons::PRIORITY_MED,
                }
            };
            let p_color = theme::priority_color(&prio_str);

            let mut title_spans = Vec::new();
            let indent_width = row.depth * 2;
            if indent_width > 0 {
                title_spans.push(Span::raw(" ".repeat(indent_width)));
                title_spans.push(Span::styled("└─ ", Color::DarkGray));
            }

            if !is_subtask && !task.subtasks.is_empty() {
                let icon = if state.expanded_tasks.contains(&task.id) {
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

            Row::new(vec![
                Cell::from(Span::raw(if is_subtask {
                    "".to_string()
                } else {
                    task.id.clone()
                })),
                Cell::from(Span::styled(s_icon, s_color)),
                Cell::from(Span::styled(p_icon, p_color)),
                Cell::from(Line::from(title_spans)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(10),
    ];

    let border_style = if state.focus == FocusMode::List {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let table = Table::new(rows, widths)
        .header(Row::new(vec!["ID", "S", "P", "Title"]).style(theme::HEADER_STYLE))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Tasks")
                .border_style(border_style),
        )
        .highlight_style(theme::SELECTED_STYLE);

    f.render_stateful_widget(table, area, &mut state.table_state);
}

fn render_details(f: &mut Frame, state: &State, area: Rect) {
    let border_style = if state.focus == FocusMode::Details {
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

    let idx = state.table_state.selected().unwrap_or(0);
    if idx >= state.display_rows.len() {
        return;
    }
    let row = &state.display_rows[idx];
    let task = &state.tasks[row.task_idx];

    let mut lines = vec![
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&task.id),
            Span::raw(" "),
            Span::styled(
                format!("[{:?}]", task.status),
                theme::status_color(&format!("{:?}", task.status)),
            ),
        ]),
        Line::from(Span::styled(
            &task.title,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Description:",
            Style::default().fg(Color::Blue),
        )),
    ];

    if task.description.is_empty() {
        lines.push(Line::from(Span::styled(
            "No description.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(task.description.clone()));
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

    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(p, inner_area);
}

// fn render_footer(f: &mut Frame, _state: &State, area: Rect) {
//     let p = Paragraph::new("Help: [x]Done [Space]Expand [Enter]Details").style(theme::NORMAL_STYLE);
//     f.render_widget(p, area);
// }
