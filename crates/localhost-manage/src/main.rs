use std::{
    collections::HashSet,
    io::stdout,
    panic, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use listeners::{Protocol, SocketState};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Terminal,
};
use serde::Serialize;
use sysinfo::{Pid, Process, ProcessesToUpdate, Signal, System};

#[derive(Parser)]
#[command(version, about = "List and stop TCP listeners")]
struct Args {
    /// Print listeners as a table
    #[arg(long, conflicts_with_all = ["json", "kill"])]
    list: bool,

    /// Print listeners as JSON
    #[arg(long, conflicts_with_all = ["list", "kill"])]
    json: bool,

    /// Stop listeners on this port
    #[arg(long, value_name = "PORT", conflicts_with_all = ["list", "json"])]
    kill: Option<u16>,

    /// Use SIGKILL instead of SIGTERM
    #[arg(long, requires = "kill")]
    force: bool,

    /// Include listeners owned by other users
    #[arg(long)]
    all: bool,
}

#[derive(Clone, Serialize)]
struct ListenerRow {
    port: u16,
    pid: u32,
    name: String,
    cmdline: String,
    cwd: String,
    age: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.list {
        print_table(&discover(args.all)?);
    } else if args.json {
        println!("{}", serde_json::to_string_pretty(&discover(args.all)?)?);
    } else if let Some(port) = args.kill {
        let rows = discover(args.all)?;
        let killed = kill_port(&rows, port, args.force)?;
        for row in killed {
            println!("killed port {} pid {} ({})", row.port, row.pid, row.name);
        }
    } else {
        run_tui(args.all)?;
    }

    Ok(())
}

fn discover(include_all: bool) -> Result<Vec<ListenerRow>> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let current_user = sysinfo::get_current_pid()
        .ok()
        .and_then(|pid| system.process(pid))
        .and_then(Process::user_id)
        .cloned();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let listeners = listeners::get_all().map_err(|error| anyhow!(error.to_string()))?;
    let mut seen = HashSet::new();
    let mut rows = Vec::new();

    for listener in listeners {
        if listener.protocol != Protocol::TCP || listener.state != SocketState::Listen {
            continue;
        }
        let pid_value = listener.process.pid;
        let port = listener.socket.port();
        if !seen.insert((port, pid_value)) {
            continue;
        }

        let process = system.process(Pid::from_u32(pid_value));
        if !include_all {
            if let (Some(expected), Some(actual)) =
                (current_user.as_ref(), process.and_then(Process::user_id))
            {
                if actual != expected {
                    continue;
                }
            }
        }

        let name = process
            .map(|process| process.name().to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or(listener.process.name);
        let cmdline = process
            .map(|process| {
                process
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|command| !command.is_empty())
            .unwrap_or(listener.process.path);
        let cwd = process
            .and_then(Process::cwd)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        let age = process
            .map(|process| humanize_age(now.saturating_sub(process.start_time())))
            .unwrap_or_else(|| "unknown".to_string());
        rows.push(ListenerRow {
            port,
            pid: pid_value,
            name,
            cmdline,
            cwd,
            age,
        });
    }

    rows.sort_unstable_by_key(|row| (row.port, row.pid));
    Ok(rows)
}

fn humanize_age(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn print_table(rows: &[ListenerRow]) {
    let port_width = rows
        .iter()
        .map(|row| row.port.to_string().len())
        .max()
        .unwrap_or(4)
        .max(4);
    let pid_width = rows
        .iter()
        .map(|row| row.pid.to_string().len())
        .max()
        .unwrap_or(3)
        .max(3);
    let age_width = rows
        .iter()
        .map(|row| row.age.len())
        .max()
        .unwrap_or(3)
        .max(3);
    let name_width = rows
        .iter()
        .map(|row| row.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let cwd_width = rows
        .iter()
        .map(|row| row.cwd.len())
        .max()
        .unwrap_or(3)
        .max(3);

    println!(
        "{:<port_width$}  {:<pid_width$}  {:<age_width$}  {:<name_width$}  {:<cwd_width$}  COMMAND",
        "PORT", "PID", "AGE", "NAME", "CWD"
    );
    for row in rows {
        println!(
            "{:<port_width$}  {:<pid_width$}  {:<age_width$}  {:<name_width$}  {:<cwd_width$}  {}",
            row.port, row.pid, row.age, row.name, row.cwd, row.cmdline
        );
    }
}

fn kill_port(rows: &[ListenerRow], port: u16, force: bool) -> Result<Vec<ListenerRow>> {
    let targets: Vec<_> = rows.iter().filter(|row| row.port == port).collect();
    if targets.is_empty() {
        return Err(anyhow!("no listener on port {port}"));
    }

    let system = System::new_all();
    let mut killed = Vec::new();
    for row in targets {
        let process = system
            .process(Pid::from_u32(row.pid))
            .ok_or_else(|| anyhow!("process {} no longer exists", row.pid))?;
        if kill_process(process, force) {
            killed.push(row.clone());
        } else {
            return Err(anyhow!("failed to kill pid {}", row.pid));
        }
    }
    Ok(killed)
}

fn kill_process(process: &Process, force: bool) -> bool {
    let signal = if force { Signal::Kill } else { Signal::Term };
    process.kill_with(signal).unwrap_or_else(|| process.kill())
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn run_tui(include_all: bool) -> Result<()> {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous_hook(info);
    }));

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(include_all)?;
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('j') | KeyCode::Down => app.next(),
                    KeyCode::Char('k') | KeyCode::Up => app.previous(),
                    KeyCode::Char('r') => app.refresh(),
                    KeyCode::Char('d') | KeyCode::Delete => app.kill_selected(false),
                    KeyCode::Char('K') => app.kill_selected(true),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

struct App {
    rows: Vec<ListenerRow>,
    state: TableState,
    status: String,
    include_all: bool,
}

impl App {
    fn new(include_all: bool) -> Result<Self> {
        let rows = discover(include_all)?;
        let mut state = TableState::default();
        if !rows.is_empty() {
            state.select(Some(0));
        }
        Ok(Self {
            rows,
            state,
            status: String::new(),
            include_all,
        })
    }

    fn next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let next = self
            .state
            .selected()
            .map_or(0, |selected| (selected + 1).min(self.rows.len() - 1));
        self.state.select(Some(next));
    }

    fn previous(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let previous = self
            .state
            .selected()
            .map_or(0, |selected| selected.saturating_sub(1));
        self.state.select(Some(previous));
    }

    fn refresh(&mut self) {
        match discover(self.include_all) {
            Ok(rows) => {
                self.rows = rows;
                let selection = self
                    .state
                    .selected()
                    .filter(|_| !self.rows.is_empty())
                    .map(|selected| selected.min(self.rows.len() - 1));
                self.state.select(selection);
                self.status = format!("refreshed: {} listeners", self.rows.len());
            }
            Err(error) => self.status = format!("refresh failed: {error}"),
        }
    }

    fn kill_selected(&mut self, force: bool) {
        let Some(selected) = self.state.selected() else {
            self.status = "no listener selected".to_string();
            return;
        };
        let row = self.rows[selected].clone();
        let system = System::new_all();
        self.status = match system.process(Pid::from_u32(row.pid)) {
            Some(process) if kill_process(process, force) => {
                let signal = if force { "SIGKILL" } else { "SIGTERM" };
                format!("sent {signal} to pid {} on port {}", row.pid, row.port)
            }
            Some(_) => format!("failed to kill pid {}", row.pid),
            None => format!("pid {} no longer exists", row.pid),
        };
        thread::sleep(Duration::from_millis(300));
        let action = self.status.clone();
        self.refresh();
        self.status = action;
    }

    fn draw(&mut self, frame: &mut ratatui::Frame<'_>) {
        let [table_area, status_area] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
        let fixed = 6 + 8 + 10 + 18 + 5;
        let flexible = table_area.width.saturating_sub(fixed);
        let cwd_width = flexible / 2;
        let command_width = flexible.saturating_sub(cwd_width);
        let header = Row::new(["PORT", "PID", "AGE", "NAME", "CWD", "COMMAND"])
            .style(Style::default().add_modifier(Modifier::BOLD));
        let rows = self.rows.iter().map(|row| {
            Row::new([
                Cell::from(row.port.to_string()),
                Cell::from(row.pid.to_string()),
                Cell::from(row.age.clone()),
                Cell::from(truncate(&row.name, 18)),
                Cell::from(truncate(&row.cwd, cwd_width as usize)),
                Cell::from(truncate(&row.cmdline, command_width as usize)),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(18),
                Constraint::Length(cwd_width),
                Constraint::Min(command_width),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(" TCP listeners ")
                .borders(Borders::ALL),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
        frame.render_stateful_widget(table, table_area, &mut self.state);

        let hint = "q quit  j/k move  d term  K kill  r refresh";
        let status = if self.status.is_empty() {
            hint.to_string()
        } else {
            format!("{} | {hint}", self.status)
        };
        frame.render_widget(
            Paragraph::new(truncate(&status, status_area.width as usize)),
            status_area,
        );
    }
}

fn truncate(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }
    let mut output: String = value.chars().take(width - 1).collect();
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::{humanize_age, truncate};

    #[test]
    fn humanizes_ages() {
        assert_eq!(humanize_age(59), "0m");
        assert_eq!(humanize_age(11_520), "3h 12m");
        assert_eq!(humanize_age(183_600), "2d 3h");
    }

    #[test]
    fn truncates_without_splitting_characters() {
        assert_eq!(truncate("command", 5), "comm…");
        assert_eq!(truncate("éclair", 2), "é…");
        assert_eq!(truncate("short", 8), "short");
    }
}
