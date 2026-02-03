use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct AocConfig {
    projects_base: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Defaults,
    Projects,
    Sessions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Nav,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PickTarget {
    Defaults,
    Override,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    Normal,
    PickLayout(PickTarget),
    PickAgent(PickTarget),
    EditProjectsBase,
    SearchProjects,
    NewProject,
    Help,
}

#[derive(Clone, Debug)]
struct ProjectEntry {
    name: String,
    path: PathBuf,
}

#[derive(Clone, Debug, Default)]
struct SessionOverrides {
    layout: Option<String>,
    agent: Option<String>,
}

#[derive(Clone, Debug)]
struct PendingLaunch {
    cwd: PathBuf,
    env_overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
struct App {
    active_tab: Tab,
    focus: Focus,
    mode: Mode,
    status: String,
    should_exit: bool,
    pending_launch: Option<PendingLaunch>,
    defaults_state: ListState,
    projects_state: ListState,
    sessions_state: ListState,
    layout_picker_state: ListState,
    agent_picker_state: ListState,
    default_layout: String,
    default_agent: String,
    config: AocConfig,
    config_path: PathBuf,
    projects_base: PathBuf,
    projects: Vec<ProjectEntry>,
    project_filter: String,
    filtered_projects: Vec<usize>,
    input_buffer: String,
    input_snapshot: String,
    session_overrides: SessionOverrides,
    in_zellij: bool,
    floating_active: bool,
    close_on_exit: bool,
    pane_rename_remaining: u8,
}

impl App {
    fn new() -> io::Result<Self> {
        let config_path = config_path();
        let config = load_config(&config_path).unwrap_or_default();
        let projects_base = resolve_projects_base(&config);
        let projects = load_projects(&projects_base).unwrap_or_default();
        let mut app = Self {
            active_tab: Tab::Defaults,
            focus: Focus::Nav,
            mode: Mode::Normal,
            status: String::new(),
            should_exit: false,
            pending_launch: None,
            defaults_state: ListState::default(),
            projects_state: ListState::default(),
            sessions_state: ListState::default(),
            layout_picker_state: ListState::default(),
            agent_picker_state: ListState::default(),
            default_layout: read_default(&layout_default_path())
                .unwrap_or_else(|| "aoc".to_string()),
            default_agent: read_default(&agent_default_path())
                .unwrap_or_else(|| "codex".to_string()),
            config,
            config_path,
            projects_base,
            projects,
            project_filter: String::new(),
            filtered_projects: Vec::new(),
            input_buffer: String::new(),
            input_snapshot: String::new(),
            session_overrides: SessionOverrides::default(),
            in_zellij: in_zellij(),
            floating_active: is_floating_active(),
            close_on_exit: false,
            pane_rename_remaining: if in_zellij() { 6 } else { 0 },
        };
        app.apply_project_filter();
        app.ensure_selections();
        Ok(app)
    }

    fn ensure_selections(&mut self) {
        ensure_selection(&mut self.defaults_state, 3);
        ensure_selection(&mut self.projects_state, self.filtered_projects.len());
        ensure_selection(&mut self.sessions_state, 4);
        ensure_selection(&mut self.layout_picker_state, layout_options().len());
        ensure_selection(&mut self.agent_picker_state, agent_options().len());
    }

    fn set_status<S: Into<String>>(&mut self, message: S) {
        self.status = message.into();
    }

    fn apply_project_filter(&mut self) {
        self.filtered_projects.clear();
        let query = self.project_filter.trim().to_lowercase();
        for (idx, entry) in self.projects.iter().enumerate() {
            if query.is_empty() {
                self.filtered_projects.push(idx);
                continue;
            }
            let name = entry.name.to_lowercase();
            let path = entry.path.to_string_lossy().to_lowercase();
            if name.contains(&query) || path.contains(&query) {
                self.filtered_projects.push(idx);
            }
        }
        ensure_selection(&mut self.projects_state, self.filtered_projects.len());
    }

    fn reload_projects(&mut self) {
        match load_projects(&self.projects_base) {
            Ok(list) => {
                self.projects = list;
                self.apply_project_filter();
                self.set_status("Projects refreshed");
            }
            Err(err) => {
                self.set_status(format!("Failed to read projects: {err}"));
            }
        }
    }

    fn selected_project(&self) -> Option<ProjectEntry> {
        let selected = self.projects_state.selected().unwrap_or(0);
        let index = self.filtered_projects.get(selected).copied()?;
        self.projects.get(index).cloned()
    }

    fn start_input(&mut self, mode: Mode, initial: String) {
        self.mode = mode;
        self.input_buffer = initial.clone();
        self.input_snapshot = initial;
    }

    fn cancel_input(&mut self) {
        self.input_buffer = String::new();
        self.input_snapshot = String::new();
        self.mode = Mode::Normal;
    }

    fn commit_projects_base(&mut self) {
        let value = self.input_buffer.trim().to_string();
        if value.is_empty() {
            self.set_status("Projects base cannot be empty");
            return;
        }
        self.config.projects_base = Some(value.clone());
        if let Err(err) = save_config(&self.config_path, &self.config) {
            self.set_status(format!("Failed to save config: {err}"));
            return;
        }
        self.projects_base = PathBuf::from(value);
        self.reload_projects();
        self.set_status("Projects base updated");
        self.cancel_input();
    }

    fn commit_search(&mut self) {
        self.project_filter = self.input_buffer.clone();
        self.apply_project_filter();
        self.cancel_input();
    }

    fn commit_new_project(&mut self) {
        let input = self.input_buffer.trim();
        if input.is_empty() {
            self.set_status("Project name cannot be empty");
            return;
        }
        let project_path = resolve_project_path(input, &self.projects_base);
        if let Err(err) = fs::create_dir_all(&project_path) {
            self.set_status(format!("Failed to create project: {err}"));
            return;
        }
        if let Err(err) = run_aoc_init(&project_path) {
            self.set_status(format!("aoc-init failed: {err}"));
            return;
        }
        self.cancel_input();
        self.reload_projects();
        self.open_project(project_path);
    }

    fn open_project(&mut self, path: PathBuf) {
        if self.in_zellij {
            if let Err(err) = run_open_in_zellij(&path, &self.session_overrides) {
                self.set_status(format!("Failed to open tab: {err}"));
            } else {
                self.set_status(format!("Opened {}", path.to_string_lossy()));
            }
        } else {
            let envs = build_env_overrides(&self.session_overrides);
            self.pending_launch = Some(PendingLaunch {
                cwd: path,
                env_overrides: envs,
            });
            self.should_exit = true;
        }
    }

    fn launch_session(&mut self) {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if self.in_zellij {
            if let Err(err) = run_open_in_zellij(&cwd, &self.session_overrides) {
                self.set_status(format!("Failed to open tab: {err}"));
            } else {
                self.set_status("Opened new tab".to_string());
            }
        } else {
            let envs = build_env_overrides(&self.session_overrides);
            self.pending_launch = Some(PendingLaunch {
                cwd,
                env_overrides: envs,
            });
            self.should_exit = true;
        }
    }

    fn set_default_layout(&mut self, layout: String) {
        if let Err(err) = write_default(&layout_default_path(), &layout) {
            self.set_status(format!("Failed to write layout default: {err}"));
            return;
        }
        self.default_layout = layout.clone();
        self.set_status(format!("Default layout set to {layout}"));
        self.mode = Mode::Normal;
    }

    fn set_default_agent(&mut self, agent: String) {
        if let Err(err) = write_default(&agent_default_path(), &agent) {
            self.set_status(format!("Failed to write agent default: {err}"));
            return;
        }
        self.default_agent = agent.clone();
        self.set_status(format!("Default agent set to {agent}"));
        self.mode = Mode::Normal;
    }

    fn set_override_layout(&mut self, layout: String) {
        self.session_overrides.layout = Some(layout.clone());
        self.set_status(format!("Override layout set to {layout}"));
        self.mode = Mode::Normal;
    }

    fn set_override_agent(&mut self, agent: String) {
        self.session_overrides.agent = Some(agent.clone());
        self.set_status(format!("Override agent set to {agent}"));
        self.mode = Mode::Normal;
    }

    fn clear_overrides(&mut self) {
        self.session_overrides = SessionOverrides::default();
        self.set_status("Cleared overrides");
    }

    fn tick(&mut self) {
        if self.pane_rename_remaining == 0 {
            return;
        }
        rename_pane();
        self.pane_rename_remaining = self.pane_rename_remaining.saturating_sub(1);
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    app.tick();
    let tick = Duration::from_millis(200);

    while !app.should_exit {
        terminal.draw(|frame| draw_ui(frame, &mut app))?;
        if event::poll(tick)? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, key);
            }
        }
        app.tick();
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Some(pending) = app.pending_launch.take() {
        run_aoc_launch(&pending)?;
    }

    if app.close_on_exit {
        close_floating_pane();
    }

    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) {
    match app.mode {
        Mode::Normal => handle_key_normal(app, key),
        Mode::PickLayout(target) => handle_key_picker(app, key, Picker::Layout(target)),
        Mode::PickAgent(target) => handle_key_picker(app, key, Picker::Agent(target)),
        Mode::EditProjectsBase => handle_key_input(app, key, InputMode::ProjectsBase),
        Mode::SearchProjects => handle_key_input(app, key, InputMode::Search),
        Mode::NewProject => handle_key_input(app, key, InputMode::NewProject),
        Mode::Help => handle_key_help(app, key),
    }
}

fn handle_key_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => app.should_exit = true,
        KeyCode::Esc => {
            if app.focus == Focus::Detail {
                app.focus = Focus::Nav;
            } else {
                app.should_exit = true;
                if app.floating_active {
                    app.close_on_exit = true;
                }
            }
        }
        KeyCode::Tab => cycle_tab(app, true),
        KeyCode::BackTab => cycle_tab(app, false),
        KeyCode::Char('h') | KeyCode::Left => {
            if app.focus == Focus::Detail {
                app.focus = Focus::Nav;
            } else {
                cycle_tab(app, false);
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if app.focus == Focus::Nav {
                app.focus = Focus::Detail;
            } else {
                activate_selection(app);
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if app.focus == Focus::Nav {
                cycle_tab(app, true);
            } else {
                list_next(app);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.focus == Focus::Nav {
                cycle_tab(app, false);
            } else {
                list_prev(app);
            }
        }
        KeyCode::Enter => {
            if app.focus == Focus::Nav {
                app.focus = Focus::Detail;
            } else {
                activate_selection(app);
            }
        }
        KeyCode::Char('/') if app.active_tab == Tab::Projects && app.focus == Focus::Detail => {
            app.start_input(Mode::SearchProjects, app.project_filter.clone());
        }
        KeyCode::Char('n') if app.active_tab == Tab::Projects && app.focus == Focus::Detail => {
            app.start_input(Mode::NewProject, String::new());
        }
        KeyCode::Char('r') if app.active_tab == Tab::Projects && app.focus == Focus::Detail => {
            app.reload_projects()
        }
        KeyCode::Char('o') if app.active_tab == Tab::Projects && app.focus == Focus::Detail => {
            if let Some(project) = app.selected_project() {
                app.open_project(project.path);
            }
        }
        KeyCode::Char('c') if app.active_tab == Tab::Sessions && app.focus == Focus::Detail => {
            app.clear_overrides()
        }
        KeyCode::Char('?') => {
            app.mode = Mode::Help;
        }
        _ => {}
    }
}

enum Picker {
    Layout(PickTarget),
    Agent(PickTarget),
}

fn handle_key_picker(app: &mut App, key: KeyEvent, picker: Picker) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char('j') | KeyCode::Down => match picker {
            Picker::Layout(_) => {
                list_next_state(&mut app.layout_picker_state, layout_options().len())
            }
            Picker::Agent(_) => list_next_state(&mut app.agent_picker_state, agent_options().len()),
        },
        KeyCode::Char('k') | KeyCode::Up => match picker {
            Picker::Layout(_) => {
                list_prev_state(&mut app.layout_picker_state, layout_options().len())
            }
            Picker::Agent(_) => list_prev_state(&mut app.agent_picker_state, agent_options().len()),
        },
        KeyCode::Enter => match picker {
            Picker::Layout(target) => {
                let index = app.layout_picker_state.selected().unwrap_or(0);
                let options = layout_options();
                if let Some(choice) = options.get(index).cloned() {
                    match target {
                        PickTarget::Defaults => app.set_default_layout(choice),
                        PickTarget::Override => app.set_override_layout(choice),
                    }
                }
            }
            Picker::Agent(target) => {
                let index = app.agent_picker_state.selected().unwrap_or(0);
                let options = agent_options();
                if let Some(choice) = options.get(index).cloned() {
                    match target {
                        PickTarget::Defaults => app.set_default_agent(choice),
                        PickTarget::Override => app.set_override_agent(choice),
                    }
                }
            }
        },
        _ => {}
    }
}

enum InputMode {
    ProjectsBase,
    Search,
    NewProject,
}

fn handle_key_input(app: &mut App, key: KeyEvent, mode: InputMode) {
    match key.code {
        KeyCode::Esc => {
            app.input_buffer = app.input_snapshot.clone();
            app.cancel_input();
        }
        KeyCode::Enter => match mode {
            InputMode::ProjectsBase => app.commit_projects_base(),
            InputMode::Search => app.commit_search(),
            InputMode::NewProject => app.commit_new_project(),
        },
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(ch) => {
            app.input_buffer.push(ch);
        }
        _ => {}
    }
}

fn handle_key_help(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') => app.mode = Mode::Normal,
        _ => {}
    }
}

fn cycle_tab(app: &mut App, forward: bool) {
    app.active_tab = match (app.active_tab, forward) {
        (Tab::Defaults, true) => Tab::Projects,
        (Tab::Projects, true) => Tab::Sessions,
        (Tab::Sessions, true) => Tab::Defaults,
        (Tab::Defaults, false) => Tab::Sessions,
        (Tab::Projects, false) => Tab::Defaults,
        (Tab::Sessions, false) => Tab::Projects,
    };
}

fn list_next(app: &mut App) {
    match app.active_tab {
        Tab::Defaults => list_next_state(&mut app.defaults_state, 3),
        Tab::Projects => list_next_state(&mut app.projects_state, app.filtered_projects.len()),
        Tab::Sessions => list_next_state(&mut app.sessions_state, 4),
    }
}

fn list_prev(app: &mut App) {
    match app.active_tab {
        Tab::Defaults => list_prev_state(&mut app.defaults_state, 3),
        Tab::Projects => list_prev_state(&mut app.projects_state, app.filtered_projects.len()),
        Tab::Sessions => list_prev_state(&mut app.sessions_state, 4),
    }
}

fn activate_selection(app: &mut App) {
    match app.active_tab {
        Tab::Defaults => match app.defaults_state.selected().unwrap_or(0) {
            0 => {
                let current = app.default_layout.clone();
                select_picker(&mut app.layout_picker_state, &layout_options(), &current);
                app.mode = Mode::PickLayout(PickTarget::Defaults);
            }
            1 => {
                let current = app.default_agent.clone();
                select_picker(&mut app.agent_picker_state, &agent_options(), &current);
                app.mode = Mode::PickAgent(PickTarget::Defaults);
            }
            2 => app.start_input(
                Mode::EditProjectsBase,
                app.projects_base.to_string_lossy().to_string(),
            ),
            _ => {}
        },
        Tab::Projects => {
            if let Some(project) = app.selected_project() {
                app.open_project(project.path);
            }
        }
        Tab::Sessions => match app.sessions_state.selected().unwrap_or(0) {
            0 => app.launch_session(),
            1 => {
                let current = app
                    .session_overrides
                    .layout
                    .clone()
                    .unwrap_or_else(|| app.default_layout.clone());
                select_picker(&mut app.layout_picker_state, &layout_options(), &current);
                app.mode = Mode::PickLayout(PickTarget::Override);
            }
            2 => {
                let current = app
                    .session_overrides
                    .agent
                    .clone()
                    .unwrap_or_else(|| app.default_agent.clone());
                select_picker(&mut app.agent_picker_state, &agent_options(), &current);
                app.mode = Mode::PickAgent(PickTarget::Override);
            }
            3 => app.clear_overrides(),
            _ => {}
        },
    }
}

fn draw_ui(frame: &mut ratatui::Frame, app: &mut App) {
    let root = frame.size();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(5)])
        .split(root);
    let body = layout[0];
    let footer = layout[1];

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(1)])
        .split(body);

    draw_nav(frame, columns[0], app, app.focus == Focus::Nav);
    draw_detail(frame, columns[1], app, app.focus == Focus::Detail);
    draw_footer(frame, footer, app);

    if app.mode != Mode::Normal {
        draw_modal(frame, app);
    }
}

fn draw_nav(frame: &mut ratatui::Frame, area: Rect, app: &mut App, focused: bool) {
    let items = vec![
        ListItem::new("Settings"),
        ListItem::new("Projects"),
        ListItem::new("Launch"),
    ];
    let mut state = ListState::default();
    state.select(Some(match app.active_tab {
        Tab::Defaults => 0,
        Tab::Projects => 1,
        Tab::Sessions => 2,
    }));
    let list = List::new(items)
        .block(titled_block("AOC Control", focused))
        .highlight_style(nav_highlight_style(focused))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_detail(frame: &mut ratatui::Frame, area: Rect, app: &mut App, focused: bool) {
    match app.active_tab {
        Tab::Defaults => draw_defaults(frame, area, app, focused),
        Tab::Projects => draw_projects(frame, area, app, focused),
        Tab::Sessions => draw_sessions(frame, area, app, focused),
    }
}

fn draw_defaults(frame: &mut ratatui::Frame, area: Rect, app: &mut App, focused: bool) {
    let items = vec![
        ListItem::new(format!("Set layout: {}", app.default_layout)),
        ListItem::new(format!("Set agent: {}", app.default_agent)),
        ListItem::new(format!(
            "Projects base: {}",
            app.projects_base.to_string_lossy()
        )),
    ];
    let list = List::new(items)
        .block(titled_block("Settings", focused))
        .highlight_style(detail_highlight_style(focused))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, area, &mut app.defaults_state);
}

fn draw_projects(frame: &mut ratatui::Frame, area: Rect, app: &mut App, focused: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(1)])
        .split(area);

    let base = app.projects_base.to_string_lossy();
    let filter = if app.project_filter.is_empty() {
        "(none)"
    } else {
        app.project_filter.as_str()
    };
    let total = app.projects.len();
    let shown = app.filtered_projects.len();
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            format!("Base: {base}"),
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(vec![Span::styled(
            format!("Filter: {filter}"),
            Style::default().fg(Color::Cyan),
        )]),
        Line::from(vec![Span::styled(
            format!("Showing {shown} of {total}"),
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(Block::default().borders(Borders::ALL).title("Projects"))
    .alignment(Alignment::Left);
    frame.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = app
        .filtered_projects
        .iter()
        .filter_map(|idx| app.projects.get(*idx))
        .map(|entry| {
            ListItem::new(Line::from(vec![
                Span::styled(&entry.name, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled(
                    entry.path.to_string_lossy(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(titled_block("Project List", focused))
        .highlight_style(detail_highlight_style(focused))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, chunks[1], &mut app.projects_state);
}

fn draw_sessions(frame: &mut ratatui::Frame, area: Rect, app: &mut App, focused: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(1)])
        .split(area);

    let overrides = format!(
        "Overrides: layout={} agent={} ",
        app.session_overrides
            .layout
            .clone()
            .unwrap_or_else(|| "(default)".to_string()),
        app.session_overrides
            .agent
            .clone()
            .unwrap_or_else(|| "(default)".to_string())
    );
    let header = Paragraph::new(vec![Line::from(overrides)])
        .block(Block::default().borders(Borders::ALL).title("Launch"));
    frame.render_widget(header, chunks[0]);

    let items = vec![
        ListItem::new("Launch new tab/session"),
        ListItem::new("Set launch layout"),
        ListItem::new("Set launch agent"),
        ListItem::new("Clear overrides"),
    ];
    let list = List::new(items)
        .block(titled_block("Actions", focused))
        .highlight_style(detail_highlight_style(focused))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, chunks[1], &mut app.sessions_state);
}

fn draw_footer(frame: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let lines = footer_lines(app);
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

fn draw_modal(frame: &mut ratatui::Frame, app: &mut App) {
    let area = centered_rect(60, 40, frame.size());
    frame.render_widget(Clear, area);
    match app.mode {
        Mode::PickLayout(target) => {
            let title = match target {
                PickTarget::Defaults => "Select Layout (Default)",
                PickTarget::Override => "Select Layout (Launch)",
            };
            let items: Vec<ListItem> = layout_options().into_iter().map(ListItem::new).collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            frame.render_stateful_widget(list, area, &mut app.layout_picker_state);
        }
        Mode::PickAgent(target) => {
            let title = match target {
                PickTarget::Defaults => "Select Agent (Default)",
                PickTarget::Override => "Select Agent (Launch)",
            };
            let items: Vec<ListItem> = agent_options().into_iter().map(ListItem::new).collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            frame.render_stateful_widget(list, area, &mut app.agent_picker_state);
        }
        Mode::EditProjectsBase => draw_input_modal(frame, area, "Projects base", &app.input_buffer),
        Mode::SearchProjects => draw_input_modal(frame, area, "Search projects", &app.input_buffer),
        Mode::NewProject => draw_input_modal(frame, area, "New project", &app.input_buffer),
        Mode::Help => draw_help_modal(frame, area),
        Mode::Normal => {}
    }
}

fn draw_input_modal(frame: &mut ratatui::Frame, area: Rect, title: &str, input: &str) {
    let block = Block::default().borders(Borders::ALL).title(title);
    let paragraph = Paragraph::new(input)
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

fn draw_help_modal(frame: &mut ratatui::Frame, area: Rect) {
    let lines = vec![
        Line::from("AOC Control Help"),
        Line::from(""),
        Line::from("Navigation:"),
        Line::from("  h/l or Left/Right  focus menu/details"),
        Line::from("  Tab / Shift+Tab    cycle sections"),
        Line::from("  j/k or Up/Down     move selection"),
        Line::from("  Enter              select action"),
        Line::from("  Esc                back (quit from menu)"),
        Line::from("  q                  quit"),
        Line::from(""),
        Line::from("Settings:"),
        Line::from("  Enter  change layout/agent/base"),
        Line::from(""),
        Line::from("Projects:"),
        Line::from("  Enter or o  open project"),
        Line::from("  n  new project"),
        Line::from("  /  search filter"),
        Line::from("  r  refresh list"),
        Line::from(""),
        Line::from("Launch:"),
        Line::from("  Enter  launch"),
        Line::from("  c  clear overrides"),
        Line::from(""),
        Line::from("Press Esc or ? to close this help."),
    ];
    let block = Block::default().borders(Borders::ALL).title("Help");
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, rect: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(rect);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);
    horizontal[1]
}

fn footer_lines(app: &App) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    if app.status.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Status: Ready",
            Style::default().fg(Color::DarkGray),
        )]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.status.clone()),
        ]));
    }

    lines.push(Line::from(vec![
        keycap("h/l"),
        Span::raw(" focus  "),
        keycap("j/k"),
        Span::raw(" move  "),
        keycap("Enter"),
        Span::raw(" select  "),
        keycap("Tab"),
        Span::raw(" section  "),
        keycap("Esc"),
        Span::raw(" back  "),
        keycap("q"),
        Span::raw(" quit  "),
        keycap("?"),
        Span::raw(" help"),
    ]));

    let action_line = match app.mode {
        Mode::EditProjectsBase | Mode::SearchProjects | Mode::NewProject => vec![
            keycap("Enter"),
            Span::raw(" save  "),
            keycap("Esc"),
            Span::raw(" cancel"),
        ],
        Mode::PickLayout(_) | Mode::PickAgent(_) => vec![
            keycap("Enter"),
            Span::raw(" choose  "),
            keycap("Esc"),
            Span::raw(" cancel"),
        ],
        Mode::Help => vec![keycap("Esc"), Span::raw(" close help")],
        Mode::Normal => match app.active_tab {
            Tab::Defaults => vec![keycap("Enter"), Span::raw(" adjust settings")],
            Tab::Projects => vec![
                keycap("Enter"),
                Span::raw(" open  "),
                keycap("n"),
                Span::raw(" new  "),
                keycap("/"),
                Span::raw(" search  "),
                keycap("r"),
                Span::raw(" refresh"),
            ],
            Tab::Sessions => vec![
                keycap("Enter"),
                Span::raw(" launch  "),
                keycap("c"),
                Span::raw(" clear"),
            ],
        },
    };
    lines.push(Line::from(action_line));
    lines
}

fn keycap(label: &str) -> Span<'_> {
    Span::styled(
        format!("[{label}]"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn titled_block(title: &str, focused: bool) -> Block<'_> {
    let title_style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let title = if focused {
        format!("{title} *")
    } else {
        title.to_string()
    };
    let mut block = Block::default()
        .title(Span::styled(title, title_style))
        .borders(Borders::ALL);
    if focused {
        block = block.border_style(Style::default().fg(Color::Cyan));
    }
    block
}

fn nav_highlight_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    }
}

fn detail_highlight_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    }
}

fn select_picker(state: &mut ListState, options: &[String], current: &str) {
    if let Some(pos) = options.iter().position(|value| value == current) {
        state.select(Some(pos));
    }
}

fn ensure_selection(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    if state.selected().is_none() {
        state.select(Some(0));
    }
}

fn list_next_state(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let next = match state.selected() {
        Some(idx) => (idx + 1) % len,
        None => 0,
    };
    state.select(Some(next));
}

fn list_prev_state(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let next = match state.selected() {
        Some(0) | None => len - 1,
        Some(idx) => idx - 1,
    };
    state.select(Some(next));
}

fn load_config(path: &Path) -> io::Result<AocConfig> {
    if !path.exists() {
        return Ok(AocConfig::default());
    }
    let contents = fs::read_to_string(path)?;
    let config = toml::from_str(&contents).unwrap_or_default();
    Ok(config)
}

fn save_config(path: &Path, config: &AocConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(config).unwrap_or_default();
    fs::write(path, contents)
}

fn layout_default_path() -> PathBuf {
    state_dir().join("aoc/layout_default")
}

fn agent_default_path() -> PathBuf {
    state_dir().join("aoc/agent_default")
}

fn config_path() -> PathBuf {
    if let Ok(path) = env::var("AOC_CONFIG_PATH") {
        return PathBuf::from(path);
    }
    config_dir().join("aoc/config.toml")
}

fn state_dir() -> PathBuf {
    if let Ok(path) = env::var("XDG_STATE_HOME") {
        return PathBuf::from(path);
    }
    home_dir().join(".local/state")
}

fn config_dir() -> PathBuf {
    if let Ok(path) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(path);
    }
    home_dir().join(".config")
}

fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn read_default(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn write_default(path: &Path, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, value.as_bytes())
}

fn resolve_projects_base(config: &AocConfig) -> PathBuf {
    if let Ok(value) = env::var("AOC_PROJECTS_BASE") {
        return PathBuf::from(value);
    }
    if let Some(value) = &config.projects_base {
        return PathBuf::from(value);
    }
    let base = home_dir().join("dev");
    if base.is_dir() {
        base
    } else {
        home_dir()
    }
}

fn load_projects(base: &Path) -> io::Result<Vec<ProjectEntry>> {
    let mut entries = Vec::new();
    if !base.is_dir() {
        return Ok(entries);
    }
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(ProjectEntry { name, path });
        }
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(entries)
}

fn layout_options() -> Vec<String> {
    let mut options = vec!["aoc".to_string()];
    let layouts_dir = config_dir().join("zellij/layouts");
    if let Ok(entries) = fs::read_dir(layouts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "kdl" {
                    if let Some(name) = path.file_stem() {
                        let name = name.to_string_lossy().to_string();
                        if !options.contains(&name) {
                            options.push(name);
                        }
                    }
                }
            }
        }
    }
    options.sort();
    options
}

fn agent_options() -> Vec<String> {
    vec![
        "codex".to_string(),
        "gemini".to_string(),
        "kimi".to_string(),
        "cc".to_string(),
        "oc".to_string(),
    ]
}

fn resolve_project_path(input: &str, base: &Path) -> PathBuf {
    let trimmed = input.trim();
    if let Some(path) = trimmed.strip_prefix("~/") {
        return home_dir().join(path);
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        return path;
    }
    base.join(path)
}

fn run_aoc_init(path: &Path) -> io::Result<()> {
    let status = Command::new("aoc-init")
        .current_dir(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "aoc-init failed"))
    }
}

fn run_open_in_zellij(path: &Path, overrides: &SessionOverrides) -> io::Result<()> {
    let mut cmd = Command::new("aoc-new-tab");
    cmd.arg("--cwd").arg(path);
    for (key, value) in build_env_overrides(overrides) {
        cmd.env(key, value);
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "aoc-new-tab failed"))
    }
}

fn build_env_overrides(overrides: &SessionOverrides) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    if let Some(layout) = overrides.layout.clone() {
        envs.push(("AOC_LAYOUT".to_string(), layout));
    }
    if let Some(agent) = overrides.agent.clone() {
        envs.push(("AOC_AGENT_ID".to_string(), agent));
    }
    envs
}

fn run_aoc_launch(pending: &PendingLaunch) -> io::Result<()> {
    let mut cmd = Command::new("aoc-launch");
    cmd.current_dir(&pending.cwd);
    for (key, value) in &pending.env_overrides {
        cmd.env(key, value);
    }
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "aoc-launch failed"))
    }
}

fn in_zellij() -> bool {
    env::var("ZELLIJ").is_ok() || env::var("ZELLIJ_SESSION_NAME").is_ok()
}

fn is_floating_active() -> bool {
    env::var("AOC_CONTROL_FLOATING_ACTIVE")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn close_floating_pane() {
    if !in_zellij() {
        return;
    }
    let _ = Command::new("zellij")
        .args(["action", "close-pane"])
        .status();
}

fn rename_pane() {
    if !in_zellij() {
        return;
    }
    let name = env::var("AOC_CONTROL_PANE_NAME").unwrap_or_else(|_| "Control".to_string());
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    if let Ok(pane_id) = env::var("ZELLIJ_PANE_ID") {
        let status = Command::new("zellij")
            .args(["action", "rename-pane", "--pane-id", &pane_id, name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if matches!(status, Ok(status) if status.success()) {
            return;
        }
    }
    emit_pane_title(name);
}

fn emit_pane_title(title: &str) {
    let mut stdout = io::stdout();
    let payload = format!("\x1b]0;{}\x07", title);
    if stdout.write_all(payload.as_bytes()).is_ok() {
        let _ = stdout.flush();
    }
}
