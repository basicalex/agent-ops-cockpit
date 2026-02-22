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
    NewTheme,
    ThemeSections,
    ThemePresets,
    ThemeCustoms,
    ThemeActions,
    RtkActions,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeSource {
    Preset,
    Custom,
}

#[derive(Clone, Debug)]
struct ThemePresetEntry {
    name: String,
    installed: bool,
}

#[derive(Clone, Debug)]
struct ThemeSelection {
    name: String,
    source: ThemeSource,
}

#[derive(Clone, Debug)]
struct RtkStatus {
    mode: String,
    installed: bool,
    fail_open: bool,
    config_exists: bool,
    config_path: String,
    allow_count: usize,
}

impl Default for RtkStatus {
    fn default() -> Self {
        Self {
            mode: "off".to_string(),
            installed: false,
            fail_open: true,
            config_exists: false,
            config_path: String::new(),
            allow_count: 0,
        }
    }
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
    theme_sections_state: ListState,
    theme_presets_state: ListState,
    theme_customs_state: ListState,
    theme_actions_state: ListState,
    rtk_actions_state: ListState,
    default_layout: String,
    default_agent: String,
    rtk_status: RtkStatus,
    config: AocConfig,
    config_path: PathBuf,
    projects_base: PathBuf,
    projects: Vec<ProjectEntry>,
    project_filter: String,
    filtered_projects: Vec<usize>,
    input_buffer: String,
    input_snapshot: String,
    theme_presets: Vec<ThemePresetEntry>,
    theme_customs: Vec<String>,
    theme_actions: Vec<String>,
    theme_selection: Option<ThemeSelection>,
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
            theme_sections_state: ListState::default(),
            theme_presets_state: ListState::default(),
            theme_customs_state: ListState::default(),
            theme_actions_state: ListState::default(),
            rtk_actions_state: ListState::default(),
            default_layout: read_default(&layout_default_path())
                .unwrap_or_else(|| "aoc".to_string()),
            default_agent: read_default(&agent_default_path())
                .unwrap_or_else(|| "codex".to_string()),
            rtk_status: RtkStatus::default(),
            config,
            config_path,
            projects_base,
            projects,
            project_filter: String::new(),
            filtered_projects: Vec::new(),
            input_buffer: String::new(),
            input_snapshot: String::new(),
            theme_presets: Vec::new(),
            theme_customs: Vec::new(),
            theme_actions: Vec::new(),
            theme_selection: None,
            session_overrides: SessionOverrides::default(),
            in_zellij: in_zellij(),
            floating_active: is_floating_active(),
            close_on_exit: false,
            pane_rename_remaining: if in_zellij() { 6 } else { 0 },
        };
        app.apply_project_filter();
        app.refresh_rtk_status_quiet();
        app.ensure_selections();
        Ok(app)
    }

    fn ensure_selections(&mut self) {
        ensure_selection(&mut self.defaults_state, 5);
        ensure_selection(&mut self.projects_state, self.filtered_projects.len());
        ensure_selection(&mut self.sessions_state, 4);
        ensure_selection(&mut self.layout_picker_state, layout_options().len());
        ensure_selection(&mut self.agent_picker_state, agent_options().len());
        ensure_selection(
            &mut self.theme_sections_state,
            theme_section_options().len(),
        );
        ensure_selection(&mut self.theme_presets_state, self.theme_presets.len());
        ensure_selection(&mut self.theme_customs_state, self.theme_customs.len());
        ensure_selection(&mut self.theme_actions_state, self.theme_actions.len());
        ensure_selection(&mut self.rtk_actions_state, rtk_action_options().len());
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

    fn refresh_themes(&mut self) {
        match load_theme_presets() {
            Ok(entries) => self.theme_presets = entries,
            Err(err) => {
                self.theme_presets.clear();
                self.set_status(format!("Failed to load presets: {err}"));
            }
        }

        match load_theme_customs() {
            Ok(entries) => self.theme_customs = entries,
            Err(err) => {
                self.theme_customs.clear();
                self.set_status(format!("Failed to load custom themes: {err}"));
            }
        }

        ensure_selection(
            &mut self.theme_sections_state,
            theme_section_options().len(),
        );
        ensure_selection(&mut self.theme_presets_state, self.theme_presets.len());
        ensure_selection(&mut self.theme_customs_state, self.theme_customs.len());
    }

    fn open_theme_manager(&mut self) {
        self.refresh_themes();
        self.mode = Mode::ThemeSections;
    }

    fn open_theme_presets(&mut self) {
        self.refresh_themes();
        if self.theme_presets.is_empty() {
            self.set_status("No preset themes found");
            return;
        }
        ensure_selection(&mut self.theme_presets_state, self.theme_presets.len());
        self.mode = Mode::ThemePresets;
    }

    fn open_theme_customs(&mut self) {
        self.refresh_themes();
        if self.theme_customs.is_empty() {
            self.set_status("No custom themes found");
            return;
        }
        ensure_selection(&mut self.theme_customs_state, self.theme_customs.len());
        self.mode = Mode::ThemeCustoms;
    }

    fn open_theme_actions(&mut self, selection: ThemeSelection) {
        self.theme_actions = theme_action_options(selection.source);
        self.theme_selection = Some(selection);
        ensure_selection(&mut self.theme_actions_state, self.theme_actions.len());
        self.mode = Mode::ThemeActions;
    }

    fn back_from_theme_actions(&mut self) {
        if let Some(selection) = &self.theme_selection {
            self.mode = match selection.source {
                ThemeSource::Preset => Mode::ThemePresets,
                ThemeSource::Custom => Mode::ThemeCustoms,
            };
        } else {
            self.mode = Mode::ThemeSections;
        }
    }

    fn commit_new_theme(&mut self) {
        let theme_name = self.input_buffer.trim().to_string();
        if theme_name.is_empty() {
            self.set_status("Theme name cannot be empty");
            return;
        }

        match run_theme_command(&["init", "--name", &theme_name]) {
            Ok(message) => {
                self.set_status(if message.is_empty() {
                    format!("Created global theme '{theme_name}'")
                } else {
                    message
                });
                self.input_buffer.clear();
                self.input_snapshot.clear();
                self.refresh_themes();
                self.mode = Mode::ThemeSections;
            }
            Err(err) => self.set_status(format!("Theme init failed: {err}")),
        }
    }

    fn install_all_presets(&mut self) {
        match run_theme_command(&["presets", "install", "--all"]) {
            Ok(message) => {
                if message.is_empty() {
                    self.set_status("Installed preset themes");
                } else {
                    self.set_status(message);
                }
                self.refresh_themes();
            }
            Err(err) => self.set_status(format!("Preset install failed: {err}")),
        }
    }

    fn run_selected_theme_action(&mut self) {
        let Some(selection) = self.theme_selection.clone() else {
            self.mode = Mode::ThemeSections;
            return;
        };
        let index = self.theme_actions_state.selected().unwrap_or(0);

        let result = match (selection.source, index) {
            (ThemeSource::Preset, 0) => run_preset_apply(&selection.name),
            (ThemeSource::Preset, 1) => run_preset_set_default(&selection.name),
            (ThemeSource::Preset, 2) => run_preset_apply_and_set_default(&selection.name),
            (ThemeSource::Preset, 3) => run_preset_install_only(&selection.name),
            (ThemeSource::Preset, 4) => {
                self.back_from_theme_actions();
                return;
            }
            (ThemeSource::Custom, 0) => run_custom_apply(&selection.name),
            (ThemeSource::Custom, 1) => run_custom_set_default(&selection.name),
            (ThemeSource::Custom, 2) => run_custom_apply_and_set_default(&selection.name),
            (ThemeSource::Custom, 3) => {
                self.back_from_theme_actions();
                return;
            }
            _ => return,
        };

        match result {
            Ok(message) => {
                self.set_status(message);
                self.refresh_themes();
            }
            Err(err) => self.set_status(format!("Theme action failed: {err}")),
        }
    }

    fn refresh_rtk_status_quiet(&mut self) {
        if let Ok(status) = load_rtk_status() {
            self.rtk_status = status;
        }
    }

    fn refresh_rtk_status(&mut self) {
        match load_rtk_status() {
            Ok(status) => {
                self.rtk_status = status;
                self.set_status(format!("RTK: {}", rtk_summary(&self.rtk_status)));
            }
            Err(err) => self.set_status(format!("RTK status failed: {err}")),
        }
    }

    fn open_rtk_actions(&mut self) {
        self.refresh_rtk_status_quiet();
        ensure_selection(&mut self.rtk_actions_state, rtk_action_options().len());
        self.mode = Mode::RtkActions;
    }

    fn run_selected_rtk_action(&mut self) {
        match self.rtk_actions_state.selected().unwrap_or(0) {
            0 => self.refresh_rtk_status(),
            1 => match run_rtk_command(&["install", "--auto"]) {
                Ok(msg) => {
                    self.set_status(msg);
                    self.refresh_rtk_status_quiet();
                }
                Err(err) => self.set_status(format!("RTK install failed: {err}")),
            },
            2 => match run_rtk_command(&["enable"]) {
                Ok(msg) => {
                    self.set_status(msg);
                    self.refresh_rtk_status_quiet();
                }
                Err(err) => self.set_status(format!("RTK enable failed: {err}")),
            },
            3 => match run_rtk_command(&["disable"]) {
                Ok(msg) => {
                    self.set_status(msg);
                    self.refresh_rtk_status_quiet();
                }
                Err(err) => self.set_status(format!("RTK disable failed: {err}")),
            },
            4 => match run_rtk_command(&["doctor"]) {
                Ok(msg) => {
                    self.set_status(msg);
                    self.refresh_rtk_status_quiet();
                }
                Err(err) => self.set_status(format!("RTK doctor failed: {err}")),
            },
            5 => self.mode = Mode::Normal,
            _ => {}
        }
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
        Mode::NewTheme => handle_key_input(app, key, InputMode::NewTheme),
        Mode::ThemeSections => handle_key_theme_sections(app, key),
        Mode::ThemePresets => handle_key_theme_presets(app, key),
        Mode::ThemeCustoms => handle_key_theme_customs(app, key),
        Mode::ThemeActions => handle_key_theme_actions(app, key),
        Mode::RtkActions => handle_key_rtk_actions(app, key),
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
        KeyCode::Char('t') if app.active_tab == Tab::Defaults && app.focus == Focus::Detail => {
            app.open_theme_manager()
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
    NewTheme,
}

fn handle_key_input(app: &mut App, key: KeyEvent, mode: InputMode) {
    match key.code {
        KeyCode::Esc => {
            app.input_buffer = app.input_snapshot.clone();
            if matches!(mode, InputMode::NewTheme) {
                app.input_buffer.clear();
                app.input_snapshot.clear();
                app.mode = Mode::ThemeSections;
            } else {
                app.cancel_input();
            }
        }
        KeyCode::Enter => match mode {
            InputMode::ProjectsBase => app.commit_projects_base(),
            InputMode::Search => app.commit_search(),
            InputMode::NewProject => app.commit_new_project(),
            InputMode::NewTheme => app.commit_new_theme(),
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

fn handle_key_theme_sections(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char('j') | KeyCode::Down => {
            list_next_state(&mut app.theme_sections_state, theme_section_options().len())
        }
        KeyCode::Char('k') | KeyCode::Up => {
            list_prev_state(&mut app.theme_sections_state, theme_section_options().len())
        }
        KeyCode::Enter => match app.theme_sections_state.selected().unwrap_or(0) {
            0 => app.open_theme_presets(),
            1 => app.open_theme_customs(),
            2 => app.start_input(Mode::NewTheme, String::new()),
            3 => app.install_all_presets(),
            4 => app.mode = Mode::Normal,
            _ => {}
        },
        _ => {}
    }
}

fn handle_key_theme_presets(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::ThemeSections,
        KeyCode::Char('j') | KeyCode::Down => {
            list_next_state(&mut app.theme_presets_state, app.theme_presets.len())
        }
        KeyCode::Char('k') | KeyCode::Up => {
            list_prev_state(&mut app.theme_presets_state, app.theme_presets.len())
        }
        KeyCode::Enter => {
            let index = app.theme_presets_state.selected().unwrap_or(0);
            if let Some(entry) = app.theme_presets.get(index).cloned() {
                app.open_theme_actions(ThemeSelection {
                    name: entry.name,
                    source: ThemeSource::Preset,
                });
            }
        }
        _ => {}
    }
}

fn handle_key_theme_customs(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::ThemeSections,
        KeyCode::Char('j') | KeyCode::Down => {
            list_next_state(&mut app.theme_customs_state, app.theme_customs.len())
        }
        KeyCode::Char('k') | KeyCode::Up => {
            list_prev_state(&mut app.theme_customs_state, app.theme_customs.len())
        }
        KeyCode::Enter => {
            let index = app.theme_customs_state.selected().unwrap_or(0);
            if let Some(name) = app.theme_customs.get(index).cloned() {
                app.open_theme_actions(ThemeSelection {
                    name,
                    source: ThemeSource::Custom,
                });
            }
        }
        _ => {}
    }
}

fn handle_key_theme_actions(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.back_from_theme_actions(),
        KeyCode::Char('j') | KeyCode::Down => {
            list_next_state(&mut app.theme_actions_state, app.theme_actions.len())
        }
        KeyCode::Char('k') | KeyCode::Up => {
            list_prev_state(&mut app.theme_actions_state, app.theme_actions.len())
        }
        KeyCode::Enter => app.run_selected_theme_action(),
        _ => {}
    }
}

fn handle_key_rtk_actions(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char('j') | KeyCode::Down => {
            list_next_state(&mut app.rtk_actions_state, rtk_action_options().len())
        }
        KeyCode::Char('k') | KeyCode::Up => {
            list_prev_state(&mut app.rtk_actions_state, rtk_action_options().len())
        }
        KeyCode::Enter => app.run_selected_rtk_action(),
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
        Tab::Defaults => list_next_state(&mut app.defaults_state, 5),
        Tab::Projects => list_next_state(&mut app.projects_state, app.filtered_projects.len()),
        Tab::Sessions => list_next_state(&mut app.sessions_state, 4),
    }
}

fn list_prev(app: &mut App) {
    match app.active_tab {
        Tab::Defaults => list_prev_state(&mut app.defaults_state, 5),
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
            2 => app.open_theme_manager(),
            3 => app.start_input(
                Mode::EditProjectsBase,
                app.projects_base.to_string_lossy().to_string(),
            ),
            4 => app.open_rtk_actions(),
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
        ListItem::new("Theme manager"),
        ListItem::new(format!(
            "Projects base: {}",
            app.projects_base.to_string_lossy()
        )),
        ListItem::new(format!("RTK routing: {}", rtk_summary(&app.rtk_status))),
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
        Mode::NewTheme => {
            draw_input_modal(frame, area, "New theme (kebab-case)", &app.input_buffer)
        }
        Mode::ThemeSections => {
            let items: Vec<ListItem> = theme_section_options()
                .into_iter()
                .map(ListItem::new)
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Theme Manager"),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, area, &mut app.theme_sections_state);
        }
        Mode::ThemePresets => {
            let items: Vec<ListItem> = app
                .theme_presets
                .iter()
                .map(|entry| {
                    let status = if entry.installed {
                        "installed"
                    } else {
                        "available"
                    };
                    ListItem::new(format!("{} ({status})", entry.name))
                })
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Preset Themes"),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, area, &mut app.theme_presets_state);
        }
        Mode::ThemeCustoms => {
            let items: Vec<ListItem> = app
                .theme_customs
                .iter()
                .map(|name| ListItem::new(name.as_str()))
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Custom Themes"),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, area, &mut app.theme_customs_state);
        }
        Mode::ThemeActions => {
            let target = app
                .theme_selection
                .as_ref()
                .map(|selection| selection.name.as_str())
                .unwrap_or("Theme");
            let items: Vec<ListItem> = app
                .theme_actions
                .iter()
                .map(|action| ListItem::new(action.as_str()))
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("Theme Actions: {target}")),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, area, &mut app.theme_actions_state);
        }
        Mode::RtkActions => {
            let title = format!("RTK Controls ({})", rtk_summary(&app.rtk_status));
            let items: Vec<ListItem> = rtk_action_options()
                .into_iter()
                .map(ListItem::new)
                .collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, area, &mut app.rtk_actions_state);
        }
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
        Line::from("  Enter  change layout/agent/theme/base"),
        Line::from("  t      open theme manager"),
        Line::from("  Enter on RTK routing  open RTK setup/actions"),
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
        Mode::EditProjectsBase | Mode::SearchProjects | Mode::NewProject | Mode::NewTheme => vec![
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
        Mode::ThemeSections => vec![
            keycap("Enter"),
            Span::raw(" select  "),
            keycap("Esc"),
            Span::raw(" close"),
        ],
        Mode::ThemePresets | Mode::ThemeCustoms | Mode::ThemeActions => vec![
            keycap("Enter"),
            Span::raw(" choose  "),
            keycap("Esc"),
            Span::raw(" back"),
        ],
        Mode::RtkActions => vec![
            keycap("Enter"),
            Span::raw(" run action  "),
            keycap("Esc"),
            Span::raw(" close"),
        ],
        Mode::Normal => match app.active_tab {
            Tab::Defaults => vec![
                keycap("Enter"),
                Span::raw(" adjust settings/RTK  "),
                keycap("t"),
                Span::raw(" theme manager"),
            ],
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

fn theme_section_options() -> Vec<String> {
    vec![
        "Preset themes".to_string(),
        "Custom global themes".to_string(),
        "Create custom global theme".to_string(),
        "Install all preset themes".to_string(),
        "Back".to_string(),
    ]
}

fn theme_action_options(source: ThemeSource) -> Vec<String> {
    match source {
        ThemeSource::Preset => vec![
            "Apply now (live)".to_string(),
            "Set as default".to_string(),
            "Apply now + set default".to_string(),
            "Install preset only".to_string(),
            "Back".to_string(),
        ],
        ThemeSource::Custom => vec![
            "Apply now (live)".to_string(),
            "Set as default".to_string(),
            "Apply now + set default".to_string(),
            "Back".to_string(),
        ],
    }
}

fn rtk_action_options() -> Vec<String> {
    vec![
        "Refresh status".to_string(),
        "Install RTK (auto-fetch)".to_string(),
        "Enable routing".to_string(),
        "Disable routing".to_string(),
        "Run doctor".to_string(),
        "Back".to_string(),
    ]
}

fn rtk_summary(status: &RtkStatus) -> String {
    let mode = if status.mode == "on" { "on" } else { "off" };
    let install = if status.installed {
        "installed"
    } else {
        "missing"
    };
    let fail_open = if status.fail_open {
        "fail-open"
    } else {
        "strict"
    };
    format!("{mode}, {install}, {fail_open}")
}

fn preset_theme_names() -> &'static [&'static str] {
    &[
        "catppuccin",
        "dracula",
        "everforest",
        "gruvbox",
        "kanagawa",
        "monokai",
        "nord",
        "onedark",
        "rose-pine",
        "solarized-dark",
        "solarized-light",
        "tokyo-night",
    ]
}

fn is_preset_theme(name: &str) -> bool {
    preset_theme_names().iter().any(|preset| *preset == name)
}

fn load_theme_presets() -> io::Result<Vec<ThemePresetEntry>> {
    let output = Command::new("aoc-theme")
        .args(["presets", "list"])
        .output()?;
    if !output.status.success() {
        return Err(command_failure("aoc-theme presets list", &output));
    }

    let mut entries = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 || parts[0] != "preset" {
            continue;
        }
        entries.push(ThemePresetEntry {
            name: parts[1].to_string(),
            installed: parts[2] == "installed",
        });
    }
    Ok(entries)
}

fn load_theme_customs() -> io::Result<Vec<String>> {
    let output = Command::new("aoc-theme").arg("list").output()?;
    if !output.status.success() {
        return Err(command_failure("aoc-theme list", &output));
    }

    let mut themes = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 || parts[0] != "global" {
            continue;
        }
        let name = parts[1];
        if name == "(none)" || is_preset_theme(name) {
            continue;
        }
        themes.push(name.to_string());
    }
    Ok(themes)
}

fn run_theme_command(args: &[&str]) -> io::Result<String> {
    let output = Command::new("aoc-theme").args(args).output()?;
    if !output.status.success() {
        let rendered = if args.is_empty() {
            "aoc-theme".to_string()
        } else {
            format!("aoc-theme {}", args.join(" "))
        };
        return Err(command_failure(&rendered, &output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(first_line)
}

fn parse_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn binary_in_path(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return false;
    }

    let direct = PathBuf::from(trimmed);
    if direct.components().count() > 1 {
        return fs::metadata(direct)
            .map(|meta| meta.is_file())
            .unwrap_or(false);
    }

    let Some(path_os) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path_os).any(|dir| {
        let candidate = dir.join(trimmed);
        fs::metadata(candidate)
            .map(|meta| meta.is_file())
            .unwrap_or(false)
    })
}

fn load_rtk_status() -> io::Result<RtkStatus> {
    let output = Command::new("aoc-rtk")
        .args(["status", "--shell"])
        .output()?;
    if !output.status.success() {
        return Err(command_failure("aoc-rtk status --shell", &output));
    }

    let mut status = RtkStatus::default();
    let mut binary_name = "rtk".to_string();

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key {
            "mode" => status.mode = value.to_string(),
            "config_exists" => status.config_exists = parse_truthy(value),
            "fail_open" => status.fail_open = parse_truthy(value),
            "config_path" => status.config_path = value.to_string(),
            "binary" => binary_name = value.to_string(),
            "allow" => status.allow_count += 1,
            _ => {}
        }
    }

    if status.mode != "on" {
        status.mode = "off".to_string();
    }
    status.installed = binary_in_path(&binary_name);
    Ok(status)
}

fn run_rtk_command(args: &[&str]) -> io::Result<String> {
    let output = Command::new("aoc-rtk").args(args).output()?;
    if !output.status.success() {
        let rendered = if args.is_empty() {
            "aoc-rtk".to_string()
        } else {
            format!("aoc-rtk {}", args.join(" "))
        };
        return Err(command_failure(&rendered, &output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("RTK action completed")
        .trim()
        .to_string();
    Ok(first_line)
}

fn run_theme_command_interactive(args: &[&str]) -> io::Result<()> {
    let status = Command::new("aoc-theme").args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        let rendered = if args.is_empty() {
            "aoc-theme".to_string()
        } else {
            format!("aoc-theme {}", args.join(" "))
        };
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{rendered} exited with status {status}"),
        ))
    }
}

fn run_preset_install_only(theme_name: &str) -> io::Result<String> {
    let _ = run_theme_command(&["presets", "install", "--name", theme_name])?;
    Ok(format!("Installed preset '{theme_name}'"))
}

fn run_preset_apply(theme_name: &str) -> io::Result<String> {
    let _ = run_theme_command(&["presets", "install", "--name", theme_name])?;
    run_theme_command_interactive(&["apply", "--name", theme_name])?;
    Ok(format!("Applied preset '{theme_name}'"))
}

fn run_preset_set_default(theme_name: &str) -> io::Result<String> {
    let _ = run_theme_command(&["presets", "install", "--name", theme_name])?;
    let _ = run_theme_command(&["set-default", "--name", theme_name])?;
    Ok(format!("Set default theme '{theme_name}'"))
}

fn run_preset_apply_and_set_default(theme_name: &str) -> io::Result<String> {
    let _ = run_theme_command(&["presets", "install", "--name", theme_name])?;
    run_theme_command_interactive(&["apply", "--name", theme_name])?;
    let _ = run_theme_command(&["set-default", "--name", theme_name])?;
    Ok(format!("Applied and set default theme '{theme_name}'"))
}

fn run_custom_apply(theme_name: &str) -> io::Result<String> {
    run_theme_command_interactive(&["apply", "--name", theme_name])?;
    Ok(format!("Applied custom theme '{theme_name}'"))
}

fn run_custom_set_default(theme_name: &str) -> io::Result<String> {
    let _ = run_theme_command(&["set-default", "--name", theme_name])?;
    Ok(format!("Set default theme '{theme_name}'"))
}

fn run_custom_apply_and_set_default(theme_name: &str) -> io::Result<String> {
    run_theme_command_interactive(&["apply", "--name", theme_name])?;
    let _ = run_theme_command(&["set-default", "--name", theme_name])?;
    Ok(format!("Applied and set default theme '{theme_name}'"))
}

fn command_failure(command: &str, output: &std::process::Output) -> io::Error {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("{command} exited with status {}", output.status)
    };
    io::Error::new(io::ErrorKind::Other, details)
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
    if let Some(project_root) = find_project_root() {
        append_layout_options(&project_root.join(".aoc/layouts"), &mut options);
    }
    append_layout_options(&config_dir().join("zellij/layouts"), &mut options);
    options.sort();
    options.dedup();
    options
}

fn append_layout_options(layouts_dir: &Path, options: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(layouts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "kdl") {
                if let Some(name) = path.file_stem() {
                    options.push(name.to_string_lossy().to_string());
                }
            }
        }
    }
}

fn find_project_root() -> Option<PathBuf> {
    if let Ok(value) = env::var("AOC_PROJECT_ROOT") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            let root = PathBuf::from(trimmed);
            if root.join(".aoc").is_dir() {
                return Some(root);
            }
        }
    }

    let mut probe = env::current_dir().ok()?;
    loop {
        if probe.join(".aoc").is_dir() {
            return Some(probe);
        }
        if !probe.pop() {
            break;
        }
    }
    None
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
