use aoc_core::{ProjectData, TaskStatus};
use chrono::{DateTime, Utc};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use url::Url;

const PROTOCOL_VERSION: &str = "1";
const LOCAL_REFRESH_SECS: u64 = 1;
const HUB_STALE_SECS: i64 = 45;
const HUB_PRUNE_SECS: i64 = 90;
const HUB_OFFLINE_GRACE_SECS: i64 = 12;
const HUB_LOCAL_MISS_PRUNE_SECS: i64 = 0;
const MAX_DIFF_FILES: usize = 8;
const COMPACT_WIDTH: u16 = 92;

#[derive(Clone, Debug)]
struct Config {
    session_id: String,
    hub_url: Url,
    client_id: String,
    project_root: PathBuf,
    state_dir: PathBuf,
}

#[derive(Deserialize, Serialize)]
struct Envelope {
    version: String,
    #[serde(rename = "type")]
    r#type: String,
    session_id: String,
    sender_id: String,
    timestamp: String,
    payload: Value,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct HelloPayload {
    client_id: String,
    role: String,
    capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct AgentStatusPayload {
    agent_id: String,
    status: String,
    pane_id: String,
    project_root: String,
    #[serde(default)]
    agent_label: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct HeartbeatPayload {
    agent_id: String,
    pid: i32,
    cwd: String,
    last_update: String,
    #[serde(default)]
    pane_id: Option<String>,
    #[serde(default)]
    project_root: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffCounts {
    files: u32,
    additions: u32,
    deletions: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct UntrackedCounts {
    files: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffSummaryCounts {
    #[serde(default)]
    staged: DiffCounts,
    #[serde(default)]
    unstaged: DiffCounts,
    #[serde(default)]
    untracked: UntrackedCounts,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffFile {
    path: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    additions: u32,
    #[serde(default)]
    deletions: u32,
    #[serde(default)]
    staged: bool,
    #[serde(default)]
    untracked: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffSummaryPayload {
    agent_id: String,
    repo_root: String,
    #[serde(default)]
    git_available: bool,
    #[serde(default)]
    summary: DiffSummaryCounts,
    #[serde(default)]
    files: Vec<DiffFile>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct TaskCounts {
    #[serde(default)]
    total: u32,
    #[serde(default)]
    pending: u32,
    #[serde(default)]
    in_progress: u32,
    #[serde(default)]
    done: u32,
    #[serde(default)]
    blocked: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct ActiveTask {
    id: String,
    title: String,
    status: String,
    priority: String,
    active_agent: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct TaskSummaryPayload {
    agent_id: String,
    tag: String,
    counts: TaskCounts,
    #[serde(default)]
    active_tasks: Option<Vec<ActiveTask>>,
}

#[derive(Clone, Debug)]
struct HubAgent {
    status: Option<AgentStatusPayload>,
    last_seen: DateTime<Utc>,
    last_heartbeat: Option<DateTime<Utc>>,
    last_activity: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Default)]
struct HubCache {
    agents: HashMap<String, HubAgent>,
    tasks: HashMap<String, TaskSummaryPayload>,
    diffs: HashMap<String, DiffSummaryPayload>,
}

#[derive(Clone, Debug)]
struct OverviewRow {
    identity_key: String,
    label: String,
    pane_id: String,
    tab_index: Option<usize>,
    tab_name: Option<String>,
    tab_focused: bool,
    project_root: String,
    online: bool,
    age_secs: Option<i64>,
    source: String,
}

#[derive(Clone, Debug)]
struct TabMeta {
    index: usize,
    name: String,
    focused: bool,
}

#[derive(Clone, Debug, Default)]
struct SessionLayout {
    pane_ids: HashSet<String>,
    pane_tabs: HashMap<String, TabMeta>,
}

#[derive(Clone, Debug)]
struct WorkTagRow {
    tag: String,
    counts: TaskCounts,
    in_progress_titles: Vec<String>,
}

#[derive(Clone, Debug)]
struct WorkProject {
    project_root: String,
    scope: String,
    tags: Vec<WorkTagRow>,
}

#[derive(Clone, Debug)]
struct DiffProject {
    project_root: String,
    scope: String,
    git_available: bool,
    reason: Option<String>,
    summary: DiffSummaryCounts,
    files: Vec<DiffFile>,
}

#[derive(Clone, Debug)]
struct CheckOutcome {
    name: String,
    status: String,
    timestamp: Option<String>,
    details: Option<String>,
}

#[derive(Clone, Debug)]
struct DependencyStatus {
    name: String,
    available: bool,
    path: Option<String>,
}

#[derive(Clone, Debug)]
struct HealthSnapshot {
    dependencies: Vec<DependencyStatus>,
    checks: Vec<CheckOutcome>,
    taskmaster_status: String,
}

#[derive(Clone, Debug)]
struct LocalSnapshot {
    overview: Vec<OverviewRow>,
    work: Vec<WorkProject>,
    diff: Vec<DiffProject>,
    health: HealthSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Overview,
    Work,
    Diff,
    Health,
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Mode::Overview => "Overview",
            Mode::Work => "Work",
            Mode::Diff => "Diff",
            Mode::Health => "Health",
        }
    }

    fn next(self) -> Self {
        match self {
            Mode::Overview => Mode::Work,
            Mode::Work => Mode::Diff,
            Mode::Diff => Mode::Health,
            Mode::Health => Mode::Overview,
        }
    }
}

#[derive(Debug)]
enum HubEvent {
    Connected,
    Disconnected,
    AgentStatus {
        payload: AgentStatusPayload,
        event_at: DateTime<Utc>,
    },
    Heartbeat {
        payload: HeartbeatPayload,
        event_at: DateTime<Utc>,
    },
    TaskSummary {
        payload: TaskSummaryPayload,
        event_at: DateTime<Utc>,
    },
    DiffSummary {
        payload: DiffSummaryPayload,
        event_at: DateTime<Utc>,
    },
}

struct App {
    config: Config,
    connected: bool,
    hub: HubCache,
    local: LocalSnapshot,
    mode: Mode,
    scroll: u16,
    help_open: bool,
    selected_overview: usize,
    status_note: Option<String>,
}

impl App {
    fn new(config: Config, local: LocalSnapshot) -> Self {
        Self {
            config,
            connected: false,
            hub: HubCache::default(),
            local,
            mode: Mode::Overview,
            scroll: 0,
            help_open: false,
            selected_overview: 0,
            status_note: None,
        }
    }

    fn apply_hub_event(&mut self, event: HubEvent) {
        match event {
            HubEvent::Connected => self.connected = true,
            HubEvent::Disconnected => self.connected = false,
            HubEvent::AgentStatus { payload, event_at } => {
                let key = payload.agent_id.clone();
                let entry = self.hub.agents.entry(key).or_insert(HubAgent {
                    status: None,
                    last_seen: event_at,
                    last_heartbeat: None,
                    last_activity: None,
                });
                let has_message = payload
                    .message
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false);
                let reported_status = payload.status.to_ascii_lowercase();
                entry.status = Some(payload);
                entry.last_seen = event_at;
                if has_message || reported_status != "offline" {
                    entry.last_activity = Some(event_at);
                }
            }
            HubEvent::Heartbeat { payload, event_at } => {
                let key = payload.agent_id.clone();
                let entry = self.hub.agents.entry(key).or_insert(HubAgent {
                    status: None,
                    last_seen: event_at,
                    last_heartbeat: None,
                    last_activity: None,
                });
                entry.last_seen = event_at;
                entry.last_heartbeat = DateTime::parse_from_rfc3339(&payload.last_update)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
                    .or(Some(event_at));
            }
            HubEvent::TaskSummary { payload, event_at } => {
                let entry = self
                    .hub
                    .agents
                    .entry(payload.agent_id.clone())
                    .or_insert(HubAgent {
                        status: None,
                        last_seen: event_at,
                        last_heartbeat: None,
                        last_activity: None,
                    });
                entry.last_seen = event_at;
                entry.last_activity = Some(event_at);
                let key = format!("{}::{}", payload.agent_id, payload.tag);
                self.hub.tasks.insert(key, payload);
            }
            HubEvent::DiffSummary { payload, event_at } => {
                let entry = self
                    .hub
                    .agents
                    .entry(payload.agent_id.clone())
                    .or_insert(HubAgent {
                        status: None,
                        last_seen: event_at,
                        last_heartbeat: None,
                        last_activity: None,
                    });
                entry.last_seen = event_at;
                entry.last_activity = Some(event_at);
                self.hub.diffs.insert(payload.agent_id.clone(), payload);
            }
        }
    }

    fn set_local(&mut self, local: LocalSnapshot) {
        self.local = local;
    }

    fn prune_hub_cache(&mut self) {
        let now = Utc::now();
        let local_online: HashSet<String> = self
            .local
            .overview
            .iter()
            .filter(|row| row.online)
            .map(|row| row.identity_key.clone())
            .collect();
        self.hub.agents.retain(|agent_id, agent| {
            let age = now
                .signed_duration_since(agent.last_seen)
                .num_seconds()
                .max(0);
            if !local_online.is_empty()
                && !local_online.contains(agent_id)
                && age >= HUB_LOCAL_MISS_PRUNE_SECS
            {
                return false;
            }
            let reported_offline = agent
                .status
                .as_ref()
                .map(|status| status.status.eq_ignore_ascii_case("offline"))
                .unwrap_or(false);
            if reported_offline {
                age <= HUB_OFFLINE_GRACE_SECS
            } else {
                age <= HUB_PRUNE_SECS
            }
        });

        let active_agents: HashSet<String> = self.hub.agents.keys().cloned().collect();
        self.hub
            .diffs
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub.tasks.retain(|key, payload| {
            if active_agents.contains(&payload.agent_id) {
                return true;
            }
            key.rsplit_once("::")
                .map(|(agent_id, _)| active_agents.contains(agent_id))
                .unwrap_or(false)
        });
    }

    fn mode_source(&self) -> &'static str {
        match self.mode {
            Mode::Overview => {
                if self.connected && !self.hub.agents.is_empty() {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Work => {
                if self.connected && !self.hub.tasks.is_empty() {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Diff => {
                if self.connected && !self.hub.diffs.is_empty() {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Health => "local",
        }
    }

    fn overview_rows(&self) -> Vec<OverviewRow> {
        if self.connected && !self.hub.agents.is_empty() {
            let now = Utc::now();
            let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
            for (agent_id, agent) in &self.hub.agents {
                let status = agent.status.as_ref();
                let pane_id = status
                    .map(|s| s.pane_id.clone())
                    .unwrap_or_else(|| extract_pane_id(agent_id));
                let label = status
                    .and_then(|s| s.agent_label.clone())
                    .unwrap_or_else(|| extract_label(agent_id));
                let project_root = status
                    .map(|s| s.project_root.clone())
                    .unwrap_or_else(|| "(unknown)".to_string());
                let heartbeat_age_secs = agent
                    .last_heartbeat
                    .map(|dt| now.signed_duration_since(dt).num_seconds().max(0))
                    .or(Some(
                        now.signed_duration_since(agent.last_seen)
                            .num_seconds()
                            .max(0),
                    ));
                let age_secs = agent
                    .last_activity
                    .map(|dt| now.signed_duration_since(dt).num_seconds().max(0))
                    .or(heartbeat_age_secs);
                let reported = status
                    .map(|s| s.status.to_ascii_lowercase())
                    .unwrap_or_else(|| "running".to_string());
                let online = reported != "offline"
                    && heartbeat_age_secs.unwrap_or(HUB_STALE_SECS + 1) <= HUB_STALE_SECS;
                let row = OverviewRow {
                    identity_key: agent_id.clone(),
                    label,
                    pane_id,
                    tab_index: None,
                    tab_name: None,
                    tab_focused: false,
                    project_root,
                    online,
                    age_secs,
                    source: "hub".to_string(),
                };
                rows.insert(row.identity_key.clone(), row);
            }

            for local in &self.local.overview {
                if !local.online {
                    continue;
                }
                if let Some(existing) = rows.get_mut(&local.identity_key) {
                    if existing.project_root == "(unknown)" && local.project_root != "(unknown)" {
                        existing.project_root = local.project_root.clone();
                    }
                    if existing.label.starts_with("pane-") && !local.label.starts_with("pane-") {
                        existing.label = local.label.clone();
                    }
                    if existing.source == "hub" {
                        existing.source = format!("hub+{}", local.source);
                    }
                    if existing.tab_index.is_none() {
                        existing.tab_index = local.tab_index;
                    }
                    if existing.tab_name.is_none() {
                        existing.tab_name = local.tab_name.clone();
                    }
                    if local.tab_focused {
                        existing.tab_focused = true;
                    }
                } else {
                    let mut row = local.clone();
                    row.source = format!("local:{}", local.source);
                    rows.insert(row.identity_key.clone(), row);
                }
            }

            return rows.into_values().collect();
        }
        self.local.overview.clone()
    }

    fn work_rows(&self) -> Vec<WorkProject> {
        if self.connected && !self.hub.tasks.is_empty() {
            let mut grouped: BTreeMap<(String, String), BTreeMap<String, WorkTagRow>> =
                BTreeMap::new();
            for payload in self.hub.tasks.values() {
                let project_root = self
                    .hub
                    .agents
                    .get(&payload.agent_id)
                    .and_then(|agent| agent.status.as_ref().map(|s| s.project_root.clone()))
                    .unwrap_or_else(|| "(unknown)".to_string());
                let scope = self
                    .hub
                    .agents
                    .get(&payload.agent_id)
                    .and_then(|agent| agent.status.as_ref().and_then(|s| s.agent_label.clone()))
                    .unwrap_or_else(|| extract_label(&payload.agent_id));
                let in_progress_titles = payload
                    .active_tasks
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|task| task.status == "in-progress" || task.status == "in_progress")
                    .map(|task| format!("#{} {}", task.id, task.title))
                    .collect::<Vec<_>>();
                grouped.entry((project_root, scope)).or_default().insert(
                    payload.tag.clone(),
                    WorkTagRow {
                        tag: payload.tag.clone(),
                        counts: payload.counts.clone(),
                        in_progress_titles,
                    },
                );
            }
            let mut rows = Vec::new();
            for ((project_root, scope), tags) in grouped {
                rows.push(WorkProject {
                    project_root,
                    scope,
                    tags: tags.into_values().collect(),
                });
            }
            return rows;
        }
        self.local.work.clone()
    }

    fn diff_rows(&self) -> Vec<DiffProject> {
        if self.connected && !self.hub.diffs.is_empty() {
            let mut grouped: BTreeMap<String, (DiffProject, Vec<String>)> = BTreeMap::new();
            for payload in self.hub.diffs.values() {
                let scope = self
                    .hub
                    .agents
                    .get(&payload.agent_id)
                    .and_then(|agent| agent.status.as_ref().and_then(|s| s.agent_label.clone()))
                    .unwrap_or_else(|| extract_label(&payload.agent_id));
                let mut files = payload.files.clone();
                if files.len() > MAX_DIFF_FILES {
                    files.truncate(MAX_DIFF_FILES);
                }
                let key = payload.repo_root.clone();
                let entry = grouped.entry(key.clone()).or_insert_with(|| {
                    (
                        DiffProject {
                            project_root: key,
                            scope: String::new(),
                            git_available: payload.git_available,
                            reason: payload.reason.clone(),
                            summary: payload.summary.clone(),
                            files: files.clone(),
                        },
                        Vec::new(),
                    )
                });
                if !entry.1.iter().any(|value| value == &scope) {
                    entry.1.push(scope);
                }
                if entry.0.files.is_empty() && !files.is_empty() {
                    entry.0.files = files;
                }
                if !entry.0.git_available && payload.git_available {
                    entry.0.git_available = true;
                    entry.0.reason = payload.reason.clone();
                    entry.0.summary = payload.summary.clone();
                }
            }
            let mut rows = Vec::new();
            for (_, (mut row, scopes)) in grouped {
                row.scope = scope_summary(&scopes);
                rows.push(row);
            }
            return rows;
        }
        self.local.diff.clone()
    }

    fn selected_overview_index(&self, len: usize) -> usize {
        self.selected_overview.min(len.saturating_sub(1))
    }

    fn move_overview_selection(&mut self, step: i32) {
        let len = self.overview_rows().len();
        if len == 0 {
            self.selected_overview = 0;
            return;
        }
        let current = self.selected_overview_index(len) as i32;
        let max = len.saturating_sub(1) as i32;
        let next = (current + step).clamp(0, max) as usize;
        self.selected_overview = next;
    }

    fn focus_selected_overview_tab(&mut self) {
        if self.mode != Mode::Overview {
            return;
        }
        let rows = self.overview_rows();
        if rows.is_empty() {
            self.status_note = Some("no agents to focus".to_string());
            return;
        }
        let selected = self.selected_overview_index(rows.len());
        self.selected_overview = selected;
        let row = &rows[selected];
        let Some(tab_index) = row.tab_index else {
            self.status_note = Some(format!("no tab mapping for pane {}", row.pane_id));
            return;
        };
        match go_to_tab(&self.config.session_id, tab_index) {
            Ok(()) => {
                self.status_note = Some(format!(
                    "focused tab {} {}",
                    tab_index,
                    row.tab_name
                        .as_deref()
                        .map(|value| format!("({})", value))
                        .unwrap_or_default()
                ));
            }
            Err(err) => {
                self.status_note = Some(format!("focus failed: {err}"));
            }
        }
    }
}

#[derive(Deserialize)]
struct RuntimeSnapshot {
    session_id: String,
    pane_id: String,
    agent_id: String,
    agent_label: String,
    project_root: String,
    pid: i32,
    status: String,
    last_update: String,
}

#[derive(Clone, Debug)]
struct GitStatusEntry {
    path: String,
    status: String,
    staged: bool,
    unstaged: bool,
    untracked: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = load_config();
    init_logging();

    let initial_local = collect_local(&config);
    let mut app = App::new(config.clone(), initial_local);

    let (hub_tx, mut hub_rx) = mpsc::channel(256);
    let hub_cfg = config.clone();
    tokio::spawn(async move {
        hub_loop(hub_cfg, hub_tx).await;
    });

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(LOCAL_REFRESH_SECS));
    let mut refresh_requested = false;

    loop {
        if refresh_requested {
            app.set_local(collect_local(&app.config));
            refresh_requested = false;
        }

        app.prune_hub_cache();

        terminal.draw(|frame| render_ui(frame, &app))?;
        tokio::select! {
            _ = ticker.tick() => {
                refresh_requested = true;
            }
            Some(event) = hub_rx.recv() => {
                app.apply_hub_event(event);
            }
            maybe_event = events.next() => {
                if let Some(Ok(event)) = maybe_event {
                    if handle_input(event, &mut app, &mut refresh_requested) {
                        break;
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[derive(Clone, Copy)]
struct PulseTheme {
    bg: Color,
    surface: Color,
    border: Color,
    title: Color,
    text: Color,
    muted: Color,
    accent: Color,
    ok: Color,
    warn: Color,
    critical: Color,
    info: Color,
}

#[derive(Default)]
struct PulseKpis {
    total_agents: usize,
    online_agents: usize,
    in_progress: u32,
    blocked: u32,
    dirty_files: u32,
    churn: u32,
}

fn pulse_theme() -> PulseTheme {
    PulseTheme {
        bg: Color::Rgb(11, 18, 32),
        surface: Color::Rgb(17, 26, 46),
        border: Color::Rgb(71, 85, 105),
        title: Color::Rgb(191, 219, 254),
        text: Color::Rgb(226, 232, 240),
        muted: Color::Rgb(148, 163, 184),
        accent: Color::Rgb(56, 189, 248),
        ok: Color::Rgb(34, 197, 94),
        warn: Color::Rgb(245, 158, 11),
        critical: Color::Rgb(239, 68, 68),
        info: Color::Rgb(59, 130, 246),
    }
}

fn render_ui(frame: &mut ratatui::Frame, app: &App) {
    let size = frame.size();
    let theme = pulse_theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(size);
    frame.render_widget(render_header(app, theme, size.width), layout[0]);
    frame.render_widget(render_kpis(app, theme, size.width), layout[1]);
    if app.mode == Mode::Overview {
        render_overview_panel(frame, app, theme, layout[2], size.width);
    } else {
        frame.render_widget(render_body(app, theme, size.width), layout[2]);
    }
    if app.help_open {
        render_help_overlay(frame, app, theme);
    }
}

fn render_header(app: &App, theme: PulseTheme, width: u16) -> Paragraph<'static> {
    let compact = is_compact(width);
    let kpis = compute_kpis(app);
    let source = app.mode_source();
    let hub = if app.connected { "online" } else { "offline" };
    let session = ellipsize(&app.config.session_id, if compact { 14 } else { 28 });
    let inner_width = width.saturating_sub(4) as usize;
    let status_fields = vec![
        format!("mode={}", app.mode.title()),
        format!("hub={hub}"),
        format!("src={source}"),
        format!("agents={}/{}", kpis.online_agents, kpis.total_agents),
        format!("session={session}"),
    ];
    let status_line = fit_fields(&status_fields, inner_width.max(12));

    let controls_text = if let Some(note) = app.status_note.as_deref() {
        ellipsize(note, inner_width.max(12))
    } else if compact {
        "? help".to_string()
    } else {
        fit_fields(
            &[
                "1-4 mode".to_string(),
                "Tab next".to_string(),
                "j/k nav".to_string(),
                "Enter focus".to_string(),
                "r refresh".to_string(),
                "? help".to_string(),
                "q quit".to_string(),
            ],
            inner_width.max(12),
        )
    };

    Paragraph::new(Text::from(vec![
        Line::from(Span::styled(status_line, Style::default().fg(theme.text))),
        Line::from(Span::styled(
            controls_text,
            Style::default().fg(if app.status_note.is_some() {
                theme.info
            } else {
                theme.muted
            }),
        )),
    ]))
    .style(Style::default().fg(theme.text).bg(theme.bg))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg))
            .title(Span::styled(
                "Status",
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            )),
    )
}

fn render_kpis(app: &App, theme: PulseTheme, width: u16) -> Paragraph<'static> {
    let compact = is_compact(width);
    let kpis = compute_kpis(app);
    let inner_width = width.saturating_sub(4) as usize;
    let mut fields = vec![
        format!("online {}/{}", kpis.online_agents, kpis.total_agents),
        format!("in-progress {}", kpis.in_progress),
        format!("dirty {}", kpis.dirty_files),
        format!("blocked {}", kpis.blocked),
    ];
    if !compact {
        fields.push(format!("churn {}", kpis.churn));
    }
    let line = fit_fields(&fields, inner_width.max(12));

    Paragraph::new(Line::from(Span::styled(
        line,
        Style::default().fg(theme.text),
    )))
    .style(Style::default().fg(theme.text).bg(theme.surface))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.surface))
            .title(Span::styled(
                "Pulse",
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            )),
    )
}

fn render_body(app: &App, theme: PulseTheme, width: u16) -> Paragraph<'static> {
    let compact = is_compact(width);
    let lines = match app.mode {
        Mode::Overview => Vec::new(),
        Mode::Work => render_work_lines(app, theme, compact),
        Mode::Diff => render_diff_lines(app, theme, compact, width),
        Mode::Health => render_health_lines(app, theme, compact),
    };
    Paragraph::new(Text::from(lines))
        .style(Style::default().fg(theme.text).bg(theme.surface))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.surface))
                .title(Span::styled(
                    app.mode.title(),
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .scroll((app.scroll, 0))
}

fn render_overview_panel(
    frame: &mut ratatui::Frame,
    app: &App,
    theme: PulseTheme,
    area: Rect,
    width: u16,
) {
    let compact = is_compact(width);
    let rows = app.overview_rows();
    if rows.is_empty() {
        let paragraph = Paragraph::new(Line::from(Span::styled(
            "No active panes detected for this session.",
            Style::default().fg(theme.muted),
        )))
        .style(Style::default().fg(theme.text).bg(theme.surface))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.surface))
                .title(Span::styled(
                    "Overview",
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                )),
        );
        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| ListItem::new(Line::from(overview_row_spans(row, theme, compact, width))))
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.selected_overview_index(items.len())));
    let list = List::new(items)
        .highlight_symbol("-> ")
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.surface))
                .title(Span::styled(
                    "Overview",
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                )),
        );
    frame.render_stateful_widget(list, area, &mut state);
}

fn overview_row_spans(
    row: &OverviewRow,
    theme: PulseTheme,
    compact: bool,
    width: u16,
) -> Vec<Span<'static>> {
    let status_color = if row.online { theme.ok } else { theme.critical };
    let age_color = age_color(row.age_secs, row.online, theme);
    let pane = ellipsize(&row.pane_id, 8);
    let label = ellipsize(&row.label, if compact { 14 } else { 20 });
    let tab = row
        .tab_index
        .map(|index| format!("t{index}"))
        .unwrap_or_else(|| "t?".to_string());
    let tab_name = row
        .tab_name
        .as_deref()
        .map(|value| ellipsize(value, if compact { 8 } else { 12 }))
        .unwrap_or_else(|| "unknown".to_string());
    let source_color = if row.source == "hub" {
        theme.info
    } else {
        theme.warn
    };

    let base_cols = if compact { 56usize } else { 72usize };
    let root_budget = width.saturating_sub(base_cols as u16) as usize;
    let root = if compact || row.project_root == "(unknown)" {
        String::new()
    } else {
        format!(
            " root:{}",
            ellipsize(&short_project(&row.project_root, 20), root_budget.max(10))
        )
    };

    let mut spans = vec![
        Span::styled(
            if row.online { "*" } else { "!" },
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            label,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(format!("({pane})"), Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            if row.tab_focused {
                format!("[{tab}:{tab_name}*]")
            } else {
                format!("[{tab}:{tab_name}]")
            },
            Style::default().fg(if row.tab_focused {
                theme.accent
            } else {
                theme.muted
            }),
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "{} act:{}",
                age_meter(row.age_secs, row.online),
                format_age(row.age_secs)
            ),
            Style::default().fg(age_color),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", ellipsize(&row.source, 12)),
            Style::default().fg(source_color),
        ),
    ];
    if !root.is_empty() {
        spans.push(Span::styled(root, Style::default().fg(theme.muted)));
    }
    spans
}

fn render_help_overlay(frame: &mut ratatui::Frame, app: &App, theme: PulseTheme) {
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
            "Pulse Navigation",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  1/2/3/4  switch mode (Overview/Work/Diff/Health)"),
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
        Line::from("  q        quit pulse pane"),
        Line::from(""),
        Line::from(Span::styled(
            "Planned Agent Controls",
            Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
        )),
        Line::from("  stop/close controls in progress"),
        Line::from("  for now, use zellij tab actions directly"),
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

fn mode_help_lines(app: &App, theme: PulseTheme) -> Vec<Line<'static>> {
    match app.mode {
        Mode::Overview => vec![
            Line::from(Span::styled(
                "Overview Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      select agent row"),
            Line::from("  g        jump to first agent"),
            Line::from("  Enter    focus selected zellij tab"),
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

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100u16.saturating_sub(percent_y)) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100u16.saturating_sub(percent_y)) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100u16.saturating_sub(percent_x)) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100u16.saturating_sub(percent_x)) / 2),
        ])
        .split(vertical[1])[1]
}

fn render_work_lines(app: &App, theme: PulseTheme, compact: bool) -> Vec<Line<'static>> {
    let projects = app.work_rows();
    if projects.is_empty() {
        return vec![Line::from(Span::styled(
            "No task data available.",
            Style::default().fg(theme.muted),
        ))];
    }
    let mut lines = Vec::new();
    for project in projects {
        lines.push(Line::from(vec![
            Span::styled(
                format!("Project {}", short_project(&project.project_root, 28)),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", project.scope),
                Style::default().fg(theme.muted),
            ),
        ]));
        for tag in project.tags {
            let mut spans = vec![
                Span::raw("  "),
                Span::styled(
                    format!("{}", ellipsize(&tag.tag, 18)),
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ];
            spans.extend(task_bar_spans(
                &tag.counts,
                if compact { 12 } else { 18 },
                theme,
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("ip:{}", tag.counts.in_progress),
                Style::default().fg(if tag.counts.in_progress > 0 {
                    theme.info
                } else {
                    theme.muted
                }),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("blk:{}", tag.counts.blocked),
                Style::default().fg(if tag.counts.blocked > 0 {
                    theme.critical
                } else {
                    theme.muted
                }),
            ));
            lines.push(Line::from(spans));
            if let Some(item) = tag.in_progress_titles.first() {
                lines.push(Line::from(vec![
                    Span::raw("    -> "),
                    Span::styled(
                        ellipsize(item, if compact { 40 } else { 72 }),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
        }
        if !compact {
            lines.push(Line::from(""));
        }
    }
    lines
}

fn render_diff_lines(
    app: &App,
    theme: PulseTheme,
    compact: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let projects = app.diff_rows();
    if projects.is_empty() {
        return vec![Line::from(Span::styled(
            "No diff data available.",
            Style::default().fg(theme.muted),
        ))];
    }
    let mut lines = Vec::new();
    for project in projects {
        lines.push(Line::from(vec![
            Span::styled(
                format!("Repo {}", short_project(&project.project_root, 28)),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", project.scope),
                Style::default().fg(theme.muted),
            ),
        ]));
        if !project.git_available {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!(
                        "diff unavailable: {}",
                        project.reason.unwrap_or_else(|| "unknown".to_string())
                    ),
                    Style::default().fg(theme.critical),
                ),
            ]));
            if !compact {
                lines.push(Line::from(""));
            }
            continue;
        }
        let dirty = project.summary.staged.files
            + project.summary.unstaged.files
            + project.summary.untracked.files;
        let churn = project.summary.staged.additions
            + project.summary.staged.deletions
            + project.summary.unstaged.additions
            + project.summary.unstaged.deletions;
        let (risk_label, risk_color) = if churn > 200 || dirty > 24 {
            ("risk:high", theme.critical)
        } else if churn > 80 || dirty > 10 {
            ("risk:med", theme.warn)
        } else {
            ("risk:low", theme.ok)
        };
        let summary_line = fit_fields(
            &[
                format!("stg:{}", project.summary.staged.files),
                format!("uns:{}", project.summary.unstaged.files),
                format!("new:{}", project.summary.untracked.files),
                format!("churn:{}", churn),
            ],
            width.saturating_sub(18) as usize,
        );
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                risk_label,
                Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(summary_line, Style::default().fg(theme.muted)),
        ]));
        let file_limit = if compact { 4 } else { MAX_DIFF_FILES };
        let path_max = width.saturating_sub(if compact { 28 } else { 34 }) as usize;
        for file in project.files.iter().take(file_limit) {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{}", short_status(&file.status)),
                    Style::default()
                        .fg(diff_status_color(&file.status, theme))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("+{}", file.additions),
                    Style::default().fg(theme.ok),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("-{}", file.deletions),
                    Style::default().fg(theme.critical),
                ),
                Span::raw(" "),
                Span::styled(
                    ellipsize(&file.path, path_max.max(16)),
                    Style::default().fg(theme.text),
                ),
            ]));
        }
        if !compact {
            lines.push(Line::from(""));
        }
    }
    lines
}

fn render_health_lines(app: &App, theme: PulseTheme, compact: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            "Hub",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            if app.connected {
                "connected"
            } else {
                "offline (fallback active)"
            },
            Style::default().fg(if app.connected {
                theme.ok
            } else {
                theme.critical
            }),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Taskmaster ", Style::default().fg(theme.title)),
        Span::styled(
            ellipsize(
                &app.local.health.taskmaster_status,
                if compact { 38 } else { 80 },
            ),
            Style::default().fg(
                if app.local.health.taskmaster_status.contains("available") {
                    theme.ok
                } else {
                    theme.warn
                },
            ),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "Dependencies",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    for dep in &app.local.health.dependencies {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "*",
                Style::default().fg(if dep.available {
                    theme.ok
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(dep.name.clone(), Style::default().fg(theme.text)),
            Span::raw(" "),
            Span::styled(
                if dep.available { "ok" } else { "missing" },
                Style::default().fg(if dep.available {
                    theme.ok
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!(
                    "({})",
                    dep.path.clone().unwrap_or_else(|| "not found".to_string())
                ),
                Style::default().fg(theme.muted),
            ),
        ]));
    }
    lines.push(Line::from(Span::styled(
        "Checks",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    for check in &app.local.health.checks {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                check.name.clone(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                check.status.clone(),
                Style::default().fg(check_status_color(&check.status, theme)),
            ),
            Span::raw(" "),
            Span::styled(
                check.timestamp.clone().unwrap_or_else(|| "n/a".to_string()),
                Style::default().fg(theme.muted),
            ),
            Span::raw(" "),
            Span::styled(
                ellipsize(
                    check.details.as_deref().unwrap_or(""),
                    if compact { 24 } else { 64 },
                ),
                Style::default().fg(theme.muted),
            ),
        ]));
    }
    lines
}

fn compute_kpis(app: &App) -> PulseKpis {
    let overview = app.overview_rows();
    let work = app.work_rows();
    let diff = app.diff_rows();

    let total_agents = overview.len();
    let online_agents = overview.iter().filter(|row| row.online).count();

    let mut in_progress = 0;
    let mut blocked = 0;
    for project in work {
        for tag in project.tags {
            in_progress += tag.counts.in_progress;
            blocked += tag.counts.blocked;
        }
    }

    let mut dirty_files = 0;
    let mut churn = 0;
    for project in diff {
        if !project.git_available {
            continue;
        }
        dirty_files += project.summary.staged.files
            + project.summary.unstaged.files
            + project.summary.untracked.files;
        churn += project.summary.staged.additions
            + project.summary.staged.deletions
            + project.summary.unstaged.additions
            + project.summary.unstaged.deletions;
    }

    PulseKpis {
        total_agents,
        online_agents,
        in_progress,
        blocked,
        dirty_files,
        churn,
    }
}

fn task_bar_spans(counts: &TaskCounts, width: usize, theme: PulseTheme) -> Vec<Span<'static>> {
    let width = width.max(6);
    let total = counts.total.max(1) as usize;
    let done_w = (counts.done as usize * width) / total;
    let in_progress_w = (counts.in_progress as usize * width) / total;
    let mut blocked_w = (counts.blocked as usize * width) / total;
    if done_w + in_progress_w + blocked_w > width {
        blocked_w = blocked_w.saturating_sub((done_w + in_progress_w + blocked_w) - width);
    }
    let used = done_w + in_progress_w + blocked_w;
    let pending_w = width.saturating_sub(used);

    let mut spans = vec![Span::styled("[", Style::default().fg(theme.muted))];
    if done_w > 0 {
        spans.push(Span::styled(
            "#".repeat(done_w),
            Style::default().fg(theme.ok),
        ));
    }
    if in_progress_w > 0 {
        spans.push(Span::styled(
            "=".repeat(in_progress_w),
            Style::default().fg(theme.info),
        ));
    }
    if blocked_w > 0 {
        spans.push(Span::styled(
            "!".repeat(blocked_w),
            Style::default().fg(theme.critical),
        ));
    }
    if pending_w > 0 {
        spans.push(Span::styled(
            "-".repeat(pending_w),
            Style::default().fg(theme.muted),
        ));
    }
    spans.push(Span::styled("]", Style::default().fg(theme.muted)));
    spans
}

fn diff_status_color(status: &str, theme: PulseTheme) -> Color {
    match status {
        "added" => theme.ok,
        "deleted" => theme.critical,
        "renamed" => theme.accent,
        "untracked" => theme.info,
        _ => theme.warn,
    }
}

fn check_status_color(status: &str, theme: PulseTheme) -> Color {
    match status.to_ascii_lowercase().as_str() {
        "ok" | "pass" | "passed" | "success" | "done" => theme.ok,
        "fail" | "failed" | "error" => theme.critical,
        "unknown" => theme.muted,
        _ => theme.warn,
    }
}

fn format_age(age: Option<i64>) -> String {
    age.map(|value| format!("{value}s"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn age_meter(age: Option<i64>, online: bool) -> &'static str {
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

fn age_color(age: Option<i64>, online: bool, theme: PulseTheme) -> Color {
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

fn short_project(project_root: &str, max: usize) -> String {
    let leaf = Path::new(project_root)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(project_root);
    ellipsize(leaf, max)
}

fn scope_summary(scopes: &[String]) -> String {
    if scopes.is_empty() {
        return "local".to_string();
    }
    if scopes.len() == 1 {
        return scopes[0].clone();
    }
    format!("{}+{}", scopes[0], scopes.len() - 1)
}

fn ellipsize(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    if max <= 3 {
        return "...".chars().take(max).collect();
    }
    let prefix: String = input.chars().take(max - 3).collect();
    format!("{prefix}...")
}

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

fn is_compact(width: u16) -> bool {
    width < COMPACT_WIDTH
}

fn handle_input(event: Event, app: &mut App, refresh_requested: &mut bool) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            handle_key(key, app, refresh_requested)
        }
        _ => false,
    }
}

fn handle_key(key: KeyEvent, app: &mut App, refresh_requested: &mut bool) -> bool {
    if matches!(key.code, KeyCode::Char('?') | KeyCode::F(1)) {
        app.help_open = !app.help_open;
        return false;
    }
    if key.code == KeyCode::Esc && app.help_open {
        app.help_open = false;
        return false;
    }
    if app.help_open {
        return false;
    }

    match key.code {
        KeyCode::Char('q') => true,
        KeyCode::Char('1') => {
            app.mode = Mode::Overview;
            app.scroll = 0;
            false
        }
        KeyCode::Char('2') => {
            app.mode = Mode::Work;
            app.scroll = 0;
            false
        }
        KeyCode::Char('3') => {
            app.mode = Mode::Diff;
            app.scroll = 0;
            false
        }
        KeyCode::Char('4') => {
            app.mode = Mode::Health;
            app.scroll = 0;
            false
        }
        KeyCode::Tab => {
            app.mode = app.mode.next();
            app.scroll = 0;
            false
        }
        KeyCode::Enter => {
            app.focus_selected_overview_tab();
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.mode == Mode::Overview {
                app.move_overview_selection(1);
            } else {
                app.scroll = app.scroll.saturating_add(1);
            }
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.mode == Mode::Overview {
                app.move_overview_selection(-1);
            } else {
                app.scroll = app.scroll.saturating_sub(1);
            }
            false
        }
        KeyCode::Char('g') => {
            if app.mode == Mode::Overview {
                app.selected_overview = 0;
            }
            app.scroll = 0;
            false
        }
        KeyCode::Char('r') => {
            *refresh_requested = true;
            false
        }
        _ => false,
    }
}

fn go_to_tab(session_id: &str, tab_index: usize) -> Result<(), String> {
    if tab_index == 0 {
        return Err("invalid tab index".to_string());
    }
    let status = Command::new("zellij")
        .arg("--session")
        .arg(session_id)
        .arg("action")
        .arg("go-to-tab")
        .arg(tab_index.to_string())
        .status()
        .map_err(|_| "zellij not available".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("zellij exited with {}", status))
    }
}

async fn hub_loop(config: Config, tx: mpsc::Sender<HubEvent>) {
    let mut backoff = Duration::from_secs(1);
    loop {
        let connect = connect_async(config.hub_url.clone()).await;
        let (mut ws, _) = match connect {
            Ok(value) => value,
            Err(err) => {
                warn!("hub_connect_error: {err}");
                tokio::time::sleep(backoff).await;
                backoff = next_backoff(backoff);
                continue;
            }
        };
        backoff = Duration::from_secs(1);
        let hello = build_hello(&config);
        if ws
            .send(tokio_tungstenite::tungstenite::Message::Text(hello))
            .await
            .is_err()
        {
            let _ = ws.close(None).await;
            continue;
        }
        let _ = tx.send(HubEvent::Connected).await;
        loop {
            match ws.next().await {
                Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                    if let Some(event) = parse_hub_event(&config, &text) {
                        let _ = tx.send(event).await;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break,
            }
        }
        let _ = tx.send(HubEvent::Disconnected).await;
        let _ = ws.close(None).await;
    }
}

fn parse_hub_event(config: &Config, text: &str) -> Option<HubEvent> {
    let envelope: Envelope = serde_json::from_str(text).ok()?;
    if envelope.session_id != config.session_id {
        return None;
    }
    let event_at = DateTime::parse_from_rfc3339(&envelope.timestamp)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    match envelope.r#type.as_str() {
        "agent_status" => serde_json::from_value(envelope.payload)
            .ok()
            .map(|payload| HubEvent::AgentStatus { payload, event_at }),
        "heartbeat" => serde_json::from_value(envelope.payload)
            .ok()
            .map(|payload| HubEvent::Heartbeat { payload, event_at }),
        "task_summary" => serde_json::from_value(envelope.payload)
            .ok()
            .map(|payload| HubEvent::TaskSummary { payload, event_at }),
        "diff_summary" => serde_json::from_value(envelope.payload)
            .ok()
            .map(|payload| HubEvent::DiffSummary { payload, event_at }),
        _ => None,
    }
}

fn build_hello(config: &Config) -> String {
    let payload = HelloPayload {
        client_id: config.client_id.clone(),
        role: "subscriber".to_string(),
        capabilities: vec![
            "agent_status".to_string(),
            "heartbeat".to_string(),
            "task_summary".to_string(),
            "diff_summary".to_string(),
        ],
        agent_id: None,
    };
    let envelope = Envelope {
        version: PROTOCOL_VERSION.to_string(),
        r#type: "hello".to_string(),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        payload: serde_json::to_value(payload).unwrap_or(Value::Null),
        request_id: None,
    };
    serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string())
}

fn collect_local(config: &Config) -> LocalSnapshot {
    let mut overview = collect_runtime_overview(config);
    if overview.is_empty() {
        overview = collect_proc_overview(config);
    }
    let project_roots = collect_project_roots(&overview, &config.project_root);
    let (work, taskmaster_status) = collect_local_work(&project_roots);
    let diff = collect_local_diff(&project_roots);
    let health = collect_health(config, &taskmaster_status);
    LocalSnapshot {
        overview,
        work,
        diff,
        health,
    }
}

fn collect_runtime_overview(config: &Config) -> Vec<OverviewRow> {
    let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
    let telemetry_dir = config
        .state_dir
        .join("telemetry")
        .join(sanitize_component(&config.session_id));
    let entries = match fs::read_dir(telemetry_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let now = Utc::now();
    let session_layout = collect_session_layout(&config.session_id);
    let active_panes = session_layout.as_ref().map(|layout| &layout.pane_ids);
    let pane_tabs = session_layout.as_ref().map(|layout| &layout.pane_tabs);
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let snapshot: RuntimeSnapshot = match serde_json::from_str(&contents) {
            Ok(snapshot) => snapshot,
            Err(_) => continue,
        };
        if snapshot.session_id != config.session_id {
            continue;
        }
        if let Some(panes) = active_panes.as_ref() {
            if !panes.contains(&snapshot.pane_id) {
                continue;
            }
        }
        let heartbeat_age = DateTime::parse_from_rfc3339(&snapshot.last_update)
            .ok()
            .map(|dt| {
                now.signed_duration_since(dt.with_timezone(&Utc))
                    .num_seconds()
                    .max(0)
            });
        let online =
            runtime_process_matches(&snapshot) && !snapshot.status.eq_ignore_ascii_case("offline");
        let expected_identity = build_identity_key(&snapshot.session_id, &snapshot.pane_id);
        let identity_key = if snapshot.agent_id == expected_identity {
            snapshot.agent_id.clone()
        } else {
            expected_identity
        };
        let tab_meta = pane_tabs.and_then(|tabs| tabs.get(&snapshot.pane_id));
        rows.insert(
            identity_key.clone(),
            OverviewRow {
                identity_key,
                label: snapshot.agent_label,
                pane_id: snapshot.pane_id,
                tab_index: tab_meta.map(|meta| meta.index),
                tab_name: tab_meta.map(|meta| meta.name.clone()),
                tab_focused: tab_meta.map(|meta| meta.focused).unwrap_or(false),
                project_root: snapshot.project_root,
                online,
                age_secs: heartbeat_age,
                source: "runtime".to_string(),
            },
        );
    }
    rows.into_values().collect()
}

fn collect_session_layout(session_id: &str) -> Option<SessionLayout> {
    if session_id.trim().is_empty() {
        return None;
    }
    let output = Command::new("zellij")
        .arg("--session")
        .arg(session_id)
        .arg("action")
        .arg("dump-layout")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let layout = String::from_utf8_lossy(&output.stdout);
    let parsed = parse_layout_tabs(&layout);
    if parsed.pane_ids.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

fn parse_layout_tabs(layout: &str) -> SessionLayout {
    let mut parsed = SessionLayout::default();
    let mut current_tab_index = 0usize;
    let mut current_tab_name = String::new();
    let mut current_tab_focused = false;

    for line in layout.lines() {
        if line_is_tab_decl(line) {
            current_tab_index += 1;
            current_tab_name = extract_layout_attr(line, "name")
                .unwrap_or_else(|| format!("tab-{current_tab_index}"));
            current_tab_focused = line.contains("focus=true") || line.contains("focus true");
        }

        for pane_id in extract_pane_ids_from_layout_line(line) {
            parsed.pane_ids.insert(pane_id.clone());
            if current_tab_index > 0 {
                parsed.pane_tabs.insert(
                    pane_id,
                    TabMeta {
                        index: current_tab_index,
                        name: current_tab_name.clone(),
                        focused: current_tab_focused,
                    },
                );
            }
        }
    }
    parsed
}

fn line_is_tab_decl(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("tab ") || trimmed == "tab" || trimmed.starts_with("tab\t")
}

fn extract_layout_attr(line: &str, attr: &str) -> Option<String> {
    let with_equals = format!("{attr}=\"");
    if let Some(start) = line.find(&with_equals) {
        let value_start = start + with_equals.len();
        let tail = &line[value_start..];
        let end = tail.find('"')?;
        let value = tail[..end].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    let with_space = format!("{attr} \"");
    if let Some(start) = line.find(&with_space) {
        let value_start = start + with_space.len();
        let tail = &line[value_start..];
        let end = tail.find('"')?;
        let value = tail[..end].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn extract_pane_ids_from_layout_line(line: &str) -> Vec<String> {
    let mut pane_ids = Vec::new();
    let mut cursor = line;
    while let Some(idx) = cursor.find("--pane-id\"") {
        let tail = &cursor[idx + "--pane-id\"".len()..];
        let Some(start_quote) = tail.find('"') else {
            break;
        };
        let value_tail = &tail[start_quote + 1..];
        let Some(end_quote) = value_tail.find('"') else {
            break;
        };
        let pane_id = value_tail[..end_quote].trim();
        if !pane_id.is_empty() {
            pane_ids.push(pane_id.to_string());
        }
        cursor = &value_tail[end_quote + 1..];
    }
    pane_ids
}

fn runtime_process_matches(snapshot: &RuntimeSnapshot) -> bool {
    if snapshot.pid <= 0 {
        return false;
    }
    let proc_path = PathBuf::from("/proc").join(snapshot.pid.to_string());
    if !proc_path.exists() {
        return false;
    }
    let env_map = read_proc_environ(proc_path.join("environ"));
    if env_map.is_empty() {
        return false;
    }
    let proc_session = env_map
        .get("AOC_SESSION_ID")
        .or_else(|| env_map.get("ZELLIJ_SESSION_NAME"))
        .map(|value| value.as_str())
        .unwrap_or("");
    let proc_pane = env_map
        .get("AOC_PANE_ID")
        .or_else(|| env_map.get("ZELLIJ_PANE_ID"))
        .map(|value| value.as_str())
        .unwrap_or("");
    proc_session == snapshot.session_id.as_str() && proc_pane == snapshot.pane_id.as_str()
}

fn collect_proc_overview(config: &Config) -> Vec<OverviewRow> {
    let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
    let session_layout = collect_session_layout(&config.session_id);
    let pane_tabs = session_layout.as_ref().map(|layout| &layout.pane_tabs);
    let proc_entries = match fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    for entry in proc_entries.flatten() {
        let file_name = entry.file_name();
        let pid_str = file_name.to_string_lossy();
        if pid_str.parse::<i32>().is_err() {
            continue;
        }
        let env_map = read_proc_environ(entry.path().join("environ"));
        if env_map.is_empty() {
            continue;
        }
        let session_id = env_map
            .get("AOC_SESSION_ID")
            .or_else(|| env_map.get("ZELLIJ_SESSION_NAME"))
            .cloned();
        if session_id.as_deref() != Some(config.session_id.as_str()) {
            continue;
        }
        let pane_id = env_map
            .get("AOC_PANE_ID")
            .or_else(|| env_map.get("ZELLIJ_PANE_ID"))
            .cloned();
        let pane_id = match pane_id {
            Some(value) if !value.is_empty() => value,
            _ => continue,
        };
        let label = env_map
            .get("AOC_AGENT_LABEL")
            .or_else(|| env_map.get("AOC_AGENT_ID"))
            .cloned()
            .unwrap_or_else(|| format!("pane-{}", pane_id));
        let project_root = env_map.get("AOC_PROJECT_ROOT").cloned().unwrap_or_else(|| {
            fs::read_link(entry.path().join("cwd"))
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "(unknown)".to_string())
        });
        let key = build_identity_key(&config.session_id, &pane_id);
        let tab_meta = pane_tabs.and_then(|tabs| tabs.get(&pane_id));
        rows.entry(key.clone()).or_insert(OverviewRow {
            identity_key: key,
            label,
            pane_id,
            tab_index: tab_meta.map(|meta| meta.index),
            tab_name: tab_meta.map(|meta| meta.name.clone()),
            tab_focused: tab_meta.map(|meta| meta.focused).unwrap_or(false),
            project_root,
            online: true,
            age_secs: None,
            source: "proc".to_string(),
        });
    }
    rows.into_values().collect()
}

fn collect_project_roots(overview: &[OverviewRow], fallback: &Path) -> Vec<String> {
    let mut roots = BTreeMap::new();
    for row in overview {
        if row.project_root.is_empty() || row.project_root == "(unknown)" {
            continue;
        }
        roots.insert(row.project_root.clone(), true);
    }
    roots.insert(fallback.to_string_lossy().to_string(), true);
    roots.into_keys().collect()
}

fn collect_local_work(project_roots: &[String]) -> (Vec<WorkProject>, String) {
    let mut projects = Vec::new();
    let mut status = "tasks.json missing".to_string();
    for root in project_roots {
        let tasks_path = PathBuf::from(root)
            .join(".taskmaster")
            .join("tasks")
            .join("tasks.json");
        let contents = match fs::read_to_string(&tasks_path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let parsed: ProjectData = match serde_json::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(_) => {
                status = format!("tasks.json malformed at {}", tasks_path.display());
                continue;
            }
        };
        status = "tasks.json available".to_string();
        let mut tags = Vec::new();
        let mut entries: Vec<_> = parsed.tags.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (tag, ctx) in entries {
            let mut counts = TaskCounts {
                total: ctx.tasks.len() as u32,
                ..TaskCounts::default()
            };
            let mut in_progress_titles = Vec::new();
            for task in ctx.tasks {
                match task.status {
                    TaskStatus::Pending => counts.pending += 1,
                    TaskStatus::InProgress => {
                        counts.in_progress += 1;
                        in_progress_titles.push(format!("#{} {}", task.id, task.title));
                    }
                    TaskStatus::Blocked => counts.blocked += 1,
                    TaskStatus::Done | TaskStatus::Cancelled => counts.done += 1,
                    _ => {}
                }
            }
            tags.push(WorkTagRow {
                tag,
                counts,
                in_progress_titles,
            });
        }
        projects.push(WorkProject {
            project_root: root.clone(),
            scope: "local".to_string(),
            tags,
        });
    }
    (projects, status)
}

fn collect_local_diff(project_roots: &[String]) -> Vec<DiffProject> {
    let mut projects: BTreeMap<String, DiffProject> = BTreeMap::new();
    for root in project_roots {
        let root_path = PathBuf::from(root);
        match git_repo_root(&root_path) {
            Ok(repo_root) => match collect_git_summary(&repo_root) {
                Ok((summary, mut files)) => {
                    if files.len() > MAX_DIFF_FILES {
                        files.truncate(MAX_DIFF_FILES);
                    }
                    let key = repo_root.to_string_lossy().to_string();
                    projects.entry(key.clone()).or_insert(DiffProject {
                        project_root: key,
                        scope: "local".to_string(),
                        git_available: true,
                        reason: None,
                        summary,
                        files,
                    });
                }
                Err(err) => {
                    projects.entry(root.clone()).or_insert(DiffProject {
                        project_root: root.clone(),
                        scope: "local".to_string(),
                        git_available: false,
                        reason: Some(err),
                        summary: DiffSummaryCounts::default(),
                        files: Vec::new(),
                    });
                }
            },
            Err(reason) => {
                projects.entry(root.clone()).or_insert(DiffProject {
                    project_root: root.clone(),
                    scope: "local".to_string(),
                    git_available: false,
                    reason: Some(reason),
                    summary: DiffSummaryCounts::default(),
                    files: Vec::new(),
                });
            }
        }
    }
    projects.into_values().collect()
}

fn collect_health(config: &Config, taskmaster_status: &str) -> HealthSnapshot {
    let dependencies = vec![
        dep_status("git"),
        dep_status("zellij"),
        dep_status("aoc-hub"),
        dep_status("aoc-agent-wrap-rs"),
        dep_status("aoc-taskmaster"),
    ];
    let checks = vec![
        load_check_outcome(&config.project_root, "test"),
        load_check_outcome(&config.project_root, "lint"),
        load_check_outcome(&config.project_root, "build"),
    ];
    HealthSnapshot {
        dependencies,
        checks,
        taskmaster_status: taskmaster_status.to_string(),
    }
}

fn dep_status(name: &str) -> DependencyStatus {
    let path = which_cmd(name);
    DependencyStatus {
        name: name.to_string(),
        available: path.is_some(),
        path,
    }
}

fn load_check_outcome(project_root: &Path, kind: &str) -> CheckOutcome {
    let base = project_root.join(".aoc").join("state");
    let json_path = base.join(format!("last-{kind}.json"));
    if let Ok(contents) = fs::read_to_string(&json_path) {
        if let Ok(value) = serde_json::from_str::<Value>(&contents) {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let timestamp = value
                .get("timestamp")
                .and_then(Value::as_str)
                .map(|v| v.to_string());
            let details = value
                .get("summary")
                .and_then(Value::as_str)
                .map(|v| v.to_string());
            return CheckOutcome {
                name: kind.to_string(),
                status,
                timestamp,
                details,
            };
        }
    }
    let text_path = base.join(format!("last-{kind}"));
    if let Ok(contents) = fs::read_to_string(&text_path) {
        let line = contents
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();
        return CheckOutcome {
            name: kind.to_string(),
            status: line,
            timestamp: None,
            details: Some("from .aoc/state marker".to_string()),
        };
    }
    CheckOutcome {
        name: kind.to_string(),
        status: "unknown".to_string(),
        timestamp: None,
        details: Some("no check marker found".to_string()),
    }
}

fn git_repo_root(project_root: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output();
    let output = match output {
        Ok(output) => output,
        Err(_) => return Err("git_missing".to_string()),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("not a git repository") {
            return Err("not_git_repo".to_string());
        }
        return Err("git_error".to_string());
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err("git_error".to_string());
    }
    Ok(PathBuf::from(root))
}

fn collect_git_summary(repo_root: &Path) -> Result<(DiffSummaryCounts, Vec<DiffFile>), String> {
    let staged_raw = run_git(repo_root, &["diff", "--numstat", "--cached"])?;
    let (staged_counts, staged_map) = parse_numstat(&staged_raw);
    let unstaged_raw = run_git(repo_root, &["diff", "--numstat"])?;
    let (unstaged_counts, unstaged_map) = parse_numstat(&unstaged_raw);
    let status_raw = run_git(repo_root, &["status", "--porcelain=v1", "-u"])?;
    let status_entries = parse_status_entries(&status_raw);

    let mut files = Vec::new();
    for entry in status_entries {
        let (additions, deletions) = if entry.untracked {
            (0, 0)
        } else {
            let staged_stats = staged_map.get(&entry.path).copied().unwrap_or((0, 0));
            let unstaged_stats = unstaged_map.get(&entry.path).copied().unwrap_or((0, 0));
            if entry.staged && entry.unstaged {
                (
                    staged_stats.0 + unstaged_stats.0,
                    staged_stats.1 + unstaged_stats.1,
                )
            } else if entry.staged {
                staged_stats
            } else {
                unstaged_stats
            }
        };
        files.push(DiffFile {
            path: entry.path,
            status: entry.status,
            additions,
            deletions,
            staged: entry.staged,
            untracked: entry.untracked,
        });
    }
    let untracked = files.iter().filter(|file| file.untracked).count() as u32;
    let summary = DiffSummaryCounts {
        staged: staged_counts,
        unstaged: unstaged_counts,
        untracked: UntrackedCounts { files: untracked },
    };
    Ok((summary, files))
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|_| "git_missing".to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_numstat(output: &str) -> (DiffCounts, HashMap<String, (u32, u32)>) {
    let mut counts = DiffCounts::default();
    let mut map = HashMap::new();
    for line in output.lines() {
        let mut parts = line.splitn(3, '\t');
        let additions = parts.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let deletions = parts.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let path = parts.next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        counts.files += 1;
        counts.additions += additions;
        counts.deletions += deletions;
        map.insert(path.to_string(), (additions, deletions));
    }
    (counts, map)
}

fn parse_status_entries(output: &str) -> Vec<GitStatusEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        if let Some(entry) = parse_status_line(line) {
            entries.push(entry);
        }
    }
    entries
}

fn parse_status_line(line: &str) -> Option<GitStatusEntry> {
    if line.len() < 3 {
        return None;
    }
    if line.starts_with("?? ") {
        return Some(GitStatusEntry {
            path: line[3..].trim().to_string(),
            status: "untracked".to_string(),
            staged: false,
            unstaged: false,
            untracked: true,
        });
    }
    let mut chars = line.chars();
    let x = chars.next()?;
    let y = chars.next()?;
    let mut path = line[3..].trim().to_string();
    if let Some((_, new_path)) = path.split_once("->") {
        path = new_path.trim().to_string();
    }
    let staged = x != ' ' && x != '?';
    let unstaged = y != ' ' && y != '?';
    let status = if matches!(x, 'A' | 'C') || matches!(y, 'A' | 'C') {
        "added"
    } else if x == 'D' || y == 'D' {
        "deleted"
    } else if x == 'R' || y == 'R' {
        "renamed"
    } else {
        "modified"
    };
    Some(GitStatusEntry {
        path,
        status: status.to_string(),
        staged,
        unstaged,
        untracked: false,
    })
}

fn read_proc_environ(path: PathBuf) -> HashMap<String, String> {
    let mut env_map = HashMap::new();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return env_map,
    };
    for part in bytes.split(|byte| *byte == 0) {
        if part.is_empty() {
            continue;
        }
        if let Ok(item) = String::from_utf8(part.to_vec()) {
            if let Some((key, value)) = item.split_once('=') {
                env_map.insert(key.to_string(), value.to_string());
            }
        }
    }
    env_map
}

fn short_status(status: &str) -> &'static str {
    match status {
        "added" => "A",
        "deleted" => "D",
        "renamed" => "R",
        "untracked" => "?",
        _ => "M",
    }
}

fn extract_label(identity_key: &str) -> String {
    if let Some((_, pane)) = identity_key.split_once("::") {
        return format!("pane-{pane}");
    }
    identity_key.to_string()
}

fn extract_pane_id(identity_key: &str) -> String {
    identity_key
        .split_once("::")
        .map(|(_, pane)| pane.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn build_identity_key(session_id: &str, pane_id: &str) -> String {
    format!("{session_id}::{pane_id}")
}

fn which_cmd(name: &str) -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    for part in path_var.split(':') {
        if part.is_empty() {
            continue;
        }
        let candidate = Path::new(part).join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn load_config() -> Config {
    let session_id = resolve_session_id();
    let hub_url = resolve_hub_url(&session_id);
    let client_id = format!("aoc-pulse-{}", std::process::id());
    let project_root = resolve_project_root();
    let state_dir = resolve_state_dir();
    Config {
        session_id,
        hub_url,
        client_id,
        project_root,
        state_dir,
    }
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stdout_enabled = matches!(
        std::env::var("AOC_LOG_STDOUT").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    );
    if stdout_enabled {
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(io::sink)
            .try_init();
    }
}

fn resolve_session_id() -> String {
    if let Ok(value) = std::env::var("AOC_SESSION_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = std::env::var("ZELLIJ_SESSION_NAME") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    format!("pid-{}", std::process::id())
}

fn resolve_hub_url(session_id: &str) -> Url {
    if let Ok(value) = std::env::var("AOC_HUB_URL") {
        if !value.trim().is_empty() {
            return Url::parse(&value).expect("invalid hub url");
        }
    }
    let addr = if let Ok(value) = std::env::var("AOC_HUB_ADDR") {
        if !value.trim().is_empty() {
            value
        } else {
            default_hub_addr(session_id)
        }
    } else {
        default_hub_addr(session_id)
    };
    Url::parse(&format!("ws://{addr}/ws")).expect("invalid hub addr")
}

fn resolve_project_root() -> PathBuf {
    if let Ok(value) = std::env::var("AOC_PROJECT_ROOT") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn resolve_state_dir() -> PathBuf {
    if let Ok(value) = std::env::var("XDG_STATE_HOME") {
        if !value.trim().is_empty() {
            return PathBuf::from(value).join("aoc");
        }
    }
    if let Ok(value) = std::env::var("HOME") {
        return PathBuf::from(value)
            .join(".local")
            .join("state")
            .join("aoc");
    }
    PathBuf::from(".aoc/state")
}

fn default_hub_addr(session_id: &str) -> String {
    format!("127.0.0.1:{}", derive_port(session_id))
}

fn derive_port(session_id: &str) -> u16 {
    let mut hash: u32 = 2166136261;
    for byte in session_id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    42000 + (hash % 2000) as u16
}

fn next_backoff(current: Duration) -> Duration {
    let next = current + current;
    if next > Duration::from_secs(10) {
        Duration::from_secs(10)
    } else {
        next
    }
}

fn sanitize_component(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
