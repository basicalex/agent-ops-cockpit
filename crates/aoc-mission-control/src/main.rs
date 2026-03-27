use aoc_core::{
    consultation_contracts::{
        ConsultationCheckpointRef, ConsultationConfidence, ConsultationFreshness,
        ConsultationHelpRequest, ConsultationIdentity, ConsultationPacket, ConsultationPacketKind,
        ConsultationSourceStatus, ConsultationTaskContext,
    },
    insight_contracts::{
        InsightDetachedJob, InsightDetachedJobStatus, InsightDetachedOwnerPlane,
        InsightDetachedStatusResult, InsightDetachedWorkerKind,
    },
    mind_contracts::{
        canonical_payload_hash, ArtifactTaskLink, ArtifactTaskRelation, SemanticProvenance,
        SemanticRuntime, SemanticStage,
    },
    mind_observer_feed::{
        MindInjectionPayload, MindObserverFeedEvent, MindObserverFeedPayload,
        MindObserverFeedProgress, MindObserverFeedStatus, MindObserverFeedTriggerKind,
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
use aoc_storage::{CanonRevisionState, CompactionCheckpoint, StoredCompactionT0Slice};
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
mod pulse_tabs;

use pulse_tabs::render_pulse_tab_section;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

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
    pulse_theme: PulseThemeMode,
    pulse_custom_theme: Option<PulseTheme>,
    pulse_vnext_enabled: bool,
    overview_enabled: bool,
    runtime_mode: RuntimeMode,
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
    PulsePane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PulseThemeMode {
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

#[derive(Deserialize, Clone, Debug, Default)]
struct MindSessionExportManifest {
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    active_tag: Option<String>,
    #[serde(default)]
    export_dir: String,
    #[serde(default)]
    t1_count: usize,
    #[serde(default)]
    t2_count: usize,
    #[serde(default)]
    t3_job_id: String,
    #[serde(default)]
    exported_at: String,
}

#[derive(Clone, Debug, Default)]
struct MindHandshakeEntry {
    entry_id: String,
    revision: u32,
    topic: Option<String>,
    summary: String,
}

#[derive(Clone, Debug, Default)]
struct MindCanonEntry {
    entry_id: String,
    revision: u32,
    topic: Option<String>,
    evidence_refs: Vec<String>,
    summary: String,
}

#[derive(Clone, Debug, Default)]
struct MindArtifactDrilldown {
    latest_export: Option<MindSessionExportManifest>,
    latest_compaction_checkpoint: Option<CompactionCheckpoint>,
    latest_compaction_slice: Option<StoredCompactionT0Slice>,
    compaction_marker_event_available: bool,
    compaction_rebuildable: bool,
    handshake_entries: Vec<MindHandshakeEntry>,
    active_canon_entries: Vec<MindCanonEntry>,
    stale_canon_count: usize,
}

#[derive(Clone, Debug)]
struct MindSearchHit {
    kind: &'static str,
    title: String,
    summary: String,
    score: usize,
}

#[derive(Clone, Debug)]
struct MindObserverRow {
    agent_id: String,
    scope: String,
    pane_id: String,
    tab_scope: Option<String>,
    tab_focused: bool,
    event: MindObserverFeedEvent,
    source: String,
}

#[derive(Clone, Debug)]
struct MindInjectionRow {
    scope: String,
    pane_id: String,
    tab_focused: bool,
    payload: MindInjectionPayload,
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
    PulsePane,
    Overview,
    Overseer,
    Mind,
    Fleet,
    Work,
    Diff,
    Health,
}

impl RuntimeMode {
    fn is_pulse_pane(self) -> bool {
        matches!(self, RuntimeMode::PulsePane)
    }
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Mode::PulsePane => "AOC Pulse",
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
            Mode::PulsePane => Mode::PulsePane,
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
enum MindLaneFilter {
    T0,
    T1,
    T2,
    T3,
    All,
}

impl MindLaneFilter {
    fn next(self) -> Self {
        match self {
            Self::T0 => Self::T1,
            Self::T1 => Self::T2,
            Self::T2 => Self::T3,
            Self::T3 => Self::All,
            Self::All => Self::T0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::T0 => "t0",
            Self::T1 => "t1",
            Self::T2 => "t2",
            Self::T3 => "t3",
            Self::All => "all",
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
    status_note: Option<String>,
    pending_commands: HashMap<String, PendingCommand>,
    pending_consultations: HashMap<String, PendingConsultation>,
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

fn normalized_project_root_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut normalized = trimmed.replace('\\', "/");
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

impl App {
    fn new(config: Config, command_tx: mpsc::Sender<HubOutbound>, local: LocalSnapshot) -> Self {
        let tab_cache = seed_tab_cache(&local.overview);
        let last_viewer_tab_index = local.viewer_tab_index;
        let default_mode = if config.runtime_mode.is_pulse_pane() {
            Mode::PulsePane
        } else if config.overview_enabled {
            Mode::Overview
        } else {
            Mode::Overseer
        };
        let mode = if config.runtime_mode.is_pulse_pane() {
            default_mode
        } else {
            config.start_view.unwrap_or(default_mode)
        };
        let status_note = if config.runtime_mode.is_pulse_pane() {
            Some("pulse pane mode: local Tabs + Mind/status".to_string())
        } else if config.overview_enabled {
            None
        } else {
            Some("overview disabled; using Overseer/Mind/Work/Diff/Health".to_string())
        };
        let fleet_plane_filter = config.fleet_plane_filter;
        Self {
            config,
            command_tx,
            connected: false,
            hub_disconnected_at: None,
            hub: HubCache::default(),
            local,
            tab_cache,
            mode,
            scroll: 0,
            help_open: false,
            selected_overview: 0,
            selected_fleet: 0,
            selected_fleet_job: 0,
            overview_sort_mode: OverviewSortMode::Layout,
            fleet_sort_mode: FleetSortMode::Project,
            fleet_plane_filter,
            fleet_active_only: false,
            follow_viewer_tab: true,
            last_viewer_tab_index,
            mind_lane: MindLaneFilter::T1,
            mind_show_all_tabs: false,
            mind_show_provenance: false,
            mind_search_query: String::new(),
            mind_search_editing: false,
            status_note,
            pending_commands: HashMap::new(),
            pending_consultations: HashMap::new(),
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
                self.pending_consultations.clear();
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
                self.hub.mind.clear();
                self.hub.mind_injection.clear();
                self.hub.insight_runtime.clear();
                self.hub.insight_detached.clear();
                self.hub.observer_snapshot = None;
                self.hub.observer_timeline.clear();
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
                    self.hub.mind.clear();
                    self.hub.mind_injection.clear();
                    self.hub.insight_runtime.clear();
                    self.hub.insight_detached.clear();
                    self.hub.observer_snapshot = None;
                    self.hub.observer_timeline.clear();
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
                            self.hub.mind.remove(&change.agent_id);
                            self.hub.mind_injection.remove(&change.agent_id);
                            self.hub.insight_runtime.remove(&change.agent_id);
                            self.hub.insight_detached.remove(&change.agent_id);
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
            HubEvent::ObserverSnapshot { payload } => {
                if self.config.runtime_mode.is_pulse_pane()
                    || payload.session_id != self.config.session_id
                {
                    return;
                }
                self.hub.observer_snapshot = Some(payload);
            }
            HubEvent::ObserverTimeline { payload } => {
                if self.config.runtime_mode.is_pulse_pane()
                    || payload.session_id != self.config.session_id
                {
                    return;
                }
                self.hub.observer_timeline = payload.entries;
            }
            HubEvent::LayoutState { payload } => {
                if !self.config.overview_enabled && !self.config.runtime_mode.is_pulse_pane() {
                    return;
                }
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
                            tab_scope: None,
                            agent_label: Some(extract_label(&payload.agent_id)),
                            message: None,
                            session_title: None,
                            chat_title: None,
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
            HubEvent::ConsultationResponse {
                payload,
                request_id,
            } => {
                if self.config.runtime_mode.is_pulse_pane() {
                    return;
                }
                self.apply_consultation_response(payload, request_id);
            }
        }
    }

    fn latest_compaction_checkpoint(&self) -> Option<CompactionCheckpoint> {
        load_mind_artifact_drilldown(
            Path::new(&self.config.project_root),
            &self.config.session_id,
        )
        .latest_compaction_checkpoint
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

        if let Some(source_value) = source_value_by_keys(&state.source, &["mind_observer"]) {
            match parse_mind_observer_from_source(source_value) {
                Ok(Some(feed)) => {
                    self.hub.mind.insert(state.agent_id.clone(), feed);
                }
                Ok(None) => {
                    self.hub.mind.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "mind_observer",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.mind.remove(&state.agent_id);
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["mind_injection"]) {
            match parse_mind_injection_from_source(source_value) {
                Ok(Some(payload)) => {
                    self.hub
                        .mind_injection
                        .insert(state.agent_id.clone(), payload);
                }
                Ok(None) => {
                    self.hub.mind_injection.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "mind_injection",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.mind_injection.remove(&state.agent_id);
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["insight_runtime"]) {
            match parse_insight_runtime_from_source(source_value) {
                Ok(Some(snapshot)) => {
                    self.hub
                        .insight_runtime
                        .insert(state.agent_id.clone(), snapshot);
                }
                Ok(None) => {
                    self.hub.insight_runtime.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "insight_runtime",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.insight_runtime.remove(&state.agent_id);
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["insight_detached"]) {
            match parse_insight_detached_from_source(source_value) {
                Ok(Some(snapshot)) => {
                    self.hub
                        .insight_detached
                        .insert(state.agent_id.clone(), snapshot);
                }
                Ok(None) => {
                    self.hub.insight_detached.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "insight_detached",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.insight_detached.remove(&state.agent_id);
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
        if request_id.is_some() && tracked.is_none() {
            debug!(
                event = "pulse_command_result_ignored",
                reason = "stale_request_id",
                request_id = request_id.as_deref().unwrap_or_default(),
                command = %payload.command,
                status = %payload.status
            );
            return;
        }

        let done = is_terminal_command_status(&payload.status);
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

    fn apply_consultation_response(
        &mut self,
        payload: ConsultationResponsePayload,
        request_id: Option<String>,
    ) {
        let tracked = request_id
            .as_deref()
            .and_then(|id| self.pending_consultations.get(id).cloned());
        if request_id.is_some() && tracked.is_none() {
            debug!(
                event = "pulse_consultation_result_ignored",
                reason = "stale_request_id",
                request_id = request_id.as_deref().unwrap_or_default(),
                consultation_id = %payload.consultation_id,
                status = ?payload.status
            );
            return;
        }

        if is_terminal_consultation_status(payload.status) {
            if let Some(id) = request_id.as_deref() {
                self.pending_consultations.remove(id);
            }
        }

        let (requester, responder, kind, request_packet) = tracked
            .map(|value| {
                (
                    value.requester,
                    value.responder,
                    value.kind,
                    Some(value.request_packet),
                )
            })
            .unwrap_or_else(|| {
                (
                    payload.requesting_agent_id.clone(),
                    payload.responding_agent_id.clone(),
                    ConsultationPacketKind::Summary,
                    None,
                )
            });
        if let Some(request_packet) = request_packet.as_ref() {
            if let Err(err) = persist_consultation_outcome(
                &self.config.project_root,
                request_packet,
                &payload,
                kind,
            ) {
                warn!(
                    event = "consultation_outcome_persist_failed",
                    consultation_id = %payload.consultation_id,
                    error = %err
                );
            }
        }
        let mut message = payload
            .message
            .clone()
            .unwrap_or_else(|| format!("{:?}", payload.status).to_ascii_lowercase());
        if let Some(error) = payload.error.as_ref() {
            message = format!("{} ({})", error.message, error.code);
        }
        self.status_note = Some(format!(
            "consult {:?} {} -> {} · {}",
            kind,
            requester,
            responder,
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
        let outbound = HubOutbound {
            request_id: request_id.clone(),
            msg: WireMsg::Command(CommandPayload {
                command: command.to_string(),
                target_agent_id,
                args,
            }),
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
            tab_roster,
            work,
            diff,
            health,
        } = local;
        self.set_local_overview(overview, viewer_tab_index);
        self.local.tab_roster = tab_roster;
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
            LayoutSource::Hybrid => !self.connected,
            LayoutSource::Hub => false,
        }
    }

    fn refresh_local_layout(&mut self) {
        let (overview, viewer_tab_index, tab_roster) =
            collect_layout_overview(&self.config, &self.local.overview, &self.tab_cache);
        self.set_local_overview(overview, viewer_tab_index);
        self.local.tab_roster = tab_roster;
    }

    fn collect_local_snapshot(&self) -> LocalSnapshot {
        collect_local_with_options(
            &self.config,
            !self.prefer_hub_data(!self.hub.tasks.is_empty()),
            !self.prefer_hub_data(!self.hub.diffs.is_empty()),
            !self.prefer_hub_data(!self.hub.health.is_empty()),
            Some(&self.local),
        )
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
        self.hub
            .mind
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .mind_injection
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .insight_runtime
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .insight_detached
            .retain(|agent_id, _| active_agents.contains(agent_id));
        if let Some(snapshot) = self.hub.observer_snapshot.as_mut() {
            snapshot
                .workers
                .retain(|worker| active_agents.contains(&worker.agent_id));
            snapshot
                .timeline
                .retain(|entry| active_agents.contains(&entry.agent_id));
            if snapshot.workers.is_empty() && snapshot.timeline.is_empty() {
                self.hub.observer_snapshot = None;
            }
        }
        self.hub
            .observer_timeline
            .retain(|entry| active_agents.contains(&entry.agent_id));
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
            Mode::PulsePane => {
                if self.prefer_hub_data(
                    !self.hub.agents.is_empty()
                        || !self.hub.tasks.is_empty()
                        || !self.hub.diffs.is_empty()
                        || !self.hub.health.is_empty()
                        || !self.hub.mind.is_empty(),
                ) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Overview => {
                if self.prefer_hub_data(!self.hub.agents.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Overseer => {
                if self.prefer_hub_data(
                    self.hub
                        .observer_snapshot
                        .as_ref()
                        .map(|snapshot| {
                            !snapshot.workers.is_empty() || !snapshot.timeline.is_empty()
                        })
                        .unwrap_or(false)
                        || !self.hub.observer_timeline.is_empty(),
                ) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Mind => {
                if self.prefer_hub_data(
                    !self.hub.mind.is_empty()
                        || !self.hub.insight_runtime.is_empty()
                        || !self.hub.insight_detached.is_empty(),
                ) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Fleet => {
                if self.prefer_hub_data(!self.hub.insight_detached.is_empty()) {
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

    fn has_any_hub_data(&self) -> bool {
        !self.hub.agents.is_empty()
            || !self.hub.tasks.is_empty()
            || !self.hub.diffs.is_empty()
            || !self.hub.health.is_empty()
            || !self.hub.mind.is_empty()
            || !self.hub.insight_runtime.is_empty()
            || !self.hub.insight_detached.is_empty()
            || self.hub.observer_snapshot.is_some()
            || !self.hub.observer_timeline.is_empty()
            || self.hub.layout.is_some()
    }

    #[allow(dead_code)]
    fn hub_status_label(&self) -> &'static str {
        if self.connected {
            "online"
        } else if self.hub_reconnect_grace_active() && self.has_any_hub_data() {
            "reconnecting"
        } else {
            "offline"
        }
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
        let viewer_scope = self.config.tab_scope.as_deref();
        if self.prefer_hub_data(!self.hub.agents.is_empty()) {
            let now = Utc::now();
            let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
            for (agent_id, agent) in &self.hub.agents {
                let status = agent.status.as_ref();
                let row_tab_scope = status.and_then(|s| s.tab_scope.as_deref());
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
                    tab_name: status.and_then(|s| s.tab_scope.clone()),
                    tab_focused: tab_scope_matches(viewer_scope, row_tab_scope),
                    project_root,
                    online,
                    age_secs,
                    source: "hub".to_string(),
                    session_title: status.and_then(|s| s.session_title.clone()),
                    chat_title: status.and_then(|s| s.chat_title.clone()),
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
                    if local.online {
                        existing.online = true;
                        existing.age_secs = match (existing.age_secs, local.age_secs) {
                            (Some(left), Some(right)) => Some(left.min(right)),
                            (None, Some(right)) => Some(right),
                            (left, None) => left,
                        };
                        if existing.lifecycle == "offline" {
                            existing.lifecycle = local.lifecycle.clone();
                        }
                    }
                    if existing.session_title.is_none() {
                        existing.session_title = local.session_title.clone();
                    }
                    if existing.chat_title.is_none() {
                        existing.chat_title = local.chat_title.clone();
                    }
                } else {
                    let mut local_row = local.clone();
                    if local_row.source == "runtime" || local_row.source == "proc" {
                        local_row.source = "loc".to_string();
                    }
                    rows.insert(local_row.identity_key.clone(), local_row);
                }
            }

            let mut merged_rows: Vec<OverviewRow> = rows.into_values().collect();
            for row in &mut merged_rows {
                if let Some(meta) = self
                    .active_hub_layout()
                    .and_then(|layout| layout.pane_tabs.get(&row.pane_id))
                {
                    if row.tab_index.is_none() {
                        row.tab_index = Some(meta.index);
                    }
                    if row.tab_name.is_none() {
                        row.tab_name = Some(meta.name.clone());
                    }
                }
                apply_cached_tab_meta(row, &self.tab_cache);
                if !row.tab_focused {
                    row.tab_focused = tab_scope_matches(viewer_scope, row.tab_name.as_deref());
                }
            }
            return self.sort_overview_rows_for_mode(merged_rows);
        }
        let mut local_rows = self.local.overview.clone();
        for row in &mut local_rows {
            if let Some(meta) = self
                .active_hub_layout()
                .and_then(|layout| layout.pane_tabs.get(&row.pane_id))
            {
                if row.tab_index.is_none() {
                    row.tab_index = Some(meta.index);
                }
                if row.tab_name.is_none() {
                    row.tab_name = Some(meta.name.clone());
                }
            }
            apply_cached_tab_meta(row, &self.tab_cache);
            if !row.tab_focused {
                row.tab_focused = tab_scope_matches(viewer_scope, row.tab_name.as_deref());
            }
        }
        self.sort_overview_rows_for_mode(local_rows)
    }

    fn sort_overview_rows_for_mode(&self, rows: Vec<OverviewRow>) -> Vec<OverviewRow> {
        match self.overview_sort_mode {
            OverviewSortMode::Layout => sort_overview_rows(rows),
            OverviewSortMode::Attention => sort_overview_rows_attention(rows),
        }
    }

    fn toggle_overview_sort_mode(&mut self) {
        self.overview_sort_mode = self.overview_sort_mode.toggle();
        self.follow_viewer_tab = true;
        self.selected_overview = 0;
        self.status_note = Some(format!(
            "overview sort: {}",
            self.overview_sort_mode.label()
        ));
    }

    fn cycle_mode(&mut self) {
        if self.config.runtime_mode.is_pulse_pane() {
            self.mode = Mode::PulsePane;
            return;
        }
        self.mode = if self.config.overview_enabled {
            self.mode.next()
        } else {
            match self.mode {
                Mode::PulsePane | Mode::Overview => Mode::Overseer,
                Mode::Overseer => Mode::Mind,
                Mode::Mind => Mode::Fleet,
                Mode::Fleet => Mode::Work,
                Mode::Work => Mode::Diff,
                Mode::Diff => Mode::Health,
                Mode::Health => Mode::Overseer,
            }
        };
    }

    fn overview_context_hint(&self, row: &OverviewRow) -> String {
        if let Some(snippet) = row
            .snippet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return snippet.to_string();
        }

        let mut active_titles = self
            .hub
            .tasks
            .values()
            .filter(|payload| payload.agent_id == row.identity_key)
            .flat_map(|payload| payload.active_tasks.clone().unwrap_or_default().into_iter())
            .filter(|task| {
                task.status == "in-progress" || task.status == "in_progress" || task.active_agent
            })
            .map(|task| task.title)
            .collect::<Vec<_>>();
        active_titles.sort();
        if let Some(title) = active_titles.into_iter().next() {
            return title;
        }

        match normalize_lifecycle(&row.lifecycle).as_str() {
            "needs-input" => "awaiting input".to_string(),
            "blocked" => "blocked".to_string(),
            "busy" => "working".to_string(),
            "idle" => "idle".to_string(),
            "error" => "error reported".to_string(),
            _ => "running".to_string(),
        }
    }

    fn overview_task_signal(&self, row: &OverviewRow) -> Option<String> {
        let mut total = 0u32;
        let mut in_progress = 0u32;
        for payload in self
            .hub
            .tasks
            .values()
            .filter(|payload| payload.agent_id == row.identity_key)
        {
            total = total.saturating_add(payload.counts.total);
            in_progress = in_progress.saturating_add(payload.counts.in_progress);
        }
        if total == 0 {
            return None;
        }
        Some(format!("W:{in_progress}/{total}"))
    }

    fn overview_git_signal(&self, row: &OverviewRow) -> Option<String> {
        let diff = self.hub.diffs.get(&row.identity_key)?;
        if !diff.git_available {
            return Some("G:n/a".to_string());
        }
        let additions = diff
            .summary
            .staged
            .additions
            .saturating_add(diff.summary.unstaged.additions);
        let deletions = diff
            .summary
            .staged
            .deletions
            .saturating_add(diff.summary.unstaged.deletions);
        let untracked = diff.summary.untracked.files;
        if additions == 0 && deletions == 0 && untracked == 0 {
            return None;
        }
        Some(if untracked > 0 {
            format!("G:+{additions}/-{deletions} ?{untracked}")
        } else {
            format!("G:+{additions}/-{deletions}")
        })
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

    fn overseer_snapshot(&self) -> Option<&ObserverSnapshot> {
        self.hub.observer_snapshot.as_ref().filter(|snapshot| {
            self.prefer_hub_data(!snapshot.workers.is_empty() || !snapshot.timeline.is_empty())
        })
    }

    fn overseer_workers(&self) -> Vec<WorkerSnapshot> {
        let Some(snapshot) = self.overseer_snapshot() else {
            return Vec::new();
        };
        let mut workers = snapshot.workers.clone();
        workers.sort_by(|left, right| {
            overseer_attention_rank(&left.attention.level)
                .cmp(&overseer_attention_rank(&right.attention.level))
                .reverse()
                .then_with(|| {
                    overseer_drift_rank(&left.drift_risk)
                        .cmp(&overseer_drift_rank(&right.drift_risk))
                        .reverse()
                })
                .then_with(|| left.agent_id.cmp(&right.agent_id))
        });
        workers
    }

    fn overseer_timeline(&self) -> Vec<ObserverTimelineEntry> {
        let mut entries = if !self.hub.observer_timeline.is_empty() {
            self.hub.observer_timeline.clone()
        } else {
            self.overseer_snapshot()
                .map(|snapshot| snapshot.timeline.clone())
                .unwrap_or_default()
        };
        entries.sort_by(|left, right| {
            right
                .emitted_at_ms
                .unwrap_or_default()
                .cmp(&left.emitted_at_ms.unwrap_or_default())
                .then_with(|| left.agent_id.cmp(&right.agent_id))
        });
        entries
    }

    fn overseer_mind_event(&self, agent_id: &str) -> Option<&MindObserverFeedEvent> {
        let feed = self.hub.mind.get(agent_id)?;
        feed.events.iter().max_by_key(|event| {
            mind_event_sort_ms(event.completed_at.as_deref())
                .or_else(|| mind_event_sort_ms(event.started_at.as_deref()))
                .or_else(|| mind_event_sort_ms(event.enqueued_at.as_deref()))
                .unwrap_or(0)
        })
    }

    fn insight_runtime_rollup(&self) -> Option<InsightRuntimeSnapshot> {
        if !self.prefer_hub_data(!self.hub.insight_runtime.is_empty()) {
            return None;
        }
        let mut agg = InsightRuntimeSnapshot::default();
        for snapshot in self.hub.insight_runtime.values() {
            agg.reflector_enabled = agg.reflector_enabled || snapshot.reflector_enabled;
            agg.reflector_ticks = agg.reflector_ticks.saturating_add(snapshot.reflector_ticks);
            agg.reflector_lock_conflicts = agg
                .reflector_lock_conflicts
                .saturating_add(snapshot.reflector_lock_conflicts);
            agg.reflector_jobs_completed = agg
                .reflector_jobs_completed
                .saturating_add(snapshot.reflector_jobs_completed);
            agg.reflector_jobs_failed = agg
                .reflector_jobs_failed
                .saturating_add(snapshot.reflector_jobs_failed);
            agg.t3_enabled = agg.t3_enabled || snapshot.t3_enabled;
            agg.t3_ticks = agg.t3_ticks.saturating_add(snapshot.t3_ticks);
            agg.t3_lock_conflicts = agg
                .t3_lock_conflicts
                .saturating_add(snapshot.t3_lock_conflicts);
            agg.t3_jobs_completed = agg
                .t3_jobs_completed
                .saturating_add(snapshot.t3_jobs_completed);
            agg.t3_jobs_failed = agg.t3_jobs_failed.saturating_add(snapshot.t3_jobs_failed);
            agg.t3_jobs_requeued = agg
                .t3_jobs_requeued
                .saturating_add(snapshot.t3_jobs_requeued);
            agg.t3_jobs_dead_lettered = agg
                .t3_jobs_dead_lettered
                .saturating_add(snapshot.t3_jobs_dead_lettered);
            agg.t3_queue_depth = agg
                .t3_queue_depth
                .saturating_add(snapshot.t3_queue_depth.max(0));
            agg.supervisor_runs = agg.supervisor_runs.saturating_add(snapshot.supervisor_runs);
            agg.supervisor_failures = agg
                .supervisor_failures
                .saturating_add(snapshot.supervisor_failures);
            agg.queue_depth = agg.queue_depth.saturating_add(snapshot.queue_depth.max(0));
            if agg.last_tick_ms.is_none() || snapshot.last_tick_ms > agg.last_tick_ms {
                agg.last_tick_ms = snapshot.last_tick_ms;
            }
            if agg.last_error.is_none() {
                agg.last_error = snapshot.last_error.clone();
            }
        }
        Some(agg)
    }

    fn insight_detached_jobs(&self) -> Vec<InsightDetachedJob> {
        if !self.prefer_hub_data(!self.hub.insight_detached.is_empty()) {
            return Vec::new();
        }
        let mut jobs = self
            .hub
            .insight_detached
            .iter()
            .filter(|(agent_id, _)| {
                if !self.config.mind_project_scoped {
                    return true;
                }
                self.hub
                    .agents
                    .get(*agent_id)
                    .and_then(|agent| agent.status.as_ref())
                    .map(|status| self.mind_project_matches(&status.project_root))
                    .unwrap_or(false)
            })
            .flat_map(|(_, snapshot)| snapshot.jobs.clone())
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| {
            right
                .created_at_ms
                .cmp(&left.created_at_ms)
                .then_with(|| left.job_id.cmp(&right.job_id))
        });
        jobs
    }

    fn detached_fleet_rows(&self) -> Vec<DetachedFleetRow> {
        if !self.prefer_hub_data(!self.hub.insight_detached.is_empty()) {
            return Vec::new();
        }
        let mut grouped: BTreeMap<(String, u8), Vec<InsightDetachedJob>> = BTreeMap::new();
        for (agent_id, snapshot) in &self.hub.insight_detached {
            let project_root = self
                .hub
                .agents
                .get(agent_id)
                .and_then(|agent| {
                    agent
                        .status
                        .as_ref()
                        .map(|status| status.project_root.clone())
                })
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "(unknown project)".to_string());
            for job in &snapshot.jobs {
                let plane_rank = match job.owner_plane {
                    InsightDetachedOwnerPlane::Delegated => 0,
                    InsightDetachedOwnerPlane::Mind => 1,
                };
                grouped
                    .entry((project_root.clone(), plane_rank))
                    .or_default()
                    .push(job.clone());
            }
        }

        let mut rows = grouped
            .into_iter()
            .map(|((project_root, _), mut jobs)| {
                jobs.sort_by(|left, right| {
                    right
                        .created_at_ms
                        .cmp(&left.created_at_ms)
                        .then_with(|| left.job_id.cmp(&right.job_id))
                });
                DetachedFleetRow {
                    project_root,
                    owner_plane: jobs
                        .first()
                        .map(|job| job.owner_plane)
                        .unwrap_or(InsightDetachedOwnerPlane::Delegated),
                    jobs,
                }
            })
            .filter(|row| self.fleet_plane_filter.matches(row.owner_plane))
            .filter(|row| {
                !self.fleet_active_only
                    || row.jobs.iter().any(|job| {
                        matches!(
                            job.status,
                            InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
                        )
                    })
            })
            .collect::<Vec<_>>();

        let row_rank = |row: &DetachedFleetRow| -> (usize, usize, usize) {
            let mut active = 0usize;
            let mut errorish = 0usize;
            let mut latest_created = 0i64;
            for job in &row.jobs {
                latest_created = latest_created.max(job.created_at_ms);
                match job.status {
                    InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running => {
                        active += 1
                    }
                    InsightDetachedJobStatus::Error
                    | InsightDetachedJobStatus::Fallback
                    | InsightDetachedJobStatus::Stale => errorish += 1,
                    InsightDetachedJobStatus::Success | InsightDetachedJobStatus::Cancelled => {}
                }
            }
            (active, errorish, latest_created as usize)
        };

        rows.sort_by(|left, right| {
            let left_rank = row_rank(left);
            let right_rank = row_rank(right);
            match self.fleet_sort_mode {
                FleetSortMode::Project => {
                    left.project_root.cmp(&right.project_root).then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    })
                }
                FleetSortMode::Newest => right_rank
                    .2
                    .cmp(&left_rank.2)
                    .then_with(|| left.project_root.cmp(&right.project_root))
                    .then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    }),
                FleetSortMode::ActiveFirst => right_rank
                    .0
                    .cmp(&left_rank.0)
                    .then_with(|| right_rank.2.cmp(&left_rank.2))
                    .then_with(|| left.project_root.cmp(&right.project_root))
                    .then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    }),
                FleetSortMode::ErrorFirst => right_rank
                    .1
                    .cmp(&left_rank.1)
                    .then_with(|| right_rank.0.cmp(&left_rank.0))
                    .then_with(|| right_rank.2.cmp(&left_rank.2))
                    .then_with(|| left.project_root.cmp(&right.project_root))
                    .then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    }),
            }
        });
        rows
    }

    fn selected_fleet_index_for_rows(&self, rows: &[DetachedFleetRow]) -> usize {
        self.selected_fleet.min(rows.len().saturating_sub(1))
    }

    fn move_fleet_selection(&mut self, step: i32) {
        let rows = self.detached_fleet_rows();
        if rows.is_empty() {
            self.selected_fleet = 0;
            self.selected_fleet_job = 0;
            return;
        }
        let current = self.selected_fleet_index_for_rows(&rows) as i32;
        let max = rows.len().saturating_sub(1) as i32;
        let next = (current + step).clamp(0, max) as usize;
        self.selected_fleet = next;
        self.selected_fleet_job = 0;
    }

    fn selected_fleet_job_index_for_row(&self, row: &DetachedFleetRow) -> usize {
        self.selected_fleet_job
            .min(row.jobs.len().saturating_sub(1))
    }

    fn move_fleet_job_selection(&mut self, step: i32) {
        let Some(row) = self.selected_fleet_row() else {
            return;
        };
        if row.jobs.is_empty() {
            self.selected_fleet_job = 0;
            return;
        }
        let current = self.selected_fleet_job_index_for_row(&row) as i32;
        let max = row.jobs.len().saturating_sub(1) as i32;
        let next = (current + step).clamp(0, max) as usize;
        self.selected_fleet_job = next;
    }

    fn toggle_fleet_plane_filter(&mut self) {
        self.fleet_plane_filter = self.fleet_plane_filter.next();
        self.selected_fleet = 0;
        self.selected_fleet_job = 0;
        self.scroll = 0;
        self.status_note = Some(format!("fleet plane: {}", self.fleet_plane_filter.label()));
    }

    fn toggle_fleet_active_only(&mut self) {
        self.fleet_active_only = !self.fleet_active_only;
        self.selected_fleet = 0;
        self.selected_fleet_job = 0;
        self.scroll = 0;
        self.status_note = Some(if self.fleet_active_only {
            "fleet scope: active-only".to_string()
        } else {
            "fleet scope: all jobs".to_string()
        });
    }

    fn toggle_fleet_sort_mode(&mut self) {
        self.fleet_sort_mode = self.fleet_sort_mode.next();
        self.selected_fleet = 0;
        self.selected_fleet_job = 0;
        self.scroll = 0;
        self.status_note = Some(format!("fleet sort: {}", self.fleet_sort_mode.label()));
    }

    fn selected_fleet_row(&mut self) -> Option<DetachedFleetRow> {
        if self.mode != Mode::Fleet {
            self.status_note = Some("switch to Fleet mode for detached job actions".to_string());
            return None;
        }
        let rows = self.detached_fleet_rows();
        if rows.is_empty() {
            self.status_note = Some("no detached fleet groups available".to_string());
            return None;
        }
        let selected = self.selected_fleet_index_for_rows(&rows);
        self.selected_fleet = selected;
        let row = rows.get(selected).cloned();
        if let Some(row) = row.as_ref() {
            self.selected_fleet_job = self.selected_fleet_job_index_for_row(row);
        }
        row
    }

    fn selected_fleet_job(&mut self) -> Option<(DetachedFleetRow, InsightDetachedJob)> {
        let row = self.selected_fleet_row()?;
        let Some(job) = row
            .jobs
            .get(self.selected_fleet_job_index_for_row(&row))
            .cloned()
        else {
            self.status_note = Some("selected fleet group has no jobs".to_string());
            return None;
        };
        Some((row, job))
    }

    fn render_fleet_brief(
        &self,
        row: &DetachedFleetRow,
        job: &InsightDetachedJob,
        handoff_only: bool,
    ) -> String {
        let target = job
            .agent
            .as_deref()
            .or(job.chain.as_deref())
            .or(job.team.as_deref())
            .unwrap_or("detached-job");
        let summary = job
            .output_excerpt
            .as_deref()
            .or(job.error.as_deref())
            .unwrap_or("No detached job summary available.");
        let action = if handoff_only {
            "/subagent-handoff"
        } else {
            "/subagent-inspect"
        };
        let recovery = detached_job_recovery_guidance(job)
            .into_iter()
            .map(|line| format!("- {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "# Mission Control fleet brief\n\n- project: {}\n- job_id: {}\n- target: {}\n- owner_plane: {}\n- worker_kind: {}\n- status: {}\n- fallback_used: {}\n\n## Detached summary\n{}\n\n## Recovery guidance\n{}\n\n## Main session follow-up\n- In the owning Pi session, run: `{action} {}`\n- If the job is still active, optionally cancel from Mission Control Fleet with `x`.\n- If more context is needed, compare against recent jobs in the same project/plane group.\n",
            row.project_root,
            job.job_id,
            target,
            detached_owner_plane_label(job.owner_plane),
            detached_worker_kind_label(job.worker_kind),
            detached_job_status_label(job.status),
            if job.fallback_used { "yes" } else { "no" },
            summary,
            recovery,
            job.job_id,
        )
    }

    fn write_fleet_brief(
        &self,
        row: &DetachedFleetRow,
        job: &InsightDetachedJob,
        handoff_only: bool,
    ) -> Result<PathBuf, String> {
        let dir = self.config.state_dir.join("mission-control").join("fleet");
        fs::create_dir_all(&dir).map_err(|err| format!("create fleet brief dir failed: {err}"))?;
        let slug = sanitize_slug(&format!(
            "{}-{}-{}",
            if handoff_only { "handoff" } else { "inspect" },
            job.job_id,
            Utc::now().format("%Y%m%d%H%M%S")
        ));
        let path = dir.join(format!("{slug}.md"));
        fs::write(&path, self.render_fleet_brief(row, job, handoff_only))
            .map_err(|err| format!("write fleet brief failed: {err}"))?;
        Ok(path)
    }

    fn launch_fleet_followup(&mut self, handoff_only: bool) {
        let Some((row, job)) = self.selected_fleet_job() else {
            return;
        };
        let brief_path = match self.write_fleet_brief(&row, &job, handoff_only) {
            Ok(path) => path,
            Err(err) => {
                self.status_note = Some(err);
                return;
            }
        };
        let project_root = PathBuf::from(&row.project_root);
        let launch_root = if project_root.exists() {
            project_root
        } else {
            self.config.project_root.clone()
        };
        let tab_name = if handoff_only {
            format!("Handoff {}", ellipsize(&job.job_id, 18))
        } else {
            format!("Inspect {}", ellipsize(&job.job_id, 18))
        };
        let agent_id = resolve_launch_agent_id();
        let plan = build_worker_launch_plan(
            &launch_root,
            &agent_id,
            &tab_name,
            Some(&brief_path),
            in_zellij_session(),
        );
        match execute_worker_launch_plan(&plan) {
            Ok(()) => {
                self.status_note = Some(format!(
                    "launched {} follow-up for {}; brief: {}",
                    if handoff_only { "handoff" } else { "inspect" },
                    job.job_id,
                    brief_path.display()
                ));
            }
            Err(err) => {
                self.status_note = Some(format!("fleet launch failed: {err}"));
            }
        }
    }

    fn cancel_selected_fleet_job(&mut self) {
        let Some((_row, job)) = self.selected_fleet_job() else {
            return;
        };
        if !matches!(
            job.status,
            InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
        ) {
            self.status_note = Some(format!("selected job {} is not active", job.job_id));
            return;
        }
        self.queue_hub_command(
            "insight_detached_cancel",
            None,
            serde_json::json!({"job_id": job.job_id, "reason": "mission_control_fleet"}),
            format!("detached job {}", job.job_id),
        );
    }

    fn focus_selected_fleet_project(&mut self) {
        let Some(row) = self.selected_fleet_row() else {
            return;
        };
        let candidate = self
            .hub
            .agents
            .iter()
            .filter_map(|(agent_id, agent)| {
                let status = agent.status.as_ref()?;
                if status.project_root != row.project_root {
                    return None;
                }
                let tab_meta = self
                    .active_hub_layout()
                    .and_then(|layout| layout.pane_tabs.get(&status.pane_id))
                    .cloned()
                    .or_else(|| self.tab_cache.get(&status.pane_id).cloned());
                Some((agent_id.clone(), status.pane_id.clone(), tab_meta))
            })
            .next();
        let Some((agent_id, pane_id, tab_meta)) = candidate else {
            self.status_note = Some(format!(
                "no live tab found for project {}",
                row.project_root
            ));
            return;
        };
        self.focus_tab_target(
            &pane_id,
            tab_meta.as_ref().map(|meta| meta.index),
            tab_meta.map(|meta| meta.name),
        );
        self.status_note = Some(format!("focused project tab via {}", agent_id));
    }

    fn mind_project_matches(&self, project_root: &str) -> bool {
        if !self.config.mind_project_scoped {
            return true;
        }
        let candidate = normalized_project_root_key(project_root);
        !candidate.is_empty()
            && candidate == normalized_project_root_key(&self.config.project_root.to_string_lossy())
    }

    fn mind_rows_for_lane(&self, lane_filter: MindLaneFilter) -> Vec<MindObserverRow> {
        if !self.prefer_hub_data(!self.hub.mind.is_empty()) {
            return Vec::new();
        }

        let viewer_scope = self.config.tab_scope.as_deref();
        let mut rows = Vec::new();
        for (agent_id, feed) in &self.hub.mind {
            if feed.events.is_empty() {
                continue;
            }
            let status = self
                .hub
                .agents
                .get(agent_id)
                .and_then(|agent| agent.status.as_ref());
            let scope = status
                .and_then(|value| value.agent_label.clone())
                .unwrap_or_else(|| extract_label(agent_id));
            let pane_id = status
                .map(|value| value.pane_id.clone())
                .unwrap_or_else(|| extract_pane_id(agent_id));
            let tab_scope = status.and_then(|value| value.tab_scope.clone());
            let tab_focused = tab_scope_matches(viewer_scope, tab_scope.as_deref());
            if let Some(project_root) = status.map(|value| value.project_root.as_str()) {
                if !self.mind_project_matches(project_root) {
                    continue;
                }
            } else if self.config.mind_project_scoped {
                continue;
            }
            if !self.mind_show_all_tabs && viewer_scope.is_some() && !tab_focused {
                continue;
            }

            for event in &feed.events {
                let lane = mind_event_lane(event);
                if !mind_lane_matches(lane_filter, lane) {
                    continue;
                }
                rows.push(MindObserverRow {
                    agent_id: agent_id.clone(),
                    scope: scope.clone(),
                    pane_id: pane_id.clone(),
                    tab_scope: tab_scope.clone(),
                    tab_focused,
                    event: event.clone(),
                    source: "hub".to_string(),
                });
            }
        }

        rows.sort_by(|left, right| {
            let left_ts = mind_event_sort_ms(left.event.completed_at.as_deref())
                .or_else(|| mind_event_sort_ms(left.event.started_at.as_deref()))
                .or_else(|| mind_event_sort_ms(left.event.enqueued_at.as_deref()))
                .unwrap_or(0);
            let right_ts = mind_event_sort_ms(right.event.completed_at.as_deref())
                .or_else(|| mind_event_sort_ms(right.event.started_at.as_deref()))
                .or_else(|| mind_event_sort_ms(right.event.enqueued_at.as_deref()))
                .unwrap_or(0);
            right_ts
                .cmp(&left_ts)
                .then_with(|| right.tab_focused.cmp(&left.tab_focused))
                .then_with(|| left.scope.cmp(&right.scope))
                .then_with(|| left.pane_id.cmp(&right.pane_id))
        });
        rows
    }

    fn mind_rows(&self) -> Vec<MindObserverRow> {
        self.mind_rows_for_lane(self.mind_lane)
    }

    fn mind_injection_rows(&self) -> Vec<MindInjectionRow> {
        if !self.prefer_hub_data(!self.hub.mind_injection.is_empty()) {
            return Vec::new();
        }

        let viewer_scope = self.config.tab_scope.as_deref();
        let mut rows = Vec::new();
        for (agent_id, payload) in &self.hub.mind_injection {
            let status = self
                .hub
                .agents
                .get(agent_id)
                .and_then(|agent| agent.status.as_ref());
            let scope = status
                .and_then(|value| value.agent_label.clone())
                .unwrap_or_else(|| extract_label(agent_id));
            let pane_id = status
                .map(|value| value.pane_id.clone())
                .unwrap_or_else(|| extract_pane_id(agent_id));
            let tab_scope = status.and_then(|value| value.tab_scope.clone());
            let tab_focused = tab_scope_matches(viewer_scope, tab_scope.as_deref());
            if let Some(project_root) = status.map(|value| value.project_root.as_str()) {
                if !self.mind_project_matches(project_root) {
                    continue;
                }
            } else if self.config.mind_project_scoped {
                continue;
            }
            if !self.mind_show_all_tabs && viewer_scope.is_some() && !tab_focused {
                continue;
            }
            rows.push(MindInjectionRow {
                scope,
                pane_id,
                tab_focused,
                payload: payload.clone(),
            });
        }

        rows.sort_by(|left, right| {
            let left_ts = mind_event_sort_ms(Some(&left.payload.queued_at)).unwrap_or(0);
            let right_ts = mind_event_sort_ms(Some(&right.payload.queued_at)).unwrap_or(0);
            right_ts
                .cmp(&left_ts)
                .then_with(|| right.tab_focused.cmp(&left.tab_focused))
                .then_with(|| left.scope.cmp(&right.scope))
                .then_with(|| left.pane_id.cmp(&right.pane_id))
        });
        rows
    }

    fn mind_target_agent(&self) -> Option<OverviewRow> {
        let rows = self.overview_rows();
        rows.into_iter()
            .find(|row| row.tab_focused && self.mind_project_matches(&row.project_root))
            .or_else(|| {
                self.hub.agents.iter().find_map(|(agent_id, agent)| {
                    let status = agent.status.as_ref()?;
                    let tab_focused = tab_scope_matches(
                        self.config.tab_scope.as_deref(),
                        status.tab_scope.as_deref(),
                    );
                    if !tab_focused || !self.mind_project_matches(&status.project_root) {
                        return None;
                    }
                    Some(OverviewRow {
                        identity_key: agent_id.clone(),
                        label: status
                            .agent_label
                            .clone()
                            .unwrap_or_else(|| extract_label(agent_id)),
                        lifecycle: status.status.clone(),
                        snippet: status.message.clone(),
                        pane_id: status.pane_id.clone(),
                        tab_index: None,
                        tab_name: status.tab_scope.clone(),
                        tab_focused,
                        project_root: status.project_root.clone(),
                        online: true,
                        age_secs: None,
                        source: "hub".to_string(),
                        session_title: status.session_title.clone(),
                        chat_title: status.chat_title.clone(),
                    })
                })
            })
            .or_else(|| {
                self.hub.agents.iter().find_map(|(agent_id, agent)| {
                    let status = agent.status.as_ref();
                    if self.config.mind_project_scoped
                        && !status
                            .map(|value| self.mind_project_matches(&value.project_root))
                            .unwrap_or(false)
                    {
                        return None;
                    }
                    Some(OverviewRow {
                        identity_key: agent_id.clone(),
                        label: status
                            .and_then(|value| value.agent_label.clone())
                            .unwrap_or_else(|| extract_label(agent_id)),
                        lifecycle: status
                            .map(|value| value.status.clone())
                            .unwrap_or_else(|| "unknown".to_string()),
                        snippet: status.and_then(|value| value.message.clone()),
                        pane_id: status
                            .map(|value| value.pane_id.clone())
                            .unwrap_or_else(|| extract_pane_id(agent_id)),
                        tab_index: None,
                        tab_name: status.and_then(|value| value.tab_scope.clone()),
                        tab_focused: false,
                        project_root: status
                            .map(|value| value.project_root.clone())
                            .unwrap_or_else(|| "(unknown)".to_string()),
                        online: true,
                        age_secs: None,
                        source: "hub".to_string(),
                        session_title: status.and_then(|value| value.session_title.clone()),
                        chat_title: status.and_then(|value| value.chat_title.clone()),
                    })
                })
            })
    }

    fn selected_overseer_worker(&mut self) -> Option<WorkerSnapshot> {
        if self.mode != Mode::Overseer {
            self.status_note = Some("switch to Overseer mode for worker consultation".to_string());
            return None;
        }
        let workers = self.overseer_workers();
        if workers.is_empty() {
            self.status_note = Some("no workers available for consultation".to_string());
            return None;
        }
        let selected = self.selected_overview.min(workers.len().saturating_sub(1));
        self.selected_overview = selected;
        workers.get(selected).cloned()
    }

    fn selected_overview_row(&mut self) -> Option<OverviewRow> {
        let rows = self.overview_rows();
        if rows.is_empty() {
            self.status_note = Some("no agents available".to_string());
            return None;
        }
        let selected = self.selected_overview_index_for_rows(&rows);
        self.selected_overview = selected;
        rows.get(selected).cloned()
    }

    fn selected_pane_target(&mut self) -> Option<(String, String, PathBuf)> {
        match self.mode {
            Mode::Overview => self.selected_overview_row().map(|row| {
                let project_root = if row.project_root.trim().is_empty() {
                    self.config.project_root.clone()
                } else {
                    PathBuf::from(row.project_root)
                };
                (row.pane_id, row.label, project_root)
            }),
            Mode::Overseer => self.selected_overseer_worker().map(|worker| {
                (
                    worker.pane_id,
                    worker.agent_id,
                    self.config.project_root.clone(),
                )
            }),
            Mode::Mind => self.mind_target_agent().map(|row| {
                let project_root = if row.project_root.trim().is_empty() {
                    self.config.project_root.clone()
                } else {
                    PathBuf::from(row.project_root)
                };
                (row.pane_id, row.label, project_root)
            }),
            _ => {
                self.status_note = Some(
                    "pane evidence is available in Overview, Overseer, or Mind mode".to_string(),
                );
                None
            }
        }
    }

    fn capture_selected_pane_evidence(&mut self) {
        let Some((pane_id, label, _project_root)) = self.selected_pane_target() else {
            return;
        };
        let dir = self
            .config
            .state_dir
            .join("mission-control")
            .join("pane-evidence");
        if let Err(err) = fs::create_dir_all(&dir) {
            self.status_note = Some(format!("create evidence dir failed: {err}"));
            return;
        }
        let stamp = Utc::now().format("%Y%m%d%H%M%S");
        let filename = format!(
            "{}-pane-{}-{}.ansi",
            sanitize_slug(&label),
            sanitize_slug(&pane_id),
            stamp
        );
        let path = dir.join(filename);
        match dump_pane_evidence(&self.config.session_id, &pane_id, &path) {
            Ok(()) => {
                self.status_note = Some(format!(
                    "pane evidence saved for {} ({}) -> {}",
                    label,
                    pane_id,
                    path.display()
                ));
            }
            Err(err) => {
                self.status_note = Some(format!("pane evidence failed for {pane_id}: {err}"));
            }
        }
    }

    fn follow_selected_pane_live(&mut self) {
        let Some((pane_id, label, project_root)) = self.selected_pane_target() else {
            return;
        };
        if !in_zellij_session() {
            self.status_note =
                Some("live pane follow requires running Mission Control inside Zellij".to_string());
            return;
        }
        match launch_pane_follow(&self.config.session_id, &pane_id, &label, &project_root) {
            Ok(()) => {
                self.status_note = Some(format!(
                    "live pane follow opened for {} ({})",
                    label, pane_id
                ));
            }
            Err(err) => {
                self.status_note = Some(format!("live pane follow failed for {pane_id}: {err}"));
            }
        }
    }

    fn consultation_peer_for(&self, focal_agent_id: &str) -> Option<WorkerSnapshot> {
        self.overseer_workers()
            .into_iter()
            .filter(|worker| worker.agent_id != focal_agent_id)
            .max_by_key(|worker| {
                let status_rank = match worker.status {
                    WorkerStatus::Active => 4,
                    WorkerStatus::Done => 3,
                    WorkerStatus::Idle => 2,
                    WorkerStatus::NeedsInput | WorkerStatus::Blocked => 1,
                    WorkerStatus::Paused | WorkerStatus::Offline => 0,
                };
                let aligned = matches!(worker.plan_alignment, PlanAlignment::High) as u8;
                (status_rank, aligned)
            })
    }

    fn request_overseer_consultation(&mut self, kind: ConsultationPacketKind) {
        let Some(requester) = self.selected_overseer_worker() else {
            return;
        };
        let Some(responder) = self.consultation_peer_for(&requester.agent_id) else {
            self.status_note = Some("need at least two workers for peer consultation".to_string());
            return;
        };
        if !self.connected {
            self.status_note = Some("hub offline; consultation unavailable".to_string());
            return;
        }

        let checkpoint = self.latest_compaction_checkpoint();
        let mind_event = self.overseer_mind_event(&requester.agent_id);
        let packet =
            derive_overseer_consultation_packet(&requester, checkpoint.as_ref(), mind_event)
                .normalize();
        let request_packet = ConsultationPacket { kind, ..packet };
        let request_id = self.next_command_request_id();
        let consultation_id = format!(
            "{}:{}:{}",
            requester.session_id, requester.agent_id, request_id
        );
        let outbound = HubOutbound {
            request_id: request_id.clone(),
            msg: WireMsg::ConsultationRequest(ConsultationRequestPayload {
                consultation_id,
                requesting_agent_id: requester.agent_id.clone(),
                target_agent_id: responder.agent_id.clone(),
                packet: request_packet.clone(),
            }),
        };
        match self.command_tx.try_send(outbound) {
            Ok(()) => {
                self.pending_consultations.insert(
                    request_id,
                    PendingConsultation {
                        kind,
                        requester: requester.agent_id.clone(),
                        responder: responder.agent_id.clone(),
                        request_packet,
                    },
                );
                self.status_note = Some(format!(
                    "consult {:?} queued {} -> {}",
                    kind, requester.agent_id, responder.agent_id
                ));
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.status_note = Some("hub consultation queue full".to_string());
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.status_note = Some("hub consultation channel closed".to_string());
            }
        }
    }

    fn selected_overseer_worker_ref(&self) -> Option<WorkerSnapshot> {
        let workers = self.overseer_workers();
        workers
            .get(self.selected_overview.min(workers.len().saturating_sub(1)))
            .cloned()
    }

    fn orchestrator_tools(&self) -> Vec<OrchestratorTool> {
        let selected = self.selected_overseer_worker_ref();
        let has_peer = selected
            .as_ref()
            .map(|worker| self.consultation_peer_for(&worker.agent_id).is_some())
            .unwrap_or(false);
        let snapshot_ready = self.overseer_snapshot().is_some() || self.connected;
        let timeline_ready = !self.overseer_timeline().is_empty() || self.connected;
        let launch_ready = self.worker_launch_supported();

        let mut tools = vec![
            OrchestratorTool {
                id: OrchestratorToolId::SessionSnapshot,
                label: "session snapshot",
                scope: "session",
                shortcut: None,
                status: if snapshot_ready {
                    OrchestratorToolStatus::Ready
                } else {
                    OrchestratorToolStatus::Unavailable
                },
                summary: "inspect the current worker snapshot".to_string(),
                reason: (!snapshot_ready).then(|| "waiting for hub snapshot".to_string()),
            },
            OrchestratorTool {
                id: OrchestratorToolId::SessionTimeline,
                label: "session timeline",
                scope: "session",
                shortcut: None,
                status: if timeline_ready {
                    OrchestratorToolStatus::Ready
                } else {
                    OrchestratorToolStatus::Unavailable
                },
                summary: "inspect recent overseer events".to_string(),
                reason: (!timeline_ready).then(|| "waiting for hub timeline".to_string()),
            },
        ];

        if let Some(worker) = selected {
            let worker_target = worker.agent_id.clone();
            tools.extend([
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerFocus,
                    label: "focus worker tab",
                    scope: "worker",
                    shortcut: Some("Enter"),
                    status: OrchestratorToolStatus::Ready,
                    summary: format!("focus {worker_target} in zellij"),
                    reason: None,
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerReview,
                    label: "peer review",
                    scope: "worker",
                    shortcut: Some("c"),
                    status: if self.connected && has_peer {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("request bounded peer review for {worker_target}"),
                    reason: if !self.connected {
                        Some("hub offline".to_string())
                    } else if !has_peer {
                        Some("need another in-session worker".to_string())
                    } else {
                        None
                    },
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerHelp,
                    label: "peer unblock",
                    scope: "worker",
                    shortcut: Some("u"),
                    status: if self.connected && has_peer {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("request unblock/help guidance for {worker_target}"),
                    reason: if !self.connected {
                        Some("hub offline".to_string())
                    } else if !has_peer {
                        Some("need another in-session worker".to_string())
                    } else {
                        None
                    },
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerObserve,
                    label: "run observer",
                    scope: "worker",
                    shortcut: Some("o"),
                    status: if self.connected {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("request fresh observer run for {worker_target}"),
                    reason: (!self.connected).then(|| "hub offline".to_string()),
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerStop,
                    label: "stop worker",
                    scope: "worker",
                    shortcut: Some("x"),
                    status: if self.connected {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("stop {worker_target} via hub command"),
                    reason: (!self.connected).then(|| "hub offline".to_string()),
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerSpawn,
                    label: "spawn worker",
                    scope: "session",
                    shortcut: Some("s"),
                    status: if launch_ready {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: "launch a fresh worker tab from Mission Control".to_string(),
                    reason: (!launch_ready)
                        .then(|| "project root unavailable for launcher".to_string()),
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerDelegate,
                    label: "delegate task",
                    scope: "worker",
                    shortcut: Some("d"),
                    status: if launch_ready {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!(
                        "spawn a delegated worker with bounded brief for {worker_target}"
                    ),
                    reason: (!launch_ready)
                        .then(|| "project root unavailable for launcher".to_string()),
                },
            ]);
        }

        tools
    }

    fn orchestration_graph_ir(&self) -> OrchestrationGraphIr {
        let workers = self.overseer_workers();
        let selected = self.selected_overseer_worker_ref();
        let tools = self.orchestrator_tools();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let session_id = selected
            .as_ref()
            .map(|worker| worker.session_id.clone())
            .or_else(|| {
                self.overseer_snapshot()
                    .map(|snapshot| snapshot.session_id.clone())
            })
            .unwrap_or_else(|| self.config.session_id.clone());
        let session_node_id = format!("session:{session_id}");
        let mut session_attrs = BTreeMap::new();
        session_attrs.insert(
            "hub".to_string(),
            if self.connected { "online" } else { "offline" }.to_string(),
        );
        session_attrs.insert(
            "mode".to_string(),
            format!("{:?}", self.mode).to_ascii_lowercase(),
        );
        nodes.push(OrchestrationGraphNode {
            id: session_node_id.clone(),
            kind: OrchestrationGraphNodeKind::Session,
            label: "Mission Control session".to_string(),
            status: if self.connected { "online" } else { "offline" }.to_string(),
            attrs: session_attrs,
        });

        for worker in workers {
            let worker_id = format!("worker:{}", worker.agent_id);
            let mut attrs = BTreeMap::new();
            attrs.insert("pane_id".to_string(), worker.pane_id.clone());
            if let Some(role) = worker.role.as_ref() {
                attrs.insert("role".to_string(), role.clone());
            }
            if let Some(task_id) = worker.assignment.task_id.as_ref() {
                attrs.insert("task_id".to_string(), task_id.clone());
            }
            if let Some(tag) = worker.assignment.tag.as_ref() {
                attrs.insert("tag".to_string(), tag.clone());
            }
            if selected
                .as_ref()
                .map(|candidate| candidate.agent_id == worker.agent_id)
                .unwrap_or(false)
            {
                attrs.insert("selected".to_string(), "true".to_string());
            }
            nodes.push(OrchestrationGraphNode {
                id: worker_id.clone(),
                kind: OrchestrationGraphNodeKind::Worker,
                label: worker.agent_id.clone(),
                status: format!("{:?}", worker.status).to_ascii_lowercase(),
                attrs,
            });
            edges.push(OrchestrationGraphEdge {
                from: session_node_id.clone(),
                to: worker_id.clone(),
                kind: if selected
                    .as_ref()
                    .map(|candidate| candidate.agent_id == worker.agent_id)
                    .unwrap_or(false)
                {
                    OrchestrationGraphEdgeKind::Selects
                } else {
                    OrchestrationGraphEdgeKind::Enumerates
                },
                summary: if selected
                    .as_ref()
                    .map(|candidate| candidate.agent_id == worker.agent_id)
                    .unwrap_or(false)
                {
                    "selected worker in current overseer view".to_string()
                } else {
                    "worker snapshot in current session".to_string()
                },
            });
        }

        for tool in &tools {
            let tool_id = format!("tool:{}", orchestrator_tool_id_slug(tool.id));
            let mut attrs = BTreeMap::new();
            attrs.insert("scope".to_string(), tool.scope.to_string());
            if let Some(shortcut) = tool.shortcut {
                attrs.insert("shortcut".to_string(), shortcut.to_string());
            }
            nodes.push(OrchestrationGraphNode {
                id: tool_id.clone(),
                kind: OrchestrationGraphNodeKind::Tool,
                label: tool.label.to_string(),
                status: match tool.status {
                    OrchestratorToolStatus::Ready => "ready",
                    OrchestratorToolStatus::Unavailable => "blocked",
                }
                .to_string(),
                attrs,
            });
            edges.push(OrchestrationGraphEdge {
                from: session_node_id.clone(),
                to: tool_id.clone(),
                kind: OrchestrationGraphEdgeKind::Enumerates,
                summary: "tool surfaced in Mission Control".to_string(),
            });

            if let Some(worker) = selected.as_ref() {
                if tool.scope == "worker" {
                    edges.push(OrchestrationGraphEdge {
                        from: tool_id.clone(),
                        to: format!("worker:{}", worker.agent_id),
                        kind: OrchestrationGraphEdgeKind::OperatesOn,
                        summary: tool.summary.clone(),
                    });
                }
                if tool.id == OrchestratorToolId::WorkerDelegate {
                    let artifact_id = format!(
                        "artifact:delegation-brief:{}",
                        sanitize_slug(&worker.agent_id)
                    );
                    let mut attrs = BTreeMap::new();
                    attrs.insert(
                        "path".to_string(),
                        self.config
                            .state_dir
                            .join("mission-control")
                            .join("delegations")
                            .to_string_lossy()
                            .to_string(),
                    );
                    nodes.push(OrchestrationGraphNode {
                        id: artifact_id.clone(),
                        kind: OrchestrationGraphNodeKind::Artifact,
                        label: "delegation brief".to_string(),
                        status: match tool.status {
                            OrchestratorToolStatus::Ready => "ready",
                            OrchestratorToolStatus::Unavailable => "blocked",
                        }
                        .to_string(),
                        attrs,
                    });
                    edges.push(OrchestrationGraphEdge {
                        from: tool_id.clone(),
                        to: artifact_id.clone(),
                        kind: OrchestrationGraphEdgeKind::Writes,
                        summary: "write bounded delegation brief before launch".to_string(),
                    });
                    edges.push(OrchestrationGraphEdge {
                        from: artifact_id,
                        to: format!("worker:{}", worker.agent_id),
                        kind: OrchestrationGraphEdgeKind::DelegatesFrom,
                        summary: "delegated worker inherits bounded source context".to_string(),
                    });
                }
                if matches!(
                    tool.id,
                    OrchestratorToolId::WorkerSpawn | OrchestratorToolId::WorkerDelegate
                ) {
                    edges.push(OrchestrationGraphEdge {
                        from: tool_id,
                        to: session_node_id.clone(),
                        kind: OrchestrationGraphEdgeKind::Launches,
                        summary: "compile path launches a fresh worker tab".to_string(),
                    });
                }
            }
        }

        let compile_paths = tools
            .iter()
            .filter(|tool| {
                matches!(
                    tool.id,
                    OrchestratorToolId::WorkerReview
                        | OrchestratorToolId::WorkerHelp
                        | OrchestratorToolId::WorkerObserve
                        | OrchestratorToolId::WorkerStop
                        | OrchestratorToolId::WorkerSpawn
                        | OrchestratorToolId::WorkerDelegate
                )
            })
            .map(|tool| self.compile_orchestration_path(tool, selected.as_ref()))
            .collect();

        OrchestrationGraphIr {
            session_id,
            selected_worker_id: selected.as_ref().map(|worker| worker.agent_id.clone()),
            nodes,
            edges,
            compile_paths,
        }
    }

    fn compile_orchestration_path(
        &self,
        tool: &OrchestratorTool,
        selected: Option<&WorkerSnapshot>,
    ) -> OrchestrationCompilePath {
        let selected_label = selected
            .map(|worker| worker.agent_id.clone())
            .unwrap_or_else(|| "selected worker".to_string());
        let steps = match tool.id {
            OrchestratorToolId::SessionSnapshot => {
                vec!["inspect current session snapshot".to_string()]
            }
            OrchestratorToolId::SessionTimeline => {
                vec!["inspect recent overseer timeline".to_string()]
            }
            OrchestratorToolId::WorkerFocus => vec![
                format!("resolve tab target for {selected_label}"),
                "focus target tab in zellij when metadata is present".to_string(),
            ],
            OrchestratorToolId::WorkerReview => vec![
                format!("select requester {selected_label}"),
                "resolve a peer worker in the same session".to_string(),
                "queue bounded peer review consultation through hub".to_string(),
            ],
            OrchestratorToolId::WorkerHelp => vec![
                format!("select requester {selected_label}"),
                "resolve a peer worker in the same session".to_string(),
                "queue bounded unblock/help consultation through hub".to_string(),
            ],
            OrchestratorToolId::WorkerObserve => vec![
                format!("select target {selected_label}"),
                "queue run_observer command through hub".to_string(),
            ],
            OrchestratorToolId::WorkerStop => vec![
                format!("select target {selected_label}"),
                "queue stop_agent command through hub".to_string(),
            ],
            OrchestratorToolId::WorkerSpawn => vec![
                format!(
                    "resolve launch agent from env/current agent for {}",
                    self.config.session_id
                ),
                format!("compile worker tab name {}", self.next_worker_tab_name()),
                "launch fresh worker tab via aoc-new-tab or aoc-launch".to_string(),
                "return focus to Mission Control tab when possible".to_string(),
            ],
            OrchestratorToolId::WorkerDelegate => vec![
                format!("select source worker {selected_label}"),
                "render bounded delegation brief from worker snapshot".to_string(),
                "write delegation brief under state_dir/mission-control/delegations".to_string(),
                format!(
                    "compile delegated tab name {}",
                    selected
                        .map(App::delegation_tab_name)
                        .unwrap_or_else(|| "delegated-worker".to_string())
                ),
                "launch delegated worker tab and export AOC_DELEGATION_BRIEF_PATH".to_string(),
                "return focus to Mission Control tab when possible".to_string(),
            ],
        };
        OrchestrationCompilePath {
            entry_tool: tool.id,
            review_label: tool.label.to_string(),
            status: tool.status,
            steps,
        }
    }

    fn request_manual_observer_run(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for observer run".to_string());
            return;
        };
        self.queue_hub_command(
            "run_observer",
            Some(target.identity_key.clone()),
            serde_json::json!({"trigger": "manual_shortcut", "reason": "pulse_user_request"}),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    fn request_insight_dispatch_chain(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for insight dispatch".to_string());
            return;
        };
        self.queue_hub_command(
            "insight_dispatch",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "mode": "chain",
                "chain": "insight-handoff",
                "reason": "pulse_mind_action",
                "input": "Mind panel dispatch (T1 -> T2)"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    fn request_insight_bootstrap(&mut self, dry_run: bool) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for insight bootstrap".to_string());
            return;
        };
        self.queue_hub_command(
            "insight_bootstrap",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "dry_run": dry_run,
                "max_gaps": 12
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    fn request_mind_force_finalize(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for force finalize".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_finalize_session",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator force finalize"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    fn request_mind_t3_requeue(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for t3 requeue".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_t3_requeue",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator requeue"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    fn request_mind_handshake_rebuild(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for handshake rebuild".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_handshake_rebuild",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator rebuild"}
            ),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    fn request_mind_compaction_rebuild(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for compaction rebuild".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_compaction_rebuild",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator compaction rebuild"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    fn toggle_mind_lane(&mut self) {
        self.mind_lane = self.mind_lane.next();
        self.scroll = 0;
        self.status_note = Some(format!("mind lane: {}", self.mind_lane.label()));
    }

    fn toggle_mind_scope(&mut self) {
        self.mind_show_all_tabs = !self.mind_show_all_tabs;
        self.scroll = 0;
        self.status_note = Some(if self.mind_show_all_tabs {
            "mind scope: all tabs".to_string()
        } else {
            "mind scope: active tab".to_string()
        });
    }

    fn toggle_mind_provenance(&mut self) {
        self.mind_show_provenance = !self.mind_show_provenance;
        self.scroll = 0;
        self.status_note = Some(if self.mind_show_provenance {
            "mind provenance: expanded".to_string()
        } else {
            "mind provenance: compact".to_string()
        });
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
            if let Some(index) = rows.iter().position(|row| row.tab_focused) {
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

    fn focus_tab_target(
        &mut self,
        pane_id: &str,
        tab_index: Option<usize>,
        tab_name: Option<String>,
    ) {
        if self.connected {
            let mut args = serde_json::Map::new();
            if let Some(tab_index) = tab_index {
                args.insert("tab_index".to_string(), Value::from(tab_index as u64));
            }
            if let Some(tab_name) = tab_name.as_ref().filter(|value| !value.trim().is_empty()) {
                args.insert("tab_name".to_string(), Value::String(tab_name.clone()));
            }
            if args.is_empty() {
                self.status_note = Some(format!("no tab id/name for pane {pane_id}"));
                return;
            }
            self.queue_hub_command(
                "focus_tab",
                None,
                Value::Object(args),
                format!("pane {pane_id}"),
            );
            return;
        }

        let Some(tab_index) = tab_index else {
            self.status_note = Some(format!("no tab id/name for pane {pane_id}"));
            return;
        };
        if let Err(err) = go_to_tab(&self.config.session_id, tab_index) {
            self.status_note = Some(format!("focus failed: {err}"));
        } else {
            self.status_note = Some(format!("focused tab {tab_index} for pane {pane_id}"));
        }
    }

    fn current_tab_index(&self) -> Option<usize> {
        self.active_hub_layout()
            .and_then(|layout| {
                layout
                    .pane_tabs
                    .get(&self.config.pane_id)
                    .map(|meta| meta.index)
                    .or(layout.focused_tab_index)
            })
            .or(self.local.viewer_tab_index)
    }

    fn worker_launch_supported(&self) -> bool {
        self.config.project_root.exists()
    }

    fn next_worker_tab_name(&self) -> String {
        format!("Worker {}", self.overseer_workers().len().saturating_add(1))
    }

    fn delegation_tab_name(worker: &WorkerSnapshot) -> String {
        if let Some(task_id) = worker
            .assignment
            .task_id
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            return format!("Delegate {task_id}");
        }
        if let Some(tag) = worker
            .assignment
            .tag
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            return format!("Delegate {tag}");
        }
        if let Some(role) = worker
            .role
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            return format!("Delegate {role}");
        }
        "Delegated Worker".to_string()
    }

    fn render_delegation_brief(&self, worker: &WorkerSnapshot) -> String {
        let summary = worker
            .summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("No bounded worker summary available.");
        let task = worker.assignment.task_id.as_deref().unwrap_or("unassigned");
        let tag = worker.assignment.tag.as_deref().unwrap_or("unscoped");
        let role = worker.role.as_deref().unwrap_or("worker");
        let blocker = worker.blocker.as_deref().unwrap_or("none reported");
        format!(
            "# Mission Control delegation brief\n\n- session: {}\n- source worker: {}\n- pane: {}\n- role: {}\n- task: {}\n- tag: {}\n- status: {:?}\n- plan alignment: {:?}\n- drift risk: {:?}\n- blocker: {}\n\n## Focus summary\n{}\n\n## Operator guidance\n- Use this as bounded context only; re-observe before making major plan changes.\n- Prefer explicit task/tag alignment and a narrow validation goal.\n- Request peer review or unblock consultation if uncertainty remains.\n",
            worker.session_id,
            worker.agent_id,
            worker.pane_id,
            role,
            task,
            tag,
            worker.status,
            worker.plan_alignment,
            worker.drift_risk,
            blocker,
            summary,
        )
    }

    fn write_delegation_brief(&self, worker: &WorkerSnapshot) -> Result<PathBuf, String> {
        let dir = self
            .config
            .state_dir
            .join("mission-control")
            .join("delegations");
        fs::create_dir_all(&dir).map_err(|err| format!("create delegation dir failed: {err}"))?;
        let slug = sanitize_slug(&format!(
            "{}-{}-{}",
            worker.agent_id,
            worker.assignment.task_id.as_deref().unwrap_or("worker"),
            Utc::now().format("%Y%m%d%H%M%S")
        ));
        let path = dir.join(format!("{slug}.md"));
        fs::write(&path, self.render_delegation_brief(worker))
            .map_err(|err| format!("write delegation brief failed: {err}"))?;
        Ok(path)
    }

    fn launch_worker_tab(
        &mut self,
        tab_name: &str,
        brief_path: Option<&Path>,
    ) -> Result<String, String> {
        if !self.worker_launch_supported() {
            return Err("project root unavailable for worker launch".to_string());
        }
        let in_zellij = in_zellij_session();
        let return_tab = self.current_tab_index();
        let agent_id = resolve_launch_agent_id();
        let plan = build_worker_launch_plan(
            &self.config.project_root,
            &agent_id,
            tab_name,
            brief_path,
            in_zellij,
        );
        execute_worker_launch_plan(&plan)?;
        if in_zellij {
            if let Some(tab_index) = return_tab {
                let _ = go_to_tab(&self.config.session_id, tab_index);
            }
        }
        Ok(agent_id)
    }

    fn request_spawn_worker(&mut self) {
        if self.mode != Mode::Overseer {
            self.status_note = Some("switch to Overseer mode to spawn workers".to_string());
            return;
        }
        let tab_name = self.next_worker_tab_name();
        match self.launch_worker_tab(&tab_name, None) {
            Ok(agent_id) => {
                self.status_note = Some(format!("spawned {tab_name} with agent {agent_id}"));
            }
            Err(err) => {
                self.status_note = Some(format!("spawn failed: {err}"));
            }
        }
    }

    fn request_delegate_worker(&mut self) {
        let Some(worker) = self.selected_overseer_worker() else {
            return;
        };
        let tab_name = Self::delegation_tab_name(&worker);
        match self.write_delegation_brief(&worker) {
            Ok(brief_path) => match self.launch_worker_tab(&tab_name, Some(&brief_path)) {
                Ok(agent_id) => {
                    self.status_note = Some(format!(
                        "delegated {} via {tab_name} ({agent_id}); brief: {}",
                        worker.agent_id,
                        brief_path.display()
                    ));
                }
                Err(err) => {
                    self.status_note = Some(format!("delegate failed: {err}"));
                }
            },
            Err(err) => {
                self.status_note = Some(format!("delegate failed: {err}"));
            }
        }
    }

    fn focus_selected_overview_tab(&mut self) {
        match self.mode {
            Mode::Overview => {
                let rows = self.overview_rows();
                if rows.is_empty() {
                    self.status_note = Some("no agents to focus".to_string());
                    return;
                }
                let selected = self.selected_overview_index_for_rows(&rows);
                self.selected_overview = selected;
                let row = &rows[selected];
                self.focus_tab_target(&row.pane_id, row.tab_index, row.tab_name.clone());
            }
            Mode::Overseer => {
                let Some(worker) = self.selected_overseer_worker() else {
                    return;
                };
                let tab_meta = self
                    .active_hub_layout()
                    .and_then(|layout| layout.pane_tabs.get(&worker.pane_id))
                    .cloned()
                    .or_else(|| self.tab_cache.get(&worker.pane_id).cloned());
                self.focus_tab_target(
                    &worker.pane_id,
                    tab_meta.as_ref().map(|meta| meta.index),
                    tab_meta.map(|meta| meta.name),
                );
            }
            _ => {}
        }
    }

    fn stop_selected_overview_agent(&mut self) {
        match self.mode {
            Mode::Overview => {
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
            Mode::Overseer => {
                let Some(worker) = self.selected_overseer_worker() else {
                    return;
                };
                self.queue_hub_command(
                    "stop_agent",
                    Some(worker.agent_id.clone()),
                    serde_json::json!({"reason": "pulse_user_request"}),
                    format!("{}::{}", worker.agent_id, worker.pane_id),
                );
            }
            _ => {}
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
    #[serde(default)]
    tab_scope: Option<String>,
    #[serde(default)]
    session_title: Option<String>,
    #[serde(default)]
    chat_title: Option<String>,
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
                if app.config.overview_enabled || app.config.runtime_mode.is_pulse_pane() {
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

#[derive(Clone, Copy, Debug)]
struct PulseTheme {
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

fn pulse_theme(mode: PulseThemeMode) -> PulseTheme {
    match mode {
        PulseThemeMode::Terminal => PulseTheme {
            surface: Color::Reset,
            border: Color::DarkGray,
            title: Color::Cyan,
            text: Color::Reset,
            muted: Color::DarkGray,
            accent: Color::Blue,
            ok: Color::Green,
            warn: Color::Yellow,
            critical: Color::Red,
            info: Color::Cyan,
        },
        PulseThemeMode::Dark => PulseTheme {
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
        },
        PulseThemeMode::Light => PulseTheme {
            surface: Color::Rgb(245, 247, 250),
            border: Color::Rgb(148, 163, 184),
            title: Color::Rgb(30, 64, 175),
            text: Color::Rgb(15, 23, 42),
            muted: Color::Rgb(100, 116, 139),
            accent: Color::Rgb(2, 132, 199),
            ok: Color::Rgb(22, 163, 74),
            warn: Color::Rgb(217, 119, 6),
            critical: Color::Rgb(220, 38, 38),
            info: Color::Rgb(37, 99, 235),
        },
    }
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

fn resolve_custom_pulse_theme() -> Option<PulseTheme> {
    let env_color = |key: &str| -> Option<Color> {
        std::env::var(key).ok().as_deref().and_then(parse_hex_color)
    };
    let env_color_any =
        |keys: &[&str]| -> Option<Color> { keys.iter().find_map(|key| env_color(key)) };

    Some(PulseTheme {
        // Inherit the pane/terminal background so Pulse matches the rest of the AOC layout.
        surface: Color::Reset,
        border: env_color_any(&["AOC_THEME_BG_ELEVATED", "AOC_THEME_BLACK"])?,
        title: env_color_any(&["AOC_THEME_UI_ACCENT", "AOC_THEME_BLUE"])?,
        text: env_color_any(&["AOC_THEME_UI_PRIMARY", "AOC_THEME_FG"])?,
        muted: env_color_any(&["AOC_THEME_UI_MUTED", "AOC_THEME_WHITE"])?,
        accent: env_color_any(&["AOC_THEME_UI_ACCENT", "AOC_THEME_BLUE"])?,
        ok: env_color_any(&["AOC_THEME_UI_SUCCESS", "AOC_THEME_GREEN"])?,
        warn: env_color_any(&["AOC_THEME_UI_WARNING", "AOC_THEME_YELLOW"])?,
        critical: env_color_any(&["AOC_THEME_UI_DANGER", "AOC_THEME_RED"])?,
        info: env_color_any(&["AOC_THEME_UI_INFO", "AOC_THEME_CYAN"])?,
    })
}

fn render_ui(frame: &mut ratatui::Frame, app: &App) {
    let size = frame.size();
    let theme = app
        .config
        .pulse_custom_theme
        .unwrap_or_else(|| pulse_theme(app.config.pulse_theme));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(size);
    if !app.config.runtime_mode.is_pulse_pane()
        && app.mode == Mode::Overview
        && app.config.overview_enabled
    {
        render_overview_panel(frame, app, theme, layout[0]);
    } else {
        frame.render_widget(render_body(app, theme, size.width), layout[0]);
    }
    if app.help_open {
        render_help_overlay(frame, app, theme);
    }
}

fn render_body(app: &App, theme: PulseTheme, width: u16) -> Paragraph<'static> {
    let compact = is_compact(width);
    let lines = match app.mode {
        Mode::PulsePane => render_pulse_pane_lines(app, theme, compact, width),
        Mode::Overview => Vec::new(),
        Mode::Overseer => render_overseer_lines(app, theme, compact),
        Mode::Mind => render_mind_lines(app, theme, compact),
        Mode::Fleet => render_fleet_lines(app, theme, compact),
        Mode::Work => render_work_lines(app, theme, compact),
        Mode::Diff => render_diff_lines(app, theme, compact, width),
        Mode::Health => render_health_lines(app, theme, compact),
    };
    let panel_title = if app.mode == Mode::PulsePane {
        "AOC Pulse".to_string()
    } else if app.mode == Mode::Mind {
        "✦ Mind / Insight".to_string()
    } else if app.mode == Mode::Fleet {
        "Detached Fleet".to_string()
    } else if app.mode == Mode::Overseer {
        "Session Overseer".to_string()
    } else {
        app.mode.title().to_string()
    };
    Paragraph::new(Text::from(lines))
        .style(Style::default().fg(theme.text).bg(theme.surface))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.surface))
                .title(Span::styled(
                    panel_title,
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
    let title = format!("Overview [{}]", app.overview_sort_mode.label());
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
                    title,
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
                app, row, theme, compact, area.width,
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
                    title,
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                )),
        );
    frame.render_stateful_widget(list, area, &mut state);
}

fn overview_row_spans(
    app: &App,
    row: &OverviewRow,
    theme: PulseTheme,
    compact: bool,
    width: u16,
) -> Vec<Span<'static>> {
    let decorations = OverviewDecorations {
        attention_chip: attention_chip_from_row(row),
        context: app.overview_context_hint(row),
        task_signal: app.overview_task_signal(row),
        git_signal: app.overview_git_signal(row),
    };
    let presenter = overview_row_presenter(row, &decorations, compact, width);
    let lifecycle_color = lifecycle_color(&row.lifecycle, row.online, theme);
    let age_color = age_color(row.age_secs, row.online, theme);
    let badge_color = overview_badge_color(presenter.badge, theme);
    let mut spans = vec![
        Span::styled(
            presenter.badge.bracketed(),
            Style::default()
                .fg(badge_color)
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
        Span::styled(presenter.freshness, Style::default().fg(age_color)),
    ];
    if let Some(task_signal) = presenter.task_signal {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(task_signal, Style::default().fg(theme.info)));
    }
    if let Some(git_signal) = presenter.git_signal {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            git_signal.clone(),
            Style::default().fg(if git_signal.contains("+") || git_signal.contains('?') {
                theme.warn
            } else {
                theme.muted
            }),
        ));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        presenter.context,
        Style::default().fg(theme.muted),
    ));
    spans
}

#[derive(Clone, Debug)]
struct OverviewDecorations {
    attention_chip: AttentionChip,
    context: String,
    task_signal: Option<String>,
    git_signal: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttentionChip {
    Err,
    Needs,
    Blocked,
    Stale,
    Drift,
    Ok,
}

impl AttentionChip {
    fn label(self) -> &'static str {
        match self {
            AttentionChip::Err => "ERR",
            AttentionChip::Needs => "NEEDS",
            AttentionChip::Blocked => "BLOCK",
            AttentionChip::Stale => "STALE",
            AttentionChip::Drift => "DRIFT",
            AttentionChip::Ok => "OK",
        }
    }

    fn severity(self) -> u8 {
        match self {
            AttentionChip::Err => 0,
            AttentionChip::Needs => 1,
            AttentionChip::Blocked => 2,
            AttentionChip::Stale => 3,
            AttentionChip::Drift => 4,
            AttentionChip::Ok => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverviewBadge {
    Attention(AttentionChip),
}

impl OverviewBadge {
    fn bracketed(self) -> String {
        match self {
            OverviewBadge::Attention(chip) => format!("[{}]", chip.label()),
        }
    }
}

fn attention_chip_from_row(row: &OverviewRow) -> AttentionChip {
    if !row.online {
        return AttentionChip::Stale;
    }
    match normalize_lifecycle(&row.lifecycle).as_str() {
        "error" => AttentionChip::Err,
        "needs-input" => AttentionChip::Needs,
        "blocked" => AttentionChip::Blocked,
        _ => {
            if source_chip_from_row(&row.source) == SourceChip::Mixed {
                AttentionChip::Drift
            } else {
                AttentionChip::Ok
            }
        }
    }
}

fn attention_chip_color(chip: AttentionChip, theme: PulseTheme) -> Color {
    match chip {
        AttentionChip::Err => theme.critical,
        AttentionChip::Needs => theme.warn,
        AttentionChip::Blocked => theme.warn,
        AttentionChip::Stale => theme.critical,
        AttentionChip::Drift => theme.info,
        AttentionChip::Ok => theme.ok,
    }
}

fn overview_badge_color(chip: OverviewBadge, theme: PulseTheme) -> Color {
    match chip {
        OverviewBadge::Attention(value) => attention_chip_color(value, theme),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceChip {
    Hub,
    Local,
    Mixed,
}

#[derive(Clone, Debug)]
struct OverviewRowPresenter {
    identity: String,
    lifecycle_chip: String,
    location_chip: String,
    badge: OverviewBadge,
    freshness: String,
    context: String,
    task_signal: Option<String>,
    git_signal: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct PresenterBudgets {
    label: usize,
    pane: usize,
    tab_name: usize,
    context: usize,
    include_task_signal: bool,
    include_git_signal: bool,
    include_meter: bool,
}

fn overview_row_presenter(
    row: &OverviewRow,
    decorations: &OverviewDecorations,
    compact: bool,
    width: u16,
) -> OverviewRowPresenter {
    let mut plans = vec![PresenterBudgets {
        label: if compact { 14 } else { 20 },
        pane: if compact { 8 } else { 12 },
        tab_name: if compact { 8 } else { 14 },
        context: if compact { 20 } else { 28 },
        include_task_signal: true,
        include_git_signal: !compact,
        include_meter: !compact,
    }];
    plans.push(PresenterBudgets {
        include_git_signal: false,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        include_task_signal: false,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 6,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 0,
        context: 16,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 0,
        label: 12,
        context: 12,
        ..plans[0]
    });
    plans.push(PresenterBudgets {
        tab_name: 0,
        label: 8,
        pane: 6,
        context: 10,
        ..plans[0]
    });

    let max_width = width.saturating_sub(8) as usize;
    for plan in plans {
        let presenter = overview_row_presenter_with_budget(row, decorations, plan);
        if presenter_text_len(&presenter) <= max_width.max(28) {
            return presenter;
        }
    }
    overview_row_presenter_with_budget(
        row,
        decorations,
        PresenterBudgets {
            label: 8,
            pane: 6,
            tab_name: 0,
            context: 8,
            include_task_signal: false,
            include_git_signal: false,
            include_meter: false,
        },
    )
}

fn overview_row_presenter_with_budget(
    row: &OverviewRow,
    decorations: &OverviewDecorations,
    budget: PresenterBudgets,
) -> OverviewRowPresenter {
    let identity = format!(
        "{}::{}",
        ellipsize(&row.label, budget.label.max(4)),
        ellipsize(&row.pane_id, budget.pane.max(4))
    );
    let lifecycle_chip = format!("[{:<5}]", lifecycle_chip_label(&row.lifecycle, row.online));
    let location_chip = overview_location_chip(row, budget.tab_name);
    let badge = OverviewBadge::Attention(decorations.attention_chip);
    let freshness = if budget.include_meter {
        format!(
            "HB:{} {}",
            age_meter(row.age_secs, row.online),
            format_age(row.age_secs)
        )
    } else {
        format_age(row.age_secs)
    };
    let context = format!(
        "M:{}",
        ellipsize(&decorations.context, budget.context.max(8))
    );

    OverviewRowPresenter {
        identity,
        lifecycle_chip,
        location_chip,
        badge,
        freshness,
        context,
        task_signal: budget
            .include_task_signal
            .then(|| decorations.task_signal.clone())
            .flatten(),
        git_signal: budget
            .include_git_signal
            .then(|| decorations.git_signal.clone())
            .flatten(),
    }
}

fn overview_location_chip(row: &OverviewRow, tab_name_budget: usize) -> String {
    let focused_suffix = if row.tab_focused { "*" } else { "" };
    let tab_name = row
        .tab_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(tab_index) = row.tab_index else {
        if let Some(name) = tab_name {
            if tab_name_budget == 0 {
                return format!("T?{focused_suffix}");
            }
            return format!("T?:{}{}", ellipsize(name, tab_name_budget), focused_suffix);
        }
        return "T?:???".to_string();
    };
    if tab_name_budget == 0 {
        return format!("T{tab_index}{focused_suffix}");
    }
    if let Some(tab_name) = tab_name {
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

fn presenter_text_len(presenter: &OverviewRowPresenter) -> usize {
    let mut len = presenter.badge.bracketed().chars().count()
        + 1
        + presenter.identity.chars().count()
        + 1
        + presenter.lifecycle_chip.chars().count()
        + 1
        + presenter.location_chip.chars().count()
        + 1
        + presenter.freshness.chars().count()
        + 1
        + presenter.context.chars().count();
    if let Some(task_signal) = presenter.task_signal.as_ref() {
        len += 1 + task_signal.chars().count();
    }
    if let Some(git_signal) = presenter.git_signal.as_ref() {
        len += 1 + git_signal.chars().count();
    }
    len
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
        Line::from(if app.config.overview_enabled {
            "  1/2/3/4/5/6/7 switch mode (Overview/Overseer/Mind/Fleet/Work/Diff/Health)"
        } else {
            "  2/3/4/5/6/7 switch mode (Overseer/Mind/Fleet/Work/Diff/Health)"
        }),
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
        Mode::PulsePane => vec![
            Line::from(Span::styled(
                "AOC Pulse Pane",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll local pulse summary"),
            Line::from("  g        jump to top"),
            Line::from("  r        refresh local snapshot while hub catches up"),
            Line::from("  ?        show help"),
            Line::from("  q        quit pane"),
        ],
        Mode::Overview => {
            if !app.config.overview_enabled {
                return vec![
                    Line::from(Span::styled(
                        "Overview Deprecated",
                        Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
                    )),
                    Line::from("  Overview display and local polling are disabled."),
                    Line::from(
                        "  Use Overseer/Mind/Work/Diff/Health modes for current operations.",
                    ),
                ];
            }
            vec![
                Line::from(Span::styled(
                    "Overview Mode",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from("  j/k      select agent row (>> + reverse)"),
                Line::from("  g        jump to first agent"),
                Line::from("  Enter    focus selected tab; unmapped -> pane note"),
                Line::from("  e        capture selected pane evidence"),
                Line::from("  E        open live pane follow"),
                Line::from("  x        request stop selected agent"),
                Line::from("  a        toggle sort (layout/attention)"),
                Line::from("  o        request manual observer run"),
            ]
        }
        Mode::Overseer => vec![
            Line::from(Span::styled(
                "Session Overseer Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll overseer snapshot and timeline"),
            Line::from("  g        jump to top"),
            Line::from("  Enter    focus selected worker tab"),
            Line::from("  e        capture selected worker pane evidence"),
            Line::from("  E        open live pane follow"),
            Line::from("  x        stop selected worker"),
            Line::from("  c        request peer review for selected worker"),
            Line::from("  u        request peer unblock/help for selected worker"),
            Line::from("  s        spawn a fresh worker tab"),
            Line::from("  d        delegate selected worker into a new tab + brief"),
            Line::from("  o        request fresh observer run for selected worker"),
            Line::from("  r        refresh local snapshot while hub catches up"),
        ],
        Mode::Mind => vec![
            Line::from(Span::styled(
                "✦ Mind/Insight Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      scroll observer timeline"),
            Line::from("  g        jump to top"),
            Line::from("  e        capture focused pane evidence"),
            Line::from("  E        open live pane follow"),
            Line::from("  o        request manual observer run (T1)"),
            Line::from("  O        run insight_dispatch chain (T1->T2)"),
            Line::from("  b / B    bootstrap dry-run / seed enqueue"),
            Line::from("  F        force finalize session"),
            Line::from("  C        rebuild/requeue latest compaction checkpoint"),
            Line::from("  R        requeue latest T3 export slice"),
            Line::from("  H        rebuild handshake baseline"),
            Line::from("  /        edit local project Mind search query"),
            Line::from("  t        toggle lane (t0/t1/t2/t3/all)"),
            Line::from("  v        toggle scope (active tab/all tabs)"),
            Line::from("  p        toggle provenance drilldown"),
        ],
        Mode::Fleet => vec![
            Line::from(Span::styled(
                "Detached Fleet Mode",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  j/k      select fleet group"),
            Line::from("  Left/Right or [/]  select job within the group"),
            Line::from("  g        jump to top of groups + jobs"),
            Line::from("  Enter    focus a live tab for the selected project"),
            Line::from("  i        launch inspect follow-up tab + brief"),
            Line::from("  h        launch handoff follow-up tab + brief"),
            Line::from("  x        cancel selected active detached job"),
            Line::from("  f        toggle plane filter (all/delegated/mind)"),
            Line::from("  S        toggle sort (project/newest/active-first/error-first)"),
            Line::from("  A        toggle active-only groups"),
            Line::from("  grouped  by project root and ownership plane"),
            Line::from("  lower    drilldown shows selected group details + recent jobs"),
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

fn render_overseer_lines(app: &App, theme: PulseTheme, compact: bool) -> Vec<Line<'static>> {
    let workers = app.overseer_workers();
    let timeline = app.overseer_timeline();
    let generated_at = app
        .overseer_snapshot()
        .and_then(|snapshot| snapshot.generated_at_ms)
        .and_then(ms_to_datetime);
    let artifact_drilldown =
        load_mind_artifact_drilldown(&app.config.project_root, &app.config.session_id);
    let checkpoint = artifact_drilldown.latest_compaction_checkpoint.as_ref();

    if workers.is_empty() && timeline.is_empty() {
        return vec![
            Line::from(Span::styled(
                "No overseer snapshot received yet.",
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                "Waiting for hub observer_snapshot / observer_timeline topics.",
                Style::default().fg(theme.muted),
            )),
        ];
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Workers ",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}", workers.len()),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · timeline "),
        Span::styled(
            format!("{}", timeline.len()),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · generated "),
        Span::styled(
            generated_at
                .map(|value| value.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            Style::default().fg(theme.muted),
        ),
    ])];

    if let Some(snapshot) = app
        .overseer_snapshot()
        .and_then(|snapshot| snapshot.degraded_reason.as_ref())
    {
        lines.push(Line::from(Span::styled(
            format!("degraded: {snapshot}"),
            Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Workers",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));

    for worker in workers.iter().take(if compact { 8 } else { 12 }) {
        let mind_event = app.overseer_mind_event(&worker.agent_id);
        let consultation_packet =
            derive_overseer_consultation_packet(worker, checkpoint, mind_event);
        lines.push(Line::from(render_overseer_worker_line(
            worker, mind_event, theme, compact,
        )));
        if let Some(summary) = worker
            .summary
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(Line::from(Span::styled(
                format!(
                    "    {}",
                    truncate_text(summary, if compact { 72 } else { 110 })
                ),
                Style::default().fg(theme.muted),
            )));
        }
        if should_render_overseer_attention_reason(worker) {
            if let Some(reason) = worker
                .attention
                .reason
                .as_ref()
                .filter(|value| !value.trim().is_empty())
            {
                lines.push(Line::from(Span::styled(
                    format!(
                        "    attention: {}",
                        truncate_text(reason, if compact { 68 } else { 104 })
                    ),
                    Style::default().fg(theme.warn),
                )));
            }
        }
        if let Some(event) = mind_event {
            if let Some(line) = render_overseer_mind_line(event, theme, compact) {
                lines.push(line);
            }
        }
        if should_render_overseer_consultation_line(&consultation_packet, worker) {
            if let Some(line) =
                render_overseer_consultation_line(&consultation_packet, worker, theme, compact)
            {
                lines.push(line);
            }
        }
    }

    let tools = app.orchestrator_tools();
    if !tools.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Mission Control tools",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        for tool in tools.iter().take(if compact { 5 } else { 8 }) {
            lines.push(render_orchestrator_tool_line(tool, theme, compact));
        }

        let graph = app.orchestration_graph_ir();
        if !graph.compile_paths.is_empty() {
            lines.push(render_orchestration_graph_summary_line(&graph, theme));
            lines.push(Line::from(Span::styled(
                "Reviewable compile",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )));
            for path in graph.compile_paths.iter().take(if compact { 2 } else { 6 }) {
                lines.push(render_orchestration_compile_line(path, theme, compact));
            }
        }
    }

    if !timeline.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recent timeline",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        for entry in timeline.iter().take(if compact { 6 } else { 10 }) {
            lines.push(Line::from(render_overseer_timeline_line(entry, theme)));
        }
    }

    lines
}

fn render_overseer_worker_line(
    worker: &WorkerSnapshot,
    mind_event: Option<&MindObserverFeedEvent>,
    theme: PulseTheme,
    compact: bool,
) -> Vec<Span<'static>> {
    let scope = worker
        .role
        .clone()
        .unwrap_or_else(|| extract_label(&worker.agent_id));
    let task = worker
        .assignment
        .task_id
        .clone()
        .or_else(|| worker.assignment.tag.clone())
        .unwrap_or_else(|| "unassigned".to_string());
    let progress = worker
        .progress
        .percent
        .map(|value| format!("{}%", value))
        .unwrap_or_else(|| format!("{:?}", worker.progress.phase).to_ascii_lowercase());
    let align = format!("{:?}", worker.plan_alignment).to_ascii_lowercase();
    let drift = format!("{:?}", worker.drift_risk).to_ascii_lowercase();
    let status = format!("{:?}", worker.status).to_ascii_lowercase();
    let mut spans = vec![
        Span::styled(
            format!("[{}]", overseer_attention_label(worker.attention.level)),
            Style::default()
                .fg(overseer_attention_color(worker.attention.level, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            scope,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" ({}) ", worker.pane_id)),
        Span::styled(
            status,
            Style::default().fg(lifecycle_color_for_worker(worker, theme)),
        ),
        Span::raw(" · "),
        Span::styled(task, Style::default().fg(theme.info)),
        Span::raw(" · "),
        Span::styled(progress, Style::default().fg(theme.text)),
        Span::raw(" · "),
        Span::styled(
            format!("align:{align}"),
            Style::default().fg(overseer_plan_alignment_color(worker.plan_alignment, theme)),
        ),
        Span::raw(" "),
        Span::styled(
            format!("drift:{drift}"),
            Style::default().fg(overseer_drift_color(worker.drift_risk, theme)),
        ),
    ];
    if let Some(duplicate) = worker.duplicate_work.as_ref() {
        let overlap_count =
            duplicate.overlapping_task_ids.len() + duplicate.overlapping_files.len();
        if overlap_count > 0 {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("dup:{overlap_count}"),
                Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
            ));
        }
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        overseer_provenance_label(worker, mind_event),
        Style::default().fg(overseer_provenance_color(mind_event, theme)),
    ));
    if !compact {
        if let Some(branch) = worker
            .branch
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            spans.push(Span::raw(" · "));
            spans.push(Span::styled(
                branch.clone(),
                Style::default().fg(theme.muted),
            ));
        }
    }
    spans
}

fn derive_overseer_consultation_packet(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    mind_event: Option<&MindObserverFeedEvent>,
) -> ConsultationPacket {
    let mut degraded_inputs = Vec::new();
    if checkpoint.is_none() {
        degraded_inputs.push("mind.compaction_checkpoint".to_string());
    }
    if mind_event.is_none() {
        degraded_inputs.push("mind.t1".to_string());
    }
    if worker
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        degraded_inputs.push("overseer.summary".to_string());
    }

    let source_status = if matches!(worker.status, WorkerStatus::Offline) {
        ConsultationSourceStatus::Stale
    } else if !degraded_inputs.is_empty() {
        ConsultationSourceStatus::Partial
    } else {
        ConsultationSourceStatus::Complete
    };

    ConsultationPacket {
        packet_id: format!("mc:{}:{}", worker.session_id, worker.agent_id),
        kind: ConsultationPacketKind::Align,
        identity: ConsultationIdentity {
            session_id: worker.session_id.clone(),
            agent_id: worker.agent_id.clone(),
            pane_id: Some(worker.pane_id.clone()),
            conversation_id: checkpoint.map(|value| value.conversation_id.clone()),
            role: worker.role.clone(),
        },
        task_context: ConsultationTaskContext {
            active_tag: worker.assignment.tag.clone(),
            task_ids: worker.assignment.task_id.iter().cloned().collect(),
            focus_summary: worker.summary.clone().or_else(|| worker.blocker.clone()),
        },
        summary: worker
            .summary
            .clone()
            .or_else(|| worker.blocker.clone())
            .or_else(|| {
                Some(format!(
                    "status={} phase={:?}",
                    format!("{:?}", worker.status).to_ascii_lowercase(),
                    worker.progress.phase
                ))
            }),
        checkpoint: checkpoint.map(|value| ConsultationCheckpointRef {
            checkpoint_id: value.checkpoint_id.clone(),
            conversation_id: Some(value.conversation_id.clone()),
            compaction_entry_id: value.compaction_entry_id.clone(),
            ts: Some(value.ts.to_rfc3339()),
        }),
        freshness: ConsultationFreshness {
            packet_generated_at: Some(Utc::now().to_rfc3339()),
            source_updated_at: worker
                .last_update_at_ms
                .and_then(ms_to_datetime)
                .map(|ts| ts.to_rfc3339()),
            stale_after_ms: worker.stale_after_ms,
            source_status,
            degraded_inputs: degraded_inputs.clone(),
        },
        confidence: ConsultationConfidence {
            overall_bps: Some(overseer_consultation_confidence_bps(
                worker, checkpoint, mind_event,
            )),
            rationale: Some(overseer_consultation_rationale(
                worker, checkpoint, mind_event,
            )),
        },
        help_request: overseer_help_request(worker),
        degraded_reason: (!degraded_inputs.is_empty()).then(|| {
            format!(
                "packet derived with partial inputs: {}",
                degraded_inputs.join(", ")
            )
        }),
        ..Default::default()
    }
    .normalize()
}

fn overseer_help_request(worker: &WorkerSnapshot) -> Option<ConsultationHelpRequest> {
    if matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        return Some(ConsultationHelpRequest {
            kind: if matches!(worker.status, WorkerStatus::Blocked) {
                "blocker_escalation".to_string()
            } else {
                "alignment_request".to_string()
            },
            question: worker
                .blocker
                .clone()
                .unwrap_or_else(|| "need bounded manager guidance".to_string()),
            requested_from: Some("mission_control".to_string()),
            urgency: Some(if matches!(worker.status, WorkerStatus::Blocked) {
                "high".to_string()
            } else {
                "medium".to_string()
            }),
        });
    }
    None
}

fn should_render_overseer_attention_reason(worker: &WorkerSnapshot) -> bool {
    if worker
        .attention
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return false;
    }

    if worker.attention.kind.as_deref() == Some("duplicate_work") {
        return false;
    }

    !matches!(worker.attention.level, AttentionLevel::None)
}

fn should_render_overseer_consultation_line(
    packet: &ConsultationPacket,
    worker: &WorkerSnapshot,
) -> bool {
    matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) || matches!(worker.drift_risk, DriftRisk::High)
        || matches!(
            packet.freshness.source_status,
            ConsultationSourceStatus::Partial | ConsultationSourceStatus::Stale
        )
        || worker.assignment.task_id.is_none() && worker.assignment.tag.is_none()
        || packet.help_request.is_some()
}

fn overseer_consultation_confidence_bps(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    mind_event: Option<&MindObserverFeedEvent>,
) -> u16 {
    let mut score = 600u16;
    if checkpoint.is_some() {
        score += 100;
    }
    if mind_event.is_some() {
        score += 100;
    }
    if worker.assignment.task_id.is_some() || worker.assignment.tag.is_some() {
        score += 100;
    }
    if matches!(worker.drift_risk, DriftRisk::High) {
        score = score.saturating_sub(150);
    }
    if matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        score = score.saturating_sub(100);
    }
    score.min(1000)
}

fn overseer_consultation_rationale(
    worker: &WorkerSnapshot,
    checkpoint: Option<&CompactionCheckpoint>,
    mind_event: Option<&MindObserverFeedEvent>,
) -> String {
    let mut parts = Vec::new();
    parts.push(if checkpoint.is_some() {
        "checkpoint linked"
    } else {
        "checkpoint missing"
    });
    parts.push(if mind_event.is_some() {
        "mind signal present"
    } else {
        "mind signal missing"
    });
    parts.push(
        if worker.assignment.task_id.is_some() || worker.assignment.tag.is_some() {
            "task context present"
        } else {
            "task context missing"
        },
    );
    if matches!(worker.drift_risk, DriftRisk::High) {
        parts.push("high drift risk");
    }
    if matches!(
        worker.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        parts.push("operator input needed");
    }
    parts.join(", ")
}

fn render_orchestrator_tool_line(
    tool: &OrchestratorTool,
    theme: PulseTheme,
    compact: bool,
) -> Line<'static> {
    let status_style = match tool.status {
        OrchestratorToolStatus::Ready => Style::default().fg(theme.ok),
        OrchestratorToolStatus::Unavailable => Style::default().fg(theme.warn),
    }
    .add_modifier(Modifier::BOLD);
    let status_label = match tool.status {
        OrchestratorToolStatus::Ready => "ready",
        OrchestratorToolStatus::Unavailable => "blocked",
    };
    let mut spans = vec![
        Span::styled("    tool ", Style::default().fg(theme.muted)),
        Span::styled(format!("[{status_label}]"), status_style),
        Span::raw(" "),
        Span::styled(tool.label.to_string(), Style::default().fg(theme.info)),
    ];
    if let Some(shortcut) = tool.shortcut {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("key:{shortcut}"),
            Style::default().fg(theme.accent),
        ));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        truncate_text(&tool.summary, if compact { 36 } else { 72 }),
        Style::default().fg(theme.text),
    ));
    if let Some(reason) = tool.reason.as_ref() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            truncate_text(reason, if compact { 20 } else { 36 }),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans)
}

fn render_orchestration_graph_summary_line(
    graph: &OrchestrationGraphIr,
    theme: PulseTheme,
) -> Line<'static> {
    Line::from(vec![
        Span::styled("    graph ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{} nodes", graph.nodes.len()),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("{} edges", graph.edges.len()),
            Style::default().fg(theme.info),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("{} review paths", graph.compile_paths.len()),
            Style::default().fg(theme.text),
        ),
    ])
}

fn render_orchestration_compile_line(
    path: &OrchestrationCompilePath,
    theme: PulseTheme,
    compact: bool,
) -> Line<'static> {
    let status_style = match path.status {
        OrchestratorToolStatus::Ready => Style::default().fg(theme.ok),
        OrchestratorToolStatus::Unavailable => Style::default().fg(theme.warn),
    }
    .add_modifier(Modifier::BOLD);
    let status_label = match path.status {
        OrchestratorToolStatus::Ready => "ready",
        OrchestratorToolStatus::Unavailable => "blocked",
    };
    let preview = truncate_text(&path.steps.join(" -> "), if compact { 52 } else { 96 });
    Line::from(vec![
        Span::styled("    plan ", Style::default().fg(theme.muted)),
        Span::styled(format!("[{status_label}]"), status_style),
        Span::raw(" "),
        Span::styled(path.review_label.clone(), Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::styled(preview, Style::default().fg(theme.text)),
    ])
}

fn render_overseer_consultation_line(
    packet: &ConsultationPacket,
    worker: &WorkerSnapshot,
    theme: PulseTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let suggestion = if matches!(worker.status, WorkerStatus::Blocked) {
        "ask for unblock plan + evidence-backed next step".to_string()
    } else if matches!(worker.status, WorkerStatus::NeedsInput) {
        "send alignment prompt with explicit decision request".to_string()
    } else if matches!(worker.drift_risk, DriftRisk::High) {
        "request concise alignment + validation plan".to_string()
    } else if worker.assignment.task_id.is_none() && worker.assignment.tag.is_none() {
        "assign task/tag before further implementation".to_string()
    } else if packet.freshness.source_status == ConsultationSourceStatus::Stale {
        "request fresh status update before steering".to_string()
    } else {
        "continue current lane; request validation if milestone reached".to_string()
    };

    let meta = format!(
        "src:{} conf:{}",
        format!("{:?}", packet.freshness.source_status).to_ascii_lowercase(),
        packet.confidence.overall_bps.unwrap_or_default()
    );
    Some(Line::from(vec![
        Span::styled("    mc ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("[{}]", consultation_packet_kind_label(packet.kind)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate_text(&suggestion, if compact { 48 } else { 84 }),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(meta, Style::default().fg(theme.muted)),
    ]))
}

fn consultation_packet_kind_label(kind: ConsultationPacketKind) -> &'static str {
    match kind {
        ConsultationPacketKind::Summary => "summary",
        ConsultationPacketKind::Plan => "plan",
        ConsultationPacketKind::Blockers => "blockers",
        ConsultationPacketKind::Review => "review",
        ConsultationPacketKind::Align => "align",
        ConsultationPacketKind::CheckpointStatus => "checkpoint",
        ConsultationPacketKind::HelpRequest => "help",
    }
}

fn render_overseer_mind_line(
    event: &MindObserverFeedEvent,
    theme: PulseTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let lane = mind_event_lane(event);
    let mut detail = event
        .reason
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| {
            event
                .failure_kind
                .as_ref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("failure:{value}"))
        })
        .or_else(|| {
            event.progress.as_ref().map(|progress| {
                format!(
                    "tokens:{}→{} next:{}",
                    progress.t0_estimated_tokens,
                    progress.t1_target_tokens,
                    progress.tokens_until_next_run
                )
            })
        })?;
    detail = truncate_text(&detail, if compact { 58 } else { 92 });
    Some(Line::from(vec![
        Span::styled("    semantic ", Style::default().fg(theme.muted)),
        Span::styled(
            format!(
                "[{}:{}]",
                mind_lane_label(lane),
                mind_status_label(event.status)
            ),
            Style::default()
                .fg(mind_status_color(event.status, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(detail, Style::default().fg(theme.info)),
    ]))
}

fn render_overseer_timeline_line(
    entry: &ObserverTimelineEntry,
    theme: PulseTheme,
) -> Vec<Span<'static>> {
    let when = entry
        .emitted_at_ms
        .and_then(ms_to_datetime)
        .map(|value| value.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".to_string());
    let kind = format!("{:?}", entry.kind).to_ascii_lowercase();
    let mut spans = vec![
        Span::styled(when, Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(entry.agent_id.clone(), Style::default().fg(theme.info)),
        Span::raw(" "),
        Span::styled(
            kind,
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(summary) = entry
        .summary
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(
            summary.clone(),
            Style::default().fg(theme.text),
        ));
    }
    if let Some(reason) = entry
        .reason
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(
            reason.clone(),
            Style::default().fg(theme.muted),
        ));
    }
    spans
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn overseer_attention_rank(level: &AttentionLevel) -> usize {
    match level {
        AttentionLevel::Critical => 4,
        AttentionLevel::Warn => 3,
        AttentionLevel::Info => 2,
        AttentionLevel::None => 1,
    }
}

fn overseer_drift_rank(risk: &DriftRisk) -> usize {
    match risk {
        DriftRisk::High => 4,
        DriftRisk::Medium => 3,
        DriftRisk::Low => 2,
        DriftRisk::Unknown => 1,
    }
}

fn overseer_attention_label(level: AttentionLevel) -> &'static str {
    match level {
        AttentionLevel::Critical => "critical",
        AttentionLevel::Warn => "warn",
        AttentionLevel::Info => "info",
        AttentionLevel::None => "ok",
    }
}

fn overseer_provenance_label(
    worker: &WorkerSnapshot,
    mind_event: Option<&MindObserverFeedEvent>,
) -> String {
    let base = worker
        .provenance
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(match worker.source {
            aoc_core::session_overseer::OverseerSourceKind::Wrapper => "heuristic:wrapper",
            aoc_core::session_overseer::OverseerSourceKind::Hub => "heuristic:hub",
            aoc_core::session_overseer::OverseerSourceKind::Mind => "semantic:mind",
            aoc_core::session_overseer::OverseerSourceKind::Manager => "heuristic:manager",
            aoc_core::session_overseer::OverseerSourceKind::LocalFallback => "heuristic:local",
        });
    if let Some(event) = mind_event {
        format!(
            "[prov:{}+mind:{}:{}]",
            base,
            mind_lane_label(mind_event_lane(event)),
            mind_status_label(event.status)
        )
    } else {
        format!("[prov:{base}]")
    }
}

fn overseer_provenance_color(
    mind_event: Option<&MindObserverFeedEvent>,
    theme: PulseTheme,
) -> Color {
    if let Some(event) = mind_event {
        mind_status_color(event.status, theme)
    } else {
        theme.muted
    }
}

fn overseer_attention_color(level: AttentionLevel, theme: PulseTheme) -> Color {
    match level {
        AttentionLevel::Critical => theme.critical,
        AttentionLevel::Warn => theme.warn,
        AttentionLevel::Info => theme.info,
        AttentionLevel::None => theme.ok,
    }
}

fn overseer_plan_alignment_color(level: PlanAlignment, theme: PulseTheme) -> Color {
    match level {
        PlanAlignment::High => theme.ok,
        PlanAlignment::Medium => theme.info,
        PlanAlignment::Low => theme.warn,
        PlanAlignment::Unassigned => theme.warn,
        PlanAlignment::Unknown => theme.muted,
    }
}

fn overseer_drift_color(level: DriftRisk, theme: PulseTheme) -> Color {
    match level {
        DriftRisk::High => theme.critical,
        DriftRisk::Medium => theme.warn,
        DriftRisk::Low => theme.ok,
        DriftRisk::Unknown => theme.muted,
    }
}

fn lifecycle_color_for_worker(worker: &WorkerSnapshot, theme: PulseTheme) -> Color {
    match worker.status {
        WorkerStatus::Done => theme.ok,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput => theme.warn,
        WorkerStatus::Offline => theme.muted,
        WorkerStatus::Active => theme.info,
        WorkerStatus::Paused | WorkerStatus::Idle => theme.muted,
    }
}

fn render_pulse_pane_lines(
    app: &App,
    theme: PulseTheme,
    compact: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let project_root = app.config.project_root.to_string_lossy().to_string();
    let viewer_scope = app.config.tab_scope.as_deref();
    let overview_rows = app.overview_rows();

    let focus_row = overview_rows
        .iter()
        .find(|row| row.tab_focused)
        .cloned()
        .or_else(|| {
            overview_rows
                .iter()
                .find(|row| row.project_root == project_root)
                .cloned()
        });

    let status_label = if app.connected {
        "connected"
    } else if app.has_any_hub_data() {
        "reconnecting"
    } else {
        "offline"
    };
    let status_color = if app.connected {
        theme.ok
    } else if app.has_any_hub_data() {
        theme.warn
    } else {
        theme.critical
    };
    let scope_label = viewer_scope.unwrap_or("current-tab");
    lines.push(Line::from(vec![
        Span::styled("scope:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            scope_label.to_string(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("hub:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            status_label,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("mode:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled("pulse-pane", Style::default().fg(theme.info)),
    ]));

    if let Some(row) = focus_row.as_ref() {
        let lifecycle_color = lifecycle_color(&row.lifecycle, row.online, theme);
        let age_label = row
            .age_secs
            .map(|secs| format!("age:{}s", secs.max(0)))
            .unwrap_or_else(|| "age:n/a".to_string());
        lines.push(Line::from(vec![
            Span::styled(
                row.label.clone(),
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", normalize_lifecycle(&row.lifecycle)),
                Style::default().fg(lifecycle_color),
            ),
            Span::raw(" "),
            Span::styled(age_label, Style::default().fg(theme.muted)),
        ]));
        if let Some(snippet) = row
            .snippet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            lines.push(Line::from(vec![
                Span::raw("  -> "),
                Span::styled(
                    ellipsize(snippet, if compact { 50 } else { 88 }),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }
    }

    let tab_lines = render_pulse_tab_section(app, &overview_rows, theme, compact, width);
    if !tab_lines.is_empty() {
        lines.push(Line::from(""));
        lines.extend(tab_lines);
    }

    let mut work_projects = app
        .work_rows()
        .into_iter()
        .filter(|project| project.project_root == project_root)
        .collect::<Vec<_>>();
    if let Some(project) = work_projects.pop() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Tasks",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        )));
        for tag in project.tags.into_iter().take(if compact { 2 } else { 3 }) {
            let mut spans = vec![
                Span::raw("  "),
                Span::styled(
                    ellipsize(&tag.tag, 18),
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ];
            spans.extend(task_bar_spans(
                &tag.counts,
                if compact { 10 } else { 14 },
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
            if let Some(title) = tag.in_progress_titles.first() {
                lines.push(Line::from(vec![
                    Span::raw("    -> "),
                    Span::styled(
                        ellipsize(title, if compact { 46 } else { 76 }),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
        }
    }

    let mind_rows = app.mind_rows();
    let injection_rows = app.mind_injection_rows();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Mind",
        Style::default()
            .fg(theme.title)
            .add_modifier(Modifier::BOLD),
    )));
    let status_rollup = mind_status_rollup(&mind_rows);
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("q:{}", status_rollup.queued),
            Style::default().fg(theme.warn),
        ),
        Span::raw(" "),
        Span::styled(
            format!("run:{}", status_rollup.running),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(
            format!("ok:{}", status_rollup.success),
            Style::default().fg(theme.ok),
        ),
        Span::raw(" "),
        Span::styled(
            format!("fb:{}", status_rollup.fallback),
            Style::default().fg(theme.warn),
        ),
        Span::raw(" "),
        Span::styled(
            format!("err:{}", status_rollup.error),
            Style::default().fg(theme.critical),
        ),
    ]));
    if let Some(line) = render_mind_injection_rollup_line(&injection_rows, theme, compact) {
        lines.push(Line::from(vec![Span::raw("  ")]));
        lines.push(line);
    }
    if let Some(row) = mind_rows.first() {
        let status_label = mind_status_label(row.event.status);
        let trigger_label = mind_trigger_label(row.event.trigger);
        let lane = mind_event_lane(&row.event);
        let lane_label = mind_lane_label(lane);
        let when = row
            .event
            .completed_at
            .as_deref()
            .or(row.event.started_at.as_deref())
            .or(row.event.enqueued_at.as_deref())
            .and_then(mind_timestamp_label)
            .unwrap_or_else(|| "--:--:--".to_string());
        lines.push(Line::from(vec![
            Span::raw("  latest: "),
            Span::styled(
                format!("[{}]", lane_label),
                Style::default()
                    .fg(mind_lane_color(lane, theme))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", status_label),
                Style::default()
                    .fg(mind_status_color(row.event.status, theme))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", trigger_label),
                Style::default().fg(theme.info),
            ),
            Span::raw(" "),
            Span::styled(format!("@{}", when), Style::default().fg(theme.muted)),
        ]));
        let context = row
            .event
            .reason
            .clone()
            .or_else(|| row.event.failure_kind.clone())
            .or_else(|| row.event.conversation_id.clone())
            .unwrap_or_else(|| format!("source:{} agent:{}", row.source, row.agent_id));
        lines.push(Line::from(vec![
            Span::raw("    -> "),
            Span::styled(
                ellipsize(&context, if compact { 48 } else { 84 }),
                Style::default().fg(theme.muted),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "No local Mind activity yet.",
                Style::default().fg(theme.muted),
            ),
        ]));
    }

    let mut diff_projects = app
        .diff_rows()
        .into_iter()
        .filter(|project| project.project_root == project_root)
        .collect::<Vec<_>>();
    if let Some(project) = diff_projects.pop() {
        let churn = project.summary.staged.additions
            + project.summary.staged.deletions
            + project.summary.unstaged.additions
            + project.summary.unstaged.deletions;
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Repo",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                fit_fields(
                    &[
                        format!("stg:{}", project.summary.staged.files),
                        format!("uns:{}", project.summary.unstaged.files),
                        format!("new:{}", project.summary.untracked.files),
                        format!("churn:{}", churn),
                    ],
                    width.saturating_sub(6) as usize,
                ),
                Style::default().fg(theme.muted),
            ),
        ]));
    }

    let health = app
        .health_rows()
        .into_iter()
        .find(|row| row.project_root == project_root)
        .map(|row| row.snapshot)
        .unwrap_or_else(|| app.local.health.clone());
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "Health",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            ellipsize(&health.taskmaster_status, if compact { 42 } else { 72 }),
            Style::default().fg(if health.taskmaster_status.contains("available") {
                theme.ok
            } else {
                theme.warn
            }),
        ),
    ]));

    lines
}

fn render_fleet_lines(app: &App, theme: PulseTheme, compact: bool) -> Vec<Line<'static>> {
    let rows = app.detached_fleet_rows();
    let mut lines = Vec::new();

    if rows.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "Detached fleet",
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("hub data unavailable", Style::default().fg(theme.muted)),
        ]));
        lines.push(Line::from(Span::styled(
            "No detached job snapshots published yet. Launch detached specialists or reconnect Mission Control to the session hub.",
            Style::default().fg(theme.muted),
        )));
        return lines;
    }

    let total_jobs: usize = rows.iter().map(|row| row.jobs.len()).sum();
    let delegated_jobs: usize = rows
        .iter()
        .filter(|row| matches!(row.owner_plane, InsightDetachedOwnerPlane::Delegated))
        .map(|row| row.jobs.len())
        .sum();
    let mind_jobs = total_jobs.saturating_sub(delegated_jobs);
    let selected = app.selected_fleet_index_for_rows(&rows);
    lines.push(Line::from(vec![
        Span::styled("groups:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("{}", rows.len()),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("jobs:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("{}", total_jobs),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("plane:{}", app.fleet_plane_filter.label()),
            Style::default().fg(theme.accent),
        ),
        Span::raw("  "),
        Span::styled(
            if app.fleet_active_only {
                "scope:active"
            } else {
                "scope:all"
            },
            Style::default().fg(theme.accent),
        ),
        Span::raw("  "),
        Span::styled(
            format!("sort:{}", app.fleet_sort_mode.label()),
            Style::default().fg(theme.accent),
        ),
        Span::raw("  "),
        Span::styled(
            format!("delegated:{}", delegated_jobs),
            Style::default().fg(theme.accent),
        ),
        Span::raw(" "),
        Span::styled(
            format!("mind:{}", mind_jobs),
            Style::default().fg(theme.accent),
        ),
    ]));

    for (index, row) in rows.iter().enumerate() {
        let latest = match row.jobs.first() {
            Some(job) => job,
            None => continue,
        };
        let is_selected = index == selected;
        let mut queued = 0usize;
        let mut running = 0usize;
        let mut success = 0usize;
        let mut fallback = 0usize;
        let mut error = 0usize;
        let mut cancelled = 0usize;
        let mut stale = 0usize;
        for job in &row.jobs {
            match job.status {
                InsightDetachedJobStatus::Queued => queued += 1,
                InsightDetachedJobStatus::Running => running += 1,
                InsightDetachedJobStatus::Success => success += 1,
                InsightDetachedJobStatus::Fallback => fallback += 1,
                InsightDetachedJobStatus::Error => error += 1,
                InsightDetachedJobStatus::Cancelled => cancelled += 1,
                InsightDetachedJobStatus::Stale => stale += 1,
            }
        }
        let when = latest
            .finished_at_ms
            .or(latest.started_at_ms)
            .unwrap_or(latest.created_at_ms);
        let when = Utc
            .timestamp_millis_opt(when)
            .single()
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "--:--:--".to_string());
        let mut summary_fields = vec![
            format!("jobs:{}", row.jobs.len()),
            format!("q:{queued}"),
            format!("run:{running}"),
            format!("ok:{success}"),
            format!("fb:{fallback}"),
            format!("err:{error}"),
        ];
        if cancelled > 0 {
            summary_fields.push(format!("cx:{cancelled}"));
        }
        if stale > 0 {
            summary_fields.push(format!("stale:{stale}"));
        }
        let summary = fit_fields(&summary_fields, if compact { 42 } else { 76 });
        let latest_label = latest
            .agent
            .as_deref()
            .or(latest.chain.as_deref())
            .or(latest.team.as_deref())
            .unwrap_or("detached-job");
        lines.push(Line::from(vec![
            Span::styled(
                if is_selected { ">>" } else { "  " },
                Style::default().fg(if is_selected {
                    theme.accent
                } else {
                    theme.muted
                }),
            ),
            Span::raw(" "),
            Span::styled(
                detached_owner_plane_label(row.owner_plane),
                Style::default()
                    .fg(if is_selected {
                        theme.accent
                    } else {
                        theme.info
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                ellipsize(&row.project_root, if compact { 28 } else { 52 }),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(summary, Style::default().fg(theme.accent)),
            Span::raw(" "),
            Span::styled(format!("@{when}"), Style::default().fg(theme.muted)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   latest: "),
            Span::styled(
                format!("[{}]", detached_job_status_label(latest.status)),
                Style::default()
                    .fg(detached_job_status_color(latest.status, theme))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                ellipsize(latest_label, if compact { 18 } else { 28 }),
                Style::default().fg(theme.info),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{}", detached_worker_kind_label(latest.worker_kind)),
                Style::default().fg(theme.muted),
            ),
        ]));
    }

    if let Some(row) = rows.get(selected) {
        let selected_job_index = app.selected_fleet_job_index_for_row(row);
        let selected_job = row.jobs.get(selected_job_index);
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "Drilldown",
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{} · {} jobs", row.project_root, row.jobs.len()),
                Style::default().fg(theme.info),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "job:{}/{}",
                    selected_job_index.saturating_add(1),
                    row.jobs.len()
                ),
                Style::default().fg(theme.accent),
            ),
        ]));
        if let Some(job) = selected_job {
            let target = job
                .agent
                .as_deref()
                .or(job.chain.as_deref())
                .or(job.team.as_deref())
                .unwrap_or("detached-job");
            lines.push(Line::from(vec![
                Span::raw("  selected: "),
                Span::styled(job.job_id.clone(), Style::default().fg(theme.accent)),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", detached_job_status_label(job.status)),
                    Style::default()
                        .fg(detached_job_status_color(job.status, theme))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(target.to_string(), Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  plane/kind: "),
                Span::styled(
                    format!(
                        "{} / {}",
                        detached_owner_plane_label(job.owner_plane),
                        detached_worker_kind_label(job.worker_kind)
                    ),
                    Style::default().fg(theme.muted),
                ),
                Span::raw("  "),
                Span::raw("steps: "),
                Span::styled(
                    match (job.current_step_index, job.step_count) {
                        (Some(current), Some(total)) => format!("{current}/{total}"),
                        (_, Some(total)) => format!("?/{total}"),
                        _ => "n/a".to_string(),
                    },
                    Style::default().fg(theme.muted),
                ),
                Span::raw("  "),
                Span::raw("fallback: "),
                Span::styled(
                    if job.fallback_used { "yes" } else { "no" },
                    Style::default().fg(if job.fallback_used {
                        theme.warn
                    } else {
                        theme.ok
                    }),
                ),
            ]));
            if let Some(detail) = job
                .output_excerpt
                .as_deref()
                .or(job.error.as_deref())
                .map(|value| ellipsize(value, if compact { 56 } else { 104 }))
            {
                lines.push(Line::from(vec![
                    Span::raw("  summary: "),
                    Span::styled(detail, Style::default().fg(theme.muted)),
                ]));
            }
            lines.push(Line::from(Span::styled(
                "  recovery:",
                Style::default().fg(theme.title),
            )));
            for guidance in detached_job_recovery_guidance(job)
                .into_iter()
                .take(if compact { 2 } else { 3 })
            {
                lines.push(Line::from(vec![
                    Span::raw("    - "),
                    Span::styled(
                        ellipsize(&guidance, if compact { 64 } else { 116 }),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
            lines.push(Line::from(Span::styled(
                "  recent jobs:",
                Style::default().fg(theme.title),
            )));
            for (index, job) in row
                .jobs
                .iter()
                .take(if compact { 3 } else { 5 })
                .enumerate()
            {
                let label = job
                    .agent
                    .as_deref()
                    .or(job.chain.as_deref())
                    .or(job.team.as_deref())
                    .unwrap_or("detached-job");
                let is_selected_job = index == selected_job_index;
                lines.push(Line::from(vec![
                    Span::styled(
                        if is_selected_job { "    > " } else { "    - " },
                        Style::default().fg(if is_selected_job {
                            theme.accent
                        } else {
                            theme.muted
                        }),
                    ),
                    Span::styled(
                        job.job_id.clone(),
                        Style::default().fg(if is_selected_job {
                            theme.accent
                        } else {
                            theme.info
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("[{}]", detached_job_status_label(job.status)),
                        Style::default().fg(detached_job_status_color(job.status, theme)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        ellipsize(label, if compact { 16 } else { 28 }),
                        Style::default().fg(theme.text),
                    ),
                ]));
            }
        }
    }

    lines
}

fn render_mind_lines(app: &App, theme: PulseTheme, compact: bool) -> Vec<Line<'static>> {
    let rows = app.mind_rows();
    let all_rows = app.mind_rows_for_lane(MindLaneFilter::All);
    let detached_jobs = app.insight_detached_jobs();
    let artifact_snapshot =
        load_mind_artifact_drilldown(&app.config.project_root, &app.config.session_id);
    let mut lines = Vec::new();

    let lane_label = app.mind_lane.label().to_ascii_uppercase();
    let scope_label = if app.config.mind_project_scoped {
        "project"
    } else if app.mind_show_all_tabs {
        "all-tabs"
    } else {
        "active-tab"
    };
    let project_label = app.config.project_root.to_string_lossy().to_string();
    let lane_rollup = mind_lane_rollup(&all_rows);
    let mut header = vec![
        Span::styled("lane:", Style::default().fg(theme.muted)),
        Span::styled(
            lane_label,
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("scope:", Style::default().fg(theme.muted)),
        Span::styled(
            scope_label,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!(
                "t0:{} t1:{} t2:{} t3:{}",
                lane_rollup[0], lane_rollup[1], lane_rollup[2], lane_rollup[3]
            ),
            Style::default().fg(theme.muted),
        ),
    ];

    if let Some(runtime) = app.insight_runtime_rollup() {
        header.push(Span::raw("  "));
        header.push(Span::styled(
            format!(
                "t2q:{} done:{} fail:{} lock:{} | t3q:{} done:{} fail:{} rq:{} dlq:{} lock:{}",
                runtime.queue_depth,
                runtime.reflector_jobs_completed,
                runtime.reflector_jobs_failed,
                runtime.reflector_lock_conflicts,
                runtime.t3_queue_depth,
                runtime.t3_jobs_completed,
                runtime.t3_jobs_failed,
                runtime.t3_jobs_requeued,
                runtime.t3_jobs_dead_lettered,
                runtime.t3_lock_conflicts
            ),
            Style::default().fg(theme.muted),
        ));
    }
    lines.push(Line::from(header));
    lines.push(Line::from(vec![
        Span::styled("project:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            ellipsize(&project_label, if compact { 44 } else { 88 }),
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            if app.config.mind_project_scoped {
                "[project-scoped]"
            } else {
                "[session-scoped]"
            },
            Style::default().fg(theme.muted),
        ),
    ]));

    let status_rollup = mind_status_rollup(&rows);
    lines.push(Line::from(vec![
        Span::styled("status:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("q:{}", status_rollup.queued),
            Style::default().fg(theme.warn),
        ),
        Span::raw(" "),
        Span::styled(
            format!("run:{}", status_rollup.running),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(
            format!("ok:{}", status_rollup.success),
            Style::default().fg(theme.ok),
        ),
        Span::raw(" "),
        Span::styled(
            format!("fb:{}", status_rollup.fallback),
            Style::default().fg(theme.warn),
        ),
        Span::raw(" "),
        Span::styled(
            format!("err:{}", status_rollup.error),
            Style::default().fg(theme.critical),
        ),
    ]));

    let export_status = artifact_snapshot
        .latest_export
        .as_ref()
        .map(|manifest| {
            mind_timestamp_label(&manifest.exported_at)
                .map(|label| format!("latest@{label}"))
                .unwrap_or_else(|| "latest:present".to_string())
        })
        .unwrap_or_else(|| "latest:none".to_string());
    let recovery_status = if artifact_snapshot.compaction_rebuildable {
        "recovery:ready"
    } else if artifact_snapshot.compaction_marker_event_available {
        "recovery:partial"
    } else {
        "recovery:none"
    };
    lines.push(Line::from(vec![
        Span::styled("overview:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("handshake:{}", artifact_snapshot.handshake_entries.len()),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(
            format!("canon:{}", artifact_snapshot.active_canon_entries.len()),
            Style::default().fg(theme.accent),
        ),
        Span::raw(" "),
        Span::styled(
            format!("stale:{}", artifact_snapshot.stale_canon_count),
            Style::default().fg(if artifact_snapshot.stale_canon_count > 0 {
                theme.warn
            } else {
                theme.muted
            }),
        ),
        Span::raw(" "),
        Span::styled(export_status, Style::default().fg(theme.ok)),
        Span::raw(" "),
        Span::styled(
            recovery_status,
            Style::default().fg(if artifact_snapshot.compaction_rebuildable {
                theme.ok
            } else if artifact_snapshot.compaction_marker_event_available {
                theme.warn
            } else {
                theme.muted
            }),
        ),
        Span::raw(" "),
        Span::styled(
            format!("detached:{}", detached_jobs.len()),
            Style::default().fg(theme.muted),
        ),
    ]));

    let injection_rows = app.mind_injection_rows();
    if let Some(line) = render_mind_injection_rollup_line(&injection_rows, theme, compact) {
        lines.push(line);
    }

    if let Some(line) = render_insight_detached_rollup_line(&detached_jobs, theme, compact) {
        lines.push(line);
    }

    let search_lines = render_mind_search_lines(
        &artifact_snapshot,
        &app.mind_search_query,
        app.mind_search_editing,
        theme,
        compact,
    );
    let artifact_lines = render_mind_artifact_drilldown_lines(
        &app.config.project_root,
        &app.config.session_id,
        theme,
        compact,
        app.mind_show_provenance,
        &all_rows,
        app.insight_runtime_rollup(),
    );

    lines.push(Line::from(vec![
        Span::styled(
            "Observer activity",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{} events]", rows.len()),
            Style::default().fg(theme.muted),
        ),
    ]));

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "No observer activity yet for current lane/scope.",
            Style::default().fg(theme.muted),
        )));
        lines.push(Line::from(Span::styled(
            "Try: o (T1), O (T1->T2 chain), b (bootstrap dry-run), t/v (filters).",
            Style::default().fg(theme.muted),
        )));
        lines.push(Line::from(""));
        lines.extend(search_lines);
        if !artifact_lines.is_empty() {
            lines.push(Line::from(""));
            lines.extend(artifact_lines);
        }
        return lines;
    }

    for row in rows {
        let status_label = mind_status_label(row.event.status);
        let status_color = mind_status_color(row.event.status, theme);
        let trigger_label = mind_trigger_label(row.event.trigger);
        let lane = mind_event_lane(&row.event);
        let lane_label = mind_lane_label(lane);
        let runtime_label = row
            .event
            .runtime
            .as_deref()
            .map(mind_runtime_label)
            .unwrap_or("runtime:n/a".to_string());
        let latency = row
            .event
            .latency_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "n/a".to_string());
        let attempts = row
            .event
            .attempt_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        let when = row
            .event
            .completed_at
            .as_deref()
            .or(row.event.started_at.as_deref())
            .or(row.event.enqueued_at.as_deref())
            .and_then(mind_timestamp_label)
            .unwrap_or_else(|| "--:--:--".to_string());

        let mut primary_spans = vec![
            Span::styled("✦", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", lane_label),
                Style::default()
                    .fg(mind_lane_color(lane, theme))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", status_label),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", trigger_label),
                Style::default().fg(theme.info),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", runtime_label),
                Style::default().fg(theme.accent),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{}::{}", row.scope, row.pane_id),
                Style::default()
                    .fg(if row.tab_focused {
                        theme.accent
                    } else {
                        theme.text
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(format!("lat:{latency}"), Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(format!("att:{attempts}"), Style::default().fg(theme.muted)),
        ];
        if let Some(progress) = row.event.progress.as_ref() {
            primary_spans.push(Span::raw(" "));
            primary_spans.push(Span::styled(
                mind_progress_label(progress),
                Style::default().fg(theme.muted),
            ));
        }
        primary_spans.push(Span::raw(" "));
        primary_spans.push(Span::styled(
            format!("@{when}"),
            Style::default().fg(theme.muted),
        ));
        lines.push(Line::from(primary_spans));

        let mut context = row
            .event
            .reason
            .clone()
            .or_else(|| row.event.failure_kind.clone())
            .or_else(|| row.event.conversation_id.map(|id| format!("conv:{id}")))
            .unwrap_or_else(|| {
                format!(
                    "source:{} tab:{} agent:{}",
                    row.source,
                    row.tab_scope
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or("n/a"),
                    row.agent_id
                )
            });
        if compact {
            context = ellipsize(&context, 52);
        }
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(context, Style::default().fg(theme.muted)),
        ]));
    }

    lines.push(Line::from(""));
    lines.extend(search_lines);

    if !artifact_lines.is_empty() {
        lines.push(Line::from(""));
        lines.extend(artifact_lines);
    }

    lines
}

fn render_mind_artifact_drilldown_lines(
    project_root: &Path,
    session_id: &str,
    theme: PulseTheme,
    compact: bool,
    show_provenance: bool,
    observer_rows: &[MindObserverRow],
    runtime: Option<InsightRuntimeSnapshot>,
) -> Vec<Line<'static>> {
    let snapshot = load_mind_artifact_drilldown(project_root, session_id);
    if snapshot.latest_export.is_none()
        && snapshot.latest_compaction_checkpoint.is_none()
        && snapshot.latest_compaction_slice.is_none()
        && snapshot.handshake_entries.is_empty()
        && snapshot.active_canon_entries.is_empty()
        && snapshot.stale_canon_count == 0
    {
        return Vec::new();
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Knowledge artifacts",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("Artifact drilldown", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            if show_provenance {
                "[provenance:on]"
            } else {
                "[provenance:off]"
            },
            Style::default().fg(theme.muted),
        ),
    ])];

    if let Some(checkpoint) = snapshot.latest_compaction_checkpoint.as_ref() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Compaction / recovery", Style::default().fg(theme.title)),
        ]));
        let mut spans = vec![
            Span::raw("  "),
            Span::styled("compact:", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                ellipsize(
                    checkpoint
                        .compaction_entry_id
                        .as_deref()
                        .unwrap_or(&checkpoint.checkpoint_id),
                    if compact { 24 } else { 40 },
                ),
                Style::default().fg(theme.info),
            ),
        ];
        if let Some(tokens_before) = checkpoint.tokens_before {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("tokens:{}", tokens_before),
                Style::default().fg(theme.muted),
            ));
        }
        if let Some(first_kept) = checkpoint.first_kept_entry_id.as_deref() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("keep:{}", ellipsize(first_kept, 14)),
                Style::default().fg(theme.muted),
            ));
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("@{}", ellipsize(&checkpoint.ts.to_rfc3339(), 20)),
            Style::default().fg(theme.muted),
        ));
        lines.push(Line::from(spans));

        let t1_state = latest_compaction_t1_state(checkpoint, observer_rows);
        let t0_label = if snapshot.latest_compaction_slice.is_some() {
            "stored"
        } else {
            "missing"
        };
        let replay_label = if snapshot.compaction_rebuildable {
            "ready"
        } else if snapshot.compaction_marker_event_available {
            "partial"
        } else {
            "missing"
        };
        let mut health_spans = vec![
            Span::raw("  -> "),
            Span::styled("health:", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                format!("t0:{t0_label}"),
                Style::default().fg(if snapshot.latest_compaction_slice.is_some() {
                    theme.ok
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!("replay:{replay_label}"),
                Style::default().fg(if snapshot.compaction_rebuildable {
                    theme.ok
                } else if snapshot.compaction_marker_event_available {
                    theme.warn
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!("t1:{}", t1_state.label),
                Style::default().fg(t1_state.color(theme)),
            ),
        ];
        if let Some(runtime) = runtime.as_ref() {
            health_spans.push(Span::raw(" "));
            health_spans.push(Span::styled(
                format!("t2q:{} t3q:{}", runtime.queue_depth, runtime.t3_queue_depth),
                Style::default().fg(theme.muted),
            ));
        }
        lines.push(Line::from(health_spans));

        if let Some(slice) = snapshot.latest_compaction_slice.as_ref() {
            lines.push(Line::from(vec![
                Span::raw("  -> "),
                Span::styled(
                    format!(
                        "evidence: src:{} read:{} modified:{} policy:{}",
                        slice.source_event_ids.len(),
                        slice.read_files.len(),
                        slice.modified_files.len(),
                        ellipsize(&slice.policy_version, 18)
                    ),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }

        if let Some(summary) = checkpoint
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            lines.push(Line::from(vec![
                Span::raw("  -> "),
                Span::styled(
                    ellipsize(summary, if compact { 52 } else { 88 }),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "recovery: press 'C' to rebuild/requeue latest compaction checkpoint",
                Style::default().fg(theme.warn),
            ),
        ]));
    }

    if let Some(manifest) = snapshot.latest_export.as_ref() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Recent export", Style::default().fg(theme.title)),
        ]));
        let export_leaf = Path::new(&manifest.export_dir)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("(unknown)");
        let mut spans = vec![
            Span::raw("  "),
            Span::styled("latest:", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                ellipsize(export_leaf, if compact { 30 } else { 48 }),
                Style::default().fg(theme.accent),
            ),
            Span::raw(" "),
            Span::styled(
                format!("session:{}", ellipsize(&manifest.session_id, 18)),
                Style::default().fg(theme.muted),
            ),
            Span::raw(" "),
            Span::styled(
                format!("t1:{} t2:{}", manifest.t1_count, manifest.t2_count),
                Style::default().fg(theme.muted),
            ),
        ];
        if let Some(active_tag) = manifest
            .active_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("tag:{}", ellipsize(active_tag, 16)),
                Style::default().fg(theme.info),
            ));
        }
        if !manifest.exported_at.trim().is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("@{}", ellipsize(&manifest.exported_at, 20)),
                Style::default().fg(theme.muted),
            ));
        }
        if !manifest.t3_job_id.trim().is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("t3:{}", ellipsize(&manifest.t3_job_id, 20)),
                Style::default().fg(theme.warn),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("Handshake + canon", Style::default().fg(theme.title)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!(
                "handshake:{} active_canon:{} stale_canon:{}",
                snapshot.handshake_entries.len(),
                snapshot.active_canon_entries.len(),
                snapshot.stale_canon_count
            ),
            Style::default().fg(theme.muted),
        ),
    ]));

    if !show_provenance {
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "press 'p' to expand handshake → canon → evidence links",
                Style::default().fg(theme.muted),
            ),
        ]));
        return lines;
    }

    let mut canon_by_key = HashMap::new();
    for entry in &snapshot.active_canon_entries {
        canon_by_key.insert(canon_key(&entry.entry_id, entry.revision), entry);
    }

    let limit = if compact { 2 } else { 5 };
    for handshake in snapshot.handshake_entries.iter().take(limit) {
        lines.push(Line::from(vec![
            Span::raw("  ↳ "),
            Span::styled(
                format!("[{} r{}]", handshake.entry_id, handshake.revision),
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                handshake
                    .topic
                    .as_deref()
                    .map(|topic| format!("topic={topic}"))
                    .unwrap_or_else(|| "topic=global".to_string()),
                Style::default().fg(theme.muted),
            ),
        ]));

        let summary = ellipsize(&handshake.summary, if compact { 44 } else { 80 });
        lines.push(Line::from(vec![
            Span::raw("     "),
            Span::styled(summary, Style::default().fg(theme.muted)),
        ]));

        let key = canon_key(&handshake.entry_id, handshake.revision);
        if let Some(canon) = canon_by_key.get(&key) {
            let refs = if canon.evidence_refs.is_empty() {
                "(none)".to_string()
            } else {
                canon
                    .evidence_refs
                    .iter()
                    .take(if compact { 2 } else { 4 })
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let refs_count = canon.evidence_refs.len();
            lines.push(Line::from(vec![
                Span::raw("     trace: "),
                Span::styled("handshake", Style::default().fg(theme.info)),
                Span::raw(" -> "),
                Span::styled("canon", Style::default().fg(theme.accent)),
                Span::raw(" -> "),
                Span::styled(
                    format!("evidence[{refs_count}] {refs}"),
                    Style::default().fg(theme.warn),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("     trace: "),
                Span::styled(
                    "handshake -> canon (missing active entry)",
                    Style::default().fg(theme.critical),
                ),
            ]));
        }
    }

    lines
}

#[derive(Clone, Copy, Debug)]
struct CompactionT1State {
    label: &'static str,
}

impl CompactionT1State {
    fn color(self, theme: PulseTheme) -> Color {
        match self.label {
            "ok" => theme.ok,
            "pending" => theme.warn,
            "fallback" => theme.warn,
            "error" => theme.critical,
            _ => theme.muted,
        }
    }
}

fn latest_compaction_t1_state(
    checkpoint: &CompactionCheckpoint,
    observer_rows: &[MindObserverRow],
) -> CompactionT1State {
    let checkpoint_ms = checkpoint.ts.timestamp_millis();
    observer_rows
        .iter()
        .filter(|row| {
            mind_event_lane(&row.event) == MindLaneFilter::T1
                && row.event.trigger == MindObserverFeedTriggerKind::Compaction
                && row
                    .event
                    .conversation_id
                    .as_deref()
                    .map(|id| id == checkpoint.conversation_id)
                    .unwrap_or(false)
                && mind_event_sort_ms(
                    row.event
                        .completed_at
                        .as_deref()
                        .or(row.event.started_at.as_deref())
                        .or(row.event.enqueued_at.as_deref()),
                )
                .map(|ts| ts >= checkpoint_ms.saturating_sub(1_000))
                .unwrap_or(true)
        })
        .max_by_key(|row| {
            mind_event_sort_ms(
                row.event
                    .completed_at
                    .as_deref()
                    .or(row.event.started_at.as_deref())
                    .or(row.event.enqueued_at.as_deref()),
            )
            .unwrap_or(0)
        })
        .map(|row| match row.event.status {
            MindObserverFeedStatus::Success => CompactionT1State { label: "ok" },
            MindObserverFeedStatus::Fallback => CompactionT1State { label: "fallback" },
            MindObserverFeedStatus::Running | MindObserverFeedStatus::Queued => {
                CompactionT1State { label: "pending" }
            }
            MindObserverFeedStatus::Error => CompactionT1State { label: "error" },
        })
        .unwrap_or(CompactionT1State { label: "unknown" })
}

fn compaction_rebuildable_from_attrs(attrs: &BTreeMap<String, Value>) -> bool {
    attrs.contains_key("mind_compaction_modified_files")
        || attrs.contains_key("pi_detail_read_files")
        || attrs.contains_key("pi_detail_modified_files")
}

fn canon_key(entry_id: &str, revision: u32) -> String {
    format!("{}#{}", entry_id.trim(), revision)
}

fn load_mind_artifact_drilldown(project_root: &Path, session_id: &str) -> MindArtifactDrilldown {
    let mut snapshot = MindArtifactDrilldown::default();

    let insight_dir = project_root.join(".aoc").join("mind").join("insight");
    if let Some(manifest) = load_latest_session_export_manifest(&insight_dir) {
        snapshot.latest_export = Some(manifest);
    }

    let compatibility = mind_feed_compatibility_mode();
    let store_path = mind_store_path(project_root);
    if compatibility != MindFeedCompatibilityMode::Legacy && store_path.exists() {
        if let Ok(store) = aoc_storage::MindStore::open(&store_path) {
            snapshot.latest_compaction_checkpoint = store
                .latest_compaction_checkpoint_for_session(session_id)
                .ok()
                .flatten();
            snapshot.latest_compaction_slice = store
                .latest_compaction_t0_slice_for_session(session_id)
                .ok()
                .flatten();
            if let Some(checkpoint) = snapshot.latest_compaction_checkpoint.as_ref() {
                if let Some(marker_event_id) = checkpoint.marker_event_id.as_deref() {
                    if let Ok(marker_event) = store.raw_event_by_id(marker_event_id) {
                        snapshot.compaction_marker_event_available = marker_event.is_some();
                        snapshot.compaction_rebuildable = marker_event
                            .as_ref()
                            .map(|event| compaction_rebuildable_from_attrs(&event.attrs))
                            .unwrap_or(false);
                    }
                }
                if snapshot.latest_compaction_slice.is_none() {
                    snapshot.latest_compaction_slice = store
                        .compaction_t0_slice_for_checkpoint(&checkpoint.checkpoint_id)
                        .ok()
                        .flatten();
                }
            }

            let scope_key = project_scope_key(project_root);
            if let Ok(Some(handshake)) = store.latest_handshake_snapshot("project", &scope_key) {
                snapshot.handshake_entries = parse_handshake_entries(&handshake.payload_text);
            }
            if let Ok(active) = store.active_canon_entries(None) {
                snapshot.active_canon_entries = active
                    .into_iter()
                    .map(|entry| MindCanonEntry {
                        entry_id: entry.entry_id,
                        revision: entry.revision.max(0) as u32,
                        topic: entry.topic,
                        evidence_refs: entry.evidence_refs,
                        summary: entry.summary,
                    })
                    .collect();
            }
            if let Ok(stale) = store.canon_entries_by_state(CanonRevisionState::Stale, None) {
                snapshot.stale_canon_count = stale.len();
            }
        }
    }

    let should_fallback_legacy = compatibility != MindFeedCompatibilityMode::Canonical
        && (snapshot.handshake_entries.is_empty()
            || snapshot.active_canon_entries.is_empty() && snapshot.stale_canon_count == 0);
    if should_fallback_legacy {
        load_legacy_mind_artifact_drilldown(project_root, &mut snapshot);
    }

    snapshot
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MindFeedCompatibilityMode {
    Canonical,
    Hybrid,
    Legacy,
}

fn mind_feed_compatibility_mode() -> MindFeedCompatibilityMode {
    match env::var("AOC_MIND_FEED_COMPAT")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("canonical") | Some("v2") | Some("store") => MindFeedCompatibilityMode::Canonical,
        Some("legacy") | Some("v1") | Some("files") => MindFeedCompatibilityMode::Legacy,
        _ => MindFeedCompatibilityMode::Hybrid,
    }
}

fn project_scope_key(project_root: &Path) -> String {
    format!("project:{}", project_root.to_string_lossy())
}

fn load_legacy_mind_artifact_drilldown(project_root: &Path, snapshot: &mut MindArtifactDrilldown) {
    let t3_dir = project_root.join(".aoc").join("mind").join("t3");
    if snapshot.handshake_entries.is_empty() {
        let handshake_path = t3_dir.join("handshake.md");
        if let Ok(payload) = fs::read_to_string(&handshake_path) {
            snapshot.handshake_entries = parse_handshake_entries(&payload);
        }
    }

    if snapshot.active_canon_entries.is_empty() && snapshot.stale_canon_count == 0 {
        let canon_path = t3_dir.join("project_mind.md");
        if let Ok(payload) = fs::read_to_string(&canon_path) {
            let (active, stale_count) = parse_project_canon_entries(&payload);
            snapshot.active_canon_entries = active;
            snapshot.stale_canon_count = stale_count;
        }
    }
}

fn resolve_aoc_state_home() -> PathBuf {
    if let Ok(value) = env::var("XDG_STATE_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".local/state")
}

fn sanitize_runtime_component(input: &str) -> String {
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

fn mind_store_path(project_root: &Path) -> PathBuf {
    env::var("AOC_MIND_STORE_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            resolve_aoc_state_home()
                .join("aoc")
                .join("mind")
                .join("projects")
                .join(sanitize_runtime_component(&project_root.to_string_lossy()))
                .join("project.sqlite")
        })
}

fn consultation_kind_slug(kind: ConsultationPacketKind) -> &'static str {
    match kind {
        ConsultationPacketKind::Summary => "summary",
        ConsultationPacketKind::Plan => "plan",
        ConsultationPacketKind::Blockers => "blockers",
        ConsultationPacketKind::Review => "review",
        ConsultationPacketKind::Align => "align",
        ConsultationPacketKind::CheckpointStatus => "checkpoint_status",
        ConsultationPacketKind::HelpRequest => "help_request",
    }
}

fn persist_consultation_outcome(
    project_root: &Path,
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
    kind: ConsultationPacketKind,
) -> Result<String, String> {
    let store_path = mind_store_path(project_root);
    if let Some(parent) = store_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create mind store directory failed: {err}"))?;
    }
    let store = aoc_storage::MindStore::open(&store_path)
        .map_err(|err| format!("open mind store failed: {err}"))?;
    let ts = payload
        .packet
        .as_ref()
        .and_then(|packet| packet.freshness.packet_generated_at.as_deref())
        .and_then(parse_rfc3339_utc)
        .unwrap_or_else(Utc::now);
    let artifact_id = format!("consult:{}", payload.consultation_id);
    let conversation_id = consultation_memory_conversation_id(request_packet, payload);
    let trace_ids = consultation_memory_trace_ids(request_packet, payload, kind);
    let text = render_consultation_outcome_markdown(request_packet, payload, kind, ts);
    let input_hash =
        canonical_payload_hash(&(request_packet, payload, consultation_kind_slug(kind)))
            .map_err(|err| format!("hash consultation outcome failed: {err}"))?;

    store
        .insert_reflection(&artifact_id, &conversation_id, ts, &text, &trace_ids)
        .map_err(|err| format!("persist consultation reflection failed: {err}"))?;
    store
        .upsert_semantic_provenance(&SemanticProvenance {
            artifact_id: artifact_id.clone(),
            stage: SemanticStage::T2Reflector,
            runtime: SemanticRuntime::Deterministic,
            provider_name: None,
            model_id: None,
            prompt_version: "mission-control.consultation-memory.v1".to_string(),
            input_hash,
            output_hash: None,
            latency_ms: None,
            attempt_count: 1,
            fallback_used: false,
            fallback_reason: None,
            failure_kind: None,
            created_at: ts,
        })
        .map_err(|err| format!("persist consultation provenance failed: {err}"))?;

    let task_ids = consultation_memory_task_ids(request_packet, payload);
    for task_id in task_ids {
        let link = ArtifactTaskLink::new(
            artifact_id.clone(),
            task_id,
            ArtifactTaskRelation::Mentioned,
            800,
            Vec::new(),
            "mission-control.consultation-memory".to_string(),
            ts,
            None,
        )
        .map_err(|err| format!("build consultation task link failed: {err}"))?;
        store
            .upsert_artifact_task_link(&link)
            .map_err(|err| format!("persist consultation task link failed: {err}"))?;
    }

    for evidence in consultation_memory_evidence_refs(request_packet, payload) {
        if let Some(path) = evidence
            .path
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            store
                .upsert_artifact_file_link(&aoc_storage::ArtifactFileLink {
                    artifact_id: artifact_id.clone(),
                    path: path.clone(),
                    relation: evidence
                        .relation
                        .clone()
                        .unwrap_or_else(|| "consultation_evidence".to_string()),
                    source: "mission-control.consultation-memory".to_string(),
                    additions: None,
                    deletions: None,
                    staged: false,
                    untracked: false,
                    created_at: ts,
                    updated_at: ts,
                })
                .map_err(|err| format!("persist consultation file link failed: {err}"))?;
        }
    }

    Ok(artifact_id)
}

fn consultation_memory_conversation_id(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
) -> String {
    request_packet
        .identity
        .conversation_id
        .clone()
        .or_else(|| {
            payload
                .packet
                .as_ref()
                .and_then(|packet| packet.identity.conversation_id.clone())
        })
        .unwrap_or_else(|| format!("consultation:{}", request_packet.identity.session_id))
}

fn consultation_memory_trace_ids(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
    kind: ConsultationPacketKind,
) -> Vec<String> {
    let mut trace_ids = vec![
        format!("consultation:{}", payload.consultation_id),
        format!("consultation_kind:{}", consultation_kind_slug(kind)),
        format!("consultation_status:{:?}", payload.status).to_ascii_lowercase(),
        format!("requester:{}", payload.requesting_agent_id),
        format!("responder:{}", payload.responding_agent_id),
    ];
    if !request_packet.packet_id.trim().is_empty() {
        trace_ids.push(format!("request_packet:{}", request_packet.packet_id));
    }
    if let Some(packet) = payload.packet.as_ref() {
        if !packet.packet_id.trim().is_empty() {
            trace_ids.push(format!("response_packet:{}", packet.packet_id));
        }
    }
    trace_ids.sort();
    trace_ids.dedup();
    trace_ids
}

fn consultation_memory_task_ids(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
) -> Vec<String> {
    let mut task_ids = request_packet.task_context.task_ids.clone();
    if let Some(packet) = payload.packet.as_ref() {
        task_ids.extend(packet.task_context.task_ids.iter().cloned());
    }
    task_ids.sort();
    task_ids.dedup();
    task_ids.retain(|value| !value.trim().is_empty());
    task_ids
}

fn consultation_memory_evidence_refs(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
) -> Vec<aoc_core::consultation_contracts::ConsultationEvidenceRef> {
    let mut refs = request_packet.evidence_refs.clone();
    if let Some(packet) = payload.packet.as_ref() {
        refs.extend(packet.evidence_refs.iter().cloned());
    }
    refs.sort_by(|left, right| {
        left.reference
            .cmp(&right.reference)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.relation.cmp(&right.relation))
    });
    refs.dedup_by(|left, right| {
        left.reference == right.reference
            && left.path == right.path
            && left.relation == right.relation
    });
    refs
}

fn render_consultation_outcome_markdown(
    request_packet: &ConsultationPacket,
    payload: &ConsultationResponsePayload,
    kind: ConsultationPacketKind,
    ts: DateTime<Utc>,
) -> String {
    let mut lines = vec![
        "# Consultation outcome".to_string(),
        String::new(),
        format!("- consultation_id: {}", payload.consultation_id),
        format!("- kind: {}", consultation_kind_slug(kind)),
        format!(
            "- status: {}",
            format!("{:?}", payload.status).to_ascii_lowercase()
        ),
        format!("- requester: {}", payload.requesting_agent_id),
        format!("- responder: {}", payload.responding_agent_id),
        format!("- recorded_at: {}", ts.to_rfc3339()),
    ];
    if let Some(tag) = request_packet.task_context.active_tag.as_ref() {
        lines.push(format!("- active_tag: {tag}"));
    }
    if !request_packet.task_context.task_ids.is_empty() {
        lines.push(format!(
            "- tasks: {}",
            request_packet.task_context.task_ids.join(", ")
        ));
    }
    lines.push(String::new());
    lines.push("## Request".to_string());
    if let Some(summary) = request_packet.summary.as_ref() {
        lines.push(summary.clone());
    }
    if let Some(help) = request_packet.help_request.as_ref() {
        lines.push(format!("- help_request [{}]: {}", help.kind, help.question));
    }
    if !request_packet.blockers.is_empty() {
        lines.push("- blockers:".to_string());
        for blocker in &request_packet.blockers {
            lines.push(format!("  - {}", blocker.summary));
        }
    }
    if !request_packet.current_plan.is_empty() {
        lines.push("- request_plan:".to_string());
        for item in &request_packet.current_plan {
            lines.push(format!("  - {}", item.title));
        }
    }

    lines.push(String::new());
    lines.push("## Response".to_string());
    if let Some(packet) = payload.packet.as_ref() {
        if let Some(summary) = packet.summary.as_ref() {
            lines.push(summary.clone());
        }
        if !packet.current_plan.is_empty() {
            lines.push("- response_plan:".to_string());
            for item in &packet.current_plan {
                lines.push(format!("  - {}", item.title));
            }
        }
        if !packet.blockers.is_empty() {
            lines.push("- response_blockers:".to_string());
            for blocker in &packet.blockers {
                lines.push(format!("  - {}", blocker.summary));
            }
        }
        if let Some(rationale) = packet.confidence.rationale.as_ref() {
            lines.push(format!("- rationale: {rationale}"));
        }
    } else if let Some(message) = payload.message.as_ref() {
        lines.push(message.clone());
    }
    if let Some(error) = payload.error.as_ref() {
        lines.push(format!("- error [{}]: {}", error.code, error.message));
    }

    let evidence_refs = consultation_memory_evidence_refs(request_packet, payload);
    if !evidence_refs.is_empty() {
        lines.push(String::new());
        lines.push("## Evidence refs".to_string());
        for evidence in evidence_refs {
            let mut line = format!("- {}", evidence.reference);
            if let Some(label) = evidence.label.as_ref() {
                line.push_str(&format!(" — {label}"));
            }
            if let Some(path) = evidence.path.as_ref() {
                line.push_str(&format!(" ({path})"));
            }
            lines.push(line);
        }
    }

    lines.join("\n")
}

fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|ts| ts.with_timezone(&Utc))
}

fn load_latest_session_export_manifest(insight_dir: &Path) -> Option<MindSessionExportManifest> {
    let entries = fs::read_dir(insight_dir).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    let latest = dirs.pop()?;
    let payload = fs::read_to_string(latest.join("manifest.json")).ok()?;
    serde_json::from_str::<MindSessionExportManifest>(&payload).ok()
}

fn parse_handshake_entries(payload: &str) -> Vec<MindHandshakeEntry> {
    let mut entries = Vec::new();
    for line in payload.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("- [") else {
            continue;
        };
        let Some(end_bracket) = rest.find(']') else {
            continue;
        };
        let head = rest[..end_bracket].trim();
        let Some((entry_id, revision_raw)) = head.rsplit_once(" r") else {
            continue;
        };
        let Ok(revision) = revision_raw.trim().parse::<u32>() else {
            continue;
        };

        let tail = rest[end_bracket + 1..].trim();
        let topic = tail
            .split_whitespace()
            .find_map(|segment| segment.strip_prefix("topic="))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let summary = tail
            .split_once("::")
            .map(|(_, text)| text.trim().to_string())
            .unwrap_or_default();

        entries.push(MindHandshakeEntry {
            entry_id: entry_id.trim().to_string(),
            revision,
            topic,
            summary,
        });
    }
    entries
}

fn parse_project_canon_entries(payload: &str) -> (Vec<MindCanonEntry>, usize) {
    enum Section {
        None,
        Active,
        Stale,
    }

    let mut section = Section::None;
    let mut active_entries = Vec::new();
    let mut stale_count = 0usize;
    let mut current: Option<MindCanonEntry> = None;

    let flush_current = |section: &Section,
                         current: &mut Option<MindCanonEntry>,
                         active_entries: &mut Vec<MindCanonEntry>,
                         stale_count: &mut usize| {
        let Some(entry) = current.take() else {
            return;
        };
        match section {
            Section::Active => active_entries.push(entry),
            Section::Stale => *stale_count += 1,
            Section::None => {}
        }
    };

    for raw_line in payload.lines() {
        let line = raw_line.trim();

        if line == "## Active canon" {
            flush_current(
                &section,
                &mut current,
                &mut active_entries,
                &mut stale_count,
            );
            section = Section::Active;
            continue;
        }
        if line == "## Stale canon" {
            flush_current(
                &section,
                &mut current,
                &mut active_entries,
                &mut stale_count,
            );
            section = Section::Stale;
            continue;
        }

        if let Some(header) = line.strip_prefix("### ") {
            flush_current(
                &section,
                &mut current,
                &mut active_entries,
                &mut stale_count,
            );
            let Some((entry_id, revision_raw)) = header.rsplit_once(" r") else {
                current = None;
                continue;
            };
            let Ok(revision) = revision_raw.trim().parse::<u32>() else {
                current = None;
                continue;
            };
            current = Some(MindCanonEntry {
                entry_id: entry_id.trim().to_string(),
                revision,
                topic: None,
                evidence_refs: Vec::new(),
                summary: String::new(),
            });
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };

        if let Some(topic) = line.strip_prefix("- topic:") {
            let topic = topic.trim();
            if !topic.is_empty() {
                entry.topic = Some(topic.to_string());
            }
            continue;
        }

        if let Some(refs) = line.strip_prefix("- evidence_refs:") {
            entry.evidence_refs = refs
                .split(',')
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            continue;
        }

        if !line.is_empty() && !line.starts_with('-') && entry.summary.is_empty() {
            entry.summary = line.to_string();
        }
    }

    flush_current(
        &section,
        &mut current,
        &mut active_entries,
        &mut stale_count,
    );

    active_entries.sort_by(|left, right| {
        left.entry_id
            .cmp(&right.entry_id)
            .then_with(|| left.revision.cmp(&right.revision))
    });

    (active_entries, stale_count)
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

fn mind_event_is_t3(event: &MindObserverFeedEvent) -> bool {
    event
        .runtime
        .as_deref()
        .map(|runtime| {
            let runtime = runtime.to_ascii_lowercase();
            runtime.contains("t3") || runtime.contains("backlog")
        })
        .unwrap_or(false)
        || event
            .reason
            .as_deref()
            .map(|reason| {
                let reason = reason.to_ascii_lowercase();
                reason.contains("t3") || reason.contains("backlog") || reason.contains("canon")
            })
            .unwrap_or(false)
}

fn mind_event_is_t2(event: &MindObserverFeedEvent) -> bool {
    event
        .runtime
        .as_deref()
        .map(|runtime| {
            let runtime = runtime.to_ascii_lowercase();
            runtime.contains("t2") || runtime.contains("reflector")
        })
        .unwrap_or(false)
        || event
            .reason
            .as_deref()
            .map(|reason| {
                let reason = reason.to_ascii_lowercase();
                reason.contains("t2") || reason.contains("reflector")
            })
            .unwrap_or(false)
}

fn mind_event_is_t0(event: &MindObserverFeedEvent) -> bool {
    if event.progress.is_some() && event.runtime.is_none() {
        return true;
    }
    event
        .reason
        .as_deref()
        .map(|reason| reason.to_ascii_lowercase().contains("t0"))
        .unwrap_or(false)
}

fn mind_event_lane(event: &MindObserverFeedEvent) -> MindLaneFilter {
    if mind_event_is_t3(event) {
        MindLaneFilter::T3
    } else if mind_event_is_t2(event) {
        MindLaneFilter::T2
    } else if mind_event_is_t0(event) {
        MindLaneFilter::T0
    } else {
        MindLaneFilter::T1
    }
}

fn mind_lane_matches(filter: MindLaneFilter, lane: MindLaneFilter) -> bool {
    match filter {
        MindLaneFilter::All => true,
        MindLaneFilter::T0 => lane == MindLaneFilter::T0,
        MindLaneFilter::T1 => lane == MindLaneFilter::T1,
        MindLaneFilter::T2 => lane == MindLaneFilter::T2,
        MindLaneFilter::T3 => lane == MindLaneFilter::T3,
    }
}

fn mind_lane_label(lane: MindLaneFilter) -> &'static str {
    match lane {
        MindLaneFilter::T0 => "t0",
        MindLaneFilter::T1 => "t1",
        MindLaneFilter::T2 => "t2",
        MindLaneFilter::T3 => "t3",
        MindLaneFilter::All => "all",
    }
}

fn mind_lane_color(lane: MindLaneFilter, theme: PulseTheme) -> Color {
    match lane {
        MindLaneFilter::T0 => theme.muted,
        MindLaneFilter::T1 => theme.info,
        MindLaneFilter::T2 => theme.accent,
        MindLaneFilter::T3 => theme.warn,
        MindLaneFilter::All => theme.text,
    }
}

#[derive(Default)]
struct MindStatusRollup {
    queued: usize,
    running: usize,
    success: usize,
    fallback: usize,
    error: usize,
}

fn mind_status_rollup(rows: &[MindObserverRow]) -> MindStatusRollup {
    let mut rollup = MindStatusRollup::default();
    for row in rows {
        match row.event.status {
            MindObserverFeedStatus::Queued => rollup.queued += 1,
            MindObserverFeedStatus::Running => rollup.running += 1,
            MindObserverFeedStatus::Success => rollup.success += 1,
            MindObserverFeedStatus::Fallback => rollup.fallback += 1,
            MindObserverFeedStatus::Error => rollup.error += 1,
        }
    }
    rollup
}

fn mind_lane_rollup(rows: &[MindObserverRow]) -> [usize; 4] {
    let mut lanes = [0usize; 4];
    for row in rows {
        match mind_event_lane(&row.event) {
            MindLaneFilter::T0 => lanes[0] += 1,
            MindLaneFilter::T1 => lanes[1] += 1,
            MindLaneFilter::T2 => lanes[2] += 1,
            MindLaneFilter::T3 => lanes[3] += 1,
            MindLaneFilter::All => {}
        }
    }
    lanes
}

fn mind_status_label(status: MindObserverFeedStatus) -> &'static str {
    match status {
        MindObserverFeedStatus::Queued => "queued",
        MindObserverFeedStatus::Running => "running",
        MindObserverFeedStatus::Success => "success",
        MindObserverFeedStatus::Fallback => "fallback",
        MindObserverFeedStatus::Error => "error",
    }
}

fn mind_status_color(status: MindObserverFeedStatus, theme: PulseTheme) -> Color {
    match status {
        MindObserverFeedStatus::Queued => theme.warn,
        MindObserverFeedStatus::Running => theme.info,
        MindObserverFeedStatus::Success => theme.ok,
        MindObserverFeedStatus::Fallback => theme.warn,
        MindObserverFeedStatus::Error => theme.critical,
    }
}

fn mind_trigger_label(trigger: MindObserverFeedTriggerKind) -> &'static str {
    match trigger {
        MindObserverFeedTriggerKind::TokenThreshold => "token",
        MindObserverFeedTriggerKind::TaskCompleted => "task",
        MindObserverFeedTriggerKind::ManualShortcut => "manual",
        MindObserverFeedTriggerKind::Handoff => "handoff",
        MindObserverFeedTriggerKind::Compaction => "compact",
    }
}

fn mind_progress_label(progress: &MindObserverFeedProgress) -> String {
    if progress.t1_target_tokens == 0 {
        return format!("t0:{}", progress.t0_estimated_tokens);
    }
    format!(
        "t0:{}/{} next:{}",
        progress.t0_estimated_tokens, progress.t1_target_tokens, progress.tokens_until_next_run
    )
}

fn score_search_text(text: &str, terms: &[String]) -> usize {
    let haystack = text.to_ascii_lowercase();
    terms
        .iter()
        .map(|term| {
            let mut hits = 0usize;
            let mut start = 0usize;
            while let Some(pos) = haystack[start..].find(term) {
                hits += 1;
                start += pos + term.len();
            }
            hits
        })
        .sum()
}

fn collect_mind_search_hits(snapshot: &MindArtifactDrilldown, query: &str) -> Vec<MindSearchHit> {
    let terms = query
        .split_whitespace()
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return Vec::new();
    }

    let mut hits = Vec::new();
    for entry in &snapshot.handshake_entries {
        let title = format!("{} r{}", entry.entry_id, entry.revision);
        let topic = entry.topic.as_deref().unwrap_or("global");
        let summary = format!("topic={topic} · {}", entry.summary);
        let score = score_search_text(&format!("{title} {summary}"), &terms);
        if score > 0 {
            hits.push(MindSearchHit {
                kind: "handshake",
                title,
                summary,
                score,
            });
        }
    }
    for entry in &snapshot.active_canon_entries {
        let title = format!("{} r{}", entry.entry_id, entry.revision);
        let topic = entry.topic.as_deref().unwrap_or("global");
        let refs = if entry.evidence_refs.is_empty() {
            "evidence:none".to_string()
        } else {
            format!("evidence:{}", entry.evidence_refs.join(", "))
        };
        let summary = format!("topic={topic} · {} · {refs}", entry.summary);
        let score = score_search_text(&format!("{title} {summary}"), &terms);
        if score > 0 {
            hits.push(MindSearchHit {
                kind: "canon",
                title,
                summary,
                score,
            });
        }
    }
    if let Some(export) = snapshot.latest_export.as_ref() {
        let title = export
            .active_tag
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "recent-export".to_string());
        let summary = format!(
            "session:{} t1:{} t2:{} exported:{}",
            export.session_id, export.t1_count, export.t2_count, export.exported_at
        );
        let score = score_search_text(&format!("{title} {summary}"), &terms);
        if score > 0 {
            hits.push(MindSearchHit {
                kind: "export",
                title,
                summary,
                score,
            });
        }
    }
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.kind.cmp(right.kind))
            .then_with(|| left.title.cmp(&right.title))
    });
    hits.truncate(6);
    hits
}

fn render_mind_search_lines(
    snapshot: &MindArtifactDrilldown,
    query: &str,
    editing: bool,
    theme: PulseTheme,
    compact: bool,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Retrieval / search",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            if editing { "[editing]" } else { "[/ to edit]" },
            Style::default().fg(theme.muted),
        ),
    ])];
    let prompt = if query.trim().is_empty() {
        ""
    } else {
        query.trim()
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("query:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            if prompt.is_empty() {
                "(empty)".to_string()
            } else if editing {
                format!("> {prompt}_")
            } else {
                format!("> {prompt}")
            },
            Style::default().fg(if prompt.is_empty() {
                theme.muted
            } else {
                theme.accent
            }),
        ),
    ]));
    if prompt.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "Type / then a query to search handshake, canon, and recent export summaries.",
                Style::default().fg(theme.muted),
            ),
        ]));
        return lines;
    }

    let hits = collect_mind_search_hits(snapshot, prompt);
    if hits.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "No local project Mind hits.",
                Style::default().fg(theme.warn),
            ),
        ]));
        return lines;
    }

    lines.push(Line::from(vec![
        Span::raw("  -> "),
        Span::styled(
            format!("{} local hits", hits.len()),
            Style::default().fg(theme.muted),
        ),
    ]));
    for hit in hits {
        lines.push(Line::from(vec![
            Span::raw("  • "),
            Span::styled(format!("[{}]", hit.kind), Style::default().fg(theme.info)),
            Span::raw(" "),
            Span::styled(hit.title, Style::default().fg(theme.accent)),
            Span::raw(" "),
            Span::styled(
                format!("score:{}", hit.score),
                Style::default().fg(theme.muted),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                ellipsize(&hit.summary, if compact { 68 } else { 108 }),
                Style::default().fg(theme.muted),
            ),
        ]));
    }
    lines
}

fn render_mind_injection_rollup_line(
    rows: &[MindInjectionRow],
    theme: PulseTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let latest = rows.first()?;
    let trigger = latest.payload.trigger.as_str();
    let status = latest.payload.status.trim();
    let when =
        mind_timestamp_label(&latest.payload.queued_at).unwrap_or_else(|| "--:--:--".to_string());
    let mut detail_fields = Vec::new();
    if let Some(tag) = latest.payload.active_tag.as_deref() {
        detail_fields.push(format!("tag:{tag}"));
    }
    if let Some(tokens) = latest.payload.token_estimate {
        detail_fields.push(format!("tokens:{tokens}"));
    }
    if let Some(snapshot_id) = latest.payload.snapshot_id.as_deref() {
        detail_fields.push(format!("hs:{}", ellipsize(snapshot_id, 18)));
    }
    detail_fields.push(format!("scope:{}", latest.payload.scope));
    detail_fields.push(format!("pane:{}", latest.pane_id));
    if rows.len() > 1 {
        detail_fields.push(format!("agents:{}", rows.len()));
    }
    let detail = fit_fields(&detail_fields, if compact { 44 } else { 84 });
    let reason = latest
        .payload
        .reason
        .as_deref()
        .map(|value| ellipsize(value, if compact { 42 } else { 76 }))
        .unwrap_or_else(|| "awaiting bounded context injection".to_string());
    Some(Line::from(vec![
        Span::styled("inject:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", trigger),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", status),
            Style::default()
                .fg(mind_injection_status_color(status, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(detail, Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::styled(format!("@{when}"), Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(reason, Style::default().fg(theme.muted)),
    ]))
}

fn mind_injection_status_color(status: &str, theme: PulseTheme) -> Color {
    match status.trim() {
        "pending" => theme.info,
        "skipped-cooldown" | "skipped-duplicate" => theme.warn,
        "suppressed-pressure" => theme.critical,
        _ => theme.muted,
    }
}

fn render_insight_detached_rollup_line(
    jobs: &[InsightDetachedJob],
    theme: PulseTheme,
    compact: bool,
) -> Option<Line<'static>> {
    let latest = jobs.first()?;
    let mut queued = 0usize;
    let mut running = 0usize;
    let mut success = 0usize;
    let mut fallback = 0usize;
    let mut error = 0usize;
    let mut cancelled = 0usize;
    let mut stale = 0usize;
    for job in jobs {
        match job.status {
            InsightDetachedJobStatus::Queued => queued += 1,
            InsightDetachedJobStatus::Running => running += 1,
            InsightDetachedJobStatus::Success => success += 1,
            InsightDetachedJobStatus::Fallback => fallback += 1,
            InsightDetachedJobStatus::Error => error += 1,
            InsightDetachedJobStatus::Cancelled => cancelled += 1,
            InsightDetachedJobStatus::Stale => stale += 1,
        }
    }
    let delegated = jobs
        .iter()
        .filter(|job| matches!(job.owner_plane, InsightDetachedOwnerPlane::Delegated))
        .count();
    let mind = jobs.len().saturating_sub(delegated);
    let label = latest
        .agent
        .as_deref()
        .or(latest.chain.as_deref())
        .or(latest.team.as_deref())
        .unwrap_or("detached-job");
    let when = latest
        .finished_at_ms
        .or(latest.started_at_ms)
        .unwrap_or(latest.created_at_ms);
    let when = Utc
        .timestamp_millis_opt(when)
        .single()
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".to_string());
    let mut detail_fields = vec![
        format!("q:{queued}"),
        format!("run:{running}"),
        format!("ok:{success}"),
        format!("fb:{fallback}"),
        format!("err:{error}"),
    ];
    if cancelled > 0 {
        detail_fields.push(format!("cx:{cancelled}"));
    }
    if stale > 0 {
        detail_fields.push(format!("stale:{stale}"));
    }
    detail_fields.push(format!("pl:d{}|m{}", delegated, mind));
    detail_fields.push(format!(
        "kind:{}",
        detached_worker_kind_label(latest.worker_kind)
    ));
    if let Some(step_count) = latest.step_count {
        detail_fields.push(format!("steps:{step_count}"));
    }
    let detail = fit_fields(&detail_fields, if compact { 42 } else { 76 });
    let summary = latest
        .output_excerpt
        .as_deref()
        .or(latest.error.as_deref())
        .map(|value| ellipsize(value, if compact { 38 } else { 72 }))
        .unwrap_or_else(|| "detached runtime idle".to_string());
    Some(Line::from(vec![
        Span::styled("subagents:", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", detached_job_status_label(latest.status)),
            Style::default()
                .fg(detached_job_status_color(latest.status, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "{}:{}",
                detached_owner_plane_label(latest.owner_plane),
                ellipsize(label, if compact { 16 } else { 24 })
            ),
            Style::default().fg(theme.info),
        ),
        Span::raw(" "),
        Span::styled(detail, Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::styled(format!("@{when}"), Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(summary, Style::default().fg(theme.muted)),
    ]))
}

fn detached_owner_plane_label(owner_plane: InsightDetachedOwnerPlane) -> &'static str {
    match owner_plane {
        InsightDetachedOwnerPlane::Delegated => "delegated",
        InsightDetachedOwnerPlane::Mind => "mind",
    }
}

fn detached_worker_kind_label(worker_kind: Option<InsightDetachedWorkerKind>) -> &'static str {
    match worker_kind {
        Some(InsightDetachedWorkerKind::Specialist) => "specialist",
        Some(InsightDetachedWorkerKind::ChainStep) => "chain",
        Some(InsightDetachedWorkerKind::TeamFanout) => "fanout",
        Some(InsightDetachedWorkerKind::T1) => "t1",
        Some(InsightDetachedWorkerKind::T2) => "t2",
        Some(InsightDetachedWorkerKind::T3) => "t3",
        None => "unknown",
    }
}

fn detached_job_status_label(status: InsightDetachedJobStatus) -> &'static str {
    match status {
        InsightDetachedJobStatus::Queued => "queued",
        InsightDetachedJobStatus::Running => "running",
        InsightDetachedJobStatus::Success => "success",
        InsightDetachedJobStatus::Fallback => "fallback",
        InsightDetachedJobStatus::Error => "error",
        InsightDetachedJobStatus::Cancelled => "cancelled",
        InsightDetachedJobStatus::Stale => "stale",
    }
}

fn detached_job_status_color(status: InsightDetachedJobStatus, theme: PulseTheme) -> Color {
    match status {
        InsightDetachedJobStatus::Queued => theme.warn,
        InsightDetachedJobStatus::Running => theme.info,
        InsightDetachedJobStatus::Success => theme.ok,
        InsightDetachedJobStatus::Fallback => theme.warn,
        InsightDetachedJobStatus::Error => theme.critical,
        InsightDetachedJobStatus::Cancelled | InsightDetachedJobStatus::Stale => theme.muted,
    }
}

fn detached_job_recovery_guidance(job: &InsightDetachedJob) -> Vec<String> {
    let mut steps = Vec::new();
    match job.status {
        InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running => {
            steps.push(
                "job is active; wait, inspect, or cancel with x if it is no longer useful"
                    .to_string(),
            );
        }
        InsightDetachedJobStatus::Success => {
            steps.push("inspect or handoff the selected result into a follow-up tab if operator review is needed".to_string());
        }
        InsightDetachedJobStatus::Fallback => {
            steps.push("job completed with degraded execution; inspect the brief/error before trusting the result".to_string());
            steps.push("if the result is insufficient, rerun the specialist from the owning Pi session with a narrower prompt".to_string());
        }
        InsightDetachedJobStatus::Error => {
            steps.push("job failed; inspect stderr/error context and rerun from the owning Pi session after correcting scope or environment".to_string());
            steps.push("compare against other recent jobs in this group to see whether the failure is isolated or systemic".to_string());
        }
        InsightDetachedJobStatus::Cancelled => {
            steps.push("job was cancelled; relaunch only if the work is still needed".to_string());
        }
        InsightDetachedJobStatus::Stale => {
            steps.push("job lost live ownership or wrapper continuity; treat it as interrupted, not successful".to_string());
            steps.push("inspect any partial output, then rerun from the owning session if you still need a complete result".to_string());
        }
    }
    if job.fallback_used
        && !matches!(
            job.status,
            InsightDetachedJobStatus::Fallback | InsightDetachedJobStatus::Stale
        )
    {
        steps.push("fallback behavior was recorded; verify the result before using it as authoritative evidence".to_string());
    }
    steps
}

fn mind_runtime_label(runtime: &str) -> String {
    if runtime.trim().is_empty() {
        return "runtime:n/a".to_string();
    }
    let compact = runtime.trim().to_ascii_lowercase();
    match compact.as_str() {
        "pi-semantic" => "runtime:pi".to_string(),
        "deterministic" => "runtime:det".to_string(),
        "external-semantic" => "runtime:ext".to_string(),
        "t2_reflector" => "runtime:t2".to_string(),
        _ => format!("runtime:{}", ellipsize(&compact, 12)),
    }
}

fn mind_timestamp_label(value: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).format("%H:%M:%S").to_string())
}

fn mind_event_sort_ms(value: Option<&str>) -> Option<i64> {
    let value = value?;
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp_millis())
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

fn overview_layout_cmp(left: &OverviewRow, right: &OverviewRow) -> std::cmp::Ordering {
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
}

fn sort_overview_rows(mut rows: Vec<OverviewRow>) -> Vec<OverviewRow> {
    rows.sort_by(overview_layout_cmp);
    rows
}

fn sort_overview_rows_attention(mut rows: Vec<OverviewRow>) -> Vec<OverviewRow> {
    rows.sort_by(|left, right| {
        attention_chip_from_row(left)
            .severity()
            .cmp(&attention_chip_from_row(right).severity())
            .then_with(|| right.tab_focused.cmp(&left.tab_focused))
            .then_with(|| overview_layout_cmp(left, right))
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
    let tab_scope = source_string_field(&state.source, "tab_scope");
    let agent_label = source_string_field(&state.source, "agent_label")
        .or_else(|| source_string_field(&state.source, "label"))
        .or_else(|| Some(extract_label(&state.agent_id)));
    AgentStatusPayload {
        agent_id: state.agent_id.clone(),
        status: lifecycle,
        pane_id: state.pane_id.clone(),
        project_root,
        tab_scope,
        agent_label,
        message: state.snippet.clone(),
        session_title: source_string_field(&state.source, "session_title"),
        chat_title: source_string_field(&state.source, "chat_title"),
    }
}

fn source_string_field(source: &Option<Value>, key: &str) -> Option<String> {
    source_value_field(source, key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn canonical_tab_scope(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

fn tab_scope_matches(viewer_scope: Option<&str>, candidate_scope: Option<&str>) -> bool {
    let Some(viewer) = viewer_scope.and_then(canonical_tab_scope) else {
        return false;
    };
    let Some(candidate) = candidate_scope.and_then(canonical_tab_scope) else {
        return false;
    };
    viewer == candidate
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

fn parse_mind_observer_from_source(
    value: &Value,
) -> Result<Option<MindObserverFeedPayload>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut payload: MindObserverFeedPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    for event in &mut payload.events {
        event.reason = event
            .reason
            .as_ref()
            .map(|reason| reason.trim().to_string())
            .filter(|reason| !reason.is_empty());
        event.runtime = event
            .runtime
            .as_ref()
            .map(|runtime| runtime.trim().to_string())
            .filter(|runtime| !runtime.is_empty());
        event.failure_kind = event
            .failure_kind
            .as_ref()
            .map(|kind| kind.trim().to_string())
            .filter(|kind| !kind.is_empty());
        if let Some(progress) = event.progress.as_mut() {
            if progress.t1_target_tokens == 0 {
                event.progress = None;
                continue;
            }
            if progress.t1_hard_cap_tokens < progress.t1_target_tokens {
                progress.t1_hard_cap_tokens = progress.t1_target_tokens;
            }
            progress.tokens_until_next_run = progress
                .tokens_until_next_run
                .min(progress.t1_target_tokens);
        }
    }
    if payload.events.is_empty() {
        return Ok(None);
    }
    Ok(Some(payload))
}

fn parse_mind_injection_from_source(value: &Value) -> Result<Option<MindInjectionPayload>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut payload: MindInjectionPayload =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    payload.status = payload.status.trim().to_ascii_lowercase().replace('_', "-");
    payload.scope = payload.scope.trim().to_string();
    payload.scope_key = payload.scope_key.trim().to_string();
    payload.active_tag = payload
        .active_tag
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    payload.reason = payload
        .reason
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    payload.snapshot_id = payload
        .snapshot_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    payload.payload_hash = payload
        .payload_hash
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if payload.status.is_empty() || payload.scope.is_empty() || payload.scope_key.is_empty() {
        return Ok(None);
    }
    Ok(Some(payload))
}

fn parse_insight_runtime_from_source(
    value: &Value,
) -> Result<Option<InsightRuntimeSnapshot>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut snapshot: InsightRuntimeSnapshot =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    snapshot.last_error = snapshot
        .last_error
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if snapshot.queue_depth < 0 {
        snapshot.queue_depth = 0;
    }
    if snapshot.t3_queue_depth < 0 {
        snapshot.t3_queue_depth = 0;
    }
    Ok(Some(snapshot))
}

fn parse_insight_detached_from_source(
    value: &Value,
) -> Result<Option<InsightDetachedStatusResult>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let mut snapshot: InsightDetachedStatusResult =
        serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    snapshot.jobs.retain(|job| !job.job_id.trim().is_empty());
    snapshot.jobs.sort_by(|a, b| {
        b.created_at_ms
            .cmp(&a.created_at_ms)
            .then_with(|| a.job_id.cmp(&b.job_id))
    });
    snapshot.active_jobs = snapshot
        .jobs
        .iter()
        .filter(|job| {
            matches!(
                job.status,
                InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
            )
        })
        .count();
    if snapshot.status.trim().is_empty() {
        snapshot.status = if snapshot.jobs.is_empty() {
            "idle".to_string()
        } else {
            "ok".to_string()
        };
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
    if app.mode == Mode::Mind && app.mind_search_editing {
        match key.code {
            KeyCode::Esc => {
                app.mind_search_editing = false;
                app.status_note = Some("mind search edit cancelled".to_string());
                return false;
            }
            KeyCode::Enter => {
                app.mind_search_editing = false;
                app.status_note = Some(if app.mind_search_query.trim().is_empty() {
                    "mind search cleared".to_string()
                } else {
                    format!("mind search: {}", app.mind_search_query.trim())
                });
                app.scroll = 0;
                return false;
            }
            KeyCode::Backspace => {
                app.mind_search_query.pop();
                return false;
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.mind_search_query.push(ch);
                return false;
            }
            _ => return false,
        }
    }

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

    if app.config.runtime_mode.is_pulse_pane() {
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Tab
            | KeyCode::Char('1')
            | KeyCode::Char('2')
            | KeyCode::Char('3')
            | KeyCode::Char('4')
            | KeyCode::Char('5')
            | KeyCode::Char('6')
            | KeyCode::Char('7')
            | KeyCode::Char('m')
            | KeyCode::Enter
            | KeyCode::Char('x')
            | KeyCode::Char('e')
            | KeyCode::Char('E')
            | KeyCode::Char('o')
            | KeyCode::Char('i')
            | KeyCode::Char('h')
            | KeyCode::Char('c')
            | KeyCode::Char('u')
            | KeyCode::Char('s')
            | KeyCode::Char('d')
            | KeyCode::Char('O')
            | KeyCode::Char('b')
            | KeyCode::Char('B')
            | KeyCode::Char('F')
            | KeyCode::Char('C')
            | KeyCode::Char('R')
            | KeyCode::Char('H')
            | KeyCode::Char('t')
            | KeyCode::Char('v')
            | KeyCode::Char('p')
            | KeyCode::Char('/')
            | KeyCode::Char('f')
            | KeyCode::Char('A')
            | KeyCode::Char('S')
            | KeyCode::Left
            | KeyCode::Char('[')
            | KeyCode::Right
            | KeyCode::Char(']') => {
                app.mode = Mode::PulsePane;
                app.scroll = 0;
                app.status_note = Some("pulse pane is fixed to local Tabs/Mind/status".to_string());
                return false;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => true,
        KeyCode::Char('1') => {
            if app.config.overview_enabled {
                app.mode = Mode::Overview;
            } else {
                app.mode = Mode::Overseer;
                app.status_note = Some("overview disabled; switched to Overseer".to_string());
            }
            app.scroll = 0;
            false
        }
        KeyCode::Char('2') => {
            app.mode = Mode::Overseer;
            app.scroll = 0;
            false
        }
        KeyCode::Char('3') | KeyCode::Char('m') => {
            app.mode = Mode::Mind;
            app.scroll = 0;
            false
        }
        KeyCode::Char('4') => {
            app.mode = Mode::Fleet;
            app.scroll = 0;
            false
        }
        KeyCode::Char('5') => {
            app.mode = Mode::Work;
            app.scroll = 0;
            false
        }
        KeyCode::Char('6') => {
            app.mode = Mode::Diff;
            app.scroll = 0;
            false
        }
        KeyCode::Char('7') => {
            app.mode = Mode::Health;
            app.scroll = 0;
            false
        }
        KeyCode::Tab => {
            app.cycle_mode();
            app.scroll = 0;
            false
        }
        KeyCode::Enter => {
            if app.mode == Mode::Fleet {
                app.focus_selected_fleet_project();
            } else {
                app.focus_selected_overview_tab();
            }
            false
        }
        KeyCode::Char('x') => {
            if app.mode == Mode::Fleet {
                app.cancel_selected_fleet_job();
            } else {
                app.stop_selected_overview_agent();
            }
            false
        }
        KeyCode::Char('e') => {
            app.capture_selected_pane_evidence();
            false
        }
        KeyCode::Char('E') => {
            app.follow_selected_pane_live();
            false
        }
        KeyCode::Char('o') => {
            app.request_manual_observer_run();
            false
        }
        KeyCode::Char('i') => {
            if app.mode == Mode::Fleet {
                app.launch_fleet_followup(false);
            }
            false
        }
        KeyCode::Char('h') => {
            if app.mode == Mode::Fleet {
                app.launch_fleet_followup(true);
            }
            false
        }
        KeyCode::Char('c') => {
            if app.mode == Mode::Overseer {
                app.request_overseer_consultation(ConsultationPacketKind::Review);
            }
            false
        }
        KeyCode::Char('u') => {
            if app.mode == Mode::Overseer {
                app.request_overseer_consultation(ConsultationPacketKind::HelpRequest);
            }
            false
        }
        KeyCode::Char('s') => {
            if app.mode == Mode::Overseer {
                app.request_spawn_worker();
            }
            false
        }
        KeyCode::Char('d') => {
            if app.mode == Mode::Overseer {
                app.request_delegate_worker();
            }
            false
        }
        KeyCode::Char('O') => {
            app.request_insight_dispatch_chain();
            false
        }
        KeyCode::Char('b') => {
            app.request_insight_bootstrap(true);
            false
        }
        KeyCode::Char('B') => {
            app.request_insight_bootstrap(false);
            false
        }
        KeyCode::Char('F') => {
            app.request_mind_force_finalize();
            false
        }
        KeyCode::Char('C') => {
            app.request_mind_compaction_rebuild();
            false
        }
        KeyCode::Char('R') => {
            app.request_mind_t3_requeue();
            false
        }
        KeyCode::Char('H') => {
            app.request_mind_handshake_rebuild();
            false
        }
        KeyCode::Char('t') => {
            if app.mode == Mode::Mind {
                app.toggle_mind_lane();
            }
            false
        }
        KeyCode::Char('v') => {
            if app.mode == Mode::Mind {
                app.toggle_mind_scope();
            }
            false
        }
        KeyCode::Char('p') => {
            if app.mode == Mode::Mind {
                app.toggle_mind_provenance();
            }
            false
        }
        KeyCode::Char('/') => {
            if app.mode == Mode::Mind {
                app.mind_search_editing = true;
                app.status_note = Some("editing mind search query".to_string());
            }
            false
        }
        KeyCode::Char('f') => {
            if app.mode == Mode::Fleet {
                app.toggle_fleet_plane_filter();
            }
            false
        }
        KeyCode::Char('A') => {
            if app.mode == Mode::Fleet {
                app.toggle_fleet_active_only();
            }
            false
        }
        KeyCode::Char('S') => {
            if app.mode == Mode::Fleet {
                app.toggle_fleet_sort_mode();
            }
            false
        }
        KeyCode::Left | KeyCode::Char('[') => {
            if app.mode == Mode::Fleet {
                app.move_fleet_job_selection(-1);
            }
            false
        }
        KeyCode::Right | KeyCode::Char(']') => {
            if app.mode == Mode::Fleet {
                app.move_fleet_job_selection(1);
            }
            false
        }
        KeyCode::Char('a') => {
            if app.mode == Mode::Overview {
                app.toggle_overview_sort_mode();
            }
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.mode == Mode::Overview {
                app.move_overview_selection(1);
            } else if app.mode == Mode::Fleet {
                app.move_fleet_selection(1);
            } else {
                app.scroll = app.scroll.saturating_add(1);
            }
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.mode == Mode::Overview {
                app.move_overview_selection(-1);
            } else if app.mode == Mode::Fleet {
                app.move_fleet_selection(-1);
            } else {
                app.scroll = app.scroll.saturating_sub(1);
            }
            false
        }
        KeyCode::Char('g') => {
            if app.mode == Mode::Overview {
                app.selected_overview = 0;
            }
            if app.mode == Mode::Fleet {
                app.selected_fleet = 0;
                app.selected_fleet_job = 0;
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

fn in_zellij_session() -> bool {
    env::var("ZELLIJ").is_ok() || env::var("ZELLIJ_SESSION_NAME").is_ok()
}

fn resolve_launch_agent_id() -> String {
    for key in ["AOC_LAUNCH_AGENT_ID", "AOC_AGENT_ID"] {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    if let Ok(output) = Command::new("aoc-agent").arg("--current").output() {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !value.is_empty() {
                return value;
            }
        }
    }

    "pi".to_string()
}

fn sanitize_slug(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn build_worker_launch_plan(
    project_root: &Path,
    agent_id: &str,
    tab_name: &str,
    brief_path: Option<&Path>,
    in_zellij: bool,
) -> WorkerLaunchPlan {
    let mut env = vec![("AOC_LAUNCH_AGENT_ID".to_string(), agent_id.to_string())];
    if let Some(path) = brief_path {
        env.push((
            "AOC_DELEGATION_BRIEF_PATH".to_string(),
            path.display().to_string(),
        ));
    }

    if in_zellij {
        WorkerLaunchPlan {
            program: "aoc-new-tab".to_string(),
            args: vec![
                "--aoc".to_string(),
                "--name".to_string(),
                tab_name.to_string(),
                "--cwd".to_string(),
                project_root.display().to_string(),
            ],
            env,
            cwd: project_root.to_path_buf(),
            tab_name: tab_name.to_string(),
        }
    } else {
        WorkerLaunchPlan {
            program: "aoc-launch".to_string(),
            args: Vec::new(),
            env,
            cwd: project_root.to_path_buf(),
            tab_name: tab_name.to_string(),
        }
    }
}

fn execute_worker_launch_plan(plan: &WorkerLaunchPlan) -> Result<(), String> {
    let mut cmd = Command::new(&plan.program);
    cmd.current_dir(&plan.cwd);
    for (key, value) in &plan.env {
        cmd.env(key, value);
    }
    cmd.args(&plan.args);
    let status = cmd
        .status()
        .map_err(|err| format!("{} failed: {err}", plan.program))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with {}", plan.program, status))
    }
}

fn dump_pane_evidence(session_id: &str, pane_id: &str, output_path: &Path) -> Result<(), String> {
    if pane_id.trim().is_empty() {
        return Err("empty pane id".to_string());
    }
    let output = Command::new("aoc-pane-evidence")
        .arg("--pane-id")
        .arg(pane_id)
        .arg("--session")
        .arg(session_id)
        .output()
        .map_err(|err| format!("aoc-pane-evidence failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(format!("aoc-pane-evidence exited with {}", output.status));
        }
        return Err(stderr);
    }
    fs::write(output_path, &output.stdout)
        .map_err(|err| format!("write evidence failed: {err}"))?;
    Ok(())
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn launch_pane_follow(
    session_id: &str,
    pane_id: &str,
    label: &str,
    project_root: &Path,
) -> Result<(), String> {
    if pane_id.trim().is_empty() {
        return Err("empty pane id".to_string());
    }
    let follow_cmd = format!(
        "exec aoc-pane-evidence --pane-id {} --session {} --follow --scrollback 300",
        shell_single_quote(pane_id),
        shell_single_quote(session_id)
    );
    let title = format!("Follow {}", ellipsize(label, 18));
    let mut cmd = Command::new("zellij");
    cmd.arg("action")
        .arg("new-pane")
        .arg("--floating")
        .arg("--close-on-exit")
        .arg("--borderless")
        .arg("true")
        .arg("--name")
        .arg(&title)
        .arg("--cwd")
        .arg(project_root)
        .arg("--")
        .arg("bash")
        .arg("-lc")
        .arg(follow_cmd);
    let status = cmd
        .status()
        .map_err(|err| format!("zellij new-pane failed: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("zellij action new-pane exited with {}", status))
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
    mut command_rx: mpsc::Receiver<HubOutbound>,
) {
    let _ = tx.send(HubEvent::Disconnected).await;
    while command_rx.recv().await.is_some() {}
}

#[cfg(unix)]
async fn hub_loop(
    config: Config,
    tx: mpsc::Sender<HubEvent>,
    mut command_rx: mpsc::Receiver<HubOutbound>,
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
                            WireMsg::ObserverSnapshot(payload) => {
                                let _ = tx.send(HubEvent::ObserverSnapshot { payload }).await;
                            }
                            WireMsg::ObserverTimeline(payload) => {
                                let _ = tx.send(HubEvent::ObserverTimeline { payload }).await;
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
                            WireMsg::ConsultationResponse(payload) => {
                                let _ = tx
                                    .send(HubEvent::ConsultationResponse {
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
                            let envelope = build_outbound_envelope(&config, command);
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
    let capabilities = vec![
        "snapshot".to_string(),
        "delta".to_string(),
        "heartbeat".to_string(),
        "command".to_string(),
        "command_result".to_string(),
    ];

    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Hello(PulseHelloPayload {
            client_id: config.client_id.clone(),
            role: "subscriber".to_string(),
            capabilities,
            agent_id: None,
            pane_id: None,
            project_root: Some(config.project_root.to_string_lossy().to_string()),
        }),
    }
}

fn build_pulse_subscribe(config: &Config) -> WireEnvelope {
    let mut topics = vec![
        "agent_state".to_string(),
        "command_result".to_string(),
        "layout_state".to_string(),
    ];
    if !config.runtime_mode.is_pulse_pane() {
        topics.push("consultation_response".to_string());
        topics.push("observer_snapshot".to_string());
        topics.push("observer_timeline".to_string());
    }

    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Subscribe(SubscribePayload {
            topics,
            since_seq: None,
        }),
    }
}

fn build_outbound_envelope(config: &Config, outbound: HubOutbound) -> WireEnvelope {
    WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: config.session_id.clone(),
        sender_id: config.client_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: Some(outbound.request_id),
        msg: outbound.msg,
    }
}

fn parse_event_at(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

fn collect_local(config: &Config) -> LocalSnapshot {
    collect_local_with_options(config, true, true, true, None)
}

fn collect_local_with_options(
    config: &Config,
    include_work: bool,
    include_diff: bool,
    include_health: bool,
    previous: Option<&LocalSnapshot>,
) -> LocalSnapshot {
    let session_layout = collect_session_layout(&config.session_id);
    let viewer_tab_index = collect_viewer_tab_index(config, session_layout.as_ref());
    let tab_roster = session_layout
        .as_ref()
        .map(|layout| layout.tabs.clone())
        .or_else(|| previous.map(|snapshot| snapshot.tab_roster.clone()))
        .unwrap_or_default();
    let mut overview = collect_runtime_overview(config, session_layout.as_ref());
    if overview.is_empty() {
        overview = collect_proc_overview(config, session_layout.as_ref());
    }
    let project_roots = collect_project_roots(&overview, &config.project_root);
    let (work, taskmaster_status) = if include_work || include_health {
        collect_local_work(&project_roots)
    } else {
        (
            previous
                .map(|snapshot| snapshot.work.clone())
                .unwrap_or_default(),
            previous
                .map(|snapshot| snapshot.health.taskmaster_status.clone())
                .unwrap_or_else(|| "unknown".to_string()),
        )
    };
    let diff = if include_diff {
        collect_local_diff(&project_roots)
    } else {
        previous
            .map(|snapshot| snapshot.diff.clone())
            .unwrap_or_default()
    };
    let health = if include_health {
        collect_health(config, &taskmaster_status)
    } else {
        previous
            .map(|snapshot| snapshot.health.clone())
            .unwrap_or(HealthSnapshot {
                dependencies: Vec::new(),
                checks: Vec::new(),
                taskmaster_status,
            })
    };
    LocalSnapshot {
        overview,
        viewer_tab_index,
        tab_roster,
        work,
        diff,
        health,
    }
}

fn collect_layout_overview(
    config: &Config,
    existing_rows: &[OverviewRow],
    tab_cache: &HashMap<String, TabMeta>,
) -> (Vec<OverviewRow>, Option<usize>, Vec<TabMeta>) {
    let session_layout = collect_session_layout(&config.session_id);
    let viewer_tab_index = collect_viewer_tab_index(config, session_layout.as_ref());
    let tab_roster = session_layout
        .as_ref()
        .map(|layout| layout.tabs.clone())
        .unwrap_or_default();
    if existing_rows.is_empty() {
        return (Vec::new(), viewer_tab_index, tab_roster);
    }
    let Some(layout) = session_layout.as_ref() else {
        let mut rows = existing_rows.to_vec();
        for row in &mut rows {
            apply_cached_tab_meta(row, tab_cache);
        }
        return (sort_overview_rows(rows), viewer_tab_index, tab_roster);
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
    (sort_overview_rows(rows), viewer_tab_index, tab_roster)
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
    let viewer_scope = config.tab_scope.as_deref();
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
        let tab_name = tab_meta
            .map(|meta| meta.name.clone())
            .or_else(|| snapshot.tab_scope.clone());
        let tab_focused = tab_scope_matches(viewer_scope, snapshot.tab_scope.as_deref())
            || tab_scope_matches(viewer_scope, tab_name.as_deref());
        let session_title = snapshot.session_title;
        rows.insert(
            identity_key.clone(),
            OverviewRow {
                identity_key,
                label: snapshot.agent_label,
                lifecycle: normalize_lifecycle(&snapshot.status),
                snippet: None,
                pane_id: snapshot.pane_id,
                tab_index: tab_meta.map(|meta| meta.index),
                tab_name,
                tab_focused,
                project_root: snapshot.project_root,
                online,
                age_secs: heartbeat_age,
                source: "runtime".to_string(),
                session_title,
                chat_title: snapshot.chat_title,
            },
        );
    }
    rows.into_values().collect()
}

fn collect_session_layout(session_id: &str) -> Option<SessionLayout> {
    if session_id.trim().is_empty() {
        return None;
    }
    if let Ok(Some(snapshot)) = query_session_snapshot(session_id) {
        let mut parsed = SessionLayout::default();
        parsed.pane_ids = snapshot.pane_ids;
        parsed.tabs = snapshot
            .tabs
            .into_iter()
            .filter_map(|tab| {
                usize::try_from(tab.index).ok().map(|index| TabMeta {
                    index,
                    name: tab.name,
                    focused: tab.focused,
                })
            })
            .collect();
        parsed.focused_tab_index = snapshot
            .current_tab_index
            .and_then(|index| usize::try_from(index).ok())
            .or_else(|| {
                parsed
                    .tabs
                    .iter()
                    .find(|tab| tab.focused)
                    .map(|tab| tab.index)
            });
        for pane in snapshot.panes {
            parsed.pane_tabs.insert(
                pane.pane_id,
                TabMeta {
                    index: pane.tab_index as usize,
                    name: pane.tab_name,
                    focused: pane.tab_focused,
                },
            );
        }
        for (project_root, tab) in snapshot.project_tabs {
            parsed.project_tabs.insert(
                project_root,
                TabMeta {
                    index: tab.index as usize,
                    name: tab.name,
                    focused: tab.focused,
                },
            );
        }
        if !parsed.pane_ids.is_empty() || !parsed.project_tabs.is_empty() || !parsed.tabs.is_empty()
        {
            return Some(parsed);
        }
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
    if parsed.pane_ids.is_empty() && parsed.project_tabs.is_empty() && parsed.tabs.is_empty() {
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
            parsed.tabs.push(TabMeta {
                index: current_tab_index,
                name: current_tab_name.clone(),
                focused: current_tab_focused,
            });
            if current_tab_focused {
                parsed.focused_tab_index = Some(current_tab_index);
            }
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
    let viewer_scope = config.tab_scope.as_deref();
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
        let agent_label = env_map.get("AOC_AGENT_LABEL").cloned();
        let agent_id = env_map.get("AOC_AGENT_ID").cloned();
        let agent_run = env_map
            .get("AOC_AGENT_RUN")
            .and_then(|value| parse_bool_flag(value))
            .unwrap_or(false);
        let has_agent_identity = agent_label
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || agent_id
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
        if !has_agent_identity && !agent_run {
            continue;
        }
        let label = agent_label
            .filter(|value| !value.trim().is_empty())
            .or_else(|| agent_id.filter(|value| !value.trim().is_empty()))
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
        let proc_tab_scope = env_map
            .get("AOC_TAB_SCOPE")
            .or_else(|| env_map.get("AOC_TAB_NAME"))
            .or_else(|| env_map.get("ZELLIJ_TAB_NAME"))
            .cloned();
        let tab_name = tab_meta
            .map(|meta| meta.name.clone())
            .or(proc_tab_scope.clone());
        let tab_focused = tab_scope_matches(viewer_scope, proc_tab_scope.as_deref())
            || tab_scope_matches(viewer_scope, tab_name.as_deref());
        rows.entry(key.clone()).or_insert(OverviewRow {
            identity_key: key,
            label,
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id,
            tab_index: tab_meta.map(|meta| meta.index),
            tab_name,
            tab_focused,
            project_root,
            online: true,
            age_secs: None,
            source: "proc".to_string(),
            session_title: None,
            chat_title: None,
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
        dep_status_any(
            "task-control",
            &["aoc-task", "tm", "aoc-taskmaster", "task-master"],
        ),
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

fn dep_status_any(name: &str, candidates: &[&str]) -> DependencyStatus {
    for candidate in candidates {
        if let Some(path) = which_cmd(candidate) {
            return DependencyStatus {
                name: name.to_string(),
                available: true,
                path: Some(path),
            };
        }
    }

    DependencyStatus {
        name: name.to_string(),
        available: false,
        path: None,
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
    let tab_scope = resolve_tab_scope();
    let pulse_socket_path = resolve_pulse_socket_path(&session_id);
    let pulse_theme = resolve_pulse_theme_mode();
    let pulse_custom_theme = resolve_custom_pulse_theme();
    let pulse_vnext_enabled = resolve_pulse_vnext_enabled();
    let overview_enabled = resolve_overview_enabled();
    let runtime_mode = resolve_runtime_mode();
    let start_view = resolve_start_view(runtime_mode);
    let fleet_plane_filter = resolve_fleet_plane_filter();
    let layout_source = resolve_layout_source();
    let client_id = format!("aoc-pulse-{}", std::process::id());
    let project_root = resolve_project_root();
    let mind_project_scoped = resolve_mind_project_scoped();
    let state_dir = resolve_state_dir();
    Config {
        session_id,
        pane_id,
        tab_scope,
        pulse_socket_path,
        pulse_theme,
        pulse_custom_theme,
        pulse_vnext_enabled,
        overview_enabled,
        runtime_mode,
        start_view,
        fleet_plane_filter,
        layout_source,
        client_id,
        project_root,
        mind_project_scoped,
        state_dir,
    }
}

fn resolve_local_layout_refresh_ms() -> u64 {
    std::env::var("AOC_MISSION_CONTROL_LAYOUT_REFRESH_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|value| value.clamp(LOCAL_LAYOUT_REFRESH_MS_MIN, LOCAL_LAYOUT_REFRESH_MS_MAX))
        .unwrap_or(LOCAL_LAYOUT_REFRESH_MS_DEFAULT)
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

fn resolve_overview_enabled() -> bool {
    std::env::var("AOC_PULSE_OVERVIEW_ENABLED")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(true)
}

fn parse_runtime_mode(value: &str) -> Option<RuntimeMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pulse-pane" | "pulse_pane" | "pulse" => Some(RuntimeMode::PulsePane),
        "mission-control" | "mission_control" | "mission" | "mc" => {
            Some(RuntimeMode::MissionControl)
        }
        _ => None,
    }
}

fn parse_start_view(value: &str) -> Option<Mode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "overview" | "ov" => Some(Mode::Overview),
        "overseer" => Some(Mode::Overseer),
        "mind" => Some(Mode::Mind),
        "fleet" | "detached" | "subagents" => Some(Mode::Fleet),
        "work" => Some(Mode::Work),
        "diff" => Some(Mode::Diff),
        "health" => Some(Mode::Health),
        _ => None,
    }
}

fn parse_fleet_plane_filter(value: &str) -> Option<FleetPlaneFilter> {
    match value.trim().to_ascii_lowercase().as_str() {
        "all" => Some(FleetPlaneFilter::All),
        "delegated" | "specialist" | "subagents" => Some(FleetPlaneFilter::Delegated),
        "mind" => Some(FleetPlaneFilter::Mind),
        _ => None,
    }
}

fn resolve_runtime_mode() -> RuntimeMode {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--mode=") {
            if let Some(mode) = parse_runtime_mode(value) {
                return mode;
            }
        }
        if arg == "--mode" {
            if let Some(value) = args.next() {
                if let Some(mode) = parse_runtime_mode(&value) {
                    return mode;
                }
            }
        }
    }

    if let Some(mode) = std::env::var("AOC_MISSION_CONTROL_MODE")
        .ok()
        .as_deref()
        .and_then(parse_runtime_mode)
    {
        return mode;
    }

    if std::env::var("AOC_PULSE_LIGHT_PANE")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(false)
    {
        return RuntimeMode::PulsePane;
    }

    RuntimeMode::MissionControl
}

fn resolve_local_snapshot_refresh_secs() -> u64 {
    std::env::var("AOC_MISSION_CONTROL_SNAPSHOT_REFRESH_SECS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|value| {
            value.clamp(
                LOCAL_SNAPSHOT_REFRESH_SECS_MIN,
                LOCAL_SNAPSHOT_REFRESH_SECS_MAX,
            )
        })
        .unwrap_or(LOCAL_SNAPSHOT_REFRESH_SECS_DEFAULT)
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

fn resolve_start_view(runtime_mode: RuntimeMode) -> Option<Mode> {
    if runtime_mode.is_pulse_pane() {
        return None;
    }
    std::env::var("AOC_MISSION_CONTROL_START_VIEW")
        .ok()
        .as_deref()
        .and_then(parse_start_view)
}

fn resolve_fleet_plane_filter() -> FleetPlaneFilter {
    std::env::var("AOC_MISSION_CONTROL_FLEET_PLANE")
        .ok()
        .as_deref()
        .and_then(parse_fleet_plane_filter)
        .unwrap_or(FleetPlaneFilter::All)
}

fn parse_pulse_theme_mode(value: &str) -> Option<PulseThemeMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "terminal" | "auto" => Some(PulseThemeMode::Terminal),
        "dark" => Some(PulseThemeMode::Dark),
        "light" => Some(PulseThemeMode::Light),
        _ => None,
    }
}

fn resolve_pulse_theme_mode() -> PulseThemeMode {
    std::env::var("AOC_PULSE_THEME")
        .ok()
        .as_deref()
        .and_then(parse_pulse_theme_mode)
        .unwrap_or(PulseThemeMode::Terminal)
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
    if let Ok(value) = std::env::var("ZELLIJ_SESSION_NAME") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = std::env::var("AOC_SESSION_ID") {
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

fn resolve_tab_scope() -> Option<String> {
    for key in ["AOC_TAB_SCOPE", "AOC_TAB_NAME", "ZELLIJ_TAB_NAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
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

fn resolve_mind_project_scoped() -> bool {
    std::env::var("AOC_MIND_PROJECT_SCOPED")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(false)
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
    use crossterm::event::KeyModifiers;

    fn test_config() -> Config {
        Config {
            session_id: "session-test".to_string(),
            pane_id: "12".to_string(),
            tab_scope: Some("agent".to_string()),
            pulse_socket_path: PathBuf::from("/tmp/pulse-test.sock"),
            pulse_theme: PulseThemeMode::Terminal,
            pulse_custom_theme: None,
            pulse_vnext_enabled: true,
            overview_enabled: true,
            runtime_mode: RuntimeMode::MissionControl,
            start_view: None,
            fleet_plane_filter: FleetPlaneFilter::All,
            layout_source: LayoutSource::Hub,
            client_id: "pulse-test".to_string(),
            project_root: PathBuf::from("/tmp"),
            mind_project_scoped: false,
            state_dir: PathBuf::from("/tmp"),
        }
    }

    fn empty_local() -> LocalSnapshot {
        LocalSnapshot {
            overview: Vec::new(),
            viewer_tab_index: None,
            tab_roster: Vec::new(),
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
    fn light_pane_defaults_to_pulse_mode_and_renders_local_sections() {
        let mut cfg = test_config();
        cfg.runtime_mode = RuntimeMode::PulsePane;
        let (tx, _rx) = mpsc::channel(4);
        let local = LocalSnapshot {
            overview: vec![OverviewRow {
                identity_key: "session-test::12".to_string(),
                label: "OpenCode".to_string(),
                lifecycle: "running".to_string(),
                snippet: Some("synthesizing local mind state".to_string()),
                pane_id: "12".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: true,
                project_root: "/tmp".to_string(),
                online: true,
                age_secs: Some(2),
                source: "runtime".to_string(),
                session_title: Some("Implement Custom Layout Support".to_string()),
                chat_title: None,
            }],
            viewer_tab_index: Some(1),
            tab_roster: vec![TabMeta {
                index: 1,
                name: "Agent".to_string(),
                focused: true,
            }],
            work: vec![WorkProject {
                project_root: "/tmp".to_string(),
                scope: "Agent".to_string(),
                tags: vec![WorkTagRow {
                    tag: "session-overseer".to_string(),
                    counts: TaskCounts {
                        total: 3,
                        pending: 1,
                        in_progress: 1,
                        done: 1,
                        blocked: 0,
                    },
                    in_progress_titles: vec!["#149 split pulse pane".to_string()],
                }],
            }],
            diff: Vec::new(),
            health: HealthSnapshot {
                dependencies: Vec::new(),
                checks: Vec::new(),
                taskmaster_status: "available".to_string(),
            },
        };
        let app = App::new(cfg, tx, local);
        assert_eq!(app.mode, Mode::PulsePane);
        let rendered =
            render_pulse_pane_lines(&app, pulse_theme(PulseThemeMode::Terminal), false, 120)
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n");
        assert!(rendered.contains("pulse-pane"));
        assert!(rendered.contains("Tabs"));
        assert!(rendered.contains("1 Agent"));
        assert!(rendered.contains("Implement Custom Layout Support"));
        assert!(rendered.contains("Tasks"));
        assert!(rendered.contains("Mind"));
        assert!(rendered.contains("Health"));
        assert!(!rendered.contains("Session Overseer"));
    }

    #[test]
    fn pulse_pane_defaults_pi_subtitle_to_new_without_explicit_title() {
        let mut cfg = test_config();
        cfg.runtime_mode = RuntimeMode::PulsePane;
        let (tx, _rx) = mpsc::channel(4);
        let local = LocalSnapshot {
            overview: vec![OverviewRow {
                identity_key: "session-test::12".to_string(),
                label: "PI Agent (npm)".to_string(),
                lifecycle: "running".to_string(),
                snippet: Some("Implement Custom Layout Support".to_string()),
                pane_id: "12".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: true,
                project_root: "/tmp".to_string(),
                online: true,
                age_secs: Some(2),
                source: "runtime".to_string(),
                session_title: None,
                chat_title: None,
            }],
            viewer_tab_index: Some(1),
            tab_roster: vec![TabMeta {
                index: 1,
                name: "Agent".to_string(),
                focused: true,
            }],
            work: Vec::new(),
            diff: Vec::new(),
            health: HealthSnapshot {
                dependencies: Vec::new(),
                checks: Vec::new(),
                taskmaster_status: "available".to_string(),
            },
        };
        let app = App::new(cfg, tx, local);
        let rendered =
            render_pulse_pane_lines(&app, pulse_theme(PulseThemeMode::Terminal), false, 120)
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n");

        assert!(rendered.contains("PI Agent (npm) — new"));
    }

    #[test]
    fn pulse_pane_renders_roster_tabs_without_agent_rows() {
        let mut cfg = test_config();
        cfg.runtime_mode = RuntimeMode::PulsePane;
        let (tx, _rx) = mpsc::channel(4);
        let local = LocalSnapshot {
            overview: Vec::new(),
            viewer_tab_index: Some(2),
            tab_roster: vec![
                TabMeta {
                    index: 1,
                    name: "Agent".to_string(),
                    focused: false,
                },
                TabMeta {
                    index: 2,
                    name: "Review".to_string(),
                    focused: true,
                },
            ],
            work: Vec::new(),
            diff: Vec::new(),
            health: HealthSnapshot {
                dependencies: Vec::new(),
                checks: Vec::new(),
                taskmaster_status: "available".to_string(),
            },
        };
        let app = App::new(cfg, tx, local);
        let rendered =
            render_pulse_pane_lines(&app, pulse_theme(PulseThemeMode::Terminal), true, 80)
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n");

        assert!(rendered.contains("Tabs"));
        assert!(rendered.contains("1 Agent"));
        assert!(rendered.contains("2 Review"));
    }

    #[test]
    fn pulse_pane_blocks_mode_switches_and_orchestrator_drilldown() {
        let mut cfg = test_config();
        cfg.runtime_mode = RuntimeMode::PulsePane;
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(cfg, tx, empty_local());
        let mut refresh_requested = false;

        handle_key(
            KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE),
            &mut app,
            &mut refresh_requested,
        );
        assert_eq!(app.mode, Mode::PulsePane);
        assert_eq!(
            app.status_note.as_deref(),
            Some("pulse pane is fixed to local Tabs/Mind/status")
        );

        app.status_note = None;
        handle_key(
            KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE),
            &mut app,
            &mut refresh_requested,
        );
        assert_eq!(app.mode, Mode::PulsePane);
        assert_eq!(
            app.status_note.as_deref(),
            Some("pulse pane is fixed to local Tabs/Mind/status")
        );
        assert!(!refresh_requested);
    }

    #[test]
    fn pulse_pane_allows_local_refresh() {
        let mut cfg = test_config();
        cfg.runtime_mode = RuntimeMode::PulsePane;
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(cfg, tx, empty_local());
        let mut refresh_requested = false;

        handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            &mut app,
            &mut refresh_requested,
        );

        assert!(refresh_requested);
        assert_eq!(app.mode, Mode::PulsePane);
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
        assert_eq!(payload.tab_scope, None);
    }

    #[test]
    fn status_payload_extracts_tab_scope_from_source() {
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
                    "project_root": "/repo",
                    "tab_scope": "Agent"
                }
            })),
        };

        let payload = status_payload_from_state(&state);
        assert_eq!(payload.tab_scope.as_deref(), Some("Agent"));
    }

    #[test]
    fn tab_scope_matches_ignores_case_and_whitespace() {
        assert!(tab_scope_matches(Some(" Agent  "), Some("agent")));
        assert!(!tab_scope_matches(Some("agent"), Some("review")));
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
    fn command_result_keeps_pending_on_accepted_status() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.pending_commands.insert(
            "req-2".to_string(),
            PendingCommand {
                command: "run_validation".to_string(),
                target: "pane-7".to_string(),
            },
        );

        app.apply_hub_event(HubEvent::CommandResult {
            payload: CommandResultPayload {
                command: "run_validation".to_string(),
                status: "accepted".to_string(),
                message: Some("queued".to_string()),
                error: None,
            },
            request_id: Some("req-2".to_string()),
        });

        assert!(app.pending_commands.contains_key("req-2"));
        assert!(app.status_note.unwrap_or_default().contains("queued"));
    }

    #[test]
    fn command_result_ignores_stale_request_ids() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.status_note = Some("unchanged".to_string());

        app.apply_hub_event(HubEvent::CommandResult {
            payload: CommandResultPayload {
                command: "pause_and_summarize".to_string(),
                status: "ok".to_string(),
                message: Some("late duplicate".to_string()),
                error: None,
            },
            request_id: Some("req-stale".to_string()),
        });

        assert_eq!(app.status_note.as_deref(), Some("unchanged"));
    }

    #[test]
    fn overseer_consultation_queues_review_request_for_peer_worker() {
        let (tx, mut rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Overseer;
        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::12".to_string(),
                        pane_id: "12".to_string(),
                        role: Some("builder".to_string()),
                        status: WorkerStatus::Active,
                        summary: Some("implementing transport".to_string()),
                        assignment: aoc_core::session_overseer::WorkerAssignment {
                            task_id: Some("160".to_string()),
                            tag: Some("mind".to_string()),
                            epic_id: None,
                        },
                        plan_alignment: PlanAlignment::Medium,
                        ..Default::default()
                    },
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::24".to_string(),
                        pane_id: "24".to_string(),
                        role: Some("reviewer".to_string()),
                        status: WorkerStatus::Active,
                        summary: Some("available for review".to_string()),
                        assignment: aoc_core::session_overseer::WorkerAssignment {
                            task_id: Some("161".to_string()),
                            tag: Some("mind".to_string()),
                            epic_id: None,
                        },
                        plan_alignment: PlanAlignment::High,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        });

        app.request_overseer_consultation(ConsultationPacketKind::Review);

        let outbound = rx.try_recv().expect("consultation outbound queued");
        let WireMsg::ConsultationRequest(payload) = outbound.msg else {
            panic!("expected consultation request")
        };
        assert_eq!(payload.requesting_agent_id, "session-test::12");
        assert_eq!(payload.target_agent_id, "session-test::24");
        assert_eq!(payload.packet.kind, ConsultationPacketKind::Review);
        assert_eq!(
            app.pending_consultations
                .get(&outbound.request_id)
                .map(|value| value.responder.as_str()),
            Some("session-test::24")
        );
    }

    #[test]
    fn consultation_response_clears_pending_on_terminal_status() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.pending_consultations.insert(
            "req-consult".to_string(),
            PendingConsultation {
                kind: ConsultationPacketKind::Review,
                requester: "session-test::12".to_string(),
                responder: "session-test::24".to_string(),
                request_packet: ConsultationPacket {
                    packet_id: "packet-request".to_string(),
                    kind: ConsultationPacketKind::Review,
                    identity: ConsultationIdentity {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::12".to_string(),
                        conversation_id: Some("conv-req".to_string()),
                        ..Default::default()
                    },
                    task_context: ConsultationTaskContext {
                        active_tag: Some("mind".to_string()),
                        task_ids: vec!["165".to_string()],
                        focus_summary: Some("persist consultation outcomes".to_string()),
                    },
                    summary: Some("request review".to_string()),
                    ..Default::default()
                },
            },
        );

        app.apply_hub_event(HubEvent::ConsultationResponse {
            payload: ConsultationResponsePayload {
                consultation_id: "consult-1".to_string(),
                requesting_agent_id: "session-test::12".to_string(),
                responding_agent_id: "session-test::24".to_string(),
                status: ConsultationStatus::Completed,
                packet: None,
                message: Some("review completed".to_string()),
                error: None,
            },
            request_id: Some("req-consult".to_string()),
        });

        assert!(!app.pending_consultations.contains_key("req-consult"));
        assert!(app
            .status_note
            .unwrap_or_default()
            .contains("review completed"));
    }

    #[test]
    fn consultation_response_persists_outcome_into_mind_store() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mc-consult-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&test_root).expect("create test root");

        let mut config = test_config();
        config.project_root = test_root.clone();
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(config, tx, empty_local());
        app.pending_consultations.insert(
            "req-consult".to_string(),
            PendingConsultation {
                kind: ConsultationPacketKind::Review,
                requester: "session-test::12".to_string(),
                responder: "session-test::24".to_string(),
                request_packet: ConsultationPacket {
                    packet_id: "packet-request".to_string(),
                    kind: ConsultationPacketKind::Review,
                    identity: ConsultationIdentity {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::12".to_string(),
                        conversation_id: Some("conv-req".to_string()),
                        ..Default::default()
                    },
                    task_context: ConsultationTaskContext {
                        active_tag: Some("mind".to_string()),
                        task_ids: vec!["165".to_string()],
                        focus_summary: Some("persist consultation outcomes".to_string()),
                    },
                    summary: Some("request peer review".to_string()),
                    evidence_refs: vec![
                        aoc_core::consultation_contracts::ConsultationEvidenceRef {
                            reference: "file:docs/mission-control.md".to_string(),
                            label: Some("mission control docs".to_string()),
                            path: Some("docs/mission-control.md".to_string()),
                            relation: Some("reads".to_string()),
                        },
                    ],
                    ..Default::default()
                },
            },
        );

        app.apply_hub_event(HubEvent::ConsultationResponse {
            payload: ConsultationResponsePayload {
                consultation_id: "consult-1".to_string(),
                requesting_agent_id: "session-test::12".to_string(),
                responding_agent_id: "session-test::24".to_string(),
                status: ConsultationStatus::Completed,
                packet: Some(ConsultationPacket {
                    packet_id: "packet-response".to_string(),
                    kind: ConsultationPacketKind::Review,
                    identity: ConsultationIdentity {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::24".to_string(),
                        conversation_id: Some("conv-resp".to_string()),
                        ..Default::default()
                    },
                    summary: Some("peer review found one follow-up".to_string()),
                    current_plan: vec![aoc_core::consultation_contracts::ConsultationPlanItem {
                        title: "tighten persistence coverage".to_string(),
                        ..Default::default()
                    }],
                    confidence: ConsultationConfidence {
                        overall_bps: Some(8700),
                        rationale: Some("bounded evidence refs and live worker state".to_string()),
                    },
                    freshness: ConsultationFreshness {
                        packet_generated_at: Some("2026-03-12T18:00:00Z".to_string()),
                        ..Default::default()
                    },
                    evidence_refs: vec![
                        aoc_core::consultation_contracts::ConsultationEvidenceRef {
                            reference: "file:crates/aoc-mission-control/src/main.rs".to_string(),
                            label: Some("mission control source".to_string()),
                            path: Some("crates/aoc-mission-control/src/main.rs".to_string()),
                            relation: Some("modified".to_string()),
                        },
                    ],
                    ..Default::default()
                }),
                message: Some("review completed".to_string()),
                error: None,
            },
            request_id: Some("req-consult".to_string()),
        });

        let store = aoc_storage::MindStore::open(mind_store_path(&test_root)).expect("open store");
        let artifact = store
            .artifact_by_id("consult:consult-1")
            .expect("artifact lookup")
            .expect("consultation artifact persisted");
        assert_eq!(artifact.kind, "t2");
        assert_eq!(artifact.conversation_id, "conv-req");
        assert!(artifact.text.contains("# Consultation outcome"));
        assert!(artifact.text.contains("peer review found one follow-up"));
        assert!(artifact
            .trace_ids
            .iter()
            .any(|value| value == "consultation:consult-1"));
        assert_eq!(
            store
                .artifact_task_links_for_artifact("consult:consult-1")
                .expect("task links")
                .len(),
            1
        );
        assert_eq!(
            store
                .artifact_file_links("consult:consult-1")
                .expect("file links")
                .len(),
            2
        );
        assert_eq!(
            store
                .semantic_provenance_for_artifact("consult:consult-1")
                .expect("semantic provenance")
                .len(),
            1
        );

        let _ = fs::remove_dir_all(&test_root);
    }

    #[test]
    fn orchestrator_tool_surface_marks_spawn_and_delegate_ready() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Overseer;
        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::12".to_string(),
                        pane_id: "12".to_string(),
                        status: WorkerStatus::Active,
                        plan_alignment: PlanAlignment::Medium,
                        ..Default::default()
                    },
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::24".to_string(),
                        pane_id: "24".to_string(),
                        status: WorkerStatus::Active,
                        plan_alignment: PlanAlignment::High,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        });

        let tools = app.orchestrator_tools();
        assert!(tools
            .iter()
            .any(|tool| tool.id == OrchestratorToolId::WorkerReview
                && tool.status == OrchestratorToolStatus::Ready));
        assert!(tools
            .iter()
            .any(|tool| tool.id == OrchestratorToolId::WorkerSpawn
                && tool.status == OrchestratorToolStatus::Ready
                && tool.shortcut == Some("s")));
        assert!(tools
            .iter()
            .any(|tool| tool.id == OrchestratorToolId::WorkerDelegate
                && tool.status == OrchestratorToolStatus::Ready
                && tool.shortcut == Some("d")));
    }

    #[test]
    fn build_worker_launch_plan_uses_new_tab_in_zellij_and_launch_otherwise() {
        let project_root = PathBuf::from("/tmp/project-root");
        let brief_path = PathBuf::from("/tmp/delegation.md");

        let zellij_plan =
            build_worker_launch_plan(&project_root, "pi", "Worker 3", Some(&brief_path), true);
        assert_eq!(zellij_plan.program, "aoc-new-tab");
        assert_eq!(
            zellij_plan.args,
            vec![
                "--aoc".to_string(),
                "--name".to_string(),
                "Worker 3".to_string(),
                "--cwd".to_string(),
                "/tmp/project-root".to_string(),
            ]
        );
        assert!(zellij_plan
            .env
            .contains(&("AOC_LAUNCH_AGENT_ID".to_string(), "pi".to_string())));
        assert!(zellij_plan.env.contains(&(
            "AOC_DELEGATION_BRIEF_PATH".to_string(),
            "/tmp/delegation.md".to_string()
        )));

        let standalone_plan =
            build_worker_launch_plan(&project_root, "pi", "Worker 3", None, false);
        assert_eq!(standalone_plan.program, "aoc-launch");
        assert!(standalone_plan.args.is_empty());
    }

    #[test]
    fn delegation_brief_captures_selected_worker_context() {
        let (tx, _rx) = mpsc::channel(4);
        let app = App::new(test_config(), tx, empty_local());
        let worker = WorkerSnapshot {
            session_id: "session-test".to_string(),
            agent_id: "session-test::24".to_string(),
            pane_id: "24".to_string(),
            role: Some("reviewer".to_string()),
            status: WorkerStatus::Blocked,
            summary: Some("waiting on fixture regeneration".to_string()),
            blocker: Some("need fresh snapshot".to_string()),
            assignment: aoc_core::session_overseer::WorkerAssignment {
                task_id: Some("164".to_string()),
                tag: Some("mind".to_string()),
                epic_id: None,
            },
            plan_alignment: PlanAlignment::Medium,
            drift_risk: DriftRisk::Medium,
            ..Default::default()
        };

        let brief = app.render_delegation_brief(&worker);
        assert!(brief.contains("Mission Control delegation brief"));
        assert!(brief.contains("source worker: session-test::24"));
        assert!(brief.contains("task: 164"));
        assert!(brief.contains("need fresh snapshot"));
        assert!(brief.contains("waiting on fixture regeneration"));
    }

    #[test]
    fn orchestration_graph_ir_compiles_reviewable_delegate_path() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Overseer;
        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::12".to_string(),
                        pane_id: "12".to_string(),
                        role: Some("implementer".to_string()),
                        status: WorkerStatus::Active,
                        assignment: aoc_core::session_overseer::WorkerAssignment {
                            task_id: Some("166".to_string()),
                            tag: Some("mind".to_string()),
                            epic_id: None,
                        },
                        ..Default::default()
                    },
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::24".to_string(),
                        pane_id: "24".to_string(),
                        status: WorkerStatus::Active,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        });

        let graph = app.orchestration_graph_ir();
        assert_eq!(graph.session_id, "session-test");
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == OrchestrationGraphNodeKind::Session));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == OrchestrationGraphNodeKind::Artifact
                && node.label == "delegation brief"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == OrchestrationGraphEdgeKind::Writes
                && edge.summary.contains("delegation brief")));
        assert!(graph.compile_paths.iter().any(|path| {
            path.entry_tool == OrchestratorToolId::WorkerDelegate
                && path
                    .steps
                    .iter()
                    .any(|step| step.contains("write delegation brief"))
                && path
                    .steps
                    .iter()
                    .any(|step| step.contains("AOC_DELEGATION_BRIEF_PATH"))
        }));
    }

    #[test]
    fn overseer_render_includes_orchestrator_tool_surface() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Overseer;
        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::12".to_string(),
                        pane_id: "12".to_string(),
                        status: WorkerStatus::Active,
                        plan_alignment: PlanAlignment::Medium,
                        ..Default::default()
                    },
                    WorkerSnapshot {
                        session_id: "session-test".to_string(),
                        agent_id: "session-test::24".to_string(),
                        pane_id: "24".to_string(),
                        status: WorkerStatus::Active,
                        plan_alignment: PlanAlignment::High,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        });

        let rendered = render_overseer_lines(&app, pulse_theme(PulseThemeMode::Terminal), false)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Mission Control tools"));
        assert!(rendered.contains("peer review"));
        assert!(rendered.contains("spawn worker"));
        assert!(rendered.contains("Reviewable compile"));
        assert!(rendered.contains("graph "));
        assert!(rendered.contains("plan [ready] delegate task"));
    }

    #[test]
    fn overseer_snapshot_ignores_other_sessions() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());

        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "other-session".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![WorkerSnapshot {
                    session_id: "other-session".to_string(),
                    agent_id: "other-session::1".to_string(),
                    pane_id: "1".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
        });

        assert!(app.overseer_snapshot().is_none());
        assert!(app.overseer_workers().is_empty());
    }

    #[test]
    fn overseer_timeline_ignores_other_sessions() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());

        app.apply_hub_event(HubEvent::ObserverTimeline {
            payload: ObserverTimelinePayload {
                session_id: "other-session".to_string(),
                generated_at_ms: Some(1_700_000_000_123),
                entries: vec![ObserverTimelineEntry {
                    event_id: "evt-9".to_string(),
                    session_id: "other-session".to_string(),
                    agent_id: "other-session::1".to_string(),
                    ..Default::default()
                }],
            },
        });

        assert!(app.overseer_timeline().is_empty());
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
                session_title: None,
                chat_title: None,
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
                session_title: None,
                chat_title: None,
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
                session_title: None,
                chat_title: None,
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
                session_title: None,
                chat_title: None,
            },
        ];

        let sorted = sort_overview_rows(rows);
        assert_eq!(sorted[0].pane_id, "2");
        assert_eq!(sorted[1].pane_id, "10");
    }

    #[test]
    fn overview_attention_sort_prioritizes_blockers_over_layout_order() {
        let rows = vec![
            OverviewRow {
                identity_key: "session-test::1".to_string(),
                label: "pane-1".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "1".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: false,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "hub".to_string(),
                session_title: None,
                chat_title: None,
            },
            OverviewRow {
                identity_key: "session-test::2".to_string(),
                label: "pane-2".to_string(),
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
                session_title: None,
                chat_title: None,
            },
        ];

        let sorted = sort_overview_rows_attention(rows);
        assert_eq!(sorted[0].identity_key, "session-test::2");
        assert_eq!(sorted[1].identity_key, "session-test::1");
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
                session_title: None,
                chat_title: None,
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
                session_title: None,
                chat_title: None,
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
                },
                "mind_observer": {
                    "events": [
                        {
                            "status": "fallback",
                            "trigger": "task_completed",
                            "runtime": "deterministic",
                            "attempt_count": 2,
                            "latency_ms": 95,
                            "reason": "semantic observer failed (timeout)",
                            "failure_kind": "timeout",
                            "conversation_id": "conv-1",
                            "completed_at": "2026-02-25T16:30:00Z"
                        }
                    ]
                },
                "insight_runtime": {
                    "reflector_enabled": true,
                    "reflector_jobs_completed": 2,
                    "reflector_jobs_failed": 1,
                    "reflector_lock_conflicts": 3,
                    "t3_enabled": true,
                    "t3_jobs_completed": 4,
                    "t3_jobs_failed": 1,
                    "t3_jobs_requeued": 2,
                    "t3_jobs_dead_lettered": 1,
                    "t3_lock_conflicts": 2,
                    "t3_queue_depth": 6,
                    "supervisor_runs": 4,
                    "supervisor_failures": 1,
                    "queue_depth": 5
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

        assert_eq!(app.hub.mind.len(), 1);
        let mind = app
            .hub
            .mind
            .get("session-test::12")
            .expect("mind observer payload should exist");
        assert_eq!(mind.events.len(), 1);
        assert_eq!(mind.events[0].status, MindObserverFeedStatus::Fallback);

        let insight = app
            .hub
            .insight_runtime
            .get("session-test::12")
            .expect("insight runtime payload should exist");
        assert_eq!(insight.queue_depth, 5);
        assert_eq!(insight.reflector_jobs_completed, 2);
        assert_eq!(insight.t3_queue_depth, 6);
        assert_eq!(insight.t3_jobs_completed, 4);

        app.mode = Mode::Health;
        assert_eq!(app.mode_source(), "hub");
        assert_eq!(app.health_rows().len(), 1);
    }

    #[test]
    fn mind_rows_filter_to_active_tab_scope_and_render_fallback_badge() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);
        app.mode = Mode::Mind;

        let fallback_event = serde_json::json!({
            "status": "fallback",
            "trigger": "task_completed",
            "runtime": "deterministic",
            "attempt_count": 2,
            "latency_ms": 220,
            "reason": "semantic observer failed (timeout)",
            "completed_at": "2026-02-26T06:45:00Z",
            "progress": {
                "t0_estimated_tokens": 7612,
                "t1_target_tokens": 28000,
                "t1_hard_cap_tokens": 32000,
                "tokens_until_next_run": 20388
            }
        });
        let success_event = serde_json::json!({
            "status": "success",
            "trigger": "token_threshold",
            "runtime": "pi-semantic",
            "attempt_count": 1,
            "latency_ms": 80,
            "completed_at": "2026-02-26T06:44:00Z"
        });

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![
                    AgentState {
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
                                "project_root": "/repo",
                                "tab_scope": "agent"
                            },
                            "mind_observer": {
                                "events": [fallback_event]
                            }
                        })),
                    },
                    AgentState {
                        agent_id: "session-test::99".to_string(),
                        session_id: "session-test".to_string(),
                        pane_id: "99".to_string(),
                        lifecycle: "running".to_string(),
                        snippet: None,
                        last_heartbeat_ms: Some(1),
                        last_activity_ms: Some(1),
                        updated_at_ms: Some(1),
                        source: Some(serde_json::json!({
                            "agent_status": {
                                "agent_label": "Other",
                                "project_root": "/repo",
                                "tab_scope": "review"
                            },
                            "mind_observer": {
                                "events": [success_event]
                            }
                        })),
                    },
                ],
            },
            event_at: Utc::now(),
        });

        let rows = app.mind_rows();
        assert_eq!(rows.len(), 1, "non-active tab rows should be filtered out");
        assert_eq!(rows[0].pane_id, "12");
        assert_eq!(rows[0].event.status, MindObserverFeedStatus::Fallback);

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("[t1]"));
        assert!(rendered.contains("[fallback]"));
        assert!(rendered.contains("[task]"));
        assert!(rendered.contains("runtime:det"));
        assert!(rendered.contains("t0:7612/28000"));
    }

    #[test]
    fn manual_observer_shortcut_queues_run_observer_command() {
        let (tx, mut rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.set_local(LocalSnapshot {
            overview: vec![OverviewRow {
                identity_key: "session-test::12".to_string(),
                label: "OpenCode".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "12".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: true,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "runtime".to_string(),
                session_title: None,
                chat_title: None,
            }],
            viewer_tab_index: Some(1),
            tab_roster: vec![TabMeta {
                index: 1,
                name: "Agent".to_string(),
                focused: true,
            }],
            work: Vec::new(),
            diff: Vec::new(),
            health: empty_local().health,
        });

        app.request_manual_observer_run();
        let command = rx.try_recv().expect("manual command should be queued");
        let WireMsg::Command(payload) = command.msg else {
            panic!("expected command")
        };
        assert_eq!(payload.command, "run_observer");
        assert_eq!(payload.target_agent_id.as_deref(), Some("session-test::12"));
        assert_eq!(
            payload
                .args
                .get("trigger")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            "manual_shortcut"
        );
    }

    #[test]
    fn mind_shortcuts_queue_insight_and_operator_commands() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;
        app.set_local(LocalSnapshot {
            overview: vec![OverviewRow {
                identity_key: "session-test::12".to_string(),
                label: "OpenCode".to_string(),
                lifecycle: "running".to_string(),
                snippet: None,
                pane_id: "12".to_string(),
                tab_index: Some(1),
                tab_name: Some("Agent".to_string()),
                tab_focused: true,
                project_root: "/repo".to_string(),
                online: true,
                age_secs: Some(1),
                source: "runtime".to_string(),
                session_title: None,
                chat_title: None,
            }],
            viewer_tab_index: Some(1),
            tab_roster: vec![TabMeta {
                index: 1,
                name: "Agent".to_string(),
                focused: true,
            }],
            work: Vec::new(),
            diff: Vec::new(),
            health: empty_local().health,
        });

        app.request_insight_dispatch_chain();
        app.request_insight_bootstrap(true);
        app.request_mind_force_finalize();
        app.request_mind_compaction_rebuild();
        app.request_mind_t3_requeue();
        app.request_mind_handshake_rebuild();

        let first = rx.try_recv().expect("dispatch command");
        let WireMsg::Command(first_payload) = first.msg else {
            panic!("expected command")
        };
        assert_eq!(first_payload.command, "insight_dispatch");
        assert_eq!(
            first_payload.target_agent_id.as_deref(),
            Some("session-test::12")
        );
        assert_eq!(
            first_payload
                .args
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            "chain"
        );

        let second = rx.try_recv().expect("bootstrap command");
        let WireMsg::Command(second_payload) = second.msg else {
            panic!("expected command")
        };
        assert_eq!(second_payload.command, "insight_bootstrap");
        assert_eq!(
            second_payload
                .args
                .get("dry_run")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            true
        );

        let third = rx.try_recv().expect("force finalize command");
        let WireMsg::Command(third_payload) = third.msg else {
            panic!("expected command")
        };
        assert_eq!(third_payload.command, "mind_finalize_session");

        let fourth = rx.try_recv().expect("compaction rebuild command");
        let WireMsg::Command(fourth_payload) = fourth.msg else {
            panic!("expected command")
        };
        assert_eq!(fourth_payload.command, "mind_compaction_rebuild");

        let fifth = rx.try_recv().expect("t3 requeue command");
        let WireMsg::Command(fifth_payload) = fifth.msg else {
            panic!("expected command")
        };
        assert_eq!(fifth_payload.command, "mind_t3_requeue");

        let sixth = rx.try_recv().expect("handshake rebuild command");
        let WireMsg::Command(sixth_payload) = sixth.msg else {
            panic!("expected command")
        };
        assert_eq!(sixth_payload.command, "mind_handshake_rebuild");
    }

    #[test]
    fn mind_lane_toggle_cycles_t0_t1_t2_t3_and_all() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
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
                            "project_root": "/repo",
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {
                                    "status":"queued",
                                    "trigger":"token_threshold",
                                    "progress": {
                                        "t0_estimated_tokens": 1200,
                                        "t1_target_tokens": 28000,
                                        "t1_hard_cap_tokens": 32000,
                                        "tokens_until_next_run": 26800
                                    }
                                },
                                {"status":"success","trigger":"token_threshold","runtime":"pi-semantic"},
                                {"status":"success","trigger":"task_completed","runtime":"t2_reflector","reason":"t2 reflector processed 1 job(s)"},
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

        assert_eq!(app.mind_lane, MindLaneFilter::T1);
        assert_eq!(app.mind_rows().len(), 1);

        app.toggle_mind_lane();
        assert_eq!(app.mind_lane, MindLaneFilter::T2);
        assert_eq!(app.mind_rows().len(), 1);

        app.toggle_mind_lane();
        assert_eq!(app.mind_lane, MindLaneFilter::T3);
        assert_eq!(app.mind_rows().len(), 1);

        app.toggle_mind_lane();
        assert_eq!(app.mind_lane, MindLaneFilter::All);
        assert_eq!(app.mind_rows().len(), 4);

        app.toggle_mind_lane();
        assert_eq!(app.mind_lane, MindLaneFilter::T0);
        assert_eq!(app.mind_rows().len(), 1);
    }

    #[test]
    fn render_mind_lines_shows_t3_runtime_rollup() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;
        app.mind_lane = MindLaneFilter::All;

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
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
                            "project_root": "/repo",
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "insight_runtime": {
                            "queue_depth": 2,
                            "reflector_jobs_completed": 1,
                            "reflector_jobs_failed": 0,
                            "reflector_lock_conflicts": 0,
                            "t3_queue_depth": 7,
                            "t3_jobs_completed": 4,
                            "t3_jobs_failed": 1,
                            "t3_jobs_requeued": 2,
                            "t3_jobs_dead_lettered": 1,
                            "t3_lock_conflicts": 3
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("t3q:7 done:4 fail:1 rq:2 dlq:1 lock:3"));
        assert!(rendered.contains("[t3]"));
    }

    #[test]
    fn render_mind_lines_shows_detached_subagent_rollup() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;
        app.mind_lane = MindLaneFilter::All;

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
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
                            "project_root": "/repo",
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "insight_detached": {
                            "status": "ok",
                            "active_jobs": 1,
                            "fallback_used": false,
                            "jobs": [
                                {
                                    "job_id": "detached-123",
                                    "mode": "dispatch",
                                    "status": "running",
                                    "agent": "reviewer-contracts",
                                    "created_at_ms": 1000,
                                    "started_at_ms": 1500,
                                    "step_count": 1,
                                    "output_excerpt": "reviewing canonical store-first cutover"
                                }
                            ]
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("subagents:"));
        assert!(rendered.contains("reviewer-contracts"));
        assert!(rendered.contains("run:1"));
    }

    #[test]
    fn render_mind_lines_project_scoped_filters_other_projects() {
        let (tx, _rx) = mpsc::channel(4);
        let mut config = test_config();
        config.project_root = PathBuf::from("/repo-a");
        config.mind_project_scoped = true;
        let mut app = App::new(config, tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;
        app.mind_lane = MindLaneFilter::All;

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![
                    AgentState {
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
                                "agent_label": "Repo A",
                                "project_root": "/repo-a",
                                "tab_scope": "agent"
                            },
                            "mind_observer": {
                                "events": [
                                    {"status":"success","trigger":"manual_shortcut","runtime":"observer","reason":"repo a event"}
                                ]
                            }
                        })),
                    },
                    AgentState {
                        agent_id: "session-test::13".to_string(),
                        session_id: "session-test".to_string(),
                        pane_id: "13".to_string(),
                        lifecycle: "running".to_string(),
                        snippet: None,
                        last_heartbeat_ms: Some(1),
                        last_activity_ms: Some(1),
                        updated_at_ms: Some(1),
                        source: Some(serde_json::json!({
                            "agent_status": {
                                "agent_label": "Repo B",
                                "project_root": "/repo-b",
                                "tab_scope": "agent"
                            },
                            "mind_observer": {
                                "events": [
                                    {"status":"success","trigger":"manual_shortcut","runtime":"observer","reason":"repo b event"}
                                ]
                            }
                        })),
                    }
                ],
            },
            event_at: Utc::now(),
        });

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("project: /repo-a [project-scoped]"));
        assert!(rendered.contains("repo a event"));
        assert!(!rendered.contains("repo b event"));
    }

    #[test]
    fn render_mind_lines_search_query_returns_local_hits() {
        let root = std::env::temp_dir().join(format!(
            "aoc-mission-control-mind-search-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let store_path = mind_store_path(&root);
        std::fs::create_dir_all(store_path.parent().expect("store parent"))
            .expect("create mind dir");
        let store = aoc_storage::MindStore::open(&store_path).expect("open store");
        let now = Utc::now();
        store
            .upsert_handshake_snapshot(
                "project",
                &project_scope_key(&root),
                "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:planner-drift r1] topic=planner confidence=8800 freshness=95 :: Planner drift contract and routing notes\n",
                "hash:handshake",
                128,
                now,
            )
            .expect("upsert handshake");
        store
            .upsert_canon_entry_revision(
                "canon:planner-drift",
                Some("planner"),
                "Planner drift contract and routing notes",
                8800,
                95,
                None,
                &["obs:planner".to_string()],
                now,
            )
            .expect("upsert canon");

        let (tx, _rx) = mpsc::channel(4);
        let mut config = test_config();
        config.project_root = root.clone();
        config.mind_project_scoped = true;
        let mut app = App::new(config, tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;
        app.mind_lane = MindLaneFilter::All;
        app.mind_search_query = "planner drift".to_string();

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Retrieval / search"));
        assert!(rendered.contains("query: > planner drift"));
        assert!(rendered.contains("[canon] canon:planner-drift r1"));
        assert!(rendered.contains("Planner drift contract and routing notes"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn render_mind_lines_without_observer_events_still_shows_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "aoc-mission-control-mind-artifacts-only-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let store_path = mind_store_path(&root);
        std::fs::create_dir_all(store_path.parent().expect("store parent"))
            .expect("create mind dir");
        let store = aoc_storage::MindStore::open(&store_path).expect("open store");
        let now = Utc::now();
        store
            .upsert_handshake_snapshot(
                "project",
                &project_scope_key(&root),
                "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:entry-a r1] topic=mind confidence=8800 freshness=95 :: Keep this in startup context\n",
                "hash:handshake",
                128,
                now,
            )
            .expect("upsert handshake");

        let (tx, _rx) = mpsc::channel(4);
        let mut config = test_config();
        config.project_root = root.clone();
        config.mind_project_scoped = true;
        let mut app = App::new(config, tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;
        app.mind_lane = MindLaneFilter::All;

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Observer activity [0 events]"));
        assert!(rendered.contains(
            "overview: handshake:1 canon:0 stale:0 latest:none recovery:none detached:0"
        ));
        assert!(rendered.contains("Retrieval / search"));
        assert!(rendered.contains("No observer activity yet for current lane/scope."));
        assert!(rendered.contains("Knowledge artifacts Artifact drilldown"));
        assert!(rendered.contains("Handshake + canon"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn render_mind_lines_shows_injection_rollup_and_store_backed_drilldown() {
        let root = std::env::temp_dir().join(format!(
            "aoc-mission-control-mind-v2-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let store_path = mind_store_path(&root);
        std::fs::create_dir_all(store_path.parent().expect("store parent"))
            .expect("create mind dir");
        let store = aoc_storage::MindStore::open(&store_path).expect("open store");
        let now = Utc::now();
        store
            .upsert_handshake_snapshot(
                "project",
                &project_scope_key(&root),
                "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:entry-a r1] topic=mind confidence=8800 freshness=95 :: Keep this in startup context\n",
                "hash:handshake",
                128,
                now,
            )
            .expect("upsert handshake");
        store
            .upsert_canon_entry_revision(
                "canon:entry-a",
                Some("mind"),
                "Consolidated summary",
                8800,
                95,
                None,
                &["obs:1".to_string(), "ref:2".to_string()],
                now,
            )
            .expect("upsert canon");

        let (tx, _rx) = mpsc::channel(4);
        let mut config = test_config();
        config.project_root = root.clone();
        let mut app = App::new(config, tx, empty_local());
        app.connected = true;
        app.mode = Mode::Mind;
        app.mind_lane = MindLaneFilter::All;
        app.mind_show_provenance = true;

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
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
                            "project_root": root.to_string_lossy(),
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "mind_injection": {
                            "status": "pending",
                            "trigger": "resume",
                            "scope": "project",
                            "scope_key": project_scope_key(&root),
                            "active_tag": "mind",
                            "reason": "resume handshake refresh",
                            "snapshot_id": "hs:abc123",
                            "payload_hash": "hash:abc123",
                            "token_estimate": 128,
                            "queued_at": "2026-03-01T12:10:00Z"
                        },
                        "insight_runtime": {
                            "queue_depth": 1,
                            "t3_queue_depth": 0
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("overview: handshake:1 canon:1 stale:0"));
        assert!(rendered.contains("inject: [resume] [pending]"));
        assert!(rendered.contains("resume handshake refresh"));
        assert!(rendered.contains("handshake:1 active_canon:1 stale_canon:0"));
        assert!(rendered.contains("trace: handshake -> canon -> evidence[2] obs:1, ref:2"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn render_mind_lines_includes_artifact_provenance_drilldown() {
        let root = std::env::temp_dir().join(format!(
            "aoc-mission-control-drilldown-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let t3_dir = root.join(".aoc").join("mind").join("t3");
        let insight_dir = root
            .join(".aoc")
            .join("mind")
            .join("insight")
            .join("session-test_20260301T120000Z_abc123def456");
        std::fs::create_dir_all(&t3_dir).expect("create t3 dir");
        std::fs::create_dir_all(&insight_dir).expect("create insight dir");

        std::fs::write(
            t3_dir.join("handshake.md"),
            "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:entry-a r3] topic=mind confidence=8800 freshness=95 :: Keep this in startup context\n",
        )
        .expect("write handshake");

        std::fs::write(
            t3_dir.join("project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:entry-a r3\n- topic: mind\n- evidence_refs: obs:1, ref:2\n\nConsolidated summary\n\n## Stale canon\n\n### canon:entry-old r1\n- topic: mind\n\nOld summary\n",
        )
        .expect("write project mind");

        std::fs::write(
            insight_dir.join("manifest.json"),
            r#"{
  "session_id": "session-test",
  "active_tag": "mind",
  "export_dir": "/tmp/session-test_20260301T120000Z_abc123def456",
  "t1_count": 2,
  "t2_count": 1,
  "t1_artifact_ids": ["obs:1", "obs:2"],
  "t2_artifact_ids": ["ref:2"],
  "slice_start_id": "obs:1",
  "slice_end_id": "ref:2",
  "t3_job_id": "t3:job:42",
  "exported_at": "2026-03-01T12:00:00Z"
}"#,
        )
        .expect("write manifest");

        let store_path = mind_store_path(&root);
        std::fs::create_dir_all(store_path.parent().expect("store parent"))
            .expect("create mind dir");
        let store = aoc_storage::MindStore::open(&store_path).expect("open store");
        let marker_event = aoc_core::mind_contracts::RawEvent {
            event_id: "evt-compaction-session-test-1".to_string(),
            conversation_id: "conv-compact".to_string(),
            agent_id: "agent-1".to_string(),
            ts: Utc
                .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
                .single()
                .expect("ts"),
            body: aoc_core::mind_contracts::RawEventBody::Other {
                payload: serde_json::json!({"type": "compaction"}),
            },
            attrs: std::collections::BTreeMap::from([
                (
                    "mind_compaction_modified_files".to_string(),
                    serde_json::json!(["src/main.rs", "README.md"]),
                ),
                (
                    "pi_detail_read_files".to_string(),
                    serde_json::json!(["src/lib.rs"]),
                ),
            ]),
        };
        store
            .insert_raw_event(&marker_event)
            .expect("insert marker");
        let checkpoint = aoc_storage::CompactionCheckpoint {
            checkpoint_id: "cmpchk:conv-compact:compact-1".to_string(),
            conversation_id: "conv-compact".to_string(),
            session_id: "session-test".to_string(),
            ts: marker_event.ts,
            trigger_source: "pi_compaction_checkpoint".to_string(),
            reason: Some("pi compaction".to_string()),
            summary: Some("Compacted prior work into durable summary".to_string()),
            tokens_before: Some(4096),
            first_kept_entry_id: Some("entry-42".to_string()),
            compaction_entry_id: Some("compact-1".to_string()),
            from_extension: true,
            marker_event_id: Some(marker_event.event_id.clone()),
            schema_version: 1,
            created_at: marker_event.ts,
            updated_at: marker_event.ts,
        };
        store
            .upsert_compaction_checkpoint(&checkpoint)
            .expect("upsert checkpoint");
        let slice = aoc_core::mind_contracts::build_compaction_t0_slice(
            &checkpoint.conversation_id,
            &checkpoint.session_id,
            checkpoint.ts,
            &checkpoint.trigger_source,
            checkpoint.reason.as_deref(),
            checkpoint.summary.as_deref(),
            checkpoint.tokens_before,
            checkpoint.first_kept_entry_id.as_deref(),
            checkpoint.compaction_entry_id.as_deref(),
            checkpoint.from_extension,
            "pi_compaction_checkpoint",
            &[marker_event.event_id.clone()],
            &["src/lib.rs".to_string()],
            &["src/main.rs".to_string(), "README.md".to_string()],
            Some(&checkpoint.checkpoint_id),
            "t0.compaction.v1",
        )
        .expect("build slice");
        store
            .upsert_compaction_t0_slice(&slice)
            .expect("upsert slice");

        let (tx, _rx) = mpsc::channel(4);
        let mut cfg = test_config();
        cfg.project_root = root.clone();
        let mut app = App::new(cfg, tx, empty_local());
        app.mode = Mode::Mind;
        app.connected = true;
        app.mind_lane = MindLaneFilter::All;
        app.mind_show_provenance = true;

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
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
                            "project_root": root.to_string_lossy().to_string(),
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"compaction","runtime":"deterministic","conversation_id":"conv-compact","reason":"pi compaction checkpoint","completed_at":"2026-03-01T12:00:02Z"},
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "insight_runtime": {
                            "queue_depth": 1,
                            "t3_queue_depth": 0
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

        let lines = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Artifact drilldown"));
        assert!(rendered.contains("[provenance:on]"));
        assert!(rendered.contains("health: t0:stored replay:ready t1:ok t2q:1 t3q:0"));
        assert!(rendered.contains("evidence: src:1 read:1 modified:2 policy:t0.compaction.v1"));
        assert!(rendered
            .contains("recovery: press 'C' to rebuild/requeue latest compaction checkpoint"));
        assert!(rendered.contains("[canon:entry-a r3]"));
        assert!(rendered.contains("trace: handshake -> canon -> evidence[2] obs:1, ref:2"));

        let _ = std::fs::remove_dir_all(root);
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
    fn parse_pulse_theme_mode_accepts_known_values() {
        assert_eq!(
            parse_pulse_theme_mode("terminal"),
            Some(PulseThemeMode::Terminal)
        );
        assert_eq!(
            parse_pulse_theme_mode("AUTO"),
            Some(PulseThemeMode::Terminal)
        );
        assert_eq!(parse_pulse_theme_mode("dark"), Some(PulseThemeMode::Dark));
        assert_eq!(parse_pulse_theme_mode("light"), Some(PulseThemeMode::Light));
        assert_eq!(parse_pulse_theme_mode("solarized"), None);
    }

    #[test]
    fn layout_state_event_updates_local_tab_overlay() {
        let (tx, _rx) = mpsc::channel(4);
        let mut cfg = test_config();
        cfg.runtime_mode = RuntimeMode::PulsePane;
        cfg.overview_enabled = false;
        let mut app = App::new(cfg, tx, empty_local());
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
                session_title: None,
                chat_title: None,
            }],
            viewer_tab_index: None,
            tab_roster: Vec::new(),
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
    fn hybrid_layout_source_uses_hub_layout_when_connected() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.config.layout_source = LayoutSource::Hybrid;
        app.connected = true;
        app.hub.layout = Some(HubLayout {
            layout_seq: 3,
            pane_tabs: HashMap::from([(
                "12".to_string(),
                TabMeta {
                    index: 2,
                    name: "Agent".to_string(),
                    focused: true,
                },
            )]),
            focused_tab_index: Some(2),
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
            session_title: None,
            chat_title: None,
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
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .any(|row| row.identity_key == "session-test::12"));
        assert!(rows
            .iter()
            .any(|row| row.identity_key == "session-test::99"));
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
            session_title: None,
            chat_title: None,
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
    fn disconnect_clears_pending_commands_and_reconnect_restores_hub_mode() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);
        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![hub_state("session-test::12", "12", "/repo")],
            },
            event_at: Utc::now(),
        });
        app.pending_commands.insert(
            "req-reconnect".to_string(),
            PendingCommand {
                command: "run_validation".to_string(),
                target: "pane-12".to_string(),
            },
        );

        app.apply_hub_event(HubEvent::Disconnected);
        assert!(app.pending_commands.is_empty());
        assert_eq!(app.hub_status_label(), "reconnecting");
        assert_eq!(app.mode_source(), "hub");

        app.apply_hub_event(HubEvent::Connected);
        assert_eq!(app.hub_status_label(), "online");
        assert_eq!(app.mode_source(), "hub");
        assert_eq!(app.status_note.as_deref(), Some("hub connected"));
    }

    #[test]
    fn reconnect_followed_by_snapshot_replaces_cached_hub_rows() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);
        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![hub_state("session-test::12", "12", "/repo")],
            },
            event_at: Utc::now(),
        });
        app.apply_hub_event(HubEvent::Disconnected);
        app.apply_hub_event(HubEvent::Connected);
        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 2,
                states: vec![hub_state("session-test::21", "21", "/repo")],
            },
            event_at: Utc::now(),
        });

        let rows = app.overview_rows();
        assert!(rows
            .iter()
            .any(|row| row.identity_key == "session-test::21"));
        assert!(!rows
            .iter()
            .any(|row| row.identity_key == "session-test::12"));
    }

    #[test]
    fn overview_hub_mode_includes_local_only_rows() {
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
            session_title: None,
            chat_title: None,
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
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .any(|row| row.identity_key == "session-test::12"));
        assert!(rows
            .iter()
            .any(|row| row.identity_key == "session-test::99"));
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
                session_title: None,
                chat_title: None,
            }],
            viewer_tab_index: Some(2),
            tab_roster: vec![TabMeta {
                index: 2,
                name: "tab-2".to_string(),
                focused: false,
            }],
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
                session_title: None,
                chat_title: None,
            }],
            viewer_tab_index: Some(2),
            tab_roster: vec![TabMeta {
                index: 2,
                name: "tab-2".to_string(),
                focused: false,
            }],
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
            session_title: None,
            chat_title: None,
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
            session_title: None,
            chat_title: None,
        };

        let decorations = OverviewDecorations {
            attention_chip: attention_chip_from_row(&row),
            context: "waiting on credentials and operator input".to_string(),
            task_signal: Some("W:1/4".to_string()),
            git_signal: Some("G:+7/-2 ?1".to_string()),
        };
        let presenter = overview_row_presenter(&row, &decorations, true, 80);
        assert!(presenter.identity.contains("::"));
        assert_eq!(presenter.location_chip, "T?:???");
        assert_eq!(presenter.lifecycle_chip, "[BLOCK]");
        assert_eq!(
            presenter.badge,
            OverviewBadge::Attention(AttentionChip::Blocked)
        );
        assert!(presenter.freshness.contains("47s"));
        assert!(presenter.context.starts_with("M:"));
        assert!(presenter_text_len(&presenter) <= 72);
    }

    #[test]
    fn overview_toggle_methods_update_state() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        assert_eq!(app.overview_sort_mode, OverviewSortMode::Layout);

        app.toggle_overview_sort_mode();
        assert_eq!(app.overview_sort_mode, OverviewSortMode::Attention);
    }

    #[test]
    fn parse_runtime_mode_accepts_primary_labels() {
        assert_eq!(
            parse_runtime_mode("pulse-pane"),
            Some(RuntimeMode::PulsePane)
        );
        assert_eq!(
            parse_runtime_mode("mission-control"),
            Some(RuntimeMode::MissionControl)
        );
        assert_eq!(parse_runtime_mode("mc"), Some(RuntimeMode::MissionControl));
        assert_eq!(parse_runtime_mode("unknown"), None);
    }

    #[test]
    fn parse_start_view_accepts_fleet_and_aliases() {
        assert_eq!(parse_start_view("fleet"), Some(Mode::Fleet));
        assert_eq!(parse_start_view("subagents"), Some(Mode::Fleet));
        assert_eq!(parse_start_view("overview"), Some(Mode::Overview));
        assert_eq!(parse_start_view("unknown"), None);
    }

    #[test]
    fn parse_fleet_plane_filter_accepts_delegated_aliases() {
        assert_eq!(
            parse_fleet_plane_filter("delegated"),
            Some(FleetPlaneFilter::Delegated)
        );
        assert_eq!(
            parse_fleet_plane_filter("subagents"),
            Some(FleetPlaneFilter::Delegated)
        );
        assert_eq!(
            parse_fleet_plane_filter("mind"),
            Some(FleetPlaneFilter::Mind)
        );
        assert_eq!(parse_fleet_plane_filter("unknown"), None);
    }

    #[test]
    fn app_new_honors_start_view_and_fleet_plane_filter() {
        let (tx, _rx) = mpsc::channel(4);
        let mut cfg = test_config();
        cfg.start_view = Some(Mode::Fleet);
        cfg.fleet_plane_filter = FleetPlaneFilter::Delegated;
        let app = App::new(cfg, tx, empty_local());
        assert_eq!(app.mode, Mode::Fleet);
        assert_eq!(app.fleet_plane_filter, FleetPlaneFilter::Delegated);
    }

    #[test]
    fn pulse_subscribe_includes_overseer_topics() {
        let subscribe = build_pulse_subscribe(&test_config());
        let WireMsg::Subscribe(payload) = subscribe.msg else {
            panic!("expected subscribe envelope")
        };
        assert!(payload
            .topics
            .iter()
            .any(|topic| topic == "observer_snapshot"));
        assert!(payload
            .topics
            .iter()
            .any(|topic| topic == "observer_timeline"));
        assert!(payload
            .topics
            .iter()
            .any(|topic| topic == "consultation_response"));
        assert!(payload.topics.iter().any(|topic| topic == "layout_state"));
    }

    #[test]
    fn pulse_subscribe_omits_overseer_topics_for_light_pane() {
        let mut cfg = test_config();
        cfg.runtime_mode = RuntimeMode::PulsePane;
        let subscribe = build_pulse_subscribe(&cfg);
        let WireMsg::Subscribe(payload) = subscribe.msg else {
            panic!("expected subscribe envelope")
        };
        assert!(!payload
            .topics
            .iter()
            .any(|topic| topic == "observer_snapshot"));
        assert!(!payload
            .topics
            .iter()
            .any(|topic| topic == "observer_timeline"));
        assert!(!payload
            .topics
            .iter()
            .any(|topic| topic == "consultation_response"));
        assert!(payload.topics.iter().any(|topic| topic == "agent_state"));
        assert!(payload.topics.iter().any(|topic| topic == "command_result"));
        assert!(payload.topics.iter().any(|topic| topic == "layout_state"));
    }

    #[test]
    fn overseer_mode_renders_worker_and_timeline_data() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);
        app.mode = Mode::Overseer;

        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    pane_id: "12".to_string(),
                    role: Some("worker".to_string()),
                    status: WorkerStatus::Blocked,
                    assignment: aoc_core::session_overseer::WorkerAssignment {
                        task_id: Some("149.5".to_string()),
                        tag: Some("session-overseer".to_string()),
                        epic_id: Some("149".to_string()),
                    },
                    summary: Some("waiting for Mission Control render wiring".to_string()),
                    plan_alignment: PlanAlignment::Medium,
                    drift_risk: DriftRisk::High,
                    attention: aoc_core::session_overseer::AttentionSignal {
                        level: AttentionLevel::Warn,
                        kind: Some("blocked".to_string()),
                        reason: Some("awaiting operator confirmation".to_string()),
                    },
                    ..Default::default()
                }],
                timeline: vec![],
                degraded_reason: None,
            },
        });
        app.apply_hub_event(HubEvent::ObserverTimeline {
            payload: ObserverTimelinePayload {
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_123),
                entries: vec![ObserverTimelineEntry {
                    event_id: "evt-1".to_string(),
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    kind: aoc_core::session_overseer::ObserverEventKind::Blocked,
                    summary: Some("worker reported blocker".to_string()),
                    emitted_at_ms: Some(1_700_000_000_123),
                    ..Default::default()
                }],
            },
        });

        assert_eq!(app.mode_source(), "hub");
        let lines = render_overseer_lines(&app, pulse_theme(PulseThemeMode::Terminal), false);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("149.5"));
        assert!(rendered.contains("awaiting operator confirmation"));
        assert!(rendered.contains("worker reported blocker"));
    }

    #[test]
    fn render_mind_lines_shows_partial_compaction_health_when_recovery_is_degraded() {
        let root = std::env::temp_dir().join(format!(
            "aoc-mission-control-compaction-degraded-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let store_path = mind_store_path(&root);
        std::fs::create_dir_all(store_path.parent().expect("store parent"))
            .expect("create mind dir");
        let store = aoc_storage::MindStore::open(&store_path).expect("open store");
        let marker_event = aoc_core::mind_contracts::RawEvent {
            event_id: "evt-compaction-degraded-1".to_string(),
            conversation_id: "conv-degraded".to_string(),
            agent_id: "agent-1".to_string(),
            ts: Utc
                .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
                .single()
                .expect("ts"),
            body: aoc_core::mind_contracts::RawEventBody::Other {
                payload: serde_json::json!({"type": "compaction"}),
            },
            attrs: std::collections::BTreeMap::new(),
        };
        store
            .insert_raw_event(&marker_event)
            .expect("insert marker");
        store
            .upsert_compaction_checkpoint(&aoc_storage::CompactionCheckpoint {
                checkpoint_id: "cmpchk:conv-degraded:compact-1".to_string(),
                conversation_id: "conv-degraded".to_string(),
                session_id: "session-test".to_string(),
                ts: marker_event.ts,
                trigger_source: "pi_compaction_checkpoint".to_string(),
                reason: Some("pi compaction".to_string()),
                summary: Some("checkpoint exists but replay provenance is degraded".to_string()),
                tokens_before: Some(1024),
                first_kept_entry_id: Some("entry-7".to_string()),
                compaction_entry_id: Some("compact-1".to_string()),
                from_extension: true,
                marker_event_id: Some(marker_event.event_id.clone()),
                schema_version: 1,
                created_at: marker_event.ts,
                updated_at: marker_event.ts,
            })
            .expect("upsert checkpoint");

        let (tx, _rx) = mpsc::channel(4);
        let mut cfg = test_config();
        cfg.project_root = root.clone();
        let mut app = App::new(cfg, tx, empty_local());
        app.mode = Mode::Mind;
        app.connected = true;
        app.mind_lane = MindLaneFilter::All;

        app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
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
                            "project_root": root.to_string_lossy().to_string(),
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"error","trigger":"compaction","runtime":"deterministic","conversation_id":"conv-degraded","reason":"semantic stage failed","completed_at":"2026-03-01T12:00:02Z"}
                            ]
                        },
                        "insight_runtime": {
                            "queue_depth": 2,
                            "t3_queue_depth": 1
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

        let rendered = render_mind_lines(&app, pulse_theme(PulseThemeMode::Terminal), false)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("health: t0:missing replay:partial t1:error t2q:2 t3q:1"));
        assert!(rendered
            .contains("recovery: press 'C' to rebuild/requeue latest compaction checkpoint"));

        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn overseer_mode_adds_optional_mind_enrichment_without_blocking_base_render() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);
        app.mode = Mode::Overseer;

        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    pane_id: "12".to_string(),
                    role: Some("worker".to_string()),
                    status: WorkerStatus::Active,
                    summary: Some("shipping deterministic overseer baseline".to_string()),
                    provenance: Some("heuristic:wrapper+taskmaster".to_string()),
                    ..Default::default()
                }],
                timeline: vec![],
                degraded_reason: None,
            },
        });

        let baseline = render_overseer_lines(&app, pulse_theme(PulseThemeMode::Terminal), false)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(baseline.contains("shipping deterministic overseer baseline"));
        assert!(baseline.contains("[prov:heuristic:wrapper+taskmaster]"));
        assert!(baseline.contains("mc "));
        assert!(baseline.contains("assign task/tag before further implementation"));

        app.hub.mind.insert(
            "session-test::12".to_string(),
            MindObserverFeedPayload {
                updated_at_ms: Some(1_700_000_000_111),
                events: vec![MindObserverFeedEvent {
                    status: MindObserverFeedStatus::Fallback,
                    trigger: MindObserverFeedTriggerKind::TaskCompleted,
                    conversation_id: None,
                    runtime: Some("pi-semantic".to_string()),
                    attempt_count: Some(1),
                    latency_ms: Some(96),
                    reason: Some(
                        "semantic observer timed out; using bounded heuristic summary".to_string(),
                    ),
                    failure_kind: Some("timeout".to_string()),
                    enqueued_at: None,
                    started_at: None,
                    completed_at: Some("2026-03-09T10:45:00Z".to_string()),
                    progress: None,
                }],
            },
        );

        let enriched = render_overseer_lines(&app, pulse_theme(PulseThemeMode::Terminal), false)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(enriched.contains("[prov:heuristic:wrapper+taskmaster+mind:t1:fallback]"));
        assert!(enriched.contains("semantic [t1:fallback]"));
        assert!(enriched.contains("semantic observer timed out"));
    }

    #[test]
    fn overseer_mode_renders_mission_control_prompt_for_blocked_worker() {
        let (tx, _rx) = mpsc::channel(4);
        let mut app = App::new(test_config(), tx, empty_local());
        app.apply_hub_event(HubEvent::Connected);
        app.mode = Mode::Overseer;

        app.apply_hub_event(HubEvent::ObserverSnapshot {
            payload: ObserverSnapshot {
                schema_version: 1,
                session_id: "session-test".to_string(),
                generated_at_ms: Some(1_700_000_000_000),
                workers: vec![WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::22".to_string(),
                    pane_id: "22".to_string(),
                    role: Some("reviewer".to_string()),
                    status: WorkerStatus::Blocked,
                    summary: Some("waiting on design clarification".to_string()),
                    blocker: Some("need operator decision on packet shape".to_string()),
                    ..Default::default()
                }],
                timeline: vec![],
                degraded_reason: None,
            },
        });

        let rendered = render_overseer_lines(&app, pulse_theme(PulseThemeMode::Terminal), false)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("mc "));
        assert!(rendered.contains("ask for unblock plan + evidence-backed next step"));
        assert!(rendered.contains("src:partial"));
    }
}
