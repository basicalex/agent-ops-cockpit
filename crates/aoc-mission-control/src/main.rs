use aoc_core::{
    pulse_ipc::{
        encode_frame, AgentState, CommandPayload, CommandResultPayload, DeltaPayload,
        HeartbeatPayload as PulseHeartbeatPayload, HelloPayload as PulseHelloPayload,
        LayoutStatePayload, NdjsonFrameDecoder, ProtocolVersion, SnapshotPayload, StateChangeOp,
        SubscribePayload, WireEnvelope, WireMsg, CURRENT_PROTOCOL_VERSION, DEFAULT_MAX_FRAME_BYTES,
    },
    ProjectData, TaskStatus,
};
use chrono::{DateTime, TimeZone, Utc};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
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
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

const LOCAL_LAYOUT_REFRESH_MS: u64 = 1000;
const LOCAL_SNAPSHOT_REFRESH_SECS: u64 = 2;
const HUB_STALE_SECS: i64 = 45;
const HUB_PRUNE_SECS: i64 = 90;
const HUB_OFFLINE_GRACE_SECS: i64 = 12;
const HUB_LOCAL_MISS_PRUNE_SECS: i64 = 2;
const HUB_LOCAL_ALIGNMENT_MIN_PERCENT: usize = 70;
const HUB_RECONNECT_GRACE_SECS: i64 = 2;
const MAX_DIFF_FILES: usize = 8;
const COMPACT_WIDTH: u16 = 92;
const COMMAND_QUEUE_CAPACITY: usize = 64;
const PULSE_LATENCY_WARN_MS: i64 = 1500;
const PULSE_LATENCY_INFO_EVERY: u64 = 25;

#[derive(Clone, Debug)]
struct Config {
    session_id: String,
    pane_id: String,
    pulse_socket_path: PathBuf,
    pulse_vnext_enabled: bool,
    layout_source: LayoutSource,
    client_id: String,
    project_root: PathBuf,
    state_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutSource {
    Hub,
    Local,
    Hybrid,
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
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
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
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    counts: TaskCounts,
    #[serde(default)]
    active_tasks: Option<Vec<ActiveTask>>,
    #[serde(default)]
    error: Option<PayloadError>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct PayloadError {
    code: String,
    message: String,
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
    health: HashMap<String, HealthSnapshot>,
    layout: Option<HubLayout>,
    last_seq: u64,
}

#[derive(Clone, Debug, Default)]
struct HubLayout {
    layout_seq: u64,
    pane_tabs: HashMap<String, TabMeta>,
    focused_tab_index: Option<usize>,
}

#[derive(Clone, Debug)]
struct PendingCommand {
    command: String,
    target: String,
}

#[derive(Clone, Debug)]
struct HubCommand {
    request_id: String,
    command: String,
    target_agent_id: Option<String>,
    args: Value,
}

#[derive(Clone, Debug)]
struct OverviewRow {
    identity_key: String,
    label: String,
    lifecycle: String,
    snippet: Option<String>,
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
    project_tabs: HashMap<String, TabMeta>,
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

#[derive(Deserialize, Serialize, Clone, Debug)]
struct CheckOutcome {
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    details: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct DependencyStatus {
    name: String,
    #[serde(default)]
    available: bool,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct HealthSnapshot {
    #[serde(default)]
    dependencies: Vec<DependencyStatus>,
    #[serde(default)]
    checks: Vec<CheckOutcome>,
    #[serde(default)]
    taskmaster_status: String,
}

#[derive(Clone, Debug)]
struct HealthRow {
    scope: String,
    project_root: String,
    snapshot: HealthSnapshot,
}

#[derive(Clone, Debug)]
struct LocalSnapshot {
    overview: Vec<OverviewRow>,
    viewer_tab_index: Option<usize>,
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
    Snapshot {
        payload: SnapshotPayload,
        event_at: DateTime<Utc>,
    },
    Delta {
        payload: DeltaPayload,
        event_at: DateTime<Utc>,
    },
    LayoutState {
        payload: LayoutStatePayload,
    },
    Heartbeat {
        payload: PulseHeartbeatPayload,
        event_at: DateTime<Utc>,
    },
    CommandResult {
        payload: CommandResultPayload,
        request_id: Option<String>,
    },
}

#[derive(Clone, Debug)]
struct PendingRenderLatency {
    sample_id: u64,
    agent_id: String,
    channel: &'static str,
    emitted_at_ms: i64,
    hub_event_at_ms: i64,
    ingest_latency_ms: i64,
}

struct App {
    config: Config,
    command_tx: mpsc::Sender<HubCommand>,
    connected: bool,
    hub_disconnected_at: Option<DateTime<Utc>>,
    hub: HubCache,
    local: LocalSnapshot,
    tab_cache: HashMap<String, TabMeta>,
    mode: Mode,
    scroll: u16,
    help_open: bool,
    selected_overview: usize,
    follow_viewer_tab: bool,
    last_viewer_tab_index: Option<usize>,
    status_note: Option<String>,
    pending_commands: HashMap<String, PendingCommand>,
    next_request_id: u64,
    pending_render_latency: Vec<PendingRenderLatency>,
    parser_confidence: HashMap<String, u8>,
    latency_sample_count: u64,
}

fn seed_tab_cache(rows: &[OverviewRow]) -> HashMap<String, TabMeta> {
    let mut cache = HashMap::new();
    merge_tab_cache(&mut cache, rows);
    cache
}

fn merge_tab_cache(cache: &mut HashMap<String, TabMeta>, rows: &[OverviewRow]) {
    for row in rows {
        let Some(index) = row.tab_index else {
            continue;
        };
        let name = row
            .tab_name
            .clone()
            .unwrap_or_else(|| format!("tab-{index}"));
        cache.insert(
            row.pane_id.clone(),
            TabMeta {
                index,
                name,
                focused: row.tab_focused,
            },
        );
    }
}

fn apply_cached_tab_meta(row: &mut OverviewRow, cache: &HashMap<String, TabMeta>) {
    let Some(cached) = cache.get(&row.pane_id) else {
        return;
    };
    if row.tab_index.is_none() {
        row.tab_index = Some(cached.index);
    }
    if row.tab_name.is_none() {
        row.tab_name = Some(cached.name.clone());
    }
    if !row.tab_focused && cached.focused {
        row.tab_focused = true;
    }
}

impl App {
    fn new(config: Config, command_tx: mpsc::Sender<HubCommand>, local: LocalSnapshot) -> Self {
        let tab_cache = seed_tab_cache(&local.overview);
        let last_viewer_tab_index = local.viewer_tab_index;
        Self {
            config,
            command_tx,
            connected: false,
            hub_disconnected_at: None,
            hub: HubCache::default(),
            local,
            tab_cache,
            mode: Mode::Overview,
            scroll: 0,
            help_open: false,
            selected_overview: 0,
            follow_viewer_tab: true,
            last_viewer_tab_index,
            status_note: None,
            pending_commands: HashMap::new(),
            next_request_id: 0,
            pending_render_latency: Vec::new(),
            parser_confidence: HashMap::new(),
            latency_sample_count: 0,
        }
    }

    fn apply_hub_event(&mut self, event: HubEvent) {
        match event {
            HubEvent::Connected => {
                self.connected = true;
                self.hub_disconnected_at = None;
                self.status_note = Some("hub connected".to_string());
            }
            HubEvent::Disconnected => {
                self.connected = false;
                self.hub_disconnected_at = Some(Utc::now());
                self.pending_commands.clear();
                self.pending_render_latency.clear();
                self.status_note = Some(if self.has_any_hub_data() {
                    "hub reconnecting; holding last snapshot".to_string()
                } else {
                    "hub offline; local fallback active".to_string()
                });
            }
            HubEvent::Snapshot { payload, event_at } => {
                self.hub.agents.clear();
                self.hub.tasks.clear();
                self.hub.diffs.clear();
                self.hub.health.clear();
                self.hub.last_seq = payload.seq;
                for state in payload.states {
                    self.upsert_hub_state(state, event_at, "snapshot");
                }
            }
            HubEvent::Delta { payload, event_at } => {
                if payload.seq <= self.hub.last_seq {
                    return;
                }
                if self.hub.last_seq > 0 && payload.seq > self.hub.last_seq + 1 {
                    self.hub.agents.clear();
                    self.hub.tasks.clear();
                    self.hub.diffs.clear();
                    self.hub.health.clear();
                    self.status_note = Some("hub delta gap detected; awaiting resync".to_string());
                }
                self.hub.last_seq = payload.seq;
                for change in payload.changes {
                    match change.op {
                        StateChangeOp::Upsert => {
                            if let Some(state) = change.state {
                                self.upsert_hub_state(state, event_at, "delta");
                            }
                        }
                        StateChangeOp::Remove => {
                            self.hub.agents.remove(&change.agent_id);
                            self.hub.diffs.remove(&change.agent_id);
                            self.hub.health.remove(&change.agent_id);
                            self.hub.tasks.retain(|key, payload| {
                                if payload.agent_id == change.agent_id {
                                    return false;
                                }
                                key.rsplit_once("::")
                                    .map(|(agent_id, _)| agent_id != change.agent_id)
                                    .unwrap_or(true)
                            });
                        }
                    }
                }
            }
            HubEvent::LayoutState { payload } => {
                if payload.session_id != self.config.session_id {
                    return;
                }
                if self
                    .hub
                    .layout
                    .as_ref()
                    .map(|layout| payload.layout_seq <= layout.layout_seq)
                    .unwrap_or(false)
                {
                    return;
                }
                self.hub.layout = Some(hub_layout_from_payload(&payload));
                self.update_viewer_tab_index(self.viewer_tab_index_from_hub_layout());
            }
            HubEvent::Heartbeat { payload, event_at } => {
                let entry = self
                    .hub
                    .agents
                    .entry(payload.agent_id.clone())
                    .or_insert_with(|| HubAgent {
                        status: Some(AgentStatusPayload {
                            agent_id: payload.agent_id.clone(),
                            status: payload
                                .lifecycle
                                .clone()
                                .unwrap_or_else(|| "running".to_string()),
                            pane_id: extract_pane_id(&payload.agent_id),
                            project_root: "(unknown)".to_string(),
                            agent_label: Some(extract_label(&payload.agent_id)),
                            message: None,
                        }),
                        last_seen: event_at,
                        last_heartbeat: None,
                        last_activity: None,
                    });
                entry.last_seen = event_at;
                entry.last_heartbeat = ms_to_datetime(payload.last_heartbeat_ms).or(Some(event_at));
                if let Some(lifecycle) = payload.lifecycle.as_ref() {
                    if let Some(status) = entry.status.as_mut() {
                        status.status = normalize_lifecycle(lifecycle);
                    }
                }
                self.observe_heartbeat_latency(&payload, event_at);
            }
            HubEvent::CommandResult {
                payload,
                request_id,
            } => {
                self.apply_command_result(payload, request_id);
            }
        }
    }

    fn upsert_hub_state(
        &mut self,
        state: AgentState,
        event_at: DateTime<Utc>,
        channel: &'static str,
    ) {
        let key = state.agent_id.clone();
        self.observe_state_latency(&state, event_at, channel);
        self.observe_parser_confidence_transition(&state.agent_id, &state.source, channel);
        let status = status_payload_from_state(&state);
        let project_root = status.project_root.clone();
        let heartbeat_at = state.last_heartbeat_ms.and_then(ms_to_datetime);
        let activity_at = state.last_activity_ms.and_then(ms_to_datetime);
        let entry = self.hub.agents.entry(key).or_insert(HubAgent {
            status: None,
            last_seen: event_at,
            last_heartbeat: None,
            last_activity: None,
        });
        entry.status = Some(status);
        entry.last_seen = event_at;
        entry.last_heartbeat = heartbeat_at.or(Some(event_at));
        entry.last_activity = activity_at.or(Some(event_at));

        if let Some(source_value) =
            source_value_by_keys(&state.source, &["task_summaries", "task_summary"])
        {
            match parse_task_summaries_from_source(source_value, &state.agent_id) {
                Ok(task_summaries) => {
                    self.hub
                        .tasks
                        .retain(|_, payload| payload.agent_id != state.agent_id);
                    for payload in task_summaries {
                        let key = format!("{}::{}", payload.agent_id, payload.tag);
                        self.hub.tasks.insert(key, payload);
                    }
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "task_summary",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["diff_summary"]) {
            match parse_diff_summary_from_source(source_value, &state.agent_id, &project_root) {
                Ok(Some(payload)) => {
                    self.hub.diffs.insert(state.agent_id.clone(), payload);
                }
                Ok(None) => {
                    self.hub.diffs.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "diff_summary",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        }

        if let Some(source_value) =
            source_value_by_keys(&state.source, &["health", "health_summary"])
        {
            match parse_health_from_source(source_value) {
                Ok(Some(snapshot)) => {
                    self.hub.health.insert(state.agent_id.clone(), snapshot);
                }
                Ok(None) => {
                    self.hub.health.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "health",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        }
    }

    fn observe_state_latency(
        &mut self,
        state: &AgentState,
        event_at: DateTime<Utc>,
        channel: &'static str,
    ) {
        let emitted_at_ms = state
            .updated_at_ms
            .or(state.last_activity_ms)
            .or(state.last_heartbeat_ms);
        let Some(emitted_at_ms) = emitted_at_ms else {
            return;
        };
        let hub_event_at_ms = event_at.timestamp_millis();
        let ingest_latency_ms = hub_event_at_ms.saturating_sub(emitted_at_ms);
        self.latency_sample_count = self.latency_sample_count.saturating_add(1);
        let sample_id = self.latency_sample_count;
        if ingest_latency_ms >= PULSE_LATENCY_WARN_MS {
            warn!(
                event = "pulse_end_to_end_latency",
                stage = "hub_ingest",
                sample_id,
                channel,
                agent_id = %state.agent_id,
                emit_ts_ms = emitted_at_ms,
                hub_event_ts_ms = hub_event_at_ms,
                latency_ms = ingest_latency_ms
            );
        } else if sample_id % PULSE_LATENCY_INFO_EVERY == 0 {
            info!(
                event = "pulse_end_to_end_latency",
                stage = "hub_ingest",
                sample_id,
                channel,
                agent_id = %state.agent_id,
                latency_ms = ingest_latency_ms
            );
        }
        self.pending_render_latency.push(PendingRenderLatency {
            sample_id,
            agent_id: state.agent_id.clone(),
            channel,
            emitted_at_ms,
            hub_event_at_ms,
            ingest_latency_ms,
        });
    }

    fn observe_heartbeat_latency(
        &mut self,
        payload: &PulseHeartbeatPayload,
        event_at: DateTime<Utc>,
    ) {
        let ingest_latency_ms = event_at
            .timestamp_millis()
            .saturating_sub(payload.last_heartbeat_ms);
        if ingest_latency_ms >= PULSE_LATENCY_WARN_MS {
            warn!(
                event = "pulse_end_to_end_latency",
                stage = "heartbeat_ingest",
                agent_id = %payload.agent_id,
                latency_ms = ingest_latency_ms
            );
        }
    }

    fn observe_parser_confidence_transition(
        &mut self,
        agent_id: &str,
        source: &Option<Value>,
        channel: &'static str,
    ) {
        let Some(next_confidence) = source_confidence(source) else {
            return;
        };
        let previous = self
            .parser_confidence
            .insert(agent_id.to_string(), next_confidence);
        if previous == Some(next_confidence) {
            return;
        }
        info!(
            event = "pulse_parser_confidence_transition",
            channel,
            agent_id,
            previous = previous.unwrap_or(0),
            next = next_confidence
        );
    }

    fn observe_render_latency(&mut self) {
        if self.pending_render_latency.is_empty() {
            return;
        }
        let now_ms = Utc::now().timestamp_millis();
        for sample in self.pending_render_latency.drain(..) {
            let total_latency_ms = now_ms.saturating_sub(sample.emitted_at_ms);
            let render_latency_ms = now_ms.saturating_sub(sample.hub_event_at_ms);
            if total_latency_ms >= PULSE_LATENCY_WARN_MS {
                warn!(
                    event = "pulse_end_to_end_latency",
                    stage = "render",
                    sample_id = sample.sample_id,
                    channel = sample.channel,
                    agent_id = %sample.agent_id,
                    ingest_latency_ms = sample.ingest_latency_ms,
                    hub_to_render_ms = render_latency_ms,
                    total_latency_ms
                );
            } else if sample.sample_id % PULSE_LATENCY_INFO_EVERY == 0 {
                info!(
                    event = "pulse_end_to_end_latency",
                    stage = "render",
                    sample_id = sample.sample_id,
                    channel = sample.channel,
                    agent_id = %sample.agent_id,
                    ingest_latency_ms = sample.ingest_latency_ms,
                    hub_to_render_ms = render_latency_ms,
                    total_latency_ms
                );
            }
        }
    }

    fn apply_command_result(&mut self, payload: CommandResultPayload, request_id: Option<String>) {
        let tracked = request_id
            .as_deref()
            .and_then(|id| self.pending_commands.get(id).cloned());
        let done = !payload.status.eq_ignore_ascii_case("accepted");
        if done {
            if let Some(id) = request_id.as_deref() {
                self.pending_commands.remove(id);
            }
        }
        let target = tracked
            .as_ref()
            .map(|value| value.target.clone())
            .unwrap_or_else(|| "hub".to_string());
        let command_name = tracked
            .as_ref()
            .map(|value| value.command.clone())
            .unwrap_or_else(|| payload.command.clone());
        let mut message = payload
            .message
            .clone()
            .unwrap_or_else(|| payload.status.clone());
        if let Some(error) = payload.error.as_ref() {
            message = format!("{} ({})", error.message, error.code);
        }
        self.status_note = Some(format!(
            "{} {} -> {}",
            command_name,
            target,
            ellipsize(&message, 72)
        ));
    }

    fn next_command_request_id(&mut self) -> String {
        self.next_request_id = self.next_request_id.saturating_add(1);
        format!("pulse-{}-{}", std::process::id(), self.next_request_id)
    }

    fn queue_hub_command(
        &mut self,
        command: &str,
        target_agent_id: Option<String>,
        args: Value,
        target_label: String,
    ) {
        if !self.connected {
            self.status_note = Some("hub offline; command unavailable".to_string());
            return;
        }
        let request_id = self.next_command_request_id();
        let outbound = HubCommand {
            request_id: request_id.clone(),
            command: command.to_string(),
            target_agent_id,
            args,
        };
        match self.command_tx.try_send(outbound) {
            Ok(()) => {
                let queued_target = target_label.clone();
                self.pending_commands.insert(
                    request_id,
                    PendingCommand {
                        command: command.to_string(),
                        target: target_label,
                    },
                );
                self.status_note = Some(format!("{command} queued for {queued_target}"));
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(
                    event = "pulse_command_queue_drop",
                    reason = "queue_full",
                    command,
                    pending = self.pending_commands.len(),
                    capacity = COMMAND_QUEUE_CAPACITY
                );
                self.status_note = Some("hub command queue full".to_string());
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!(
                    event = "pulse_command_queue_drop",
                    reason = "channel_closed",
                    command,
                    pending = self.pending_commands.len()
                );
                self.status_note = Some("hub command channel closed".to_string());
            }
        }
    }

    fn set_local(&mut self, local: LocalSnapshot) {
        let LocalSnapshot {
            overview,
            viewer_tab_index,
            work,
            diff,
            health,
        } = local;
        self.set_local_overview(overview, viewer_tab_index);
        self.local.work = work;
        self.local.diff = diff;
        self.local.health = health;
    }

    fn set_local_overview(&mut self, overview: Vec<OverviewRow>, viewer_tab_index: Option<usize>) {
        let viewer_tab_index = viewer_tab_index.or(self.local.viewer_tab_index);
        merge_tab_cache(&mut self.tab_cache, &overview);
        self.update_viewer_tab_index(viewer_tab_index);
        self.local.overview = overview;
    }

    fn update_viewer_tab_index(&mut self, viewer_tab_index: Option<usize>) {
        let viewer_tab_index = viewer_tab_index.or(self.local.viewer_tab_index);
        if viewer_tab_index != self.last_viewer_tab_index {
            self.follow_viewer_tab = true;
        }
        self.last_viewer_tab_index = viewer_tab_index;
        self.local.viewer_tab_index = viewer_tab_index;
    }

    fn viewer_tab_index_from_hub_layout(&self) -> Option<usize> {
        self.active_hub_layout().and_then(|layout| {
            if self.config.pane_id.trim().is_empty() {
                return layout.focused_tab_index;
            }
            layout
                .pane_tabs
                .get(&self.config.pane_id)
                .map(|meta| meta.index)
                .or(layout.focused_tab_index)
        })
    }

    fn active_hub_layout(&self) -> Option<&HubLayout> {
        if self.config.layout_source == LayoutSource::Local {
            return None;
        }
        if self.connected || self.hub_reconnect_grace_active() {
            return self.hub.layout.as_ref();
        }
        None
    }

    fn should_poll_local_layout(&self) -> bool {
        match self.config.layout_source {
            LayoutSource::Local => true,
            LayoutSource::Hybrid => true,
            LayoutSource::Hub => {
                if !self.connected {
                    return true;
                }
                self.hub.layout.is_none()
            }
        }
    }

    fn refresh_local_layout(&mut self) {
        let (overview, viewer_tab_index) =
            collect_layout_overview(&self.config, &self.local.overview, &self.tab_cache);
        self.set_local_overview(overview, viewer_tab_index);
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
        let hub_agent_ids: HashSet<String> = self.hub.agents.keys().cloned().collect();
        let overlap = hub_agent_ids.intersection(&local_online).count();
        let local_alignment_confident = !hub_agent_ids.is_empty()
            && !local_online.is_empty()
            && (overlap * 100) >= (hub_agent_ids.len() * HUB_LOCAL_ALIGNMENT_MIN_PERCENT);
        self.hub.agents.retain(|agent_id, agent| {
            let age = now
                .signed_duration_since(agent.last_seen)
                .num_seconds()
                .max(0);
            if self.connected
                && local_alignment_confident
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
        self.hub
            .health
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
                if self.prefer_hub_data(!self.hub.agents.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Work => {
                if self.prefer_hub_data(!self.hub.tasks.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Diff => {
                if self.prefer_hub_data(!self.hub.diffs.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Health => {
                if self.prefer_hub_data(!self.hub.health.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
        }
    }

    fn hub_status_label(&self) -> &'static str {
        if self.connected {
            "online"
        } else if self.hub_reconnect_grace_active() && self.has_any_hub_data() {
            "reconnecting"
        } else {
            "offline"
        }
    }

    fn has_any_hub_data(&self) -> bool {
        !self.hub.agents.is_empty()
            || !self.hub.tasks.is_empty()
            || !self.hub.diffs.is_empty()
            || !self.hub.health.is_empty()
            || self.hub.layout.is_some()
    }

    fn hub_reconnect_grace_active(&self) -> bool {
        if self.connected {
            return false;
        }
        let Some(disconnected_at) = self.hub_disconnected_at else {
            return false;
        };
        Utc::now()
            .signed_duration_since(disconnected_at)
            .num_seconds()
            <= HUB_RECONNECT_GRACE_SECS
    }

    fn prefer_hub_data(&self, has_hub_data: bool) -> bool {
        has_hub_data && (self.connected || self.hub_reconnect_grace_active())
    }

    fn overview_rows(&self) -> Vec<OverviewRow> {
        if self.prefer_hub_data(!self.hub.agents.is_empty()) {
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
                    lifecycle: status
                        .map(|s| normalize_lifecycle(&s.status))
                        .unwrap_or_else(|| "running".to_string()),
                    snippet: status.and_then(|s| s.message.clone()),
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
                        existing.source = "mix".to_string();
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
                }
            }

            let mut merged_rows: Vec<OverviewRow> = rows.into_values().collect();
            for row in &mut merged_rows {
                if let Some(meta) = self
                    .active_hub_layout()
                    .and_then(|layout| layout.pane_tabs.get(&row.pane_id))
                {
                    row.tab_index = Some(meta.index);
                    row.tab_name = Some(meta.name.clone());
                    row.tab_focused = meta.focused;
                }
                apply_cached_tab_meta(row, &self.tab_cache);
            }
            return sort_overview_rows(merged_rows);
        }
        let mut local_rows = self.local.overview.clone();
        for row in &mut local_rows {
            if let Some(meta) = self
                .active_hub_layout()
                .and_then(|layout| layout.pane_tabs.get(&row.pane_id))
            {
                row.tab_index = Some(meta.index);
                row.tab_name = Some(meta.name.clone());
                row.tab_focused = meta.focused;
            }
            apply_cached_tab_meta(row, &self.tab_cache);
        }
        sort_overview_rows(local_rows)
    }

    fn work_rows(&self) -> Vec<WorkProject> {
        if self.prefer_hub_data(!self.hub.tasks.is_empty()) {
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
        if self.prefer_hub_data(!self.hub.diffs.is_empty()) {
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

    fn health_rows(&self) -> Vec<HealthRow> {
        if self.prefer_hub_data(!self.hub.health.is_empty()) {
            let mut rows = Vec::new();
            for (agent_id, snapshot) in &self.hub.health {
                let status = self
                    .hub
                    .agents
                    .get(agent_id)
                    .and_then(|agent| agent.status.as_ref());
                let scope = status
                    .and_then(|value| value.agent_label.clone())
                    .unwrap_or_else(|| extract_label(agent_id));
                let project_root = status
                    .map(|value| value.project_root.clone())
                    .unwrap_or_else(|| "(unknown)".to_string());
                rows.push(HealthRow {
                    scope,
                    project_root,
                    snapshot: snapshot.clone(),
                });
            }
            rows.sort_by(|left, right| {
                left.project_root
                    .cmp(&right.project_root)
                    .then_with(|| left.scope.cmp(&right.scope))
            });
            return rows;
        }
        vec![HealthRow {
            scope: "local".to_string(),
            project_root: self.config.project_root.to_string_lossy().to_string(),
            snapshot: self.local.health.clone(),
        }]
    }

    fn viewer_tab_overview_index(rows: &[OverviewRow], tab_index: Option<usize>) -> Option<usize> {
        let tab_index = tab_index?;
        rows.iter().position(|row| row.tab_index == Some(tab_index))
    }

    fn selected_overview_index_for_rows(&self, rows: &[OverviewRow]) -> usize {
        if rows.is_empty() {
            return 0;
        }
        if self.follow_viewer_tab {
            let viewer_tab = self
                .viewer_tab_index_from_hub_layout()
                .or(self.local.viewer_tab_index);
            if let Some(index) = Self::viewer_tab_overview_index(rows, viewer_tab) {
                return index;
            }
        }
        self.selected_overview.min(rows.len().saturating_sub(1))
    }

    fn move_overview_selection(&mut self, step: i32) {
        let rows = self.overview_rows();
        let len = rows.len();
        if len == 0 {
            self.selected_overview = 0;
            return;
        }
        let current = self.selected_overview_index_for_rows(&rows) as i32;
        let max = len.saturating_sub(1) as i32;
        let next = (current + step).clamp(0, max) as usize;
        self.follow_viewer_tab = false;
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
        let selected = self.selected_overview_index_for_rows(&rows);
        self.selected_overview = selected;
        let row = &rows[selected];
        if self.connected {
            let mut args = serde_json::Map::new();
            if let Some(tab_index) = row.tab_index {
                args.insert("tab_index".to_string(), Value::from(tab_index as u64));
            }
            if let Some(tab_name) = row.tab_name.as_ref() {
                if !tab_name.trim().is_empty() {
                    args.insert("tab_name".to_string(), Value::String(tab_name.clone()));
                }
            }
            if args.is_empty() {
                self.status_note = Some(format!("no tab mapping for pane {}", row.pane_id));
                return;
            }
            self.queue_hub_command(
                "focus_tab",
                None,
                Value::Object(args),
                format!("pane {}", row.pane_id),
            );
            return;
        }

        let Some(tab_index) = row.tab_index else {
            self.status_note = Some(format!("no tab mapping for pane {}", row.pane_id));
            return;
        };
        if let Err(err) = go_to_tab(&self.config.session_id, tab_index) {
            self.status_note = Some(format!("focus failed: {err}"));
        } else {
            self.status_note = Some(format!(
                "focused tab {} for pane {}",
                tab_index, row.pane_id
            ));
        }
    }

    fn stop_selected_overview_agent(&mut self) {
        if self.mode != Mode::Overview {
            return;
        }
        let rows = self.overview_rows();
        if rows.is_empty() {
            self.status_note = Some("no agents to stop".to_string());
            return;
        }
        let selected = self.selected_overview_index_for_rows(&rows);
        self.selected_overview = selected;
        let row = &rows[selected];
        self.queue_hub_command(
            "stop_agent",
            Some(row.identity_key.clone()),
            serde_json::json!({"reason": "pulse_user_request"}),
            format!("{}::{}", row.label, row.pane_id),
        );
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
    let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let mut app = App::new(config.clone(), cmd_tx, initial_local);

    let (hub_tx, mut hub_rx) = mpsc::channel(256);
    let _hub_tx_guard = if config.pulse_vnext_enabled {
        let hub_cfg = config.clone();
        tokio::spawn(async move {
            hub_loop(hub_cfg, hub_tx, cmd_rx).await;
        });
        None
    } else {
        Some(hub_tx)
    };
    if !config.pulse_vnext_enabled {
        app.status_note = Some(
            "pulse vNext disabled (AOC_PULSE_VNEXT_ENABLED=0); local fallback active".to_string(),
        );
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut events = EventStream::new();
    let jitter_seed = u64::from(std::process::id()) % 5;
    let mut layout_ticker = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_millis(jitter_seed * 120),
        Duration::from_millis(LOCAL_LAYOUT_REFRESH_MS),
    );
    let mut snapshot_ticker = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_millis(jitter_seed * 240),
        Duration::from_secs(LOCAL_SNAPSHOT_REFRESH_SECS),
    );
    let mut layout_refresh_requested = false;
    let mut snapshot_refresh_requested = false;

    loop {
        if snapshot_refresh_requested {
            app.set_local(collect_local(&app.config));
            snapshot_refresh_requested = false;
            layout_refresh_requested = false;
        } else if layout_refresh_requested {
            app.refresh_local_layout();
            layout_refresh_requested = false;
        }

        app.prune_hub_cache();

        terminal.draw(|frame| render_ui(frame, &app))?;
        app.observe_render_latency();
        tokio::select! {
            _ = snapshot_ticker.tick() => {
                snapshot_refresh_requested = true;
            }
            _ = layout_ticker.tick() => {
                if app.mode == Mode::Overview && app.should_poll_local_layout() {
                    layout_refresh_requested = true;
                }
            }
            Some(event) = hub_rx.recv() => {
                app.apply_hub_event(event);
            }
            maybe_event = events.next() => {
                if let Some(Ok(event)) = maybe_event {
                    if handle_input(event, &mut app, &mut snapshot_refresh_requested) {
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
        render_overview_panel(frame, app, theme, layout[2]);
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
    let hub = app.hub_status_label();
    let session = ellipsize(&app.config.session_id, if compact { 14 } else { 28 });
    let inner_width = width.saturating_sub(4) as usize;
    let status_fields = vec![
        format!("Mode: {}", app.mode.title()),
        format!("Hub: {hub}"),
        format!("Source: {source}"),
        format!(
            "Agents: {}/{} Online",
            kpis.online_agents, kpis.total_agents
        ),
        format!("Session: {session}"),
    ];
    let status_line = fit_fields(&status_fields, inner_width.max(12));

    let action_text = if let Some(note) = app.status_note.as_deref() {
        format!("Last Action: {}", ellipsize(note, inner_width.max(12)))
    } else if compact {
        "Last Action: ready".to_string()
    } else {
        "Last Action: ready (Enter focus, x stop, ? help)".to_string()
    };

    Paragraph::new(Text::from(vec![
        Line::from(Span::styled(status_line, Style::default().fg(theme.text))),
        Line::from(Span::styled(
            ellipsize(&action_text, inner_width.max(12)),
            Style::default().fg(if app.status_note.is_some() {
                status_note_color(app.status_note.as_deref().unwrap_or_default(), theme)
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
        format!(
            "Agents: {}/{} Online",
            kpis.online_agents, kpis.total_agents
        ),
        format!("Global Tasks: {} Active", kpis.in_progress),
        format!("Unsynced Changes: {}", kpis.dirty_files),
        format!("Blocked: {}", kpis.blocked),
    ];
    if !compact {
        fields.push(format!("Churn: {}", kpis.churn));
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

fn render_overview_panel(frame: &mut ratatui::Frame, app: &App, theme: PulseTheme, area: Rect) {
    let compact = is_compact(area.width);
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
        .map(|row| {
            ListItem::new(Line::from(overview_row_spans(
                row, theme, compact, area.width,
            )))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.selected_overview_index_for_rows(&rows)));
    let list = List::new(items)
        .highlight_symbol(">> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
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
    let presenter = overview_row_presenter(row, compact, width);
    let lifecycle_color = lifecycle_color(&row.lifecycle, row.online, theme);
    let age_color = age_color(row.age_secs, row.online, theme);
    let source_color = source_chip_color(presenter.source_chip, theme);
    let liveness_color = if row.online { theme.ok } else { theme.critical };
    let mut spans = vec![
        Span::styled(
            presenter.liveness_chip,
            Style::default()
                .fg(liveness_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            presenter.identity,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            presenter.lifecycle_chip,
            Style::default()
                .fg(lifecycle_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            presenter.location_chip,
            Style::default().fg(if row.tab_focused {
                theme.accent
            } else {
                theme.muted
            }),
        ),
        Span::raw(" "),
        Span::styled(
            presenter.source_chip.bracketed(),
            Style::default()
                .fg(source_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(presenter.heartbeat, Style::default().fg(age_color)),
    ];
    if let Some(context) = presenter.context {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(context, Style::default().fg(theme.muted)));
    }
    spans
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceChip {
    Hub,
    Local,
    Mixed,
}

impl SourceChip {
    fn label(self) -> &'static str {
        match self {
            SourceChip::Hub => "HUB",
            SourceChip::Local => "LOC",
            SourceChip::Mixed => "MIX",
        }
    }

    fn bracketed(self) -> String {
        format!("[{}]", self.label())
    }
}

#[derive(Clone, Debug)]
struct OverviewRowPresenter {
    liveness_chip: String,
    identity: String,
    lifecycle_chip: String,
    location_chip: String,
    source_chip: SourceChip,
    heartbeat: String,
    context: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct PresenterBudgets {
    label: usize,
    pane: usize,
    tab_name: usize,
    root: usize,
    snippet: usize,
    include_root: bool,
    include_snippet: bool,
}

fn overview_row_presenter(row: &OverviewRow, compact: bool, width: u16) -> OverviewRowPresenter {
    let mut plans = vec![PresenterBudgets {
        label: if compact { 14 } else { 20 },
        pane: if compact { 10 } else { 14 },
        tab_name: if compact { 8 } else { 14 },
        root: if compact { 0 } else { 16 },
        snippet: if compact { 0 } else { 22 },
        include_root: !compact,
        include_snippet: !compact,
    }];
    plans.push(PresenterBudgets {
        include_snippet: false,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        include_snippet: false,
        include_root: false,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        include_snippet: false,
        include_root: false,
        tab_name: 6,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        include_snippet: false,
        include_root: false,
        tab_name: 0,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        include_snippet: false,
        include_root: false,
        tab_name: 0,
        label: 12,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        include_snippet: false,
        include_root: false,
        tab_name: 0,
        label: 8,
        pane: 8,
        ..plans[0]
    });

    let max_width = width.saturating_sub(8) as usize;
    for plan in plans {
        let presenter = overview_row_presenter_with_budget(row, plan);
        if presenter_text_len(&presenter) <= max_width.max(28) {
            return presenter;
        }
    }
    overview_row_presenter_with_budget(
        row,
        PresenterBudgets {
            label: 8,
            pane: 8,
            tab_name: 0,
            root: 0,
            snippet: 0,
            include_root: false,
            include_snippet: false,
        },
    )
}

fn overview_row_presenter_with_budget(
    row: &OverviewRow,
    budget: PresenterBudgets,
) -> OverviewRowPresenter {
    let liveness_chip = format!("[{:<4}]", if row.online { "+ON" } else { "!OFF" });
    let identity = format!(
        "{}::{}",
        ellipsize(&row.label, budget.label.max(4)),
        ellipsize(&row.pane_id, budget.pane.max(4))
    );
    let lifecycle_chip = format!("[{:<5}]", lifecycle_chip_label(&row.lifecycle, row.online));
    let location_chip = overview_location_chip(row, budget.tab_name);
    let source_chip = source_chip_from_row(&row.source);
    let heartbeat = format!(
        "HB:{} {}",
        age_meter(row.age_secs, row.online),
        format_age(row.age_secs)
    );

    let root_context = if budget.include_root && row.project_root != "(unknown)" {
        Some(format!(
            "R:{}",
            ellipsize(
                &short_project(&row.project_root, budget.root.max(4)),
                budget.root.max(4)
            )
        ))
    } else {
        None
    };
    let snippet_context = if budget.include_snippet {
        row.snippet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("M:{}", ellipsize(value, budget.snippet.max(8))))
    } else {
        None
    };
    let context = match (root_context, snippet_context) {
        (Some(root), Some(snippet)) => Some(format!("{root} {snippet}")),
        (Some(root), None) => Some(root),
        (None, Some(snippet)) => Some(snippet),
        (None, None) => None,
    };

    OverviewRowPresenter {
        liveness_chip,
        identity,
        lifecycle_chip,
        location_chip,
        source_chip,
        heartbeat,
        context,
    }
}

fn overview_location_chip(row: &OverviewRow, tab_name_budget: usize) -> String {
    let Some(tab_index) = row.tab_index else {
        return "T?:???".to_string();
    };
    let focused_suffix = if row.tab_focused { "*" } else { "" };
    if tab_name_budget == 0 {
        return format!("T{tab_index}{focused_suffix}");
    }
    if let Some(tab_name) = row
        .tab_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!(
            "T{tab_index}:{}{}",
            ellipsize(tab_name, tab_name_budget),
            focused_suffix
        );
    }
    format!("T{tab_index}:???{focused_suffix}")
}

fn lifecycle_chip_label(lifecycle: &str, online: bool) -> &'static str {
    if !online {
        return "OFF";
    }
    match normalize_lifecycle(lifecycle).as_str() {
        "error" => "ERR",
        "blocked" => "BLOCK",
        "needs-input" => "NEEDS",
        "busy" => "BUSY",
        "idle" => "IDLE",
        _ => "RUN",
    }
}

fn source_chip_from_row(source: &str) -> SourceChip {
    let normalized = source.trim().to_ascii_lowercase();
    if normalized == "hub" {
        return SourceChip::Hub;
    }
    if normalized.contains("hub+")
        || normalized.contains("+hub")
        || normalized == "mix"
        || normalized.starts_with("mix+")
    {
        return SourceChip::Mixed;
    }
    SourceChip::Local
}

fn source_chip_color(chip: SourceChip, theme: PulseTheme) -> Color {
    match chip {
        SourceChip::Hub => theme.info,
        SourceChip::Local => theme.warn,
        SourceChip::Mixed => theme.accent,
    }
}

fn presenter_text_len(presenter: &OverviewRowPresenter) -> usize {
    let mut len = presenter.liveness_chip.chars().count()
        + 1
        + presenter.identity.chars().count()
        + 1
        + presenter.lifecycle_chip.chars().count()
        + 1
        + presenter.location_chip.chars().count()
        + 1
        + presenter.source_chip.bracketed().chars().count()
        + 1
        + presenter.heartbeat.chars().count();
    if let Some(context) = presenter.context.as_ref() {
        len += 1 + context.chars().count();
    }
    len
}

fn status_note_color(note: &str, theme: PulseTheme) -> Color {
    let normalized = note.to_ascii_lowercase();
    if normalized.contains("failed")
        || normalized.contains("error")
        || normalized.contains("offline")
        || normalized.contains("no tab mapping")
    {
        return theme.critical;
    }
    if normalized.contains("queued") || normalized.contains("focus") || normalized.contains("stop")
    {
        return theme.info;
    }
    theme.warn
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
            Line::from("  j/k      select agent row (>> + reverse)"),
            Line::from("  g        jump to first agent"),
            Line::from("  Enter    focus selected tab; unmapped -> pane note"),
            Line::from("  x        request stop selected agent"),
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
    let health_rows = app.health_rows();
    for (idx, row) in health_rows.iter().enumerate() {
        if idx > 0 && !compact {
            lines.push(Line::from(""));
        }
        push_health_snapshot_lines(
            &mut lines,
            &row.snapshot,
            &row.scope,
            &row.project_root,
            theme,
            compact,
        );
    }
    lines
}

fn push_health_snapshot_lines(
    lines: &mut Vec<Line<'static>>,
    snapshot: &HealthSnapshot,
    scope: &str,
    project_root: &str,
    theme: PulseTheme,
    compact: bool,
) {
    lines.push(Line::from(vec![
        Span::styled(scope.to_string(), Style::default().fg(theme.title)),
        Span::raw(" "),
        Span::styled(
            format!(
                "@{}",
                short_project(project_root, if compact { 16 } else { 28 })
            ),
            Style::default().fg(theme.muted),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  taskmaster ", Style::default().fg(theme.title)),
        Span::styled(
            ellipsize(&snapshot.taskmaster_status, if compact { 34 } else { 72 }),
            Style::default().fg(if snapshot.taskmaster_status.contains("available") {
                theme.ok
            } else {
                theme.warn
            }),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "  dependencies",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    for dep in &snapshot.dependencies {
        lines.push(Line::from(vec![
            Span::raw("    "),
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
        "  checks",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    for check in &snapshot.checks {
        lines.push(Line::from(vec![
            Span::raw("    "),
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
                    if compact { 20 } else { 52 },
                ),
                Style::default().fg(theme.muted),
            ),
        ]));
    }
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

fn normalize_lifecycle(raw: &str) -> String {
    let normalized = raw.trim().to_ascii_lowercase().replace('_', "-");
    if normalized.is_empty() {
        "running".to_string()
    } else {
        normalized
    }
}

fn lifecycle_color(lifecycle: &str, online: bool, theme: PulseTheme) -> Color {
    if !online {
        return theme.critical;
    }
    match normalize_lifecycle(lifecycle).as_str() {
        "error" => theme.critical,
        "needs-input" | "blocked" => theme.warn,
        "busy" => theme.info,
        "idle" => theme.muted,
        _ => theme.ok,
    }
}

fn pane_id_number(pane_id: &str) -> Option<u64> {
    pane_id.trim().parse::<u64>().ok()
}

fn sort_overview_rows(mut rows: Vec<OverviewRow>) -> Vec<OverviewRow> {
    rows.sort_by(|left, right| {
        left.tab_index
            .is_none()
            .cmp(&right.tab_index.is_none())
            .then_with(|| {
                left.tab_index
                    .unwrap_or(usize::MAX)
                    .cmp(&right.tab_index.unwrap_or(usize::MAX))
            })
            .then_with(|| {
                let left_pane = pane_id_number(&left.pane_id);
                let right_pane = pane_id_number(&right.pane_id);
                left_pane
                    .is_none()
                    .cmp(&right_pane.is_none())
                    .then_with(|| {
                        left_pane
                            .unwrap_or(u64::MAX)
                            .cmp(&right_pane.unwrap_or(u64::MAX))
                    })
                    .then_with(|| left.pane_id.cmp(&right.pane_id))
            })
            .then_with(|| left.identity_key.cmp(&right.identity_key))
    });
    rows
}

fn ms_to_datetime(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_millis_opt(value).single()
}

fn status_payload_from_state(state: &AgentState) -> AgentStatusPayload {
    let lifecycle = normalize_lifecycle(&state.lifecycle);
    let project_root = source_string_field(&state.source, "project_root")
        .unwrap_or_else(|| "(unknown)".to_string());
    let agent_label = source_string_field(&state.source, "agent_label")
        .or_else(|| source_string_field(&state.source, "label"))
        .or_else(|| Some(extract_label(&state.agent_id)));
    AgentStatusPayload {
        agent_id: state.agent_id.clone(),
        status: lifecycle,
        pane_id: state.pane_id.clone(),
        project_root,
        agent_label,
        message: state.snippet.clone(),
    }
}

fn source_string_field(source: &Option<Value>, key: &str) -> Option<String> {
    source_value_field(source, key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn source_value_by_keys<'a>(source: &'a Option<Value>, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = source_value_field(source, key) {
            return Some(value);
        }
    }
    None
}

fn source_value_field<'a>(source: &'a Option<Value>, key: &str) -> Option<&'a Value> {
    let root = source.as_ref()?.as_object()?;
    if let Some(value) = root.get(key) {
        return Some(value);
    }
    for nested_key in ["agent_status", "pulse", "telemetry"] {
        if let Some(value) = root
            .get(nested_key)
            .and_then(Value::as_object)
            .and_then(|nested| nested.get(key))
        {
            return Some(value);
        }
    }
    None
}

fn parse_task_summaries_from_source(
    value: &Value,
    fallback_agent_id: &str,
) -> Result<Vec<TaskSummaryPayload>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    if let Some(items) = value.as_array() {
        let mut parsed = Vec::new();
        for item in items {
            parsed.push(parse_task_summary_item(item, fallback_agent_id, "default")?);
        }
        parsed.sort_by(|left, right| left.tag.cmp(&right.tag));
        return Ok(parsed);
    }
    if let Some(map) = value.as_object() {
        if looks_like_task_summary_payload(map) {
            return Ok(vec![parse_task_summary_item(
                value,
                fallback_agent_id,
                "default",
            )?]);
        }
        let mut parsed = Vec::new();
        for (tag, item) in map {
            parsed.push(parse_task_summary_item(item, fallback_agent_id, tag)?);
        }
        parsed.sort_by(|left, right| left.tag.cmp(&right.tag));
        return Ok(parsed);
    }
    Err("unsupported task summary source shape".to_string())
}

fn parse_task_summary_item(
    value: &Value,
    fallback_agent_id: &str,
    fallback_tag: &str,
) -> Result<TaskSummaryPayload, String> {
    let mut payload: TaskSummaryPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    if payload.agent_id.trim().is_empty() {
        payload.agent_id = fallback_agent_id.to_string();
    }
    payload.tag = if payload.tag.trim().is_empty() {
        fallback_tag.to_string()
    } else {
        payload.tag.trim().to_string()
    };
    if payload.counts.total == 0
        && payload.counts.pending == 0
        && payload.counts.in_progress == 0
        && payload.counts.done == 0
        && payload.counts.blocked == 0
    {
        if let Some(map) = value.as_object() {
            if !map.contains_key("counts")
                && (map.contains_key("total")
                    || map.contains_key("pending")
                    || map.contains_key("in_progress")
                    || map.contains_key("done")
                    || map.contains_key("blocked"))
            {
                if let Ok(counts) = serde_json::from_value::<TaskCounts>(value.clone()) {
                    payload.counts = counts;
                }
            }
        }
    }
    if let Some(active_tasks) = payload.active_tasks.as_mut() {
        for task in active_tasks {
            task.status = task.status.trim().to_ascii_lowercase().replace('_', "-");
        }
    }
    Ok(payload)
}

fn looks_like_task_summary_payload(map: &serde_json::Map<String, Value>) -> bool {
    map.contains_key("tag")
        || map.contains_key("counts")
        || map.contains_key("active_tasks")
        || map.contains_key("error")
        || map.contains_key("agent_id")
}

fn parse_diff_summary_from_source(
    value: &Value,
    fallback_agent_id: &str,
    fallback_project_root: &str,
) -> Result<Option<DiffSummaryPayload>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut payload: DiffSummaryPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    if payload.agent_id.trim().is_empty() {
        payload.agent_id = fallback_agent_id.to_string();
    }
    if payload.repo_root.trim().is_empty() {
        payload.repo_root = fallback_project_root.to_string();
    }
    payload.reason = payload
        .reason
        .as_ref()
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty());
    Ok(Some(payload))
}

fn parse_health_from_source(value: &Value) -> Result<Option<HealthSnapshot>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut snapshot: HealthSnapshot =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    if snapshot.taskmaster_status.trim().is_empty() {
        snapshot.taskmaster_status = "unknown".to_string();
    }
    for check in &mut snapshot.checks {
        if check.status.trim().is_empty() {
            check.status = "unknown".to_string();
        }
    }
    Ok(Some(snapshot))
}

fn source_confidence(source: &Option<Value>) -> Option<u8> {
    source
        .as_ref()
        .and_then(|value| source_numeric_field(value, "parser_confidence"))
        .or_else(|| {
            source
                .as_ref()
                .and_then(|value| source_numeric_field(value, "lifecycle_confidence"))
        })
}

fn source_numeric_field(source: &Value, key: &str) -> Option<u8> {
    source_value_field(&Some(source.clone()), key)
        .and_then(Value::as_u64)
        .and_then(|number| u8::try_from(number).ok())
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
        KeyCode::Char('x') => {
            app.stop_selected_overview_agent();
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

#[cfg(not(unix))]
async fn hub_loop(
    _config: Config,
    tx: mpsc::Sender<HubEvent>,
    mut command_rx: mpsc::Receiver<HubCommand>,
) {
    let _ = tx.send(HubEvent::Disconnected).await;
    while command_rx.recv().await.is_some() {}
}

#[cfg(unix)]
async fn hub_loop(
    config: Config,
    tx: mpsc::Sender<HubEvent>,
    mut command_rx: mpsc::Receiver<HubCommand>,
) {
    let mut backoff = Duration::from_secs(1);
    let mut command_open = true;

    loop {
        let stream = match UnixStream::connect(&config.pulse_socket_path).await {
            Ok(stream) => stream,
            Err(err) => {
                warn!("pulse_connect_error: {err}");
                tokio::time::sleep(backoff).await;
                backoff = next_backoff(backoff);
                continue;
            }
        };
        backoff = Duration::from_secs(1);

        let (reader_half, mut writer_half) = stream.into_split();
        let hello = build_pulse_hello(&config);
        if send_wire_envelope(&mut writer_half, &hello).await.is_err() {
            tokio::time::sleep(backoff).await;
            backoff = next_backoff(backoff);
            continue;
        }
        let subscribe = build_pulse_subscribe(&config);
        if send_wire_envelope(&mut writer_half, &subscribe)
            .await
            .is_err()
        {
            tokio::time::sleep(backoff).await;
            backoff = next_backoff(backoff);
            continue;
        }

        let _ = tx.send(HubEvent::Connected).await;
        let mut reader = BufReader::new(reader_half);
        let mut decoder = NdjsonFrameDecoder::<WireEnvelope>::new(DEFAULT_MAX_FRAME_BYTES);
        let mut read_buf = [0u8; 8192];
        let mut last_seq = 0u64;
        let mut reconnect_requested = false;

        loop {
            tokio::select! {
                read = reader.read(&mut read_buf) => {
                    let read = match read {
                        Ok(value) => value,
                        Err(err) => {
                            warn!("pulse_read_error: {err}");
                            break;
                        }
                    };
                    if read == 0 {
                        break;
                    }
                    let report = decoder.push_chunk(&read_buf[..read]);
                    for err in report.errors {
                        warn!("pulse_decode_error: {err}");
                    }
                    for envelope in report.frames {
                        if envelope.session_id != config.session_id {
                            continue;
                        }
                        if envelope.version.0 > CURRENT_PROTOCOL_VERSION {
                            continue;
                        }
                        let event_at = parse_event_at(&envelope.timestamp);
                        match envelope.msg {
                            WireMsg::Snapshot(payload) => {
                                last_seq = payload.seq;
                                let _ = tx.send(HubEvent::Snapshot { payload, event_at }).await;
                            }
                            WireMsg::Delta(payload) => {
                                if payload.seq <= last_seq {
                                    continue;
                                }
                                if last_seq > 0 && payload.seq > last_seq + 1 {
                                    warn!("pulse_delta_gap: last_seq={last_seq} next_seq={}", payload.seq);
                                    reconnect_requested = true;
                                    break;
                                }
                                last_seq = payload.seq;
                                let _ = tx.send(HubEvent::Delta { payload, event_at }).await;
                            }
                            WireMsg::LayoutState(payload) => {
                                let _ = tx.send(HubEvent::LayoutState { payload }).await;
                            }
                            WireMsg::Heartbeat(payload) => {
                                let _ = tx.send(HubEvent::Heartbeat { payload, event_at }).await;
                            }
                            WireMsg::CommandResult(payload) => {
                                let _ = tx
                                    .send(HubEvent::CommandResult {
                                        payload,
                                        request_id: envelope.request_id,
                                    })
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }
                maybe_command = command_rx.recv(), if command_open => {
                    match maybe_command {
                        Some(command) => {
                            let envelope = build_command_envelope(&config, command);
                            if send_wire_envelope(&mut writer_half, &envelope).await.is_err() {
                                break;
                            }
                        }
                        None => {
                            command_open = false;
                        }
                    }
                }
            }

            if reconnect_requested {
                break;
            }
        }

        let final_report = decoder.finish();
        for err in final_report.errors {
            warn!("pulse_decode_error: {err}");
        }
        let _ = tx.send(HubEvent::Disconnected).await;
        tokio::time::sleep(backoff).await;
        backoff = next_backoff(backoff);
    }
}

#[cfg(unix)]
async fn send_wire_envelope(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    envelope: &WireEnvelope,
) -> io::Result<()> {
    let frame = encode_frame(envelope, DEFAULT_MAX_FRAME_BYTES)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    writer.write_all(&frame).await?;
    writer.flush().await
}

fn build_pulse_hello(config: &Config) -> WireEnvelope {
    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Hello(PulseHelloPayload {
            client_id: config.client_id.clone(),
            role: "subscriber".to_string(),
            capabilities: vec![
                "snapshot".to_string(),
                "delta".to_string(),
                "heartbeat".to_string(),
                "command".to_string(),
                "command_result".to_string(),
                "layout_state".to_string(),
            ],
            agent_id: None,
            pane_id: None,
            project_root: Some(config.project_root.to_string_lossy().to_string()),
        }),
    }
}

fn build_pulse_subscribe(config: &Config) -> WireEnvelope {
    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Subscribe(SubscribePayload {
            topics: vec![
                "agent_state".to_string(),
                "command_result".to_string(),
                "layout_state".to_string(),
            ],
            since_seq: None,
        }),
    }
}

fn build_command_envelope(config: &Config, command: HubCommand) -> WireEnvelope {
    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: Some(command.request_id),
        msg: WireMsg::Command(CommandPayload {
            command: command.command,
            target_agent_id: command.target_agent_id,
            args: command.args,
        }),
    }
}

fn parse_event_at(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

fn collect_local(config: &Config) -> LocalSnapshot {
    let viewer_tab_index = None;
    let mut overview = collect_runtime_overview(config, None);
    if overview.is_empty() {
        overview = collect_proc_overview(config, None);
    }
    let project_roots = collect_project_roots(&overview, &config.project_root);
    let (work, taskmaster_status) = collect_local_work(&project_roots);
    let diff = collect_local_diff(&project_roots);
    let health = collect_health(config, &taskmaster_status);
    LocalSnapshot {
        overview,
        viewer_tab_index,
        work,
        diff,
        health,
    }
}

fn collect_layout_overview(
    config: &Config,
    existing_rows: &[OverviewRow],
    tab_cache: &HashMap<String, TabMeta>,
) -> (Vec<OverviewRow>, Option<usize>) {
    let session_layout = collect_session_layout(&config.session_id);
    let viewer_tab_index = collect_viewer_tab_index(config, session_layout.as_ref());
    if existing_rows.is_empty() {
        return (Vec::new(), viewer_tab_index);
    }
    let Some(layout) = session_layout.as_ref() else {
        let mut rows = existing_rows.to_vec();
        for row in &mut rows {
            apply_cached_tab_meta(row, tab_cache);
        }
        return (sort_overview_rows(rows), viewer_tab_index);
    };

    let mut rows = existing_rows.to_vec();
    for row in &mut rows {
        if let Some(meta) = layout
            .pane_tabs
            .get(&row.pane_id)
            .or_else(|| layout.project_tabs.get(&row.project_root))
        {
            row.tab_index = Some(meta.index);
            row.tab_name = Some(meta.name.clone());
            row.tab_focused = meta.focused;
        } else {
            row.tab_focused = false;
            apply_cached_tab_meta(row, tab_cache);
        }
    }
    (sort_overview_rows(rows), viewer_tab_index)
}

fn collect_viewer_tab_index(
    config: &Config,
    session_layout: Option<&SessionLayout>,
) -> Option<usize> {
    if config.pane_id.trim().is_empty() {
        return None;
    }
    session_layout
        .and_then(|layout| layout.pane_tabs.get(&config.pane_id))
        .map(|meta| meta.index)
}

fn hub_layout_from_payload(payload: &LayoutStatePayload) -> HubLayout {
    let mut pane_tabs = HashMap::new();
    for pane in &payload.panes {
        let Ok(index) = usize::try_from(pane.tab_index) else {
            continue;
        };
        pane_tabs.insert(
            pane.pane_id.clone(),
            TabMeta {
                index,
                name: pane.tab_name.clone(),
                focused: pane.tab_focused,
            },
        );
    }

    let focused_tab_index = payload
        .tabs
        .iter()
        .find(|tab| tab.focused)
        .and_then(|tab| usize::try_from(tab.index).ok())
        .or_else(|| {
            payload
                .panes
                .iter()
                .find(|pane| pane.tab_focused)
                .and_then(|pane| usize::try_from(pane.tab_index).ok())
        });

    HubLayout {
        layout_seq: payload.layout_seq,
        pane_tabs,
        focused_tab_index,
    }
}

fn collect_runtime_overview(
    config: &Config,
    session_layout: Option<&SessionLayout>,
) -> Vec<OverviewRow> {
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
    let active_panes = session_layout.as_ref().and_then(|layout| {
        if layout.pane_ids.is_empty() {
            None
        } else {
            Some(&layout.pane_ids)
        }
    });
    let pane_tabs = session_layout.as_ref().map(|layout| &layout.pane_tabs);
    let project_tabs = session_layout.as_ref().map(|layout| &layout.project_tabs);
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
        if !runtime_process_matches(&snapshot) {
            continue;
        }
        let online = !snapshot.status.eq_ignore_ascii_case("offline");
        let expected_identity = build_identity_key(&snapshot.session_id, &snapshot.pane_id);
        let identity_key = if snapshot.agent_id == expected_identity {
            snapshot.agent_id.clone()
        } else {
            expected_identity
        };
        let tab_meta = pane_tabs
            .and_then(|tabs| tabs.get(&snapshot.pane_id))
            .or_else(|| project_tabs.and_then(|tabs| tabs.get(&snapshot.project_root)));
        rows.insert(
            identity_key.clone(),
            OverviewRow {
                identity_key,
                label: snapshot.agent_label,
                lifecycle: normalize_lifecycle(&snapshot.status),
                snippet: None,
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
    if parsed.pane_ids.is_empty() && parsed.project_tabs.is_empty() {
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
    let mut base_cwd: Option<String> = None;

    for line in layout.lines() {
        if base_cwd.is_none() {
            if let Some(cwd) = extract_layout_attr(line, "cwd") {
                if cwd.starts_with('/') {
                    base_cwd = Some(cwd);
                }
            }
        }

        if line_is_tab_decl(line) {
            current_tab_index += 1;
            current_tab_name = extract_layout_attr(line, "name")
                .unwrap_or_else(|| format!("tab-{current_tab_index}"));
            current_tab_focused = line.contains("focus=true") || line.contains("focus true");
        }

        if current_tab_index > 0
            && (line.contains("name=\"Agent [") || line.contains("name=\"Agent["))
        {
            if let Some(cwd) = extract_layout_attr(line, "cwd") {
                if let Some(project_root) = resolve_layout_cwd(base_cwd.as_deref(), &cwd) {
                    parsed.project_tabs.insert(
                        project_root,
                        TabMeta {
                            index: current_tab_index,
                            name: current_tab_name.clone(),
                            focused: current_tab_focused,
                        },
                    );
                }
            }
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

fn resolve_layout_cwd(base_cwd: Option<&str>, cwd: &str) -> Option<String> {
    if cwd.trim().is_empty() {
        return None;
    }
    let path = PathBuf::from(cwd);
    if path.is_absolute() {
        return Some(path.to_string_lossy().to_string());
    }
    let base = base_cwd?;
    Some(PathBuf::from(base).join(path).to_string_lossy().to_string())
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
    let mut pane_ids = extract_quoted_flag_values(line, "--pane-id");
    pane_ids.extend(extract_attr_values(line, "pane_id"));
    pane_ids.extend(extract_attr_values(line, "pane-id"));
    pane_ids.sort();
    pane_ids.dedup();
    pane_ids
}

fn extract_quoted_flag_values(line: &str, flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() < 3 {
        return out;
    }
    let mut idx = 1usize;
    while idx + 2 < parts.len() {
        if parts[idx].trim() == flag {
            let value = parts[idx + 2].trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
        }
        idx += 2;
    }
    out
}

fn extract_attr_values(line: &str, attr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let marker = format!("{attr}=\"");
    let mut cursor = line;
    while let Some(idx) = cursor.find(&marker) {
        let tail = &cursor[idx + marker.len()..];
        let Some(end_quote) = tail.find('"') else {
            break;
        };
        let value = tail[..end_quote].trim();
        if !value.is_empty() {
            out.push(value.to_string());
        }
        cursor = &tail[end_quote + 1..];
    }
    out
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

fn collect_proc_overview(
    config: &Config,
    session_layout: Option<&SessionLayout>,
) -> Vec<OverviewRow> {
    let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
    let pane_tabs = session_layout.map(|layout| &layout.pane_tabs);
    let project_tabs = session_layout.map(|layout| &layout.project_tabs);
    let active_panes = session_layout.map(|layout| &layout.pane_ids);
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
        if let Some(active_panes) = active_panes {
            if !active_panes.contains(&pane_id) {
                continue;
            }
        }
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
        let tab_meta = pane_tabs
            .and_then(|tabs| tabs.get(&pane_id))
            .or_else(|| project_tabs.and_then(|tabs| tabs.get(&project_root)));
        rows.entry(key.clone()).or_insert(OverviewRow {
            identity_key: key,
            label,
            lifecycle: "running".to_string(),
            snippet: None,
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
    let pane_id = resolve_pane_id();
    let pulse_socket_path = resolve_pulse_socket_path(&session_id);
    let pulse_vnext_enabled = resolve_pulse_vnext_enabled();
    let layout_source = resolve_layout_source();
    let client_id = format!("aoc-pulse-{}", std::process::id());
    let project_root = resolve_project_root();
    let state_dir = resolve_state_dir();
    Config {
        session_id,
        pane_id,
        pulse_socket_path,
        pulse_vnext_enabled,
        layout_source,
        client_id,
        project_root,
        state_dir,
    }
}

fn parse_bool_flag(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn resolve_pulse_vnext_enabled() -> bool {
    std::env::var("AOC_PULSE_VNEXT_ENABLED")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(true)
}

fn resolve_layout_source() -> LayoutSource {
    match std::env::var("AOC_PULSE_LAYOUT_SOURCE") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "local" => LayoutSource::Local,
            "hybrid" => LayoutSource::Hybrid,
            _ => LayoutSource::Hub,
        },
        Err(_) => LayoutSource::Hub,
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

fn resolve_pane_id() -> String {
    if let Ok(value) = std::env::var("AOC_PANE_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = std::env::var("ZELLIJ_PANE_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    String::new()
}

fn resolve_pulse_socket_path(session_id: &str) -> PathBuf {
    if let Ok(value) = std::env::var("AOC_PULSE_SOCK") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    let runtime_dir = if let Ok(value) = std::env::var("XDG_RUNTIME_DIR") {
        if !value.trim().is_empty() {
            PathBuf::from(value)
        } else {
            PathBuf::from("/tmp")
        }
    } else if let Ok(uid) = std::env::var("UID") {
        PathBuf::from(format!("/run/user/{uid}"))
    } else {
        PathBuf::from("/tmp")
    };
    runtime_dir
        .join("aoc")
        .join(session_slug(session_id))
        .join("pulse.sock")
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

fn session_slug(session_id: &str) -> String {
    let mut slug = String::with_capacity(session_id.len());
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            slug.push(ch);
        } else {
            slug.push('-');
        }
    }
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-').to_string();
    let base = if slug.is_empty() {
        "session".to_string()
    } else {
        slug
    };
    let hash = stable_hash_hex(session_id);
    let short = if base.len() > 48 {
        &base[..48]
    } else {
        base.as_str()
    };
    format!("{short}-{hash}")
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in input.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("{hash:08x}")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            session_id: "session-test".to_string(),
            pane_id: "12".to_string(),
            pulse_socket_path: PathBuf::from("/tmp/pulse-test.sock"),
            pulse_vnext_enabled: true,
            layout_source: LayoutSource::Hub,
            client_id: "pulse-test".to_string(),
            project_root: PathBuf::from("/tmp"),
            state_dir: PathBuf::from("/tmp"),
        }
    }

    fn empty_local() -> LocalSnapshot {
        LocalSnapshot {
            overview: Vec::new(),
            viewer_tab_index: None,
            work: Vec::new(),
            diff: Vec::new(),
            health: HealthSnapshot {
                dependencies: Vec::new(),
                checks: Vec::new(),
                taskmaster_status: "unknown".to_string(),
            },
        }
    }

    fn hub_state(agent_id: &str, pane_id: &str, project_root: &str) -> AgentState {
        AgentState {
            agent_id: agent_id.to_string(),
            session_id: "session-test".to_string(),
            pane_id: pane_id.to_string(),
            lifecycle: "running".to_string(),
            snippet: Some("working".to_string()),
            last_heartbeat_ms: Some(1),
            last_activity_ms: Some(1),
            updated_at_ms: Some(1),
            source: Some(serde_json::json!({
                "agent_status": {
                    "agent_label": "OpenCode",
                    "project_root": project_root,
                    "pane_id": pane_id,
                    "status": "running"
                }
            })),
        }
    }

    #[test]
    fn status_payload_prefers_source_metadata() {
        let state = AgentState {
            agent_id: "session-test::12".to_string(),
            session_id: "session-test".to_string(),
            pane_id: "12".to_string(),
            lifecycle: "needs_input".to_string(),
            snippet: Some("awaiting input".to_string()),
            last_heartbeat_ms: Some(1),
            last_activity_ms: Some(1),
            updated_at_ms: Some(1),
            source: Some(serde_json::json!({
                "agent_label": "OpenCode",
                "project_root": "/repo"
            })),
        };

        let payload = status_payload_from_state(&state);
        assert_eq!(payload.status, "needs-input");
        assert_eq!(payload.project_root, "/repo");
        assert_eq!(payload.agent_label.as_deref(), Some("OpenCode"));
        assert_eq!(payload.message.as_deref(), Some("awaiting input"));
    }

    #[test]
    fn command_result_clears_pending_on_terminal_status() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.pending_commands.insert(
            "req-1".to_string(),
            PendingCommand {
                command: "stop_agent".to_string(),
                target: "pane-12".to_string(),
            },
        );

        app.apply_hub_event(HubEvent::CommandResult {
            payload: CommandResultPayload {
                command: "stop_agent".to_string(),
                status: "ok".to_string(),
                message: Some("terminated".to_string()),
                error: None,
            },
            request_id: Some("req-1".to_string()),
        });

        assert!(app.pending_commands.is_empty());
        let note = app.status_note.unwrap_or_default();
        assert!(note.contains("stop_agent"));
        assert!(note.contains("terminated"));
    }

    #[test]
    fn overview_sort_prioritizes_tab_position() {
        let rows = vec![
            OverviewRow {
                identity_key: "session-test::2".to_string(),
                label: "needs-input-pane".to_string(),
                lifecycle: "needs-input".to_string(),
                snippet: None,
                pane_id: "2".to_string(),
                tab_index: Some(2),
                tab_name: Some("Agent".to_string()),
                tab_focused: false,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "hub".to_string(),
            },
            OverviewRow {
                identity_key: "session-test::1".to_string(),
                label: "idle-pane".to_string(),
                lifecycle: "idle".to_string(),
                snippet: None,
                pane_id: "1".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: false,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "hub".to_string(),
            },
        ];

        let sorted = sort_overview_rows(rows);
        assert_eq!(sorted[0].identity_key, "session-test::1");
        assert_eq!(sorted[1].identity_key, "session-test::2");
    }

    #[test]
    fn overview_sort_uses_numeric_pane_id_within_tab() {
        let rows = vec![
            OverviewRow {
                identity_key: "session-test::10".to_string(),
                label: "pane-10".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "10".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: false,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "hub".to_string(),
            },
            OverviewRow {
                identity_key: "session-test::2".to_string(),
                label: "pane-2".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "2".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: false,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "hub".to_string(),
            },
        ];

        let sorted = sort_overview_rows(rows);
        assert_eq!(sorted[0].pane_id, "2");
        assert_eq!(sorted[1].pane_id, "10");
    }

    #[test]
    fn overview_selection_follows_viewer_tab_by_default() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        let rows = vec![
            OverviewRow {
                identity_key: "session-test::1".to_string(),
                label: "pane-1".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "1".to_string(),
                tab_index: Some(1),
                tab_name: Some("tab-1".to_string()),
                tab_focused: false,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "runtime".to_string(),
            },
            OverviewRow {
                identity_key: "session-test::2".to_string(),
                label: "pane-2".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "2".to_string(),
                tab_index: Some(2),
                tab_name: Some("tab-2".to_string()),
                tab_focused: false,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "runtime".to_string(),
            },
        ];

        app.local.viewer_tab_index = Some(2);
        assert_eq!(app.selected_overview_index_for_rows(&rows), 1);

        app.follow_viewer_tab = false;
        app.selected_overview = 0;
        assert_eq!(app.selected_overview_index_for_rows(&rows), 0);
    }

    #[test]
    fn source_confidence_supports_top_level_and_nested_fields() {
        let top = serde_json::json!({"parser_confidence": 3});
        assert_eq!(source_confidence(&Some(top)), Some(3));

        let nested = serde_json::json!({"agent_status": {"lifecycle_confidence": 2}});
        assert_eq!(source_confidence(&Some(nested)), Some(2));
    }

    #[test]
    fn parse_task_summaries_supports_tag_map_counts_shape() {
        let value = serde_json::json!({
            "master": {
                "total": 5,
                "pending": 2,
                "in_progress": 1,
                "blocked": 1,
                "done": 1
            }
        });
        let parsed = parse_task_summaries_from_source(&value, "session-test::12").unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].agent_id, "session-test::12");
        assert_eq!(parsed[0].tag, "master");
        assert_eq!(parsed[0].counts.total, 5);
        assert_eq!(parsed[0].counts.in_progress, 1);
        assert_eq!(parsed[0].counts.blocked, 1);
    }

    #[test]
    fn hub_state_upsert_wires_task_diff_and_health_from_source() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);

        let state = AgentState {
            agent_id: "session-test::12".to_string(),
            session_id: "session-test".to_string(),
            pane_id: "12".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            last_heartbeat_ms: Some(1),
            last_activity_ms: Some(1),
            updated_at_ms: Some(1),
            source: Some(serde_json::json!({
                "agent_status": {
                    "agent_label": "OpenCode",
                    "project_root": "/repo"
                },
                "task_summaries": {
                    "master": {
                        "total": 3,
                        "pending": 1,
                        "in_progress": 1,
                        "blocked": 0,
                        "done": 1
                    }
                },
                "diff_summary": {
                    "repo_root": "/repo",
                    "git_available": true,
                    "summary": {
                        "staged": {"files": 1, "additions": 2, "deletions": 0},
                        "unstaged": {"files": 1, "additions": 1, "deletions": 1},
                        "untracked": {"files": 0}
                    },
                    "files": []
                },
                "health": {
                    "taskmaster_status": "available",
                    "dependencies": [
                        {"name": "git", "available": true, "path": "/usr/bin/git"}
                    ],
                    "checks": [
                        {"name": "test", "status": "ok", "timestamp": "now", "details": "pass"}
                    ]
                }
            })),
        };

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![state],
            },
            event_at: Utc::now(),
        });

        assert_eq!(app.hub.tasks.len(), 1);
        let task = app
            .hub
            .tasks
            .get("session-test::12::master")
            .expect("task payload should exist");
        assert_eq!(task.counts.total, 3);
        assert_eq!(task.counts.in_progress, 1);

        assert_eq!(app.hub.diffs.len(), 1);
        let diff = app
            .hub
            .diffs
            .get("session-test::12")
            .expect("diff payload should exist");
        assert_eq!(diff.repo_root, "/repo");
        assert!(diff.git_available);

        assert_eq!(app.hub.health.len(), 1);
        let health = app
            .hub
            .health
            .get("session-test::12")
            .expect("health payload should exist");
        assert_eq!(health.taskmaster_status, "available");

        app.mode = Mode::Health;
        assert_eq!(app.mode_source(), "hub");
        assert_eq!(app.health_rows().len(), 1);
    }

    #[test]
    fn parse_bool_flag_accepts_rollout_values() {
        assert_eq!(parse_bool_flag("1"), Some(true));
        assert_eq!(parse_bool_flag("on"), Some(true));
        assert_eq!(parse_bool_flag("0"), Some(false));
        assert_eq!(parse_bool_flag("off"), Some(false));
        assert_eq!(parse_bool_flag("maybe"), None);
    }

    #[test]
    fn layout_state_event_updates_local_tab_overlay() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);
        app.set_local(LocalSnapshot {
            overview: vec![OverviewRow {
                identity_key: "session-test::12".to_string(),
                label: "pane-12".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "12".to_string(),
                tab_index: None,
                tab_name: None,
                tab_focused: false,
                project_root: "/tmp".to_string(),
                online: true,
                age_secs: Some(1),
                source: "runtime".to_string(),
            }],
            viewer_tab_index: None,
            work: Vec::new(),
            diff: Vec::new(),
            health: empty_local().health,
        });

        app.apply_hub_event(HubEvent::LayoutState {
            payload: LayoutStatePayload {
                layout_seq: 1,
                session_id: "session-test".to_string(),
                emitted_at_ms: 1,
                tabs: vec![aoc_core::pulse_ipc::LayoutTab {
                    index: 3,
                    name: "Agent".to_string(),
                    focused: true,
                }],
                panes: vec![aoc_core::pulse_ipc::LayoutPane {
                    pane_id: "12".to_string(),
                    tab_index: 3,
                    tab_name: "Agent".to_string(),
                    tab_focused: true,
                }],
            },
        });

        assert_eq!(app.viewer_tab_index_from_hub_layout(), Some(3));
        let rows = app.overview_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tab_index, Some(3));
        assert!(rows[0].tab_focused);
    }

    #[test]
    fn hub_layout_source_disables_local_layout_poll_when_connected() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.config.layout_source = LayoutSource::Hub;
        app.connected = true;
        app.hub.layout = Some(HubLayout {
            layout_seq: 2,
            pane_tabs: HashMap::from([(
                "12".to_string(),
                TabMeta {
                    index: 1,
                    name: "Agent".to_string(),
                    focused: true,
                },
            )]),
            focused_tab_index: Some(1),
        });

        assert!(!app.should_poll_local_layout());
    }

    #[test]
    fn disconnected_hub_uses_cached_rows_during_grace() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.local.overview.push(OverviewRow {
            identity_key: "session-test::99".to_string(),
            label: "local-only".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "99".to_string(),
            tab_index: Some(9),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/tmp".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
        });

        app.apply_hub_event(HubEvent::Connected);
        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![hub_state("session-test::12", "12", "/repo")],
            },
            event_at: Utc::now(),
        });
        app.apply_hub_event(HubEvent::Disconnected);

        assert_eq!(app.mode_source(), "hub");
        assert_eq!(app.hub_status_label(), "reconnecting");
        let rows = app.overview_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].identity_key, "session-test::12");
    }

    #[test]
    fn disconnected_hub_falls_back_after_grace_window() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.local.overview.push(OverviewRow {
            identity_key: "session-test::99".to_string(),
            label: "local-only".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "99".to_string(),
            tab_index: Some(9),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/tmp".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
        });

        app.apply_hub_event(HubEvent::Connected);
        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![hub_state("session-test::12", "12", "/repo")],
            },
            event_at: Utc::now(),
        });
        app.apply_hub_event(HubEvent::Disconnected);
        app.hub_disconnected_at =
            Some(Utc::now() - chrono::Duration::seconds(HUB_RECONNECT_GRACE_SECS + 1));

        assert_eq!(app.mode_source(), "local");
        assert_eq!(app.hub_status_label(), "offline");
        let rows = app.overview_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].identity_key, "session-test::99");
    }

    #[test]
    fn overview_hub_mode_skips_local_only_rows() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.local.overview.push(OverviewRow {
            identity_key: "session-test::99".to_string(),
            label: "local-only".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "99".to_string(),
            tab_index: Some(9),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/tmp".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
        });

        app.apply_hub_event(HubEvent::Connected);
        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![hub_state("session-test::12", "12", "/repo")],
            },
            event_at: Utc::now(),
        });

        let rows = app.overview_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].identity_key, "session-test::12");
    }

    #[test]
    fn overview_reuses_cached_tab_metadata_when_local_row_lacks_it() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());

        app.set_local(LocalSnapshot {
            overview: vec![OverviewRow {
                identity_key: "session-test::11".to_string(),
                label: "OpenCode".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "11".to_string(),
                tab_index: Some(2),
                tab_name: Some("tab-2".to_string()),
                tab_focused: false,
                project_root: "/tmp/project".to_string(),
                online: true,
                age_secs: Some(1),
                source: "runtime".to_string(),
            }],
            viewer_tab_index: Some(2),
            work: Vec::new(),
            diff: Vec::new(),
            health: empty_local().health,
        });

        app.set_local(LocalSnapshot {
            overview: vec![OverviewRow {
                identity_key: "session-test::11".to_string(),
                label: "OpenCode".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "11".to_string(),
                tab_index: None,
                tab_name: None,
                tab_focused: false,
                project_root: "/tmp/project".to_string(),
                online: true,
                age_secs: Some(1),
                source: "runtime".to_string(),
            }],
            viewer_tab_index: Some(2),
            work: Vec::new(),
            diff: Vec::new(),
            health: empty_local().health,
        });

        let rows = app.overview_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tab_index, Some(2));
        assert_eq!(rows[0].tab_name.as_deref(), Some("tab-2"));
    }

    #[test]
    fn prune_hub_cache_skips_local_miss_prune_when_overlap_is_low() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.local.overview.push(OverviewRow {
            identity_key: "session-test::1".to_string(),
            label: "OpenCode".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "1".to_string(),
            tab_index: Some(1),
            tab_name: Some("Agent".to_string()),
            tab_focused: true,
            project_root: "/tmp/project".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
        });

        app.apply_hub_event(HubEvent::Connected);
        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![
                    hub_state("session-test::1", "1", "/tmp/project"),
                    hub_state("session-test::2", "2", "/tmp/project"),
                    hub_state("session-test::3", "3", "/tmp/project"),
                ],
            },
            event_at: Utc::now() - chrono::Duration::seconds(HUB_LOCAL_MISS_PRUNE_SECS + 1),
        });

        app.prune_hub_cache();
        assert_eq!(app.hub.agents.len(), 3);
    }

    #[test]
    fn extract_layout_pane_ids_supports_pane_id_attribute() {
        let line = r#"pane pane_id="44" name="Agent""#;
        let pane_ids = extract_pane_ids_from_layout_line(line);
        assert_eq!(pane_ids, vec!["44".to_string()]);
    }

    #[test]
    fn extract_layout_pane_ids_supports_flag_and_hyphen_attribute() {
        let line = r#"pane command="runner" args "--pane-id" "55" pane-id="77""#;
        let pane_ids = extract_pane_ids_from_layout_line(line);
        assert_eq!(pane_ids, vec!["55".to_string(), "77".to_string()]);
    }

    #[test]
    fn lifecycle_normalization_and_chips_are_stable() {
        assert_eq!(normalize_lifecycle(" needs_input "), "needs-input");
        assert_eq!(lifecycle_chip_label("needs_input", true), "NEEDS");
    }

    #[test]
    fn overview_presenter_compact_keeps_critical_fields() {
        let row = OverviewRow {
            identity_key: "session-test::991122".to_string(),
            label: "very-long-opencode-agent-label".to_string(),
            lifecycle: "blocked".to_string(),
            snippet: Some("waiting on credentials and operator input".to_string()),
            pane_id: "9911223344".to_string(),
            tab_index: None,
            tab_name: None,
            tab_focused: false,
            project_root: "/tmp/some/project/with/long/path".to_string(),
            online: true,
            age_secs: Some(47),
            source: "hub+runtime".to_string(),
        };

        let presenter = overview_row_presenter(&row, true, 80);
        assert!(presenter.identity.contains("::"));
        assert_eq!(presenter.location_chip, "T?:???");
        assert_eq!(presenter.lifecycle_chip, "[BLOCK]");
        assert_eq!(presenter.source_chip, SourceChip::Mixed);
        assert!(presenter.heartbeat.contains("47s"));
        assert!(presenter_text_len(&presenter) <= 72);
    }
}
