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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main
            Constraint::Length(1), // Footer
        ])
        .split(f.size());

    render_header(f, state, chunks[0]);

    if state.show_detail {
        let main = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);
        render_main(f, state, main[0]);
        render_details(f, state, main[1]);
    } else {
        render_main(f, state, chunks[1]);
    }

    render_footer(f, state, chunks[2]);
}

fn render_header(f: &mut Frame, state: &State, area: Rect) {
    // Calculate stats
    let total = state.tasks.len();
    let done = state
        .tasks
        .iter()
        .filter(|t| format!("{:?}", t.status).to_lowercase() == "done")
        .count();
    let percent = if total > 0 {
        (done as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // Progress Bar (ASCII)
    let bar_width = 20;
    let filled = (percent / 100.0 * bar_width as f64) as usize;
    let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(bar_width - filled));

    let title_line = Line::from(vec![
        Span::styled(
            format!(" AOC TASKMASTER v2 [{}] ", state.current_tag),
            theme::HEADER_STYLE,
        ),
        Span::raw(format!("Progress: {} {}/{} ", bar, done, total)),
        Span::styled(format!("Filter: {} ", state.filter.label()), Color::Yellow),
    ]);

    let block = Block::default().borders(Borders::ALL);
    let p = Paragraph::new(title_line).block(block);
    f.render_widget(p, area);
}

fn render_main(f: &mut Frame, state: &mut State, area: Rect) {
    let rows: Vec<Row> = state
        .display_rows
        .iter()
        .map(|row| {
            let task = &state.tasks[row.task_idx];

            let status_str = format!("{:?}", task.status).to_lowercase();
            let s_icon = match status_str.as_str() {
                "done" => icons::CHECK,
                "inprogress" | "in-progress" => icons::IN_PROGRESS,
                "blocked" => icons::BLOCKED,
                "review" => icons::REVIEW,
                "cancelled" => icons::BLOCKED,
                _ => icons::PENDING,
            };
            let s_color = theme::status_color(&status_str);

            let prio_str = format!("{:?}", task.priority).to_lowercase();
            let p_icon = match prio_str.as_str() {
                "high" => icons::PRIORITY_HIGH,
                "low" => icons::PRIORITY_LOW,
                _ => icons::PRIORITY_MED,
            };
            let p_color = theme::priority_color(&prio_str);

            let mut title_spans = Vec::new();
            let indent_width = row.depth * 2;
            if indent_width > 0 {
                title_spans.push(Span::raw(" ".repeat(indent_width)));
                title_spans.push(Span::styled("└─ ", Color::DarkGray));
            }

            if !task.subtasks.is_empty() {
                let icon = if state.expanded_tasks.contains(&task.id) {
                    icons::EXPANDED
                } else {
                    icons::COLLAPSED
                };
                title_spans.push(Span::styled(format!("{} ", icon), Color::Blue));
            }

            if task.active_agent {
                title_spans.push(Span::styled(format!("{} ", icons::AGENT), Color::Magenta));
            }

            title_spans.push(Span::raw(task.title.clone()));

            Row::new(vec![
                Cell::from(Span::raw(task.id.clone())),
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

fn render_footer(f: &mut Frame, _state: &State, area: Rect) {
    let p = Paragraph::new("Help: [x]Done [Space]Expand [Enter]Details [q]Quit")
        .style(theme::NORMAL_STYLE);
    f.render_widget(p, area);
}
