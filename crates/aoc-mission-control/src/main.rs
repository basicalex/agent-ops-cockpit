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
    layout::{Constraint, Direction, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
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
const LOCAL_REFRESH_SECS: u64 = 3;
const HUB_STALE_SECS: i64 = 45;
const MAX_DIFF_FILES: usize = 8;

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
    project_root: String,
    online: bool,
    age_secs: Option<i64>,
    source: String,
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
    AgentStatus(AgentStatusPayload),
    Heartbeat(HeartbeatPayload),
    TaskSummary(TaskSummaryPayload),
    DiffSummary(DiffSummaryPayload),
}

struct App {
    config: Config,
    connected: bool,
    hub: HubCache,
    local: LocalSnapshot,
    mode: Mode,
    scroll: u16,
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
        }
    }

    fn apply_hub_event(&mut self, event: HubEvent) {
        match event {
            HubEvent::Connected => self.connected = true,
            HubEvent::Disconnected => self.connected = false,
            HubEvent::AgentStatus(payload) => {
                let key = payload.agent_id.clone();
                let entry = self.hub.agents.entry(key).or_insert(HubAgent {
                    status: None,
                    last_seen: Utc::now(),
                    last_heartbeat: None,
                });
                entry.status = Some(payload);
                entry.last_seen = Utc::now();
            }
            HubEvent::Heartbeat(payload) => {
                let key = payload.agent_id.clone();
                let entry = self.hub.agents.entry(key).or_insert(HubAgent {
                    status: None,
                    last_seen: Utc::now(),
                    last_heartbeat: None,
                });
                entry.last_seen = Utc::now();
                entry.last_heartbeat = DateTime::parse_from_rfc3339(&payload.last_update)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
                    .or(Some(Utc::now()));
            }
            HubEvent::TaskSummary(payload) => {
                let key = format!("{}::{}", payload.agent_id, payload.tag);
                self.hub.tasks.insert(key, payload);
            }
            HubEvent::DiffSummary(payload) => {
                self.hub.diffs.insert(payload.agent_id.clone(), payload);
            }
        }
    }

    fn set_local(&mut self, local: LocalSnapshot) {
        self.local = local;
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
            let mut rows = Vec::new();
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
                let age_secs = agent
                    .last_heartbeat
                    .map(|dt| now.signed_duration_since(dt).num_seconds().max(0))
                    .or(Some(
                        now.signed_duration_since(agent.last_seen)
                            .num_seconds()
                            .max(0),
                    ));
                let reported = status
                    .map(|s| s.status.to_ascii_lowercase())
                    .unwrap_or_else(|| "running".to_string());
                let online = reported != "offline"
                    && age_secs.unwrap_or(HUB_STALE_SECS + 1) <= HUB_STALE_SECS;
                rows.push(OverviewRow {
                    identity_key: agent_id.clone(),
                    label,
                    pane_id,
                    project_root,
                    online,
                    age_secs,
                    source: "hub".to_string(),
                });
            }
            rows.sort_by(|a, b| a.identity_key.cmp(&b.identity_key));
            return rows;
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
            let mut rows = Vec::new();
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
                rows.push(DiffProject {
                    project_root: payload.repo_root.clone(),
                    scope,
                    git_available: payload.git_available,
                    reason: payload.reason.clone(),
                    summary: payload.summary.clone(),
                    files,
                });
            }
            rows.sort_by(|a, b| a.project_root.cmp(&b.project_root));
            return rows;
        }
        self.local.diff.clone()
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

fn render_ui(frame: &mut ratatui::Frame, app: &App) {
    let size = frame.size();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(size);
    frame.render_widget(render_header(app), layout[0]);
    frame.render_widget(render_body(app), layout[1]);
}

fn render_header(app: &App) -> Paragraph<'static> {
    let hub = if app.connected {
        "connected"
    } else {
        "offline"
    };
    let source = app.mode_source();
    let line = Line::from(format!(
		"AOC Pulse | mode={} | source={} | hub={} | session={} | 1-4 switch  Tab cycle  j/k scroll  r refresh  q quit",
		app.mode.title(),
		source,
		hub,
		app.config.session_id
	));
    Paragraph::new(line).block(Block::default().borders(Borders::ALL).title("Status"))
}

fn render_body(app: &App) -> Paragraph<'static> {
    let lines = match app.mode {
        Mode::Overview => render_overview_lines(app),
        Mode::Work => render_work_lines(app),
        Mode::Diff => render_diff_lines(app),
        Mode::Health => render_health_lines(app),
    };
    Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{}", app.mode.title())),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0))
}

fn render_overview_lines(app: &App) -> Vec<Line<'static>> {
    let rows = app.overview_rows();
    if rows.is_empty() {
        return vec![Line::from("No active panes detected for this session.")];
    }
    let mut lines = Vec::new();
    lines.push(Line::from("Agents/panes in current session:"));
    for row in rows {
        let status = if row.online { "online" } else { "offline" };
        let age = row
            .age_secs
            .map(|v| format!("{}s", v))
            .unwrap_or_else(|| "n/a".to_string());
        lines.push(Line::from(format!(
            "- [{}] label={} pane={} age={} src={} key={} root={}",
            status, row.label, row.pane_id, age, row.source, row.identity_key, row.project_root
        )));
    }
    lines
}

fn render_work_lines(app: &App) -> Vec<Line<'static>> {
    let projects = app.work_rows();
    if projects.is_empty() {
        return vec![Line::from("No task data available.")];
    }
    let mut lines = Vec::new();
    for project in projects {
        lines.push(Line::from(format!(
            "Project: {}  Scope: {}",
            project.project_root, project.scope
        )));
        for tag in project.tags {
            lines.push(Line::from(format!(
                "  - tag={} total={} pending={} in_progress={} done={} blocked={}",
                tag.tag,
                tag.counts.total,
                tag.counts.pending,
                tag.counts.in_progress,
                tag.counts.done,
                tag.counts.blocked
            )));
            if !tag.in_progress_titles.is_empty() {
                for item in tag.in_progress_titles.iter().take(3) {
                    lines.push(Line::from(format!("      * {}", item)));
                }
            }
        }
        lines.push(Line::from(""));
    }
    lines
}

fn render_diff_lines(app: &App) -> Vec<Line<'static>> {
    let projects = app.diff_rows();
    if projects.is_empty() {
        return vec![Line::from("No diff data available.")];
    }
    let mut lines = Vec::new();
    for project in projects {
        lines.push(Line::from(format!(
            "Project: {}  Scope: {}",
            project.project_root, project.scope
        )));
        if !project.git_available {
            lines.push(Line::from(format!(
                "  - diff unavailable: {}",
                project.reason.unwrap_or_else(|| "unknown".to_string())
            )));
            lines.push(Line::from(""));
            continue;
        }
        lines.push(Line::from(format!(
            "  - staged: files={} +{} -{}",
            project.summary.staged.files,
            project.summary.staged.additions,
            project.summary.staged.deletions
        )));
        lines.push(Line::from(format!(
            "  - unstaged: files={} +{} -{}",
            project.summary.unstaged.files,
            project.summary.unstaged.additions,
            project.summary.unstaged.deletions
        )));
        lines.push(Line::from(format!(
            "  - untracked: files={}",
            project.summary.untracked.files
        )));
        for file in project.files.iter().take(MAX_DIFF_FILES) {
            lines.push(Line::from(format!(
                "      * {} +{} -{} {}",
                short_status(&file.status),
                file.additions,
                file.deletions,
                file.path
            )));
        }
        lines.push(Line::from(""));
    }
    lines
}

fn render_health_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(format!(
        "Hub connection: {}",
        if app.connected {
            "available"
        } else {
            "unreachable (local fallback active)"
        }
    )));
    lines.push(Line::from(format!(
        "Taskmaster data: {}",
        app.local.health.taskmaster_status
    )));
    lines.push(Line::from("Dependencies:"));
    for dep in &app.local.health.dependencies {
        let status = if dep.available { "ok" } else { "missing" };
        let detail = dep.path.clone().unwrap_or_else(|| "not found".to_string());
        lines.push(Line::from(format!(
            "  - {}: {} ({})",
            dep.name, status, detail
        )));
    }
    lines.push(Line::from("Checks (if available):"));
    for check in &app.local.health.checks {
        let ts = check.timestamp.clone().unwrap_or_else(|| "n/a".to_string());
        let details = check.details.clone().unwrap_or_else(|| "".to_string());
        lines.push(Line::from(format!(
            "  - {}: {} at {} {}",
            check.name, check.status, ts, details
        )));
    }
    lines
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
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll = app.scroll.saturating_add(1);
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll = app.scroll.saturating_sub(1);
            false
        }
        KeyCode::Char('g') => {
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
    match envelope.r#type.as_str() {
        "agent_status" => serde_json::from_value(envelope.payload)
            .ok()
            .map(HubEvent::AgentStatus),
        "heartbeat" => serde_json::from_value(envelope.payload)
            .ok()
            .map(HubEvent::Heartbeat),
        "task_summary" => serde_json::from_value(envelope.payload)
            .ok()
            .map(HubEvent::TaskSummary),
        "diff_summary" => serde_json::from_value(envelope.payload)
            .ok()
            .map(HubEvent::DiffSummary),
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
        let heartbeat_age = DateTime::parse_from_rfc3339(&snapshot.last_update)
            .ok()
            .map(|dt| {
                now.signed_duration_since(dt.with_timezone(&Utc))
                    .num_seconds()
                    .max(0)
            });
        let alive = process_exists(snapshot.pid);
        let online = alive && snapshot.status != "offline";
        rows.insert(
            snapshot.agent_id.clone(),
            OverviewRow {
                identity_key: snapshot.agent_id,
                label: snapshot.agent_label,
                pane_id: snapshot.pane_id,
                project_root: snapshot.project_root,
                online,
                age_secs: heartbeat_age,
                source: "runtime".to_string(),
            },
        );
    }
    rows.into_values().collect()
}

fn collect_proc_overview(config: &Config) -> Vec<OverviewRow> {
    let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
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
        rows.entry(key.clone()).or_insert(OverviewRow {
            identity_key: key,
            label,
            pane_id,
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
    let mut projects = Vec::new();
    for root in project_roots {
        let root_path = PathBuf::from(root);
        match git_repo_root(&root_path) {
            Ok(repo_root) => match collect_git_summary(&repo_root) {
                Ok((summary, mut files)) => {
                    if files.len() > MAX_DIFF_FILES {
                        files.truncate(MAX_DIFF_FILES);
                    }
                    projects.push(DiffProject {
                        project_root: repo_root.to_string_lossy().to_string(),
                        scope: "local".to_string(),
                        git_available: true,
                        reason: None,
                        summary,
                        files,
                    });
                }
                Err(err) => projects.push(DiffProject {
                    project_root: root.clone(),
                    scope: "local".to_string(),
                    git_available: false,
                    reason: Some(err),
                    summary: DiffSummaryCounts::default(),
                    files: Vec::new(),
                }),
            },
            Err(reason) => projects.push(DiffProject {
                project_root: root.clone(),
                scope: "local".to_string(),
                git_available: false,
                reason: Some(reason),
                summary: DiffSummaryCounts::default(),
                files: Vec::new(),
            }),
        }
    }
    projects
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

fn process_exists(pid: i32) -> bool {
    PathBuf::from("/proc").join(pid.to_string()).exists()
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
