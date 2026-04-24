use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseEvent,
        MouseEventKind,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, is_raw_mode_enabled, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
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
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::{
    env, fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct AocConfig {
    projects_base: Option<String>,
    pi_compaction_context_window: Option<u64>,
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
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsSection {
    Root,
    Theme,
    ThemeManager,
    Layout,
    Tools,
    ToolsPiCompaction,
    ToolsAgentBrowser,
    ToolsVercel,
    ToolsHyperframes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    Normal,
    PickLayout(PickTarget),
    PickAgent(PickTarget),
    PickBackgroundProfile,
    EditProjectsBase,
    SearchProjects,
    NewProject,
    NewProjectLayout,
    NewGlobalLayout,
    NewTheme,
    RtkActions,
    AgentInstallActions,
    PiCompactionActions,
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

#[derive(Debug)]
struct AgentBrowserJob {
    action: String,
    log_path: PathBuf,
    child: Child,
}

#[derive(Debug)]
struct SearchJob {
    action: String,
    success_message: String,
    log_path: PathBuf,
    child: Child,
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
struct ThemeListEntry {
    name: String,
    source: ThemeSource,
    installed: bool,
}

#[derive(Clone, Copy, Debug)]
struct ThemePalette {
    fg: Color,
    bg: Color,
    black: Color,
    red: Color,
    green: Color,
    yellow: Color,
    blue: Color,
    magenta: Color,
    cyan: Color,
    white: Color,
    orange: Color,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            fg: Color::Rgb(205, 214, 244),
            bg: Color::Rgb(30, 30, 46),
            black: Color::Rgb(108, 112, 134),
            red: Color::Rgb(243, 139, 168),
            green: Color::Rgb(166, 227, 161),
            yellow: Color::Rgb(249, 226, 175),
            blue: Color::Rgb(137, 180, 250),
            magenta: Color::Rgb(203, 166, 247),
            cyan: Color::Rgb(148, 226, 213),
            white: Color::Rgb(147, 153, 178),
            orange: Color::Rgb(250, 179, 135),
        }
    }
}

#[derive(Clone, Debug)]
struct PendingThemePreview {
    source: ThemeSource,
    name: String,
    due_at: Instant,
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

#[derive(Clone, Debug)]
struct AgentInstallEntry {
    id: String,
    label: String,
    installed: bool,
}

#[derive(Clone, Debug)]
struct PiCompactionStatus {
    enabled: bool,
    reserve_tokens: u64,
    keep_recent_tokens: u64,
    project_override: bool,
}

impl Default for PiCompactionStatus {
    fn default() -> Self {
        Self {
            enabled: true,
            reserve_tokens: 16_384,
            keep_recent_tokens: 20_000,
            project_override: false,
        }
    }
}

#[derive(Clone, Debug)]
struct PiCompactionPreset {
    label: String,
    enabled: bool,
    reserve_tokens: u64,
    keep_recent_tokens: u64,
    description: String,
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

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct SearchStatusSummary {
    configured: bool,
    enabled: bool,
    managed: bool,
    runtime_status: String,
    healthy: bool,
    message: String,
}

impl Default for SearchStatusSummary {
    fn default() -> Self {
        Self {
            configured: false,
            enabled: true,
            managed: false,
            runtime_status: "unconfigured".to_string(),
            healthy: false,
            message: "Search is not configured".to_string(),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct SearchConfig {
    enabled: bool,
    provider: String,
    managed: bool,
    auto_start: bool,
    base_url: String,
    healthcheck_url: String,
    compose_file: String,
    service_name: String,
}

#[derive(Debug)]
enum StartupUpdate {
    Rtk(RtkStatus),
    AgentInstallEntries(Vec<AgentInstallEntry>),
    PiCompactionStatus(PiCompactionStatus),
}

#[derive(Debug)]
struct App {
    active_tab: Tab,
    focus: Focus,
    mode: Mode,
    status: String,
    should_exit: bool,
    pending_launch: Option<PendingLaunch>,
    settings_section: SettingsSection,
    defaults_state: ListState,
    settings_theme_state: ListState,
    settings_layout_state: ListState,
    settings_tools_state: ListState,
    settings_tools_pi_compaction_state: ListState,
    settings_tools_agent_browser_state: ListState,
    settings_tools_vercel_state: ListState,
    settings_tools_hyperframes_state: ListState,
    projects_state: ListState,
    sessions_state: ListState,
    layout_picker_state: ListState,
    agent_picker_state: ListState,
    background_picker_state: ListState,
    theme_manager_state: ListState,
    rtk_actions_state: ListState,
    agent_install_actions_state: ListState,
    pi_compaction_actions_state: ListState,
    default_layout: String,
    default_agent: String,
    active_theme_name: String,
    effective_theme_name: String,
    background_profile: String,
    rtk_status: RtkStatus,
    agent_install_entries: Vec<AgentInstallEntry>,
    pi_compaction_status: PiCompactionStatus,
    pi_compaction_context_window: u64,
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
    theme_entries: Vec<ThemeListEntry>,
    theme_preview_base: Option<String>,
    theme_preview_selected: Option<String>,
    theme_preview_live: Option<String>,
    theme_preview_pending: Option<PendingThemePreview>,
    theme_preview_scroll: u16,
    theme_preview_area: Option<Rect>,
    theme_preview_palette_for: Option<String>,
    theme_preview_palette: ThemePalette,
    theme_preview_palette_error: Option<String>,
    zellij_config_dir: PathBuf,
    session_overrides: SessionOverrides,
    agent_browser_job: Option<AgentBrowserJob>,
    agent_browser_runtime_checked: bool,
    agent_browser_runtime_ready: bool,
    agent_browser_log_tail: Vec<String>,
    agent_browser_log_scroll: usize,
    search_job: Option<SearchJob>,
    search_log_tail: Vec<String>,
    search_log_scroll: usize,
    search_start_last_status: String,
    search_verify_last_status: String,
    search_status_checked: bool,
    search_status: SearchStatusSummary,
    rtk_loaded: bool,
    agent_install_entries_loaded: bool,
    pi_compaction_status_loaded: bool,
    in_zellij: bool,
    floating_active: bool,
    close_on_exit: bool,
    pane_rename_remaining: u8,
    startup_rx: Option<Receiver<StartupUpdate>>,
}

impl App {
    fn new() -> io::Result<Self> {
        let config_path = config_path();
        let config = load_config(&config_path).unwrap_or_default();
        let projects_base = resolve_projects_base(&config);
        let projects = load_projects(&projects_base).unwrap_or_default();
        let default_agent_raw =
            read_default(&agent_default_path()).unwrap_or_else(|| "pi".to_string());
        let default_agent = match default_agent_raw.as_str() {
            "pi" => default_agent_raw,
            _ => "pi".to_string(),
        };
        let mut app = Self {
            active_tab: Tab::Defaults,
            focus: Focus::Nav,
            mode: Mode::Normal,
            status: String::new(),
            should_exit: false,
            pending_launch: None,
            settings_section: SettingsSection::Root,
            defaults_state: ListState::default(),
            settings_theme_state: ListState::default(),
            settings_layout_state: ListState::default(),
            settings_tools_state: ListState::default(),
            settings_tools_pi_compaction_state: ListState::default(),
            settings_tools_agent_browser_state: ListState::default(),
            settings_tools_vercel_state: ListState::default(),
            settings_tools_hyperframes_state: ListState::default(),
            projects_state: ListState::default(),
            sessions_state: ListState::default(),
            layout_picker_state: ListState::default(),
            agent_picker_state: ListState::default(),
            background_picker_state: ListState::default(),
            theme_manager_state: ListState::default(),
            rtk_actions_state: ListState::default(),
            agent_install_actions_state: ListState::default(),
            pi_compaction_actions_state: ListState::default(),
            default_layout: read_default(&layout_default_path())
                .unwrap_or_else(|| "aoc".to_string()),
            default_agent,
            active_theme_name: load_active_theme_name()
                .ok()
                .flatten()
                .unwrap_or_else(|| "unknown".to_string()),
            effective_theme_name: load_effective_theme_name()
                .ok()
                .flatten()
                .or_else(|| load_active_theme_name().ok().flatten())
                .unwrap_or_else(|| "unknown".to_string()),
            background_profile: load_background_profile_name()
                .unwrap_or_else(|_| "follow-theme".to_string()),
            rtk_status: RtkStatus::default(),
            agent_install_entries: Vec::new(),
            pi_compaction_status: PiCompactionStatus::default(),
            pi_compaction_context_window: resolve_pi_compaction_context_window(&config),
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
            theme_entries: Vec::new(),
            theme_preview_base: None,
            theme_preview_selected: None,
            theme_preview_live: None,
            theme_preview_pending: None,
            theme_preview_scroll: 0,
            theme_preview_area: None,
            theme_preview_palette_for: None,
            theme_preview_palette: ThemePalette::default(),
            theme_preview_palette_error: None,
            zellij_config_dir: resolve_zellij_config_dir(),
            session_overrides: SessionOverrides::default(),
            agent_browser_job: None,
            agent_browser_runtime_checked: false,
            agent_browser_runtime_ready: false,
            agent_browser_log_tail: Vec::new(),
            agent_browser_log_scroll: 0,
            search_job: None,
            search_log_tail: Vec::new(),
            search_log_scroll: 0,
            search_start_last_status: "idle".to_string(),
            search_verify_last_status: "idle".to_string(),
            search_status_checked: false,
            search_status: SearchStatusSummary::default(),
            rtk_loaded: false,
            agent_install_entries_loaded: false,
            pi_compaction_status_loaded: false,
            in_zellij: in_zellij(),
            floating_active: is_floating_active(),
            close_on_exit: false,
            pane_rename_remaining: if in_zellij() { 6 } else { 0 },
            startup_rx: None,
        };
        app.apply_project_filter();
        app.refresh_theme_identity_quiet();
        app.start_background_startup_refresh();
        app.ensure_selections();
        Ok(app)
    }

    fn start_background_startup_refresh(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.startup_rx = Some(rx);
        thread::spawn(move || {
            if let Ok(status) = load_rtk_status() {
                let _ = tx.send(StartupUpdate::Rtk(status));
            }
            let _ = tx.send(StartupUpdate::AgentInstallEntries(
                load_agent_install_entries(),
            ));
            if let Ok(status) = load_pi_compaction_status() {
                let _ = tx.send(StartupUpdate::PiCompactionStatus(status));
            }
        });
    }

    fn poll_startup_refresh(&mut self) {
        let mut disconnected = false;
        if let Some(rx) = self.startup_rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(StartupUpdate::Rtk(status)) => {
                        self.rtk_status = status;
                        self.rtk_loaded = true;
                    }
                    Ok(StartupUpdate::AgentInstallEntries(entries)) => {
                        self.agent_install_entries = entries;
                        self.agent_install_entries_loaded = true;
                    }
                    Ok(StartupUpdate::PiCompactionStatus(status)) => {
                        self.pi_compaction_status = status;
                        self.pi_compaction_status_loaded = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        if disconnected {
            self.startup_rx = None;
        }
    }

    fn ensure_selections(&mut self) {
        ensure_selection(&mut self.defaults_state, settings_root_options().len());
        ensure_selection(
            &mut self.settings_theme_state,
            settings_theme_options().len(),
        );
        ensure_selection(
            &mut self.settings_layout_state,
            settings_layout_options().len(),
        );
        ensure_selection(
            &mut self.settings_tools_state,
            settings_tools_options().len(),
        );
        ensure_selection(
            &mut self.settings_tools_pi_compaction_state,
            settings_tools_pi_compaction_options().len(),
        );
        ensure_selection(
            &mut self.settings_tools_agent_browser_state,
            settings_tools_agent_browser_options().len(),
        );
        ensure_selection(
            &mut self.settings_tools_vercel_state,
            settings_tools_vercel_options().len(),
        );
        ensure_selection(&mut self.projects_state, self.filtered_projects.len());
        ensure_selection(&mut self.sessions_state, 4);
        ensure_selection(&mut self.layout_picker_state, layout_options().len());
        ensure_selection(&mut self.agent_picker_state, agent_options().len());
        ensure_selection(
            &mut self.background_picker_state,
            background_profile_options().len(),
        );
        ensure_selection(&mut self.theme_manager_state, self.theme_entries.len());
        ensure_selection(&mut self.rtk_actions_state, rtk_action_options().len());
        ensure_selection(
            &mut self.agent_install_actions_state,
            self.agent_install_entries.len(),
        );
        ensure_selection(
            &mut self.pi_compaction_actions_state,
            pi_compaction_presets(self.pi_compaction_context_window).len(),
        );
    }

    fn set_status<S: Into<String>>(&mut self, message: S) {
        self.status = message.into();
    }

    fn refresh_theme_identity_quiet(&mut self) {
        if let Ok(Some(name)) = load_active_theme_name() {
            self.active_theme_name = name;
        }
        if let Ok(Some(effective)) = load_effective_theme_name() {
            self.effective_theme_name = effective;
        } else {
            self.effective_theme_name = self.active_theme_name.clone();
        }
        if let Ok(profile) = load_background_profile_name() {
            self.background_profile = profile;
        }
    }

    fn theme_identity_label(&self) -> String {
        if self.active_theme_name == self.effective_theme_name {
            self.active_theme_name.clone()
        } else {
            format!(
                "{} -> {}",
                self.active_theme_name, self.effective_theme_name
            )
        }
    }

    fn set_settings_section(&mut self, section: SettingsSection) {
        self.settings_section = section;
        if self.settings_section == SettingsSection::ToolsPiCompaction {
            self.refresh_pi_compaction_status_quiet();
        }
        if self.settings_section == SettingsSection::ToolsAgentBrowser {
            self.refresh_agent_browser_runtime_quiet();
            self.refresh_search_status_quiet();
        }
        match self.settings_section {
            SettingsSection::Root => {
                ensure_selection(&mut self.defaults_state, settings_root_options().len())
            }
            SettingsSection::Theme => ensure_selection(
                &mut self.settings_theme_state,
                settings_theme_options().len(),
            ),
            SettingsSection::ThemeManager => {
                ensure_selection(&mut self.theme_manager_state, self.theme_entries.len())
            }
            SettingsSection::Layout => ensure_selection(
                &mut self.settings_layout_state,
                settings_layout_options().len(),
            ),
            SettingsSection::Tools => ensure_selection(
                &mut self.settings_tools_state,
                settings_tools_options().len(),
            ),
            SettingsSection::ToolsPiCompaction => ensure_selection(
                &mut self.settings_tools_pi_compaction_state,
                settings_tools_pi_compaction_options().len(),
            ),
            SettingsSection::ToolsAgentBrowser => ensure_selection(
                &mut self.settings_tools_agent_browser_state,
                settings_tools_agent_browser_options().len(),
            ),
            SettingsSection::ToolsVercel => ensure_selection(
                &mut self.settings_tools_vercel_state,
                settings_tools_vercel_options().len(),
            ),
            SettingsSection::ToolsHyperframes => ensure_selection(
                &mut self.settings_tools_hyperframes_state,
                settings_tools_hyperframes_options().len(),
            ),
        }
    }

    fn back_settings_section(&mut self) {
        if self.settings_section == SettingsSection::ThemeManager {
            self.end_theme_preview();
            self.theme_preview_scroll = 0;
        }

        let target = match self.settings_section {
            SettingsSection::Root => SettingsSection::Root,
            SettingsSection::Theme | SettingsSection::Layout | SettingsSection::Tools => {
                SettingsSection::Root
            }
            SettingsSection::ThemeManager => SettingsSection::Theme,
            SettingsSection::ToolsPiCompaction
            | SettingsSection::ToolsAgentBrowser
            | SettingsSection::ToolsVercel
            | SettingsSection::ToolsHyperframes => SettingsSection::Tools,
        };
        self.set_settings_section(target);
    }

    fn selected_settings_index(&self) -> usize {
        match self.settings_section {
            SettingsSection::Root => self.defaults_state.selected().unwrap_or(0),
            SettingsSection::Theme => self.settings_theme_state.selected().unwrap_or(0),
            SettingsSection::ThemeManager => self.theme_manager_state.selected().unwrap_or(0),
            SettingsSection::Layout => self.settings_layout_state.selected().unwrap_or(0),
            SettingsSection::Tools => self.settings_tools_state.selected().unwrap_or(0),
            SettingsSection::ToolsPiCompaction => self
                .settings_tools_pi_compaction_state
                .selected()
                .unwrap_or(0),
            SettingsSection::ToolsAgentBrowser => self
                .settings_tools_agent_browser_state
                .selected()
                .unwrap_or(0),
            SettingsSection::ToolsVercel => {
                self.settings_tools_vercel_state.selected().unwrap_or(0)
            }
            SettingsSection::ToolsHyperframes => {
                self.settings_tools_hyperframes_state.selected().unwrap_or(0)
            }
        }
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
            if let Err(err) =
                run_open_in_zellij(&path, &self.session_overrides, &self.default_agent)
            {
                self.set_status(format!("Failed to open tab: {err}"));
            } else {
                self.set_status(format!("Opened {}", path.to_string_lossy()));
            }
        } else {
            let envs = build_env_overrides(&self.session_overrides, &self.default_agent);
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
            if let Err(err) = run_open_in_zellij(&cwd, &self.session_overrides, &self.default_agent)
            {
                self.set_status(format!("Failed to open tab: {err}"));
            } else {
                self.set_status("Opened new tab".to_string());
            }
        } else {
            let envs = build_env_overrides(&self.session_overrides, &self.default_agent);
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

    fn open_background_picker(&mut self) {
        let options = background_profile_options();
        select_picker(
            &mut self.background_picker_state,
            &options,
            &self.background_profile,
        );
        self.mode = Mode::PickBackgroundProfile;
    }

    fn set_background_profile(&mut self, profile: String) {
        match run_theme_command(&["background", "set", "--profile", profile.as_str()]) {
            Ok(message) => {
                self.background_profile =
                    load_background_profile_name().unwrap_or_else(|_| profile.clone());
                self.refresh_theme_identity_quiet();
                self.set_status(if message.is_empty() {
                    format!("Background profile set to {}", self.background_profile)
                } else {
                    message
                });
                self.mode = Mode::Normal;
            }
            Err(err) => self.set_status(format!("Background profile update failed: {err}")),
        }
    }

    fn set_override_layout(&mut self, layout: String) {
        self.session_overrides.layout = Some(layout.clone());
        self.set_status(format!("Override layout set to {layout}"));
        self.mode = Mode::Normal;
    }

    fn commit_new_project_layout(&mut self) {
        let layout_name = self.input_buffer.trim().to_string();
        if layout_name.is_empty() {
            self.set_status("Layout name cannot be empty");
            return;
        }
        match run_layout_command_interactive(&[
            "--create",
            layout_name.as_str(),
            "--scope",
            "project",
        ]) {
            Ok(()) => {
                self.set_status(format!(
                    "Created/opened project custom layout '{}'",
                    layout_name
                ));
                self.cancel_input();
            }
            Err(err) => self.set_status(format!("Project custom layout create failed: {err}")),
        }
    }

    fn commit_new_global_layout(&mut self) {
        let layout_name = self.input_buffer.trim().to_string();
        if layout_name.is_empty() {
            self.set_status("Layout name cannot be empty");
            return;
        }
        match run_layout_command_interactive(&[
            "--create",
            layout_name.as_str(),
            "--scope",
            "global",
        ]) {
            Ok(()) => {
                self.set_status(format!(
                    "Created/opened global custom layout '{}'",
                    layout_name
                ));
                self.cancel_input();
            }
            Err(err) => self.set_status(format!("Global custom layout create failed: {err}")),
        }
    }

    fn edit_custom_layout(&mut self, layout: String) {
        match run_layout_command_interactive(&["--edit", layout.as_str(), "--scope", "auto"]) {
            Ok(()) => {
                self.set_status(format!("Opened custom layout '{}'", layout));
                self.mode = Mode::Normal;
            }
            Err(err) => self.set_status(format!("Custom layout edit failed: {err}")),
        }
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

        self.theme_entries = self
            .theme_presets
            .iter()
            .map(|entry| ThemeListEntry {
                name: entry.name.clone(),
                source: ThemeSource::Preset,
                installed: entry.installed,
            })
            .chain(self.theme_customs.iter().map(|name| ThemeListEntry {
                name: name.clone(),
                source: ThemeSource::Custom,
                installed: true,
            }))
            .collect();

        ensure_selection(&mut self.theme_manager_state, self.theme_entries.len());
    }

    fn open_theme_manager(&mut self) {
        self.theme_preview_base = None;
        self.theme_preview_selected = None;
        self.theme_preview_live = None;
        self.theme_preview_pending = None;
        self.theme_preview_scroll = 0;
        self.refresh_themes();

        if let Some(active) = load_active_theme_name().ok().flatten() {
            if let Some(index) = self
                .theme_entries
                .iter()
                .position(|entry| entry.name == active)
            {
                self.theme_manager_state.select(Some(index));
            }
        }

        self.set_settings_section(SettingsSection::ThemeManager);
        self.begin_theme_preview();
        self.queue_preview_selected_theme();
    }

    fn selected_theme_entry(&self) -> Option<ThemeListEntry> {
        let index = self.theme_manager_state.selected().unwrap_or(0);
        self.theme_entries.get(index).cloned()
    }

    fn refresh_selected_theme_palette(&mut self) {
        let Some(entry) = self.selected_theme_entry() else {
            self.theme_preview_palette_for = None;
            self.theme_preview_palette = ThemePalette::default();
            self.theme_preview_palette_error = None;
            return;
        };

        if self.theme_preview_palette_for.as_deref() == Some(entry.name.as_str()) {
            return;
        }

        self.theme_preview_palette_for = Some(entry.name.clone());
        match load_theme_palette(&self.zellij_config_dir, &entry.name) {
            Ok(palette) => {
                self.theme_preview_palette = palette;
                self.theme_preview_palette_error = None;
            }
            Err(err) => {
                self.theme_preview_palette = ThemePalette::default();
                self.theme_preview_palette_error = Some(err.to_string());
            }
        }
    }

    fn begin_theme_preview(&mut self) {
        if self.theme_preview_base.is_some() {
            return;
        }
        let active = load_active_theme_name().ok().flatten();
        self.theme_preview_base = active.clone();
        self.theme_preview_selected = active;
        self.theme_preview_live = None;
        self.theme_preview_pending = None;
    }

    fn end_theme_preview(&mut self) {
        let fallback = self
            .theme_preview_selected
            .clone()
            .or_else(|| self.theme_preview_base.clone());

        if let (Some(live_name), Some(theme_name)) = (self.theme_preview_live.clone(), fallback) {
            if live_name != theme_name {
                if let Err(err) = run_theme_apply_quiet(&theme_name) {
                    self.set_status(format!("Theme preview restore failed: {err}"));
                }
            }
        }

        self.theme_preview_base = None;
        self.theme_preview_selected = None;
        self.theme_preview_live = None;
        self.theme_preview_pending = None;
    }

    fn preview_theme_name(&mut self, source: ThemeSource, theme_name: &str) -> io::Result<()> {
        if matches!(source, ThemeSource::Preset) {
            let needs_install = self
                .theme_presets
                .iter()
                .find(|entry| entry.name == theme_name)
                .map(|entry| !entry.installed)
                .unwrap_or(false);
            if needs_install {
                let _ = run_theme_command(&["presets", "install", "--name", theme_name])?;
                if let Some(entry) = self
                    .theme_presets
                    .iter_mut()
                    .find(|entry| entry.name == theme_name)
                {
                    entry.installed = true;
                }
                self.refresh_themes();
            }
        }

        if self.theme_preview_live.as_deref() == Some(theme_name) {
            return Ok(());
        }

        run_theme_apply_quiet(theme_name)?;
        self.theme_preview_live = Some(theme_name.to_string());
        Ok(())
    }

    fn queue_preview_selected_theme(&mut self) {
        self.refresh_selected_theme_palette();

        let Some(entry) = self.selected_theme_entry() else {
            return;
        };

        if self.theme_preview_live.as_deref() == Some(entry.name.as_str()) {
            return;
        }

        self.theme_preview_pending = Some(PendingThemePreview {
            source: entry.source,
            name: entry.name,
            due_at: Instant::now() + Duration::from_millis(90),
        });
    }

    fn flush_pending_theme_preview(&mut self) {
        let Some(pending) = self.theme_preview_pending.clone() else {
            return;
        };

        if Instant::now() < pending.due_at {
            return;
        }

        self.theme_preview_pending = None;

        if let Err(err) = self.preview_theme_name(pending.source, &pending.name) {
            if err.kind() == io::ErrorKind::TimedOut {
                self.set_status("Theme preview busy (timeout); keep navigating");
            } else {
                self.set_status(format!("Theme preview failed: {err}"));
            }
        }
    }

    fn activate_selected_theme(&mut self) {
        let Some(entry) = self.selected_theme_entry() else {
            self.set_status("No theme selected");
            return;
        };

        if matches!(entry.source, ThemeSource::Preset) && !entry.installed {
            if let Err(err) = run_theme_command(&["presets", "install", "--name", &entry.name]) {
                self.set_status(format!("Preset install failed: {err}"));
                return;
            }
        }

        self.theme_preview_pending = None;
        match run_theme_command_interactive(&["activate", "--name", &entry.name]) {
            Ok(()) => {
                self.theme_preview_selected = Some(entry.name.clone());
                self.theme_preview_live = Some(entry.name.clone());
                self.refresh_theme_identity_quiet();
                self.refresh_themes();
                self.set_status(format!("Activated theme '{}'", entry.name));
            }
            Err(err) => self.set_status(format!("Theme activate failed: {err}")),
        }
    }

    fn scroll_theme_preview(&mut self, delta: i16) {
        if delta < 0 {
            self.theme_preview_scroll = self
                .theme_preview_scroll
                .saturating_sub(delta.unsigned_abs());
        } else {
            self.theme_preview_scroll = self
                .theme_preview_scroll
                .saturating_add(delta as u16)
                .min(200);
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
                if let Some(idx) = self
                    .theme_entries
                    .iter()
                    .position(|entry| entry.name == theme_name)
                {
                    self.theme_manager_state.select(Some(idx));
                    self.queue_preview_selected_theme();
                }
                self.mode = Mode::Normal;
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

    fn refresh_rtk_status_quiet(&mut self) {
        if let Ok(status) = load_rtk_status() {
            self.rtk_status = status;
            self.rtk_loaded = true;
        }
    }

    fn refresh_rtk_status(&mut self) {
        match load_rtk_status() {
            Ok(status) => {
                self.rtk_status = status;
                self.rtk_loaded = true;
                self.set_status(format!("RTK: {}", rtk_summary(&self.rtk_status)));
            }
            Err(err) => self.set_status(format!("RTK status failed: {err}")),
        }
    }

    fn open_rtk_actions(&mut self) {
        ensure_selection(&mut self.rtk_actions_state, rtk_action_options().len());
        self.mode = Mode::RtkActions;
    }

    fn refresh_agent_install_statuses_quiet(&mut self) {
        self.agent_install_entries = load_agent_install_entries();
        self.agent_install_entries_loaded = true;
    }

    fn refresh_agent_install_statuses(&mut self) {
        self.refresh_agent_install_statuses_quiet();
        self.set_status(format!(
            "Agent installers: {}",
            agent_install_summary(&self.agent_install_entries)
        ));
    }

    fn refresh_pi_compaction_status_quiet(&mut self) {
        if let Ok(status) = load_pi_compaction_status() {
            self.pi_compaction_status = status;
            self.pi_compaction_status_loaded = true;
        }
    }

    fn refresh_pi_compaction_status(&mut self) {
        match load_pi_compaction_status() {
            Ok(status) => {
                self.pi_compaction_status = status;
                self.pi_compaction_status_loaded = true;
                self.set_status(format!(
                    "PI compaction: {}",
                    pi_compaction_summary(&self.pi_compaction_status)
                ));
            }
            Err(err) => self.set_status(format!("PI compaction status failed: {err}")),
        }
    }

    fn open_pi_compaction_actions(&mut self) {
        ensure_selection(
            &mut self.pi_compaction_actions_state,
            pi_compaction_presets(self.pi_compaction_context_window).len(),
        );
        self.mode = Mode::PiCompactionActions;
    }

    fn apply_selected_pi_compaction_preset(&mut self) {
        let index = self.pi_compaction_actions_state.selected().unwrap_or(0);
        let presets = pi_compaction_presets(self.pi_compaction_context_window);
        let Some(preset) = presets.get(index) else {
            return;
        };

        match save_pi_compaction_status(&PiCompactionStatus {
            enabled: preset.enabled,
            reserve_tokens: preset.reserve_tokens,
            keep_recent_tokens: preset.keep_recent_tokens,
            project_override: self.pi_compaction_status.project_override,
        }) {
            Ok(()) => {
                self.refresh_pi_compaction_status_quiet();
                self.set_status(format!("PI compaction preset applied: {}", preset.label));
            }
            Err(err) => self.set_status(format!("PI compaction preset failed: {err}")),
        }
    }

    fn shift_pi_compaction_context_window(&mut self, delta: isize) {
        let options = pi_compaction_context_window_options();
        let current = self.pi_compaction_context_window;
        let index = options
            .iter()
            .position(|value| *value == current)
            .unwrap_or(0) as isize;
        let next_index =
            (index + delta).clamp(0, options.len().saturating_sub(1) as isize) as usize;
        let next = options[next_index];
        if next == current {
            return;
        }
        self.pi_compaction_context_window = next;
        self.config.pi_compaction_context_window = Some(next);
        match save_config(&self.config_path, &self.config) {
            Ok(()) => self.set_status(format!(
                "PI compaction context window set to {}",
                format_token_count(next)
            )),
            Err(err) => self.set_status(format!("PI compaction context window save failed: {err}")),
        }
        ensure_selection(
            &mut self.pi_compaction_actions_state,
            pi_compaction_presets(self.pi_compaction_context_window).len(),
        );
    }

    fn refresh_agent_browser_runtime_quiet(&mut self) {
        self.agent_browser_runtime_ready = probe_agent_browser_runtime_ready();
        self.agent_browser_runtime_checked = true;
    }

    fn refresh_search_status_quiet(&mut self) {
        self.search_status = load_search_status_via_cli().unwrap_or_else(|_| load_search_status());
        self.search_status_checked = true;
    }

    fn run_aoc_map_init_action(&mut self) {
        match seed_aoc_map() {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("AOC Map seed failed: {err}")),
        }
    }

    fn run_search_enable_action(&mut self) {
        match enable_managed_search() {
            Ok(message) => {
                self.refresh_search_status_quiet();
                self.set_status(message);
            }
            Err(err) => self.set_status(format!("Managed search enable failed: {err}")),
        }
    }

    fn run_search_start_or_verify_action(&mut self) {
        if self.search_job.is_some() {
            self.set_status("Search action already running");
            return;
        }

        self.search_start_last_status = "running".to_string();
        match spawn_search_command(
            "start-verify",
            "Managed search start/verify completed",
            "aoc-search",
            &["start", "--wait"],
        ) {
            Ok(job) => {
                let log_path = job.log_path.to_string_lossy().to_string();
                self.search_log_tail.clear();
                self.search_log_scroll = 0;
                self.search_job = Some(job);
                self.set_status(format!(
                    "Managed search start/verify started (logs: {log_path})"
                ));
            }
            Err(err) => {
                self.search_start_last_status = format!("failed to start: {err}");
                self.set_status(format!("Managed search start/verify failed: {err}"));
            }
        }
    }

    fn run_web_research_verify_action(&mut self) {
        if self.search_job.is_some() {
            self.set_status("Search action already running");
            return;
        }

        self.search_verify_last_status = "running".to_string();
        match spawn_search_command(
            "web-smoke",
            "Web research stack verification passed",
            "aoc-web-smoke",
            &[],
        ) {
            Ok(job) => {
                let log_path = job.log_path.to_string_lossy().to_string();
                self.search_log_tail.clear();
                self.search_log_scroll = 0;
                self.search_job = Some(job);
                self.set_status(format!(
                    "Web research stack verification started (logs: {log_path})"
                ));
            }
            Err(err) => {
                self.search_verify_last_status = format!("failed to start: {err}");
                self.set_status(format!("Web research verification failed: {err}"));
            }
        }
    }

    fn open_agent_install_actions(&mut self) {
        ensure_selection(
            &mut self.agent_install_actions_state,
            self.agent_install_entries.len(),
        );
        self.mode = Mode::AgentInstallActions;
    }

    fn run_selected_agent_install_action(&mut self) {
        let index = self.agent_install_actions_state.selected().unwrap_or(0);
        let Some(entry) = self.agent_install_entries.get(index).cloned() else {
            return;
        };
        let action = if entry.installed { "update" } else { "install" };
        match run_agent_install_command(action, &entry.id) {
            Ok(message) => {
                self.set_status(message);
                self.refresh_agent_install_statuses_quiet();
            }
            Err(err) => self.set_status(format!("{} {} failed: {err}", entry.label, action)),
        }
    }

    fn run_agent_browser_tool_action(&mut self) {
        if self.agent_browser_job.is_some() {
            self.set_status("Agent Browser action already running");
            return;
        }

        let action = if agent_browser_installed() {
            "update"
        } else {
            "install"
        };
        match spawn_agent_browser_command(action) {
            Ok(job) => {
                let log_path = job.log_path.to_string_lossy().to_string();
                self.agent_browser_log_tail.clear();
                self.agent_browser_log_scroll = 0;
                self.agent_browser_job = Some(job);
                self.set_status(format!("Agent Browser {action} started (logs: {log_path})"));
            }
            Err(err) => self.set_status(format!("Agent Browser {action} failed: {err}")),
        }
    }

    fn run_agent_browser_skill_action(&mut self) {
        match install_agent_browser_skill() {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("Agent Browser skill sync failed: {err}")),
        }
    }

    fn run_web_research_skill_action(&mut self) {
        match install_web_research_skill() {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("Web research skill sync failed: {err}")),
        }
    }

    fn run_vercel_tool_action(&mut self) {
        let action = if vercel_installed() {
            "update"
        } else {
            "install"
        };
        match run_vercel_tool_command(action) {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("Vercel CLI {action} failed: {err}")),
        }
    }

    fn run_vercel_skill_action(&mut self) {
        match install_vercel_skill() {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("Vercel skill sync failed: {err}")),
        }
    }

    fn verify_vercel_tool_action(&mut self) {
        match verify_vercel_cli() {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("Vercel CLI verify failed: {err}")),
        }
    }

    fn scroll_agent_browser_log(&mut self, delta: isize) {
        if self.agent_browser_log_tail.is_empty() {
            self.agent_browser_log_scroll = 0;
            return;
        }
        let max_scroll = self.agent_browser_log_tail.len().saturating_sub(1);
        let next = if delta.is_negative() {
            self.agent_browser_log_scroll
                .saturating_sub(delta.unsigned_abs())
        } else {
            self.agent_browser_log_scroll.saturating_add(delta as usize)
        };
        self.agent_browser_log_scroll = next.min(max_scroll);
    }

    fn scroll_search_log(&mut self, delta: isize) {
        if self.search_log_tail.is_empty() {
            self.search_log_scroll = 0;
            return;
        }
        let max_scroll = self.search_log_tail.len().saturating_sub(1);
        let next = if delta.is_negative() {
            self.search_log_scroll.saturating_sub(delta.unsigned_abs())
        } else {
            self.search_log_scroll.saturating_add(delta as usize)
        };
        self.search_log_scroll = next.min(max_scroll);
    }

    fn cancel_agent_browser_job(&mut self) {
        let Some(mut job) = self.agent_browser_job.take() else {
            self.set_status("No Agent Browser action is running");
            return;
        };

        let _ = job.child.kill();
        let _ = job.child.wait();
        if let Ok(lines) = tail_file_lines(&job.log_path, 200, 32 * 1024) {
            self.agent_browser_log_tail = lines;
        }
        self.set_status(format!(
            "Cancelled Agent Browser {} (log: {})",
            job.action,
            job.log_path.to_string_lossy()
        ));
    }

    fn open_agent_browser_log(&mut self) {
        let log_path = self
            .agent_browser_job
            .as_ref()
            .map(|job| job.log_path.clone())
            .or_else(|| latest_agent_browser_log_path());

        let Some(log_path) = log_path else {
            self.set_status("No Agent Browser log available yet");
            return;
        };

        match open_log_in_pager(&log_path) {
            Ok(()) => self.set_status(format!("Viewed log {}", log_path.to_string_lossy())),
            Err(err) => self.set_status(format!("Open log failed: {err}")),
        }
    }

    fn cancel_search_job(&mut self) {
        let Some(mut job) = self.search_job.take() else {
            self.set_status("No search action is running");
            return;
        };

        let action = job.action.clone();
        let _ = job.child.kill();
        let _ = job.child.wait();
        if let Ok(lines) = tail_file_lines(&job.log_path, 200, 32 * 1024) {
            self.search_log_tail = lines;
        }
        self.refresh_search_status_quiet();
        self.set_search_action_last_status(&action, "cancelled".to_string());
        self.set_status(format!(
            "Cancelled search action {} (log: {})",
            job.action,
            job.log_path.to_string_lossy()
        ));
    }

    fn open_search_log(&mut self) {
        let log_path = self
            .search_job
            .as_ref()
            .map(|job| job.log_path.clone())
            .or_else(|| latest_search_log_path());

        let Some(log_path) = log_path else {
            self.set_status("No search log available yet");
            return;
        };

        match open_log_in_pager(&log_path) {
            Ok(()) => self.set_status(format!("Viewed log {}", log_path.to_string_lossy())),
            Err(err) => self.set_status(format!("Open log failed: {err}")),
        }
    }

    fn run_hyperframes_init_action(&mut self) {
        match run_hyperframes_command(&["init"]) {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("aoc-hyperframes init failed: {err}")),
        }
    }

    fn run_hyperframes_sync_skills_action(&mut self) {
        match run_hyperframes_command(&["sync-skills"]) {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("aoc-hyperframes sync-skills failed: {err}")),
        }
    }

    fn run_hyperframes_doctor_action(&mut self) {
        match run_hyperframes_command(&["doctor"]) {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("aoc-hyperframes doctor failed: {err}")),
        }
    }

    fn run_hyperframes_preview_action(&mut self) {
        match open_hyperframes_preview_pane() {
            Ok(message) => self.set_status(message),
            Err(err) => self.set_status(format!("HyperFrames preview failed: {err}")),
        }
    }

    fn poll_agent_browser_job(&mut self) {
        let mut completed: Option<(String, ExitStatus, PathBuf)> = None;

        if let Some(job) = self.agent_browser_job.as_mut() {
            if let Ok(lines) = tail_file_lines(&job.log_path, 200, 32 * 1024) {
                self.agent_browser_log_tail = lines;
                let max_scroll = self.agent_browser_log_tail.len().saturating_sub(1);
                self.agent_browser_log_scroll = self.agent_browser_log_scroll.min(max_scroll);
            }

            match job.child.try_wait() {
                Ok(Some(status)) => {
                    completed = Some((job.action.clone(), status, job.log_path.clone()));
                }
                Ok(None) => {}
                Err(err) => {
                    self.agent_browser_job = None;
                    self.set_status(format!("Agent Browser job poll failed: {err}"));
                    return;
                }
            }
        }

        if let Some((action, status, log_path)) = completed {
            self.agent_browser_job = None;
            if let Ok(lines) = tail_file_lines(&log_path, 200, 32 * 1024) {
                self.agent_browser_log_tail = lines;
                let max_scroll = self.agent_browser_log_tail.len().saturating_sub(1);
                self.agent_browser_log_scroll = self.agent_browser_log_scroll.min(max_scroll);
            }

            if status.success() {
                self.refresh_agent_browser_runtime_quiet();
                if self.agent_browser_runtime_ready {
                    self.set_status(format!(
                        "Agent Browser {action} completed and verified ({})",
                        agent_browser_summary_with_runtime(
                            self.agent_browser_runtime_checked,
                            self.agent_browser_runtime_ready
                        )
                    ));
                } else {
                    self.set_status(format!(
                        "Agent Browser {action} finished, but runtime verification failed (log: {})",
                        log_path.to_string_lossy()
                    ));
                }
            } else {
                self.set_status(format!(
                    "Agent Browser {action} failed with status {status} (log: {})",
                    log_path.to_string_lossy()
                ));
            }
        }
    }

    fn set_search_action_last_status(&mut self, action: &str, status: String) {
        match action {
            "start-verify" => self.search_start_last_status = status,
            "web-smoke" => self.search_verify_last_status = status,
            _ => {}
        }
    }

    fn poll_search_job(&mut self) {
        let mut completed: Option<(String, String, ExitStatus, PathBuf)> = None;

        if let Some(job) = self.search_job.as_mut() {
            if let Ok(lines) = tail_file_lines(&job.log_path, 200, 32 * 1024) {
                self.search_log_tail = lines;
                let max_scroll = self.search_log_tail.len().saturating_sub(1);
                self.search_log_scroll = self.search_log_scroll.min(max_scroll);
            }

            match job.child.try_wait() {
                Ok(Some(status)) => {
                    completed = Some((
                        job.action.clone(),
                        job.success_message.clone(),
                        status,
                        job.log_path.clone(),
                    ));
                }
                Ok(None) => {}
                Err(err) => {
                    let action = job.action.clone();
                    self.search_job = None;
                    self.refresh_search_status_quiet();
                    self.set_search_action_last_status(&action, format!("poll error: {err}"));
                    self.set_status(format!("Search job poll failed: {err}"));
                    return;
                }
            }
        }

        if let Some((action, success_message, status, log_path)) = completed {
            self.search_job = None;
            if let Ok(lines) = tail_file_lines(&log_path, 200, 32 * 1024) {
                self.search_log_tail = lines;
                let max_scroll = self.search_log_tail.len().saturating_sub(1);
                self.search_log_scroll = self.search_log_scroll.min(max_scroll);
            }
            self.refresh_search_status_quiet();

            if status.success() {
                self.set_search_action_last_status(&action, "passed".to_string());
                self.set_status(format!(
                    "{success_message} (log: {})",
                    log_path.to_string_lossy()
                ));
            } else {
                self.set_search_action_last_status(&action, format!("failed ({status})"));
                self.set_status(format!(
                    "Search action {action} failed with status {status} (log: {})",
                    log_path.to_string_lossy()
                ));
            }
        }
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
        self.poll_startup_refresh();
        self.flush_pending_theme_preview();
        self.poll_agent_browser_job();
        self.poll_search_job();

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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    app.tick();
    let tick = Duration::from_millis(75);

    while !app.should_exit {
        terminal.draw(|frame| draw_ui(frame, &mut app))?;
        if event::poll(tick)? {
            match event::read()? {
                Event::Key(key) => handle_key(&mut app, key),
                Event::Mouse(mouse) => handle_mouse(&mut app, mouse),
                _ => {}
            }
        }
        app.tick();
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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
        Mode::PickBackgroundProfile => handle_key_picker(app, key, Picker::BackgroundProfile),
        Mode::EditProjectsBase => handle_key_input(app, key, InputMode::ProjectsBase),
        Mode::SearchProjects => handle_key_input(app, key, InputMode::Search),
        Mode::NewProject => handle_key_input(app, key, InputMode::NewProject),
        Mode::NewProjectLayout => handle_key_input(app, key, InputMode::NewProjectLayout),
        Mode::NewGlobalLayout => handle_key_input(app, key, InputMode::NewGlobalLayout),
        Mode::NewTheme => handle_key_input(app, key, InputMode::NewTheme),
        Mode::RtkActions => handle_key_rtk_actions(app, key),
        Mode::AgentInstallActions => handle_key_agent_install_actions(app, key),
        Mode::PiCompactionActions => handle_key_pi_compaction_actions(app, key),
        Mode::Help => handle_key_help(app, key),
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if app.mode != Mode::Normal {
        return;
    }

    if app.active_tab != Tab::Defaults || app.settings_section != SettingsSection::ThemeManager {
        return;
    }

    let Some(area) = app.theme_preview_area else {
        return;
    };

    let in_preview = mouse.column >= area.x
        && mouse.column < area.x.saturating_add(area.width)
        && mouse.row >= area.y
        && mouse.row < area.y.saturating_add(area.height);

    if !in_preview {
        return;
    }

    match mouse.kind {
        MouseEventKind::ScrollDown => app.scroll_theme_preview(3),
        MouseEventKind::ScrollUp => app.scroll_theme_preview(-3),
        _ => {}
    }
}

fn handle_key_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => {
            if app.active_tab == Tab::Defaults
                && app.settings_section == SettingsSection::ThemeManager
            {
                app.end_theme_preview();
            }
            app.should_exit = true;
        }
        KeyCode::Esc => {
            if app.focus == Focus::Detail {
                if app.active_tab == Tab::Defaults
                    && app.mode == Mode::Normal
                    && app.settings_section != SettingsSection::Root
                {
                    app.back_settings_section();
                } else {
                    app.focus = Focus::Nav;
                }
            } else {
                if app.active_tab == Tab::Defaults
                    && app.settings_section == SettingsSection::ThemeManager
                {
                    app.end_theme_preview();
                }
                app.should_exit = true;
                if app.floating_active {
                    app.close_on_exit = true;
                }
            }
        }
        KeyCode::Tab => cycle_tab(app, true),
        KeyCode::BackTab => cycle_tab(app, false),
        KeyCode::Char('h') | KeyCode::Left
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsPiCompaction
                && app.selected_settings_index() == 0 =>
        {
            app.shift_pi_compaction_context_window(-1);
        }
        KeyCode::Char('l') | KeyCode::Right
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsPiCompaction
                && app.selected_settings_index() == 0 =>
        {
            app.shift_pi_compaction_context_window(1);
        }
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
        KeyCode::Char('b') if app.active_tab == Tab::Projects && app.focus == Focus::Detail => {
            app.start_input(
                Mode::EditProjectsBase,
                app.projects_base.to_string_lossy().to_string(),
            );
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
            if app.settings_section == SettingsSection::ThemeManager {
                app.end_theme_preview();
                app.theme_preview_scroll = 0;
            }
            app.set_settings_section(SettingsSection::Theme)
        }
        KeyCode::Char('n')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ThemeManager =>
        {
            app.start_input(Mode::NewTheme, String::new());
        }
        KeyCode::Char('i')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ThemeManager =>
        {
            app.install_all_presets();
            app.queue_preview_selected_theme();
        }
        KeyCode::Char('r')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ThemeManager =>
        {
            app.refresh_themes();
            app.queue_preview_selected_theme();
            app.set_status("Refreshed theme list");
        }
        KeyCode::PageDown | KeyCode::Char('J')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ThemeManager =>
        {
            app.scroll_theme_preview(3);
        }
        KeyCode::PageUp | KeyCode::Char('K')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ThemeManager =>
        {
            app.scroll_theme_preview(-3);
        }
        KeyCode::PageDown
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && app.selected_settings_index() == 0 =>
        {
            app.scroll_agent_browser_log(8);
        }
        KeyCode::PageDown
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && matches!(app.selected_settings_index(), 4 | 5) =>
        {
            app.scroll_search_log(8);
        }
        KeyCode::PageUp
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && app.selected_settings_index() == 0 =>
        {
            app.scroll_agent_browser_log(-8);
        }
        KeyCode::PageUp
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && matches!(app.selected_settings_index(), 4 | 5) =>
        {
            app.scroll_search_log(-8);
        }
        KeyCode::Char('x')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && app.selected_settings_index() == 0 =>
        {
            app.cancel_agent_browser_job();
        }
        KeyCode::Char('x')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && matches!(app.selected_settings_index(), 4 | 5) =>
        {
            app.cancel_search_job();
        }
        KeyCode::Char('O')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && app.selected_settings_index() == 0 =>
        {
            app.open_agent_browser_log();
        }
        KeyCode::Char('O')
            if app.active_tab == Tab::Defaults
                && app.focus == Focus::Detail
                && app.settings_section == SettingsSection::ToolsAgentBrowser
                && matches!(app.selected_settings_index(), 4 | 5) =>
        {
            app.open_search_log();
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
    BackgroundProfile,
}

fn layout_picker_options_for(target: PickTarget) -> Vec<String> {
    match target {
        PickTarget::Edit => custom_layout_options(),
        PickTarget::Defaults | PickTarget::Override => layout_options(),
    }
}

fn handle_key_picker(app: &mut App, key: KeyEvent, picker: Picker) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char('j') | KeyCode::Down => match picker {
            Picker::Layout(target) => list_next_state(
                &mut app.layout_picker_state,
                layout_picker_options_for(target).len(),
            ),
            Picker::Agent(_) => list_next_state(&mut app.agent_picker_state, agent_options().len()),
            Picker::BackgroundProfile => list_next_state(
                &mut app.background_picker_state,
                background_profile_options().len(),
            ),
        },
        KeyCode::Char('k') | KeyCode::Up => match picker {
            Picker::Layout(target) => list_prev_state(
                &mut app.layout_picker_state,
                layout_picker_options_for(target).len(),
            ),
            Picker::Agent(_) => list_prev_state(&mut app.agent_picker_state, agent_options().len()),
            Picker::BackgroundProfile => list_prev_state(
                &mut app.background_picker_state,
                background_profile_options().len(),
            ),
        },
        KeyCode::Enter => match picker {
            Picker::Layout(target) => {
                let index = app.layout_picker_state.selected().unwrap_or(0);
                let options = layout_picker_options_for(target);
                if let Some(choice) = options.get(index).cloned() {
                    match target {
                        PickTarget::Defaults => app.set_default_layout(choice),
                        PickTarget::Override => app.set_override_layout(choice),
                        PickTarget::Edit => app.edit_custom_layout(choice),
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
                        PickTarget::Edit => app.mode = Mode::Normal,
                    }
                }
            }
            Picker::BackgroundProfile => {
                let index = app.background_picker_state.selected().unwrap_or(0);
                let options = background_profile_options();
                if let Some(choice) = options.get(index).cloned() {
                    app.set_background_profile(choice);
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
    NewProjectLayout,
    NewGlobalLayout,
    NewTheme,
}

fn handle_key_input(app: &mut App, key: KeyEvent, mode: InputMode) {
    match key.code {
        KeyCode::Esc => {
            app.input_buffer = app.input_snapshot.clone();
            if matches!(mode, InputMode::NewTheme) {
                app.input_buffer.clear();
                app.input_snapshot.clear();
                app.mode = Mode::Normal;
            } else {
                app.cancel_input();
            }
        }
        KeyCode::Enter => match mode {
            InputMode::ProjectsBase => app.commit_projects_base(),
            InputMode::Search => app.commit_search(),
            InputMode::NewProject => app.commit_new_project(),
            InputMode::NewProjectLayout => app.commit_new_project_layout(),
            InputMode::NewGlobalLayout => app.commit_new_global_layout(),
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

fn handle_key_agent_install_actions(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char('j') | KeyCode::Down => list_next_state(
            &mut app.agent_install_actions_state,
            app.agent_install_entries.len(),
        ),
        KeyCode::Char('k') | KeyCode::Up => list_prev_state(
            &mut app.agent_install_actions_state,
            app.agent_install_entries.len(),
        ),
        KeyCode::Char('r') => app.refresh_agent_install_statuses(),
        KeyCode::Enter => app.run_selected_agent_install_action(),
        _ => {}
    }
}

fn handle_key_pi_compaction_actions(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Char('j') | KeyCode::Down => list_next_state(
            &mut app.pi_compaction_actions_state,
            pi_compaction_presets(app.pi_compaction_context_window).len(),
        ),
        KeyCode::Char('k') | KeyCode::Up => list_prev_state(
            &mut app.pi_compaction_actions_state,
            pi_compaction_presets(app.pi_compaction_context_window).len(),
        ),
        KeyCode::Char('r') => app.refresh_pi_compaction_status(),
        KeyCode::Enter => app.apply_selected_pi_compaction_preset(),
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
    let was_theme_manager =
        app.active_tab == Tab::Defaults && app.settings_section == SettingsSection::ThemeManager;

    app.active_tab = match (app.active_tab, forward) {
        (Tab::Defaults, true) => Tab::Projects,
        (Tab::Projects, true) => Tab::Sessions,
        (Tab::Sessions, true) => Tab::Defaults,
        (Tab::Defaults, false) => Tab::Sessions,
        (Tab::Projects, false) => Tab::Defaults,
        (Tab::Sessions, false) => Tab::Projects,
    };

    if app.active_tab != Tab::Defaults {
        if was_theme_manager {
            app.end_theme_preview();
            app.theme_preview_scroll = 0;
        }
        app.settings_section = SettingsSection::Root;
    }
}

fn list_next(app: &mut App) {
    match app.active_tab {
        Tab::Defaults => match app.settings_section {
            SettingsSection::Root => {
                list_next_state(&mut app.defaults_state, settings_root_options().len())
            }
            SettingsSection::Theme => list_next_state(
                &mut app.settings_theme_state,
                settings_theme_options().len(),
            ),
            SettingsSection::ThemeManager => {
                list_next_state(&mut app.theme_manager_state, app.theme_entries.len());
                app.queue_preview_selected_theme();
            }
            SettingsSection::Layout => list_next_state(
                &mut app.settings_layout_state,
                settings_layout_options().len(),
            ),
            SettingsSection::Tools => list_next_state(
                &mut app.settings_tools_state,
                settings_tools_options().len(),
            ),
            SettingsSection::ToolsPiCompaction => list_next_state(
                &mut app.settings_tools_pi_compaction_state,
                settings_tools_pi_compaction_options().len(),
            ),
            SettingsSection::ToolsAgentBrowser => list_next_state(
                &mut app.settings_tools_agent_browser_state,
                settings_tools_agent_browser_options().len(),
            ),
            SettingsSection::ToolsVercel => list_next_state(
                &mut app.settings_tools_vercel_state,
                settings_tools_vercel_options().len(),
            ),
            SettingsSection::ToolsHyperframes => list_next_state(
                &mut app.settings_tools_hyperframes_state,
                settings_tools_hyperframes_options().len(),
            ),
        },
        Tab::Projects => list_next_state(&mut app.projects_state, app.filtered_projects.len()),
        Tab::Sessions => list_next_state(&mut app.sessions_state, 4),
    }
}

fn list_prev(app: &mut App) {
    match app.active_tab {
        Tab::Defaults => match app.settings_section {
            SettingsSection::Root => {
                list_prev_state(&mut app.defaults_state, settings_root_options().len())
            }
            SettingsSection::Theme => list_prev_state(
                &mut app.settings_theme_state,
                settings_theme_options().len(),
            ),
            SettingsSection::ThemeManager => {
                list_prev_state(&mut app.theme_manager_state, app.theme_entries.len());
                app.queue_preview_selected_theme();
            }
            SettingsSection::Layout => list_prev_state(
                &mut app.settings_layout_state,
                settings_layout_options().len(),
            ),
            SettingsSection::Tools => list_prev_state(
                &mut app.settings_tools_state,
                settings_tools_options().len(),
            ),
            SettingsSection::ToolsPiCompaction => list_prev_state(
                &mut app.settings_tools_pi_compaction_state,
                settings_tools_pi_compaction_options().len(),
            ),
            SettingsSection::ToolsAgentBrowser => list_prev_state(
                &mut app.settings_tools_agent_browser_state,
                settings_tools_agent_browser_options().len(),
            ),
            SettingsSection::ToolsVercel => list_prev_state(
                &mut app.settings_tools_vercel_state,
                settings_tools_vercel_options().len(),
            ),
            SettingsSection::ToolsHyperframes => list_prev_state(
                &mut app.settings_tools_hyperframes_state,
                settings_tools_hyperframes_options().len(),
            ),
        },
        Tab::Projects => list_prev_state(&mut app.projects_state, app.filtered_projects.len()),
        Tab::Sessions => list_prev_state(&mut app.sessions_state, 4),
    }
}

fn activate_selection(app: &mut App) {
    match app.active_tab {
        Tab::Defaults => match app.settings_section {
            SettingsSection::Root => match app.defaults_state.selected().unwrap_or(0) {
                0 => app.set_settings_section(SettingsSection::Theme),
                1 => app.set_settings_section(SettingsSection::Layout),
                2 => app.set_settings_section(SettingsSection::Tools),
                _ => {}
            },
            SettingsSection::Theme => match app.settings_theme_state.selected().unwrap_or(0) {
                0 => app.open_theme_manager(),
                1 => app.open_background_picker(),
                2 => app.set_settings_section(SettingsSection::Root),
                _ => {}
            },
            SettingsSection::ThemeManager => app.activate_selected_theme(),
            SettingsSection::Layout => match app.settings_layout_state.selected().unwrap_or(0) {
                0 => {
                    let current = app.default_layout.clone();
                    select_picker(&mut app.layout_picker_state, &layout_options(), &current);
                    app.mode = Mode::PickLayout(PickTarget::Defaults);
                }
                1 => app.start_input(Mode::NewProjectLayout, String::new()),
                2 => app.start_input(Mode::NewGlobalLayout, String::new()),
                3 => {
                    let options = custom_layout_options();
                    if options.is_empty() {
                        app.set_status("No custom layouts found yet");
                    } else {
                        let current = if options.iter().any(|value| value == &app.default_layout) {
                            app.default_layout.clone()
                        } else {
                            options[0].clone()
                        };
                        select_picker(&mut app.layout_picker_state, &options, &current);
                        app.mode = Mode::PickLayout(PickTarget::Edit);
                    }
                }
                4 => app.set_settings_section(SettingsSection::Root),
                _ => {}
            },
            SettingsSection::Tools => match app.settings_tools_state.selected().unwrap_or(0) {
                0 => app.open_rtk_actions(),
                1 => app.open_agent_install_actions(),
                2 => app.set_settings_section(SettingsSection::ToolsPiCompaction),
                3 => app.set_settings_section(SettingsSection::ToolsAgentBrowser),
                4 => app.run_aoc_map_init_action(),
                5 => app.set_settings_section(SettingsSection::ToolsVercel),
                6 => app.set_settings_section(SettingsSection::ToolsHyperframes),
                7 => app.set_settings_section(SettingsSection::Root),
                _ => {}
            },
            SettingsSection::ToolsPiCompaction => {
                match app
                    .settings_tools_pi_compaction_state
                    .selected()
                    .unwrap_or(0)
                {
                    0 => {}
                    1 => app.open_pi_compaction_actions(),
                    2 => app.refresh_pi_compaction_status(),
                    3 => app.set_settings_section(SettingsSection::Tools),
                    _ => {}
                }
            }
            SettingsSection::ToolsAgentBrowser => {
                match app
                    .settings_tools_agent_browser_state
                    .selected()
                    .unwrap_or(0)
                {
                    0 => app.run_agent_browser_tool_action(),
                    1 => app.run_agent_browser_skill_action(),
                    2 => app.run_web_research_skill_action(),
                    3 => app.run_search_enable_action(),
                    4 => app.run_search_start_or_verify_action(),
                    5 => app.run_web_research_verify_action(),
                    6 => app.set_settings_section(SettingsSection::Tools),
                    _ => {}
                }
            }
            SettingsSection::ToolsVercel => {
                match app.settings_tools_vercel_state.selected().unwrap_or(0) {
                    0 => app.run_vercel_tool_action(),
                    1 => app.run_vercel_skill_action(),
                    2 => app.verify_vercel_tool_action(),
                    3 => app.set_settings_section(SettingsSection::Tools),
                    _ => {}
                }
            }
            SettingsSection::ToolsHyperframes => {
                match app.settings_tools_hyperframes_state.selected().unwrap_or(0) {
                    0 => app.run_hyperframes_init_action(),
                    1 => app.run_hyperframes_sync_skills_action(),
                    2 => app.run_hyperframes_doctor_action(),
                    3 => app.run_hyperframes_preview_action(),
                    4 => app.set_settings_section(SettingsSection::Tools),
                    _ => {}
                }
            }
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
    app.theme_preview_area = None;

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
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(area);

    let (title, items) = match app.settings_section {
        SettingsSection::Root => {
            let items = vec![
                ListItem::new("Theme"),
                ListItem::new(format!("Layout · {}", app.default_layout)),
                ListItem::new("Tools"),
            ];
            ("Settings", items)
        }
        SettingsSection::Theme => {
            let items = vec![
                ListItem::new(format!("Theme manager · {}", app.theme_identity_label())),
                ListItem::new(format!("Background profile · {}", app.background_profile)),
                ListItem::new("Back"),
            ];
            ("Settings · Theme", items)
        }
        SettingsSection::ThemeManager => {
            let items = if app.theme_entries.is_empty() {
                vec![ListItem::new("(no themes found)")]
            } else {
                app.theme_entries
                    .iter()
                    .map(|entry| {
                        let source = match entry.source {
                            ThemeSource::Preset => "preset",
                            ThemeSource::Custom => "custom",
                        };
                        let install = if matches!(entry.source, ThemeSource::Preset) {
                            if entry.installed {
                                "installed"
                            } else {
                                "available"
                            }
                        } else {
                            "global"
                        };
                        let selected =
                            if app.theme_preview_selected.as_deref() == Some(entry.name.as_str()) {
                                " · selected"
                            } else {
                                ""
                            };
                        ListItem::new(format!(
                            "{} · {} ({}){}",
                            source, entry.name, install, selected
                        ))
                    })
                    .collect()
            };
            ("Settings · Theme Manager", items)
        }
        SettingsSection::Layout => {
            let items = vec![
                ListItem::new(format!("Default layout · {}", app.default_layout)),
                ListItem::new("Create project custom layout"),
                ListItem::new("Create global custom layout"),
                ListItem::new(format!(
                    "Edit custom layout · {} available",
                    custom_layout_options().len()
                )),
                ListItem::new("Back"),
            ];
            ("Settings · Layout", items)
        }
        SettingsSection::Tools => {
            let items = vec![
                ListItem::new(format!(
                    "RTK routing · {}",
                    if app.rtk_loaded {
                        rtk_summary(&app.rtk_status)
                    } else {
                        "loading...".to_string()
                    }
                )),
                ListItem::new(format!(
                    "PI agent installer · {}",
                    if app.agent_install_entries_loaded {
                        agent_install_summary(&app.agent_install_entries)
                    } else {
                        "loading...".to_string()
                    }
                )),
                ListItem::new(format!(
                    "PI compaction · {}",
                    if app.pi_compaction_status_loaded {
                        pi_compaction_summary(&app.pi_compaction_status)
                    } else {
                        "loading...".to_string()
                    }
                )),
                ListItem::new(format!(
                    "Agent Browser + Search · {}",
                    agent_browser_search_summary(
                        app.agent_browser_runtime_checked,
                        app.agent_browser_runtime_ready,
                        &app.search_status,
                    )
                )),
                ListItem::new(format!("AOC Map microsite · {}", aoc_map_summary())),
                ListItem::new(format!("Vercel CLI + PI skill · {}", vercel_summary())),
                ListItem::new(format!("HyperFrames video · {}", hyperframes_summary())),
                ListItem::new("Back"),
            ];
            ("Settings · Tools", items)
        }
        SettingsSection::ToolsPiCompaction => {
            let items = vec![
                ListItem::new(format!(
                    "Context window · {}",
                    format_token_count(app.pi_compaction_context_window)
                )),
                ListItem::new(format!(
                    "Apply preset · {}",
                    if app.pi_compaction_status_loaded {
                        pi_compaction_summary(&app.pi_compaction_status)
                    } else {
                        "loading...".to_string()
                    }
                )),
                ListItem::new("Refresh status"),
                ListItem::new("Back"),
            ];
            ("Settings · Tools · PI Compaction", items)
        }
        SettingsSection::ToolsAgentBrowser => {
            let action = if agent_browser_installed() {
                "Update tool"
            } else {
                "Install tool"
            };
            let running = if app.agent_browser_job.is_some() {
                " · running"
            } else {
                ""
            };
            let items = vec![
                ListItem::new(format!(
                    "{action} · {}{running}",
                    agent_browser_summary_with_runtime(
                        app.agent_browser_runtime_checked,
                        app.agent_browser_runtime_ready
                    )
                )),
                ListItem::new("Install/update PI skill"),
                ListItem::new(format!(
                    "Install/update PI web research skill · {}",
                    if web_research_skill_installed() {
                        "skill present"
                    } else {
                        "skill missing"
                    }
                )),
                ListItem::new(format!(
                    "Enable managed local search (SearXNG) · {}",
                    search_summary(&app.search_status)
                )),
                ListItem::new(format!(
                    "Start/verify local search · {}{}",
                    search_summary(&app.search_status),
                    if app
                        .search_job
                        .as_ref()
                        .map(|job| job.action == "start-verify")
                        .unwrap_or(false)
                    {
                        " · running"
                    } else {
                        ""
                    }
                )),
                ListItem::new(format!(
                    "Verify web research stack{}",
                    if app
                        .search_job
                        .as_ref()
                        .map(|job| job.action == "web-smoke")
                        .unwrap_or(false)
                    {
                        " · running"
                    } else {
                        ""
                    }
                )),
                ListItem::new("Back"),
            ];
            ("Settings · Tools · Agent Browser + Search", items)
        }
        SettingsSection::ToolsVercel => {
            let action = if vercel_installed() {
                "Update tool"
            } else {
                "Install tool"
            };
            let items = vec![
                ListItem::new(format!("{action} · {}", vercel_summary())),
                ListItem::new("Install/update PI skill"),
                ListItem::new("Verify CLI"),
                ListItem::new("Back"),
            ];
            ("Settings · Tools · Vercel", items)
        }
        SettingsSection::ToolsHyperframes => {
            let items = vec![
                ListItem::new("Init workspace + sync PI skills"),
                ListItem::new("Sync HyperFrames PI skills only"),
                ListItem::new("Run HyperFrames doctor"),
                ListItem::new("Start preview pane"),
                ListItem::new("Back"),
            ];
            ("Settings · Tools · HyperFrames", items)
        }
    };

    let list = List::new(items)
        .block(titled_block(title, focused))
        .highlight_style(detail_highlight_style(focused))
        .highlight_symbol("> ");

    match app.settings_section {
        SettingsSection::Root => {
            frame.render_stateful_widget(list, columns[0], &mut app.defaults_state)
        }
        SettingsSection::Theme => {
            frame.render_stateful_widget(list, columns[0], &mut app.settings_theme_state)
        }
        SettingsSection::ThemeManager => {
            frame.render_stateful_widget(list, columns[0], &mut app.theme_manager_state)
        }
        SettingsSection::Layout => {
            frame.render_stateful_widget(list, columns[0], &mut app.settings_layout_state)
        }
        SettingsSection::Tools => {
            frame.render_stateful_widget(list, columns[0], &mut app.settings_tools_state)
        }
        SettingsSection::ToolsPiCompaction => frame.render_stateful_widget(
            list,
            columns[0],
            &mut app.settings_tools_pi_compaction_state,
        ),
        SettingsSection::ToolsAgentBrowser => frame.render_stateful_widget(
            list,
            columns[0],
            &mut app.settings_tools_agent_browser_state,
        ),
        SettingsSection::ToolsVercel => {
            frame.render_stateful_widget(list, columns[0], &mut app.settings_tools_vercel_state)
        }
        SettingsSection::ToolsHyperframes => {
            frame.render_stateful_widget(list, columns[0], &mut app.settings_tools_hyperframes_state)
        }
    }

    app.theme_preview_area = if app.settings_section == SettingsSection::ThemeManager {
        Some(columns[1])
    } else {
        None
    };

    let detail_title = if app.settings_section == SettingsSection::ThemeManager {
        "Theme Preview"
    } else {
        "Details"
    };
    let detail_lines = if app.settings_section == SettingsSection::ThemeManager {
        theme_preview_lines(app)
    } else {
        settings_detail_lines(app)
    };
    let mut details = Paragraph::new(detail_lines)
        .block(Block::default().borders(Borders::ALL).title(detail_title))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });
    if app.settings_section == SettingsSection::ThemeManager {
        details = details.scroll((app.theme_preview_scroll, 0));
    }
    frame.render_widget(details, columns[1]);
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
                PickTarget::Edit => "Select Custom Layout to Edit",
            };
            let items: Vec<ListItem> = layout_picker_options_for(target)
                .into_iter()
                .map(ListItem::new)
                .collect();
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
                PickTarget::Edit => "Select Agent",
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
        Mode::PickBackgroundProfile => {
            let items: Vec<ListItem> = background_profile_options()
                .into_iter()
                .map(ListItem::new)
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Select Background Profile"),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            frame.render_stateful_widget(list, area, &mut app.background_picker_state);
        }
        Mode::EditProjectsBase => draw_input_modal(frame, area, "Projects base", &app.input_buffer),
        Mode::SearchProjects => draw_input_modal(frame, area, "Search projects", &app.input_buffer),
        Mode::NewProject => draw_input_modal(frame, area, "New project", &app.input_buffer),
        Mode::NewProjectLayout => {
            draw_input_modal(frame, area, "New project custom layout", &app.input_buffer)
        }
        Mode::NewGlobalLayout => {
            draw_input_modal(frame, area, "New global custom layout", &app.input_buffer)
        }
        Mode::NewTheme => {
            draw_input_modal(frame, area, "New theme (kebab-case)", &app.input_buffer)
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
        Mode::AgentInstallActions => {
            let items: Vec<ListItem> = app
                .agent_install_entries
                .iter()
                .map(|entry| {
                    let status = if entry.installed {
                        "installed"
                    } else {
                        "missing"
                    };
                    let action = if entry.installed { "update" } else { "install" };
                    ListItem::new(format!("{} ({status})  Enter: {action}", entry.label))
                })
                .collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(format!(
                    "Agent Installers ({})",
                    agent_install_summary(&app.agent_install_entries)
                )))
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, area, &mut app.agent_install_actions_state);
        }
        Mode::PiCompactionActions => {
            let items: Vec<ListItem> = pi_compaction_presets(app.pi_compaction_context_window)
                .into_iter()
                .map(|preset| ListItem::new(format!("{} · {}", preset.label, preset.description)))
                .collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(format!(
                    "PI Compaction Presets ({})",
                    pi_compaction_summary(&app.pi_compaction_status)
                )))
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, area, &mut app.pi_compaction_actions_state);
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
        Line::from("  Enter  open section/action"),
        Line::from("  Esc    back one settings level"),
        Line::from("  t      jump to Theme section"),
        Line::from("  Tools includes RTK, agent installers, PI compaction, Agent Browser, AOC Map, Vercel CLI, HyperFrames"),
        Line::from("  Right pane shows details for selected settings item"),
        Line::from("  Agent Browser/search jobs: PgUp/PgDn scroll, x cancel, Shift+O open log"),
        Line::from("  Theme manager: j/k preview, Enter activate+persist, n/i/r actions"),
        Line::from("  Layout section can create/edit custom layouts through your $EDITOR"),
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
        lines.push(Line::from(vec![
            Span::styled("Status: Ready", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("  ·  Theme: {}", app.theme_identity_label()),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
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
        Mode::EditProjectsBase
        | Mode::SearchProjects
        | Mode::NewProject
        | Mode::NewProjectLayout
        | Mode::NewGlobalLayout
        | Mode::NewTheme => vec![
            keycap("Enter"),
            Span::raw(" save  "),
            keycap("Esc"),
            Span::raw(" cancel"),
        ],
        Mode::PickLayout(_) | Mode::PickAgent(_) | Mode::PickBackgroundProfile => vec![
            keycap("Enter"),
            Span::raw(" choose  "),
            keycap("Esc"),
            Span::raw(" cancel"),
        ],
        Mode::Help => vec![keycap("Esc"), Span::raw(" close help")],
        Mode::RtkActions => vec![
            keycap("Enter"),
            Span::raw(" run action  "),
            keycap("Esc"),
            Span::raw(" close"),
        ],
        Mode::AgentInstallActions => vec![
            keycap("Enter"),
            Span::raw(" install/update  "),
            keycap("r"),
            Span::raw(" refresh  "),
            keycap("Esc"),
            Span::raw(" close"),
        ],
        Mode::PiCompactionActions => vec![
            keycap("Enter"),
            Span::raw(" apply preset  "),
            keycap("r"),
            Span::raw(" refresh  "),
            keycap("Esc"),
            Span::raw(" close"),
        ],
        Mode::Normal => match app.active_tab {
            Tab::Defaults if app.settings_section == SettingsSection::ThemeManager => vec![
                keycap("j/k"),
                Span::raw(" preview  "),
                keycap("Enter"),
                Span::raw(" activate+persist  "),
                keycap("n/i/r"),
                Span::raw(" new/install/refresh  "),
                keycap("PgUp/PgDn"),
                Span::raw(" scroll"),
            ],
            Tab::Defaults => {
                if app.settings_section == SettingsSection::ToolsAgentBrowser
                    && app.selected_settings_index() == 0
                    && app.focus == Focus::Detail
                {
                    vec![
                        keycap("Enter"),
                        Span::raw(" start  "),
                        keycap("PgUp/PgDn"),
                        Span::raw(" log  "),
                        keycap("x"),
                        Span::raw(" cancel  "),
                        keycap("Shift+O"),
                        Span::raw(" open log"),
                    ]
                } else if app.settings_section == SettingsSection::ToolsPiCompaction
                    && app.selected_settings_index() == 0
                    && app.focus == Focus::Detail
                {
                    vec![
                        keycap("h/l"),
                        Span::raw(" change window  "),
                        keycap("Enter"),
                        Span::raw(" keep focus  "),
                        keycap("Esc"),
                        Span::raw(" back section"),
                    ]
                } else {
                    vec![
                        keycap("Enter"),
                        Span::raw(" open section/action  "),
                        keycap("Esc"),
                        Span::raw(" back section  "),
                        keycap("t"),
                        Span::raw(" theme section"),
                    ]
                }
            }
            Tab::Projects => vec![
                keycap("Enter"),
                Span::raw(" open  "),
                keycap("n"),
                Span::raw(" new  "),
                keycap("b"),
                Span::raw(" base  "),
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

fn settings_root_options() -> Vec<String> {
    vec![
        "Theme".to_string(),
        "Layout".to_string(),
        "Tools".to_string(),
    ]
}

fn settings_theme_options() -> Vec<String> {
    vec![
        "Theme manager".to_string(),
        "Background profile".to_string(),
        "Back".to_string(),
    ]
}

fn settings_layout_options() -> Vec<String> {
    vec![
        "Default layout".to_string(),
        "Create project custom layout".to_string(),
        "Create global custom layout".to_string(),
        "Edit custom layout".to_string(),
        "Back".to_string(),
    ]
}

fn settings_tools_options() -> Vec<String> {
    vec![
        "RTK routing".to_string(),
        "PI agent installer".to_string(),
        "PI compaction".to_string(),
        "Agent Browser + Search".to_string(),
        "AOC Map microsite".to_string(),
        "Vercel CLI + PI skill".to_string(),
        "HyperFrames video".to_string(),
        "Back".to_string(),
    ]
}

fn settings_tools_pi_compaction_options() -> Vec<String> {
    vec![
        "Context window".to_string(),
        "Apply preset".to_string(),
        "Refresh status".to_string(),
        "Back".to_string(),
    ]
}

fn settings_tools_agent_browser_options() -> Vec<String> {
    vec![
        "Install/update Agent Browser tool".to_string(),
        "Install/update PI browser skill".to_string(),
        "Install/update PI web research skill".to_string(),
        "Enable managed local search (SearXNG)".to_string(),
        "Start/verify local search".to_string(),
        "Verify web research stack".to_string(),
        "Back".to_string(),
    ]
}

fn settings_tools_vercel_options() -> Vec<String> {
    vec![
        "Install/update tool".to_string(),
        "Install/update PI skill".to_string(),
        "Verify CLI".to_string(),
        "Back".to_string(),
    ]
}

fn settings_tools_hyperframes_options() -> Vec<String> {
    vec![
        "Init workspace + sync skills".to_string(),
        "Sync HyperFrames skills".to_string(),
        "Run doctor".to_string(),
        "Start preview pane".to_string(),
        "Back".to_string(),
    ]
}

fn pi_compaction_context_window_options() -> Vec<u64> {
    vec![75_000, 125_000, 250_000, 1_000_000]
}

fn resolve_pi_compaction_context_window(config: &AocConfig) -> u64 {
    let options = pi_compaction_context_window_options();
    let configured = config.pi_compaction_context_window.unwrap_or(200_000);
    options
        .into_iter()
        .min_by_key(|value| value.abs_diff(configured))
        .unwrap_or(250_000)
}

fn compaction_trigger_percent(context_window: u64, reserve_tokens: u64) -> u64 {
    if context_window == 0 || reserve_tokens >= context_window {
        return 0;
    }
    (((context_window - reserve_tokens) as f64 / context_window as f64) * 100.0).round() as u64
}

fn reserve_for_trigger_percent(context_window: u64, trigger_percent: u64) -> u64 {
    context_window.saturating_sub((context_window * trigger_percent) / 100)
}

fn pi_compaction_presets(context_window: u64) -> Vec<PiCompactionPreset> {
    let default_trigger = compaction_trigger_percent(context_window, 16_384);
    vec![
        PiCompactionPreset {
            label: format!("PI default (~{}%)", default_trigger),
            enabled: true,
            reserve_tokens: 16_384,
            keep_recent_tokens: 20_000,
            description: format!(
                "Default pi behavior for a {} window; compact late with max raw history.",
                format_token_count(context_window)
            ),
        },
        PiCompactionPreset {
            label: "Parallel balanced (~60%)".to_string(),
            enabled: true,
            reserve_tokens: reserve_for_trigger_percent(context_window, 60),
            keep_recent_tokens: 20_000,
            description: "Earlier compaction for multi-session work without going too aggressive."
                .to_string(),
        },
        PiCompactionPreset {
            label: "Parallel aggressive (~45%)".to_string(),
            enabled: true,
            reserve_tokens: reserve_for_trigger_percent(context_window, 45),
            keep_recent_tokens: 20_000,
            description: "Recommended for local CPU pressure and parallel PI sessions.".to_string(),
        },
        PiCompactionPreset {
            label: "Max throughput (~40%)".to_string(),
            enabled: true,
            reserve_tokens: reserve_for_trigger_percent(context_window, 40),
            keep_recent_tokens: 20_000,
            description:
                "Compacts very early to keep sessions light at the cost of more summary churn."
                    .to_string(),
        },
        PiCompactionPreset {
            label: "Disable auto-compaction".to_string(),
            enabled: false,
            reserve_tokens: 16_384,
            keep_recent_tokens: 20_000,
            description: "Turn off automatic compaction; manual /compact still works.".to_string(),
        },
    ]
}

fn format_token_count(value: u64) -> String {
    if value >= 1_000 {
        let scaled = value as f64 / 1_000.0;
        if (scaled - scaled.floor()).abs() < f64::EPSILON {
            format!("{scaled:.0}k")
        } else {
            format!("{scaled:.1}k")
        }
    } else {
        value.to_string()
    }
}

fn pi_compaction_summary(status: &PiCompactionStatus) -> String {
    if status.enabled {
        let mut summary = format!(
            "on · reserve {} · keep {}",
            format_token_count(status.reserve_tokens),
            format_token_count(status.keep_recent_tokens)
        );
        if status.project_override {
            summary.push_str(" · project override");
        }
        summary
    } else {
        let mut summary = "off".to_string();
        if status.project_override {
            summary.push_str(" · project override");
        }
        summary
    }
}

fn settings_detail_lines(app: &App) -> Vec<Line<'static>> {
    let selected = app.selected_settings_index();
    let mut lines: Vec<Line<'static>> = Vec::new();

    match app.settings_section {
        SettingsSection::Root => match selected {
            0 => {
                lines.push(Line::from("Theme"));
                lines.push(Line::from(""));
                lines.push(Line::from("Manage visual styling and backgrounds."));
                lines.push(Line::from("Contains Theme manager and Background profile."));
                lines.push(Line::from("Enter to open Theme settings."));
            }
            1 => {
                lines.push(Line::from("Layout"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current default layout: {}",
                    app.default_layout
                )));
                lines.push(Line::from("Set how new AOC tabs are arranged by default."));
                lines.push(Line::from("Enter to open Layout settings."));
            }
            _ => {
                lines.push(Line::from("Tools"));
                lines.push(Line::from(""));
                lines.push(Line::from("Manage optional tooling and installers."));
                lines.push(Line::from(
                    "Includes RTK, PI compaction, Agent Browser, Vercel CLI, HyperFrames setup.",
                ));
                lines.push(Line::from("Enter to open Tools settings."));
            }
        },
        SettingsSection::Theme => match selected {
            0 => {
                lines.push(Line::from("Theme manager"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Active/effective: {}",
                    app.theme_identity_label()
                )));
                lines.push(Line::from(
                    "Open the integrated manager with live list + preview panel.",
                ));
                lines.push(Line::from("Enter opens Theme manager in-place."));
            }
            1 => {
                lines.push(Line::from("Background profile"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current profile: {}",
                    app.background_profile
                )));
                lines.push(Line::from(
                    "Switch background behavior used by AOC theme tooling.",
                ));
                lines.push(Line::from("Enter opens profile picker."));
            }
            _ => {
                lines.push(Line::from("Back"));
                lines.push(Line::from(""));
                lines.push(Line::from("Return to top-level Settings menu."));
            }
        },
        SettingsSection::ThemeManager => {
            lines.push(Line::from("Theme manager"));
            lines.push(Line::from(""));
            lines.push(Line::from("Use the theme list on the left."));
            lines.push(Line::from(
                "j/k previews live · Enter activates + persists.",
            ));
            lines.push(Line::from("n new custom · i install presets · r refresh."));
            lines.push(Line::from("PgUp/PgDn (or K/J) scrolls this preview pane."));
        }
        SettingsSection::Layout => match selected {
            0 => {
                lines.push(Line::from("Default layout"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current layout: {}",
                    app.default_layout
                )));
                lines.push(Line::from(
                    "Used when launching new sessions without overrides.",
                ));
                lines.push(Line::from("Enter opens layout picker."));
            }
            1 => {
                lines.push(Line::from("Create project custom layout"));
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Creates .aoc/layouts/<name>.kdl inside the current project.",
                ));
                lines.push(Line::from(
                    "A starter template includes the managed zjstatus top bar and metadata sync.",
                ));
                lines.push(Line::from(
                    "Enter prompts for a layout name, then opens $EDITOR.",
                ));
            }
            2 => {
                lines.push(Line::from("Create global custom layout"));
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Creates ~/.config/zellij/layouts/<name>.kdl for personal reuse across repos.",
                ));
                lines.push(Line::from(
                    "Use this for personal workflows that should not be committed to a repo.",
                ));
                lines.push(Line::from(
                    "Enter prompts for a layout name, then opens $EDITOR.",
                ));
            }
            3 => {
                lines.push(Line::from("Edit custom layout"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Available custom layouts: {}",
                    custom_layout_options().len()
                )));
                lines.push(Line::from(
                    "Picks a project/global custom layout and opens it in your editor.",
                ));
                lines.push(Line::from(
                    "Managed layout 'aoc' is intentionally excluded from direct editing.",
                ));
            }
            _ => {
                lines.push(Line::from("Back"));
                lines.push(Line::from(""));
                lines.push(Line::from("Return to top-level Settings menu."));
            }
        },
        SettingsSection::Tools => match selected {
            0 => {
                lines.push(Line::from("RTK routing"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Status: {}",
                    if app.rtk_loaded {
                        rtk_summary(&app.rtk_status)
                    } else {
                        "loading...".to_string()
                    }
                )));
                lines.push(Line::from(
                    "Enter opens RTK controls (install/enable/disable/doctor).",
                ));
            }
            1 => {
                lines.push(Line::from("PI agent installer"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Status: {}",
                    if app.agent_install_entries_loaded {
                        agent_install_summary(&app.agent_install_entries)
                    } else {
                        "loading...".to_string()
                    }
                )));
                lines.push(Line::from("Enter opens PI install/update actions."));
            }
            2 => {
                lines.push(Line::from("PI compaction"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Status: {}",
                    if app.pi_compaction_status_loaded {
                        pi_compaction_summary(&app.pi_compaction_status)
                    } else {
                        "loading...".to_string()
                    }
                )));
                lines.push(Line::from(
                    "Manage global auto-compaction presets written to ~/.pi/agent/settings.json.",
                ));
                if app.pi_compaction_status.project_override {
                    lines.push(Line::from(
                        "Project override detected in .pi/settings.json for this repo.",
                    ));
                }
                lines.push(Line::from("Enter opens preset + refresh actions."));
            }
            3 => {
                lines.push(Line::from("Agent Browser + Search"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Status: {}",
                    agent_browser_search_summary(
                        app.agent_browser_runtime_checked,
                        app.agent_browser_runtime_ready,
                        &app.search_status,
                    )
                )));
                if app.agent_browser_job.is_some() {
                    lines.push(Line::from("Tool install/update currently running."));
                }
                lines.push(Line::from(
                    "Enter opens nested actions for browser install/sync plus managed local search enable/start.",
                ));
            }
            4 => {
                lines.push(Line::from("AOC Map microsite"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!("Status: {}", aoc_map_summary())));
                lines.push(Line::from(
                    "Enter syncs .pi/skills/aoc-map/SKILL.md and runs 'aoc-map init' in the current repo (.aoc/map workspace).",
                ));
                lines.push(Line::from(
                    "Use this to ensure both the agent skill and the project-local visualization microsite shell exist.",
                ));
            }
            5 => {
                lines.push(Line::from("Vercel CLI + PI skill"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!("Status: {}", vercel_summary())));
                lines.push(Line::from(
                    "Enter opens nested actions (tool install/update, skill sync, verify).",
                ));
            }
            6 => {
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Enter opens nested actions (host init, local source, update).",
                ));
            }
            _ => {
                lines.push(Line::from("Back"));
                lines.push(Line::from(""));
                lines.push(Line::from("Return to top-level Settings menu."));
            }
        },
        SettingsSection::ToolsPiCompaction => match selected {
            0 => {
                lines.push(Line::from("Context window"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Preset math target: {}",
                    format_token_count(app.pi_compaction_context_window)
                )));
                lines.push(Line::from(
                    "Use h/l to decrease/increase the model context window used for preset calculations.",
                ));
                lines.push(Line::from(
                    "Options: 75k, 125k, 250k, 1m. This does not change the provider model itself.",
                ));
            }
            1 => {
                lines.push(Line::from("Apply preset"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current global policy: {}",
                    if app.pi_compaction_status_loaded {
                        pi_compaction_summary(&app.pi_compaction_status)
                    } else {
                        "loading...".to_string()
                    }
                )));
                lines.push(Line::from(
                    "Enter opens preset choices for global ~/.pi/agent/settings.json.",
                ));
                lines.push(Line::from(
                    "Preset labels and reserve-token values are computed from the selected context window.",
                ));
            }
            2 => {
                lines.push(Line::from("Refresh status"));
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Reload the current global compaction settings from disk.",
                ));
                if app.pi_compaction_status.project_override {
                    lines.push(Line::from(
                        "This repo has a .pi/settings.json compaction override.",
                    ));
                }
            }
            _ => {
                lines.push(Line::from("Back"));
                lines.push(Line::from(""));
                lines.push(Line::from("Return to Tools menu."));
            }
        },
        SettingsSection::ToolsAgentBrowser => match selected {
            0 => {
                let action = if agent_browser_installed() {
                    "update"
                } else {
                    "install"
                };
                lines.push(Line::from("Install/update Agent Browser tool"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current status: {}",
                    agent_browser_summary_with_runtime(
                        app.agent_browser_runtime_checked,
                        app.agent_browser_runtime_ready
                    )
                )));
                if !app.agent_browser_runtime_checked {
                    lines.push(Line::from(
                        "Runtime probe is lazy: it runs when you enter this section or after install/update.",
                    ));
                }
                lines.push(Line::from(format!(
                    "Enter starts background {action}; completion is verified against a real runtime probe.",
                )));
                lines.push(Line::from(
                    "Overrides: AOC_AGENT_BROWSER_INSTALL_CMD / AOC_AGENT_BROWSER_UPDATE_CMD",
                ));

                if let Some(job) = &app.agent_browser_job {
                    lines.push(Line::from(""));
                    lines.push(Line::from(format!("Running: {}", job.action)));
                    lines.push(Line::from(format!(
                        "Log: {}",
                        job.log_path.to_string_lossy()
                    )));
                    lines.push(Line::from(
                        "PgUp/PgDn scroll · x cancel · Shift+O open full log",
                    ));
                    if app.agent_browser_log_tail.is_empty() {
                        lines.push(Line::from("(waiting for log output...)"));
                    } else {
                        let visible = 12usize;
                        let max_start = app.agent_browser_log_tail.len().saturating_sub(visible);
                        let start = app.agent_browser_log_scroll.min(max_start);
                        let end = (start + visible).min(app.agent_browser_log_tail.len());
                        lines.push(Line::from(format!(
                            "Recent output: lines {}-{} of {}",
                            start + 1,
                            end,
                            app.agent_browser_log_tail.len()
                        )));
                        for line in &app.agent_browser_log_tail[start..end] {
                            lines.push(Line::from(format!("  {line}")));
                        }
                    }
                } else if let Some(log_path) = latest_agent_browser_log_path() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(format!(
                        "Latest log: {}",
                        log_path.to_string_lossy()
                    )));
                    lines.push(Line::from("Shift+O opens the full log in pager."));
                }
            }
            1 => {
                lines.push(Line::from("Install/update PI browser skill"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Skill status: {}",
                    if agent_browser_skill_installed() {
                        "present"
                    } else {
                        "missing"
                    }
                )));
                lines.push(Line::from(
                    "Enter syncs .pi/skills/agent-browser/SKILL.md from upstream.",
                ));
            }
            2 => {
                lines.push(Line::from("Install/update PI web research skill"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Skill status: {}",
                    if web_research_skill_installed() {
                        "present"
                    } else {
                        "missing"
                    }
                )));
                lines.push(Line::from(
                    "Enter writes .pi/skills/web-research/SKILL.md into the current repo.",
                ));
                lines.push(Line::from(
                    "Use this alongside managed local search so agents can follow a search-first, browse-second workflow.",
                ));
            }
            3 => {
                lines.push(Line::from("Enable managed local search (SearXNG)"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current search status: {}",
                    search_summary(&app.search_status)
                )));
                lines.push(Line::from(format!(
                    "Current note: {}",
                    app.search_status.message
                )));
                lines.push(Line::from(
                    "Enter writes .aoc/search.toml and .aoc/services/searxng/* using AOC-managed defaults.",
                ));
                lines.push(Line::from(
                    "It also syncs the PI browser skill and PI web research skill so agents get search-first, browse-second guidance immediately.",
                ));
                lines.push(Line::from("Requires Docker Compose to be available."));
            }
            4 => {
                lines.push(Line::from("Start/verify local search"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current search status: {}",
                    search_summary(&app.search_status)
                )));
                lines.push(Line::from(format!(
                    "Current note: {}",
                    app.search_status.message
                )));
                lines.push(Line::from(format!(
                    "Job state: {}",
                    app.search_start_last_status
                )));
                lines.push(Line::from(
                    "Enter starts managed SearXNG in the background and verifies the configured health endpoint.",
                ));
                lines.push(Line::from(
                    "Use this after enabling search or to re-check a running local instance.",
                ));
                push_search_job_detail(&mut lines, app, "start-verify");
            }
            5 => {
                lines.push(Line::from("Verify web research stack"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Current search status: {}",
                    search_summary(&app.search_status)
                )));
                lines.push(Line::from(format!(
                    "Job state: {}",
                    app.search_verify_last_status
                )));
                lines.push(Line::from(
                    "Enter runs bin/aoc-web-smoke in the background to verify search + browser integration.",
                ));
                lines.push(Line::from(
                    "This confirms aoc-search can return results and agent-browser can open and inspect the top hit.",
                ));
                push_search_job_detail(&mut lines, app, "web-smoke");
            }
            _ => {
                lines.push(Line::from("Back"));
                lines.push(Line::from(""));
                lines.push(Line::from("Return to Tools menu."));
            }
        },
        SettingsSection::ToolsVercel => match selected {
            0 => {
                let action = if vercel_installed() {
                    "update"
                } else {
                    "install"
                };
                lines.push(Line::from("Install/update tool"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!("Current status: {}", vercel_summary())));
                lines.push(Line::from(format!(
                    "Enter runs the {action} flow for the Vercel CLI.",
                )));
                lines.push(Line::from(
                    "Overrides: AOC_VERCEL_BIN / AOC_VERCEL_INSTALL_CMD / AOC_VERCEL_UPDATE_CMD",
                ));
                lines.push(Line::from(
                    "Login remains user-scoped via `vercel login` or VERCEL_TOKEN.",
                ));
            }
            1 => {
                lines.push(Line::from("Install/update PI skill"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!(
                    "Skill status: {}",
                    if vercel_skill_installed() {
                        "present"
                    } else {
                        "missing"
                    }
                )));
                lines.push(Line::from(
                    "Enter writes .pi/skills/vercel-cli/SKILL.md into the current repo.",
                ));
                lines.push(Line::from(
                    "The skill documents deploy, env, domains, logs, projects, teams, and safety patterns.",
                ));
            }
            2 => {
                lines.push(Line::from("Verify CLI"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!("Current status: {}", vercel_summary())));
                lines.push(Line::from(
                    "Enter runs a quick `vercel --version` probe and reports auth signal state.",
                ));
            }
            _ => {
                lines.push(Line::from("Back"));
                lines.push(Line::from(""));
                lines.push(Line::from("Return to Tools menu."));
            }
        },
        SettingsSection::ToolsHyperframes => match selected {
            0 => {
                lines.push(Line::from("Init HyperFrames workspace + skills"));
                lines.push(Line::from(""));
                lines.push(Line::from(format!("Current status: {}", hyperframes_summary())));
                lines.push(Line::from("Enter runs `aoc-hyperframes init`."));
                lines.push(Line::from("Then use Alt+X -> HyperFrames for video authoring."));
            }
            1 => {
                lines.push(Line::from("Sync HyperFrames PI skills"));
                lines.push(Line::from(""));
                lines.push(Line::from("Enter runs `aoc-hyperframes sync-skills`."));
            }
            2 => {
                lines.push(Line::from("Run HyperFrames doctor"));
                lines.push(Line::from(""));
                lines.push(Line::from("Checks Node.js >= 22, FFmpeg, and HyperFrames environment."));
            }
            3 => {
                lines.push(Line::from("Start preview pane"));
                lines.push(Line::from(""));
                lines.push(Line::from("Opens a Zellij pane below and runs `npx hyperframes preview` in the `hyperframes/` workspace."));
                lines.push(Line::from("Preview usually serves at http://localhost:3002."));
            }
            _ => {
                lines.push(Line::from("Back"));
                lines.push(Line::from(""));
                lines.push(Line::from("Return to Tools menu."));
            }
        },
    }

    lines
}

fn theme_preview_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let palette = app.theme_preview_palette;

    let selected = app.selected_theme_entry();
    if let Some(entry) = &selected {
        let source = match entry.source {
            ThemeSource::Preset => "preset",
            ThemeSource::Custom => "custom",
        };
        let install = if matches!(entry.source, ThemeSource::Preset) {
            if entry.installed {
                "installed"
            } else {
                "available"
            }
        } else {
            "global"
        };

        lines.push(Line::from(vec![
            Span::styled("Selected: ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.name.clone(), Style::default().fg(Color::Cyan)),
            Span::raw(format!("  ({source}, {install})")),
        ]));

        if let Some(err) = &app.theme_preview_palette_error {
            lines.push(Line::from(vec![
                Span::styled("Palette: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("fallback ({err})"),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }
    } else {
        lines.push(Line::from("No theme selected."));
    }

    lines.push(Line::from(vec![
        Span::styled("Active/effective: ", Style::default().fg(Color::DarkGray)),
        Span::raw(app.theme_identity_label()),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Controls: ", Style::default().fg(Color::DarkGray)),
        Span::raw("j/k preview  Enter activate+persist  Esc back"),
    ]));
    lines.push(Line::from(
        "          n new custom  i install presets  r refresh",
    ));
    lines.push(Line::from(
        "          PgUp/PgDn or K/J scroll preview examples",
    ));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Palette swatches",
        Style::default()
            .fg(palette.yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled(" fg ", Style::default().fg(palette.bg).bg(palette.fg)),
        Span::raw(" "),
        Span::styled(" bg ", Style::default().fg(palette.fg).bg(palette.bg)),
        Span::raw(" "),
        Span::styled(" blue ", Style::default().fg(palette.bg).bg(palette.blue)),
        Span::raw(" "),
        Span::styled(" green ", Style::default().fg(palette.bg).bg(palette.green)),
        Span::raw(" "),
        Span::styled(" red ", Style::default().fg(palette.bg).bg(palette.red)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            " orange ",
            Style::default().fg(palette.bg).bg(palette.orange),
        ),
        Span::raw(" "),
        Span::styled(
            " yellow ",
            Style::default().fg(palette.bg).bg(palette.yellow),
        ),
        Span::raw(" "),
        Span::styled(" cyan ", Style::default().fg(palette.bg).bg(palette.cyan)),
        Span::raw(" "),
        Span::styled(
            " magenta ",
            Style::default().fg(palette.bg).bg(palette.magenta),
        ),
        Span::raw(" "),
        Span::styled(" white ", Style::default().fg(palette.bg).bg(palette.white)),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Workspace UI preview",
        Style::default()
            .fg(palette.yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled(
            "Header",
            Style::default()
                .fg(palette.blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  tabs · status · commands",
            Style::default().fg(palette.fg),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Status OK", Style::default().fg(palette.green)),
        Span::raw("  |  "),
        Span::styled("Warning", Style::default().fg(palette.yellow)),
        Span::raw("  |  "),
        Span::styled("Error", Style::default().fg(palette.red)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Task #42", Style::default().fg(palette.cyan)),
        Span::styled("  active  priority: ", Style::default().fg(palette.fg)),
        Span::styled("high", Style::default().fg(palette.magenta)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Diff", Style::default().fg(palette.white)),
        Span::styled(" +12 ", Style::default().fg(palette.green)),
        Span::styled("-3", Style::default().fg(palette.red)),
        Span::styled("  ~2", Style::default().fg(palette.yellow)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Prompt", Style::default().fg(palette.white)),
        Span::raw(" "),
        Span::styled("$", Style::default().fg(palette.green)),
        Span::styled(
            " aoc-theme activate --name ",
            Style::default().fg(palette.fg),
        ),
        Span::styled("catppuccin", Style::default().fg(palette.cyan)),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Panel samples",
        Style::default()
            .fg(palette.yellow)
            .add_modifier(Modifier::BOLD),
    )));
    for idx in 1..=18 {
        lines.push(Line::from(vec![
            Span::styled(
                format!("Panel {idx:02}"),
                Style::default().fg(if idx % 2 == 0 {
                    palette.cyan
                } else {
                    palette.blue
                }),
            ),
            Span::styled(
                " · list row · details text · highlights · borders",
                Style::default().fg(if idx % 2 == 0 {
                    palette.fg
                } else {
                    palette.white
                }),
            ),
        ]));
    }

    lines
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

fn agent_install_targets() -> Vec<(&'static str, &'static str)> {
    vec![("pi", "PI Agent (npm)")]
}

fn agent_install_summary(entries: &[AgentInstallEntry]) -> String {
    if entries.is_empty() {
        return "0/0 installed".to_string();
    }
    let installed = entries.iter().filter(|entry| entry.installed).count();
    format!("{installed}/{} installed", entries.len())
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
        "catppuccin-mocha",
        "cyberpunk",
        "dracula",
        "everforest",
        "gruvbox",
        "kanagawa",
        "midnight-ocean",
        "monokai",
        "nord",
        "ocean-breeze",
        "onedark",
        "rose-pine",
        "solarized-dark",
        "solarized-light",
        "synthwave",
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

fn resolve_zellij_config_dir() -> PathBuf {
    if let Ok(explicit_config) = env::var("AOC_ZELLIJ_CONFIG") {
        let path = PathBuf::from(explicit_config);
        if let Some(parent) = path.parent() {
            return parent.to_path_buf();
        }
    }

    if let Ok(output) = Command::new("zellij").args(["setup", "--check"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if !trimmed.starts_with("[CONFIG DIR]:") {
                    continue;
                }
                let dir = trimmed
                    .trim_start_matches("[CONFIG DIR]:")
                    .trim()
                    .trim_matches('"');
                if !dir.is_empty() {
                    return PathBuf::from(dir);
                }
            }
        }
    }

    config_dir().join("zellij")
}

fn load_theme_palette(zellij_config_dir: &Path, theme_name: &str) -> io::Result<ThemePalette> {
    let path = zellij_config_dir
        .join("themes")
        .join(format!("{theme_name}.kdl"));
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} not found", path.to_string_lossy()),
        ));
    }

    let contents = fs::read_to_string(&path)?;
    parse_theme_palette_from_kdl(theme_name, &contents)
        .or_else(|| parse_any_palette_from_kdl(&contents))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unable to parse palette from {}", path.to_string_lossy()),
            )
        })
}

fn parse_theme_palette_from_kdl(theme_name: &str, contents: &str) -> Option<ThemePalette> {
    let mut palette = ThemePalette::default();
    let mut in_themes_block = false;
    let mut themes_depth: i32 = 0;
    let mut found_theme = false;
    let mut target_depth: i32 = 0;

    for raw_line in contents.lines() {
        let line_no_comment = raw_line.split("//").next().unwrap_or("");
        let trimmed = line_no_comment.trim();
        if trimmed.is_empty() {
            continue;
        }

        let opens = trimmed.chars().filter(|c| *c == '{').count() as i32;
        let closes = trimmed.chars().filter(|c| *c == '}').count() as i32;

        if !found_theme {
            if !in_themes_block && line_starts_named_node(trimmed, "themes") {
                in_themes_block = true;
                themes_depth = opens - closes;
                if themes_depth <= 0 {
                    in_themes_block = false;
                }
                continue;
            }

            if line_has_theme_decl(trimmed, theme_name)
                || line_starts_named_node(trimmed, theme_name)
                || (in_themes_block && line_starts_named_node(trimmed, theme_name))
            {
                found_theme = true;
                target_depth = opens - closes;
                continue;
            }

            if in_themes_block {
                themes_depth += opens - closes;
                if themes_depth <= 0 {
                    in_themes_block = false;
                }
            }
            continue;
        }

        if target_depth <= 0 {
            target_depth += opens - closes;
            continue;
        }

        if let Some(color) = parse_theme_color_line(trimmed, "fg") {
            palette.fg = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "bg") {
            palette.bg = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "black") {
            palette.black = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "red") {
            palette.red = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "green") {
            palette.green = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "yellow") {
            palette.yellow = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "blue") {
            palette.blue = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "magenta") {
            palette.magenta = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "cyan") {
            palette.cyan = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "white") {
            palette.white = color;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "orange") {
            palette.orange = color;
        }

        target_depth += opens - closes;
        if target_depth <= 0 {
            return Some(palette);
        }
    }

    if found_theme {
        Some(palette)
    } else {
        None
    }
}

fn parse_any_palette_from_kdl(contents: &str) -> Option<ThemePalette> {
    let mut palette = ThemePalette::default();
    let mut hit_count = 0usize;

    for raw_line in contents.lines() {
        let line_no_comment = raw_line.split("//").next().unwrap_or("");
        let trimmed = line_no_comment.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(color) = parse_theme_color_line(trimmed, "fg") {
            palette.fg = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "bg") {
            palette.bg = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "black") {
            palette.black = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "red") {
            palette.red = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "green") {
            palette.green = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "yellow") {
            palette.yellow = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "blue") {
            palette.blue = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "magenta") {
            palette.magenta = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "cyan") {
            palette.cyan = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "white") {
            palette.white = color;
            hit_count += 1;
        }
        if let Some(color) = parse_theme_color_line(trimmed, "orange") {
            palette.orange = color;
            hit_count += 1;
        }
    }

    if hit_count >= 3 {
        Some(palette)
    } else {
        None
    }
}

fn line_has_theme_decl(line: &str, name: &str) -> bool {
    line.starts_with("theme ") && line.contains('"') && line.contains(&format!("\"{name}\""))
}

fn line_starts_named_node(line: &str, name: &str) -> bool {
    let mut token = String::new();
    for ch in line.chars() {
        if ch.is_whitespace() || ch == '{' {
            break;
        }
        token.push(ch);
    }

    if token.is_empty() {
        return false;
    }

    token.trim_matches('"') == name
}

fn parse_theme_color_line(line: &str, key: &str) -> Option<Color> {
    let rest = line.strip_prefix(key)?;
    let first = rest.chars().next()?;
    if !first.is_whitespace() {
        return None;
    }
    let value = rest.trim_start();
    if value.is_empty() {
        return None;
    }

    if let Some(stripped) = value.strip_prefix('"') {
        let end = stripped.find('"')?;
        return parse_hex_color(&stripped[..end]);
    }

    if let Some(token) = value.split_whitespace().next() {
        if token.starts_with('#') {
            return parse_hex_color(token);
        }
    }

    let mut parts = value.split_whitespace();
    let r = parts.next()?.parse::<u8>().ok()?;
    let g = parts.next()?.parse::<u8>().ok()?;
    let b = parts.next()?.parse::<u8>().ok()?;
    Some(Color::Rgb(r, g, b))
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim().trim_end_matches(';').trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod theme_parser_tests {
    use super::*;

    #[test]
    fn parses_named_theme_in_themes_block_hex() {
        let input = r##"
themes {
    gruvbox {
        fg "#d5c4a1"
        bg "#282828"
        black "#3c3836"
        red "#cc241d"
        green "#98971a"
        yellow "#d79921"
        blue "#3c8588"
        magenta "#b16286"
        cyan "#689d6a"
        white "#fbf1c7"
        orange "#d65d0e"
    }
}
"##;

        let parsed = parse_theme_palette_from_kdl("gruvbox", input).expect("palette");
        assert_eq!(parsed.bg, Color::Rgb(0x28, 0x28, 0x28));
        assert_eq!(parsed.blue, Color::Rgb(0x3c, 0x85, 0x88));
    }

    #[test]
    fn parses_named_theme_in_themes_block_rgb() {
        let input = r#"
themes {
    catppuccin-mocha {
        fg 205 214 244
        bg 30 30 46
        black 69 71 90
        red 243 139 168
        green 166 227 161
        yellow 249 226 175
        blue 137 180 250
        magenta 245 194 231
        cyan 148 226 213
        white 186 194 222
        orange 250 179 135
    }
}
"#;

        let parsed = parse_theme_palette_from_kdl("catppuccin-mocha", input).expect("palette");
        assert_eq!(parsed.fg, Color::Rgb(205, 214, 244));
        assert_eq!(parsed.red, Color::Rgb(243, 139, 168));
    }

    #[test]
    fn parse_any_palette_fallback_extracts_colors() {
        let input = r##"
weird_wrapper {
    strange_theme {
        fg "#AABBCC"
        bg "#112233"
        blue "#445566"
    }
}
"##;

        let parsed = parse_any_palette_from_kdl(input).expect("fallback palette");
        assert_eq!(parsed.fg, Color::Rgb(0xAA, 0xBB, 0xCC));
        assert_eq!(parsed.bg, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(parsed.blue, Color::Rgb(0x44, 0x55, 0x66));
    }
}

fn load_active_theme_name() -> io::Result<Option<String>> {
    let profile_path = config_dir().join("aoc/theme.env");
    if !profile_path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(profile_path)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("export AOC_THEME_NAME=") {
            continue;
        }
        let raw = trimmed.trim_start_matches("export AOC_THEME_NAME=").trim();
        let value = raw.trim_matches('"').trim_matches('\'');
        if !value.is_empty() {
            return Ok(Some(value.to_string()));
        }
    }

    Ok(None)
}

fn load_effective_theme_name() -> io::Result<Option<String>> {
    let profile_path = config_dir().join("aoc/theme.env");
    if !profile_path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(profile_path)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("export AOC_THEME_EFFECTIVE_NAME=") {
            continue;
        }
        let raw = trimmed
            .trim_start_matches("export AOC_THEME_EFFECTIVE_NAME=")
            .trim();
        let value = raw.trim_matches('"').trim_matches('\'');
        if !value.is_empty() {
            return Ok(Some(value.to_string()));
        }
    }

    Ok(None)
}

fn load_background_profile_name() -> io::Result<String> {
    let profile_path = config_dir().join("aoc/theme.env");
    if !profile_path.exists() {
        return Ok("follow-theme".to_string());
    }

    let contents = fs::read_to_string(profile_path)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("export AOC_THEME_BG_PROFILE=") {
            continue;
        }
        let raw = trimmed
            .trim_start_matches("export AOC_THEME_BG_PROFILE=")
            .trim();
        let value = raw.trim_matches('"').trim_matches('\'');
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }

    Ok("follow-theme".to_string())
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

fn project_root_path() -> Option<PathBuf> {
    env::current_dir().ok()
}

fn project_relative_exists(relative: &str) -> bool {
    project_root_path()
        .map(|root| root.join(relative))
        .map(|path| path.exists())
        .unwrap_or(false)
}

fn project_relative_is_dir(relative: &str) -> bool {
    project_root_path()
        .map(|root| root.join(relative))
        .map(|path| path.is_dir())
        .unwrap_or(false)
}

fn agent_browser_bin_name() -> String {
    env::var("AOC_AGENT_BROWSER_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "agent-browser".to_string())
}

fn agent_browser_installed() -> bool {
    binary_in_path(&agent_browser_bin_name())
}

fn probe_agent_browser_runtime_ready() -> bool {
    if !agent_browser_installed() {
        return false;
    }

    let bin = agent_browser_bin_name();
    let probe = format!("{bin} open about:blank >/dev/null 2>&1 && {bin} close >/dev/null 2>&1");

    match Command::new("bash")
        .args(["-lc", &probe])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => match wait_with_timeout(child, Duration::from_secs(20)) {
            Ok(status) => status.success(),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

fn agent_browser_skill_installed() -> bool {
    project_relative_exists(".pi/skills/agent-browser/SKILL.md")
}

fn agent_browser_summary_with_runtime(runtime_checked: bool, runtime_ready: bool) -> String {
    let tool = if agent_browser_installed() {
        "tool installed"
    } else {
        "tool missing"
    };
    let runtime = if !runtime_checked {
        "runtime unchecked"
    } else if runtime_ready {
        "runtime ready"
    } else if agent_browser_installed() {
        "runtime missing"
    } else {
        "runtime unknown"
    };
    let skill = if agent_browser_skill_installed() {
        "skill present"
    } else {
        "skill missing"
    };
    format!("{tool}, {runtime}, {skill}")
}

fn search_config_path() -> Option<PathBuf> {
    project_root_path().map(|root| root.join(".aoc/search.toml"))
}

fn managed_search_dir() -> Option<PathBuf> {
    project_root_path().map(|root| root.join(".aoc/services/searxng"))
}

fn managed_search_compose_path() -> Option<PathBuf> {
    managed_search_dir().map(|dir| dir.join("docker-compose.yml"))
}

fn managed_search_settings_path() -> Option<PathBuf> {
    managed_search_dir().map(|dir| dir.join("settings.yml"))
}

fn search_config_exists() -> bool {
    search_config_path()
        .map(|path| path.exists())
        .unwrap_or(false)
}

fn docker_installed() -> bool {
    binary_in_path("docker")
}

fn docker_compose_available() -> bool {
    if !docker_installed() {
        return false;
    }

    let docker_compose = Command::new("docker")
        .args(["compose", "version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    docker_compose || binary_in_path("docker-compose")
}

fn run_docker_compose(args: &[&str]) -> io::Result<std::process::Output> {
    if docker_installed() {
        let output = Command::new("docker")
            .args(["compose"])
            .args(args)
            .output()?;
        if output.status.success() {
            return Ok(output);
        }
        if !binary_in_path("docker-compose") {
            return Err(command_failure(
                &format!("docker compose {}", args.join(" ")),
                &output,
            ));
        }
    }

    if binary_in_path("docker-compose") {
        let output = Command::new("docker-compose").args(args).output()?;
        if output.status.success() {
            return Ok(output);
        }
        return Err(command_failure(
            &format!("docker-compose {}", args.join(" ")),
            &output,
        ));
    }

    Err(io::Error::other("Docker Compose not available"))
}

fn parse_toml_string_literal(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_toml_bool(value: &str) -> bool {
    matches!(value.trim(), "true" | "1")
}

fn load_search_config() -> io::Result<SearchConfig> {
    let path =
        search_config_path().ok_or_else(|| io::Error::other("unable to resolve project root"))?;
    let content = fs::read_to_string(&path)?;
    let mut section = String::new();
    let mut enabled = None;
    let mut provider = None;
    let mut managed = None;
    let mut auto_start = None;
    let mut base_url = None;
    let mut healthcheck_url = None;
    let mut compose_file = None;
    let mut service_name = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(&['[', ']'][..]).to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match (section.as_str(), key) {
            ("search", "enabled") => enabled = Some(parse_toml_bool(value)),
            ("search", "provider") => provider = Some(parse_toml_string_literal(value)),
            ("search", "managed") => managed = Some(parse_toml_bool(value)),
            ("search", "auto_start") => auto_start = Some(parse_toml_bool(value)),
            ("search.runtime", "base_url") => base_url = Some(parse_toml_string_literal(value)),
            ("search.runtime", "healthcheck_url") => {
                healthcheck_url = Some(parse_toml_string_literal(value))
            }
            ("search.runtime", "compose_file") => {
                compose_file = Some(parse_toml_string_literal(value))
            }
            ("search.runtime", "service_name") => {
                service_name = Some(parse_toml_string_literal(value))
            }
            _ => {}
        }
    }

    Ok(SearchConfig {
        enabled: enabled.ok_or_else(|| io::Error::other("missing search.enabled"))?,
        provider: provider.ok_or_else(|| io::Error::other("missing search.provider"))?,
        managed: managed.ok_or_else(|| io::Error::other("missing search.managed"))?,
        auto_start: auto_start.ok_or_else(|| io::Error::other("missing search.auto_start"))?,
        base_url: base_url.ok_or_else(|| io::Error::other("missing search.runtime.base_url"))?,
        healthcheck_url: healthcheck_url
            .ok_or_else(|| io::Error::other("missing search.runtime.healthcheck_url"))?,
        compose_file: compose_file
            .ok_or_else(|| io::Error::other("missing search.runtime.compose_file"))?,
        service_name: service_name
            .ok_or_else(|| io::Error::other("missing search.runtime.service_name"))?,
    })
}

fn search_compose_file_path(config: &SearchConfig) -> Option<PathBuf> {
    project_root_path().map(|root| root.join(&config.compose_file))
}

fn probe_search_health(config: &SearchConfig) -> io::Result<bool> {
    if binary_in_path("curl") {
        let output = Command::new("curl")
            .args([
                "-fsSL",
                "--connect-timeout",
                "3",
                "--max-time",
                "10",
                &config.healthcheck_url,
            ])
            .output()?;
        if !output.status.success() {
            return Ok(false);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Ok(!stdout.trim().is_empty());
    }

    if binary_in_path("wget") {
        let output = Command::new("wget")
            .args(["-q", "-T", "10", "-O", "-", &config.healthcheck_url])
            .output()?;
        if !output.status.success() {
            return Ok(false);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Ok(!stdout.trim().is_empty());
    }

    Err(io::Error::other(
        "curl or wget is required to verify managed search health",
    ))
}

fn load_search_status() -> SearchStatusSummary {
    if !search_config_exists() {
        return SearchStatusSummary::default();
    }

    let config = match load_search_config() {
        Ok(config) => config,
        Err(err) => {
            return SearchStatusSummary {
                configured: true,
                enabled: true,
                managed: false,
                runtime_status: "error".to_string(),
                healthy: false,
                message: format!("Search config invalid: {err}"),
            }
        }
    };

    if !config.enabled {
        return SearchStatusSummary {
            configured: true,
            enabled: false,
            managed: config.managed,
            runtime_status: "disabled".to_string(),
            healthy: false,
            message: "Search is disabled in .aoc/search.toml".to_string(),
        };
    }

    if config.managed && !docker_compose_available() {
        return SearchStatusSummary {
            configured: true,
            enabled: true,
            managed: true,
            runtime_status: "error".to_string(),
            healthy: false,
            message: "Docker Compose not available".to_string(),
        };
    }

    let Some(compose_file) = search_compose_file_path(&config) else {
        return SearchStatusSummary {
            configured: true,
            enabled: true,
            managed: config.managed,
            runtime_status: "error".to_string(),
            healthy: false,
            message: "Unable to resolve compose file path".to_string(),
        };
    };

    if config.managed && !compose_file.exists() {
        return SearchStatusSummary {
            configured: true,
            enabled: true,
            managed: true,
            runtime_status: "error".to_string(),
            healthy: false,
            message: "Managed search compose file missing".to_string(),
        };
    }

    if config.managed {
        let compose_arg = compose_file.to_string_lossy().to_string();
        let service_arg = config.service_name.clone();
        match run_docker_compose(&["-f", &compose_arg, "ps", "-q", &service_arg]) {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.trim().is_empty() {
                    return SearchStatusSummary {
                        configured: true,
                        enabled: true,
                        managed: true,
                        runtime_status: "stopped".to_string(),
                        healthy: false,
                        message: "Managed search is configured but stopped".to_string(),
                    };
                }
            }
            Err(err) => {
                return SearchStatusSummary {
                    configured: true,
                    enabled: true,
                    managed: true,
                    runtime_status: "error".to_string(),
                    healthy: false,
                    message: format!("Managed search status failed: {err}"),
                }
            }
        }
    }

    match probe_search_health(&config) {
        Ok(true) => SearchStatusSummary {
            configured: true,
            enabled: true,
            managed: config.managed,
            runtime_status: "healthy".to_string(),
            healthy: true,
            message: format!(
                "Managed SearXNG is running and healthy at {}",
                config.base_url
            ),
        },
        Ok(false) => SearchStatusSummary {
            configured: true,
            enabled: true,
            managed: config.managed,
            runtime_status: "unhealthy".to_string(),
            healthy: false,
            message: "Managed search is running but health check failed".to_string(),
        },
        Err(err) => SearchStatusSummary {
            configured: true,
            enabled: true,
            managed: config.managed,
            runtime_status: "error".to_string(),
            healthy: false,
            message: format!("Managed search health check failed: {err}"),
        },
    }
}

fn project_bin_path(name: &str) -> Option<PathBuf> {
    project_root_path().map(|root| root.join("bin").join(name))
}

fn run_project_bin_capture(name: &str, args: &[&str]) -> io::Result<std::process::Output> {
    let path =
        project_bin_path(name).ok_or_else(|| io::Error::other("unable to resolve project root"))?;
    Command::new(path).args(args).output()
}

fn parse_search_status_json(value: &JsonValue) -> SearchStatusSummary {
    let object = value.as_object();
    let configured = object
        .and_then(|obj| obj.get("configured"))
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let enabled = object
        .and_then(|obj| obj.get("enabled"))
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let managed = object
        .and_then(|obj| obj.get("managed"))
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let runtime_status = object
        .and_then(|obj| obj.get("runtimeStatus"))
        .and_then(JsonValue::as_str)
        .unwrap_or("error")
        .to_string();
    let healthy = object
        .and_then(|obj| obj.get("healthy"))
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let message = object
        .and_then(|obj| obj.get("message"))
        .and_then(JsonValue::as_str)
        .unwrap_or("Search status unavailable")
        .to_string();

    SearchStatusSummary {
        configured,
        enabled,
        managed,
        runtime_status,
        healthy,
        message,
    }
}

fn load_search_status_via_cli() -> io::Result<SearchStatusSummary> {
    let output = run_project_bin_capture("aoc-search", &["status", "--json"])?;
    if !output.status.success() {
        return Err(command_failure("bin/aoc-search status --json", &output));
    }
    let value: JsonValue = serde_json::from_slice(&output.stdout)
        .map_err(|err| io::Error::other(format!("invalid aoc-search status json: {err}")))?;
    Ok(parse_search_status_json(&value))
}

fn search_summary(status: &SearchStatusSummary) -> String {
    status.runtime_status.clone()
}

fn push_search_job_detail(lines: &mut Vec<Line<'static>>, app: &App, action: &str) {
    if let Some(job) = app.search_job.as_ref().filter(|job| job.action == action) {
        lines.push(Line::from(""));
        lines.push(Line::from(format!("Running: {}", job.action)));
        lines.push(Line::from(format!(
            "Log: {}",
            job.log_path.to_string_lossy()
        )));
        lines.push(Line::from(
            "PgUp/PgDn scroll · x cancel · Shift+O open full log",
        ));
        if app.search_log_tail.is_empty() {
            lines.push(Line::from("(waiting for log output...)"));
        } else {
            let visible = 12usize;
            let max_start = app.search_log_tail.len().saturating_sub(visible);
            let start = app.search_log_scroll.min(max_start);
            let end = (start + visible).min(app.search_log_tail.len());
            lines.push(Line::from(format!(
                "Recent output: lines {}-{} of {}",
                start + 1,
                end,
                app.search_log_tail.len()
            )));
            for line in &app.search_log_tail[start..end] {
                lines.push(Line::from(format!("  {line}")));
            }
        }
    } else if let Some(log_path) = latest_search_log_path() {
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "Latest log: {}",
            log_path.to_string_lossy()
        )));
        lines.push(Line::from("Shift+O opens the full log in pager."));
    }
}

fn agent_browser_search_summary(
    runtime_checked: bool,
    runtime_ready: bool,
    search_status: &SearchStatusSummary,
) -> String {
    let browser = if agent_browser_installed() {
        "browser installed"
    } else {
        "browser missing"
    };
    let browser_runtime = if !runtime_checked {
        "runtime unchecked"
    } else if runtime_ready {
        "runtime ready"
    } else if agent_browser_installed() {
        "runtime missing"
    } else {
        "runtime unknown"
    };
    let search = format!("search {}", search_summary(search_status));
    let browser_skill = if agent_browser_skill_installed() {
        "browser skill present"
    } else {
        "browser skill missing"
    };
    let research_skill = if web_research_skill_installed() {
        "research skill present"
    } else {
        "research skill missing"
    };
    format!("{browser}, {browser_runtime}, {search}, {browser_skill}, {research_skill}")
}

fn managed_search_config_contents() -> String {
    r#"version = 1

[search]
enabled = true
provider = "searxng"
managed = true
auto_start = true

[search.runtime]
base_url = "http://127.0.0.1:8888"
healthcheck_url = "http://127.0.0.1:8888/search?q=aoc+health&format=json"
compose_file = ".aoc/services/searxng/docker-compose.yml"
service_name = "searxng"

[search.query_defaults]
format = "json"
language = "en"
categories = "general"
safe_search = 0

[search.agent_policy]
allow_auto_start = true
prompt_when_missing = true
prompt_when_unhealthy = true
"#
    .to_string()
}

fn managed_search_compose_contents() -> String {
    r#"services:
  searxng:
    image: docker.io/searxng/searxng:latest
    container_name: aoc-searxng
    ports:
      - "127.0.0.1:8888:8080"
    volumes:
      - ./settings.yml:/etc/searxng/settings.yml
    restart: unless-stopped
"#
    .to_string()
}

fn managed_search_settings_contents() -> String {
    r#"use_default_settings: true
server:
  secret_key: "aoc-searxng-local-dev-key"
  bind_address: "0.0.0.0"
  port: 8080
  limiter: false
search:
  safe_search: 0
  autocomplete: ""
  formats:
    - html
    - json
    - csv
    - rss
engines:
  - name: google
    disabled: true
  - name: duckduckgo
    disabled: true
  - name: brave
    disabled: true
  - name: ahmia
    disabled: true
  - name: torch
    disabled: true
"#
    .to_string()
}

fn enable_managed_search() -> io::Result<String> {
    if !docker_compose_available() {
        return Err(io::Error::other(
            "Docker Compose not found; install Docker/Compose before enabling search",
        ));
    }

    let config_path =
        search_config_path().ok_or_else(|| io::Error::other("unable to resolve project root"))?;
    let service_dir =
        managed_search_dir().ok_or_else(|| io::Error::other("unable to resolve project root"))?;
    let compose_path = managed_search_compose_path()
        .ok_or_else(|| io::Error::other("unable to resolve project root"))?;
    let settings_path = managed_search_settings_path()
        .ok_or_else(|| io::Error::other("unable to resolve project root"))?;

    fs::create_dir_all(config_path.parent().unwrap_or(Path::new(".")))?;
    fs::create_dir_all(&service_dir)?;
    fs::write(&config_path, managed_search_config_contents())?;
    fs::write(&compose_path, managed_search_compose_contents())?;
    fs::write(&settings_path, managed_search_settings_contents())?;

    let browser_skill_message = install_agent_browser_skill()?;
    let research_skill_message = install_web_research_skill()?;

    Ok(format!(
        "Managed search enabled (.aoc/search.toml + .aoc/services/searxng/* written); {browser_skill_message}; {research_skill_message}"
    ))
}

fn web_research_skill_installed() -> bool {
    project_relative_exists(".pi/skills/web-research/SKILL.md")
}

fn install_web_research_skill() -> io::Result<String> {
    let Some(project_root) = project_root_path() else {
        return Err(io::Error::other("unable to resolve project root"));
    };

    let target_dir = project_root.join(".pi").join("skills").join("web-research");
    fs::create_dir_all(&target_dir)?;
    let target_file = target_dir.join("SKILL.md");
    fs::write(
        &target_file,
        include_str!("../../../.pi/skills/web-research/SKILL.md"),
    )?;

    Ok(format!(
        "Synced web research skill ({})",
        target_file.to_string_lossy()
    ))
}

fn agent_browser_skill_url() -> String {
    env::var("AOC_AGENT_BROWSER_SKILL_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            "https://raw.githubusercontent.com/vercel-labs/agent-browser/main/skills/agent-browser/SKILL.md"
                .to_string()
        })
}

fn install_agent_browser_skill() -> io::Result<String> {
    let Some(project_root) = project_root_path() else {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "unable to resolve project root",
        ));
    };

    let target_dir = project_root
        .join(".pi")
        .join("skills")
        .join("agent-browser");
    fs::create_dir_all(&target_dir)?;
    let target_file = target_dir.join("SKILL.md");
    let url = agent_browser_skill_url();

    let output = if binary_in_path("curl") {
        Command::new("curl")
            .args([
                "-fsSL",
                "--connect-timeout",
                "10",
                "--max-time",
                "120",
                &url,
                "-o",
                &target_file.to_string_lossy(),
            ])
            .output()?
    } else if binary_in_path("wget") {
        Command::new("wget")
            .args([
                "-q",
                "--timeout=10",
                "-O",
                &target_file.to_string_lossy(),
                &url,
            ])
            .output()?
    } else {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "curl or wget is required to install Agent Browser skill",
        ));
    };

    if !output.status.success() {
        return Err(command_failure(
            &format!("download agent-browser skill from {url}"),
            &output,
        ));
    }

    Ok(format!(
        "Synced Agent Browser skill ({})",
        target_file.to_string_lossy()
    ))
}

fn vercel_bin_name() -> String {
    env::var("AOC_VERCEL_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "vercel".to_string())
}

fn vercel_installed() -> bool {
    binary_in_path(&vercel_bin_name())
}

fn vercel_skill_installed() -> bool {
    project_relative_exists(".pi/skills/vercel-cli/SKILL.md")
}

fn vercel_auth_configured() -> bool {
    env::var("VERCEL_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        || home_dir().join(".vercel/auth.json").exists()
}

fn vercel_summary() -> String {
    let tool = if vercel_installed() {
        "tool installed"
    } else {
        "tool missing"
    };
    let auth = if vercel_auth_configured() {
        "auth configured"
    } else {
        "auth not configured"
    };
    let skill = if vercel_skill_installed() {
        "skill present"
    } else {
        "skill missing"
    };
    format!("{tool}, {auth}, {skill}")
}

fn install_vercel_skill() -> io::Result<String> {
    let Some(project_root) = project_root_path() else {
        return Err(io::Error::other("unable to resolve project root"));
    };

    let target_dir = project_root.join(".pi").join("skills").join("vercel-cli");
    fs::create_dir_all(&target_dir)?;
    let target_file = target_dir.join("SKILL.md");
    fs::write(
        &target_file,
        include_str!("../../../.pi/skills/vercel-cli/SKILL.md"),
    )?;

    Ok(format!(
        "Synced Vercel skill ({})",
        target_file.to_string_lossy()
    ))
}

fn default_vercel_install_cmd() -> String {
    "if command -v pnpm >/dev/null 2>&1; then pnpm add -g vercel; elif command -v npm >/dev/null 2>&1; then npm install -g --prefix \"${AOC_NPM_GLOBAL_PREFIX:-$HOME/.local}\" vercel; elif command -v corepack >/dev/null 2>&1; then corepack enable && corepack prepare pnpm@latest --activate && pnpm add -g vercel; else echo 'pnpm/npm/corepack not found' >&2; exit 1; fi && vercel --version".to_string()
}

fn default_vercel_update_cmd() -> String {
    "if command -v pnpm >/dev/null 2>&1; then pnpm add -g vercel@latest; elif command -v npm >/dev/null 2>&1; then npm install -g --prefix \"${AOC_NPM_GLOBAL_PREFIX:-$HOME/.local}\" vercel@latest; elif command -v corepack >/dev/null 2>&1; then corepack enable && corepack prepare pnpm@latest --activate && pnpm add -g vercel@latest; else echo 'pnpm/npm/corepack not found' >&2; exit 1; fi && vercel --version".to_string()
}

fn resolve_vercel_cmd(action: &str) -> String {
    match action {
        "install" => env::var("AOC_VERCEL_INSTALL_CMD")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(default_vercel_install_cmd),
        _ => env::var("AOC_VERCEL_UPDATE_CMD")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                env::var("AOC_VERCEL_INSTALL_CMD")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(default_vercel_update_cmd),
    }
}

fn run_vercel_tool_command(action: &str) -> io::Result<String> {
    let cmd = resolve_vercel_cmd(action);
    let output = Command::new("bash").args(["-lc", &cmd]).output()?;
    if !output.status.success() {
        return Err(command_failure(&format!("vercel {action}"), &output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("Vercel CLI updated")
        .trim()
        .to_string();

    Ok(format!("{first_line} ({})", vercel_summary()))
}

fn verify_vercel_cli() -> io::Result<String> {
    let bin = vercel_bin_name();
    let output = Command::new(&bin).arg("--version").output()?;
    if !output.status.success() {
        return Err(command_failure(&format!("{bin} --version"), &output));
    }

    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("version reported")
        .trim()
        .to_string();

    Ok(format!("Vercel CLI {version} ({})", vercel_summary()))
}

fn aoc_map_skill_installed() -> bool {
    project_relative_exists(".pi/skills/aoc-map/SKILL.md")
}

fn install_aoc_map_skill() -> io::Result<String> {
    let Some(project_root) = project_root_path() else {
        return Err(io::Error::other("unable to resolve project root"));
    };

    let target_dir = project_root.join(".pi").join("skills").join("aoc-map");
    fs::create_dir_all(&target_dir)?;
    let target_file = target_dir.join("SKILL.md");
    fs::write(
        &target_file,
        include_str!("../../../.pi/skills/aoc-map/SKILL.md"),
    )?;

    Ok(format!(
        "Synced AOC Map skill ({})",
        target_file.to_string_lossy()
    ))
}

fn aoc_map_summary() -> String {
    let map_root = project_relative_is_dir(".aoc/map")
        || project_relative_is_dir(".aoc/see")
        || project_relative_is_dir(".aoc/diagrams");
    let pages = project_relative_is_dir(".aoc/map/pages")
        || project_relative_is_dir(".aoc/see/pages")
        || project_relative_is_dir(".aoc/diagrams/pages");
    let manifest = project_relative_exists(".aoc/map/manifest.json")
        || project_relative_exists(".aoc/see/manifest.json")
        || project_relative_exists(".aoc/diagrams/manifest.json");
    let index = project_relative_exists(".aoc/map/index.html")
        || project_relative_exists(".aoc/see/index.html")
        || project_relative_exists(".aoc/diagrams/index.html");
    let workspace = match (map_root, pages, manifest, index) {
        (true, true, true, true) => "workspace seeded",
        (true, _, _, _) => "workspace partial",
        _ => "workspace missing",
    };
    let skill = if aoc_map_skill_installed() {
        "skill present"
    } else {
        "skill missing"
    };
    format!("{workspace}, {skill}")
}

fn run_aoc_map_init() -> io::Result<String> {
    let project_root =
        project_root_path().ok_or_else(|| io::Error::other("unable to resolve project root"))?;
    let output = Command::new("aoc-map")
        .args(["init"])
        .current_dir(project_root)
        .output()?;
    if !output.status.success() {
        return Err(command_failure("aoc-map init", &output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("AOC Map initialized")
        .trim()
        .to_string();

    Ok(format!("{first_line} ({})", aoc_map_summary()))
}

fn seed_aoc_map() -> io::Result<String> {
    let skill_message = install_aoc_map_skill()?;
    let init_message = run_aoc_map_init()?;
    Ok(format!("{skill_message}; {init_message}"))
}

fn hyperframes_summary() -> String {
    let nested = if project_relative_is_dir("hyperframes") {
        "workspace present"
    } else {
        "workspace missing"
    };
    let prompt = if project_relative_exists(".pi/prompts/hyperframes.md") {
        "prompt present"
    } else {
        "prompt missing"
    };
    let skills = if project_relative_is_dir(".pi/skills/hyperframes") {
        "skills present"
    } else {
        "skills missing"
    };
    format!("{nested}, {prompt}, {skills}")
}

fn default_agent_browser_install_cmd() -> String {
    r#"set -e
if command -v pnpm >/dev/null 2>&1; then
  pnpm add -g agent-browser
elif command -v npm >/dev/null 2>&1; then
  npm install -g --prefix "${AOC_NPM_GLOBAL_PREFIX:-$HOME/.local}" agent-browser
elif command -v corepack >/dev/null 2>&1; then
  corepack enable
  corepack prepare pnpm@latest --activate
  pnpm add -g agent-browser
else
  echo 'pnpm/npm/corepack not found' >&2
  exit 1
fi

pw_version=$(python - <<'PY'
import glob, json
paths=sorted(glob.glob('/home/' + __import__('os').path.expanduser('~').split('/')[-1] + '/.local/share/pnpm/global/5/.pnpm/agent-browser@*/node_modules/agent-browser/package.json'))
if not paths:
    paths=sorted(glob.glob(__import__('os').path.expanduser('~/.local/share/pnpm/global/5/.pnpm/agent-browser@*/node_modules/agent-browser/package.json')))
if not paths:
    raise SystemExit(1)
with open(paths[-1]) as f:
    pkg=json.load(f)
ver=pkg.get('dependencies',{}).get('playwright-core') or pkg.get('devDependencies',{}).get('playwright') or ''
print(ver.lstrip('^~'))
PY
)

if [ -z "$pw_version" ]; then
  echo 'Could not resolve Agent Browser Playwright version' >&2
  exit 1
fi

if command -v pnpm >/dev/null 2>&1; then
  pnpm add -g "playwright@$pw_version"
else
  npm install -g --prefix "${AOC_NPM_GLOBAL_PREFIX:-$HOME/.local}" "playwright@$pw_version"
fi

playwright install chromium chromium-headless-shell
agent-browser open about:blank >/dev/null 2>&1
agent-browser close >/dev/null 2>&1 || true"#
        .to_string()
}

fn default_agent_browser_update_cmd() -> String {
    r#"set -e
if command -v pnpm >/dev/null 2>&1; then
  pnpm add -g agent-browser@latest
elif command -v npm >/dev/null 2>&1; then
  npm install -g --prefix "${AOC_NPM_GLOBAL_PREFIX:-$HOME/.local}" agent-browser@latest
elif command -v corepack >/dev/null 2>&1; then
  corepack enable
  corepack prepare pnpm@latest --activate
  pnpm add -g agent-browser@latest
else
  echo 'pnpm/npm/corepack not found' >&2
  exit 1
fi

pw_version=$(python - <<'PY'
import glob, json
paths=sorted(glob.glob('/home/' + __import__('os').path.expanduser('~').split('/')[-1] + '/.local/share/pnpm/global/5/.pnpm/agent-browser@*/node_modules/agent-browser/package.json'))
if not paths:
    paths=sorted(glob.glob(__import__('os').path.expanduser('~/.local/share/pnpm/global/5/.pnpm/agent-browser@*/node_modules/agent-browser/package.json')))
if not paths:
    raise SystemExit(1)
with open(paths[-1]) as f:
    pkg=json.load(f)
ver=pkg.get('dependencies',{}).get('playwright-core') or pkg.get('devDependencies',{}).get('playwright') or ''
print(ver.lstrip('^~'))
PY
)

if [ -z "$pw_version" ]; then
  echo 'Could not resolve Agent Browser Playwright version' >&2
  exit 1
fi

if command -v pnpm >/dev/null 2>&1; then
  pnpm add -g "playwright@$pw_version"
else
  npm install -g --prefix "${AOC_NPM_GLOBAL_PREFIX:-$HOME/.local}" "playwright@$pw_version"
fi

playwright install chromium chromium-headless-shell
agent-browser open about:blank >/dev/null 2>&1
agent-browser close >/dev/null 2>&1 || true"#
        .to_string()
}

fn resolve_agent_browser_cmd(action: &str) -> String {
    match action {
        "install" => env::var("AOC_AGENT_BROWSER_INSTALL_CMD")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(default_agent_browser_install_cmd),
        _ => env::var("AOC_AGENT_BROWSER_UPDATE_CMD")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                env::var("AOC_AGENT_BROWSER_INSTALL_CMD")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(default_agent_browser_update_cmd),
    }
}

fn agent_browser_log_path(action: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    env::temp_dir().join(format!(
        "aoc-control-agent-browser-{action}-{}-{stamp}.log",
        std::process::id()
    ))
}

fn search_log_path(action: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    env::temp_dir().join(format!(
        "aoc-control-search-{action}-{}-{stamp}.log",
        std::process::id()
    ))
}

fn spawn_agent_browser_command(action: &str) -> io::Result<AgentBrowserJob> {
    let cmd = resolve_agent_browser_cmd(action);
    let log_path = agent_browser_log_path(action);
    let log_file = fs::File::create(&log_path)?;
    let log_file_err = log_file.try_clone()?;

    let child = Command::new("bash")
        .args(["-lc", &cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_err))
        .spawn()?;

    Ok(AgentBrowserJob {
        action: action.to_string(),
        log_path,
        child,
    })
}

fn spawn_search_command(
    action: &str,
    success_message: &str,
    bin_name: &str,
    args: &[&str],
) -> io::Result<SearchJob> {
    let path = project_bin_path(bin_name)
        .ok_or_else(|| io::Error::other("unable to resolve project root"))?;
    let log_path = search_log_path(action);
    let log_file = fs::File::create(&log_path)?;
    let log_file_err = log_file.try_clone()?;

    let child = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_err))
        .spawn()?;

    Ok(SearchJob {
        action: action.to_string(),
        success_message: success_message.to_string(),
        log_path,
        child,
    })
}

fn tail_file_lines(path: &Path, max_lines: usize, max_bytes: usize) -> io::Result<Vec<String>> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max_bytes as u64);
    if start > 0 {
        file.seek(SeekFrom::Start(start))?;
    }

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text.lines().map(|line| line.to_string()).collect();

    if start > 0 && !text.starts_with('\n') && !lines.is_empty() {
        lines.remove(0);
    }

    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }

    Ok(lines)
}

fn latest_agent_browser_log_path() -> Option<PathBuf> {
    latest_log_path_with_prefix("aoc-control-agent-browser-")
}

fn latest_search_log_path() -> Option<PathBuf> {
    latest_log_path_with_prefix("aoc-control-search-")
}

fn latest_log_path_with_prefix(prefix: &str) -> Option<PathBuf> {
    let mut entries: Vec<(SystemTime, PathBuf)> = fs::read_dir(env::temp_dir())
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !name.starts_with(prefix) || !name.ends_with(".log") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    entries.sort_by_key(|(modified, _)| *modified);
    entries.pop().map(|(_, path)| path)
}

fn open_log_in_pager(path: &Path) -> io::Result<()> {
    let pager = env::var("PAGER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "less".to_string());
    let status = with_cooked_mode(|| Command::new(&pager).arg(path).status())?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{pager} {} exited with status {status}",
            path.to_string_lossy()
        )))
    }
}

fn open_hyperframes_preview_pane() -> io::Result<String> {
    let root = project_root_path().unwrap_or_else(|| PathBuf::from("."));
    let workspace = root.join("hyperframes");
    if !workspace.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "HyperFrames workspace missing; run aoc-hyperframes init first",
        ));
    }

    let command = "printf '\033]2;HyperFrames Preview\007'; echo 'HyperFrames preview: http://localhost:3002'; npx hyperframes preview; exec bash";
    if in_zellij() {
        let status = Command::new("zellij")
            .args(["action", "new-pane", "--direction", "down", "--cwd"])
            .arg(&workspace)
            .args(["--", "bash", "-lc", command])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            return Ok("Started HyperFrames preview in a new pane below (usually http://localhost:3002)".to_string());
        }
        return Err(io::Error::new(io::ErrorKind::Other, "zellij new-pane failed"));
    }

    Err(io::Error::new(
        io::ErrorKind::Other,
        format!(
            "not running inside Zellij; run manually: cd {} && npx hyperframes preview",
            workspace.to_string_lossy()
        ),
    ))
}

fn run_hyperframes_command(args: &[&str]) -> io::Result<String> {
    let output = Command::new("aoc-hyperframes").args(args).output()?;
    let command_label = if args.is_empty() {
        "aoc-hyperframes".to_string()
    } else {
        format!("aoc-hyperframes {}", args.join(" "))
    };
    if !output.status.success() {
        return Err(command_failure(&command_label, &output));
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let base = if text.is_empty() {
        "HyperFrames integration updated".to_string()
    } else {
text.lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("HyperFrames integration updated")
            .trim()
            .to_string()
    };
    let status = hyperframes_summary();
    Ok(format!("{base} ({status})"))
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

fn load_agent_install_entries() -> Vec<AgentInstallEntry> {
    let mut entries = Vec::new();
    for (id, label) in agent_install_targets() {
        let installed = load_agent_install_status(id).unwrap_or(false);
        entries.push(AgentInstallEntry {
            id: id.to_string(),
            label: label.to_string(),
            installed,
        });
    }
    entries
}

fn load_agent_install_status(agent: &str) -> io::Result<bool> {
    let output = Command::new("aoc-agent-install")
        .args(["status", agent])
        .output()?;
    if !output.status.success() {
        return Err(command_failure(
            &format!("aoc-agent-install status {agent}"),
            &output,
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("missing")
        .trim()
        .to_ascii_lowercase();
    Ok(first_line.contains("installed"))
}

fn run_agent_install_command(action: &str, agent: &str) -> io::Result<String> {
    let output = Command::new("aoc-agent-install")
        .args([action, agent])
        .output()?;
    if !output.status.success() {
        return Err(command_failure(
            &format!("aoc-agent-install {action} {agent}"),
            &output,
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("Agent installer action completed")
        .trim()
        .to_string();
    Ok(first_line)
}

fn with_cooked_mode<T>(f: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, DisableMouseCapture);

    let was_raw = is_raw_mode_enabled().unwrap_or(false);
    if was_raw {
        disable_raw_mode()?;
    }

    let result = f();

    if was_raw {
        let _ = enable_raw_mode();
    }
    let mut stdout = io::stdout();
    let _ = execute!(stdout, EnableMouseCapture);

    result
}

fn run_theme_command_interactive(args: &[&str]) -> io::Result<()> {
    let status = with_cooked_mode(|| {
        Command::new("aoc-theme")
            .env("AOC_THEME_QUIET", "1")
            .args(args)
            .status()
    })?;
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

fn run_theme_apply_quiet(theme_name: &str) -> io::Result<()> {
    with_cooked_mode(|| {
        let mut child = Command::new("aoc-theme")
            .env("AOC_THEME_QUIET", "1")
            .env("AOC_THEME_SKIP_SYNC", "1")
            .args(["apply", "--name", theme_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        thread::spawn(move || {
            let _ = child.wait();
        });

        Ok(())
    })
}

fn run_layout_command_interactive(args: &[&str]) -> io::Result<()> {
    let status = with_cooked_mode(|| Command::new("aoc-layout").args(args).status())?;
    if status.success() {
        Ok(())
    } else {
        let rendered = if args.is_empty() {
            "aoc-layout".to_string()
        } else {
            format!("aoc-layout {}", args.join(" "))
        };
        Err(io::Error::other(format!(
            "{rendered} exited with status {status}"
        )))
    }
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

fn wait_with_timeout(mut child: Child, timeout: Duration) -> io::Result<ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("timed out after {}ms", timeout.as_millis()),
            ));
        }

        thread::sleep(Duration::from_millis(20));
    }
}

fn load_pi_compaction_status() -> io::Result<PiCompactionStatus> {
    let path = pi_global_settings_path();
    let mut status = PiCompactionStatus::default();

    if path.exists() {
        let contents = fs::read_to_string(&path)?;
        if !contents.trim().is_empty() {
            let json: JsonValue = serde_json::from_str(&contents).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: {err}", path.display()),
                )
            })?;
            if let Some(compaction) = json.get("compaction") {
                if let Some(enabled) = compaction.get("enabled").and_then(JsonValue::as_bool) {
                    status.enabled = enabled;
                }
                if let Some(reserve_tokens) =
                    compaction.get("reserveTokens").and_then(JsonValue::as_u64)
                {
                    status.reserve_tokens = reserve_tokens;
                }
                if let Some(keep_recent_tokens) = compaction
                    .get("keepRecentTokens")
                    .and_then(JsonValue::as_u64)
                {
                    status.keep_recent_tokens = keep_recent_tokens;
                }
            }
        }
    }

    status.project_override = project_pi_compaction_override_exists();
    Ok(status)
}

fn save_pi_compaction_status(status: &PiCompactionStatus) -> io::Result<()> {
    let path = pi_global_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut root = if path.exists() {
        let contents = fs::read_to_string(&path)?;
        if contents.trim().is_empty() {
            JsonValue::Object(JsonMap::new())
        } else {
            serde_json::from_str::<JsonValue>(&contents).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: {err}", path.display()),
                )
            })?
        }
    } else {
        JsonValue::Object(JsonMap::new())
    };

    let root_object = match &mut root {
        JsonValue::Object(object) => object,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} must contain a JSON object", path.display()),
            ));
        }
    };

    let compaction = root_object
        .entry("compaction".to_string())
        .or_insert_with(|| JsonValue::Object(JsonMap::new()));
    let compaction_object = match compaction {
        JsonValue::Object(object) => object,
        _ => {
            *compaction = JsonValue::Object(JsonMap::new());
            match compaction {
                JsonValue::Object(object) => object,
                _ => unreachable!(),
            }
        }
    };

    compaction_object.insert("enabled".to_string(), JsonValue::Bool(status.enabled));
    compaction_object.insert(
        "reserveTokens".to_string(),
        JsonValue::Number(status.reserve_tokens.into()),
    );
    compaction_object.insert(
        "keepRecentTokens".to_string(),
        JsonValue::Number(status.keep_recent_tokens.into()),
    );

    let contents = serde_json::to_string_pretty(&root)
        .map_err(|err| io::Error::other(format!("serialize pi settings failed: {err}")))?;
    fs::write(path, format!("{contents}\n"))
}

fn pi_global_settings_path() -> PathBuf {
    home_dir().join(".pi/agent/settings.json")
}

fn project_pi_compaction_override_exists() -> bool {
    let Some(project_root) = find_project_root() else {
        return false;
    };
    let path = project_root.join(".pi/settings.json");
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    if contents.trim().is_empty() {
        return false;
    }
    match serde_json::from_str::<JsonValue>(&contents) {
        Ok(value) => value.get("compaction").is_some(),
        Err(_) => false,
    }
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

fn normalize_layout_name(layout: &str) -> String {
    match layout {
        "" => "aoc".to_string(),
        "unstat" | "minimal" | "aoc-zjstatus-single" | "aoc-zjstatus-test" | "aoc.hybrid" => {
            "aoc".to_string()
        }
        _ => layout.to_string(),
    }
}

fn layout_is_hidden_internal(layout: &str) -> bool {
    matches!(
        layout,
        "unstat"
            | "minimal"
            | "mission-control"
            | "aoc-zjstatus-single"
            | "aoc-zjstatus-test"
            | "aoc.hybrid"
    )
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

fn custom_layout_options() -> Vec<String> {
    layout_options()
        .into_iter()
        .filter(|layout| layout != "aoc")
        .collect()
}

fn append_layout_options(layouts_dir: &Path, options: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(layouts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "kdl") {
                if let Some(name) = path.file_stem() {
                    let name = normalize_layout_name(&name.to_string_lossy());
                    if layout_is_hidden_internal(&name) {
                        continue;
                    }
                    options.push(name);
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
    vec!["pi".to_string()]
}

fn background_profile_options() -> Vec<String> {
    vec![
        "follow-theme".to_string(),
        "deeper".to_string(),
        "softer".to_string(),
        "high-contrast".to_string(),
        "low-glare".to_string(),
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

fn run_open_in_zellij(
    path: &Path,
    overrides: &SessionOverrides,
    default_agent: &str,
) -> io::Result<()> {
    let mut cmd = Command::new("aoc-new-tab");
    cmd.arg("--cwd").arg(path);
    for (key, value) in build_env_overrides(overrides, default_agent) {
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

fn build_env_overrides(overrides: &SessionOverrides, default_agent: &str) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    if let Some(layout) = overrides.layout.clone() {
        envs.push(("AOC_LAYOUT".to_string(), layout));
    }
    let agent = overrides
        .agent
        .clone()
        .unwrap_or_else(|| default_agent.to_string());
    if !agent.trim().is_empty() {
        envs.push(("AOC_LAUNCH_AGENT_ID".to_string(), agent));
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
