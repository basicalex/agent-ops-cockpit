use aoc_core::{
    consultation_contracts::{
        ConsultationCheckpointRef, ConsultationConfidence, ConsultationFreshness,
        ConsultationHelpRequest, ConsultationIdentity, ConsultationPacket, ConsultationPacketKind,
        ConsultationSourceStatus, ConsultationTaskContext,
    },
    insight_contracts::{
        InsightDetachedJob, InsightDetachedJobStatus, InsightDetachedOwnerPlane,
        InsightDetachedStatusResult,
    },
    mind_contracts::{
        canonical_payload_hash, ArtifactTaskLink, ArtifactTaskRelation, SemanticProvenance,
        SemanticRuntime, SemanticStage,
    },
    mind_observer_feed::{
        MindInjectionPayload, MindObserverFeedEvent, MindObserverFeedPayload,
        MindObserverFeedStatus, MindObserverFeedTriggerKind,
    },
    pulse_ipc::{
        encode_frame, AgentState, CommandPayload, CommandResultPayload, ConsultationRequestPayload,
        ConsultationResponsePayload, ConsultationStatus, DeltaPayload,
        HeartbeatPayload as PulseHeartbeatPayload, HelloPayload as PulseHelloPayload,
        LayoutStatePayload, NdjsonFrameDecoder, ObserverTimelinePayload, ProtocolVersion,
        SnapshotPayload, StateChangeOp, SubscribePayload, WireEnvelope, WireMsg,
        CURRENT_PROTOCOL_VERSION, DEFAULT_MAX_FRAME_BYTES,
    },
    session_overseer::{
        AttentionLevel, DriftRisk, ObserverSnapshot, ObserverTimelineEntry, PlanAlignment,
        WorkerSnapshot, WorkerStatus,
    },
    zellij_cli::query_session_snapshot,
    ProjectData, TaskStatus,
};
use aoc_storage::CompactionCheckpoint;
use chrono::{DateTime, TimeZone, Utc};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
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
    env,
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
mod app;
mod collectors;
mod config;
mod consultation_memory;
mod diff;
mod fleet;
mod health;
mod hub;
mod input;
mod mind_artifact_drilldown;
mod mind_glue;
mod mind_host_render;
mod ops;
mod overseer;
mod overview;
mod overview_support;
mod render_host;
mod shared_render;
mod source_parse;
mod theme;
mod wire;
mod work;

use aoc_mind::{
    canon_key, collect_mind_search_hits, detached_job_attention_label,
    detached_job_recovery_guidance, detached_job_status_label, detached_owner_plane_label,
    detached_worker_kind_display, load_mind_artifact_drilldown, mind_event_lane,
    mind_event_sort_ms, mind_lane_label, mind_lane_matches, mind_lane_rollup, mind_progress_label,
    mind_runtime_label, mind_status_label, mind_status_rollup, mind_store_path,
    mind_timestamp_label, mind_trigger_label, MindArtifactDrilldown, MindInjectionRow,
    MindLaneFilter, MindObserverRow,
};
pub(crate) use app::*;
use collectors::*;
use config::*;
pub(crate) use diff::*;
pub(crate) use fleet::*;
pub(crate) use health::*;
use hub::*;
use input::*;
pub(crate) use mind_glue::*;
use ops::*;
pub(crate) use overseer::*;
pub(crate) use overview::*;
pub(crate) use overview_support::*;
pub(crate) use render_host::*;
pub(crate) use shared_render::*;
pub(crate) use source_parse::*;
pub(crate) use theme::*;
use tracing::{debug, info, warn};
use wire::*;
pub(crate) use work::*;

const LOCAL_LAYOUT_REFRESH_MS_DEFAULT: u64 = 3000;
const LOCAL_LAYOUT_REFRESH_MS_MIN: u64 = 500;
const LOCAL_LAYOUT_REFRESH_MS_MAX: u64 = 15000;
const LOCAL_SNAPSHOT_REFRESH_SECS_DEFAULT: u64 = 2;
const LOCAL_SNAPSHOT_REFRESH_SECS_MIN: u64 = 1;
const LOCAL_SNAPSHOT_REFRESH_SECS_MAX: u64 = 30;
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
    tab_scope: Option<String>,
    pulse_socket_path: PathBuf,
    mission_theme: MissionThemeMode,
    mission_custom_theme: Option<MissionTheme>,
    pulse_vnext_enabled: bool,
    overview_enabled: bool,
    start_view: Option<Mode>,
    fleet_plane_filter: FleetPlaneFilter,
    layout_source: LayoutSource,
    client_id: String,
    project_root: PathBuf,
    mind_project_scoped: bool,
    state_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeMode {
    MissionControl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MissionThemeMode {
    Terminal,
    Dark,
    Light,
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
    tab_scope: Option<String>,
    #[serde(default)]
    agent_label: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    session_title: Option<String>,
    #[serde(default)]
    chat_title: Option<String>,
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
    mind: HashMap<String, MindObserverFeedPayload>,
    mind_injection: HashMap<String, MindInjectionPayload>,
    insight_runtime: HashMap<String, InsightRuntimeSnapshot>,
    insight_detached: HashMap<String, InsightDetachedStatusResult>,
    observer_snapshot: Option<ObserverSnapshot>,
    observer_timeline: Vec<ObserverTimelineEntry>,
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
struct PendingConsultation {
    kind: ConsultationPacketKind,
    requester: String,
    responder: String,
    request_packet: ConsultationPacket,
}

fn is_terminal_command_status(status: &str) -> bool {
    !status.eq_ignore_ascii_case("accepted")
}

fn is_terminal_consultation_status(status: ConsultationStatus) -> bool {
    !matches!(status, ConsultationStatus::Accepted)
}

fn orchestrator_tool_id_slug(id: OrchestratorToolId) -> &'static str {
    match id {
        OrchestratorToolId::SessionSnapshot => "session-snapshot",
        OrchestratorToolId::SessionTimeline => "session-timeline",
        OrchestratorToolId::WorkerFocus => "worker-focus",
        OrchestratorToolId::WorkerReview => "worker-review",
        OrchestratorToolId::WorkerHelp => "worker-help",
        OrchestratorToolId::WorkerObserve => "worker-observe",
        OrchestratorToolId::WorkerStop => "worker-stop",
        OrchestratorToolId::WorkerSpawn => "worker-spawn",
        OrchestratorToolId::WorkerDelegate => "worker-delegate",
    }
}

#[derive(Clone, Debug)]
struct HubOutbound {
    request_id: String,
    msg: WireMsg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OrchestratorToolId {
    SessionSnapshot,
    SessionTimeline,
    WorkerFocus,
    WorkerReview,
    WorkerHelp,
    WorkerObserve,
    WorkerStop,
    WorkerSpawn,
    WorkerDelegate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OrchestratorToolStatus {
    Ready,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
struct OrchestratorTool {
    id: OrchestratorToolId,
    label: &'static str,
    scope: &'static str,
    shortcut: Option<&'static str>,
    status: OrchestratorToolStatus,
    summary: String,
    reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OrchestrationGraphNodeKind {
    Session,
    Worker,
    Tool,
    Artifact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OrchestrationGraphEdgeKind {
    Enumerates,
    Selects,
    OperatesOn,
    Launches,
    Writes,
    DelegatesFrom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct OrchestrationGraphNode {
    id: String,
    kind: OrchestrationGraphNodeKind,
    label: String,
    status: String,
    attrs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct OrchestrationGraphEdge {
    from: String,
    to: String,
    kind: OrchestrationGraphEdgeKind,
    summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct OrchestrationCompilePath {
    entry_tool: OrchestratorToolId,
    review_label: String,
    status: OrchestratorToolStatus,
    steps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct OrchestrationGraphIr {
    session_id: String,
    selected_worker_id: Option<String>,
    nodes: Vec<OrchestrationGraphNode>,
    edges: Vec<OrchestrationGraphEdge>,
    compile_paths: Vec<OrchestrationCompilePath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkerLaunchPlan {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    cwd: PathBuf,
    tab_name: String,
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
    session_title: Option<String>,
    chat_title: Option<String>,
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
    tabs: Vec<TabMeta>,
    focused_tab_index: Option<usize>,
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

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct InsightRuntimeSnapshot {
    #[serde(default)]
    reflector_enabled: bool,
    #[serde(default)]
    reflector_ticks: u64,
    #[serde(default)]
    reflector_lock_conflicts: u64,
    #[serde(default)]
    reflector_jobs_completed: u64,
    #[serde(default)]
    reflector_jobs_failed: u64,
    #[serde(default)]
    t3_enabled: bool,
    #[serde(default)]
    t3_ticks: u64,
    #[serde(default)]
    t3_lock_conflicts: u64,
    #[serde(default)]
    t3_jobs_completed: u64,
    #[serde(default)]
    t3_jobs_failed: u64,
    #[serde(default)]
    t3_jobs_requeued: u64,
    #[serde(default)]
    t3_jobs_dead_lettered: u64,
    #[serde(default)]
    t3_queue_depth: i64,
    #[serde(default)]
    supervisor_runs: u64,
    #[serde(default)]
    supervisor_failures: u64,
    #[serde(default)]
    queue_depth: i64,
    #[serde(default)]
    last_tick_ms: Option<i64>,
    #[serde(default)]
    last_error: Option<String>,
}

#[derive(Clone, Debug)]
struct DetachedFleetRow {
    project_root: String,
    owner_plane: InsightDetachedOwnerPlane,
    jobs: Vec<InsightDetachedJob>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FleetPlaneFilter {
    All,
    Delegated,
    Mind,
}

impl FleetPlaneFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Delegated,
            Self::Delegated => Self::Mind,
            Self::Mind => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Delegated => "delegated",
            Self::Mind => "mind",
        }
    }

    fn matches(self, plane: InsightDetachedOwnerPlane) -> bool {
        match self {
            Self::All => true,
            Self::Delegated => matches!(plane, InsightDetachedOwnerPlane::Delegated),
            Self::Mind => matches!(plane, InsightDetachedOwnerPlane::Mind),
        }
    }
}

#[derive(Clone, Debug)]
struct LocalSnapshot {
    overview: Vec<OverviewRow>,
    viewer_tab_index: Option<usize>,
    tab_roster: Vec<TabMeta>,
    work: Vec<WorkProject>,
    diff: Vec<DiffProject>,
    health: HealthSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Overview,
    Overseer,
    Mind,
    Fleet,
    Work,
    Diff,
    Health,
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Mode::Overview => "Overview",
            Mode::Overseer => "Overseer",
            Mode::Mind => "Mind",
            Mode::Fleet => "Fleet",
            Mode::Work => "Work",
            Mode::Diff => "Diff",
            Mode::Health => "Health",
        }
    }

    fn next(self) -> Self {
        match self {
            Mode::Overview => Mode::Overseer,
            Mode::Overseer => Mode::Mind,
            Mode::Mind => Mode::Fleet,
            Mode::Fleet => Mode::Work,
            Mode::Work => Mode::Diff,
            Mode::Diff => Mode::Health,
            Mode::Health => Mode::Overview,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverviewSortMode {
    Layout,
    Attention,
}

impl OverviewSortMode {
    fn toggle(self) -> Self {
        match self {
            Self::Layout => Self::Attention,
            Self::Attention => Self::Layout,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Attention => "attention",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FleetSortMode {
    Project,
    Newest,
    ActiveFirst,
    ErrorFirst,
}

impl FleetSortMode {
    fn next(self) -> Self {
        match self {
            Self::Project => Self::Newest,
            Self::Newest => Self::ActiveFirst,
            Self::ActiveFirst => Self::ErrorFirst,
            Self::ErrorFirst => Self::Project,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Newest => "newest",
            Self::ActiveFirst => "active-first",
            Self::ErrorFirst => "error-first",
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
    ObserverSnapshot {
        payload: ObserverSnapshot,
    },
    ObserverTimeline {
        payload: ObserverTimelinePayload,
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
    ConsultationResponse {
        payload: ConsultationResponsePayload,
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
    command_tx: mpsc::Sender<HubOutbound>,
    connected: bool,
    hub_disconnected_at: Option<DateTime<Utc>>,
    hub: HubCache,
    local: LocalSnapshot,
    tab_cache: HashMap<String, TabMeta>,
    mode: Mode,
    scroll: u16,
    help_open: bool,
    selected_overview: usize,
    selected_fleet: usize,
    selected_fleet_job: usize,
    overview_sort_mode: OverviewSortMode,
    fleet_sort_mode: FleetSortMode,
    fleet_plane_filter: FleetPlaneFilter,
    fleet_active_only: bool,
    follow_viewer_tab: bool,
    last_viewer_tab_index: Option<usize>,
    mind_lane: MindLaneFilter,
    mind_show_all_tabs: bool,
    mind_show_provenance: bool,
    mind_search_query: String,
    mind_search_editing: bool,
    mind_search_selected: usize,
    status_note: Option<String>,
    pending_commands: HashMap<String, PendingCommand>,
    pending_consultations: HashMap<String, PendingConsultation>,
    next_request_id: u64,
    pending_render_latency: Vec<PendingRenderLatency>,
    parser_confidence: HashMap<String, u8>,
    latency_sample_count: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = load_config();
    init_logging();
    let local_layout_refresh_ms = resolve_local_layout_refresh_ms();

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
        Duration::from_millis(local_layout_refresh_ms),
    );
    let mut snapshot_ticker = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_millis(jitter_seed * 240),
        Duration::from_secs(resolve_local_snapshot_refresh_secs()),
    );
    let mut layout_refresh_requested = false;
    let mut snapshot_refresh_requested = false;

    loop {
        if snapshot_refresh_requested {
            let local_snapshot = app.collect_local_snapshot();
            app.set_local(local_snapshot);
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
                if app.config.overview_enabled {
                    snapshot_refresh_requested = true;
                }
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

#[cfg(test)]
mod tests;
