mod insight_orchestrator;

use aoc_core::{
    consultation_contracts::{
        ConsultationBlocker, ConsultationConfidence, ConsultationEvidenceRef,
        ConsultationFreshness, ConsultationIdentity, ConsultationPacket, ConsultationPacketKind,
        ConsultationSourceStatus, ConsultationTaskContext,
    },
    insight_contracts::{
        InsightBootstrapRequest, InsightCommand, InsightDetachedStatusResult,
        InsightDispatchRequest, InsightRetrievalCitation, InsightRetrievalDrilldownRef,
        InsightRetrievalHit, InsightRetrievalMode, InsightRetrievalRequest, InsightRetrievalResult,
        InsightRetrievalScope, InsightStatusResult,
    },
    mind_contracts::{
        build_compaction_t0_slice, canonical_lineage_attrs, canonical_payload_hash,
        compact_raw_event_to_t0, compose_context_pack, sanitize_raw_event_for_storage,
        text_contains_unredacted_secret, ContextLayer, ContextPackInput,
        ConversationLineageMetadata, ConversationRole, MessageEvent, RawEvent, RawEventBody,
        SemanticProvenance, SemanticRuntime, SemanticRuntimeMode, SemanticStage,
        T0CompactionPolicy, ToolExecutionStatus, ToolResultEvent,
    },
    mind_observer_feed::{
        MindInjectionPayload, MindInjectionTriggerKind, MindObserverFeedEvent,
        MindObserverFeedPayload, MindObserverFeedProgress, MindObserverFeedStatus,
        MindObserverFeedTriggerKind,
    },
    provenance_contracts::{
        MindProvenanceCommand, MindProvenanceEdge, MindProvenanceEdgeKind, MindProvenanceExport,
        MindProvenanceNode, MindProvenanceNodeKind, MindProvenanceQueryRequest,
        MindProvenanceQueryResult,
    },
    pulse_ipc::{
        decode_frame, encode_frame, AgentState as PulseAgentState,
        CommandError as PulseCommandError, CommandResultPayload as PulseCommandResultPayload,
        ConsultationRequestPayload as PulseConsultationRequestPayload,
        ConsultationResponsePayload as PulseConsultationResponsePayload,
        ConsultationStatus as PulseConsultationStatus, DeltaPayload as PulseDeltaPayload,
        HeartbeatPayload as PulseHeartbeatPayload, HelloPayload as PulseHelloPayload,
        ProtocolVersion, StateChange as PulseStateChange, StateChangeOp,
        WireEnvelope as PulseWireEnvelope, WireMsg, CURRENT_PROTOCOL_VERSION,
        DEFAULT_MAX_FRAME_BYTES,
    },
    session_overseer::{
        AttentionSignal, DriftRisk, ObserverEvent, ObserverEventKind, OverseerSourceKind,
        PlanAlignment, ProgressPhase, ProgressPosition, WorkerAssignment, WorkerSnapshot,
        WorkerStatus, OVERSEER_SCHEMA_VERSION,
    },
    ProjectData, Task, TaskStatus,
};
use aoc_mind::{
    observer_feed_event_from_outcome, DetachedReflectorWorker, DetachedT3Worker,
    DistillationConfig, PiObserverAdapter, ReflectorRuntimeConfig, SemanticObserverConfig,
    SessionObserverSidecar, T3RuntimeConfig,
};
use aoc_storage::{
    ArtifactFileLink, CanonRevisionState, CompactionCheckpoint, MindStore, ProjectWatermark,
    ReflectorJob, StoredArtifact, T3BacklogJob,
};
use chrono::{TimeZone, Utc};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use insight_orchestrator::{DetachedInsightRuntime, InsightSupervisor};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, HashSet},
    env,
    fs::OpenOptions,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex as StdMutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs,
    io::AsyncReadExt,
    process::Command,
    sync::{mpsc, oneshot, Mutex},
    time::Instant,
};
#[cfg(unix)]
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixStream},
};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt::writer::BoxMakeWriter, EnvFilter};
use url::Url;

const PROTOCOL_VERSION: &str = "1";
const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_FILES_LIST: usize = 500;
const TASK_DEBOUNCE_MS: u64 = 500;
const DIFF_INTERVAL_SECS: u64 = 2;
const DIFF_SHARED_CACHE_TTL_MS: u64 = 1500;
const STATUS_MESSAGE_MAX_CHARS: usize = 140;
const TAP_RING_MAX_BYTES: usize = 64 * 1024;
const TAP_REPORT_INTERVAL_MS: u64 = 250;
const TAP_RESEND_INTERVAL_MS: u64 = 3000;
const STOP_SIGINT_GRACE_MS: u64 = 1500;
const STOP_TERM_GRACE_MS: u64 = 800;
const HEALTH_INTERVAL_SECS: u64 = 5;
const TASK_CONTEXT_HEARTBEAT_SECS: u64 = 20;
const TASK_CONTEXT_COMMAND_DEBOUNCE_MS: u64 = 1500;
const MAX_MIND_OBSERVER_EVENTS: usize = 40;
const MAX_OVERSEER_EVENTS: usize = 24;
const MAX_CONSULTATION_EVENTS: usize = 12;
const MIND_DEBOUNCE_RUN_MS: i64 = 300;
const MIND_FINALIZE_DRAIN_TIMEOUT_MS: i64 = 5_000;
const MIND_IDLE_FINALIZE_MS: i64 = 120_000;
const MIND_IDLE_CHECK_INTERVAL_MS: i64 = 5_000;
const MIND_T3_TICK_INTERVAL_SECS: u64 = 5;
const MIND_T3_MAX_ATTEMPTS: u16 = 3;
const MIND_T3_CANON_SUMMARY_MAX_CHARS: usize = 280;
const MIND_T3_CANON_STALE_AFTER_DAYS: i64 = 14;
const MIND_T3_HANDSHAKE_TOKEN_BUDGET: u32 = 500;
const MIND_T3_HANDSHAKE_MAX_ITEMS: usize = 12;
const MIND_INJECTION_COOLDOWN_MS: i64 = 20_000;
const MIND_INJECTION_PRESSURE_SUPPRESS_PCT: u8 = 70;
const MIND_CONTEXT_PACK_SCHEMA_VERSION: u16 = 1;
const MIND_CONTEXT_PACK_COMPACT_MAX_LINES: usize = 24;
const MIND_CONTEXT_PACK_EXPANDED_MAX_LINES: usize = 48;
const MIND_CONTEXT_PACK_COMPACT_SOURCE_MAX_LINES: usize = 5;
const MIND_CONTEXT_PACK_EXPANDED_SOURCE_MAX_LINES: usize = 10;
const MIND_CHILD_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TERM",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "COLORTERM",
    "TMPDIR",
    "TMP",
    "TEMP",
    "NO_COLOR",
    "CI",
    "RUST_LOG",
    "RUST_BACKTRACE",
    "RUST_LIB_BACKTRACE",
];
const REDACTED_SECRET: &str = "[REDACTED]";

fn mind_child_env<I, K, V>(extra_env: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut pairs = Vec::new();
    for key in MIND_CHILD_ENV_ALLOWLIST {
        if let Ok(value) = env::var(key) {
            pairs.push(((*key).to_string(), value));
        }
    }
    for (key, value) in extra_env {
        pairs.push((key.as_ref().to_string(), value.as_ref().to_string()));
    }
    pairs
}

fn configure_mind_child_command_env<I, K, V>(command: &mut Command, extra_env: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    command.env_clear();
    for (key, value) in mind_child_env(extra_env) {
        command.env(key, value);
    }
}

fn configure_mind_child_std_command_env<I, K, V>(command: &mut std::process::Command, extra_env: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    command.env_clear();
    for (key, value) in mind_child_env(extra_env) {
        command.env(key, value);
    }
}

fn configure_mind_child_pty_env<I, K, V>(builder: &mut CommandBuilder, extra_env: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    builder.env_clear();
    for (key, value) in mind_child_env(extra_env) {
        builder.env(key, value);
    }
}
const TELEMETRY_SECRET_KEYS: [&str; 13] = [
    "access_token",
    "api_key",
    "apikey",
    "auth_token",
    "client_secret",
    "id_token",
    "password",
    "passwd",
    "private_key",
    "refresh_token",
    "secret",
    "session_token",
    "token",
];
const DISABLE_MOUSE_SEQ: &str =
    "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1007l\x1b[?1004l";

#[derive(Parser, Debug)]
#[command(name = "aoc-agent-wrap-rs")]
struct Args {
    #[arg(long, default_value = "")]
    session: String,
    #[arg(long, default_value = "")]
    pane_id: String,
    #[arg(long, default_value = "")]
    agent_id: String,
    #[arg(long, default_value = "")]
    project_root: String,
    #[arg(long, default_value = "")]
    tab_scope: String,
    #[arg(long, default_value = "")]
    hub_addr: String,
    #[arg(long, default_value = "")]
    hub_url: String,
    #[arg(long, default_value = "")]
    pulse_socket_path: String,
    #[arg(long, default_value = "")]
    log_dir: String,
    #[arg(long, default_value_t = 10)]
    heartbeat_interval: u64,
    #[arg(last = true)]
    cmd: Vec<String>,
}

#[derive(Clone, Debug)]
struct ClientConfig {
    session_id: String,
    agent_key: String,
    agent_label: String,
    pane_id: String,
    project_root: String,
    tab_scope: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    version: String,
    #[serde(rename = "type")]
    r#type: String,
    session_id: String,
    sender_id: String,
    timestamp: String,
    payload: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct HelloPayload {
    client_id: String,
    role: String,
    capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pane_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_root: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AgentStatusPayload {
    agent_id: String,
    status: String,
    pane_id: String,
    project_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct HeartbeatPayload {
    agent_id: String,
    pid: i32,
    cwd: String,
    last_update: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pane_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_scope: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct DiffPatchRequestPayload {
    agent_id: String,
    path: String,
    #[serde(default)]
    context_lines: Option<i32>,
    #[serde(default)]
    include_untracked: Option<bool>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PayloadError {
    code: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct DiffPatchResponsePayload {
    agent_id: String,
    path: String,
    status: String,
    is_binary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<PayloadError>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct DiffCounts {
    files: u32,
    additions: u32,
    deletions: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct UntrackedCounts {
    files: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct DiffSummaryCounts {
    staged: DiffCounts,
    unstaged: DiffCounts,
    untracked: UntrackedCounts,
}

#[derive(Serialize, Deserialize, Clone)]
struct DiffFile {
    path: String,
    status: String,
    additions: u32,
    deletions: u32,
    staged: bool,
    untracked: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct DiffSummaryPayload {
    agent_id: String,
    repo_root: String,
    git_available: bool,
    summary: DiffSummaryCounts,
    files: Vec<DiffFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SharedDiffSummaryCacheEntry {
    saved_at_ms: u64,
    payload: DiffSummaryPayload,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct TaskCounts {
    total: u32,
    pending: u32,
    in_progress: u32,
    done: u32,
    blocked: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct ActiveTask {
    id: String,
    title: String,
    status: String,
    priority: String,
    active_agent: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct TaskSummaryPayload {
    agent_id: String,
    tag: String,
    counts: TaskCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_tasks: Option<Vec<ActiveTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<PayloadError>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct CurrentTagPayload {
    tag: String,
    task_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    prd_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct CheckOutcome {
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    details: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct DependencyStatus {
    name: String,
    #[serde(default)]
    available: bool,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct HealthSnapshotPayload {
    #[serde(default)]
    dependencies: Vec<DependencyStatus>,
    #[serde(default)]
    checks: Vec<CheckOutcome>,
    #[serde(default)]
    taskmaster_status: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct InsightRuntimeHealthPayload {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_tick_ms: Option<i64>,
    #[serde(default)]
    queue_depth: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ConsultationInboxEntry {
    consultation_id: String,
    requesting_agent_id: String,
    summary: Option<String>,
    kind: ConsultationPacketKind,
    received_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ConsultationOutboxEntry {
    consultation_id: String,
    requesting_agent_id: String,
    responding_agent_id: String,
    status: PulseConsultationStatus,
    summary: Option<String>,
    responded_at: String,
}

#[derive(Clone)]
enum PulseUpdate {
    Status {
        lifecycle: String,
        snippet: Option<String>,
        parser_confidence: Option<u8>,
    },
    TaskSummaries(HashMap<String, TaskSummaryPayload>),
    CurrentTag(CurrentTagPayload),
    DiffSummary(DiffSummaryPayload),
    Health(HealthSnapshotPayload),
    InsightRuntime(InsightRuntimeHealthPayload),
    InsightDetached(InsightDetachedStatusResult),
    MindObserverEvent(MindObserverFeedEvent),
    MindInjection(MindInjectionPayload),
    ConsultationInbox(ConsultationInboxEntry),
    ConsultationOutbox(ConsultationOutboxEntry),
    Heartbeat {
        lifecycle: Option<String>,
    },
    Remove,
    Shutdown,
}

struct PulseState {
    lifecycle: String,
    snippet: Option<String>,
    parser_confidence: Option<u8>,
    task_summaries: HashMap<String, TaskSummaryPayload>,
    current_tag: Option<CurrentTagPayload>,
    diff_summary: Option<DiffSummaryPayload>,
    health: Option<HealthSnapshotPayload>,
    insight_runtime: Option<InsightRuntimeHealthPayload>,
    insight_detached: Option<InsightDetachedStatusResult>,
    mind_observer: MindObserverFeedPayload,
    mind_injection: Option<MindInjectionPayload>,
    consultation_inbox: Vec<ConsultationInboxEntry>,
    consultation_outbox: Vec<ConsultationOutboxEntry>,
    observer_events: Vec<ObserverEvent>,
    last_heartbeat_ms: Option<i64>,
    last_activity_ms: Option<i64>,
    updated_at_ms: Option<i64>,
}

impl PulseState {
    fn new() -> Self {
        Self {
            lifecycle: "running".to_string(),
            snippet: None,
            parser_confidence: None,
            task_summaries: HashMap::new(),
            current_tag: None,
            diff_summary: None,
            health: None,
            insight_runtime: None,
            insight_detached: None,
            mind_observer: MindObserverFeedPayload::default(),
            mind_injection: None,
            consultation_inbox: Vec::new(),
            consultation_outbox: Vec::new(),
            observer_events: Vec::new(),
            last_heartbeat_ms: None,
            last_activity_ms: None,
            updated_at_ms: None,
        }
    }
}

#[derive(Clone)]
struct RuntimeConfig {
    client: ClientConfig,
    hub_url: Url,
    pulse_socket_path: PathBuf,
    pulse_vnext_enabled: bool,
    heartbeat_interval: Duration,
    cmd: Vec<String>,
    log_dir: String,
    log_stdout: bool,
}

#[derive(Default)]
struct CachedMessages {
    status: Option<String>,
    diff_summary: Option<String>,
    task_summary: HashMap<String, String>,
    task_done_counts: HashMap<String, u32>,
    current_tag: Option<CurrentTagPayload>,
}

struct LogGuard {
    file: Option<Arc<StdMutex<std::fs::File>>>,
}

struct MultiWriter {
    stdout_enabled: bool,
    file: Option<Arc<StdMutex<std::fs::File>>>,
}

enum GitError {
    Missing,
    NotRepo,
    Error(()),
}

enum TaskError {
    Missing,
    Malformed(String),
    Io(String),
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskmasterStateFile {
    #[serde(default)]
    current_tag: Option<String>,
}

struct GitStatusEntry {
    path: String,
    status: String,
    staged: bool,
    unstaged: bool,
    untracked: bool,
}

#[derive(Serialize)]
struct RuntimeSnapshot {
    session_id: String,
    pane_id: String,
    agent_id: String,
    agent_label: String,
    project_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_scope: Option<String>,
    pid: i32,
    status: String,
    last_update: String,
}

struct TapBuffer {
    max_bytes: usize,
    bytes: Vec<u8>,
    version: u64,
}

impl TapBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            bytes: Vec::with_capacity(max_bytes),
            version: 0,
        }
    }

    fn append(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        if chunk.len() >= self.max_bytes {
            let start = chunk.len().saturating_sub(self.max_bytes);
            self.bytes.clear();
            self.bytes.extend_from_slice(&chunk[start..]);
            self.version = self.version.wrapping_add(1);
            return;
        }
        let total = self.bytes.len() + chunk.len();
        if total > self.max_bytes {
            let trim = total - self.max_bytes;
            self.bytes.drain(0..trim);
        }
        self.bytes.extend_from_slice(chunk);
        self.version = self.version.wrapping_add(1);
    }

    fn snapshot(&self) -> (u64, Vec<u8>) {
        (self.version, self.bytes.clone())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentLifecycle {
    Running,
    Busy,
    NeedsInput,
    Error,
    Idle,
}

impl AgentLifecycle {
    fn as_status(self) -> &'static str {
        match self {
            AgentLifecycle::Running => "running",
            AgentLifecycle::Busy => "busy",
            AgentLifecycle::NeedsInput => "needs-input",
            AgentLifecycle::Error => "error",
            AgentLifecycle::Idle => "idle",
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let config = load_config(args);
    let _log_guard = init_logging(&config);

    if config.cmd.is_empty() {
        error!("missing command to wrap");
        std::process::exit(1);
    }

    let (tx, rx) = mpsc::channel::<String>(256);
    let (pulse_tx, pulse_rx) = mpsc::channel::<PulseUpdate>(256);
    let (task_context_refresh_tx, task_context_refresh_rx) = mpsc::unbounded_channel::<()>();
    let cache = Arc::new(Mutex::new(CachedMessages::default()));
    let tap_buffer = Arc::new(StdMutex::new(TapBuffer::new(TAP_RING_MAX_BYTES)));
    let hub_cfg = config.client.clone();
    let hub_url = config.hub_url.clone();
    let cache_clone = cache.clone();
    let mut hub_rx = rx;
    let hub_task = tokio::spawn(async move {
        hub_loop(hub_cfg, hub_url, &mut hub_rx, cache_clone).await;
    });
    let pulse_task = if config.pulse_vnext_enabled {
        let pulse_cfg = config.client.clone();
        let pulse_socket_path = config.pulse_socket_path.clone();
        Some(tokio::spawn(async move {
            pulse_loop(pulse_cfg, pulse_socket_path, pulse_rx).await;
        }))
    } else {
        drop(pulse_rx);
        None
    };

    let online = build_agent_status(&config.client, "running", None);
    {
        let mut cached = cache.lock().await;
        cached.status = Some(online.clone());
    }
    let _ = tx.send(online).await;
    publish_pulse_update(
        &pulse_tx,
        PulseUpdate::Status {
            lifecycle: "running".to_string(),
            snippet: None,
            parser_confidence: Some(1),
        },
    );
    let _ = persist_runtime_snapshot(&config.client, "running").await;

    let heartbeat_cfg = config.clone();
    let heartbeat_tx = tx.clone();
    let heartbeat_pulse_tx = pulse_tx.clone();
    let heartbeat_task = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(heartbeat_cfg.heartbeat_interval);
        loop {
            ticker.tick().await;
            let msg = build_heartbeat(&heartbeat_cfg.client);
            if heartbeat_tx.send(msg).await.is_err() {
                break;
            }
            publish_pulse_update(
                &heartbeat_pulse_tx,
                PulseUpdate::Heartbeat { lifecycle: None },
            );
            let _ = persist_runtime_snapshot(&heartbeat_cfg.client, "running").await;
        }
    });

    let task_cfg = config.client.clone();
    let task_tx = tx.clone();
    let task_cache = cache.clone();
    let task_pulse_tx = pulse_tx.clone();
    let task_context_rx = task_context_refresh_rx;
    let task_task = tokio::spawn(async move {
        task_summary_loop(
            task_cfg,
            task_tx,
            task_cache,
            task_pulse_tx,
            task_context_rx,
        )
        .await;
    });

    let diff_cfg = config.client.clone();
    let diff_tx = tx.clone();
    let diff_cache = cache.clone();
    let diff_pulse_tx = pulse_tx.clone();
    let diff_task = tokio::spawn(async move {
        diff_summary_loop(diff_cfg, diff_tx, diff_cache, diff_pulse_tx).await;
    });

    let health_task = if config.pulse_vnext_enabled {
        let health_cfg = config.client.clone();
        let health_pulse_tx = pulse_tx.clone();
        Some(tokio::spawn(async move {
            health_summary_loop(health_cfg, health_pulse_tx).await;
        }))
    } else {
        None
    };

    let reporter_cfg = config.client.clone();
    let reporter_tx = tx.clone();
    let reporter_cache = cache.clone();
    let reporter_tap = tap_buffer.clone();
    let reporter_pulse_tx = pulse_tx.clone();
    let reporter_task_context_tx = task_context_refresh_tx.clone();
    let reporter_task = tokio::spawn(async move {
        tap_state_reporter_loop(
            reporter_cfg,
            reporter_tx,
            reporter_cache,
            reporter_tap,
            reporter_pulse_tx,
            reporter_task_context_tx,
        )
        .await;
    });

    let use_pty = resolve_use_pty();
    let exit_code = if use_pty {
        match run_child_pty(&config.cmd, Some(tap_buffer.clone())).await {
            Ok(code) => code,
            Err(err) => {
                warn!("pty_spawn_failed: {err}; falling back to pipes");
                run_child_piped(&config.cmd).await
            }
        }
    } else {
        run_child_piped(&config.cmd).await
    };

    let offline = build_agent_status(&config.client, "offline", Some("exit"));
    {
        let mut cached = cache.lock().await;
        cached.status = Some(offline.clone());
    }
    let _ = tx.send(offline).await;
    publish_pulse_update(
        &pulse_tx,
        PulseUpdate::Status {
            lifecycle: "offline".to_string(),
            snippet: Some("exit".to_string()),
            parser_confidence: Some(3),
        },
    );
    publish_pulse_update(&pulse_tx, PulseUpdate::Remove);
    publish_pulse_update(&pulse_tx, PulseUpdate::Shutdown);
    let _ = persist_runtime_snapshot(&config.client, "offline").await;
    drop(tx);
    drop(pulse_tx);
    heartbeat_task.abort();
    task_task.abort();
    diff_task.abort();
    if let Some(task) = health_task {
        task.abort();
    }
    reporter_task.abort();
    hub_task.abort();
    if let Some(task) = pulse_task {
        let _ = tokio::time::timeout(Duration::from_millis(500), task).await;
    }
    std::process::exit(exit_code);
}

fn load_config(args: Args) -> RuntimeConfig {
    let session_id = if !args.session.trim().is_empty() {
        args.session
    } else {
        resolve_session_id()
    };
    let project_root = resolve_project_root(&args.project_root);
    let pane_id = resolve_pane_id(&args.pane_id);
    let agent_label = resolve_agent_label(&args.agent_id, &project_root);
    let tab_scope = resolve_tab_scope(&args.tab_scope);
    let agent_key = build_agent_key(&session_id, &pane_id);
    let hub_url = resolve_hub_url(&args.hub_url, &args.hub_addr, &session_id);
    let pulse_socket_path = resolve_pulse_socket_path(&session_id, &args.pulse_socket_path);
    let pulse_vnext_enabled = resolve_pulse_vnext_enabled();
    let log_dir = resolve_log_dir(&args.log_dir);
    let log_stdout = resolve_log_stdout();
    RuntimeConfig {
        client: ClientConfig {
            session_id,
            agent_key,
            agent_label,
            pane_id,
            project_root,
            tab_scope,
        },
        hub_url,
        pulse_socket_path,
        pulse_vnext_enabled,
        heartbeat_interval: Duration::from_secs(args.heartbeat_interval),
        cmd: args.cmd,
        log_dir,
        log_stdout,
    }
}

async fn hub_loop(
    cfg: ClientConfig,
    hub_url: Url,
    rx: &mut mpsc::Receiver<String>,
    cache: Arc<Mutex<CachedMessages>>,
) {
    let mut backoff = Duration::from_secs(1);
    loop {
        let connect = connect_async(hub_url.clone()).await;
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

        let hello = build_hello(&cfg);
        if ws
            .send(tokio_tungstenite::tungstenite::Message::Text(hello))
            .await
            .is_err()
        {
            warn!("hub_hello_error");
            let _ = ws.close(None).await;
            continue;
        }

        let (cached_status, cached_diff, cached_tasks) = {
            let cache = cache.lock().await;
            (
                cache.status.clone(),
                cache.diff_summary.clone(),
                cache.task_summary.clone(),
            )
        };
        if let Some(status) = cached_status {
            let _ = ws
                .send(tokio_tungstenite::tungstenite::Message::Text(status))
                .await;
        }
        if let Some(diff) = cached_diff {
            let _ = ws
                .send(tokio_tungstenite::tungstenite::Message::Text(diff))
                .await;
        }
        if !cached_tasks.is_empty() {
            let mut tags: Vec<_> = cached_tasks.keys().cloned().collect();
            tags.sort();
            for tag in tags {
                if let Some(msg) = cached_tasks.get(&tag) {
                    let _ = ws
                        .send(tokio_tungstenite::tungstenite::Message::Text(msg.clone()))
                        .await;
                }
            }
        }

        loop {
            tokio::select! {
                Some(msg) = ws.next() => {
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                            if let Some(response) = handle_incoming(&cfg, &text).await {
                                let _ = ws.send(tokio_tungstenite::tungstenite::Message::Text(response)).await;
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                Some(out) = rx.recv() => {
                    if ws.send(tokio_tungstenite::tungstenite::Message::Text(out)).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
        let _ = ws.close(None).await;
    }
}

fn publish_pulse_update(tx: &mpsc::Sender<PulseUpdate>, update: PulseUpdate) {
    match tx.try_send(update) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(_)) => {
            warn!("pulse_update_drop: queue_full");
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {}
    }
}

#[cfg(not(unix))]
async fn pulse_loop(
    _cfg: ClientConfig,
    _socket_path: PathBuf,
    mut rx: mpsc::Receiver<PulseUpdate>,
) {
    while rx.recv().await.is_some() {}
}

#[cfg(unix)]
async fn pulse_loop(cfg: ClientConfig, socket_path: PathBuf, mut rx: mpsc::Receiver<PulseUpdate>) {
    let mut state = PulseState::new();
    let mut backoff = Duration::from_secs(1);
    let mut mind_runtime = match MindRuntime::new(&cfg) {
        Ok(runtime) => Some(runtime),
        Err(err) => {
            warn!("mind_runtime_init_failed: {err}");
            None
        }
    };
    if let Some(runtime) = mind_runtime.as_ref() {
        let startup_injection = build_mind_injection_payload(
            &cfg,
            Some(runtime),
            MindInjectionTriggerKind::Startup,
            cfg.tab_scope.as_deref(),
            Some("session startup baseline handshake".to_string()),
        );
        apply_pulse_update(&mut state, PulseUpdate::MindInjection(startup_injection));
        apply_pulse_update(&mut state, runtime.detached_status_update());
    }

    loop {
        let stream = match UnixStream::connect(&socket_path).await {
            Ok(stream) => stream,
            Err(err) => {
                warn!("pulse_connect_error: {err}");
                let sleep = tokio::time::sleep(backoff);
                tokio::pin!(sleep);
                loop {
                    tokio::select! {
                        _ = &mut sleep => break,
                        update = rx.recv() => {
                            match update {
                                Some(PulseUpdate::Shutdown) | None => return,
                                Some(PulseUpdate::Remove) => {}
                                Some(PulseUpdate::Heartbeat { lifecycle }) => {
                                    state.last_heartbeat_ms = Some(Utc::now().timestamp_millis());
                                    if let Some(lifecycle) = lifecycle {
                                        state.lifecycle = normalize_lifecycle_status(&lifecycle);
                                    }
                                }
                                Some(other) => {
                                    apply_pulse_update_with_injection_adapters(
                                        &cfg,
                                        &mut state,
                                        mind_runtime.as_ref(),
                                        other,
                                    );
                                }
                            }
                        }
                    }
                }
                backoff = next_backoff(backoff);
                continue;
            }
        };

        backoff = Duration::from_secs(1);
        let (reader_half, mut writer_half) = stream.into_split();
        if send_pulse_envelope(&mut writer_half, &build_pulse_hello(&cfg))
            .await
            .is_err()
        {
            continue;
        }

        let mut last_state_hash: Option<u64> = None;
        if send_pulse_upsert(&cfg, &state, &mut writer_half, &mut last_state_hash)
            .await
            .is_err()
        {
            continue;
        }

        let mut reader = BufReader::new(reader_half);
        let mut reflector_ticker =
            tokio::time::interval(Duration::from_secs(MIND_T3_TICK_INTERVAL_SECS));
        loop {
            tokio::select! {
                update = rx.recv() => {
                    match update {
                        Some(PulseUpdate::Shutdown) => {
                            if let Some(runtime) = mind_runtime.as_mut() {
                                let finalize = runtime.finalize_session(
                                    &cfg,
                                    MindFinalizeTrigger::Shutdown,
                                    Some("process shutdown".to_string()),
                                );
                                for update in finalize.updates {
                                    apply_pulse_update_with_injection_adapters(
                                        &cfg,
                                        &mut state,
                                        Some(&*runtime),
                                        update,
                                    );
                                }
                                for update in runtime.tick_t3_runtime() {
                                    apply_pulse_update_with_injection_adapters(
                                        &cfg,
                                        &mut state,
                                        Some(&*runtime),
                                        update,
                                    );
                                }
                                let _ = send_pulse_upsert(&cfg, &state, &mut writer_half, &mut last_state_hash).await;
                            }
                            let _ = send_pulse_envelope(&mut writer_half, &build_pulse_remove(&cfg)).await;
                            return;
                        }
                        Some(PulseUpdate::Remove) => {
                            if send_pulse_envelope(&mut writer_half, &build_pulse_remove(&cfg)).await.is_err() {
                                break;
                            }
                            last_state_hash = None;
                        }
                        Some(PulseUpdate::Heartbeat { lifecycle }) => {
                            let now = Utc::now().timestamp_millis();
                            state.last_heartbeat_ms = Some(now);
                            if let Some(lifecycle) = lifecycle {
                                state.lifecycle = normalize_lifecycle_status(&lifecycle);
                                state.updated_at_ms = Some(now);
                            }
                            let heartbeat = build_pulse_heartbeat(&cfg, Some(state.lifecycle.clone()));
                            if send_pulse_envelope(&mut writer_half, &heartbeat).await.is_err() {
                                break;
                            }
                        }
                        Some(other) => {
                            apply_pulse_update_with_injection_adapters(
                                &cfg,
                                &mut state,
                                mind_runtime.as_ref(),
                                other,
                            );
                            if send_pulse_upsert(&cfg, &state, &mut writer_half, &mut last_state_hash)
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        None => {
                            let _ = send_pulse_envelope(&mut writer_half, &build_pulse_remove(&cfg)).await;
                            return;
                        }
                    }
                }
                _ = reflector_ticker.tick() => {
                    if let Some(runtime) = mind_runtime.as_mut() {
                        let mut updates = runtime.tick_reflector_runtime();
                        if let Some(finalize) = runtime.maybe_finalize_idle(&cfg, Utc::now()) {
                            updates.extend(finalize.updates);
                        }
                        updates.extend(runtime.tick_t3_runtime());
                        updates.push(runtime.detached_status_update());
                        for update in updates {
                            apply_pulse_update_with_injection_adapters(
                                &cfg,
                                &mut state,
                                Some(&*runtime),
                                update,
                            );
                        }
                        if send_pulse_upsert(&cfg, &state, &mut writer_half, &mut last_state_hash)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                inbound = read_next_pulse_frame(&mut reader) => {
                    let Some(envelope) = inbound else {
                        break;
                    };
                    if envelope.session_id != cfg.session_id || envelope.version.0 > CURRENT_PROTOCOL_VERSION {
                        continue;
                    }
                    if let Some(command) = build_pulse_command_response(&cfg, &envelope, mind_runtime.as_mut()) {
                        if send_pulse_envelope(&mut writer_half, &command.response).await.is_err() {
                            break;
                        }
                        if !command.pulse_updates.is_empty() {
                            for update in command.pulse_updates {
                                apply_pulse_update_with_injection_adapters(
                                    &cfg,
                                    &mut state,
                                    mind_runtime.as_ref(),
                                    update,
                                );
                            }
                            if send_pulse_upsert(&cfg, &state, &mut writer_half, &mut last_state_hash)
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        if command.interrupt {
                            if let Err(err) = trigger_self_interrupt() {
                                warn!("pulse_stop_signal_error: {err}");
                            }
                        }
                    } else if let Some(consultation) = build_pulse_consultation_response(&cfg, &state, &envelope) {
                        if send_pulse_envelope(&mut writer_half, &consultation.response).await.is_err() {
                            break;
                        }
                        if !consultation.pulse_updates.is_empty() {
                            for update in consultation.pulse_updates {
                                apply_pulse_update_with_injection_adapters(
                                    &cfg,
                                    &mut state,
                                    mind_runtime.as_ref(),
                                    update,
                                );
                            }
                            if send_pulse_upsert(&cfg, &state, &mut writer_half, &mut last_state_hash)
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn apply_pulse_update(state: &mut PulseState, update: PulseUpdate) {
    let now = Utc::now().timestamp_millis();
    match update {
        PulseUpdate::Status {
            lifecycle,
            snippet,
            parser_confidence,
        } => {
            state.lifecycle = normalize_lifecycle_status(&lifecycle);
            state.snippet = snippet.and_then(|value| {
                let trimmed = redact_telemetry_text(&value).trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            });
            state.parser_confidence = parser_confidence;
            state.last_activity_ms = Some(now);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::TaskSummaries(task_summaries) => {
            state.task_summaries = task_summaries;
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::CurrentTag(current_tag) => {
            state.current_tag = Some(current_tag);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::DiffSummary(diff_summary) => {
            state.diff_summary = Some(diff_summary);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::Health(health) => {
            state.health = Some(health);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::InsightRuntime(health) => {
            state.insight_runtime = Some(health);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::InsightDetached(detached) => {
            state.insight_detached = Some(detached);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::MindObserverEvent(event) => {
            state.mind_observer.updated_at_ms = Some(now);
            state.mind_observer.events.insert(0, event);
            if state.mind_observer.events.len() > MAX_MIND_OBSERVER_EVENTS {
                state
                    .mind_observer
                    .events
                    .truncate(MAX_MIND_OBSERVER_EVENTS);
            }
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::MindInjection(payload) => {
            apply_mind_injection_with_gates(state, payload, now);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::ConsultationInbox(entry) => {
            state.consultation_inbox.insert(0, entry);
            if state.consultation_inbox.len() > MAX_CONSULTATION_EVENTS {
                state.consultation_inbox.truncate(MAX_CONSULTATION_EVENTS);
            }
            state.last_activity_ms = Some(now);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::ConsultationOutbox(entry) => {
            state.consultation_outbox.insert(0, entry);
            if state.consultation_outbox.len() > MAX_CONSULTATION_EVENTS {
                state.consultation_outbox.truncate(MAX_CONSULTATION_EVENTS);
            }
            state.last_activity_ms = Some(now);
            state.updated_at_ms = Some(now);
        }
        PulseUpdate::Heartbeat { lifecycle } => {
            state.last_heartbeat_ms = Some(now);
            if let Some(lifecycle) = lifecycle {
                state.lifecycle = normalize_lifecycle_status(&lifecycle);
            }
        }
        PulseUpdate::Remove | PulseUpdate::Shutdown => {}
    }
}

fn apply_mind_injection_with_gates(
    state: &mut PulseState,
    payload: MindInjectionPayload,
    now_ms: i64,
) {
    let now = Utc
        .timestamp_millis_opt(now_ms)
        .single()
        .unwrap_or_else(Utc::now);
    let context_pressure_pct = resolve_context_pressure_pct();
    let gated = gate_mind_injection_payload(
        state.mind_injection.as_ref(),
        payload,
        now,
        context_pressure_pct,
    );
    state.mind_injection = Some(gated);
}

fn gate_mind_injection_payload(
    previous: Option<&MindInjectionPayload>,
    mut next: MindInjectionPayload,
    now: chrono::DateTime<chrono::Utc>,
    context_pressure_pct: u8,
) -> MindInjectionPayload {
    let urgent = matches!(
        next.trigger,
        MindInjectionTriggerKind::Resume | MindInjectionTriggerKind::Handoff
    );

    let is_duplicate = next
        .payload_hash
        .as_ref()
        .zip(previous.and_then(|prev| prev.payload_hash.as_ref()))
        .map(|(left, right)| left == right)
        .unwrap_or(false);

    if is_duplicate {
        next.status = "skipped_duplicate".to_string();
        next.reason = append_reason(next.reason, "duplicate payload hash");
        return next;
    }

    if context_pressure_pct >= MIND_INJECTION_PRESSURE_SUPPRESS_PCT && !urgent {
        next.status = "suppressed_pressure".to_string();
        next.reason = append_reason(
            next.reason,
            &format!(
                "context pressure {}% >= {}%",
                context_pressure_pct, MIND_INJECTION_PRESSURE_SUPPRESS_PCT
            ),
        );
        return next;
    }

    let cooldown = chrono::Duration::milliseconds(MIND_INJECTION_COOLDOWN_MS.max(0));
    let last_queued_at = previous
        .and_then(|prev| chrono::DateTime::parse_from_rfc3339(&prev.queued_at).ok())
        .map(|ts| ts.with_timezone(&Utc));
    if !urgent
        && cooldown.num_milliseconds() > 0
        && last_queued_at
            .map(|queued_at| now < queued_at + cooldown)
            .unwrap_or(false)
    {
        next.status = "skipped_cooldown".to_string();
        next.reason = append_reason(
            next.reason,
            &format!(
                "cooldown window active ({}ms)",
                MIND_INJECTION_COOLDOWN_MS.max(0)
            ),
        );
        return next;
    }

    next.status = "pending".to_string();
    next
}

fn append_reason(existing: Option<String>, extra: &str) -> Option<String> {
    let extra = extra.trim();
    if extra.is_empty() {
        return existing;
    }

    match existing {
        Some(existing) if !existing.trim().is_empty() => {
            Some(format!("{}; {}", existing.trim(), extra))
        }
        _ => Some(extra.to_string()),
    }
}

fn resolve_context_pressure_pct() -> u8 {
    env::var("AOC_MIND_CONTEXT_PRESSURE_PCT")
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(0)
        .min(100)
}

fn apply_pulse_update_with_injection_adapters(
    cfg: &ClientConfig,
    state: &mut PulseState,
    runtime: Option<&MindRuntime>,
    update: PulseUpdate,
) {
    let update_for_event = update.clone();
    match update {
        PulseUpdate::CurrentTag(current_tag) => {
            let next_tag = current_tag.tag.trim().to_string();
            let previous_tag = state
                .current_tag
                .as_ref()
                .map(|payload| payload.tag.trim().to_string());
            apply_pulse_update(state, PulseUpdate::CurrentTag(current_tag));

            if let Some(previous_tag) = previous_tag {
                if !next_tag.is_empty() && previous_tag != next_tag {
                    let reason = format!("tag changed from {previous_tag} to {next_tag}");
                    let payload = build_mind_injection_payload(
                        cfg,
                        runtime,
                        MindInjectionTriggerKind::TagSwitch,
                        Some(next_tag.as_str()),
                        Some(reason),
                    );
                    apply_pulse_update(state, PulseUpdate::MindInjection(payload));
                }
            }
        }
        other => apply_pulse_update(state, other),
    }

    if let Some(event) = synthesize_overseer_event(cfg, state, &update_for_event) {
        state.observer_events.insert(0, event);
        if state.observer_events.len() > MAX_OVERSEER_EVENTS {
            state.observer_events.truncate(MAX_OVERSEER_EVENTS);
        }
    }
}

fn mind_context_pack_mode_for_trigger(trigger: MindInjectionTriggerKind) -> MindContextPackMode {
    match trigger {
        MindInjectionTriggerKind::Startup => MindContextPackMode::Startup,
        MindInjectionTriggerKind::TagSwitch => MindContextPackMode::TagSwitch,
        MindInjectionTriggerKind::Resume => MindContextPackMode::Resume,
        MindInjectionTriggerKind::Handoff => MindContextPackMode::Handoff,
    }
}

fn compile_mind_context_pack(
    cfg: &ClientConfig,
    runtime: Option<&MindRuntime>,
    request: MindContextPackRequest,
    overrides: Option<&MindContextPackSourceOverrides>,
) -> Result<MindContextPack, String> {
    let active_tag = request
        .active_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let role = request
        .role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let profile = request.profile;
    let line_budget = match profile {
        MindContextPackProfile::Compact => MIND_CONTEXT_PACK_COMPACT_MAX_LINES,
        MindContextPackProfile::Expanded => MIND_CONTEXT_PACK_EXPANDED_MAX_LINES,
    };
    let source_line_limit = match profile {
        MindContextPackProfile::Compact => MIND_CONTEXT_PACK_COMPACT_SOURCE_MAX_LINES,
        MindContextPackProfile::Expanded => MIND_CONTEXT_PACK_EXPANDED_SOURCE_MAX_LINES,
    };

    let mut sections = Vec::new();
    let mut citations = Vec::new();

    if let Some(text) = overrides
        .and_then(|value| value.aoc_mem.clone())
        .or_else(|| load_context_cli_output(&cfg.project_root, "aoc-mem", &["read"]))
    {
        push_context_pack_section(
            &mut sections,
            &mut citations,
            ContextLayer::AocMem,
            "aoc_mem",
            "AOC memory",
            "cmd:aoc-mem read",
            extract_nonempty_lines(&text, source_line_limit),
        );
    }

    let stm_source = match request.mode {
        MindContextPackMode::Resume => overrides
            .and_then(|value| value.aoc_stm_resume.clone())
            .or_else(|| load_context_cli_output(&cfg.project_root, "aoc-stm", &["resume"]))
            .or_else(|| overrides.and_then(|value| value.aoc_stm_current.clone()))
            .or_else(|| load_context_cli_output(&cfg.project_root, "aoc-stm", &[])),
        _ => overrides
            .and_then(|value| value.aoc_stm_current.clone())
            .or_else(|| load_context_cli_output(&cfg.project_root, "aoc-stm", &[])),
    };
    if let Some(text) = stm_source {
        let label = match request.mode {
            MindContextPackMode::Resume => "cmd:aoc-stm resume",
            _ => "cmd:aoc-stm",
        };
        push_context_pack_section(
            &mut sections,
            &mut citations,
            ContextLayer::AocStm,
            "aoc_stm",
            "AOC short-term memory",
            label,
            extract_nonempty_lines(&text, source_line_limit),
        );
    }

    let handshake_text = overrides
        .and_then(|value| value.handshake_markdown.clone())
        .or_else(|| {
            runtime.and_then(|runtime| {
                runtime
                    .store
                    .latest_handshake_snapshot(
                        "project",
                        &t3_scope_id_for_project_root(&cfg.project_root),
                    )
                    .ok()
                    .flatten()
                    .map(|snapshot| snapshot.payload_text)
            })
        })
        .or_else(|| {
            read_optional_text(
                &PathBuf::from(&cfg.project_root)
                    .join(".aoc")
                    .join("mind")
                    .join("t3")
                    .join("handshake.md"),
            )
        });
    if let Some(text) = handshake_text {
        push_context_pack_section(
            &mut sections,
            &mut citations,
            ContextLayer::AocMind,
            "t3_handshake",
            "Mind handshake canon",
            ".aoc/mind/t3/handshake.md",
            extract_nonempty_lines(&text, source_line_limit),
        );
    }

    if matches!(profile, MindContextPackProfile::Expanded) {
        if let Some(text) = overrides
            .and_then(|value| value.project_mind_markdown.clone())
            .or_else(|| {
                read_optional_text(
                    &PathBuf::from(&cfg.project_root)
                        .join(".aoc")
                        .join("mind")
                        .join("t3")
                        .join("project_mind.md"),
                )
            })
        {
            let canon_lines =
                extract_project_mind_lines(&text, active_tag.as_deref(), source_line_limit);
            push_context_pack_section(
                &mut sections,
                &mut citations,
                ContextLayer::AocMind,
                "t3_canon",
                "Project mind canon",
                ".aoc/mind/t3/project_mind.md",
                canon_lines,
            );
        }
    }

    let export_manifest = overrides
        .and_then(|value| value.latest_export_manifest.clone())
        .or_else(|| load_latest_session_export_manifest(&cfg.project_root).ok());
    if let Some(manifest) = export_manifest.filter(|manifest| {
        export_matches_active_tag(manifest.active_tag.as_deref(), active_tag.as_deref())
    }) {
        let export_dir = PathBuf::from(&manifest.export_dir);
        let t2_text = overrides
            .and_then(|value| value.latest_t2_markdown.clone())
            .or_else(|| read_optional_text(&export_dir.join("t2.md")));
        if let Some(text) = t2_text {
            push_context_pack_section(
                &mut sections,
                &mut citations,
                ContextLayer::AocMind,
                "session_t2",
                "Session reflections",
                &format!("{}/t2.md", manifest.export_dir),
                extract_nonempty_lines(&text, source_line_limit),
            );
        }

        let t1_text = overrides
            .and_then(|value| value.latest_t1_markdown.clone())
            .or_else(|| read_optional_text(&export_dir.join("t1.md")));
        if let Some(text) = t1_text {
            push_context_pack_section(
                &mut sections,
                &mut citations,
                ContextLayer::AocMind,
                "session_t1",
                "Session observations",
                &format!("{}/t1.md", manifest.export_dir),
                extract_nonempty_lines(&text, source_line_limit),
            );
        }
    }

    if sections.is_empty() {
        return Err("no context-pack sources available".to_string());
    }

    let inputs = sections
        .iter()
        .map(|section| ContextPackInput {
            layer: section.layer,
            lines: render_context_pack_section(section),
        })
        .collect::<Vec<_>>();
    let composed = compose_context_pack(&inputs, line_budget)
        .map_err(|err| format!("compose context pack failed: {err}"))?;
    let section_truncated = sections.iter().any(|section| section.truncated);

    Ok(MindContextPack {
        schema_version: MIND_CONTEXT_PACK_SCHEMA_VERSION,
        mode: request.mode,
        profile,
        role,
        active_tag,
        reason,
        line_budget,
        truncated: composed.truncated || section_truncated,
        rendered_lines: composed.lines,
        sections,
        citations,
        generated_at: Utc::now().to_rfc3339(),
    })
}

fn push_context_pack_section(
    sections: &mut Vec<MindContextPackSection>,
    citations: &mut Vec<MindContextPackCitation>,
    layer: ContextLayer,
    source_id: &str,
    title: &str,
    reference: &str,
    extracted: (Vec<String>, bool),
) {
    let (lines, truncated) = extracted;
    if lines.is_empty() {
        return;
    }

    let citation = format!("[{source_id}]");
    sections.push(MindContextPackSection {
        source_id: source_id.to_string(),
        layer,
        title: title.to_string(),
        citation: citation.clone(),
        lines,
        truncated,
    });
    citations.push(MindContextPackCitation {
        source_id: source_id.to_string(),
        label: title.to_string(),
        reference: reference.to_string(),
    });
}

fn render_context_pack_section(section: &MindContextPackSection) -> Vec<String> {
    let mut lines = vec![format!("{} {}", section.citation, section.title)];
    lines.extend(section.lines.iter().cloned());
    lines
}

fn extract_nonempty_lines(text: &str, max_lines: usize) -> (Vec<String>, bool) {
    let cleaned = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| *line != "(none)" && *line != "(empty)")
        .filter(|line| !line.starts_with("generated_at:") && !line.starts_with("_generated_at:"))
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let truncated = cleaned.len() > max_lines;
    (cleaned.into_iter().take(max_lines).collect(), truncated)
}

fn extract_project_mind_lines(
    text: &str,
    active_tag: Option<&str>,
    max_lines: usize,
) -> (Vec<String>, bool) {
    let requested = active_tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let mut selected = Vec::new();
    let mut block = Vec::new();
    let mut include_block = requested.is_none();

    let flush_block = |selected: &mut Vec<String>, block: &mut Vec<String>, include_block: bool| {
        if include_block {
            selected.extend(block.iter().cloned());
        }
        block.clear();
    };

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("## ") {
            flush_block(&mut selected, &mut block, include_block);
            include_block = false;
            continue;
        }
        if line.starts_with("### ") {
            flush_block(&mut selected, &mut block, include_block);
            include_block = requested.is_none();
            block.push(line.to_string());
            continue;
        }
        if line.is_empty() || line == "(none)" {
            continue;
        }
        if let Some(requested) = requested.as_ref() {
            if let Some(topic) = line.strip_prefix("- topic:") {
                let topic = topic.trim().to_ascii_lowercase();
                include_block = topic == *requested || topic == "global";
            }
        }
        block.push(line.to_string());
    }
    flush_block(&mut selected, &mut block, include_block);

    let truncated = selected.len() > max_lines;
    (selected.into_iter().take(max_lines).collect(), truncated)
}

fn export_matches_active_tag(export_tag: Option<&str>, requested_tag: Option<&str>) -> bool {
    match (
        export_tag.map(str::trim).filter(|value| !value.is_empty()),
        requested_tag
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (_, None) => true,
        (Some(export_tag), Some(requested_tag)) => export_tag == requested_tag,
        (None, Some(_)) => false,
    }
}

fn compile_insight_retrieval(
    project_root: &str,
    request: InsightRetrievalRequest,
) -> InsightRetrievalResult {
    let active_tag = request
        .active_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let resolved_scope = match request.scope {
        InsightRetrievalScope::Session => InsightRetrievalScope::Session,
        InsightRetrievalScope::Project => InsightRetrievalScope::Project,
        InsightRetrievalScope::Auto => {
            if load_latest_session_export_manifest(project_root)
                .ok()
                .filter(|manifest| {
                    export_matches_active_tag(manifest.active_tag.as_deref(), active_tag.as_deref())
                })
                .is_some()
            {
                InsightRetrievalScope::Auto
            } else {
                InsightRetrievalScope::Project
            }
        }
    };

    let max_results = request
        .max_results
        .unwrap_or(INSIGHT_RETRIEVAL_MAX_RESULTS_DEFAULT)
        .clamp(1, INSIGHT_RETRIEVAL_MAX_RESULTS_CAP);
    let sources =
        collect_insight_retrieval_sources(project_root, resolved_scope, active_tag.as_deref());
    let mut hits = sources
        .into_iter()
        .filter_map(|source| {
            rank_insight_retrieval_source(&request.query, &request.mode, max_results, source)
        })
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    if hits.len() > max_results {
        hits.truncate(max_results);
    }

    let citations = hits
        .iter()
        .flat_map(|hit| hit.citations.iter().cloned())
        .collect::<Vec<_>>();

    let fallback_used = hits.is_empty();
    let status = if fallback_used { "fallback" } else { "ok" }.to_string();
    let summary_lines = if fallback_used {
        vec![format!(
            "no retrieval hits for query '{}' in {:?} scope",
            request.query, resolved_scope
        )]
    } else {
        match request.mode {
            InsightRetrievalMode::Brief => hits
                .iter()
                .map(|hit| format!("{} [{}]", hit.label, hit.reference))
                .collect(),
            InsightRetrievalMode::Refs => hits
                .iter()
                .map(|hit| {
                    let drilldown = hit
                        .drilldown_refs
                        .iter()
                        .map(|item| format!("{}:{}", item.kind, item.reference))
                        .collect::<Vec<_>>()
                        .join(", ");
                    if drilldown.is_empty() {
                        format!("{} -> {}", hit.label, hit.reference)
                    } else {
                        format!("{} -> {} ({})", hit.label, hit.reference, drilldown)
                    }
                })
                .collect(),
            InsightRetrievalMode::Snips => hits
                .iter()
                .map(|hit| {
                    let preview = hit.lines.first().cloned().unwrap_or_default();
                    format!("{} -> {}", hit.label, preview)
                })
                .collect(),
        }
    };
    let line_budget_per_hit = insight_retrieval_line_budget(&request.mode, max_results);

    InsightRetrievalResult {
        query: request.query,
        scope: request.scope,
        resolved_scope,
        mode: request.mode,
        status,
        summary_lines,
        hits,
        citations,
        fallback_used,
        hit_budget: max_results,
        line_budget_per_hit,
    }
}

fn collect_insight_retrieval_sources(
    project_root: &str,
    scope: InsightRetrievalScope,
    active_tag: Option<&str>,
) -> Vec<InsightRetrievalSource> {
    let mut sources = Vec::new();

    if matches!(
        scope,
        InsightRetrievalScope::Project | InsightRetrievalScope::Auto
    ) {
        if let Some(text) = read_optional_text(
            &PathBuf::from(project_root)
                .join(".aoc")
                .join("mind")
                .join("t3")
                .join("project_mind.md"),
        ) {
            sources.extend(parse_project_mind_retrieval_sources(&text, active_tag));
        }
    }

    if matches!(
        scope,
        InsightRetrievalScope::Session | InsightRetrievalScope::Auto
    ) {
        if let Ok(manifest) = load_latest_session_export_manifest(project_root) {
            if export_matches_active_tag(manifest.active_tag.as_deref(), active_tag) {
                let export_dir = PathBuf::from(&manifest.export_dir);
                if let Some(text) = read_optional_text(&export_dir.join("t2.md")) {
                    sources.extend(parse_session_export_retrieval_sources(
                        &text,
                        "t2",
                        &manifest,
                        &format!("{}/t2.md", manifest.export_dir),
                    ));
                }
                if let Some(text) = read_optional_text(&export_dir.join("t1.md")) {
                    sources.extend(parse_session_export_retrieval_sources(
                        &text,
                        "t1",
                        &manifest,
                        &format!("{}/t1.md", manifest.export_dir),
                    ));
                }
            }
        }
    }

    sources
}

fn parse_project_mind_retrieval_sources(
    text: &str,
    active_tag: Option<&str>,
) -> Vec<InsightRetrievalSource> {
    let requested = active_tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let mut sources = Vec::new();
    let mut state = "active";
    let mut heading: Option<String> = None;
    let mut topic: Option<String> = None;
    let mut evidence_refs: Vec<String> = Vec::new();
    let mut body_lines: Vec<String> = Vec::new();

    let flush =
        |sources: &mut Vec<InsightRetrievalSource>,
         heading: &mut Option<String>,
         topic: &mut Option<String>,
         evidence_refs: &mut Vec<String>,
         body_lines: &mut Vec<String>,
         state: &str,
         requested: Option<&String>| {
            let Some(entry_heading) = heading.take() else {
                topic.take();
                evidence_refs.clear();
                body_lines.clear();
                return;
            };
            let topic_value = topic.take();
            let include = match (requested, topic_value.as_deref()) {
                (None, _) => true,
                (Some(requested), Some(topic)) => {
                    let topic = topic.trim().to_ascii_lowercase();
                    topic == *requested || topic == "global"
                }
                (Some(_), None) => false,
            };
            if !include {
                evidence_refs.clear();
                body_lines.clear();
                return;
            }

            let mut lines = vec![entry_heading.clone()];
            if let Some(topic) = topic_value.as_deref() {
                lines.push(format!("- topic: {topic}"));
            }
            lines.extend(body_lines.iter().cloned());
            let lines = lines
                .into_iter()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>();
            if lines.is_empty() {
                evidence_refs.clear();
                body_lines.clear();
                return;
            }

            let entry_id = entry_heading
                .trim_start_matches("### ")
                .split_whitespace()
                .next()
                .unwrap_or("canon-entry")
                .to_string();
            let reference = format!(".aoc/mind/t3/project_mind.md#{entry_id}");
            let mut citations = vec![InsightRetrievalCitation {
                source_id: format!("t3_canon:{entry_id}"),
                label: format!("Canon entry {entry_id}"),
                reference: reference.clone(),
                score: 0,
            }];
            citations.extend(
                evidence_refs
                    .iter()
                    .map(|evidence| InsightRetrievalCitation {
                        source_id: evidence.clone(),
                        label: format!("Evidence {evidence}"),
                        reference: evidence.clone(),
                        score: 0,
                    }),
            );

            let mut drilldown_refs = vec![InsightRetrievalDrilldownRef {
                kind: "canon_entry".to_string(),
                label: format!("Canon entry {entry_id}"),
                reference: reference.clone(),
            }];
            drilldown_refs.extend(evidence_refs.iter().map(|evidence| {
                InsightRetrievalDrilldownRef {
                    kind: "evidence_ref".to_string(),
                    label: format!("Evidence {evidence}"),
                    reference: evidence.clone(),
                }
            }));

            sources.push(InsightRetrievalSource {
                source_id: format!("t3_canon:{entry_id}"),
                scope: InsightRetrievalScope::Project,
                label: format!("Project canon {entry_id} ({state})"),
                reference,
                lines,
                citations,
                drilldown_refs,
                score_bias: if state == "active" { 20 } else { -10 },
            });
            evidence_refs.clear();
            body_lines.clear();
        };

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("## Active canon") {
            flush(
                &mut sources,
                &mut heading,
                &mut topic,
                &mut evidence_refs,
                &mut body_lines,
                state,
                requested.as_ref(),
            );
            state = "active";
            continue;
        }
        if line.starts_with("## Stale canon") {
            flush(
                &mut sources,
                &mut heading,
                &mut topic,
                &mut evidence_refs,
                &mut body_lines,
                state,
                requested.as_ref(),
            );
            state = "stale";
            continue;
        }
        if line.starts_with("### ") {
            flush(
                &mut sources,
                &mut heading,
                &mut topic,
                &mut evidence_refs,
                &mut body_lines,
                state,
                requested.as_ref(),
            );
            heading = Some(line.to_string());
            continue;
        }
        if heading.is_none() || line.is_empty() || line == "(none)" {
            continue;
        }
        if let Some(value) = line.strip_prefix("- topic:") {
            topic = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("- evidence_refs:") {
            evidence_refs = value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .collect();
            continue;
        }
        if line.starts_with("-") {
            body_lines.push(line.to_string());
            continue;
        }
        body_lines.push(line.to_string());
    }

    flush(
        &mut sources,
        &mut heading,
        &mut topic,
        &mut evidence_refs,
        &mut body_lines,
        state,
        requested.as_ref(),
    );
    sources
}

fn parse_session_export_retrieval_sources(
    text: &str,
    kind: &str,
    manifest: &SessionExportManifest,
    reference: &str,
) -> Vec<InsightRetrievalSource> {
    let mut sources = Vec::new();
    let mut heading: Option<String> = None;
    let mut body_lines = Vec::new();

    let flush = |sources: &mut Vec<InsightRetrievalSource>,
                 heading: &mut Option<String>,
                 body_lines: &mut Vec<String>| {
        let title = heading.take();
        let lines = body_lines
            .iter()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && line != "(empty)")
            .collect::<Vec<_>>();
        body_lines.clear();
        if lines.is_empty() {
            return;
        }

        let label = match title.as_deref() {
            Some(value) => format!(
                "Session {} {}",
                kind.to_uppercase(),
                value.trim_start_matches("## ")
            ),
            None => format!("Session {} export", kind.to_uppercase()),
        };
        let source_id = match title.as_deref() {
            Some(value) => {
                let artifact_id = value
                    .trim_start_matches("## ")
                    .split_whitespace()
                    .next()
                    .unwrap_or(kind);
                format!("session_{}:{}", kind, artifact_id)
            }
            None => format!("session_{}:{}", kind, manifest.session_id),
        };
        let mut citations = vec![InsightRetrievalCitation {
            source_id: source_id.clone(),
            label: label.clone(),
            reference: reference.to_string(),
            score: 0,
        }];
        citations.push(InsightRetrievalCitation {
            source_id: format!("session:{}", manifest.session_id),
            label: format!("Session {}", manifest.session_id),
            reference: manifest.export_dir.clone(),
            score: 0,
        });
        let drilldown_refs = vec![
            InsightRetrievalDrilldownRef {
                kind: "export_file".to_string(),
                label: format!("{} export file", kind.to_uppercase()),
                reference: reference.to_string(),
            },
            InsightRetrievalDrilldownRef {
                kind: "session_export".to_string(),
                label: format!("Session {} export dir", manifest.session_id),
                reference: manifest.export_dir.clone(),
            },
        ];
        sources.push(InsightRetrievalSource {
            source_id,
            scope: InsightRetrievalScope::Session,
            label,
            reference: reference.to_string(),
            lines,
            citations,
            drilldown_refs,
            score_bias: if kind == "t2" { 8 } else { 4 },
        });
    };

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("# ") {
            continue;
        }
        if line.starts_with("## ") {
            flush(&mut sources, &mut heading, &mut body_lines);
            heading = Some(line.to_string());
            continue;
        }
        if line.is_empty() {
            flush(&mut sources, &mut heading, &mut body_lines);
            continue;
        }
        body_lines.push(line.to_string());
    }
    flush(&mut sources, &mut heading, &mut body_lines);

    if sources.is_empty() {
        let (lines, _) = extract_nonempty_lines(text, 48);
        if !lines.is_empty() {
            sources.push(InsightRetrievalSource {
                source_id: format!("session_{}:{}", kind, manifest.session_id),
                scope: InsightRetrievalScope::Session,
                label: format!("Session {} export", kind.to_uppercase()),
                reference: reference.to_string(),
                lines,
                citations: vec![InsightRetrievalCitation {
                    source_id: format!("session:{}", manifest.session_id),
                    label: format!("Session {}", manifest.session_id),
                    reference: manifest.export_dir.clone(),
                    score: 0,
                }],
                drilldown_refs: vec![
                    InsightRetrievalDrilldownRef {
                        kind: "export_file".to_string(),
                        label: format!("{} export file", kind.to_uppercase()),
                        reference: reference.to_string(),
                    },
                    InsightRetrievalDrilldownRef {
                        kind: "session_export".to_string(),
                        label: format!("Session {} export dir", manifest.session_id),
                        reference: manifest.export_dir.clone(),
                    },
                ],
                score_bias: if kind == "t2" { 8 } else { 4 },
            });
        }
    }

    sources
}

fn insight_retrieval_line_budget(mode: &InsightRetrievalMode, max_results: usize) -> usize {
    match mode {
        InsightRetrievalMode::Brief => INSIGHT_RETRIEVAL_BRIEF_LINE_BUDGET,
        InsightRetrievalMode::Refs => INSIGHT_RETRIEVAL_REFS_LINE_BUDGET,
        InsightRetrievalMode::Snips => INSIGHT_RETRIEVAL_SNIPS_LINE_BUDGET,
    }
    .min(
        max_results
            .saturating_mul(INSIGHT_RETRIEVAL_SNIPS_LINE_BUDGET)
            .max(1),
    )
}

fn rank_insight_retrieval_source(
    query: &str,
    mode: &InsightRetrievalMode,
    max_results: usize,
    source: InsightRetrievalSource,
) -> Option<InsightRetrievalHit> {
    let terms = query
        .split_whitespace()
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let score_line = |line: &str| {
        let normalized = line.to_ascii_lowercase();
        let term_hits = terms
            .iter()
            .map(|term| {
                if normalized.contains(term) {
                    10 + term.len() as i64
                } else {
                    0
                }
            })
            .sum::<i64>();
        let heading_bonus = if normalized.starts_with("### ") || normalized.starts_with("## ") {
            6
        } else {
            0
        };
        term_hits + heading_bonus
    };

    let mut matched = source
        .lines
        .iter()
        .filter_map(|line| {
            let score = if terms.is_empty() {
                1
            } else {
                score_line(line)
            };
            if score > 0 {
                Some((line.clone(), score))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if matched.is_empty() {
        return None;
    }
    matched.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let line_budget = insight_retrieval_line_budget(mode, max_results);

    let lines = match mode {
        InsightRetrievalMode::Refs => Vec::new(),
        _ => matched
            .iter()
            .take(line_budget)
            .map(|(line, _)| line.clone())
            .collect(),
    };
    let lines_truncated =
        matched.len() > line_budget && !matches!(mode, InsightRetrievalMode::Refs);
    let score = source.score_bias
        + matched
            .iter()
            .take(line_budget.max(1))
            .map(|(_, score)| *score)
            .sum::<i64>();
    let citations = source
        .citations
        .into_iter()
        .map(|citation| InsightRetrievalCitation { score, ..citation })
        .collect();

    Some(InsightRetrievalHit {
        source_id: source.source_id,
        scope: source.scope,
        label: source.label,
        reference: source.reference,
        score,
        lines,
        citations,
        drilldown_refs: source.drilldown_refs,
        line_budget,
        lines_truncated,
    })
}

fn edge_kind_key(kind: MindProvenanceEdgeKind) -> &'static str {
    match kind {
        MindProvenanceEdgeKind::ScopeSession => "scope_session",
        MindProvenanceEdgeKind::ScopeHandshake => "scope_handshake",
        MindProvenanceEdgeKind::ScopeBacklogJob => "scope_backlog_job",
        MindProvenanceEdgeKind::SessionConversation => "session_conversation",
        MindProvenanceEdgeKind::ConversationParent => "conversation_parent",
        MindProvenanceEdgeKind::ConversationRoot => "conversation_root",
        MindProvenanceEdgeKind::ConversationArtifact => "conversation_artifact",
        MindProvenanceEdgeKind::ConversationCheckpoint => "conversation_checkpoint",
        MindProvenanceEdgeKind::ArtifactTrace => "artifact_trace",
        MindProvenanceEdgeKind::ArtifactSemanticProvenance => "artifact_semantic_provenance",
        MindProvenanceEdgeKind::ArtifactFileLink => "artifact_file_link",
        MindProvenanceEdgeKind::ArtifactTaskLink => "artifact_task_link",
        MindProvenanceEdgeKind::CheckpointSlice => "checkpoint_slice",
        MindProvenanceEdgeKind::SliceFileRead => "slice_file_read",
        MindProvenanceEdgeKind::SliceFileModified => "slice_file_modified",
        MindProvenanceEdgeKind::CanonSupersedes => "canon_supersedes",
        MindProvenanceEdgeKind::CanonEvidence => "canon_evidence",
        MindProvenanceEdgeKind::HandshakeCanon => "handshake_canon",
        MindProvenanceEdgeKind::BacklogJobArtifact => "backlog_job_artifact",
        MindProvenanceEdgeKind::BacklogJobCanon => "backlog_job_canon",
    }
}

struct MindProvenanceGraphBuilder {
    max_nodes: usize,
    max_edges: usize,
    nodes: Vec<MindProvenanceNode>,
    edges: Vec<MindProvenanceEdge>,
    node_ids: HashSet<String>,
    edge_ids: HashSet<String>,
    truncated: bool,
}

impl MindProvenanceGraphBuilder {
    fn new(max_nodes: usize, max_edges: usize) -> Self {
        Self {
            max_nodes: max_nodes.max(1),
            max_edges: max_edges.max(1),
            nodes: Vec::new(),
            edges: Vec::new(),
            node_ids: HashSet::new(),
            edge_ids: HashSet::new(),
            truncated: false,
        }
    }

    fn add_node(&mut self, node: MindProvenanceNode) {
        if self.node_ids.contains(&node.node_id) {
            return;
        }
        if self.nodes.len() >= self.max_nodes {
            self.truncated = true;
            return;
        }
        self.node_ids.insert(node.node_id.clone());
        self.nodes.push(node);
    }

    fn add_edge(
        &mut self,
        kind: MindProvenanceEdgeKind,
        from: impl Into<String>,
        to: impl Into<String>,
        label: Option<String>,
        attrs: std::collections::BTreeMap<String, serde_json::Value>,
    ) {
        let from = from.into();
        let to = to.into();
        if !self.node_ids.contains(&from) || !self.node_ids.contains(&to) {
            return;
        }
        let edge_id = format!(
            "{}:{}:{}:{}",
            edge_kind_key(kind),
            from,
            to,
            label.as_deref().unwrap_or_default()
        );
        if self.edge_ids.contains(&edge_id) {
            return;
        }
        if self.edges.len() >= self.max_edges {
            self.truncated = true;
            return;
        }
        self.edge_ids.insert(edge_id.clone());
        self.edges.push(MindProvenanceEdge {
            edge_id,
            kind,
            from,
            to,
            label,
            attrs,
        });
    }

    fn finish(
        mut self,
        status: &str,
        summary: String,
        seed_refs: Vec<String>,
    ) -> MindProvenanceQueryResult {
        self.nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        self.edges.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
        MindProvenanceQueryResult {
            status: status.to_string(),
            summary,
            seed_refs,
            nodes: self.nodes,
            edges: self.edges,
            truncated: self.truncated,
        }
    }
}

fn json_string_attr(
    attrs: &mut std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
    value: impl Into<String>,
) {
    attrs.insert(key.to_string(), serde_json::Value::String(value.into()));
}

fn json_bool_attr(
    attrs: &mut std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
    value: bool,
) {
    attrs.insert(key.to_string(), serde_json::Value::Bool(value));
}

fn json_u64_attr(
    attrs: &mut std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
    value: u64,
) {
    attrs.insert(
        key.to_string(),
        serde_json::Value::Number(serde_json::Number::from(value)),
    );
}

#[allow(dead_code)]
fn compile_mind_provenance_graph(
    store: &MindStore,
    request: &MindProvenanceQueryRequest,
) -> Result<MindProvenanceQueryResult, String> {
    let mut seed_refs = Vec::new();
    if let Some(project_root) = request.project_root.as_ref() {
        seed_refs.push(format!("project:{}", project_root));
    }
    if let Some(session_id) = request.session_id.as_ref() {
        seed_refs.push(format!("session:{}", session_id));
    }
    if let Some(conversation_id) = request.conversation_id.as_ref() {
        seed_refs.push(format!("conversation:{}", conversation_id));
    }
    if let Some(artifact_id) = request.artifact_id.as_ref() {
        seed_refs.push(format!("artifact:{}", artifact_id));
    }
    if let Some(checkpoint_id) = request.checkpoint_id.as_ref() {
        seed_refs.push(format!("checkpoint:{}", checkpoint_id));
    }
    if let Some(canon_entry_id) = request.canon_entry_id.as_ref() {
        seed_refs.push(format!("canon:{}", canon_entry_id));
    }

    let mut graph = MindProvenanceGraphBuilder::new(request.max_nodes, request.max_edges);
    let mut conversation_ids = HashSet::<String>::new();
    let mut artifact_ids = HashSet::<String>::new();
    let mut canon_entry_ids = HashSet::<String>::new();

    let project_scope_id = if let Some(project_root) = request.project_root.as_ref() {
        let scope_key = t3_scope_id_for_project_root(project_root);
        let mut attrs = std::collections::BTreeMap::new();
        json_string_attr(&mut attrs, "project_root", project_root.clone());
        json_string_attr(&mut attrs, "scope_key", scope_key.clone());
        if let Some(watermark) = store
            .project_watermark(&scope_key)
            .map_err(|err| format!("load project watermark failed: {err}"))?
        {
            if let Some(last_artifact_id) = watermark.last_artifact_id.as_ref() {
                json_string_attr(
                    &mut attrs,
                    "watermark_last_artifact_id",
                    last_artifact_id.clone(),
                );
            }
            if let Some(last_artifact_ts) = watermark.last_artifact_ts.as_ref() {
                json_string_attr(
                    &mut attrs,
                    "watermark_last_artifact_ts",
                    last_artifact_ts.to_rfc3339(),
                );
            }
        }
        graph.add_node(MindProvenanceNode {
            node_id: format!("scope:{}", scope_key),
            kind: MindProvenanceNodeKind::ProjectScope,
            label: project_root.clone(),
            reference: Some(scope_key.clone()),
            attrs,
        });
        Some(scope_key)
    } else {
        None
    };

    if let Some(session_id) = request.session_id.as_ref() {
        graph.add_node(MindProvenanceNode {
            node_id: format!("session:{}", session_id),
            kind: MindProvenanceNodeKind::Session,
            label: format!("Session {session_id}"),
            reference: Some(session_id.clone()),
            attrs: std::collections::BTreeMap::new(),
        });
        if let Some(scope_key) = project_scope_id.as_ref() {
            graph.add_edge(
                MindProvenanceEdgeKind::ScopeSession,
                format!("scope:{}", scope_key),
                format!("session:{}", session_id),
                Some("contains".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
        for conversation_id in store
            .conversation_ids_for_session(session_id)
            .map_err(|err| format!("list conversation lineage failed: {err}"))?
        {
            conversation_ids.insert(conversation_id);
        }
    }

    if let Some(conversation_id) = request.conversation_id.as_ref() {
        conversation_ids.insert(conversation_id.clone());
    }

    if let Some(artifact_id) = request.artifact_id.as_ref() {
        artifact_ids.insert(artifact_id.clone());
        if let Some(artifact) = store
            .artifact_by_id(artifact_id)
            .map_err(|err| format!("load artifact failed: {err}"))?
        {
            conversation_ids.insert(artifact.conversation_id);
        }
    }

    if let Some(checkpoint_id) = request.checkpoint_id.as_ref() {
        if let Some(checkpoint) = store
            .compaction_checkpoint_by_id(checkpoint_id)
            .map_err(|err| format!("load checkpoint failed: {err}"))?
        {
            conversation_ids.insert(checkpoint.conversation_id.clone());
            add_provenance_checkpoint_branch(store, &mut graph, &checkpoint)?;
        }
    }

    for conversation_id in conversation_ids.iter() {
        let lineage = store
            .conversation_lineage(conversation_id)
            .map_err(|err| format!("load conversation lineage failed: {err}"))?;
        let mut attrs = std::collections::BTreeMap::new();
        if let Some(lineage) = lineage.as_ref() {
            json_string_attr(&mut attrs, "session_id", lineage.session_id.clone());
            json_string_attr(
                &mut attrs,
                "root_conversation_id",
                lineage.root_conversation_id.clone(),
            );
            if let Some(parent) = lineage.parent_conversation_id.as_ref() {
                json_string_attr(&mut attrs, "parent_conversation_id", parent.clone());
            }
        }
        graph.add_node(MindProvenanceNode {
            node_id: format!("conversation:{}", conversation_id),
            kind: MindProvenanceNodeKind::Conversation,
            label: format!("Conversation {conversation_id}"),
            reference: Some(conversation_id.clone()),
            attrs,
        });

        if let Some(lineage) = lineage.as_ref() {
            graph.add_node(MindProvenanceNode {
                node_id: format!("session:{}", lineage.session_id),
                kind: MindProvenanceNodeKind::Session,
                label: format!("Session {}", lineage.session_id),
                reference: Some(lineage.session_id.clone()),
                attrs: std::collections::BTreeMap::new(),
            });
            if let Some(scope_key) = project_scope_id.as_ref() {
                graph.add_edge(
                    MindProvenanceEdgeKind::ScopeSession,
                    format!("scope:{}", scope_key),
                    format!("session:{}", lineage.session_id),
                    Some("contains".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
            graph.add_edge(
                MindProvenanceEdgeKind::SessionConversation,
                format!("session:{}", lineage.session_id),
                format!("conversation:{}", conversation_id),
                Some("contains".to_string()),
                std::collections::BTreeMap::new(),
            );
            if let Some(parent) = lineage.parent_conversation_id.as_ref() {
                graph.add_node(MindProvenanceNode {
                    node_id: format!("conversation:{}", parent),
                    kind: MindProvenanceNodeKind::Conversation,
                    label: format!("Conversation {parent}"),
                    reference: Some(parent.clone()),
                    attrs: std::collections::BTreeMap::new(),
                });
                graph.add_edge(
                    MindProvenanceEdgeKind::ConversationParent,
                    format!("conversation:{}", conversation_id),
                    format!("conversation:{}", parent),
                    Some("parent".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
            if lineage.root_conversation_id != *conversation_id {
                graph.add_node(MindProvenanceNode {
                    node_id: format!("conversation:{}", lineage.root_conversation_id),
                    kind: MindProvenanceNodeKind::Conversation,
                    label: format!("Conversation {}", lineage.root_conversation_id),
                    reference: Some(lineage.root_conversation_id.clone()),
                    attrs: std::collections::BTreeMap::new(),
                });
                graph.add_edge(
                    MindProvenanceEdgeKind::ConversationRoot,
                    format!("conversation:{}", conversation_id),
                    format!("conversation:{}", lineage.root_conversation_id),
                    Some("root".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }

        for artifact in store
            .artifacts_for_conversation(conversation_id)
            .map_err(|err| format!("list conversation artifacts failed: {err}"))?
        {
            artifact_ids.insert(artifact.artifact_id.clone());
            add_provenance_artifact_node(&mut graph, &artifact);
            graph.add_edge(
                MindProvenanceEdgeKind::ConversationArtifact,
                format!("conversation:{}", conversation_id),
                format!("artifact:{}", artifact.artifact_id),
                Some(artifact.kind.clone()),
                std::collections::BTreeMap::new(),
            );
        }

        for checkpoint in store
            .compaction_checkpoints_for_conversation(conversation_id)
            .map_err(|err| format!("list conversation checkpoints failed: {err}"))?
        {
            add_provenance_checkpoint_branch(store, &mut graph, &checkpoint)?;
        }
    }

    let mut artifact_queue = artifact_ids.iter().cloned().collect::<Vec<_>>();
    artifact_queue.sort();
    for artifact_id in artifact_queue {
        let Some(artifact) = store
            .artifact_by_id(&artifact_id)
            .map_err(|err| format!("load artifact by id failed: {err}"))?
        else {
            continue;
        };
        add_provenance_artifact_node(&mut graph, &artifact);
        for trace_id in &artifact.trace_ids {
            if let Some(traced) = store
                .artifact_by_id(trace_id)
                .map_err(|err| format!("load traced artifact failed: {err}"))?
            {
                add_provenance_artifact_node(&mut graph, &traced);
                graph.add_edge(
                    MindProvenanceEdgeKind::ArtifactTrace,
                    format!("artifact:{}", artifact.artifact_id),
                    format!("artifact:{}", traced.artifact_id),
                    Some("trace".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
        for entry in store
            .semantic_provenance_for_artifact(&artifact.artifact_id)
            .map_err(|err| format!("load semantic provenance failed: {err}"))?
        {
            let node_id = format!(
                "semantic:{}:{}:{}",
                artifact.artifact_id, entry.prompt_version, entry.attempt_count
            );
            let mut attrs = std::collections::BTreeMap::new();
            json_string_attr(
                &mut attrs,
                "stage",
                format!("{:?}", entry.stage).to_lowercase(),
            );
            json_string_attr(
                &mut attrs,
                "runtime",
                format!("{:?}", entry.runtime).to_lowercase(),
            );
            json_string_attr(&mut attrs, "prompt_version", entry.prompt_version.clone());
            json_bool_attr(&mut attrs, "fallback_used", entry.fallback_used);
            json_u64_attr(&mut attrs, "attempt_count", entry.attempt_count as u64);
            graph.add_node(MindProvenanceNode {
                node_id: node_id.clone(),
                kind: MindProvenanceNodeKind::SemanticProvenance,
                label: format!("{} attempt {}", artifact.artifact_id, entry.attempt_count),
                reference: Some(artifact.artifact_id.clone()),
                attrs,
            });
            graph.add_edge(
                MindProvenanceEdgeKind::ArtifactSemanticProvenance,
                format!("artifact:{}", artifact.artifact_id),
                node_id,
                Some("semantic_provenance".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
        for file_link in store
            .artifact_file_links(&artifact.artifact_id)
            .map_err(|err| format!("load artifact file links failed: {err}"))?
        {
            add_provenance_file_node(&mut graph, &file_link.path, Some(&file_link.relation));
            let mut attrs = std::collections::BTreeMap::new();
            json_string_attr(&mut attrs, "relation", file_link.relation.clone());
            json_string_attr(&mut attrs, "source", file_link.source.clone());
            graph.add_edge(
                MindProvenanceEdgeKind::ArtifactFileLink,
                format!("artifact:{}", artifact.artifact_id),
                format!("file:{}", file_link.path),
                Some(file_link.relation.clone()),
                attrs,
            );
        }
        for task_link in store
            .artifact_task_links_for_artifact(&artifact.artifact_id)
            .map_err(|err| format!("load artifact task links failed: {err}"))?
        {
            let task_id = format!("task:{}", task_link.task_id);
            let mut task_attrs = std::collections::BTreeMap::new();
            json_u64_attr(
                &mut task_attrs,
                "confidence_bps",
                task_link.confidence_bps as u64,
            );
            graph.add_node(MindProvenanceNode {
                node_id: task_id.clone(),
                kind: MindProvenanceNodeKind::Task,
                label: task_link.task_id.clone(),
                reference: Some(task_link.task_id.clone()),
                attrs: task_attrs,
            });
            let mut edge_attrs = std::collections::BTreeMap::new();
            json_string_attr(
                &mut edge_attrs,
                "relation",
                format!("{:?}", task_link.relation).to_lowercase(),
            );
            json_string_attr(&mut edge_attrs, "source", task_link.source.clone());
            graph.add_edge(
                MindProvenanceEdgeKind::ArtifactTaskLink,
                format!("artifact:{}", artifact.artifact_id),
                task_id,
                Some(format!("{:?}", task_link.relation).to_lowercase()),
                edge_attrs,
            );
        }
    }

    let canon_topic = request.active_tag.as_deref();
    let mut canon_revisions = if let Some(seed_entry) = request.canon_entry_id.as_ref() {
        store
            .canon_entry_revisions(seed_entry)
            .map_err(|err| format!("load canon revisions failed: {err}"))?
    } else {
        let mut revisions = store
            .active_canon_entries(canon_topic)
            .map_err(|err| format!("load active canon failed: {err}"))?;
        if request.include_stale_canon {
            revisions.extend(
                store
                    .canon_entries_by_state(CanonRevisionState::Stale, canon_topic)
                    .map_err(|err| format!("load stale canon failed: {err}"))?,
            );
        }
        revisions
    };
    if !request.include_stale_canon {
        canon_revisions.retain(|revision| revision.state != CanonRevisionState::Stale);
    }
    canon_revisions.sort_by(|a, b| {
        a.entry_id
            .cmp(&b.entry_id)
            .then_with(|| b.revision.cmp(&a.revision))
    });
    for revision in canon_revisions {
        let node_id = format!("canon:{}#r{}", revision.entry_id, revision.revision);
        canon_entry_ids.insert(node_id.clone());
        let mut attrs = std::collections::BTreeMap::new();
        json_u64_attr(&mut attrs, "revision", revision.revision as u64);
        json_u64_attr(&mut attrs, "confidence_bps", revision.confidence_bps as u64);
        json_u64_attr(
            &mut attrs,
            "freshness_score",
            revision.freshness_score as u64,
        );
        if let Some(topic) = revision.topic.as_ref() {
            json_string_attr(&mut attrs, "topic", topic.clone());
        }
        json_string_attr(
            &mut attrs,
            "state",
            format!("{:?}", revision.state).to_lowercase(),
        );
        graph.add_node(MindProvenanceNode {
            node_id: node_id.clone(),
            kind: MindProvenanceNodeKind::CanonEntryRevision,
            label: revision.summary.clone(),
            reference: Some(format!("{}.r{}", revision.entry_id, revision.revision)),
            attrs,
        });
        if let Some(previous) = revision.supersedes_entry_id.as_ref() {
            for prior in store
                .canon_entry_revisions(previous)
                .map_err(|err| format!("load superseded canon failed: {err}"))?
                .into_iter()
                .take(1)
            {
                let prior_id = format!("canon:{}#r{}", prior.entry_id, prior.revision);
                graph.add_node(MindProvenanceNode {
                    node_id: prior_id.clone(),
                    kind: MindProvenanceNodeKind::CanonEntryRevision,
                    label: prior.summary.clone(),
                    reference: Some(format!("{}.r{}", prior.entry_id, prior.revision)),
                    attrs: std::collections::BTreeMap::new(),
                });
                graph.add_edge(
                    MindProvenanceEdgeKind::CanonSupersedes,
                    node_id.clone(),
                    prior_id,
                    Some("supersedes".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
        for evidence_ref in &revision.evidence_refs {
            if let Some(artifact) = store
                .artifact_by_id(evidence_ref)
                .map_err(|err| format!("load canon evidence artifact failed: {err}"))?
            {
                add_provenance_artifact_node(&mut graph, &artifact);
                graph.add_edge(
                    MindProvenanceEdgeKind::CanonEvidence,
                    node_id.clone(),
                    format!("artifact:{}", artifact.artifact_id),
                    Some("evidence".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
    }

    if let (Some(project_root), Some(scope_key)) =
        (request.project_root.as_ref(), project_scope_id.as_ref())
    {
        for job in store
            .t3_backlog_jobs_for_project_root(project_root)
            .map_err(|err| format!("load t3 backlog jobs failed: {err}"))?
        {
            let mut attrs = std::collections::BTreeMap::new();
            json_string_attr(&mut attrs, "session_id", job.session_id.clone());
            json_string_attr(&mut attrs, "pane_id", job.pane_id.clone());
            json_string_attr(
                &mut attrs,
                "status",
                format!("{:?}", job.status).to_lowercase(),
            );
            json_u64_attr(&mut attrs, "attempts", job.attempts as u64);
            if let Some(active_tag) = job.active_tag.as_ref() {
                json_string_attr(&mut attrs, "active_tag", active_tag.clone());
            }
            if let Some(slice_start_id) = job.slice_start_id.as_ref() {
                json_string_attr(&mut attrs, "slice_start_id", slice_start_id.clone());
            }
            if let Some(slice_end_id) = job.slice_end_id.as_ref() {
                json_string_attr(&mut attrs, "slice_end_id", slice_end_id.clone());
            }
            graph.add_node(MindProvenanceNode {
                node_id: format!("backlog:{}", job.job_id),
                kind: MindProvenanceNodeKind::T3BacklogJob,
                label: job.job_id.clone(),
                reference: Some(job.job_id.clone()),
                attrs,
            });
            graph.add_edge(
                MindProvenanceEdgeKind::ScopeBacklogJob,
                format!("scope:{}", scope_key),
                format!("backlog:{}", job.job_id),
                Some("queued".to_string()),
                std::collections::BTreeMap::new(),
            );
            for artifact_id in &job.artifact_refs {
                if let Some(artifact) = store
                    .artifact_by_id(artifact_id)
                    .map_err(|err| format!("load backlog artifact failed: {err}"))?
                {
                    add_provenance_artifact_node(&mut graph, &artifact);
                    graph.add_edge(
                        MindProvenanceEdgeKind::BacklogJobArtifact,
                        format!("backlog:{}", job.job_id),
                        format!("artifact:{}", artifact.artifact_id),
                        Some("input".to_string()),
                        std::collections::BTreeMap::new(),
                    );
                }
            }
            for canon_id in canon_entry_ids.iter().cloned().collect::<Vec<_>>() {
                graph.add_edge(
                    MindProvenanceEdgeKind::BacklogJobCanon,
                    format!("backlog:{}", job.job_id),
                    canon_id,
                    Some("targets".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }

        if let Some(snapshot) = store
            .latest_handshake_snapshot("project", scope_key)
            .map_err(|err| format!("load handshake snapshot failed: {err}"))?
        {
            let mut attrs = std::collections::BTreeMap::new();
            json_u64_attr(&mut attrs, "token_estimate", snapshot.token_estimate as u64);
            json_string_attr(&mut attrs, "scope", snapshot.scope.clone());
            graph.add_node(MindProvenanceNode {
                node_id: format!("handshake:{}", snapshot.snapshot_id),
                kind: MindProvenanceNodeKind::HandshakeSnapshot,
                label: format!("Handshake {}", project_root),
                reference: Some(".aoc/mind/t3/handshake.md".to_string()),
                attrs,
            });
            graph.add_edge(
                MindProvenanceEdgeKind::ScopeHandshake,
                format!("scope:{}", scope_key),
                format!("handshake:{}", snapshot.snapshot_id),
                Some("latest".to_string()),
                std::collections::BTreeMap::new(),
            );
            for canon_id in canon_entry_ids.iter().cloned().collect::<Vec<_>>() {
                graph.add_edge(
                    MindProvenanceEdgeKind::HandshakeCanon,
                    format!("handshake:{}", snapshot.snapshot_id),
                    canon_id,
                    Some("renders".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
    }

    let node_count = graph.nodes.len();
    let edge_count = graph.edges.len();
    Ok(graph.finish(
        "ok",
        format!(
            "{} nodes, {} edges across lineage, artifacts, checkpoints, canon, and handshake",
            node_count, edge_count
        ),
        seed_refs,
    ))
}

fn add_provenance_artifact_node(graph: &mut MindProvenanceGraphBuilder, artifact: &StoredArtifact) {
    let mut attrs = std::collections::BTreeMap::new();
    json_string_attr(
        &mut attrs,
        "conversation_id",
        artifact.conversation_id.clone(),
    );
    json_string_attr(&mut attrs, "kind", artifact.kind.clone());
    json_u64_attr(&mut attrs, "trace_count", artifact.trace_ids.len() as u64);
    graph.add_node(MindProvenanceNode {
        node_id: format!("artifact:{}", artifact.artifact_id),
        kind: MindProvenanceNodeKind::Artifact,
        label: artifact.artifact_id.clone(),
        reference: Some(artifact.artifact_id.clone()),
        attrs,
    });
}

fn add_provenance_file_node(
    graph: &mut MindProvenanceGraphBuilder,
    path: &str,
    relation: Option<&str>,
) {
    let mut attrs = std::collections::BTreeMap::new();
    if let Some(relation) = relation {
        json_string_attr(&mut attrs, "relation", relation.to_string());
    }
    graph.add_node(MindProvenanceNode {
        node_id: format!("file:{path}"),
        kind: MindProvenanceNodeKind::File,
        label: path.to_string(),
        reference: Some(path.to_string()),
        attrs,
    });
}

fn compile_mind_provenance_export(
    store: &MindStore,
    request: MindProvenanceQueryRequest,
) -> Result<MindProvenanceExport, String> {
    let graph = compile_mind_provenance_graph(store, &request)?;
    Ok(MindProvenanceExport::new(request, graph))
}

fn add_provenance_checkpoint_branch(
    store: &MindStore,
    graph: &mut MindProvenanceGraphBuilder,
    checkpoint: &CompactionCheckpoint,
) -> Result<(), String> {
    let mut attrs = std::collections::BTreeMap::new();
    json_string_attr(&mut attrs, "session_id", checkpoint.session_id.clone());
    json_string_attr(
        &mut attrs,
        "trigger_source",
        checkpoint.trigger_source.clone(),
    );
    if let Some(reason) = checkpoint.reason.as_ref() {
        json_string_attr(&mut attrs, "reason", reason.clone());
    }
    graph.add_node(MindProvenanceNode {
        node_id: format!("checkpoint:{}", checkpoint.checkpoint_id),
        kind: MindProvenanceNodeKind::CompactionCheckpoint,
        label: checkpoint.checkpoint_id.clone(),
        reference: Some(checkpoint.checkpoint_id.clone()),
        attrs,
    });
    graph.add_node(MindProvenanceNode {
        node_id: format!("conversation:{}", checkpoint.conversation_id),
        kind: MindProvenanceNodeKind::Conversation,
        label: format!("Conversation {}", checkpoint.conversation_id),
        reference: Some(checkpoint.conversation_id.clone()),
        attrs: std::collections::BTreeMap::new(),
    });
    graph.add_edge(
        MindProvenanceEdgeKind::ConversationCheckpoint,
        format!("conversation:{}", checkpoint.conversation_id),
        format!("checkpoint:{}", checkpoint.checkpoint_id),
        Some("checkpoint".to_string()),
        std::collections::BTreeMap::new(),
    );

    if let Some(slice) = store
        .compaction_t0_slice_for_checkpoint(&checkpoint.checkpoint_id)
        .map_err(|err| format!("load compaction t0 slice failed: {err}"))?
    {
        let mut slice_attrs = std::collections::BTreeMap::new();
        json_string_attr(&mut slice_attrs, "source_kind", slice.source_kind.clone());
        json_u64_attr(
            &mut slice_attrs,
            "schema_version",
            slice.schema_version as u64,
        );
        graph.add_node(MindProvenanceNode {
            node_id: format!("slice:{}", slice.slice_id),
            kind: MindProvenanceNodeKind::CompactionT0Slice,
            label: slice.slice_id.clone(),
            reference: Some(slice.slice_id.clone()),
            attrs: slice_attrs,
        });
        graph.add_edge(
            MindProvenanceEdgeKind::CheckpointSlice,
            format!("checkpoint:{}", checkpoint.checkpoint_id),
            format!("slice:{}", slice.slice_id),
            Some("materializes".to_string()),
            std::collections::BTreeMap::new(),
        );
        for path in &slice.read_files {
            add_provenance_file_node(graph, path, Some("read"));
            graph.add_edge(
                MindProvenanceEdgeKind::SliceFileRead,
                format!("slice:{}", slice.slice_id),
                format!("file:{path}"),
                Some("read".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
        for path in &slice.modified_files {
            add_provenance_file_node(graph, path, Some("modified"));
            graph.add_edge(
                MindProvenanceEdgeKind::SliceFileModified,
                format!("slice:{}", slice.slice_id),
                format!("file:{path}"),
                Some("modified".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
    }
    Ok(())
}

fn load_context_cli_output(project_root: &str, program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

fn read_optional_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn build_mind_injection_payload(
    cfg: &ClientConfig,
    runtime: Option<&MindRuntime>,
    trigger: MindInjectionTriggerKind,
    active_tag: Option<&str>,
    reason: Option<String>,
) -> MindInjectionPayload {
    let scope = "project".to_string();
    let scope_key = t3_scope_id_for_project_root(&cfg.project_root);
    let snapshot = runtime.and_then(|runtime| {
        runtime
            .store
            .latest_handshake_snapshot(&scope, &scope_key)
            .ok()
            .flatten()
    });

    let active_tag = active_tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let context_pack = compile_mind_context_pack(
        cfg,
        runtime,
        MindContextPackRequest {
            mode: mind_context_pack_mode_for_trigger(trigger),
            profile: MindContextPackProfile::Compact,
            active_tag: active_tag.clone(),
            reason: reason.clone(),
            role: None,
        },
        None,
    )
    .ok()
    .and_then(|value| serde_json::to_value(value).ok());

    MindInjectionPayload {
        status: "pending".to_string(),
        trigger,
        scope,
        scope_key,
        active_tag,
        reason,
        snapshot_id: snapshot.as_ref().map(|value| value.snapshot_id.clone()),
        payload_hash: snapshot.as_ref().map(|value| value.payload_hash.clone()),
        token_estimate: snapshot.as_ref().map(|value| value.token_estimate),
        context_pack,
        queued_at: Utc::now().to_rfc3339(),
    }
}

fn normalize_lifecycle_status(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

fn mind_observer_event(
    status: MindObserverFeedStatus,
    trigger: MindObserverFeedTriggerKind,
    reason: Option<String>,
) -> MindObserverFeedEvent {
    let now = Utc::now().to_rfc3339();
    MindObserverFeedEvent {
        status,
        trigger,
        conversation_id: None,
        runtime: None,
        attempt_count: None,
        latency_ms: None,
        reason,
        failure_kind: None,
        enqueued_at: Some(now.clone()),
        started_at: None,
        completed_at: None,
        progress: None,
    }
}

fn build_pulse_hello(cfg: &ClientConfig) -> PulseWireEnvelope {
    PulseWireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: cfg.session_id.clone(),
        sender_id: cfg.agent_key.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Hello(PulseHelloPayload {
            client_id: cfg.agent_key.clone(),
            role: "publisher".to_string(),
            capabilities: vec![
                "state_update".to_string(),
                "heartbeat".to_string(),
                "command_result".to_string(),
            ],
            agent_id: Some(cfg.agent_key.clone()),
            pane_id: Some(cfg.pane_id.clone()),
            project_root: Some(cfg.project_root.clone()),
        }),
    }
}

fn build_pulse_remove(cfg: &ClientConfig) -> PulseWireEnvelope {
    PulseWireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: cfg.session_id.clone(),
        sender_id: cfg.agent_key.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Delta(PulseDeltaPayload {
            seq: 0,
            changes: vec![PulseStateChange {
                op: StateChangeOp::Remove,
                agent_id: cfg.agent_key.clone(),
                state: None,
            }],
        }),
    }
}

fn build_pulse_heartbeat(cfg: &ClientConfig, lifecycle: Option<String>) -> PulseWireEnvelope {
    PulseWireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: cfg.session_id.clone(),
        sender_id: cfg.agent_key.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Heartbeat(PulseHeartbeatPayload {
            agent_id: cfg.agent_key.clone(),
            last_heartbeat_ms: Utc::now().timestamp_millis(),
            lifecycle,
        }),
    }
}

fn build_pulse_agent_state(cfg: &ClientConfig, state: &PulseState) -> PulseAgentState {
    let source = build_pulse_source(cfg, state);
    PulseAgentState {
        agent_id: cfg.agent_key.clone(),
        session_id: cfg.session_id.clone(),
        pane_id: cfg.pane_id.clone(),
        lifecycle: state.lifecycle.clone(),
        snippet: state.snippet.clone(),
        last_heartbeat_ms: state.last_heartbeat_ms,
        last_activity_ms: state.last_activity_ms,
        updated_at_ms: state.updated_at_ms,
        source: Some(source),
    }
}

fn build_pulse_source(cfg: &ClientConfig, state: &PulseState) -> serde_json::Value {
    let mut source = serde_json::Map::new();
    let mut agent_status = serde_json::Map::new();
    agent_status.insert(
        "agent_id".to_string(),
        serde_json::Value::String(cfg.agent_key.clone()),
    );
    agent_status.insert(
        "agent_label".to_string(),
        serde_json::Value::String(cfg.agent_label.clone()),
    );
    agent_status.insert(
        "pane_id".to_string(),
        serde_json::Value::String(cfg.pane_id.clone()),
    );
    agent_status.insert(
        "project_root".to_string(),
        serde_json::Value::String(cfg.project_root.clone()),
    );
    if let Some(tab_scope) = cfg.tab_scope.as_ref() {
        agent_status.insert(
            "tab_scope".to_string(),
            serde_json::Value::String(tab_scope.clone()),
        );
    }
    agent_status.insert(
        "status".to_string(),
        serde_json::Value::String(state.lifecycle.clone()),
    );
    if let Some(snippet) = state.snippet.as_ref() {
        agent_status.insert(
            "message".to_string(),
            serde_json::Value::String(snippet.clone()),
        );
    }
    if let Ok(cwd) = env::current_dir() {
        agent_status.insert(
            "cwd".to_string(),
            serde_json::Value::String(cwd.to_string_lossy().to_string()),
        );
    }
    if let Some(confidence) = state.parser_confidence {
        agent_status.insert(
            "lifecycle_confidence".to_string(),
            serde_json::Value::Number(serde_json::Number::from(confidence as u64)),
        );
        source.insert(
            "parser_confidence".to_string(),
            serde_json::Value::Number(serde_json::Number::from(confidence as u64)),
        );
    }
    source.insert(
        "agent_status".to_string(),
        serde_json::Value::Object(agent_status),
    );
    source.insert(
        "agent_label".to_string(),
        serde_json::Value::String(cfg.agent_label.clone()),
    );
    source.insert(
        "project_root".to_string(),
        serde_json::Value::String(cfg.project_root.clone()),
    );
    if let Some(tab_scope) = cfg.tab_scope.as_ref() {
        source.insert(
            "tab_scope".to_string(),
            serde_json::Value::String(tab_scope.clone()),
        );
    }

    if !state.task_summaries.is_empty() {
        if let Ok(value) = serde_json::to_value(&state.task_summaries) {
            source.insert("task_summaries".to_string(), value.clone());
            source.insert("task_summary".to_string(), value);
        }
    }
    if let Some(current_tag) = state.current_tag.as_ref() {
        if let Ok(value) = serde_json::to_value(current_tag) {
            source.insert("current_tag".to_string(), value);
        }
    }
    if let Some(diff_summary) = state.diff_summary.as_ref() {
        if let Ok(value) = serde_json::to_value(diff_summary) {
            source.insert("diff_summary".to_string(), value);
        }
    }
    if let Some(health) = state.health.as_ref() {
        if let Ok(value) = serde_json::to_value(health) {
            source.insert("health".to_string(), value);
        }
    }
    if let Some(insight_runtime) = state.insight_runtime.as_ref() {
        if let Ok(value) = serde_json::to_value(insight_runtime) {
            source.insert("insight_runtime".to_string(), value);
        }
    }
    if let Some(insight_detached) = state.insight_detached.as_ref() {
        if let Ok(value) = serde_json::to_value(insight_detached) {
            source.insert("insight_detached".to_string(), value);
        }
    }
    if !state.mind_observer.events.is_empty() {
        if let Ok(value) = serde_json::to_value(&state.mind_observer) {
            source.insert("mind_observer".to_string(), value);
        }
    }
    if let Some(mind_injection) = state.mind_injection.as_ref() {
        if let Ok(value) = serde_json::to_value(mind_injection) {
            source.insert("mind_injection".to_string(), value);
        }
    }
    if !state.consultation_inbox.is_empty() {
        if let Ok(value) = serde_json::to_value(&state.consultation_inbox) {
            source.insert("consultation_inbox".to_string(), value);
        }
    }
    if !state.consultation_outbox.is_empty() {
        if let Ok(value) = serde_json::to_value(&state.consultation_outbox) {
            source.insert("consultation_outbox".to_string(), value);
        }
    }
    let overseer_snapshot = build_overseer_worker_snapshot(cfg, state);
    if let Ok(value) = serde_json::to_value(&overseer_snapshot) {
        source.insert("worker_snapshot".to_string(), value.clone());
        source.insert("session_overseer".to_string(), value);
    }
    if !state.observer_events.is_empty() {
        if let Ok(value) = serde_json::to_value(&state.observer_events) {
            source.insert("observer_events".to_string(), value);
        }
    }
    serde_json::Value::Object(source)
}

fn build_overseer_worker_snapshot(cfg: &ClientConfig, state: &PulseState) -> WorkerSnapshot {
    let assignment = derive_worker_assignment(state);
    let summary = state.snippet.clone().or_else(|| {
        assignment
            .task_id
            .clone()
            .map(|task_id| format!("working on task {task_id}"))
    });
    let blocker = if matches!(
        map_lifecycle_to_worker_status(&state.lifecycle),
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        summary.clone()
    } else {
        None
    };

    WorkerSnapshot {
        session_id: cfg.session_id.clone(),
        agent_id: cfg.agent_key.clone(),
        pane_id: cfg.pane_id.clone(),
        role: Some("worker".to_string()),
        status: map_lifecycle_to_worker_status(&state.lifecycle),
        progress: ProgressPosition {
            phase: derive_progress_phase(state),
            percent: None,
        },
        assignment,
        summary,
        blocker,
        files_touched: state
            .diff_summary
            .as_ref()
            .map(|payload| {
                payload
                    .files
                    .iter()
                    .take(12)
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        plan_alignment: derive_plan_alignment(state),
        drift_risk: DriftRisk::Unknown,
        attention: derive_attention_signal(state),
        duplicate_work: None,
        branch: None,
        last_update_at_ms: state.updated_at_ms,
        last_meaningful_progress_at_ms: state.last_activity_ms.or(state.updated_at_ms),
        stale_after_ms: Some(5 * 60 * 1000),
        source: OverseerSourceKind::Wrapper,
        provenance: Some("wrapper_progress_emission".to_string()),
    }
}

fn derive_worker_assignment(state: &PulseState) -> WorkerAssignment {
    let mut assignment = WorkerAssignment::default();
    if let Some(current_tag) = state.current_tag.as_ref() {
        let tag = current_tag.tag.trim();
        if !tag.is_empty() {
            assignment.tag = Some(tag.to_string());
        }
        if let Some(task_summary) = state.task_summaries.get(tag) {
            if let Some(active_task) = task_summary
                .active_tasks
                .as_ref()
                .and_then(|tasks| tasks.iter().find(|task| task.active_agent))
                .or_else(|| {
                    task_summary
                        .active_tasks
                        .as_ref()
                        .and_then(|tasks| tasks.first())
                })
            {
                assignment.task_id = Some(active_task.id.clone());
            }
        }
    }
    assignment
}

fn derive_progress_phase(state: &PulseState) -> ProgressPhase {
    if matches!(
        map_lifecycle_to_worker_status(&state.lifecycle),
        WorkerStatus::Done | WorkerStatus::Offline
    ) {
        return ProgressPhase::Complete;
    }
    if state
        .snippet
        .as_deref()
        .map(|value| value.to_ascii_lowercase().contains("test"))
        .unwrap_or(false)
    {
        return ProgressPhase::Validation;
    }
    if state
        .mind_injection
        .as_ref()
        .map(|payload| payload.trigger == MindInjectionTriggerKind::Handoff)
        .unwrap_or(false)
    {
        return ProgressPhase::Handoff;
    }
    if state.current_tag.is_some() || !state.task_summaries.is_empty() {
        return ProgressPhase::Implementation;
    }
    ProgressPhase::Unknown
}

fn derive_plan_alignment(state: &PulseState) -> PlanAlignment {
    match derive_worker_assignment(state).task_id {
        Some(_) => PlanAlignment::High,
        None if state.current_tag.is_some() => PlanAlignment::Medium,
        None => PlanAlignment::Unassigned,
    }
}

fn derive_attention_signal(state: &PulseState) -> AttentionSignal {
    let status = map_lifecycle_to_worker_status(&state.lifecycle);
    match status {
        WorkerStatus::Blocked | WorkerStatus::NeedsInput => AttentionSignal {
            level: aoc_core::session_overseer::AttentionLevel::Warn,
            kind: Some("blocked".to_string()),
            reason: state.snippet.clone(),
        },
        WorkerStatus::Offline => AttentionSignal {
            level: aoc_core::session_overseer::AttentionLevel::Info,
            kind: Some("offline".to_string()),
            reason: state.snippet.clone(),
        },
        _ => AttentionSignal::default(),
    }
}

fn map_lifecycle_to_worker_status(lifecycle: &str) -> WorkerStatus {
    match normalize_lifecycle_status(lifecycle).as_str() {
        "running" | "busy" => WorkerStatus::Active,
        "needs-input" => WorkerStatus::NeedsInput,
        "error" => WorkerStatus::Blocked,
        "idle" => WorkerStatus::Idle,
        "offline" => WorkerStatus::Offline,
        _ => WorkerStatus::Active,
    }
}

fn synthesize_overseer_event(
    cfg: &ClientConfig,
    state: &PulseState,
    update: &PulseUpdate,
) -> Option<ObserverEvent> {
    let (kind, summary, reason) = match update {
        PulseUpdate::Status {
            lifecycle, snippet, ..
        } => {
            let normalized = normalize_lifecycle_status(lifecycle);
            let kind = match normalized.as_str() {
                "offline" => ObserverEventKind::TaskCompleted,
                "needs-input" | "error" => ObserverEventKind::Blocked,
                "busy" | "running" => ObserverEventKind::ProgressUpdate,
                _ => ObserverEventKind::StatusRefresh,
            };
            (kind, snippet.clone(), None)
        }
        PulseUpdate::TaskSummaries(_) => {
            let snapshot = build_overseer_worker_snapshot(cfg, state);
            let task_id = snapshot.assignment.task_id.clone();
            (
                ObserverEventKind::ProgressUpdate,
                task_id.map(|id| format!("task context updated: {id}")),
                None,
            )
        }
        PulseUpdate::CurrentTag(current_tag) => (
            ObserverEventKind::ProgressUpdate,
            Some(format!("active tag: {}", current_tag.tag)),
            None,
        ),
        PulseUpdate::DiffSummary(_) => return None,
        _ => return None,
    };

    Some(ObserverEvent {
        schema_version: OVERSEER_SCHEMA_VERSION,
        kind,
        session_id: cfg.session_id.clone(),
        agent_id: cfg.agent_key.clone(),
        pane_id: cfg.pane_id.clone(),
        source: OverseerSourceKind::Wrapper,
        summary,
        reason,
        snapshot: Some(build_overseer_worker_snapshot(cfg, state)),
        command: None,
        command_result: None,
        emitted_at_ms: Some(Utc::now().timestamp_millis()),
    })
}

#[cfg(unix)]
async fn send_pulse_upsert(
    cfg: &ClientConfig,
    state: &PulseState,
    writer: &mut OwnedWriteHalf,
    last_state_hash: &mut Option<u64>,
) -> io::Result<()> {
    let agent_state = build_pulse_agent_state(cfg, state);
    let hash = serde_json::to_string(&agent_state)
        .ok()
        .map(|encoded| stable_text_hash(&encoded));
    if hash.is_some() && hash == *last_state_hash {
        return Ok(());
    }
    let envelope = PulseWireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: cfg.session_id.clone(),
        sender_id: cfg.agent_key.clone(),
        timestamp: Utc::now().to_rfc3339(),
        request_id: None,
        msg: WireMsg::Delta(PulseDeltaPayload {
            seq: 0,
            changes: vec![PulseStateChange {
                op: StateChangeOp::Upsert,
                agent_id: cfg.agent_key.clone(),
                state: Some(agent_state),
            }],
        }),
    };
    send_pulse_envelope(writer, &envelope).await?;
    *last_state_hash = hash;
    Ok(())
}

#[cfg(unix)]
async fn send_pulse_envelope(
    writer: &mut OwnedWriteHalf,
    envelope: &PulseWireEnvelope,
) -> io::Result<()> {
    let frame = encode_frame(envelope, DEFAULT_MAX_FRAME_BYTES)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    writer.write_all(&frame).await?;
    writer.flush().await
}

#[cfg(unix)]
async fn read_next_pulse_frame(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
) -> Option<PulseWireEnvelope> {
    loop {
        let mut line = Vec::new();
        let read = match reader.read_until(b'\n', &mut line).await {
            Ok(value) => value,
            Err(err) => {
                warn!("pulse_read_error: {err}");
                return None;
            }
        };
        if read == 0 {
            return None;
        }
        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }
        match decode_frame::<PulseWireEnvelope>(&line, DEFAULT_MAX_FRAME_BYTES) {
            Ok(parsed) => return Some(parsed),
            Err(err) => {
                warn!("pulse_decode_error: {err}");
            }
        }
    }
}

struct PulseCommandHandling {
    response: PulseWireEnvelope,
    interrupt: bool,
    pulse_updates: Vec<PulseUpdate>,
}

struct PulseConsultationHandling {
    response: PulseWireEnvelope,
    pulse_updates: Vec<PulseUpdate>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MindIngestBody {
    Message {
        role: String,
        text: String,
    },
    ToolResult {
        tool_name: String,
        #[serde(default)]
        is_error: bool,
        #[serde(default)]
        latency_ms: Option<u64>,
        #[serde(default)]
        exit_code: Option<i32>,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        redacted: Option<bool>,
    },
}

#[derive(Debug, Deserialize)]
struct MindIngestEventPayload {
    conversation_id: String,
    event_id: String,
    #[serde(default)]
    timestamp_ms: Option<i64>,
    body: MindIngestBody,
    #[serde(default)]
    parent_conversation_id: Option<String>,
    #[serde(default)]
    root_conversation_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MindFinalizeTrigger {
    Manual,
    Shutdown,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionExportManifest {
    schema_version: u32,
    session_id: String,
    pane_id: String,
    project_root: String,
    active_tag: Option<String>,
    conversation_ids: Vec<String>,
    export_dir: String,
    t1_count: usize,
    t2_count: usize,
    t1_artifact_ids: Vec<String>,
    t2_artifact_ids: Vec<String>,
    slice_start_id: String,
    slice_end_id: String,
    slice_hash: String,
    exported_at: String,
    last_artifact_ts: String,
    watermark_scope: String,
    t3_job_id: String,
    t3_job_inserted: bool,
}

#[derive(Clone)]
struct SessionFinalizeOutcome {
    status: &'static str,
    reason: String,
    updates: Vec<PulseUpdate>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MindContextPackMode {
    Startup,
    TagSwitch,
    Resume,
    Handoff,
    Dispatch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MindContextPackProfile {
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MindContextPackCitation {
    source_id: String,
    label: String,
    reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MindContextPackSection {
    source_id: String,
    layer: ContextLayer,
    title: String,
    citation: String,
    lines: Vec<String>,
    truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MindContextPack {
    schema_version: u16,
    mode: MindContextPackMode,
    profile: MindContextPackProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    line_budget: usize,
    truncated: bool,
    rendered_lines: Vec<String>,
    sections: Vec<MindContextPackSection>,
    citations: Vec<MindContextPackCitation>,
    generated_at: String,
}

#[derive(Debug, Clone)]
struct MindContextPackRequest {
    mode: MindContextPackMode,
    profile: MindContextPackProfile,
    active_tag: Option<String>,
    reason: Option<String>,
    role: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct MindContextPackSourceOverrides {
    aoc_mem: Option<String>,
    aoc_stm_current: Option<String>,
    aoc_stm_resume: Option<String>,
    handshake_markdown: Option<String>,
    project_mind_markdown: Option<String>,
    latest_export_manifest: Option<SessionExportManifest>,
    latest_t1_markdown: Option<String>,
    latest_t2_markdown: Option<String>,
}

#[derive(Debug, Clone)]
struct InsightRetrievalSource {
    source_id: String,
    scope: InsightRetrievalScope,
    label: String,
    reference: String,
    lines: Vec<String>,
    citations: Vec<InsightRetrievalCitation>,
    drilldown_refs: Vec<InsightRetrievalDrilldownRef>,
    score_bias: i64,
}

const INSIGHT_RETRIEVAL_MAX_RESULTS_DEFAULT: usize = 4;
const INSIGHT_RETRIEVAL_MAX_RESULTS_CAP: usize = 8;
const INSIGHT_RETRIEVAL_BRIEF_LINE_BUDGET: usize = 2;
const INSIGHT_RETRIEVAL_REFS_LINE_BUDGET: usize = 0;
const INSIGHT_RETRIEVAL_SNIPS_LINE_BUDGET: usize = 5;

#[derive(Debug, Clone, Default)]
struct CompactionTrailFile {
    path: String,
    additions: Option<u32>,
    deletions: Option<u32>,
    staged: bool,
    untracked: bool,
}

struct MindRuntime {
    store: MindStore,
    sidecar: SessionObserverSidecar<PiObserverAdapter>,
    policy: T0CompactionPolicy,
    distill: DistillationConfig,
    session_id: String,
    pane_id: String,
    project_root: PathBuf,
    latest_conversation_id: Option<String>,
    last_ingest_at: Option<chrono::DateTime<chrono::Utc>>,
    last_idle_finalize_check: Option<chrono::DateTime<chrono::Utc>>,
    insight_supervisor: InsightSupervisor,
    insight_detached: DetachedInsightRuntime,
    reflector_worker: DetachedReflectorWorker,
    t3_worker: DetachedT3Worker,
    insight_health: InsightRuntimeHealthPayload,
}

#[derive(Debug, Clone, Deserialize)]
struct MindCompactionCheckpointPayload {
    conversation_id: String,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    tokens_before: Option<u32>,
    #[serde(default)]
    first_kept_entry_id: Option<String>,
    #[serde(default)]
    compaction_entry_id: Option<String>,
    #[serde(default)]
    from_extension: Option<bool>,
}

impl MindRuntime {
    fn new(cfg: &ClientConfig) -> Result<Self, String> {
        let canonical_path = resolve_mind_store_path(cfg);
        if let Some(parent) = canonical_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create mind store dir: {err}"))?;
        }

        let store = MindStore::open(&canonical_path)
            .map_err(|err| format!("mind store open failed: {err}"))?;

        if mind_store_path_override().is_none() {
            let legacy_path = resolve_legacy_mind_store_path(cfg);
            if legacy_path != canonical_path && legacy_path.exists() {
                match store.import_legacy_store(&legacy_path) {
                    Ok(report) => {
                        if report.rows_imported > 0 {
                            info!(
                                legacy = %legacy_path.display(),
                                tables_imported = report.tables_imported,
                                rows_imported = report.rows_imported,
                                "mind_legacy_import_completed"
                            );
                        }
                    }
                    Err(err) => warn!(
                        legacy = %legacy_path.display(),
                        error = %err,
                        "mind_legacy_import_failed"
                    ),
                }
            }
        }

        let mut distill = DistillationConfig::default();
        let mut semantic = SemanticObserverConfig::default();
        semantic.mode = SemanticRuntimeMode::DeterministicOnly;
        let semantic_input_limit = semantic.profile.max_input_tokens.max(1);
        distill.t1_target_tokens = distill.t1_target_tokens.min(semantic_input_limit);
        distill.t1_hard_cap_tokens = distill.t1_hard_cap_tokens.min(semantic_input_limit);
        let sidecar =
            SessionObserverSidecar::new(distill.clone(), semantic, PiObserverAdapter::default());
        let reflector_lock_path = resolve_reflector_lock_path(cfg);
        let reflector_worker = DetachedReflectorWorker::new(ReflectorRuntimeConfig {
            scope_id: cfg.session_id.clone(),
            owner_id: cfg.agent_key.clone(),
            owner_pid: Some(std::process::id() as i64),
            lock_path: reflector_lock_path,
            lease_ttl_ms: 30_000,
            max_jobs_per_tick: 2,
            requeue_on_error: true,
        });

        let t3_scope_id = t3_scope_id_for_project_root(&cfg.project_root);
        let t3_lock_path = resolve_t3_lock_path(cfg);
        let t3_worker = DetachedT3Worker::new(T3RuntimeConfig {
            scope_id: t3_scope_id,
            owner_id: cfg.agent_key.clone(),
            owner_pid: Some(std::process::id() as i64),
            lock_path: t3_lock_path,
            lease_ttl_ms: 30_000,
            stale_claim_after_ms: 60_000,
            max_jobs_per_tick: 4,
            requeue_on_error: true,
            max_attempts: MIND_T3_MAX_ATTEMPTS,
        });

        Ok(Self {
            store,
            sidecar,
            policy: T0CompactionPolicy::default(),
            distill,
            session_id: cfg.session_id.clone(),
            pane_id: cfg.pane_id.clone(),
            project_root: PathBuf::from(&cfg.project_root),
            latest_conversation_id: None,
            last_ingest_at: None,
            last_idle_finalize_check: None,
            insight_supervisor: InsightSupervisor::new(&cfg.project_root),
            insight_detached: DetachedInsightRuntime::new(
                &cfg.project_root,
                canonical_path.clone(),
            ),
            reflector_worker,
            t3_worker,
            insight_health: InsightRuntimeHealthPayload {
                reflector_enabled: true,
                t3_enabled: true,
                ..InsightRuntimeHealthPayload::default()
            },
        })
    }

    fn ingest_event(
        &mut self,
        cfg: &ClientConfig,
        payload: MindIngestEventPayload,
    ) -> Result<MindObserverFeedProgress, String> {
        let conversation_id = payload.conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("conversation_id is required".to_string());
        }

        let ts = payload
            .timestamp_ms
            .and_then(|ms| Utc.timestamp_millis_opt(ms).single())
            .unwrap_or_else(Utc::now);

        let body = match payload.body {
            MindIngestBody::Message { role, text } => {
                let role = match role.trim().to_ascii_lowercase().as_str() {
                    "system" => ConversationRole::System,
                    "user" => ConversationRole::User,
                    "assistant" => ConversationRole::Assistant,
                    _ => return Err(format!("unsupported message role: {role}")),
                };
                RawEventBody::Message(MessageEvent { role, text })
            }
            MindIngestBody::ToolResult {
                tool_name,
                is_error,
                latency_ms,
                exit_code,
                output,
                redacted,
            } => RawEventBody::ToolResult(ToolResultEvent {
                tool_name,
                status: ToolExecutionStatus::from(!is_error),
                latency_ms,
                exit_code,
                output,
                redacted: redacted.unwrap_or(false),
            }),
        };

        let parent_conversation_id = payload
            .parent_conversation_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let root_conversation_id = payload
            .root_conversation_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| conversation_id.to_string());
        let attrs = canonical_lineage_attrs(&ConversationLineageMetadata {
            session_id: cfg.session_id.clone(),
            parent_conversation_id,
            root_conversation_id,
        });

        let raw = RawEvent {
            event_id: payload.event_id,
            conversation_id: conversation_id.to_string(),
            agent_id: cfg.agent_key.clone(),
            ts,
            body,
            attrs,
        };
        let raw = sanitize_raw_event_for_storage(&raw);

        let _ = self
            .store
            .insert_raw_event(&raw)
            .map_err(|err| format!("mind raw ingest failed: {err}"))?;

        if let Some(compact) = compact_raw_event_to_t0(&raw, &self.policy)
            .map_err(|err| format!("mind compaction failed: {err}"))?
        {
            self.store
                .upsert_t0_compact_event(&compact)
                .map_err(|err| format!("mind t0 upsert failed: {err}"))?;
        }

        self.latest_conversation_id = Some(conversation_id.to_string());
        self.last_ingest_at = Some(ts);
        self.progress_for_conversation(conversation_id)
            .ok_or_else(|| "mind progress unavailable".to_string())
    }

    fn resolve_conversation_id(&self, args: &serde_json::Value) -> Option<String> {
        args.as_object()
            .and_then(|value| value.get("conversation_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .or_else(|| self.latest_conversation_id.clone())
    }

    fn progress_for_conversation(&self, conversation_id: &str) -> Option<MindObserverFeedProgress> {
        let t0_events = self
            .store
            .t0_events_for_conversation(conversation_id)
            .ok()?;
        let t0_estimated_tokens = t0_events.iter().fold(0_u32, |total, event| {
            total.saturating_add(estimate_compact_tokens(event))
        });
        Some(MindObserverFeedProgress {
            t0_estimated_tokens,
            t1_target_tokens: self.distill.t1_target_tokens,
            t1_hard_cap_tokens: self.distill.t1_hard_cap_tokens,
            tokens_until_next_run: self
                .distill
                .t1_target_tokens
                .saturating_sub(t0_estimated_tokens),
        })
    }

    fn checkpoint_compaction(
        &mut self,
        cfg: &ClientConfig,
        payload: MindCompactionCheckpointPayload,
    ) -> Result<Vec<PulseUpdate>, String> {
        let conversation_id = payload.conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("conversation_id is required".to_string());
        }

        let ts = Utc::now();
        let mut attrs = canonical_lineage_attrs(&ConversationLineageMetadata {
            session_id: cfg.session_id.clone(),
            parent_conversation_id: None,
            root_conversation_id: conversation_id.to_string(),
        });
        attrs.insert(
            "mind_checkpoint_trigger".to_string(),
            serde_json::Value::String("pi_compact".to_string()),
        );
        if let Some(reason) = payload
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            attrs.insert(
                "mind_checkpoint_reason".to_string(),
                serde_json::Value::String(reason.to_string()),
            );
        }
        if let Some(entry_id) = payload
            .compaction_entry_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            attrs.insert(
                "mind_compaction_entry_id".to_string(),
                serde_json::Value::String(entry_id.to_string()),
            );
        }
        if let Some(first_kept_entry_id) = payload
            .first_kept_entry_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            attrs.insert(
                "mind_first_kept_entry_id".to_string(),
                serde_json::Value::String(first_kept_entry_id.to_string()),
            );
        }
        if let Some(tokens_before) = payload.tokens_before {
            attrs.insert(
                "mind_tokens_before_compact".to_string(),
                serde_json::Value::Number(tokens_before.into()),
            );
        }
        if let Some(from_extension) = payload.from_extension {
            attrs.insert(
                "mind_compaction_from_extension".to_string(),
                serde_json::Value::Bool(from_extension),
            );
        }

        let trail_files = snapshot_compaction_trail_files(&self.project_root);
        let modified_files = trail_files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        if !modified_files.is_empty() {
            attrs.insert(
                "mind_compaction_modified_files".to_string(),
                serde_json::json!(modified_files),
            );
        }

        let marker_event_id = payload
            .compaction_entry_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|entry_id| format!("evt-compaction-{conversation_id}-{entry_id}"))
            .unwrap_or_else(|| {
                format!(
                    "evt-compaction-{}-{}",
                    conversation_id,
                    ts.timestamp_nanos_opt().unwrap_or_default()
                )
            });

        let marker_event = RawEvent {
            event_id: marker_event_id.clone(),
            conversation_id: conversation_id.to_string(),
            agent_id: cfg.agent_key.clone(),
            ts,
            body: RawEventBody::Other {
                payload: serde_json::json!({
                    "kind": "compaction_checkpoint",
                    "summary": payload.summary,
                    "tokens_before": payload.tokens_before,
                    "first_kept_entry_id": payload.first_kept_entry_id,
                    "compaction_entry_id": payload.compaction_entry_id,
                    "from_extension": payload.from_extension,
                }),
            },
            attrs,
        };

        let inserted = self
            .store
            .insert_raw_event(&marker_event)
            .map_err(|err| format!("mind compaction checkpoint ingest failed: {err}"))?;

        let checkpoint_id = payload
            .compaction_entry_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|entry_id| format!("cmpchk:{conversation_id}:{entry_id}"))
            .unwrap_or_else(|| {
                format!(
                    "cmpchk:{}:{}",
                    conversation_id,
                    ts.timestamp_nanos_opt().unwrap_or_default()
                )
            });
        let checkpoint = CompactionCheckpoint {
            checkpoint_id,
            conversation_id: conversation_id.to_string(),
            session_id: cfg.session_id.clone(),
            ts,
            trigger_source: "pi_compact".to_string(),
            reason: payload.reason.clone(),
            summary: payload.summary.clone(),
            tokens_before: payload.tokens_before,
            first_kept_entry_id: payload.first_kept_entry_id.clone(),
            compaction_entry_id: payload.compaction_entry_id.clone(),
            from_extension: payload.from_extension.unwrap_or(false),
            marker_event_id: Some(marker_event_id),
            schema_version: payload.schema_version.unwrap_or(1),
            created_at: ts,
            updated_at: ts,
        };
        self.store
            .upsert_compaction_checkpoint(&checkpoint)
            .map_err(|err| format!("mind compaction checkpoint persist failed: {err}"))?;

        let slice = build_compaction_t0_slice(
            conversation_id,
            &cfg.session_id,
            ts,
            "pi_compact",
            payload.reason.as_deref(),
            payload.summary.as_deref(),
            payload.tokens_before,
            payload.first_kept_entry_id.as_deref(),
            payload.compaction_entry_id.as_deref(),
            payload.from_extension.unwrap_or(false),
            "pi_compaction_checkpoint",
            &[marker_event.event_id.clone()],
            &[],
            &modified_files,
            Some(&checkpoint.checkpoint_id),
            "t0.compaction.v1",
        )
        .map_err(|err| format!("mind compaction slice build failed: {err}"))?;
        self.store
            .upsert_compaction_t0_slice(&slice)
            .map_err(|err| format!("mind compaction slice persist failed: {err}"))?;

        self.latest_conversation_id = Some(conversation_id.to_string());
        self.last_ingest_at = Some(ts);

        let default_reason = if inserted {
            "pi compaction checkpoint"
        } else {
            "pi compaction checkpoint (duplicate marker ignored)"
        };
        let reason = payload
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| default_reason.to_string());

        let before_artifact_ids: HashSet<String> = self
            .store
            .artifacts_for_conversation(conversation_id)
            .unwrap_or_default()
            .into_iter()
            .map(|artifact| artifact.artifact_id)
            .collect();

        let updates = self.enqueue_and_run(
            cfg,
            conversation_id,
            MindObserverFeedTriggerKind::Compaction,
            Some(reason),
        );

        let _ = link_new_artifacts_to_compaction_slice(
            &self.store,
            conversation_id,
            &before_artifact_ids,
            &slice.slice_id,
        );

        if !trail_files.is_empty() {
            let _ = link_new_t1_artifacts_to_trail(
                &self.store,
                conversation_id,
                &before_artifact_ids,
                &trail_files,
                ts,
            );
        }

        Ok(updates)
    }

    fn enqueue_and_run(
        &mut self,
        cfg: &ClientConfig,
        conversation_id: &str,
        trigger: MindObserverFeedTriggerKind,
        reason: Option<String>,
    ) -> Vec<PulseUpdate> {
        let now = Utc::now();
        match trigger {
            MindObserverFeedTriggerKind::TokenThreshold => {
                self.sidecar
                    .enqueue_token_threshold(&cfg.session_id, conversation_id, now)
            }
            MindObserverFeedTriggerKind::TaskCompleted => {
                self.sidecar
                    .enqueue_task_completed(&cfg.session_id, conversation_id, now)
            }
            MindObserverFeedTriggerKind::ManualShortcut => {
                self.sidecar
                    .enqueue_manual(&cfg.session_id, conversation_id, now)
            }
            MindObserverFeedTriggerKind::Handoff => {
                self.sidecar
                    .enqueue_handoff(&cfg.session_id, conversation_id, now)
            }
            MindObserverFeedTriggerKind::Compaction => {
                self.sidecar
                    .enqueue_compaction(&cfg.session_id, conversation_id, now)
            }
        }

        let progress = self.progress_for_conversation(conversation_id);
        let mut updates = vec![PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
            status: MindObserverFeedStatus::Queued,
            trigger,
            conversation_id: Some(conversation_id.to_string()),
            runtime: None,
            attempt_count: None,
            latency_ms: None,
            reason,
            failure_kind: None,
            enqueued_at: Some(now.to_rfc3339()),
            started_at: None,
            completed_at: None,
            progress: progress.clone(),
        })];

        let run_at = if trigger == MindObserverFeedTriggerKind::TokenThreshold {
            now + chrono::Duration::milliseconds(MIND_DEBOUNCE_RUN_MS)
        } else {
            now
        };
        let outcomes = self.sidecar.run_ready(&self.store, run_at);
        let completed_at = Utc::now();
        for outcome in outcomes {
            updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                status: MindObserverFeedStatus::Running,
                trigger: map_observer_trigger(outcome.trigger.kind),
                conversation_id: Some(outcome.conversation_id.clone()),
                runtime: None,
                attempt_count: None,
                latency_ms: None,
                reason: None,
                failure_kind: None,
                enqueued_at: Some(outcome.enqueued_at.to_rfc3339()),
                started_at: Some(outcome.started_at.to_rfc3339()),
                completed_at: None,
                progress: outcome.progress.clone(),
            }));
            updates.push(PulseUpdate::MindObserverEvent(
                observer_feed_event_from_outcome(&self.store, &outcome, completed_at),
            ));
        }

        updates
    }

    fn maybe_run_token_threshold(
        &mut self,
        cfg: &ClientConfig,
        conversation_id: &str,
    ) -> Vec<PulseUpdate> {
        let Some(progress) = self.progress_for_conversation(conversation_id) else {
            return Vec::new();
        };
        if progress.t0_estimated_tokens < self.distill.t1_target_tokens {
            return Vec::new();
        }

        match self.store.conversation_needs_observer_run(conversation_id) {
            Ok(true) => self.enqueue_and_run(
                cfg,
                conversation_id,
                MindObserverFeedTriggerKind::TokenThreshold,
                Some("t0 target reached".to_string()),
            ),
            Ok(false) => Vec::new(),
            Err(err) => vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                MindObserverFeedTriggerKind::TokenThreshold,
                Some(format!("mind threshold check failed: {err}")),
            ))],
        }
    }

    fn maybe_finalize_idle(
        &mut self,
        cfg: &ClientConfig,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<SessionFinalizeOutcome> {
        let Some(last_ingest_at) = self.last_ingest_at else {
            return None;
        };

        let idle_timeout_ms = resolve_mind_idle_finalize_ms();
        if idle_timeout_ms <= 0 {
            return None;
        }

        if now < last_ingest_at + chrono::Duration::milliseconds(idle_timeout_ms) {
            return None;
        }

        if let Some(last_check) = self.last_idle_finalize_check {
            if now < last_check + chrono::Duration::milliseconds(MIND_IDLE_CHECK_INTERVAL_MS) {
                return None;
            }
        }

        self.last_idle_finalize_check = Some(now);
        Some(self.finalize_session(
            cfg,
            MindFinalizeTrigger::Idle,
            Some("idle timeout finalize".to_string()),
        ))
    }

    fn finalize_session(
        &mut self,
        cfg: &ClientConfig,
        trigger: MindFinalizeTrigger,
        reason: Option<String>,
    ) -> SessionFinalizeOutcome {
        let finalize_reason = reason.unwrap_or_else(|| "session finalize requested".to_string());
        let finalize_trigger = match trigger {
            MindFinalizeTrigger::Manual => MindObserverFeedTriggerKind::ManualShortcut,
            MindFinalizeTrigger::Shutdown => MindObserverFeedTriggerKind::Handoff,
            MindFinalizeTrigger::Idle => MindObserverFeedTriggerKind::TokenThreshold,
        };

        let timeout_ms = resolve_mind_finalize_drain_timeout_ms();
        let deadline = Utc::now() + chrono::Duration::milliseconds(timeout_ms.max(0));
        let mut updates = self.drain_pending_runtime(cfg, finalize_trigger, deadline);

        let scope_key = self.session_watermark_scope_key();
        let watermark = match self.store.project_watermark(&scope_key) {
            Ok(value) => value,
            Err(err) => {
                updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                    MindObserverFeedStatus::Error,
                    finalize_trigger,
                    Some(format!("finalize watermark read failed: {err}")),
                )));
                return SessionFinalizeOutcome {
                    status: "error",
                    reason: format!("finalize failed: watermark read error: {err}"),
                    updates,
                };
            }
        };

        let (conversation_ids, delta_artifacts) =
            match self.collect_delta_artifacts(watermark.as_ref()) {
                Ok(value) => value,
                Err(err) => {
                    updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Error,
                        finalize_trigger,
                        Some(format!("finalize collect failed: {err}")),
                    )));
                    return SessionFinalizeOutcome {
                        status: "error",
                        reason: format!("finalize failed: {err}"),
                        updates,
                    };
                }
            };

        if delta_artifacts.is_empty() {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Success,
                finalize_trigger,
                Some("finalize skipped: no new artifacts".to_string()),
            )));
            return SessionFinalizeOutcome {
                status: "ok",
                reason: format!("{}: no new finalized artifacts", finalize_reason),
                updates,
            };
        }

        let active_tag = self.resolve_session_active_tag(&conversation_ids);
        let t1_artifacts = delta_artifacts
            .iter()
            .filter(|artifact| artifact.kind == "t1")
            .cloned()
            .collect::<Vec<_>>();
        let t2_artifacts = delta_artifacts
            .iter()
            .filter(|artifact| artifact.kind == "t2")
            .cloned()
            .collect::<Vec<_>>();

        if t1_artifacts.is_empty() && t2_artifacts.is_empty() {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Success,
                finalize_trigger,
                Some("finalize skipped: no t1/t2 artifacts".to_string()),
            )));
            return SessionFinalizeOutcome {
                status: "ok",
                reason: format!("{}: no t1/t2 artifacts available", finalize_reason),
                updates,
            };
        }

        let first = delta_artifacts
            .first()
            .expect("delta_artifacts is non-empty");
        let last = delta_artifacts
            .last()
            .expect("delta_artifacts is non-empty");
        let slice_start_id = first.artifact_id.clone();
        let slice_end_id = last.artifact_id.clone();
        let artifact_ids = delta_artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect::<Vec<_>>();
        let slice_hash = match canonical_payload_hash(&(
            self.session_id.as_str(),
            self.pane_id.as_str(),
            &slice_start_id,
            &slice_end_id,
            &artifact_ids,
        )) {
            Ok(value) => value,
            Err(err) => {
                updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                    MindObserverFeedStatus::Error,
                    finalize_trigger,
                    Some(format!("finalize hash failed: {err}")),
                )));
                return SessionFinalizeOutcome {
                    status: "error",
                    reason: format!("finalize failed: hash error: {err}"),
                    updates,
                };
            }
        };

        let export_dir_name = format!(
            "{}_{}_{}",
            sanitize_component(&self.session_id),
            last.ts.format("%Y%m%dT%H%M%SZ"),
            &slice_hash[..12]
        );
        let export_dir = self
            .project_root
            .join(".aoc")
            .join("mind")
            .join("insight")
            .join(export_dir_name);

        if let Err(err) = std::fs::create_dir_all(&export_dir) {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                finalize_trigger,
                Some(format!("finalize export dir failed: {err}")),
            )));
            return SessionFinalizeOutcome {
                status: "error",
                reason: format!("finalize failed: export dir error: {err}"),
                updates,
            };
        }

        let t1_markdown = render_artifact_markdown("t1", &t1_artifacts);
        let t2_markdown = render_artifact_markdown("t2", &t2_artifacts);

        if let Err(err) = ensure_safe_export_text(&t1_markdown, "t1 export") {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                finalize_trigger,
                Some(err.clone()),
            )));
            return SessionFinalizeOutcome {
                status: "error",
                reason: format!("finalize failed: {err}"),
                updates,
            };
        }

        if let Err(err) = ensure_safe_export_text(&t2_markdown, "t2 export") {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                finalize_trigger,
                Some(err.clone()),
            )));
            return SessionFinalizeOutcome {
                status: "error",
                reason: format!("finalize failed: {err}"),
                updates,
            };
        }

        if let Err(err) = std::fs::write(export_dir.join("t1.md"), t1_markdown) {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                finalize_trigger,
                Some(format!("finalize write t1.md failed: {err}")),
            )));
            return SessionFinalizeOutcome {
                status: "error",
                reason: format!("finalize failed: t1 export error: {err}"),
                updates,
            };
        }

        if let Err(err) = std::fs::write(export_dir.join("t2.md"), t2_markdown) {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                finalize_trigger,
                Some(format!("finalize write t2.md failed: {err}")),
            )));
            return SessionFinalizeOutcome {
                status: "error",
                reason: format!("finalize failed: t2 export error: {err}"),
                updates,
            };
        }

        let (t3_job_id, t3_job_inserted) = match self.store.enqueue_t3_backlog_job(
            &cfg.project_root,
            &self.session_id,
            &self.pane_id,
            active_tag.as_deref(),
            Some(&slice_start_id),
            Some(&slice_end_id),
            &artifact_ids,
            Utc::now(),
        ) {
            Ok(result) => result,
            Err(err) => {
                updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                    MindObserverFeedStatus::Error,
                    finalize_trigger,
                    Some(format!("finalize t3 enqueue failed: {err}")),
                )));
                return SessionFinalizeOutcome {
                    status: "error",
                    reason: format!("finalize failed: t3 enqueue error: {err}"),
                    updates,
                };
            }
        };

        let manifest = SessionExportManifest {
            schema_version: 1,
            session_id: self.session_id.clone(),
            pane_id: self.pane_id.clone(),
            project_root: cfg.project_root.clone(),
            active_tag,
            conversation_ids: conversation_ids.clone(),
            export_dir: export_dir.to_string_lossy().to_string(),
            t1_count: t1_artifacts.len(),
            t2_count: t2_artifacts.len(),
            t1_artifact_ids: t1_artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.clone())
                .collect(),
            t2_artifact_ids: t2_artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.clone())
                .collect(),
            slice_start_id,
            slice_end_id: slice_end_id.clone(),
            slice_hash,
            exported_at: last.ts.to_rfc3339(),
            last_artifact_ts: last.ts.to_rfc3339(),
            watermark_scope: scope_key.clone(),
            t3_job_id,
            t3_job_inserted,
        };

        let manifest_json = match serde_json::to_string_pretty(&manifest) {
            Ok(value) => value,
            Err(err) => {
                updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                    MindObserverFeedStatus::Error,
                    finalize_trigger,
                    Some(format!("finalize manifest serialize failed: {err}")),
                )));
                return SessionFinalizeOutcome {
                    status: "error",
                    reason: format!("finalize failed: manifest serialization error: {err}"),
                    updates,
                };
            }
        };

        if let Err(err) = std::fs::write(export_dir.join("manifest.json"), manifest_json) {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                finalize_trigger,
                Some(format!("finalize write manifest failed: {err}")),
            )));
            return SessionFinalizeOutcome {
                status: "error",
                reason: format!("finalize failed: manifest write error: {err}"),
                updates,
            };
        }

        if let Err(err) = self.store.advance_project_watermark(
            &scope_key,
            Some(last.ts),
            Some(&slice_end_id),
            Utc::now(),
        ) {
            updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Error,
                finalize_trigger,
                Some(format!("finalize watermark advance failed: {err}")),
            )));
            return SessionFinalizeOutcome {
                status: "error",
                reason: format!("finalize failed: watermark write error: {err}"),
                updates,
            };
        }

        updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
            MindObserverFeedStatus::Success,
            finalize_trigger,
            Some(format!(
                "{}: session export finalized: t1={} t2={} t3_job_inserted={}",
                finalize_reason, manifest.t1_count, manifest.t2_count, manifest.t3_job_inserted
            )),
        )));

        SessionFinalizeOutcome {
            status: "ok",
            reason: format!(
                "{}: session export finalized at {}",
                finalize_reason,
                export_dir.to_string_lossy()
            ),
            updates,
        }
    }

    fn session_watermark_scope_key(&self) -> String {
        format!("session:{}:pane:{}", self.session_id, self.pane_id)
    }

    fn collect_delta_artifacts(
        &self,
        watermark: Option<&ProjectWatermark>,
    ) -> Result<(Vec<String>, Vec<StoredArtifact>), String> {
        let mut conversation_ids = self
            .store
            .conversation_ids_for_session(&self.session_id)
            .map_err(|err| format!("conversation lookup failed: {err}"))?;
        if let Some(conversation_id) = self.latest_conversation_id.as_ref() {
            conversation_ids.push(conversation_id.clone());
        }
        conversation_ids.sort();
        conversation_ids.dedup();

        let mut artifacts = Vec::new();
        for conversation_id in &conversation_ids {
            let mut rows = self
                .store
                .artifacts_for_conversation(conversation_id)
                .map_err(|err| format!("artifact lookup failed for {conversation_id}: {err}"))?;
            artifacts.append(&mut rows);
        }

        artifacts.sort_by(|left, right| {
            left.ts
                .cmp(&right.ts)
                .then(left.artifact_id.cmp(&right.artifact_id))
        });
        artifacts.dedup_by(|left, right| left.artifact_id == right.artifact_id);

        let delta = artifacts
            .into_iter()
            .filter(|artifact| artifact.kind == "t1" || artifact.kind == "t2")
            .filter(|artifact| artifact_after_watermark(artifact, watermark))
            .collect::<Vec<_>>();

        Ok((conversation_ids, delta))
    }

    fn resolve_session_active_tag(&self, conversation_ids: &[String]) -> Option<String> {
        let mut latest: Option<(chrono::DateTime<chrono::Utc>, String)> = None;
        for conversation_id in conversation_ids {
            let states = match self.store.context_states(conversation_id) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for state in states {
                let Some(tag) = state
                    .active_tag
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
                else {
                    continue;
                };

                let should_update = latest
                    .as_ref()
                    .map(|(ts, _)| state.ts > *ts)
                    .unwrap_or(true);
                if should_update {
                    latest = Some((state.ts, tag));
                }
            }
        }

        latest.map(|(_, tag)| tag)
    }

    fn drain_pending_runtime(
        &mut self,
        cfg: &ClientConfig,
        trigger: MindObserverFeedTriggerKind,
        deadline: chrono::DateTime<chrono::Utc>,
    ) -> Vec<PulseUpdate> {
        let mut updates = Vec::new();

        loop {
            let run_at = Utc::now() + chrono::Duration::milliseconds(MIND_DEBOUNCE_RUN_MS + 1);
            let outcomes = self.sidecar.run_ready(&self.store, run_at);
            let completed_at = Utc::now();
            for outcome in outcomes {
                updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                    status: MindObserverFeedStatus::Running,
                    trigger: map_observer_trigger(outcome.trigger.kind),
                    conversation_id: Some(outcome.conversation_id.clone()),
                    runtime: None,
                    attempt_count: None,
                    latency_ms: None,
                    reason: None,
                    failure_kind: None,
                    enqueued_at: Some(outcome.enqueued_at.to_rfc3339()),
                    started_at: Some(outcome.started_at.to_rfc3339()),
                    completed_at: None,
                    progress: outcome.progress.clone(),
                }));
                updates.push(PulseUpdate::MindObserverEvent(
                    observer_feed_event_from_outcome(&self.store, &outcome, completed_at),
                ));
            }

            updates.extend(self.tick_reflector_runtime());

            let observer_pending = self.sidecar.queue().pending_count(&cfg.session_id);
            let observer_active = self.sidecar.queue().has_active_run(&cfg.session_id);
            let reflector_pending = self.store.pending_reflector_jobs().unwrap_or_default();

            if observer_pending == 0 && !observer_active && reflector_pending == 0 {
                break;
            }

            if Utc::now() >= deadline {
                updates.push(PulseUpdate::MindObserverEvent(mind_observer_event(
                    MindObserverFeedStatus::Fallback,
                    trigger,
                    Some("finalize drain timeout reached; exporting current slice".to_string()),
                )));
                break;
            }
        }

        updates
    }

    fn insight_dispatch(
        &mut self,
        request: InsightDispatchRequest,
    ) -> aoc_core::insight_contracts::InsightDispatchResult {
        self.insight_health.supervisor_runs = self.insight_health.supervisor_runs.saturating_add(1);
        let result = self.insight_supervisor.dispatch(&request);
        if result.fallback_used {
            self.insight_health.supervisor_failures =
                self.insight_health.supervisor_failures.saturating_add(1);
            self.insight_health.last_error = result
                .steps
                .iter()
                .find_map(|step| step.error.clone())
                .or_else(|| Some("insight dispatch fallback".to_string()));
        } else {
            self.insight_health.last_error = None;
        }
        self.insight_health.last_tick_ms = Some(Utc::now().timestamp_millis());
        result
    }

    fn insight_bootstrap(
        &mut self,
        request: InsightBootstrapRequest,
    ) -> aoc_core::insight_contracts::InsightBootstrapResult {
        let result = self.insight_supervisor.bootstrap(&request);

        if !result.dry_run && !result.seeds.is_empty() {
            let conversation_id = self
                .latest_conversation_id
                .clone()
                .unwrap_or_else(|| "bootstrap".to_string());
            let now = Utc::now();
            for seed in &result.seeds {
                let _ = self.store.enqueue_reflector_job(
                    &seed.scope_tag,
                    &seed.source_gap_ids,
                    std::slice::from_ref(&conversation_id),
                    120,
                    now,
                );
            }
        }

        self.insight_health.queue_depth = self.store.pending_reflector_jobs().unwrap_or_default();
        self.insight_health.t3_queue_depth =
            self.store.pending_t3_backlog_jobs().unwrap_or_default();
        self.insight_health.last_tick_ms = Some(Utc::now().timestamp_millis());
        result
    }

    fn insight_status(&mut self) -> InsightStatusResult {
        self.insight_health.queue_depth = self.store.pending_reflector_jobs().unwrap_or_default();
        self.insight_health.t3_queue_depth =
            self.store.pending_t3_backlog_jobs().unwrap_or_default();
        InsightStatusResult {
            queue_depth: self.insight_health.queue_depth + self.insight_health.t3_queue_depth,
            reflector_enabled: self.insight_health.reflector_enabled,
            last_tick_ms: self.insight_health.last_tick_ms,
            lock_conflicts: self.insight_health.reflector_lock_conflicts,
            jobs_completed: self.insight_health.reflector_jobs_completed,
            jobs_failed: self.insight_health.reflector_jobs_failed,
            supervisor_runs: self.insight_health.supervisor_runs,
            last_error: self.insight_health.last_error.clone(),
        }
    }

    fn insight_retrieve(&mut self, request: InsightRetrievalRequest) -> InsightRetrievalResult {
        let result = compile_insight_retrieval(&self.project_root.to_string_lossy(), request);
        self.insight_health.last_tick_ms = Some(Utc::now().timestamp_millis());
        if result.fallback_used {
            self.insight_health.last_error = Some("insight retrieval fallback".to_string());
        } else {
            self.insight_health.last_error = None;
        }
        result
    }

    fn insight_detached_dispatch(
        &mut self,
        request: aoc_core::insight_contracts::InsightDetachedDispatchRequest,
    ) -> aoc_core::insight_contracts::InsightDetachedDispatchResult {
        self.insight_health.supervisor_runs = self.insight_health.supervisor_runs.saturating_add(1);
        self.insight_health.last_tick_ms = Some(Utc::now().timestamp_millis());
        let result = self.insight_detached.dispatch(&request);
        if result.fallback_used {
            self.insight_health.supervisor_failures =
                self.insight_health.supervisor_failures.saturating_add(1);
            self.insight_health.last_error = Some(result.summary.clone());
        }
        result
    }

    fn insight_detached_status(
        &mut self,
        request: aoc_core::insight_contracts::InsightDetachedStatusRequest,
    ) -> aoc_core::insight_contracts::InsightDetachedStatusResult {
        self.insight_health.last_tick_ms = Some(Utc::now().timestamp_millis());
        self.insight_detached.status(&request)
    }

    fn insight_detached_cancel(
        &mut self,
        request: aoc_core::insight_contracts::InsightDetachedCancelRequest,
    ) -> aoc_core::insight_contracts::InsightDetachedCancelResult {
        self.insight_health.last_tick_ms = Some(Utc::now().timestamp_millis());
        let result = self.insight_detached.cancel(&request);
        if result.fallback_used {
            self.insight_health.supervisor_failures =
                self.insight_health.supervisor_failures.saturating_add(1);
            self.insight_health.last_error = Some(result.summary.clone());
        }
        result
    }

    fn detached_status_update(&self) -> PulseUpdate {
        PulseUpdate::InsightDetached(self.insight_detached.status(
            &aoc_core::insight_contracts::InsightDetachedStatusRequest {
                job_id: None,
                limit: Some(24),
            },
        ))
    }

    fn tick_reflector_runtime(&mut self) -> Vec<PulseUpdate> {
        let now = Utc::now();
        self.insight_health.reflector_ticks = self.insight_health.reflector_ticks.saturating_add(1);
        self.insight_health.last_tick_ms = Some(now.timestamp_millis());

        let mut updates = Vec::new();
        match self
            .reflector_worker
            .run_once(&self.store, now, |store, job| {
                process_reflector_job(store, job, now)
            }) {
            Ok(report) => {
                if report.lock_conflict {
                    self.insight_health.reflector_lock_conflicts = self
                        .insight_health
                        .reflector_lock_conflicts
                        .saturating_add(1);
                }
                self.insight_health.reflector_jobs_completed = self
                    .insight_health
                    .reflector_jobs_completed
                    .saturating_add(report.jobs_completed as u64);
                self.insight_health.reflector_jobs_failed = self
                    .insight_health
                    .reflector_jobs_failed
                    .saturating_add(report.jobs_failed as u64);

                if report.jobs_failed == 0 {
                    self.insight_health.last_error = None;
                }
                if report.jobs_completed > 0 {
                    updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                        status: MindObserverFeedStatus::Success,
                        trigger: MindObserverFeedTriggerKind::TaskCompleted,
                        conversation_id: self.latest_conversation_id.clone(),
                        runtime: Some("t2_reflector".to_string()),
                        attempt_count: Some(1),
                        latency_ms: None,
                        reason: Some(format!(
                            "t2 reflector processed {} job(s)",
                            report.jobs_completed
                        )),
                        failure_kind: None,
                        enqueued_at: None,
                        started_at: None,
                        completed_at: Some(now.to_rfc3339()),
                        progress: None,
                    }));
                }
                if report.jobs_failed > 0 {
                    updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                        status: MindObserverFeedStatus::Error,
                        trigger: MindObserverFeedTriggerKind::TaskCompleted,
                        conversation_id: self.latest_conversation_id.clone(),
                        runtime: Some("t2_reflector".to_string()),
                        attempt_count: Some(1),
                        latency_ms: None,
                        reason: Some(format!("t2 reflector failed {} job(s)", report.jobs_failed)),
                        failure_kind: Some("runtime_error".to_string()),
                        enqueued_at: None,
                        started_at: None,
                        completed_at: Some(now.to_rfc3339()),
                        progress: None,
                    }));
                }
            }
            Err(err) => {
                self.insight_health.last_error = Some(format!("reflector tick failed: {err}"));
                updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                    status: MindObserverFeedStatus::Error,
                    trigger: MindObserverFeedTriggerKind::TaskCompleted,
                    conversation_id: self.latest_conversation_id.clone(),
                    runtime: Some("t2_reflector".to_string()),
                    attempt_count: Some(1),
                    latency_ms: None,
                    reason: Some("reflector tick failed".to_string()),
                    failure_kind: Some("runtime_error".to_string()),
                    enqueued_at: None,
                    started_at: None,
                    completed_at: Some(now.to_rfc3339()),
                    progress: None,
                }));
            }
        }

        self.insight_health.queue_depth = self.store.pending_reflector_jobs().unwrap_or_default();
        self.insight_health.t3_queue_depth =
            self.store.pending_t3_backlog_jobs().unwrap_or_default();
        updates.push(PulseUpdate::InsightRuntime(self.insight_health.clone()));
        updates
    }

    fn tick_t3_runtime(&mut self) -> Vec<PulseUpdate> {
        let now = Utc::now();
        self.insight_health.t3_ticks = self.insight_health.t3_ticks.saturating_add(1);
        self.insight_health.last_tick_ms = Some(now.timestamp_millis());

        let mut updates = Vec::new();
        match self.t3_worker.run_once(&self.store, now, |store, job| {
            process_t3_backlog_job(store, job, now)
        }) {
            Ok(report) => {
                if report.lock_conflict {
                    self.insight_health.t3_lock_conflicts =
                        self.insight_health.t3_lock_conflicts.saturating_add(1);
                }
                self.insight_health.t3_jobs_completed = self
                    .insight_health
                    .t3_jobs_completed
                    .saturating_add(report.jobs_completed as u64);
                self.insight_health.t3_jobs_failed = self
                    .insight_health
                    .t3_jobs_failed
                    .saturating_add(report.jobs_failed as u64);
                self.insight_health.t3_jobs_requeued = self
                    .insight_health
                    .t3_jobs_requeued
                    .saturating_add(report.jobs_requeued as u64);
                self.insight_health.t3_jobs_dead_lettered = self
                    .insight_health
                    .t3_jobs_dead_lettered
                    .saturating_add(report.jobs_dead_lettered as u64);

                if report.jobs_failed == 0 {
                    self.insight_health.last_error = None;
                }

                if report.jobs_completed > 0 {
                    updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                        status: MindObserverFeedStatus::Success,
                        trigger: MindObserverFeedTriggerKind::TaskCompleted,
                        conversation_id: self.latest_conversation_id.clone(),
                        runtime: Some("t3_backlog".to_string()),
                        attempt_count: Some(1),
                        latency_ms: None,
                        reason: Some(format!(
                            "t3 backlog processed {} job(s)",
                            report.jobs_completed
                        )),
                        failure_kind: None,
                        enqueued_at: None,
                        started_at: None,
                        completed_at: Some(now.to_rfc3339()),
                        progress: None,
                    }));
                }
                if report.jobs_failed > 0 {
                    updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                        status: MindObserverFeedStatus::Error,
                        trigger: MindObserverFeedTriggerKind::TaskCompleted,
                        conversation_id: self.latest_conversation_id.clone(),
                        runtime: Some("t3_backlog".to_string()),
                        attempt_count: Some(1),
                        latency_ms: None,
                        reason: Some(format!(
                            "t3 backlog failed {} job(s), requeued {}, dead-lettered {}",
                            report.jobs_failed, report.jobs_requeued, report.jobs_dead_lettered
                        )),
                        failure_kind: Some("runtime_error".to_string()),
                        enqueued_at: None,
                        started_at: None,
                        completed_at: Some(now.to_rfc3339()),
                        progress: None,
                    }));
                }
            }
            Err(err) => {
                self.insight_health.last_error = Some(format!("t3 backlog tick failed: {err}"));
                updates.push(PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                    status: MindObserverFeedStatus::Error,
                    trigger: MindObserverFeedTriggerKind::TaskCompleted,
                    conversation_id: self.latest_conversation_id.clone(),
                    runtime: Some("t3_backlog".to_string()),
                    attempt_count: Some(1),
                    latency_ms: None,
                    reason: Some("t3 backlog tick failed".to_string()),
                    failure_kind: Some("runtime_error".to_string()),
                    enqueued_at: None,
                    started_at: None,
                    completed_at: Some(now.to_rfc3339()),
                    progress: None,
                }));
            }
        }

        self.insight_health.queue_depth = self.store.pending_reflector_jobs().unwrap_or_default();
        self.insight_health.t3_queue_depth =
            self.store.pending_t3_backlog_jobs().unwrap_or_default();
        updates.push(PulseUpdate::InsightRuntime(self.insight_health.clone()));
        updates
    }
}

fn process_reflector_job(
    store: &MindStore,
    job: &ReflectorJob,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let mut observations = Vec::new();
    for conversation_id in &job.conversation_ids {
        let artifacts = store
            .artifacts_for_conversation(conversation_id)
            .map_err(|err| format!("load artifacts failed: {err}"))?;
        for artifact in artifacts {
            if artifact.kind == "t1" && job.observation_ids.contains(&artifact.artifact_id) {
                observations.push(artifact);
            }
        }
    }

    if observations.is_empty() {
        return Err("no matching observations found for reflector job".to_string());
    }

    observations.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
    let mut lines = vec![format!(
        "T2 runtime reflection for tag={} observations={}",
        job.active_tag,
        observations.len()
    )];
    for artifact in &observations {
        let preview = truncate_chars(normalize_text(&artifact.text), 180);
        lines.push(format!("{}: {}", artifact.artifact_id, preview));
    }
    let text = lines.join("\n");

    let input_hash = canonical_payload_hash(&(
        &job.active_tag,
        &job.observation_ids,
        &job.conversation_ids,
        job.estimated_tokens,
    ))
    .map_err(|err| err.to_string())?;
    let output_hash = canonical_payload_hash(&text).map_err(|err| err.to_string())?;
    let artifact_id = format!("ref:auto:{}", &input_hash[..16]);
    let conversation_id = observations
        .first()
        .map(|artifact| artifact.conversation_id.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let trace_ids = observations
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();

    store
        .insert_reflection(&artifact_id, &conversation_id, now, &text, &trace_ids)
        .map_err(|err| format!("insert reflection failed: {err}"))?;
    store
        .upsert_semantic_provenance(&SemanticProvenance {
            artifact_id,
            stage: SemanticStage::T2Reflector,
            runtime: SemanticRuntime::Deterministic,
            provider_name: None,
            model_id: None,
            prompt_version: "deterministic.reflector.runtime.v1".to_string(),
            input_hash,
            output_hash: Some(output_hash),
            latency_ms: None,
            attempt_count: 1,
            fallback_used: false,
            fallback_reason: None,
            failure_kind: None,
            created_at: now,
        })
        .map_err(|err| format!("insert provenance failed: {err}"))?;

    Ok(())
}

fn process_t3_backlog_job(
    store: &MindStore,
    job: &T3BacklogJob,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let mut artifacts = Vec::new();
    for artifact_id in &job.artifact_refs {
        let maybe_artifact = store
            .artifact_by_id(artifact_id)
            .map_err(|err| format!("t3 load artifact {artifact_id} failed: {err}"))?;
        if let Some(artifact) = maybe_artifact {
            artifacts.push(artifact);
        }
    }

    if artifacts.is_empty() {
        return Err(format!(
            "t3 backlog job {} has no resolvable artifacts",
            job.job_id
        ));
    }

    artifacts.sort_by(|left, right| {
        left.ts
            .cmp(&right.ts)
            .then(left.artifact_id.cmp(&right.artifact_id))
    });
    artifacts.dedup_by(|left, right| left.artifact_id == right.artifact_id);

    let watermark_scope = t3_scope_id_for_project_root(&job.project_root);
    let watermark = store
        .project_watermark(&watermark_scope)
        .map_err(|err| format!("t3 watermark lookup failed: {err}"))?;

    let delta = artifacts
        .into_iter()
        .filter(|artifact| artifact_after_watermark(artifact, watermark.as_ref()))
        .collect::<Vec<_>>();

    if delta.is_empty() {
        return Ok(());
    }

    let topic = job
        .active_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let mut latest_by_entry: HashMap<String, StoredArtifact> = HashMap::new();
    for artifact in delta.iter().cloned() {
        let entry_id =
            project_canon_entry_id_for_artifact(&job.project_root, topic.as_deref(), &artifact)?;
        if let Some(current) = latest_by_entry.get(&entry_id) {
            let should_replace = artifact.ts > current.ts
                || (artifact.ts == current.ts && artifact.artifact_id > current.artifact_id);
            if !should_replace {
                continue;
            }
        }
        latest_by_entry.insert(entry_id, artifact);
    }

    let mut touched_entry_ids = Vec::new();
    let mut entry_ids = latest_by_entry.keys().cloned().collect::<Vec<_>>();
    entry_ids.sort();

    for entry_id in entry_ids {
        let artifact = latest_by_entry
            .get(&entry_id)
            .ok_or_else(|| format!("missing t3 artifact for entry {entry_id}"))?;
        let summary = project_canon_summary(artifact);
        let evidence_refs = project_canon_evidence_refs(store, artifact)?;
        let confidence_bps = project_canon_confidence_bps(now, artifact, evidence_refs.len());
        let freshness_score = project_canon_freshness_score(now, artifact.ts);

        let revision = store
            .upsert_canon_entry_revision(
                &entry_id,
                topic.as_deref(),
                &summary,
                confidence_bps,
                freshness_score,
                None,
                &evidence_refs,
                now,
            )
            .map_err(|err| format!("t3 canon upsert failed for {entry_id}: {err}"))?;
        touched_entry_ids.push(revision.entry_id);
    }

    let stale_before = now - chrono::Duration::days(MIND_T3_CANON_STALE_AFTER_DAYS);
    store
        .mark_active_canon_entries_stale(topic.as_deref(), stale_before, &touched_entry_ids)
        .map_err(|err| format!("t3 canon stale update failed: {err}"))?;

    write_project_mind_export(store, &job.project_root, now)?;
    write_handshake_export(store, &job.project_root, topic.as_deref(), now)?;

    let last = delta.last().expect("delta is non-empty");
    store
        .advance_project_watermark(
            &watermark_scope,
            Some(last.ts),
            Some(&last.artifact_id),
            now,
        )
        .map_err(|err| format!("t3 watermark advance failed: {err}"))?;

    Ok(())
}

fn project_canon_entry_id_for_artifact(
    project_root: &str,
    topic: Option<&str>,
    artifact: &StoredArtifact,
) -> Result<String, String> {
    let digest = canonical_payload_hash(&(
        project_root,
        topic,
        artifact.conversation_id.as_str(),
        artifact.kind.as_str(),
    ))
    .map_err(|err| format!("canon entry hash failed: {err}"))?;
    Ok(format!("canon:{}", &digest[..16]))
}

fn parse_git_numstat_into(
    files: &mut HashMap<String, CompactionTrailFile>,
    raw: &str,
    staged: bool,
) {
    for line in raw.lines() {
        let mut parts = line.splitn(3, '\t');
        let additions = parts.next().unwrap_or("").trim();
        let deletions = parts.next().unwrap_or("").trim();
        let path = parts.next().unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }

        let entry = files
            .entry(path.to_string())
            .or_insert_with(|| CompactionTrailFile {
                path: path.to_string(),
                ..CompactionTrailFile::default()
            });
        entry.staged |= staged;

        if let Ok(value) = additions.parse::<u32>() {
            entry.additions = Some(entry.additions.unwrap_or(0).saturating_add(value));
        }
        if let Ok(value) = deletions.parse::<u32>() {
            entry.deletions = Some(entry.deletions.unwrap_or(0).saturating_add(value));
        }
    }
}

fn snapshot_compaction_trail_files(project_root: &Path) -> Vec<CompactionTrailFile> {
    let git_dir = project_root.join(".git");
    if !git_dir.exists() {
        return Vec::new();
    }

    let run = |args: &[&str]| -> Option<String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(project_root)
            .args(args)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    };

    let mut files = HashMap::<String, CompactionTrailFile>::new();
    if let Some(raw) = run(&["diff", "--numstat", "--cached"]) {
        parse_git_numstat_into(&mut files, &raw, true);
    }
    if let Some(raw) = run(&["diff", "--numstat"]) {
        parse_git_numstat_into(&mut files, &raw, false);
    }
    if let Some(raw) = run(&["ls-files", "--others", "--exclude-standard"]) {
        for line in raw.lines() {
            let path = line.trim();
            if path.is_empty() {
                continue;
            }
            let entry = files
                .entry(path.to_string())
                .or_insert_with(|| CompactionTrailFile {
                    path: path.to_string(),
                    ..CompactionTrailFile::default()
                });
            entry.untracked = true;
        }
    }

    let mut values = files.into_values().collect::<Vec<_>>();
    values.sort_by(|a, b| a.path.cmp(&b.path));
    values
}

fn string_list_attr(
    attrs: &std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Vec<String> {
    attrs
        .get(key)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value: &serde_json::Value| value.as_str().map(|s| s.trim().to_string()))
        .filter(|value: &String| !value.is_empty())
        .collect()
}

fn rebuild_compaction_t0_slice_from_checkpoint(
    store: &MindStore,
    checkpoint: &CompactionCheckpoint,
) -> Result<Option<aoc_core::mind_contracts::CompactionT0Slice>, String> {
    let Some(marker_event_id) = checkpoint.marker_event_id.as_deref() else {
        return Ok(None);
    };
    let Some(marker_event) = store
        .raw_event_by_id(marker_event_id)
        .map_err(|err| format!("load compaction marker failed: {err}"))?
    else {
        return Ok(None);
    };

    let read_files = string_list_attr(&marker_event.attrs, "pi_detail_read_files");
    let modified_files = {
        let live = string_list_attr(&marker_event.attrs, "mind_compaction_modified_files");
        if live.is_empty() {
            string_list_attr(&marker_event.attrs, "pi_detail_modified_files")
        } else {
            live
        }
    };
    let slice = build_compaction_t0_slice(
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
        &[marker_event.event_id],
        &read_files,
        &modified_files,
        Some(&checkpoint.checkpoint_id),
        "t0.compaction.v1",
    )
    .map_err(|err| format!("rebuild compaction slice failed: {err}"))?;

    Ok(Some(slice))
}

fn link_new_artifacts_to_compaction_slice(
    store: &MindStore,
    conversation_id: &str,
    before_artifact_ids: &HashSet<String>,
    slice_id: &str,
) -> Result<(), String> {
    let artifacts = store
        .artifacts_for_conversation(conversation_id)
        .map_err(|err| format!("list artifacts failed: {err}"))?;
    let extra_trace_ids = vec![slice_id.to_string()];

    for artifact in artifacts
        .into_iter()
        .filter(|artifact| !before_artifact_ids.contains(&artifact.artifact_id))
    {
        store
            .append_trace_ids_to_artifact(&artifact.artifact_id, &extra_trace_ids)
            .map_err(|err| format!("append compaction slice trace failed: {err}"))?;
    }

    Ok(())
}

fn link_new_t1_artifacts_to_trail(
    store: &MindStore,
    conversation_id: &str,
    before_artifact_ids: &HashSet<String>,
    trail_files: &[CompactionTrailFile],
    ts: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let artifacts = store
        .artifacts_for_conversation(conversation_id)
        .map_err(|err| format!("list artifacts failed: {err}"))?;

    for artifact in artifacts.into_iter().filter(|artifact| {
        artifact.kind == "t1" && !before_artifact_ids.contains(&artifact.artifact_id)
    }) {
        for trail in trail_files {
            store
                .upsert_artifact_file_link(&ArtifactFileLink {
                    artifact_id: artifact.artifact_id.clone(),
                    path: trail.path.clone(),
                    relation: "modified".to_string(),
                    source: "pi_compaction_git_diff".to_string(),
                    additions: trail.additions,
                    deletions: trail.deletions,
                    staged: trail.staged,
                    untracked: trail.untracked,
                    created_at: ts,
                    updated_at: ts,
                })
                .map_err(|err| format!("artifact file link upsert failed: {err}"))?;
        }
    }

    Ok(())
}

fn project_canon_summary(artifact: &StoredArtifact) -> String {
    let heading = if artifact.kind == "t2" {
        "Reflection"
    } else {
        "Observation"
    };
    let preview = truncate_chars(
        normalize_text(&artifact.text),
        MIND_T3_CANON_SUMMARY_MAX_CHARS,
    );
    format!(
        "{heading} for {} in {}: {}",
        artifact.kind, artifact.conversation_id, preview
    )
}

fn project_canon_confidence_bps(
    now: chrono::DateTime<chrono::Utc>,
    artifact: &StoredArtifact,
    evidence_count: usize,
) -> u16 {
    let base = if artifact.kind == "t2" {
        8_200u16
    } else {
        7_100u16
    };
    let evidence_boost = ((evidence_count.saturating_sub(1) as u16).saturating_mul(220)).min(1_200);
    let recency_boost = if now - artifact.ts <= chrono::Duration::days(1) {
        300u16
    } else if now - artifact.ts <= chrono::Duration::days(7) {
        120u16
    } else {
        0u16
    };
    base.saturating_add(evidence_boost)
        .saturating_add(recency_boost)
        .min(10_000)
}

fn project_canon_freshness_score(
    now: chrono::DateTime<chrono::Utc>,
    artifact_ts: chrono::DateTime<chrono::Utc>,
) -> u16 {
    if artifact_ts >= now {
        return 10_000;
    }

    let age_hours = (now - artifact_ts).num_hours().max(0) as u16;
    let decay = age_hours.saturating_mul(12).min(10_000);
    10_000u16.saturating_sub(decay)
}

fn project_canon_evidence_refs(
    store: &MindStore,
    artifact: &StoredArtifact,
) -> Result<Vec<String>, String> {
    let mut evidence_refs = vec![artifact.artifact_id.clone()];

    for trace_id in &artifact.trace_ids {
        if trace_id == &artifact.artifact_id {
            continue;
        }
        let resolvable = store
            .artifact_by_id(trace_id)
            .map_err(|err| format!("t3 evidence lookup failed for {trace_id}: {err}"))?
            .is_some();
        if resolvable {
            evidence_refs.push(trace_id.clone());
        }
    }

    evidence_refs.sort();
    evidence_refs.dedup();
    if evidence_refs.is_empty() {
        return Err(format!(
            "canon evidence set is empty for artifact {}",
            artifact.artifact_id
        ));
    }

    Ok(evidence_refs)
}

fn ensure_safe_export_text(payload: &str, label: &str) -> Result<(), String> {
    if text_contains_unredacted_secret(payload) {
        return Err(format!(
            "{label} contains unredacted secret-bearing content"
        ));
    }
    Ok(())
}

fn write_project_mind_export(
    store: &MindStore,
    project_root: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let active_entries = store
        .active_canon_entries(None)
        .map_err(|err| format!("load active canon entries failed: {err}"))?;
    let stale_entries = store
        .canon_entries_by_state(CanonRevisionState::Stale, None)
        .map_err(|err| format!("load stale canon entries failed: {err}"))?;

    let payload = render_project_mind_markdown(&active_entries, &stale_entries, now);
    ensure_safe_export_text(&payload, "project_mind export")?;
    let export_path = PathBuf::from(project_root)
        .join(".aoc")
        .join("mind")
        .join("t3")
        .join("project_mind.md");
    if let Some(parent) = export_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create project_mind export directory failed: {err}"))?;
    }
    std::fs::write(&export_path, payload)
        .map_err(|err| format!("write project_mind export failed: {err}"))?;

    Ok(())
}

fn render_project_mind_markdown(
    active_entries: &[aoc_storage::CanonEntryRevision],
    stale_entries: &[aoc_storage::CanonEntryRevision],
    generated_at: chrono::DateTime<chrono::Utc>,
) -> String {
    let mut lines = vec![
        "# Project Mind Canon".to_string(),
        String::new(),
        format!("_generated_at: {}_", generated_at.to_rfc3339()),
        String::new(),
        format!("active_entries: {}", active_entries.len()),
        format!("stale_entries: {}", stale_entries.len()),
        String::new(),
        "## Active canon".to_string(),
        String::new(),
    ];

    if active_entries.is_empty() {
        lines.push("(none)".to_string());
        lines.push(String::new());
    } else {
        for entry in active_entries {
            lines.push(format!("### {} r{}", entry.entry_id, entry.revision));
            if let Some(topic) = entry.topic.as_deref() {
                lines.push(format!("- topic: {topic}"));
            }
            lines.push(format!("- confidence_bps: {}", entry.confidence_bps));
            lines.push(format!("- freshness_score: {}", entry.freshness_score));
            if let Some(supersedes) = entry.supersedes_entry_id.as_deref() {
                lines.push(format!("- supersedes_entry_id: {supersedes}"));
            }
            if !entry.evidence_refs.is_empty() {
                lines.push(format!(
                    "- evidence_refs: {}",
                    entry.evidence_refs.join(", ")
                ));
            }
            lines.push(String::new());
            lines.push(entry.summary.trim().to_string());
            lines.push(String::new());
        }
    }

    lines.push("## Stale canon".to_string());
    lines.push(String::new());
    if stale_entries.is_empty() {
        lines.push("(none)".to_string());
        lines.push(String::new());
    } else {
        for entry in stale_entries {
            lines.push(format!("### {} r{}", entry.entry_id, entry.revision));
            if let Some(topic) = entry.topic.as_deref() {
                lines.push(format!("- topic: {topic}"));
            }
            lines.push(format!("- confidence_bps: {}", entry.confidence_bps));
            lines.push(format!("- freshness_score: {}", entry.freshness_score));
            if !entry.evidence_refs.is_empty() {
                lines.push(format!(
                    "- evidence_refs: {}",
                    entry.evidence_refs.join(", ")
                ));
            }
            lines.push(String::new());
            lines.push(entry.summary.trim().to_string());
            lines.push(String::new());
        }
    }

    lines.join("\n") + "\n"
}

fn write_handshake_export(
    store: &MindStore,
    project_root: &str,
    active_tag: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let scope = "project";
    let scope_key = t3_scope_id_for_project_root(project_root);

    let mut entries = Vec::new();
    if let Some(tag) = active_tag.filter(|value| !value.trim().is_empty()) {
        let mut tagged = store
            .active_canon_entries(Some(tag))
            .map_err(|err| format!("load tag-scoped canon entries failed: {err}"))?;
        entries.append(&mut tagged);
    }

    let all_active = store
        .active_canon_entries(None)
        .map_err(|err| format!("load active canon entries failed: {err}"))?;

    for entry in all_active {
        if entries
            .iter()
            .any(|existing: &aoc_storage::CanonEntryRevision| existing.entry_id == entry.entry_id)
        {
            continue;
        }
        entries.push(entry);
    }

    if entries.len() > MIND_T3_HANDSHAKE_MAX_ITEMS {
        entries.truncate(MIND_T3_HANDSHAKE_MAX_ITEMS);
    }

    let mut selected = Vec::new();
    let mut payload = render_handshake_markdown(&selected, active_tag, now);
    let mut token_estimate = estimate_text_tokens(&payload);

    for entry in entries {
        let mut candidate = selected.clone();
        candidate.push(entry);
        let candidate_payload = render_handshake_markdown(&candidate, active_tag, now);
        let candidate_tokens = estimate_text_tokens(&candidate_payload);
        if selected.is_empty() || candidate_tokens <= MIND_T3_HANDSHAKE_TOKEN_BUDGET {
            selected = candidate;
            payload = candidate_payload;
            token_estimate = candidate_tokens;
        } else {
            break;
        }
    }

    ensure_safe_export_text(&payload, "handshake export")?;

    let payload_hash = canonical_payload_hash(&payload)
        .map_err(|err| format!("handshake payload hash failed: {err}"))?;

    let _ = store
        .upsert_handshake_snapshot(
            scope,
            &scope_key,
            &payload,
            &payload_hash,
            token_estimate,
            now,
        )
        .map_err(|err| format!("persist handshake snapshot failed: {err}"))?;

    let export_path = PathBuf::from(project_root)
        .join(".aoc")
        .join("mind")
        .join("t3")
        .join("handshake.md");
    if let Some(parent) = export_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create handshake export directory failed: {err}"))?;
    }
    std::fs::write(&export_path, payload)
        .map_err(|err| format!("write handshake export failed: {err}"))?;

    Ok(())
}

fn render_handshake_markdown(
    entries: &[aoc_storage::CanonEntryRevision],
    active_tag: Option<&str>,
    generated_at: chrono::DateTime<chrono::Utc>,
) -> String {
    let mut lines = vec![
        "# Mind Handshake Baseline".to_string(),
        String::new(),
        "version: 1".to_string(),
        format!("generated_at: {}", generated_at.to_rfc3339()),
        format!(
            "active_tag: {}",
            active_tag
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("none")
        ),
        String::new(),
        "## Priority canon".to_string(),
        String::new(),
    ];

    if entries.is_empty() {
        lines.push("- (no active canon entries yet)".to_string());
        lines.push(String::new());
        return lines.join("\n") + "\n";
    }

    for entry in entries {
        let topic = entry.topic.as_deref().unwrap_or("global");
        let summary = truncate_chars(normalize_text(&entry.summary), 180);
        lines.push(format!(
            "- [{} r{}] topic={} confidence={} freshness={} :: {}",
            entry.entry_id,
            entry.revision,
            topic,
            entry.confidence_bps,
            entry.freshness_score,
            summary
        ));
    }
    lines.push(String::new());

    lines.join("\n") + "\n"
}

fn requeue_latest_t3_from_manifest(
    store: &MindStore,
    project_root: &str,
) -> Result<(String, bool), String> {
    let manifest = load_latest_session_export_manifest(project_root)?;
    let mut artifact_ids = manifest.t1_artifact_ids.clone();
    artifact_ids.extend(manifest.t2_artifact_ids.clone());
    artifact_ids.sort();
    artifact_ids.dedup();

    if artifact_ids.is_empty() {
        return Err("latest session export has no t1/t2 artifact ids".to_string());
    }

    let slice_start = manifest.slice_start_id.trim();
    let slice_end = manifest.slice_end_id.trim();
    let (job_id, inserted) = store
        .enqueue_t3_backlog_job(
            project_root,
            &manifest.session_id,
            &manifest.pane_id,
            manifest.active_tag.as_deref(),
            if slice_start.is_empty() {
                None
            } else {
                Some(slice_start)
            },
            if slice_end.is_empty() {
                None
            } else {
                Some(slice_end)
            },
            &artifact_ids,
            Utc::now(),
        )
        .map_err(|err| format!("enqueue t3 backlog job failed: {err}"))?;

    Ok((job_id, inserted))
}

fn load_latest_session_export_manifest(
    project_root: &str,
) -> Result<SessionExportManifest, String> {
    let insight_root = PathBuf::from(project_root)
        .join(".aoc")
        .join("mind")
        .join("insight");
    let entries = std::fs::read_dir(&insight_root)
        .map_err(|err| format!("read insight export dir failed: {err}"))?;

    let mut export_dirs = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    export_dirs.sort();

    let Some(latest_dir) = export_dirs.last() else {
        return Err("no session exports found to requeue".to_string());
    };

    let manifest_path = latest_dir.join("manifest.json");
    let payload = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("read latest manifest failed: {err}"))?;
    serde_json::from_str::<SessionExportManifest>(&payload)
        .map_err(|err| format!("parse latest manifest failed: {err}"))
}

fn t3_scope_id_for_project_root(project_root: &str) -> String {
    format!("project:{}", project_root)
}

fn map_observer_trigger(kind: aoc_mind::ObserverTriggerKind) -> MindObserverFeedTriggerKind {
    match kind {
        aoc_mind::ObserverTriggerKind::TokenThreshold => {
            MindObserverFeedTriggerKind::TokenThreshold
        }
        aoc_mind::ObserverTriggerKind::TaskCompleted => MindObserverFeedTriggerKind::TaskCompleted,
        aoc_mind::ObserverTriggerKind::ManualShortcut => {
            MindObserverFeedTriggerKind::ManualShortcut
        }
        aoc_mind::ObserverTriggerKind::Handoff => MindObserverFeedTriggerKind::Handoff,
        aoc_mind::ObserverTriggerKind::Compaction => MindObserverFeedTriggerKind::Compaction,
    }
}

fn estimate_compact_tokens(event: &aoc_storage::StoredCompactEvent) -> u32 {
    let text_tokens = event
        .text
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or_default();
    let tool_tokens = event
        .tool_meta
        .as_ref()
        .map(|meta| 14 + ((meta.output_bytes as u32) / 180))
        .unwrap_or_default();
    (text_tokens + tool_tokens).max(1)
}

fn estimate_text_tokens(text: &str) -> u32 {
    (text.chars().count() as u32 / 4).max(1)
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: String, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text;
    }
    let mut out = text
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn artifact_after_watermark(
    artifact: &StoredArtifact,
    watermark: Option<&ProjectWatermark>,
) -> bool {
    let Some(watermark) = watermark else {
        return true;
    };

    match (
        watermark.last_artifact_ts,
        watermark.last_artifact_id.as_ref(),
    ) {
        (Some(last_ts), Some(last_id)) => {
            artifact.ts > last_ts || (artifact.ts == last_ts && artifact.artifact_id > *last_id)
        }
        (Some(last_ts), None) => artifact.ts > last_ts,
        (None, Some(last_id)) => artifact.artifact_id > *last_id,
        (None, None) => true,
    }
}

fn render_artifact_markdown(kind: &str, artifacts: &[StoredArtifact]) -> String {
    let mut lines = vec![format!("# {} export", kind.to_uppercase())];

    if artifacts.is_empty() {
        lines.push("(empty)".to_string());
        return lines.join("\n") + "\n";
    }

    for artifact in artifacts {
        lines.push(format!(
            "## {} [{}] ({})",
            artifact.artifact_id,
            artifact.conversation_id,
            artifact.ts.to_rfc3339()
        ));
        lines.push(artifact.text.trim().to_string());
        lines.push(String::new());
    }

    lines.join("\n")
}

fn resolve_mind_finalize_drain_timeout_ms() -> i64 {
    resolve_non_negative_i64_env(
        "AOC_MIND_FINALIZE_TIMEOUT_MS",
        MIND_FINALIZE_DRAIN_TIMEOUT_MS,
    )
}

fn resolve_mind_idle_finalize_ms() -> i64 {
    resolve_non_negative_i64_env("AOC_MIND_IDLE_FINALIZE_MS", MIND_IDLE_FINALIZE_MS)
}

fn resolve_non_negative_i64_env(name: &str, default_value: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 0)
        .unwrap_or(default_value)
}

fn command_result(
    cfg: &ClientConfig,
    envelope: &PulseWireEnvelope,
    command: &str,
    status: &str,
    message: Option<String>,
    error: Option<PulseCommandError>,
    interrupt: bool,
    pulse_updates: Vec<PulseUpdate>,
) -> PulseCommandHandling {
    PulseCommandHandling {
        response: PulseWireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: cfg.session_id.clone(),
            sender_id: cfg.agent_key.clone(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: envelope.request_id.clone(),
            msg: WireMsg::CommandResult(PulseCommandResultPayload {
                command: command.to_string(),
                status: status.to_string(),
                message,
                error,
            }),
        },
        interrupt,
        pulse_updates,
    }
}

fn consultation_response(
    cfg: &ClientConfig,
    envelope: &PulseWireEnvelope,
    payload: PulseConsultationResponsePayload,
    pulse_updates: Vec<PulseUpdate>,
) -> PulseConsultationHandling {
    PulseConsultationHandling {
        response: PulseWireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: cfg.session_id.clone(),
            sender_id: cfg.agent_key.clone(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: envelope.request_id.clone(),
            msg: WireMsg::ConsultationResponse(payload),
        },
        pulse_updates,
    }
}

fn ensure_target_matches(
    cfg: &ClientConfig,
    target: Option<&str>,
) -> Result<(), PulseCommandError> {
    if target != Some(cfg.agent_key.as_str()) {
        return Err(PulseCommandError {
            code: "invalid_target".to_string(),
            message: "target_agent_id does not match publisher".to_string(),
        });
    }
    Ok(())
}

fn build_pulse_consultation_response(
    cfg: &ClientConfig,
    state: &PulseState,
    envelope: &PulseWireEnvelope,
) -> Option<PulseConsultationHandling> {
    let WireMsg::ConsultationRequest(payload) = envelope.msg.clone() else {
        return None;
    };

    if payload.target_agent_id != cfg.agent_key {
        return Some(consultation_response(
            cfg,
            envelope,
            PulseConsultationResponsePayload {
                consultation_id: payload.consultation_id,
                requesting_agent_id: payload.requesting_agent_id,
                responding_agent_id: cfg.agent_key.clone(),
                status: PulseConsultationStatus::Failed,
                packet: None,
                message: Some("target_agent_id does not match publisher".to_string()),
                error: Some(PulseCommandError {
                    code: "invalid_target".to_string(),
                    message: "target_agent_id does not match publisher".to_string(),
                }),
            },
            Vec::new(),
        ));
    }

    let received_at = Utc::now().to_rfc3339();
    let request_summary = payload.packet.summary.clone().or_else(|| {
        payload
            .packet
            .help_request
            .as_ref()
            .map(|request| request.question.clone())
    });
    let response_packet = synthesize_consultation_response_packet(cfg, state, &payload);
    let response_summary = response_packet.summary.clone();
    Some(consultation_response(
        cfg,
        envelope,
        PulseConsultationResponsePayload {
            consultation_id: payload.consultation_id.clone(),
            requesting_agent_id: payload.requesting_agent_id.clone(),
            responding_agent_id: cfg.agent_key.clone(),
            status: PulseConsultationStatus::Completed,
            packet: Some(response_packet),
            message: Some("consultation response ready".to_string()),
            error: None,
        },
        vec![
            PulseUpdate::ConsultationInbox(ConsultationInboxEntry {
                consultation_id: payload.consultation_id.clone(),
                requesting_agent_id: payload.requesting_agent_id.clone(),
                summary: request_summary,
                kind: payload.packet.kind,
                received_at: received_at.clone(),
            }),
            PulseUpdate::ConsultationOutbox(ConsultationOutboxEntry {
                consultation_id: payload.consultation_id,
                requesting_agent_id: payload.requesting_agent_id,
                responding_agent_id: cfg.agent_key.clone(),
                status: PulseConsultationStatus::Completed,
                summary: response_summary,
                responded_at: received_at,
            }),
            PulseUpdate::MindObserverEvent(mind_observer_event(
                MindObserverFeedStatus::Queued,
                MindObserverFeedTriggerKind::ManualShortcut,
                Some("consultation request handled".to_string()),
            )),
        ],
    ))
}

fn synthesize_consultation_response_packet(
    cfg: &ClientConfig,
    state: &PulseState,
    payload: &PulseConsultationRequestPayload,
) -> ConsultationPacket {
    let assignment = derive_worker_assignment(state);
    let worker_snapshot = build_overseer_worker_snapshot(cfg, state);
    let worker_status = map_lifecycle_to_worker_status(&state.lifecycle);
    let active_summary = state
        .snippet
        .clone()
        .or_else(|| payload.packet.task_context.focus_summary.clone())
        .or_else(|| {
            assignment
                .task_id
                .clone()
                .map(|task_id| format!("active task {task_id}"))
        })
        .unwrap_or_else(|| "current worker context available".to_string());

    ConsultationPacket {
        schema_version: 1,
        packet_id: format!(
            "consult-response:{}:{}",
            payload.consultation_id, cfg.agent_key
        ),
        kind: payload.packet.kind,
        identity: ConsultationIdentity {
            session_id: cfg.session_id.clone(),
            agent_id: cfg.agent_key.clone(),
            pane_id: Some(cfg.pane_id.clone()),
            conversation_id: None,
            role: Some("peer_worker".to_string()),
        },
        task_context: ConsultationTaskContext {
            active_tag: assignment.tag,
            task_ids: assignment.task_id.into_iter().collect(),
            focus_summary: Some(active_summary.clone()),
        },
        current_plan: payload
            .packet
            .current_plan
            .iter()
            .take(3)
            .cloned()
            .collect(),
        summary: Some(format!(
            "{} responding to {:?} consult: {}",
            cfg.agent_label, payload.packet.kind, active_summary
        )),
        blockers: if matches!(
            worker_status,
            WorkerStatus::Blocked | WorkerStatus::NeedsInput
        ) {
            vec![ConsultationBlocker {
                summary: worker_snapshot
                    .blocker
                    .or_else(|| state.snippet.clone())
                    .unwrap_or_else(|| "worker is blocked pending input".to_string()),
                severity: Some("warn".to_string()),
                kind: Some("worker_status".to_string()),
                evidence_refs: vec!["worker_snapshot.blocker".to_string()],
            }]
        } else {
            Vec::new()
        },
        checkpoint: None,
        artifact_refs: Vec::new(),
        evidence_refs: consultation_response_evidence_refs(cfg, state, payload),
        freshness: ConsultationFreshness {
            packet_generated_at: Some(Utc::now().to_rfc3339()),
            source_updated_at: state
                .updated_at_ms
                .map(|ms| {
                    Utc.timestamp_millis_opt(ms)
                        .single()
                        .map(|dt| dt.to_rfc3339())
                })
                .flatten(),
            stale_after_ms: worker_snapshot.stale_after_ms,
            source_status: ConsultationSourceStatus::Complete,
            degraded_inputs: Vec::new(),
        },
        confidence: ConsultationConfidence {
            overall_bps: Some(
                if matches!(
                    worker_status,
                    WorkerStatus::Blocked | WorkerStatus::NeedsInput
                ) {
                    550
                } else {
                    720
                },
            ),
            rationale: Some(
                "response synthesized from local worker state and bounded consultation packet"
                    .to_string(),
            ),
        },
        help_request: None,
        degraded_reason: None,
    }
    .normalize()
}

fn consultation_response_evidence_refs(
    cfg: &ClientConfig,
    state: &PulseState,
    payload: &PulseConsultationRequestPayload,
) -> Vec<ConsultationEvidenceRef> {
    let mut refs = Vec::new();
    refs.push(ConsultationEvidenceRef {
        reference: format!("agent:{}", cfg.agent_key),
        label: Some("responding worker".to_string()),
        path: None,
        relation: Some("responding_agent".to_string()),
    });
    if let Some(tag) = state
        .current_tag
        .as_ref()
        .map(|value| value.tag.trim())
        .filter(|v| !v.is_empty())
    {
        refs.push(ConsultationEvidenceRef {
            reference: format!("tag:{tag}"),
            label: Some("active tag".to_string()),
            path: None,
            relation: Some("active_tag".to_string()),
        });
    }
    if let Some(task_id) = derive_worker_assignment(state).task_id {
        refs.push(ConsultationEvidenceRef {
            reference: format!("task:{task_id}"),
            label: Some("active task".to_string()),
            path: None,
            relation: Some("active_task".to_string()),
        });
    }
    refs.extend(payload.packet.evidence_refs.iter().take(4).cloned());
    refs
}

fn build_pulse_command_response(
    cfg: &ClientConfig,
    envelope: &PulseWireEnvelope,
    mut mind_runtime: Option<&mut MindRuntime>,
) -> Option<PulseCommandHandling> {
    let WireMsg::Command(payload) = envelope.msg.clone() else {
        return None;
    };

    let command = payload.command.clone();

    match payload.command.as_str() {
        "stop_agent" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }

            let mut updates = Vec::new();
            if let Some(runtime) = mind_runtime.as_deref_mut() {
                let finalize = runtime.finalize_session(
                    cfg,
                    MindFinalizeTrigger::Shutdown,
                    Some("stop_agent requested".to_string()),
                );
                updates.extend(finalize.updates);
            }

            Some(command_result(
                cfg,
                envelope,
                &command,
                "ok",
                Some("stop signal dispatched".to_string()),
                None,
                true,
                updates,
            ))
        }
        "run_observer" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let reason = payload
                .args
                .as_object()
                .and_then(|args| args.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .map(|reason| reason.to_string())
                .unwrap_or_else(|| "manual observer trigger requested".to_string());

            let updates = if let Some(runtime) = mind_runtime.as_deref_mut() {
                if let Some(conversation_id) = runtime.resolve_conversation_id(&payload.args) {
                    runtime.enqueue_and_run(
                        cfg,
                        &conversation_id,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some(reason.clone()),
                    )
                } else {
                    vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Queued,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some(reason.clone()),
                    ))]
                }
            } else {
                vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                    MindObserverFeedStatus::Queued,
                    MindObserverFeedTriggerKind::ManualShortcut,
                    Some(reason.clone()),
                ))]
            };

            Some(command_result(
                cfg,
                envelope,
                &command,
                "ok",
                Some("observer trigger queued".to_string()),
                None,
                false,
                updates,
            ))
        }
        "mind_compaction_checkpoint" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let checkpoint = match serde_json::from_value::<MindCompactionCheckpointPayload>(
                payload.args.clone(),
            ) {
                Ok(value) => value,
                Err(err) => {
                    return Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "error",
                        Some("invalid compaction checkpoint payload".to_string()),
                        Some(PulseCommandError {
                            code: "mind_compaction_payload_invalid".to_string(),
                            message: err.to_string(),
                        }),
                        false,
                        Vec::new(),
                    ));
                }
            };

            match runtime.checkpoint_compaction(cfg, checkpoint) {
                Ok(updates) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "ok",
                    Some("compaction checkpoint queued".to_string()),
                    None,
                    false,
                    updates,
                )),
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("compaction checkpoint failed".to_string()),
                    Some(PulseCommandError {
                        code: "mind_compaction_checkpoint_failed".to_string(),
                        message: err.clone(),
                    }),
                    false,
                    vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Error,
                        MindObserverFeedTriggerKind::Compaction,
                        Some(format!("compaction checkpoint failed: {err}")),
                    ))],
                )),
            }
        }
        "mind_ingest_event" | "insight_ingest" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let ingest =
                match serde_json::from_value::<MindIngestEventPayload>(payload.args.clone()) {
                    Ok(event) => runtime.ingest_event(cfg, event),
                    Err(err) => Err(err.to_string()),
                };

            match ingest {
                Ok(progress) => {
                    let conversation_id = runtime
                        .latest_conversation_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let mut updates = vec![PulseUpdate::MindObserverEvent(MindObserverFeedEvent {
                        status: MindObserverFeedStatus::Queued,
                        trigger: MindObserverFeedTriggerKind::TokenThreshold,
                        conversation_id: Some(conversation_id.clone()),
                        runtime: None,
                        attempt_count: None,
                        latency_ms: None,
                        reason: Some("t0 updated".to_string()),
                        failure_kind: None,
                        enqueued_at: Some(Utc::now().to_rfc3339()),
                        started_at: None,
                        completed_at: None,
                        progress: Some(progress),
                    })];
                    updates.extend(runtime.maybe_run_token_threshold(cfg, &conversation_id));
                    Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "ok",
                        Some("mind event ingested".to_string()),
                        None,
                        false,
                        updates,
                    ))
                }
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind ingest failed".to_string()),
                    Some(PulseCommandError {
                        code: "mind_ingest_failed".to_string(),
                        message: err.to_string(),
                    }),
                    false,
                    vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Error,
                        MindObserverFeedTriggerKind::TokenThreshold,
                        Some(format!("mind ingest failed: {err}")),
                    ))],
                )),
            }
        }
        "mind_handoff" | "insight_handoff" | "mind_resume" | "insight_resume" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let reason = payload
                .args
                .as_object()
                .and_then(|args| args.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .map(|reason| reason.to_string())
                .unwrap_or_else(|| "stm handoff".to_string());

            let active_tag = payload
                .args
                .as_object()
                .and_then(|args| args.get("active_tag"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());

            let trigger = if payload.command.ends_with("_resume")
                || reason.to_ascii_lowercase().contains("resume")
            {
                MindInjectionTriggerKind::Resume
            } else {
                MindInjectionTriggerKind::Handoff
            };

            let Some(conversation_id) = runtime.resolve_conversation_id(&payload.args) else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("conversation unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "conversation_missing".to_string(),
                        message: "no conversation available for handoff observer trigger"
                            .to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let mut updates = runtime.enqueue_and_run(
                cfg,
                &conversation_id,
                MindObserverFeedTriggerKind::Handoff,
                Some(reason.clone()),
            );
            updates.push(PulseUpdate::MindInjection(build_mind_injection_payload(
                cfg,
                Some(&*runtime),
                trigger,
                active_tag.as_deref(),
                Some(reason.clone()),
            )));

            Some(command_result(
                cfg,
                envelope,
                &command,
                "ok",
                Some("handoff/resume observer trigger queued".to_string()),
                None,
                false,
                updates,
            ))
        }
        "mind_finalize" | "mind_finalize_session" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let reason = payload
                .args
                .as_object()
                .and_then(|args| args.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .map(|reason| reason.to_string());

            let finalize = runtime.finalize_session(cfg, MindFinalizeTrigger::Manual, reason);
            let status = finalize.status;
            Some(command_result(
                cfg,
                envelope,
                &command,
                status,
                Some(finalize.reason),
                if status == "ok" {
                    None
                } else {
                    Some(PulseCommandError {
                        code: "mind_finalize_failed".to_string(),
                        message: "session finalization failed".to_string(),
                    })
                },
                false,
                finalize.updates,
            ))
        }
        "mind_compaction_rebuild" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let reason = payload
                .args
                .as_object()
                .and_then(|args| args.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .unwrap_or("operator compaction rebuild request");

            let checkpoint = match runtime
                .store
                .latest_compaction_checkpoint_for_session(&cfg.session_id)
            {
                Ok(Some(checkpoint)) => checkpoint,
                Ok(None) => {
                    return Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "error",
                        Some("no compaction checkpoint found".to_string()),
                        Some(PulseCommandError {
                            code: "mind_compaction_checkpoint_missing".to_string(),
                            message: "no compaction checkpoint found for session".to_string(),
                        }),
                        false,
                        vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                            MindObserverFeedStatus::Error,
                            MindObserverFeedTriggerKind::ManualShortcut,
                            Some(
                                "compaction rebuild failed: no compaction checkpoint found"
                                    .to_string(),
                            ),
                        ))],
                    ));
                }
                Err(err) => {
                    return Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "error",
                        Some("compaction checkpoint lookup failed".to_string()),
                        Some(PulseCommandError {
                            code: "mind_compaction_checkpoint_lookup_failed".to_string(),
                            message: err.to_string(),
                        }),
                        false,
                        vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                            MindObserverFeedStatus::Error,
                            MindObserverFeedTriggerKind::ManualShortcut,
                            Some(format!("compaction rebuild failed: {err}")),
                        ))],
                    ));
                }
            };

            match rebuild_compaction_t0_slice_from_checkpoint(&runtime.store, &checkpoint) {
                Ok(Some(slice)) => {
                    if let Err(err) = runtime.store.upsert_compaction_t0_slice(&slice) {
                        return Some(command_result(
                            cfg,
                            envelope,
                            &command,
                            "error",
                            Some("compaction rebuild failed".to_string()),
                            Some(PulseCommandError {
                                code: "mind_compaction_rebuild_failed".to_string(),
                                message: err.to_string(),
                            }),
                            false,
                            vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                                MindObserverFeedStatus::Error,
                                MindObserverFeedTriggerKind::ManualShortcut,
                                Some(format!("compaction rebuild failed: {err}")),
                            ))],
                        ));
                    }
                    let mut updates = runtime.enqueue_and_run(
                        cfg,
                        &checkpoint.conversation_id,
                        MindObserverFeedTriggerKind::Compaction,
                        Some(format!(
                            "compaction rebuild requested ({reason}): {}",
                            checkpoint.checkpoint_id
                        )),
                    );
                    updates.insert(
                        0,
                        PulseUpdate::MindObserverEvent(mind_observer_event(
                            MindObserverFeedStatus::Success,
                            MindObserverFeedTriggerKind::ManualShortcut,
                            Some(format!(
                                "compaction slice rebuilt: {}",
                                checkpoint.checkpoint_id
                            )),
                        )),
                    );
                    Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "ok",
                        Some(format!(
                            "compaction rebuilt and requeued: {}",
                            checkpoint.checkpoint_id
                        )),
                        None,
                        false,
                        updates,
                    ))
                }
                Ok(None) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("compaction rebuild unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_compaction_rebuild_unavailable".to_string(),
                        message: "checkpoint marker provenance unavailable for rebuild".to_string(),
                    }),
                    false,
                    vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Error,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some(
                            "compaction rebuild unavailable: marker provenance missing".to_string(),
                        ),
                    ))],
                )),
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("compaction rebuild failed".to_string()),
                    Some(PulseCommandError {
                        code: "mind_compaction_rebuild_failed".to_string(),
                        message: err.clone(),
                    }),
                    false,
                    vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Error,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some(format!("compaction rebuild failed: {err}")),
                    ))],
                )),
            }
        }
        "mind_t3_requeue" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let reason = payload
                .args
                .as_object()
                .and_then(|args| args.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .unwrap_or("operator requeue request");

            match requeue_latest_t3_from_manifest(&runtime.store, &cfg.project_root) {
                Ok((job_id, inserted)) => {
                    let mut updates = vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Queued,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some(format!(
                            "t3 requeue requested ({reason}): {} ({})",
                            job_id,
                            if inserted { "inserted" } else { "existing" }
                        )),
                    ))];
                    runtime.insight_health.t3_queue_depth =
                        runtime.store.pending_t3_backlog_jobs().unwrap_or_default();
                    updates.push(PulseUpdate::InsightRuntime(runtime.insight_health.clone()));
                    Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "ok",
                        Some(format!(
                            "t3 requeue {} ({})",
                            job_id,
                            if inserted { "inserted" } else { "existing" }
                        )),
                        None,
                        false,
                        updates,
                    ))
                }
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("t3 requeue failed".to_string()),
                    Some(PulseCommandError {
                        code: "mind_t3_requeue_failed".to_string(),
                        message: err.clone(),
                    }),
                    false,
                    vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Error,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some(format!("t3 requeue failed: {err}")),
                    ))],
                )),
            }
        }
        "mind_handshake_rebuild" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let active_tag = payload
                .args
                .as_object()
                .and_then(|args| args.get("active_tag"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());

            match write_handshake_export(
                &runtime.store,
                &cfg.project_root,
                active_tag.as_deref(),
                Utc::now(),
            ) {
                Ok(()) => {
                    let mut updates = vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Success,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some("handshake baseline rebuilt".to_string()),
                    ))];
                    updates.push(PulseUpdate::MindInjection(build_mind_injection_payload(
                        cfg,
                        Some(&*runtime),
                        MindInjectionTriggerKind::Startup,
                        active_tag.as_deref(),
                        Some("handshake rebuild".to_string()),
                    )));
                    Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "ok",
                        Some("handshake baseline rebuilt".to_string()),
                        None,
                        false,
                        updates,
                    ))
                }
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("handshake rebuild failed".to_string()),
                    Some(PulseCommandError {
                        code: "mind_handshake_rebuild_failed".to_string(),
                        message: err.clone(),
                    }),
                    false,
                    vec![PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Error,
                        MindObserverFeedTriggerKind::ManualShortcut,
                        Some(format!("handshake rebuild failed: {err}")),
                    ))],
                )),
            }
        }
        "mind_context_pack" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }

            let mode = payload
                .args
                .as_object()
                .and_then(|args| args.get("mode"))
                .and_then(serde_json::Value::as_str)
                .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                    "startup" => MindContextPackMode::Startup,
                    "tag_switch" | "tag-switch" => MindContextPackMode::TagSwitch,
                    "resume" => MindContextPackMode::Resume,
                    "handoff" => MindContextPackMode::Handoff,
                    "dispatch" => MindContextPackMode::Dispatch,
                    _ => MindContextPackMode::Handoff,
                })
                .unwrap_or(MindContextPackMode::Handoff);
            let profile = if payload
                .args
                .as_object()
                .and_then(|args| args.get("detail"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                MindContextPackProfile::Expanded
            } else {
                MindContextPackProfile::Compact
            };
            let active_tag = payload
                .args
                .as_object()
                .and_then(|args| args.get("active_tag"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());
            let role = payload
                .args
                .as_object()
                .and_then(|args| args.get("role"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());
            let reason = payload
                .args
                .as_object()
                .and_then(|args| args.get("reason"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());

            match compile_mind_context_pack(
                cfg,
                mind_runtime.as_deref(),
                MindContextPackRequest {
                    mode,
                    profile,
                    active_tag,
                    reason,
                    role,
                },
                None,
            ) {
                Ok(pack) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "ok",
                    serde_json::to_string(&pack).ok(),
                    None,
                    false,
                    Vec::new(),
                )),
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind context-pack unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_context_pack_unavailable".to_string(),
                        message: err,
                    }),
                    false,
                    Vec::new(),
                )),
            }
        }
        "mind_provenance_query" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let parsed = MindProvenanceCommand::parse(&command, payload.args.clone());
            let export = match parsed {
                Ok(MindProvenanceCommand::Query(args)) => {
                    match compile_mind_provenance_export(&runtime.store, args) {
                        Ok(export) => export,
                        Err(err) => {
                            return Some(command_result(
                                cfg,
                                envelope,
                                &command,
                                "error",
                                Some("mind provenance unavailable".to_string()),
                                Some(PulseCommandError {
                                    code: "mind_provenance_unavailable".to_string(),
                                    message: err,
                                }),
                                false,
                                Vec::new(),
                            ))
                        }
                    }
                }
                Err(err) => {
                    return Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "error",
                        Some("invalid provenance command payload".to_string()),
                        Some(PulseCommandError {
                            code: "invalid_args".to_string(),
                            message: err,
                        }),
                        false,
                        Vec::new(),
                    ))
                }
            };

            match serde_json::to_string(&export) {
                Ok(json) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "ok",
                    Some(json),
                    None,
                    false,
                    Vec::new(),
                )),
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("failed to serialize provenance command result".to_string()),
                    Some(PulseCommandError {
                        code: "serialization_failed".to_string(),
                        message: err.to_string(),
                    }),
                    false,
                    Vec::new(),
                )),
            }
        }
        "insight_status"
        | "insight_dispatch"
        | "insight_bootstrap"
        | "insight_retrieve"
        | "insight_detached_dispatch"
        | "insight_detached_status"
        | "insight_detached_cancel" => {
            if let Err(error) = ensure_target_matches(cfg, payload.target_agent_id.as_deref()) {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("target mismatch".to_string()),
                    Some(error),
                    false,
                    Vec::new(),
                ));
            }
            let Some(runtime) = mind_runtime.as_deref_mut() else {
                return Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("mind runtime unavailable".to_string()),
                    Some(PulseCommandError {
                        code: "mind_unavailable".to_string(),
                        message: "mind runtime unavailable".to_string(),
                    }),
                    false,
                    Vec::new(),
                ));
            };

            let parsed = InsightCommand::parse(&command, payload.args.clone());
            let result_json = match parsed {
                Ok(InsightCommand::InsightStatus) => {
                    serde_json::to_string(&runtime.insight_status())
                }
                Ok(InsightCommand::InsightDispatch(args)) => {
                    serde_json::to_string(&runtime.insight_dispatch(args))
                }
                Ok(InsightCommand::InsightBootstrap(args)) => {
                    serde_json::to_string(&runtime.insight_bootstrap(args))
                }
                Ok(InsightCommand::InsightRetrieve(args)) => {
                    serde_json::to_string(&runtime.insight_retrieve(args))
                }
                Ok(InsightCommand::InsightDetachedDispatch(args)) => {
                    serde_json::to_string(&runtime.insight_detached_dispatch(args))
                }
                Ok(InsightCommand::InsightDetachedStatus(args)) => {
                    serde_json::to_string(&runtime.insight_detached_status(args))
                }
                Ok(InsightCommand::InsightDetachedCancel(args)) => {
                    serde_json::to_string(&runtime.insight_detached_cancel(args))
                }
                Err(err) => {
                    return Some(command_result(
                        cfg,
                        envelope,
                        &command,
                        "error",
                        Some("invalid insight command payload".to_string()),
                        Some(PulseCommandError {
                            code: "invalid_args".to_string(),
                            message: err,
                        }),
                        false,
                        Vec::new(),
                    ))
                }
            };

            match result_json {
                Ok(json) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "ok",
                    Some(json),
                    None,
                    false,
                    vec![
                        PulseUpdate::InsightRuntime(runtime.insight_health.clone()),
                        runtime.detached_status_update(),
                    ],
                )),
                Err(err) => Some(command_result(
                    cfg,
                    envelope,
                    &command,
                    "error",
                    Some("failed to serialize insight command result".to_string()),
                    Some(PulseCommandError {
                        code: "serialization_failed".to_string(),
                        message: err.to_string(),
                    }),
                    false,
                    Vec::new(),
                )),
            }
        }
        _ => Some(command_result(
            cfg,
            envelope,
            &command,
            "error",
            Some("unsupported command".to_string()),
            Some(PulseCommandError {
                code: "unsupported_command".to_string(),
                message: "unsupported command".to_string(),
            }),
            false,
            Vec::new(),
        )),
    }
}

fn trigger_self_interrupt() -> Result<(), String> {
    #[cfg(unix)]
    {
        send_unix_signal(std::process::id(), "INT").map_err(|err| err.to_string())
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

fn build_hello(cfg: &ClientConfig) -> String {
    let payload = HelloPayload {
        client_id: cfg.agent_key.clone(),
        role: "publisher".to_string(),
        capabilities: vec![
            "agent_status".to_string(),
            "diff_summary".to_string(),
            "diff_patch_response".to_string(),
            "task_summary".to_string(),
            "task_update".to_string(),
            "heartbeat".to_string(),
            "error".to_string(),
        ],
        agent_id: Some(cfg.agent_key.clone()),
        pane_id: Some(cfg.pane_id.clone()),
        project_root: Some(cfg.project_root.clone()),
    };
    build_envelope("hello", &cfg.session_id, &cfg.agent_key, payload, None)
}

fn build_agent_status(cfg: &ClientConfig, status: &str, message: Option<&str>) -> String {
    let redacted_message = message.map(redact_telemetry_text).and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let payload = AgentStatusPayload {
        agent_id: cfg.agent_key.clone(),
        status: status.to_string(),
        pane_id: cfg.pane_id.clone(),
        project_root: cfg.project_root.clone(),
        tab_scope: cfg.tab_scope.clone(),
        agent_label: Some(cfg.agent_label.clone()),
        cwd: env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string()),
        message: redacted_message,
    };
    build_envelope(
        "agent_status",
        &cfg.session_id,
        &cfg.agent_key,
        payload,
        None,
    )
}

fn build_heartbeat(cfg: &ClientConfig) -> String {
    let payload = HeartbeatPayload {
        agent_id: cfg.agent_key.clone(),
        pid: std::process::id() as i32,
        cwd: env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        last_update: Utc::now().to_rfc3339(),
        pane_id: Some(cfg.pane_id.clone()),
        project_root: Some(cfg.project_root.clone()),
        tab_scope: cfg.tab_scope.clone(),
    };
    build_envelope("heartbeat", &cfg.session_id, &cfg.agent_key, payload, None)
}

async fn handle_incoming(cfg: &ClientConfig, text: &str) -> Option<String> {
    let envelope: Envelope<serde_json::Value> = serde_json::from_str(text).ok()?;
    if envelope.session_id != cfg.session_id {
        return None;
    }
    if envelope.r#type != "diff_patch_request" {
        return None;
    }
    let payload: DiffPatchRequestPayload = serde_json::from_value(envelope.payload).ok()?;
    if payload.agent_id != cfg.agent_key {
        return None;
    }
    let request_id = payload
        .request_id
        .as_deref()
        .or(envelope.request_id.as_deref());
    let response = build_diff_patch_response(cfg, &payload).await;
    Some(build_envelope(
        "diff_patch_response",
        &cfg.session_id,
        &cfg.agent_key,
        response,
        request_id,
    ))
}

fn build_envelope<T: Serialize>(
    kind: &str,
    session_id: &str,
    sender_id: &str,
    payload: T,
    request_id: Option<&str>,
) -> String {
    let envelope = Envelope {
        version: PROTOCOL_VERSION.to_string(),
        r#type: kind.to_string(),
        session_id: session_id.to_string(),
        sender_id: sender_id.to_string(),
        timestamp: Utc::now().to_rfc3339(),
        payload,
        request_id: request_id.map(|id| id.to_string()),
    };
    serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string())
}

fn resolve_use_pty() -> bool {
    if let Ok(value) = env::var("AOC_AGENT_PTY") {
        if let Some(parsed) = parse_bool_env(&value) {
            return parsed;
        }
    }
    if let Ok(value) = env::var("AOC_PTY") {
        if let Some(parsed) = parse_bool_env(&value) {
            return parsed;
        }
    }
    false
}

fn parse_bool_env(value: &str) -> Option<bool> {
    match value.trim() {
        "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON" => Some(true),
        "0" | "false" | "FALSE" | "no" | "NO" | "off" | "OFF" => Some(false),
        _ => None,
    }
}

fn resolve_filter_mouse() -> bool {
    if let Ok(value) = env::var("AOC_AGENT_FILTER_MOUSE") {
        if let Some(parsed) = parse_bool_env(&value) {
            return parsed;
        }
    }
    true
}

fn disable_mouse_reporting_stdout() {
    let mut stdout = io::stdout();
    let _ = stdout.write_all(DISABLE_MOUSE_SEQ.as_bytes());
    let _ = stdout.flush();
}

fn filter_mouse_input(input: &[u8], carry: &mut Vec<u8>) -> (Vec<u8>, bool) {
    let mut data = Vec::with_capacity(carry.len() + input.len());
    if !carry.is_empty() {
        data.extend_from_slice(carry);
        carry.clear();
    }
    data.extend_from_slice(input);

    let mut output = Vec::with_capacity(data.len());
    let mut dropped = false;
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0x1b {
            if i + 1 >= data.len() {
                carry.extend_from_slice(&data[i..]);
                break;
            }
            if data[i + 1] != b'[' {
                output.push(data[i]);
                i += 1;
                continue;
            }
            if i + 2 >= data.len() {
                carry.extend_from_slice(&data[i..]);
                break;
            }
            if data[i + 2] != b'<' {
                output.push(data[i]);
                i += 1;
                continue;
            }
            let mut j = i + 3;
            let mut found = false;
            while j < data.len() {
                let byte = data[j];
                if byte == b'M' || byte == b'm' {
                    found = true;
                    j += 1;
                    break;
                }
                j += 1;
            }
            if found {
                dropped = true;
                i = j;
                continue;
            }
            carry.extend_from_slice(&data[i..]);
            break;
        }
        if data[i] == 0x1b && i + 2 < data.len() && data[i + 1] == b'[' && data[i + 2] == b'M' {
            if i + 5 <= data.len() {
                dropped = true;
                i += 5;
                continue;
            }
            carry.extend_from_slice(&data[i..]);
            break;
        }
        output.push(data[i]);
        i += 1;
    }

    (output, dropped)
}

fn filter_mouse_output(input: &[u8], carry: &mut Vec<u8>) -> (Vec<u8>, bool) {
    let mut data = Vec::with_capacity(carry.len() + input.len());
    if !carry.is_empty() {
        data.extend_from_slice(carry);
        carry.clear();
    }
    data.extend_from_slice(input);

    let mut output = Vec::with_capacity(data.len());
    let mut dropped = false;
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0x1b && i + 2 < data.len() && data[i + 1] == b'[' && data[i + 2] == b'?' {
            let mut j = i + 3;
            let mut numbers: Vec<u32> = Vec::new();
            let mut current: u32 = 0;
            let mut has_digit = false;
            let mut found = false;
            while j < data.len() {
                let byte = data[j];
                if byte.is_ascii_digit() {
                    has_digit = true;
                    current = current
                        .saturating_mul(10)
                        .saturating_add((byte - b'0') as u32);
                    j += 1;
                    continue;
                }
                if byte == b';' {
                    if has_digit {
                        numbers.push(current);
                        current = 0;
                        has_digit = false;
                    }
                    j += 1;
                    continue;
                }
                if byte == b'h' || byte == b'l' {
                    if has_digit {
                        numbers.push(current);
                    }
                    found = true;
                    j += 1;
                    break;
                }
                break;
            }
            if found {
                let mut is_mouse = false;
                for num in &numbers {
                    if matches!(num, 1000 | 1002 | 1003 | 1004 | 1005 | 1006 | 1007 | 1015) {
                        is_mouse = true;
                        break;
                    }
                }
                if is_mouse {
                    dropped = true;
                    i = j;
                    continue;
                }
            }
            if !found {
                carry.extend_from_slice(&data[i..]);
                break;
            }
        }
        output.push(data[i]);
        i += 1;
    }

    (output, dropped)
}

fn resolve_pty_size() -> PtySize {
    let cols = env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(80);
    let rows = env::var("LINES")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(24);
    PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

async fn tap_state_reporter_loop(
    cfg: ClientConfig,
    tx: mpsc::Sender<String>,
    cache: Arc<Mutex<CachedMessages>>,
    tap: Arc<StdMutex<TapBuffer>>,
    pulse_tx: mpsc::Sender<PulseUpdate>,
    task_context_refresh_tx: mpsc::UnboundedSender<()>,
) {
    let mut ticker = tokio::time::interval(Duration::from_millis(TAP_REPORT_INTERVAL_MS));
    let mut current = AgentLifecycle::Running;
    let mut pending: Option<(AgentLifecycle, u8)> = None;
    let mut last_parser_sample: Option<(AgentLifecycle, u8)> = None;
    let mut last_task_context_refresh: Option<Instant> = None;
    let mut last_version = 0u64;
    let mut last_hash = 0u64;
    let mut last_sent = Instant::now();

    loop {
        tokio::select! {
            _ = tx.closed() => break,
            _ = ticker.tick() => {
                let (version, bytes) = {
                    match tap.lock() {
                        Ok(tap) => tap.snapshot(),
                        Err(_) => continue,
                    }
                };

                let decoded = String::from_utf8_lossy(&bytes);
                let message = extract_significant_message(&decoded);
                let hash = stable_text_hash(&message);
                let (detected, confidence) = detect_lifecycle(&decoded);
                if last_parser_sample != Some((detected, confidence)) {
                    let (previous_state, previous_confidence) = last_parser_sample
                        .map(|(state, confidence)| (state.as_status(), confidence))
                        .unwrap_or(("unknown", 0));
                    info!(
                        event = "pulse_parser_confidence_transition",
                        agent_id = %cfg.agent_key,
                        previous_state,
                        previous_confidence,
                        next_state = detected.as_status(),
                        next_confidence = confidence
                    );
                    last_parser_sample = Some((detected, confidence));
                }

                let now = Instant::now();
                if detect_taskmaster_command(&decoded) {
                    let cooldown = Duration::from_millis(TASK_CONTEXT_COMMAND_DEBOUNCE_MS);
                    let allow_refresh = last_task_context_refresh
                        .map(|last| now.duration_since(last) >= cooldown)
                        .unwrap_or(true);
                    if allow_refresh {
                        let _ = task_context_refresh_tx.send(());
                        last_task_context_refresh = Some(now);
                    }
                }
                let changed = version != last_version || hash != last_hash;
                if !changed && now.duration_since(last_sent) < Duration::from_millis(TAP_RESEND_INTERVAL_MS) {
                    continue;
                }

                let mut should_emit = false;
                if detected != current {
                    if confidence >= 3 {
                        current = detected;
                        pending = None;
                        should_emit = true;
                    } else {
                        match pending {
                            Some((state, count)) if state == detected => {
                                if count + 1 >= 2 {
                                    current = detected;
                                    pending = None;
                                    should_emit = true;
                                } else {
                                    pending = Some((state, count + 1));
                                }
                            }
                            _ => {
                                pending = Some((detected, 1));
                            }
                        }
                    }
                } else {
                    pending = None;
                    if changed && now.duration_since(last_sent) >= Duration::from_millis(TAP_RESEND_INTERVAL_MS) {
                        should_emit = true;
                    }
                }

                if should_emit {
                    let lifecycle = current.as_status().to_string();
                    let snippet = if message.is_empty() {
                        None
                    } else {
                        Some(message.clone())
                    };
                    let status = build_agent_status(
                        &cfg,
                        lifecycle.as_str(),
                        snippet.as_deref(),
                    );
                    {
                        let mut cached = cache.lock().await;
                        cached.status = Some(status.clone());
                    }
                    if tx.send(status).await.is_err() {
                        break;
                    }
                    publish_pulse_update(
                        &pulse_tx,
                        PulseUpdate::Status {
                            lifecycle,
                            snippet,
                            parser_confidence: Some(confidence),
                        },
                    );
                    last_sent = now;
                }

                last_version = version;
                last_hash = hash;
            }
        }
    }
}

fn detect_lifecycle(input: &str) -> (AgentLifecycle, u8) {
    let normalized = strip_ansi(input).to_lowercase();
    let tail = normalized
        .lines()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .join(" ");

    let errors = [
        "error",
        "panic",
        "traceback",
        "exception",
        "fatal",
        "failed",
    ];
    if errors.iter().any(|word| tail.contains(word)) {
        return (AgentLifecycle::Error, 3);
    }

    let waiting = [
        "waiting for input",
        "press enter",
        "enter to continue",
        "[y/n]",
        "(y/n)",
        "needs input",
        "continue?",
    ];
    if waiting.iter().any(|word| tail.contains(word))
        || tail.trim_end().ends_with(':')
        || tail.trim_end().ends_with('>')
    {
        return (AgentLifecycle::NeedsInput, 3);
    }

    let busy = [
        "thinking",
        "analyzing",
        "processing",
        "generating",
        "loading",
        "running",
        "working",
        "compiling",
    ];
    if busy.iter().any(|word| tail.contains(word)) {
        return (AgentLifecycle::Busy, 2);
    }

    let idle = ["ready", "idle", "done", "complete", "finished", "waiting"];
    if idle.iter().any(|word| tail.contains(word)) {
        return (AgentLifecycle::Idle, 2);
    }

    (AgentLifecycle::Running, 1)
}

fn extract_significant_message(input: &str) -> String {
    let stripped = strip_ansi(input);
    for line in stripped.lines().rev() {
        let message = sanitize_activity_line(line);
        if !message.is_empty() {
            return message;
        }
    }
    String::new()
}

fn detect_taskmaster_command(input: &str) -> bool {
    if input.trim().is_empty() {
        return false;
    }
    let normalized = strip_ansi(input).to_ascii_lowercase();
    let tail = normalized
        .lines()
        .rev()
        .take(40)
        .collect::<Vec<_>>()
        .join(" ");
    taskmaster_command_regex().is_match(&tail)
}

fn taskmaster_command_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"\b(?:tm|aoc-task|aoc-cli\s+task)\b\s+(?:tag\b|list\b|show\b|add\b|edit\b|remove\b|rm\b|status\b|done\b|reopen\b|next\b|search\b|move\b|agent\b|sub\b|subtask\b|prd\b)",
        )
        .expect("valid taskmaster command detection regex")
    })
}

fn stable_text_hash(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

fn sanitize_activity_line(input: &str) -> String {
    let stripped = strip_ansi(input);
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return String::new();
    }
    let redacted = redact_telemetry_text(&collapsed);
    redacted.chars().take(STATUS_MESSAGE_MAX_CHARS).collect()
}

fn redact_telemetry_text(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let mut redacted = authorization_scheme_regex()
        .replace_all(input, |caps: &regex::Captures| {
            let prefix = if let Some(scheme) = caps.name("scheme") {
                format!("{} ", scheme.as_str())
            } else {
                String::new()
            };
            format!(
                "{}{}{}{REDACTED_SECRET}",
                &caps["key"], &caps["sep"], prefix
            )
        })
        .into_owned();

    redacted = telemetry_secret_kv_regex()
        .replace_all(&redacted, |caps: &regex::Captures| {
            format!("{}{}{}", &caps["key"], &caps["sep"], REDACTED_SECRET)
        })
        .into_owned();

    redacted = bearer_token_regex()
        .replace_all(&redacted, |caps: &regex::Captures| {
            format!("{} {}", &caps["scheme"], REDACTED_SECRET)
        })
        .into_owned();

    for pattern in inline_secret_regexes() {
        redacted = pattern.replace_all(&redacted, REDACTED_SECRET).into_owned();
    }

    redacted
}

fn telemetry_secret_kv_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        let key_pattern = telemetry_secret_key_pattern();
        Regex::new(&format!(
            r#"(?i)\b(?P<key>{key_pattern})\b(?P<sep>\s*[:=]\s*)(?P<value>"[^"]*"|'[^']*'|[^\s,;]+)"#
        ))
        .expect("valid telemetry secret key-value regex")
    })
}

fn authorization_scheme_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?P<key>authorization)\b(?P<sep>\s*:\s*)(?:(?P<scheme>bearer|token|basic)\s+)?[^\s,;]+",
        )
        .expect("valid authorization redaction regex")
    })
}

fn bearer_token_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(?P<scheme>bearer|token)\s+[A-Za-z0-9\-\._~\+/=]{12,}")
            .expect("valid bearer token regex")
    })
}

fn inline_secret_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES
        .get_or_init(|| {
            vec![
                Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{8,}\b").expect("valid GitHub token regex"),
                Regex::new(r"\bsk-[A-Za-z0-9]{12,}\b").expect("valid OpenAI-style key regex"),
                Regex::new(r"\bAKIA[0-9A-Z]{16}\b").expect("valid AWS access key regex"),
                Regex::new(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b")
                    .expect("valid JWT regex"),
            ]
        })
        .as_slice()
}

fn telemetry_secret_key_pattern() -> String {
    let mut keys: Vec<String> = TELEMETRY_SECRET_KEYS
        .iter()
        .map(|key| regex::escape(key))
        .collect();

    if let Ok(extra) = env::var("AOC_TELEMETRY_REDACT_KEYS") {
        for key in extra.split(',') {
            let normalized = key.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                continue;
            }
            keys.push(regex::escape(&normalized));
        }
    }

    keys.sort_unstable();
    keys.dedup();

    if keys.is_empty() {
        return "secret".to_string();
    }

    keys.join("|")
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_escape = false;
    for ch in input.chars() {
        if in_escape {
            if ('@'..='~').contains(&ch) {
                in_escape = false;
            }
            continue;
        }
        if ch == '\u{1b}' {
            in_escape = true;
            continue;
        }
        if ch.is_control() {
            continue;
        }
        output.push(ch);
    }
    output
}

async fn run_child_piped(cmd: &[String]) -> i32 {
    let mut child = Command::new(&cmd[0]);
    configure_mind_child_command_env(&mut child, std::iter::empty::<(&str, &str)>());
    child
        .args(&cmd[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = match child.spawn() {
        Ok(proc) => proc,
        Err(err) => {
            error!("failed to spawn child: {err}");
            return 1;
        }
    };
    let pid = child.id();

    let exit_code = tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            stop_tokio_child_with_escalation(&mut child, pid).await
        }
        status = child.wait() => {
            match status {
                Ok(status) => status.code().unwrap_or(0),
                Err(_) => 1,
            }
        }
    };

    exit_code
}

async fn run_child_pty(
    cmd: &[String],
    tap_buffer: Option<Arc<StdMutex<TapBuffer>>>,
) -> io::Result<i32> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(resolve_pty_size())
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;

    let mut builder = CommandBuilder::new(&cmd[0]);
    builder.args(&cmd[1..]);
    configure_mind_child_pty_env(&mut builder, std::iter::empty::<(&str, &str)>());
    if env::var("TERM").is_err() {
        builder.env("TERM", "xterm-256color");
    }

    let mut child = pair
        .slave
        .spawn_command(builder)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
    let writer =
        Arc::new(StdMutex::new(pair.master.take_writer().map_err(|err| {
            io::Error::new(io::ErrorKind::Other, err.to_string())
        })?));

    let filter_mouse = resolve_filter_mouse();
    if filter_mouse {
        disable_mouse_reporting_stdout();
    }
    let writer_clone = writer.clone();
    let stdin_task = tokio::task::spawn_blocking(move || {
        let mut stdin = io::stdin();
        let mut buffer = [0u8; 4096];
        let mut mouse_carry: Vec<u8> = Vec::new();
        loop {
            let read = match stdin.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(_) => break,
            };
            let payload: Vec<u8>;
            let mut dropped_mouse = false;
            let slice = &buffer[..read];
            let out = if filter_mouse {
                let (filtered, dropped) = filter_mouse_input(slice, &mut mouse_carry);
                payload = filtered;
                dropped_mouse = dropped;
                payload.as_slice()
            } else {
                slice
            };
            if out.is_empty() {
                continue;
            }
            if dropped_mouse {
                disable_mouse_reporting_stdout();
            }
            if let Ok(mut writer) = writer_clone.lock() {
                let _ = writer.write_all(out);
                let _ = writer.flush();
            }
        }
    });

    let stdout_task = tokio::task::spawn_blocking(move || {
        let mut stdout = io::stdout();
        let mut buffer = [0u8; 8192];
        let mut mouse_carry: Vec<u8> = Vec::new();
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(_) => break,
            };
            let (filtered, dropped_mouse) = if filter_mouse {
                let (filtered, dropped) = filter_mouse_output(&buffer[..read], &mut mouse_carry);
                (filtered, dropped)
            } else {
                (buffer[..read].to_vec(), false)
            };
            if dropped_mouse {
                disable_mouse_reporting_stdout();
            }
            if filtered.is_empty() {
                continue;
            }
            if let Some(tap) = &tap_buffer {
                if let Ok(mut tap) = tap.lock() {
                    tap.append(&filtered);
                }
            }
            let _ = stdout.write_all(&filtered);
            let _ = stdout.flush();
        }
    });

    let (exit_tx, mut exit_rx) = oneshot::channel::<i32>();
    let child_pid = child.process_id();
    std::thread::spawn(move || {
        let code = match child.wait() {
            Ok(status) => status.exit_code() as i32,
            Err(_) => 1,
        };
        let _ = exit_tx.send(code);
    });

    let exit_code = tokio::select! {
        status = &mut exit_rx => status.unwrap_or(1),
        _ = tokio::signal::ctrl_c() => {
            send_ctrl_c(&writer);
            wait_or_escalate_pty_exit(&mut exit_rx, child_pid).await
        }
    };

    let _ = stdout_task.await;
    stdin_task.abort();

    Ok(exit_code)
}

fn send_ctrl_c(writer: &Arc<StdMutex<Box<dyn Write + Send>>>) {
    if let Ok(mut writer) = writer.lock() {
        let _ = writer.write_all(&[0x03]);
        let _ = writer.flush();
    }
}

async fn wait_or_escalate_pty_exit(exit_rx: &mut oneshot::Receiver<i32>, pid: Option<u32>) -> i32 {
    let grace = Duration::from_millis(STOP_SIGINT_GRACE_MS);
    if let Ok(status) = tokio::time::timeout(grace, &mut *exit_rx).await {
        return status.unwrap_or(1);
    }

    if let Some(pid) = pid {
        let _ = send_unix_signal(pid, "TERM");
        if let Ok(status) =
            tokio::time::timeout(Duration::from_millis(STOP_TERM_GRACE_MS), &mut *exit_rx).await
        {
            return status.unwrap_or(1);
        }
        let _ = send_unix_signal(pid, "KILL");
    }

    (&mut *exit_rx).await.unwrap_or(1)
}

async fn stop_tokio_child_with_escalation(
    child: &mut tokio::process::Child,
    pid: Option<u32>,
) -> i32 {
    if let Some(pid) = pid {
        let _ = send_unix_signal(pid, "INT");
    }

    let grace = Duration::from_millis(STOP_SIGINT_GRACE_MS);
    if let Ok(status) = tokio::time::timeout(grace, child.wait()).await {
        return match status {
            Ok(wait) => wait.code().unwrap_or(0),
            Err(_) => 1,
        };
    }

    if let Some(pid) = pid {
        let _ = send_unix_signal(pid, "TERM");
        if let Ok(status) =
            tokio::time::timeout(Duration::from_millis(STOP_TERM_GRACE_MS), child.wait()).await
        {
            return match status {
                Ok(wait) => wait.code().unwrap_or(0),
                Err(_) => 1,
            };
        }
    }

    let _ = child.kill().await;
    match child.wait().await {
        Ok(wait) => wait.code().unwrap_or(1),
        Err(_) => 1,
    }
}

#[cfg(unix)]
fn send_unix_signal(pid: u32, signal: &str) -> io::Result<()> {
    let status = std::process::Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("kill -{signal} {pid} failed with status {status}"),
        ))
    }
}

#[cfg(not(unix))]
fn send_unix_signal(_pid: u32, _signal: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "signals unavailable",
    ))
}

async fn task_summary_loop(
    cfg: ClientConfig,
    tx: mpsc::Sender<String>,
    cache: Arc<Mutex<CachedMessages>>,
    pulse_tx: mpsc::Sender<PulseUpdate>,
    mut task_context_refresh_rx: mpsc::UnboundedReceiver<()>,
) {
    let tasks_path = tasks_file_path(&cfg.project_root);
    let state_path = state_file_path(&cfg.project_root);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<()>();
    let mut watcher =
        match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = event_tx.send(());
            }
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                warn!("task_watch_failed: {err}");
                return;
            }
        };
    watch_path(&mut watcher, &tasks_path);
    watch_path(&mut watcher, &state_path);
    let _ = send_task_summaries(&cfg, &tx, &cache, &pulse_tx).await;
    let mut pending = false;
    let debounce = Duration::from_millis(TASK_DEBOUNCE_MS);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(TASK_CONTEXT_HEARTBEAT_SECS));
    loop {
        tokio::select! {
            _ = tx.closed() => break,
            Some(_) = event_rx.recv() => {
                pending = true;
            }
            refresh = task_context_refresh_rx.recv() => {
                if refresh.is_some() {
                    pending = true;
                }
            }
            _ = tokio::time::sleep(debounce), if pending => {
                pending = false;
                let _ = send_task_summaries(&cfg, &tx, &cache, &pulse_tx).await;
            }
            _ = heartbeat.tick() => {
                let _ = send_task_summaries(&cfg, &tx, &cache, &pulse_tx).await;
            }
        }
    }
}

async fn send_task_summaries(
    cfg: &ClientConfig,
    tx: &mpsc::Sender<String>,
    cache: &Arc<Mutex<CachedMessages>>,
    pulse_tx: &mpsc::Sender<PulseUpdate>,
) -> Result<(), ()> {
    let tasks_path = tasks_file_path(&cfg.project_root);
    let state_path = state_file_path(&cfg.project_root);
    match load_tasks(&tasks_path).await {
        Ok(data) => {
            let mut tags: Vec<_> = data.tags.into_iter().collect();
            tags.sort_by(|a, b| a.0.cmp(&b.0));
            let mut messages: HashMap<String, String> = HashMap::new();
            let mut pulse_payloads: HashMap<String, TaskSummaryPayload> = HashMap::new();
            let mut tag_prd_paths: HashMap<String, String> = HashMap::new();
            for (tag, ctx) in tags {
                let payload = build_task_summary_payload(cfg, &tag, &ctx.tasks, None);
                if let Some(prd) = ctx.tag_prd() {
                    if !prd.path.trim().is_empty() {
                        tag_prd_paths.insert(tag.clone(), prd.path);
                    }
                }
                let msg = build_envelope(
                    "task_summary",
                    &cfg.session_id,
                    &cfg.agent_key,
                    payload.clone(),
                    None,
                );
                pulse_payloads.insert(tag.clone(), payload);
                messages.insert(tag, msg);
            }
            let current_tag =
                build_current_tag_payload(&pulse_payloads, &tag_prd_paths, &state_path).await;
            let mut cache = cache.lock().await;
            let mut to_send = Vec::new();
            for (tag, msg) in &messages {
                if cache.task_summary.get(tag).map(|value| value.as_str()) != Some(msg) {
                    to_send.push(msg.clone());
                }
            }
            let done_counts = pulse_payloads
                .iter()
                .map(|(tag, payload)| (tag.clone(), payload.counts.done))
                .collect::<HashMap<_, _>>();
            let mut completion_events = Vec::new();
            for (tag, next_done) in &done_counts {
                let Some(previous_done) = cache.task_done_counts.get(tag).copied() else {
                    continue;
                };
                if *next_done > previous_done {
                    completion_events.push((tag.clone(), previous_done, *next_done));
                }
            }
            let removed = cache
                .task_summary
                .keys()
                .any(|tag| !messages.contains_key(tag));
            let changed = removed || !to_send.is_empty();
            let current_tag_changed = cache.current_tag.as_ref() != Some(&current_tag);
            cache.task_summary = messages;
            cache.task_done_counts = done_counts;
            cache.current_tag = Some(current_tag.clone());
            drop(cache);
            for msg in to_send {
                let _ = tx.send(msg).await;
            }
            if changed {
                publish_pulse_update(pulse_tx, PulseUpdate::TaskSummaries(pulse_payloads));
            }
            if changed || current_tag_changed {
                publish_pulse_update(pulse_tx, PulseUpdate::CurrentTag(current_tag));
            }
            for (tag, previous_done, next_done) in completion_events {
                publish_pulse_update(
                    pulse_tx,
                    PulseUpdate::MindObserverEvent(mind_observer_event(
                        MindObserverFeedStatus::Queued,
                        MindObserverFeedTriggerKind::TaskCompleted,
                        Some(format!(
                            "task done count increased for tag {tag}: {previous_done} -> {next_done}"
                        )),
                    )),
                );
            }
            Ok(())
        }
        Err(err) => {
            let (code, message) = match err {
                TaskError::Missing => (
                    "tasks_missing".to_string(),
                    format!("tasks.json not found at {}", tasks_path.display()),
                ),
                TaskError::Malformed(msg) => ("tasks_malformed".to_string(), msg),
                TaskError::Io(msg) => ("tasks_error".to_string(), msg),
            };
            let error = PayloadError { code, message };
            let payload = build_task_summary_payload(cfg, "default", &[], Some(error));
            let msg = build_envelope(
                "task_summary",
                &cfg.session_id,
                &cfg.agent_key,
                payload.clone(),
                None,
            );
            let current_tag =
                build_current_tag_payload(&HashMap::new(), &HashMap::new(), &state_path).await;
            let mut cache = cache.lock().await;
            let should_send = cache
                .task_summary
                .get("default")
                .map(|value| value.as_str())
                != Some(&msg);
            let current_tag_changed = cache.current_tag.as_ref() != Some(&current_tag);
            cache.task_summary.clear();
            cache.task_done_counts.clear();
            cache
                .task_summary
                .insert("default".to_string(), msg.clone());
            cache.current_tag = Some(current_tag.clone());
            drop(cache);
            if should_send {
                let _ = tx.send(msg).await;
                let mut pulse_payloads = HashMap::new();
                pulse_payloads.insert("default".to_string(), payload);
                publish_pulse_update(pulse_tx, PulseUpdate::TaskSummaries(pulse_payloads));
            }
            if should_send || current_tag_changed {
                publish_pulse_update(pulse_tx, PulseUpdate::CurrentTag(current_tag));
            }
            Err(())
        }
    }
}

fn build_task_summary_payload(
    cfg: &ClientConfig,
    tag: &str,
    tasks: &[Task],
    error: Option<PayloadError>,
) -> TaskSummaryPayload {
    let mut counts = TaskCounts {
        total: tasks.len() as u32,
        ..TaskCounts::default()
    };
    let mut active_tasks = Vec::new();
    for task in tasks {
        match task.status {
            TaskStatus::Pending => counts.pending += 1,
            TaskStatus::InProgress => counts.in_progress += 1,
            TaskStatus::Blocked => counts.blocked += 1,
            TaskStatus::Done | TaskStatus::Cancelled => counts.done += 1,
            _ => {}
        }
        if task.active_agent {
            active_tasks.push(ActiveTask {
                id: task.id.clone(),
                title: task.title.clone(),
                status: task.status.as_str().to_string(),
                priority: task.priority.as_str().to_string(),
                active_agent: task.active_agent,
            });
        }
    }
    let active_tasks = if active_tasks.is_empty() {
        None
    } else {
        Some(active_tasks)
    };
    TaskSummaryPayload {
        agent_id: cfg.agent_key.clone(),
        tag: tag.to_string(),
        counts,
        active_tasks,
        error,
    }
}

async fn build_current_tag_payload(
    task_summaries: &HashMap<String, TaskSummaryPayload>,
    tag_prd_paths: &HashMap<String, String>,
    state_path: &Path,
) -> CurrentTagPayload {
    let tag = if let Some(tag) = env_override_tag() {
        tag
    } else if let Some(tag) = load_state_current_tag(state_path).await {
        tag
    } else {
        infer_default_task_tag(task_summaries)
    };

    let task_count = task_summaries
        .get(&tag)
        .map(|summary| summary.counts.total as usize)
        .unwrap_or(0);

    let prd_path = tag_prd_paths.get(&tag).cloned();

    CurrentTagPayload {
        tag,
        task_count,
        prd_path,
    }
}

fn env_override_tag() -> Option<String> {
    env::var("AOC_TASK_TAG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("TASKMASTER_TAG")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

fn infer_default_task_tag(task_summaries: &HashMap<String, TaskSummaryPayload>) -> String {
    if task_summaries.contains_key("master") {
        return "master".to_string();
    }

    let mut tags: Vec<_> = task_summaries.keys().cloned().collect();
    tags.sort();
    tags.into_iter()
        .next()
        .unwrap_or_else(|| "master".to_string())
}

async fn load_state_current_tag(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).await.ok()?;
    let state = serde_json::from_str::<TaskmasterStateFile>(&content).ok()?;
    state
        .current_tag
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
}

async fn diff_summary_loop(
    cfg: ClientConfig,
    tx: mpsc::Sender<String>,
    cache: Arc<Mutex<CachedMessages>>,
    pulse_tx: mpsc::Sender<PulseUpdate>,
) {
    let _ = send_diff_summary(&cfg, &tx, &cache, &pulse_tx).await;
    let mut ticker = tokio::time::interval(Duration::from_secs(DIFF_INTERVAL_SECS));
    loop {
        tokio::select! {
            _ = tx.closed() => break,
            _ = ticker.tick() => {
                let _ = send_diff_summary(&cfg, &tx, &cache, &pulse_tx).await;
            }
        }
    }
}

async fn send_diff_summary(
    cfg: &ClientConfig,
    tx: &mpsc::Sender<String>,
    cache: &Arc<Mutex<CachedMessages>>,
    pulse_tx: &mpsc::Sender<PulseUpdate>,
) -> Result<(), ()> {
    let payload = build_diff_summary_payload(cfg).await;
    let msg = build_envelope(
        "diff_summary",
        &cfg.session_id,
        &cfg.agent_key,
        payload.clone(),
        None,
    );
    let mut cached = cache.lock().await;
    if cached.diff_summary.as_ref().map(|value| value.as_str()) == Some(&msg) {
        return Ok(());
    }
    cached.diff_summary = Some(msg.clone());
    drop(cached);
    let _ = tx.send(msg).await;
    publish_pulse_update(pulse_tx, PulseUpdate::DiffSummary(payload));
    Ok(())
}

async fn build_diff_summary_payload(cfg: &ClientConfig) -> DiffSummaryPayload {
    let project_root = PathBuf::from(&cfg.project_root);
    match git_repo_root(&project_root).await {
        Ok(repo_root) => {
            if let Some(mut cached) = load_shared_diff_summary_payload(&repo_root) {
                cached.agent_id = cfg.agent_key.clone();
                cached.repo_root = repo_root.to_string_lossy().to_string();
                return cached;
            }

            let payload = match collect_git_summary(&repo_root).await {
                Ok((summary, files)) => DiffSummaryPayload {
                    agent_id: cfg.agent_key.clone(),
                    repo_root: repo_root.to_string_lossy().to_string(),
                    git_available: true,
                    summary,
                    files,
                    reason: None,
                },
                Err(_err) => DiffSummaryPayload {
                    agent_id: cfg.agent_key.clone(),
                    repo_root: repo_root.to_string_lossy().to_string(),
                    git_available: false,
                    summary: DiffSummaryCounts::default(),
                    files: Vec::new(),
                    reason: Some("error".to_string()),
                },
            };
            store_shared_diff_summary_payload(&repo_root, &payload);
            payload
        }
        Err(err) => {
            let reason = match err {
                GitError::Missing => "git_missing",
                GitError::NotRepo => "not_git_repo",
                GitError::Error(_) => "error",
            };
            DiffSummaryPayload {
                agent_id: cfg.agent_key.clone(),
                repo_root: cfg.project_root.clone(),
                git_available: false,
                summary: DiffSummaryCounts::default(),
                files: Vec::new(),
                reason: Some(reason.to_string()),
            }
        }
    }
}

async fn health_summary_loop(cfg: ClientConfig, pulse_tx: mpsc::Sender<PulseUpdate>) {
    let mut ticker = tokio::time::interval(Duration::from_secs(HEALTH_INTERVAL_SECS));
    let mut last_digest = String::new();
    loop {
        if pulse_tx.is_closed() {
            break;
        }
        let snapshot = build_health_snapshot(&cfg).await;
        let digest = serde_json::to_string(&snapshot).unwrap_or_default();
        if digest != last_digest {
            publish_pulse_update(&pulse_tx, PulseUpdate::Health(snapshot));
            last_digest = digest;
        }
        ticker.tick().await;
    }
}

async fn build_health_snapshot(cfg: &ClientConfig) -> HealthSnapshotPayload {
    let tasks_path = tasks_file_path(&cfg.project_root);
    let taskmaster_status = match load_tasks(&tasks_path).await {
        Ok(_) => "available",
        Err(TaskError::Missing) => "missing",
        Err(TaskError::Malformed(_)) => "malformed",
        Err(TaskError::Io(_)) => "error",
    }
    .to_string();
    let mut dependencies = vec![
        dependency_status("git"),
        dependency_status("zellij"),
        dependency_status("aoc-hub-rs"),
        dependency_status("aoc-agent-wrap-rs"),
    ];
    dependencies.push(dependency_status_any(
        "task-control",
        &["aoc-task", "tm", "aoc-taskmaster", "task-master"],
    ));
    let checks = ["test", "lint", "build"]
        .iter()
        .map(|kind| load_check_outcome(Path::new(&cfg.project_root), kind))
        .collect();
    HealthSnapshotPayload {
        dependencies,
        checks,
        taskmaster_status,
    }
}

fn dependency_status(name: &str) -> DependencyStatus {
    let path = resolve_binary_path(name);
    DependencyStatus {
        name: name.to_string(),
        available: path.is_some(),
        path,
    }
}

fn dependency_status_any(name: &str, candidates: &[&str]) -> DependencyStatus {
    for candidate in candidates {
        if let Some(path) = resolve_binary_path(candidate) {
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

fn resolve_binary_path(name: &str) -> Option<String> {
    let candidate = Path::new(name);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(name.to_string());
    }
    let path = env::var("PATH").ok()?;
    for segment in path.split(':') {
        if segment.is_empty() {
            continue;
        }
        let candidate = Path::new(segment).join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn load_check_outcome(project_root: &Path, kind: &str) -> CheckOutcome {
    let base = project_root.join(".aoc").join("state");
    let json_path = base.join(format!("last-{kind}.json"));
    if let Ok(contents) = std::fs::read_to_string(&json_path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) {
            let status = value
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let timestamp = value
                .get("timestamp")
                .and_then(serde_json::Value::as_str)
                .map(|v| v.to_string());
            let details = value
                .get("summary")
                .and_then(serde_json::Value::as_str)
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
    if let Ok(contents) = std::fs::read_to_string(&text_path) {
        let line = contents
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();
        return CheckOutcome {
            name: kind.to_string(),
            status: if line.is_empty() {
                "unknown".to_string()
            } else {
                line
            },
            timestamp: None,
            details: None,
        };
    }

    CheckOutcome {
        name: kind.to_string(),
        status: "unknown".to_string(),
        timestamp: None,
        details: None,
    }
}

fn shared_diff_cache_path(repo_root: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    repo_root.hash(&mut hasher);
    std::env::temp_dir().join(format!("aoc-diff-summary-{:016x}.json", hasher.finish()))
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn load_shared_diff_summary_payload(repo_root: &Path) -> Option<DiffSummaryPayload> {
    let cache_path = shared_diff_cache_path(repo_root);
    let raw = std::fs::read_to_string(cache_path).ok()?;
    let entry = serde_json::from_str::<SharedDiffSummaryCacheEntry>(&raw).ok()?;
    let age_ms = current_time_ms().saturating_sub(entry.saved_at_ms);
    if age_ms > DIFF_SHARED_CACHE_TTL_MS {
        return None;
    }
    Some(entry.payload)
}

fn store_shared_diff_summary_payload(repo_root: &Path, payload: &DiffSummaryPayload) {
    let cache_path = shared_diff_cache_path(repo_root);
    let temp_path = cache_path.with_extension("json.tmp");
    let entry = SharedDiffSummaryCacheEntry {
        saved_at_ms: current_time_ms(),
        payload: payload.clone(),
    };
    let Ok(raw) = serde_json::to_vec(&entry) else {
        return;
    };
    if std::fs::write(&temp_path, raw).is_ok() {
        let _ = std::fs::rename(temp_path, cache_path);
    }
}

async fn collect_git_summary(
    repo_root: &Path,
) -> Result<(DiffSummaryCounts, Vec<DiffFile>), String> {
    let staged_raw = run_git(repo_root, &["diff", "--numstat", "--cached"]).await?;
    let (staged_counts, staged_map) = parse_numstat(&staged_raw);
    let unstaged_raw = run_git(repo_root, &["diff", "--numstat"]).await?;
    let (unstaged_counts, unstaged_map) = parse_numstat(&unstaged_raw);
    let status_raw = run_git(repo_root, &["status", "--porcelain=v1", "-u"]).await?;
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
    if files.len() > MAX_FILES_LIST {
        files.truncate(MAX_FILES_LIST);
        warn!("diff_summary_truncated");
    }
    let untracked_count = files.iter().filter(|entry| entry.untracked).count() as u32;
    let summary = DiffSummaryCounts {
        staged: staged_counts,
        unstaged: unstaged_counts,
        untracked: UntrackedCounts {
            files: untracked_count,
        },
    };
    Ok((summary, files))
}

async fn build_diff_patch_response(
    cfg: &ClientConfig,
    request: &DiffPatchRequestPayload,
) -> DiffPatchResponsePayload {
    let context_lines = request.context_lines.unwrap_or(3).max(0) as u32;
    let include_untracked = request.include_untracked.unwrap_or(true);
    let project_root = PathBuf::from(&cfg.project_root);
    let repo_root = match git_repo_root(&project_root).await {
        Ok(root) => root,
        Err(err) => {
            let error = PayloadError {
                code: match err {
                    GitError::Missing => "git_missing",
                    GitError::NotRepo => "not_git_repo",
                    GitError::Error(_) => "error",
                }
                .to_string(),
                message: "unable to locate git repo".to_string(),
            };
            return DiffPatchResponsePayload {
                agent_id: cfg.agent_key.clone(),
                path: request.path.clone(),
                status: "modified".to_string(),
                is_binary: false,
                patch: None,
                error: Some(error),
            };
        }
    };

    let rel_path = normalize_rel_path(&repo_root, &request.path);
    let status_entry = match git_status_entry(&repo_root, &rel_path).await {
        Ok(entry) => entry,
        Err(_) => None,
    };
    let status = status_entry
        .as_ref()
        .map(|entry| entry.status.clone())
        .unwrap_or_else(|| "modified".to_string());
    let untracked = status_entry
        .as_ref()
        .map(|entry| entry.untracked)
        .unwrap_or(false);

    if untracked && !include_untracked {
        return DiffPatchResponsePayload {
            agent_id: cfg.agent_key.clone(),
            path: request.path.clone(),
            status,
            is_binary: false,
            patch: None,
            error: Some(PayloadError {
                code: "untracked_excluded".to_string(),
                message: "untracked file excluded".to_string(),
            }),
        };
    }

    let abs_path = if Path::new(&request.path).is_absolute() {
        PathBuf::from(&request.path)
    } else {
        repo_root.join(&request.path)
    };

    if untracked {
        return diff_patch_for_untracked(
            &cfg.agent_key,
            request,
            &abs_path,
            context_lines,
            &status,
        )
        .await;
    }

    diff_patch_for_tracked(
        &cfg.agent_key,
        request,
        &repo_root,
        &rel_path,
        context_lines,
        &status,
    )
    .await
}

async fn diff_patch_for_untracked(
    agent_id: &str,
    request: &DiffPatchRequestPayload,
    path: &Path,
    context_lines: u32,
    status: &str,
) -> DiffPatchResponsePayload {
    if !path.exists() {
        return DiffPatchResponsePayload {
            agent_id: agent_id.to_string(),
            path: request.path.clone(),
            status: status.to_string(),
            is_binary: false,
            patch: None,
            error: Some(PayloadError {
                code: "not_found".to_string(),
                message: "file does not exist".to_string(),
            }),
        };
    }

    if is_binary_file(path).await {
        return DiffPatchResponsePayload {
            agent_id: agent_id.to_string(),
            path: request.path.clone(),
            status: status.to_string(),
            is_binary: true,
            patch: None,
            error: Some(PayloadError {
                code: "patch_unavailable".to_string(),
                message: "binary file".to_string(),
            }),
        };
    }

    let path_str = path.to_string_lossy().to_string();
    let args = [
        "diff",
        "--no-index",
        &format!("--unified={context_lines}"),
        "--",
        "/dev/null",
        path_str.as_str(),
    ];
    let patch = match run_git(path.parent().unwrap_or(Path::new(".")), &args).await {
        Ok(output) => output,
        Err(err) => {
            return DiffPatchResponsePayload {
                agent_id: agent_id.to_string(),
                path: request.path.clone(),
                status: status.to_string(),
                is_binary: false,
                patch: None,
                error: Some(PayloadError {
                    code: "patch_unavailable".to_string(),
                    message: format!("failed to build patch: {err}"),
                }),
            };
        }
    };
    if patch.as_bytes().len() > MAX_PATCH_BYTES {
        return DiffPatchResponsePayload {
            agent_id: agent_id.to_string(),
            path: request.path.clone(),
            status: status.to_string(),
            is_binary: false,
            patch: None,
            error: Some(PayloadError {
                code: "patch_unavailable".to_string(),
                message: "patch too large".to_string(),
            }),
        };
    }
    DiffPatchResponsePayload {
        agent_id: agent_id.to_string(),
        path: request.path.clone(),
        status: status.to_string(),
        is_binary: false,
        patch: Some(patch),
        error: None,
    }
}

async fn diff_patch_for_tracked(
    agent_id: &str,
    request: &DiffPatchRequestPayload,
    repo_root: &Path,
    rel_path: &str,
    context_lines: u32,
    status: &str,
) -> DiffPatchResponsePayload {
    if is_binary_git(repo_root, rel_path).await {
        return DiffPatchResponsePayload {
            agent_id: agent_id.to_string(),
            path: request.path.clone(),
            status: status.to_string(),
            is_binary: true,
            patch: None,
            error: Some(PayloadError {
                code: "patch_unavailable".to_string(),
                message: "binary file".to_string(),
            }),
        };
    }

    let args = [
        "diff",
        &format!("--unified={context_lines}"),
        "HEAD",
        "--",
        rel_path,
    ];
    let patch = match run_git(repo_root, &args).await {
        Ok(output) => output,
        Err(err) => {
            return DiffPatchResponsePayload {
                agent_id: agent_id.to_string(),
                path: request.path.clone(),
                status: status.to_string(),
                is_binary: false,
                patch: None,
                error: Some(PayloadError {
                    code: "patch_unavailable".to_string(),
                    message: format!("failed to build patch: {err}"),
                }),
            };
        }
    };
    if patch.is_empty() {
        return DiffPatchResponsePayload {
            agent_id: agent_id.to_string(),
            path: request.path.clone(),
            status: status.to_string(),
            is_binary: false,
            patch: None,
            error: Some(PayloadError {
                code: "patch_unavailable".to_string(),
                message: "no changes".to_string(),
            }),
        };
    }
    if patch.as_bytes().len() > MAX_PATCH_BYTES {
        return DiffPatchResponsePayload {
            agent_id: agent_id.to_string(),
            path: request.path.clone(),
            status: status.to_string(),
            is_binary: false,
            patch: None,
            error: Some(PayloadError {
                code: "patch_unavailable".to_string(),
                message: "patch too large".to_string(),
            }),
        };
    }
    DiffPatchResponsePayload {
        agent_id: agent_id.to_string(),
        path: request.path.clone(),
        status: status.to_string(),
        is_binary: false,
        patch: Some(patch),
        error: None,
    }
}

async fn load_tasks(path: &Path) -> Result<ProjectData, TaskError> {
    match fs::read_to_string(path).await {
        Ok(contents) => {
            serde_json::from_str(&contents).map_err(|err| TaskError::Malformed(err.to_string()))
        }
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                Err(TaskError::Missing)
            } else {
                Err(TaskError::Io(err.to_string()))
            }
        }
    }
}

async fn git_repo_root(project_root: &Path) -> Result<PathBuf, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .await;
    let output = match output {
        Ok(value) => value,
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                return Err(GitError::Missing);
            }
            return Err(GitError::Error(()));
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("not a git repository") {
            return Err(GitError::NotRepo);
        }
        return Err(GitError::Error(()));
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err(GitError::Error(()));
    }
    Ok(PathBuf::from(root))
}

async fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_numstat(output: &str) -> (DiffCounts, HashMap<String, (u32, u32)>) {
    let mut counts = DiffCounts::default();
    let mut map = HashMap::new();
    for line in output.lines() {
        let mut parts = line.splitn(3, '\t');
        let additions = parts.next().unwrap_or("0");
        let deletions = parts.next().unwrap_or("0");
        let path = parts.next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        let add_count = additions.parse::<u32>().unwrap_or(0);
        let del_count = deletions.parse::<u32>().unwrap_or(0);
        counts.files += 1;
        counts.additions += add_count;
        counts.deletions += del_count;
        map.insert(path.to_string(), (add_count, del_count));
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

async fn git_status_entry(
    repo_root: &Path,
    path: &str,
) -> Result<Option<GitStatusEntry>, GitError> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-u", "--", path])
        .current_dir(repo_root)
        .output()
        .await;
    let output = match output {
        Ok(value) => value,
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                return Err(GitError::Missing);
            }
            return Err(GitError::Error(()));
        }
    };
    if !output.status.success() {
        return Err(GitError::Error(()));
    }
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(raw.lines().next().and_then(parse_status_line))
}

async fn is_binary_git(repo_root: &Path, path: &str) -> bool {
    let output = run_git(repo_root, &["diff", "--numstat", "HEAD", "--", path]).await;
    match output {
        Ok(text) => text
            .lines()
            .next()
            .and_then(|line| line.split('\t').next())
            .map(|first| first.trim() == "-")
            .unwrap_or(false),
        Err(_) => false,
    }
}

async fn is_binary_file(path: &Path) -> bool {
    let mut file = match fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut buffer = [0u8; 8192];
    let read = match file.read(&mut buffer).await {
        Ok(count) => count,
        Err(_) => return false,
    };
    buffer[..read].iter().any(|byte| *byte == 0)
}

fn normalize_rel_path(repo_root: &Path, input: &str) -> String {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        if let Ok(stripped) = path.strip_prefix(repo_root) {
            return stripped.to_string_lossy().to_string();
        }
    }
    input.to_string()
}

fn tasks_file_path(project_root: &str) -> PathBuf {
    PathBuf::from(project_root)
        .join(".taskmaster")
        .join("tasks")
        .join("tasks.json")
}

fn state_file_path(project_root: &str) -> PathBuf {
    PathBuf::from(project_root)
        .join(".taskmaster")
        .join("state.json")
}

fn watch_path(watcher: &mut RecommendedWatcher, path: &Path) {
    if let Some(parent) = path.parent() {
        if parent.exists() {
            if let Err(err) = watcher.watch(parent, RecursiveMode::NonRecursive) {
                warn!("watch_failed: {err}");
            }
        }
    }
    if path.exists() {
        if let Err(err) = watcher.watch(path, RecursiveMode::NonRecursive) {
            warn!("watch_failed: {err}");
        }
    }
}

fn init_logging(config: &RuntimeConfig) -> Option<LogGuard> {
    let level = if let Ok(level) = env::var("AOC_LOG_LEVEL") {
        level
    } else {
        "info".to_string()
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let writer = match open_log_file(
        &config.log_dir,
        &config.client.session_id,
        &config.client.agent_key,
    ) {
        Ok(log_guard) => log_guard,
        Err(err) => {
            eprintln!("log_file_error: {err}");
            LogGuard { file: None }
        }
    };
    let file = writer.file.clone();
    let stdout_enabled = config.log_stdout;
    let make_writer = BoxMakeWriter::new(move || MultiWriter::new(file.clone(), stdout_enabled));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(make_writer)
        .finish();
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        return None;
    }
    Some(writer)
}

impl MultiWriter {
    fn new(file: Option<Arc<StdMutex<std::fs::File>>>, stdout_enabled: bool) -> Self {
        Self {
            stdout_enabled,
            file,
        }
    }
}

impl Write for MultiWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.stdout_enabled {
            let _ = io::stdout().write_all(buf);
        }
        if let Some(file) = &self.file {
            let mut file = file.lock().unwrap();
            let _ = file.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.stdout_enabled {
            let _ = io::stdout().flush();
        }
        if let Some(file) = &self.file {
            let mut file = file.lock().unwrap();
            let _ = file.flush();
        }
        Ok(())
    }
}

fn open_log_file(log_dir: &str, session_id: &str, agent_id: &str) -> io::Result<LogGuard> {
    if log_dir.trim().is_empty() {
        return Ok(LogGuard { file: None });
    }
    let dir = PathBuf::from(log_dir);
    if std::fs::create_dir_all(&dir).is_err() {
        return Ok(LogGuard { file: None });
    }
    let safe_session = sanitize_component(session_id);
    let safe_agent = sanitize_component(agent_id);
    let path = dir.join(format!("aoc-agent-wrap-{safe_session}-{safe_agent}.log"));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(path)?;
    Ok(LogGuard {
        file: Some(Arc::new(StdMutex::new(file))),
    })
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

fn resolve_aoc_state_home_with_env(xdg_state_home: Option<&str>, home: Option<&str>) -> PathBuf {
    if let Some(value) = xdg_state_home
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        PathBuf::from(value)
    } else {
        PathBuf::from(home.unwrap_or(".")).join(".local/state")
    }
}

fn resolve_aoc_state_home() -> PathBuf {
    resolve_aoc_state_home_with_env(
        env::var("XDG_STATE_HOME").ok().as_deref(),
        env::var("HOME").ok().as_deref(),
    )
}

fn resolve_mind_runtime_root_with_env(
    project_root: &str,
    xdg_state_home: Option<&str>,
    home: Option<&str>,
) -> PathBuf {
    resolve_aoc_state_home_with_env(xdg_state_home, home)
        .join("aoc")
        .join("mind")
        .join("projects")
        .join(sanitize_component(project_root))
}

fn resolve_mind_runtime_root(project_root: &str) -> PathBuf {
    resolve_mind_runtime_root_with_env(
        project_root,
        env::var("XDG_STATE_HOME").ok().as_deref(),
        env::var("HOME").ok().as_deref(),
    )
}

fn runtime_snapshot_path(session_id: &str, pane_id: &str) -> PathBuf {
    resolve_aoc_state_home()
        .join("aoc")
        .join("telemetry")
        .join(sanitize_component(session_id))
        .join(format!("{}.json", sanitize_component(pane_id)))
}

fn mind_store_path_override() -> Option<String> {
    env::var("AOC_MIND_STORE_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn reflector_lock_path_override() -> Option<String> {
    env::var("AOC_REFLECTOR_LOCK_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn t3_lock_path_override() -> Option<String> {
    env::var("AOC_MIND_T3_LOCK_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_mind_store_path(cfg: &ClientConfig) -> PathBuf {
    resolve_mind_store_path_with_override(cfg, mind_store_path_override().as_deref())
}

fn resolve_mind_store_path_with_override(
    cfg: &ClientConfig,
    override_path: Option<&str>,
) -> PathBuf {
    if let Some(path) = override_path {
        return PathBuf::from(path);
    }

    resolve_mind_runtime_root(&cfg.project_root).join("project.sqlite")
}

fn resolve_legacy_mind_store_path(cfg: &ClientConfig) -> PathBuf {
    resolve_mind_runtime_root(&cfg.project_root)
        .join("legacy")
        .join(sanitize_component(&cfg.session_id))
        .join(format!("{}.sqlite", sanitize_component(&cfg.pane_id)))
}

fn resolve_reflector_lock_path(cfg: &ClientConfig) -> PathBuf {
    resolve_reflector_lock_path_with_override(cfg, reflector_lock_path_override().as_deref())
}

fn resolve_reflector_lock_path_with_override(
    cfg: &ClientConfig,
    override_path: Option<&str>,
) -> PathBuf {
    if let Some(path) = override_path {
        return PathBuf::from(path);
    }

    resolve_mind_runtime_root(&cfg.project_root)
        .join("locks")
        .join("reflector.lock")
}

fn resolve_t3_lock_path(cfg: &ClientConfig) -> PathBuf {
    resolve_t3_lock_path_with_override(cfg, t3_lock_path_override().as_deref())
}

fn resolve_t3_lock_path_with_override(cfg: &ClientConfig, override_path: Option<&str>) -> PathBuf {
    if let Some(path) = override_path {
        return PathBuf::from(path);
    }

    resolve_mind_runtime_root(&cfg.project_root)
        .join("locks")
        .join("t3.lock")
}

async fn persist_runtime_snapshot(cfg: &ClientConfig, status: &str) -> io::Result<()> {
    let snapshot = RuntimeSnapshot {
        session_id: cfg.session_id.clone(),
        pane_id: cfg.pane_id.clone(),
        agent_id: cfg.agent_key.clone(),
        agent_label: cfg.agent_label.clone(),
        project_root: cfg.project_root.clone(),
        tab_scope: cfg.tab_scope.clone(),
        pid: std::process::id() as i32,
        status: status.to_string(),
        last_update: Utc::now().to_rfc3339(),
    };
    let payload = serde_json::to_vec_pretty(&snapshot)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
    let path = runtime_snapshot_path(&cfg.session_id, &cfg.pane_id);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    fs::write(path, payload).await
}

fn resolve_session_id() -> String {
    if let Ok(value) = env::var("AOC_SESSION_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = env::var("ZELLIJ_SESSION_NAME") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    format!("pid-{}", std::process::id())
}

fn resolve_project_root(flag: &str) -> String {
    if !flag.trim().is_empty() {
        return flag.to_string();
    }
    if let Ok(value) = env::var("AOC_PROJECT_ROOT") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

fn resolve_agent_label(flag: &str, project_root: &str) -> String {
    if !flag.trim().is_empty() {
        return flag.to_string();
    }
    if let Ok(value) = env::var("AOC_AGENT_LABEL") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = env::var("AOC_AGENT_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    let base = PathBuf::from(project_root)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "".to_string());
    if !base.is_empty() {
        return base;
    }
    format!("pid-{}", std::process::id())
}

fn build_agent_key(session_id: &str, pane_id: &str) -> String {
    format!("{session_id}::{pane_id}")
}

fn resolve_pane_id(flag: &str) -> String {
    if !flag.trim().is_empty() {
        return flag.to_string();
    }
    if let Ok(value) = env::var("AOC_PANE_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = env::var("ZELLIJ_PANE_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    std::process::id().to_string()
}

fn resolve_tab_scope(flag: &str) -> Option<String> {
    if !flag.trim().is_empty() {
        return Some(flag.trim().to_string());
    }
    if let Ok(value) = env::var("AOC_TAB_SCOPE") {
        if !value.trim().is_empty() {
            return Some(value.trim().to_string());
        }
    }
    if let Ok(value) = env::var("AOC_TAB_NAME") {
        if !value.trim().is_empty() {
            return Some(value.trim().to_string());
        }
    }
    if let Ok(value) = env::var("ZELLIJ_TAB_NAME") {
        if !value.trim().is_empty() {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn resolve_hub_url(flag_url: &str, flag_addr: &str, session_id: &str) -> Url {
    if !flag_url.trim().is_empty() {
        return Url::parse(flag_url).expect("invalid hub url");
    }
    if let Ok(value) = env::var("AOC_HUB_URL") {
        if !value.trim().is_empty() {
            return Url::parse(&value).expect("invalid hub url");
        }
    }
    let addr = if !flag_addr.trim().is_empty() {
        flag_addr.to_string()
    } else if let Ok(value) = env::var("AOC_HUB_ADDR") {
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

fn resolve_pulse_socket_path(session_id: &str, path_flag: &str) -> PathBuf {
    if !path_flag.trim().is_empty() {
        return PathBuf::from(path_flag);
    }
    if let Ok(value) = env::var("AOC_PULSE_SOCK") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }

    let runtime_dir = if let Ok(value) = env::var("XDG_RUNTIME_DIR") {
        if !value.trim().is_empty() {
            PathBuf::from(value)
        } else {
            PathBuf::from("/tmp")
        }
    } else if let Ok(uid) = env::var("UID") {
        PathBuf::from(format!("/run/user/{uid}"))
    } else {
        PathBuf::from("/tmp")
    };
    runtime_dir
        .join("aoc")
        .join(session_slug(session_id))
        .join("pulse.sock")
}

fn resolve_pulse_vnext_enabled() -> bool {
    match env::var("AOC_PULSE_VNEXT_ENABLED") {
        Ok(value) => parse_bool_env(&value).unwrap_or(true),
        Err(_) => true,
    }
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

fn resolve_log_dir(flag: &str) -> String {
    if !flag.trim().is_empty() {
        return flag.to_string();
    }
    if let Ok(value) = env::var("AOC_LOG_DIR") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    ".aoc/logs".to_string()
}

fn resolve_log_stdout() -> bool {
    if let Ok(value) = env::var("AOC_LOG_STDOUT") {
        match value.trim() {
            "1" | "true" | "TRUE" | "yes" | "YES" => return true,
            "0" | "false" | "FALSE" | "no" | "NO" => return false,
            _ => {}
        }
    }
    false
}

fn next_backoff(current: Duration) -> Duration {
    let next = current + current;
    if next > Duration::from_secs(10) {
        Duration::from_secs(10)
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_storage::T3BacklogJobStatus;
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn test_client() -> ClientConfig {
        ClientConfig {
            session_id: "session-test".to_string(),
            agent_key: "session-test::12".to_string(),
            agent_label: "OpenCode".to_string(),
            pane_id: "12".to_string(),
            project_root: "/repo".to_string(),
            tab_scope: Some("agent".to_string()),
        }
    }

    fn test_client_with_root(project_root: &str) -> ClientConfig {
        let mut cfg = test_client();
        cfg.project_root = project_root.to_string();
        cfg
    }

    #[test]
    fn mind_paths_default_to_state_home_layout() {
        let cfg = test_client_with_root("/repo");
        let runtime_root = resolve_mind_runtime_root_with_env(
            &cfg.project_root,
            Some("/state-home"),
            Some("/home/test"),
        );

        assert_eq!(
            runtime_root,
            PathBuf::from("/state-home/aoc/mind/projects/_repo")
        );
        assert_eq!(
            resolve_aoc_state_home_with_env(Some("/state-home"), Some("/home/test")),
            PathBuf::from("/state-home")
        );
        assert_eq!(
            resolve_mind_store_path_with_override(&cfg, None),
            resolve_mind_runtime_root(&cfg.project_root).join("project.sqlite")
        );
        assert_eq!(
            resolve_reflector_lock_path_with_override(&cfg, None),
            resolve_mind_runtime_root(&cfg.project_root)
                .join("locks")
                .join("reflector.lock")
        );
        assert_eq!(
            resolve_legacy_mind_store_path(&cfg),
            resolve_mind_runtime_root(&cfg.project_root)
                .join("legacy")
                .join("session-test")
                .join("12.sqlite")
        );
        assert_eq!(
            resolve_t3_lock_path_with_override(&cfg, None),
            resolve_mind_runtime_root(&cfg.project_root)
                .join("locks")
                .join("t3.lock")
        );
        assert!(!resolve_mind_store_path_with_override(&cfg, None)
            .starts_with(PathBuf::from(&cfg.project_root).join(".aoc").join("mind")));
    }

    #[test]
    fn mind_paths_honor_explicit_overrides() {
        let cfg = test_client_with_root("/repo");

        assert_eq!(
            resolve_mind_store_path_with_override(&cfg, Some("/tmp/custom-mind.sqlite")),
            PathBuf::from("/tmp/custom-mind.sqlite")
        );
        assert_eq!(
            resolve_reflector_lock_path_with_override(&cfg, Some("/tmp/custom-reflector.lock")),
            PathBuf::from("/tmp/custom-reflector.lock")
        );
        assert_eq!(
            resolve_t3_lock_path_with_override(&cfg, Some("/tmp/custom-t3.lock")),
            PathBuf::from("/tmp/custom-t3.lock")
        );
    }

    fn command_envelope_for_test(
        cfg: &ClientConfig,
        request_id: &str,
        command: &str,
        args: serde_json::Value,
    ) -> PulseWireEnvelope {
        PulseWireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: cfg.session_id.clone(),
            sender_id: "aoc-mission-control".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some(request_id.to_string()),
            msg: WireMsg::Command(aoc_core::pulse_ipc::CommandPayload {
                command: command.to_string(),
                target_agent_id: Some(cfg.agent_key.clone()),
                args,
            }),
        }
    }

    fn consultation_envelope_for_test(
        cfg: &ClientConfig,
        request_id: &str,
        requester: &str,
    ) -> PulseWireEnvelope {
        PulseWireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: cfg.session_id.clone(),
            sender_id: "aoc-mission-control".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some(request_id.to_string()),
            msg: WireMsg::ConsultationRequest(PulseConsultationRequestPayload {
                consultation_id: request_id.to_string(),
                requesting_agent_id: requester.to_string(),
                target_agent_id: cfg.agent_key.clone(),
                packet: ConsultationPacket {
                    packet_id: format!("packet-{request_id}"),
                    kind: ConsultationPacketKind::Review,
                    identity: ConsultationIdentity {
                        session_id: cfg.session_id.clone(),
                        agent_id: requester.to_string(),
                        pane_id: Some("24".to_string()),
                        conversation_id: None,
                        role: Some("builder".to_string()),
                    },
                    summary: Some("Review the migration ordering".to_string()),
                    evidence_refs: vec![ConsultationEvidenceRef {
                        reference: "task:160".to_string(),
                        label: Some("worker task".to_string()),
                        path: None,
                        relation: Some("request_context".to_string()),
                    }],
                    ..Default::default()
                },
            }),
        }
    }

    fn seed_insight_assets(root: &Path) {
        std::fs::create_dir_all(root.join(".pi/agents")).expect("create agents dir");
        std::fs::create_dir_all(root.join("docs")).expect("create docs dir");
        std::fs::create_dir_all(root.join("crates/aoc-sample/src")).expect("create crates dir");

        std::fs::write(
            root.join(".pi/agents/insight-t1-observer.md"),
            "---\nname: insight-t1-observer\n---\n",
        )
        .expect("write t1");
        std::fs::write(
            root.join(".pi/agents/insight-t2-reflector.md"),
            "---\nname: insight-t2-reflector\n---\n",
        )
        .expect("write t2");
        std::fs::write(
            root.join(".pi/agents/teams.yaml"),
            "insight-core:\n  - insight-t1-observer\n  - insight-t2-reflector\n",
        )
        .expect("write teams");
        std::fs::write(
            root.join(".pi/agents/agent-chain.yaml"),
            "insight-handoff:\n  steps:\n    - agent: insight-t1-observer\n      prompt: \"$INPUT\"\n    - agent: insight-t2-reflector\n      prompt: \"$INPUT\"\n",
        )
        .expect("write chain");

        std::fs::write(root.join("docs/runtime-contract.md"), "# Runtime contract")
            .expect("write doc");
        std::fs::write(
            root.join("crates/aoc-sample/src/runtime.rs"),
            "pub fn runtime() {}",
        )
        .expect("write code");
    }

    fn test_context_pack_manifest(active_tag: Option<&str>) -> SessionExportManifest {
        SessionExportManifest {
            schema_version: 1,
            session_id: "session-test".to_string(),
            pane_id: "1".to_string(),
            project_root: "/repo".to_string(),
            active_tag: active_tag.map(|value| value.to_string()),
            conversation_ids: vec!["conv-1".to_string()],
            export_dir: "/tmp/context-pack-export".to_string(),
            t1_count: 1,
            t2_count: 1,
            t1_artifact_ids: vec!["t1-art-1".to_string()],
            t2_artifact_ids: vec!["t2-art-1".to_string()],
            slice_start_id: "slice-start".to_string(),
            slice_end_id: "slice-end".to_string(),
            slice_hash: "slice-hash".to_string(),
            exported_at: "2026-03-08T18:00:00Z".to_string(),
            last_artifact_ts: "2026-03-08T18:00:00Z".to_string(),
            watermark_scope: "project:/repo".to_string(),
            t3_job_id: "t3-job-1".to_string(),
            t3_job_inserted: true,
        }
    }

    #[test]
    fn mind_context_pack_compose_is_stable_and_respects_precedence() {
        let cfg = test_client_with_root("/repo");
        let overrides = MindContextPackSourceOverrides {
            aoc_mem: Some("mem-1\nmem-2".to_string()),
            aoc_stm_current: Some("stm-1".to_string()),
            handshake_markdown: Some("canon-1".to_string()),
            latest_export_manifest: Some(test_context_pack_manifest(None)),
            latest_t2_markdown: Some("t2-1".to_string()),
            latest_t1_markdown: Some("t1-1".to_string()),
            ..MindContextPackSourceOverrides::default()
        };

        let first = compile_mind_context_pack(
            &cfg,
            None,
            MindContextPackRequest {
                mode: MindContextPackMode::Startup,
                profile: MindContextPackProfile::Compact,
                active_tag: None,
                reason: Some("startup".to_string()),
                role: None,
            },
            Some(&overrides),
        )
        .expect("first pack");
        let second = compile_mind_context_pack(
            &cfg,
            None,
            MindContextPackRequest {
                mode: MindContextPackMode::Startup,
                profile: MindContextPackProfile::Compact,
                active_tag: None,
                reason: Some("startup".to_string()),
                role: None,
            },
            Some(&overrides),
        )
        .expect("second pack");

        assert_eq!(first.mode, MindContextPackMode::Startup);
        assert_eq!(first.rendered_lines, second.rendered_lines);
        assert_eq!(
            first.rendered_lines,
            vec![
                "[aoc_mem] AOC memory".to_string(),
                "mem-1".to_string(),
                "mem-2".to_string(),
                "[aoc_stm] AOC short-term memory".to_string(),
                "stm-1".to_string(),
                "[t3_handshake] Mind handshake canon".to_string(),
                "canon-1".to_string(),
                "[session_t2] Session reflections".to_string(),
                "t2-1".to_string(),
                "[session_t1] Session observations".to_string(),
                "t1-1".to_string(),
            ]
        );
        assert_eq!(first.citations.len(), 5);
    }

    #[test]
    fn mind_context_pack_applies_tag_filter_and_expansion() {
        let cfg = test_client_with_root("/repo");
        let overrides = MindContextPackSourceOverrides {
            aoc_mem: Some("mem-1".to_string()),
            aoc_stm_current: Some("stm-1".to_string()),
            handshake_markdown: Some("canon-1".to_string()),
            project_mind_markdown: Some(
                "## Active canon\n\n### entry-global r1\n- topic: global\nglobal summary\n\n### entry-mind r1\n- topic: mind\nmind summary\n\n### entry-ops r1\n- topic: ops\nops summary\n".to_string(),
            ),
            latest_export_manifest: Some(test_context_pack_manifest(Some("mind"))),
            latest_t2_markdown: Some("t2-mind".to_string()),
            latest_t1_markdown: Some("t1-mind".to_string()),
            ..MindContextPackSourceOverrides::default()
        };

        let pack = compile_mind_context_pack(
            &cfg,
            None,
            MindContextPackRequest {
                mode: MindContextPackMode::Handoff,
                profile: MindContextPackProfile::Expanded,
                active_tag: Some("mind".to_string()),
                reason: Some("handoff".to_string()),
                role: None,
            },
            Some(&overrides),
        )
        .expect("pack");

        let canon_section = pack
            .sections
            .iter()
            .find(|section| section.source_id == "t3_canon")
            .expect("canon section");
        assert!(canon_section
            .lines
            .iter()
            .any(|line| line.contains("entry-global")));
        assert!(canon_section
            .lines
            .iter()
            .any(|line| line.contains("entry-mind")));
        assert!(!canon_section
            .lines
            .iter()
            .any(|line| line.contains("entry-ops")));
        assert!(pack
            .sections
            .iter()
            .any(|section| section.source_id == "session_t2"));

        let filtered = compile_mind_context_pack(
            &cfg,
            None,
            MindContextPackRequest {
                mode: MindContextPackMode::Handoff,
                profile: MindContextPackProfile::Expanded,
                active_tag: Some("ops".to_string()),
                reason: Some("handoff".to_string()),
                role: None,
            },
            Some(&overrides),
        )
        .expect("filtered pack");
        assert!(!filtered
            .sections
            .iter()
            .any(|section| section.source_id == "session_t2" || section.source_id == "session_t1"));
    }

    #[test]
    fn pulse_mind_context_pack_command_supports_dispatch_role_slice() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-context-pack-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let export_root = test_root.join(".aoc/mind/insight/export-a");
        std::fs::create_dir_all(&export_root).expect("create export root");
        std::fs::write(export_root.join("t1.md"), "t1-dispatch").expect("write t1 dispatch");
        std::fs::write(export_root.join("t2.md"), "t2-dispatch").expect("write t2 dispatch");
        std::fs::create_dir_all(test_root.join(".aoc/mind/t3")).expect("create t3 dir");
        std::fs::write(
            test_root.join(".aoc/mind/t3/handshake.md"),
            "dispatch canon",
        )
        .expect("write handshake");
        let mut manifest = test_context_pack_manifest(Some("mind"));
        manifest.project_root = test_root.to_string_lossy().to_string();
        manifest.export_dir = export_root.to_string_lossy().to_string();
        std::fs::write(
            export_root.join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");

        let envelope = command_envelope_for_test(
            &cfg,
            "req-mind-context-pack",
            "mind_context_pack",
            serde_json::json!({
                "mode": "dispatch",
                "role": "insight-t1-observer",
                "active_tag": "mind",
                "reason": "specialist dispatch",
                "detail": true
            }),
        );
        let response = build_pulse_command_response(&cfg, &envelope, None).expect("response");
        let WireMsg::CommandResult(payload) = response.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");
        let pack: MindContextPack =
            serde_json::from_str(payload.message.as_deref().unwrap_or("{}")).expect("pack json");
        assert_eq!(pack.mode, MindContextPackMode::Dispatch);
        assert_eq!(pack.role.as_deref(), Some("insight-t1-observer"));
        assert!(pack
            .rendered_lines
            .iter()
            .any(|line| line.contains("dispatch canon")));
        assert!(pack
            .sections
            .iter()
            .any(|section| section.source_id == "session_t2"));

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn tap_buffer_keeps_bounded_tail() {
        let mut tap = TapBuffer::new(10);
        tap.append(b"12345");
        tap.append(b"67890");
        tap.append(b"abcdef");
        let (_version, bytes) = tap.snapshot();
        assert_eq!(String::from_utf8_lossy(&bytes), "7890abcdef");
    }

    #[test]
    fn sanitize_activity_line_redacts_common_secret_patterns() {
        let with_key = sanitize_activity_line("api_key=sk-verysecretvalue0000");
        assert_eq!(with_key, format!("api_key={REDACTED_SECRET}"));

        let with_auth =
            sanitize_activity_line("Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456");
        assert_eq!(
            with_auth,
            format!("Authorization: Bearer {REDACTED_SECRET}")
        );

        let inline = sanitize_activity_line("publishing token ghp_abcdEFGHijklMNOPqrstUVWX");
        assert_eq!(inline, format!("publishing token {REDACTED_SECRET}"));
    }

    #[test]
    fn build_agent_status_redacts_message_before_serialization() {
        let cfg = test_client();
        let status = build_agent_status(
            &cfg,
            "running",
            Some("deploying with access_token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abc.def"),
        );

        let envelope: Envelope<AgentStatusPayload> =
            serde_json::from_str(&status).expect("valid status envelope");
        assert_eq!(envelope.r#type, "agent_status");
        assert_eq!(
            envelope.payload.message.as_deref(),
            Some("deploying with access_token=[REDACTED]")
        );
    }

    #[test]
    fn lifecycle_detection_prefers_error_and_input() {
        let (state, confidence) = detect_lifecycle("Traceback: something blew up");
        assert_eq!(state, AgentLifecycle::Error);
        assert_eq!(confidence, 3);

        let (state, confidence) = detect_lifecycle("Waiting for input [y/n]");
        assert_eq!(state, AgentLifecycle::NeedsInput);
        assert_eq!(confidence, 3);
    }

    #[test]
    fn lifecycle_detection_marks_busy_and_idle() {
        let (state, confidence) = detect_lifecycle("Analyzing repository and generating patch");
        assert_eq!(state, AgentLifecycle::Busy);
        assert_eq!(confidence, 2);

        let (state, confidence) = detect_lifecycle("Done. Ready for next command.");
        assert_eq!(state, AgentLifecycle::Idle);
        assert_eq!(confidence, 2);
    }

    #[test]
    fn taskmaster_command_detection_finds_tm_invocations() {
        assert!(detect_taskmaster_command(
            "Running command: tm tag current --json"
        ));
        assert!(detect_taskmaster_command(
            "$ aoc-task status 91 in-progress --tag omo"
        ));
        assert!(detect_taskmaster_command("aoc-cli task list --tag omo"));
        assert!(!detect_taskmaster_command(
            "reviewing docs and planning next step"
        ));
    }

    #[test]
    fn pulse_source_includes_task_diff_health_and_mind_sections() {
        let cfg = test_client();
        let mut state = PulseState::new();
        state.lifecycle = "busy".to_string();
        state.snippet = Some("working".to_string());
        state.parser_confidence = Some(3);
        state.updated_at_ms = Some(1);

        state.task_summaries.insert(
            "master".to_string(),
            TaskSummaryPayload {
                agent_id: cfg.agent_key.clone(),
                tag: "master".to_string(),
                counts: TaskCounts {
                    total: 3,
                    pending: 1,
                    in_progress: 1,
                    done: 1,
                    blocked: 0,
                },
                active_tasks: None,
                error: None,
            },
        );
        state.current_tag = Some(CurrentTagPayload {
            tag: "master".to_string(),
            task_count: 3,
            prd_path: Some(".taskmaster/docs/prds/tag-master-prd.md".to_string()),
        });
        state.diff_summary = Some(DiffSummaryPayload {
            agent_id: cfg.agent_key.clone(),
            repo_root: "/repo".to_string(),
            git_available: true,
            summary: DiffSummaryCounts::default(),
            files: Vec::new(),
            reason: None,
        });
        state.health = Some(HealthSnapshotPayload {
            dependencies: vec![DependencyStatus {
                name: "git".to_string(),
                available: true,
                path: Some("/usr/bin/git".to_string()),
            }],
            checks: vec![CheckOutcome {
                name: "test".to_string(),
                status: "ok".to_string(),
                timestamp: Some("now".to_string()),
                details: Some("pass".to_string()),
            }],
            taskmaster_status: "available".to_string(),
        });
        state.insight_detached = Some(InsightDetachedStatusResult {
            status: "ok".to_string(),
            jobs: vec![aoc_core::insight_contracts::InsightDetachedJob {
                job_id: "detached-1".to_string(),
                parent_job_id: None,
                mode: aoc_core::insight_contracts::InsightDetachedMode::Dispatch,
                status: aoc_core::insight_contracts::InsightDetachedJobStatus::Running,
                agent: Some("insight-t1-observer".to_string()),
                team: None,
                chain: None,
                created_at_ms: 1,
                started_at_ms: Some(2),
                finished_at_ms: None,
                current_step_index: Some(1),
                step_count: Some(1),
                output_excerpt: Some("working detached".to_string()),
                stdout_excerpt: Some("working detached".to_string()),
                stderr_excerpt: None,
                error: None,
                fallback_used: false,
                step_results: vec![],
            }],
            active_jobs: 1,
            fallback_used: false,
        });
        state.mind_observer.events.push(mind_observer_event(
            MindObserverFeedStatus::Fallback,
            MindObserverFeedTriggerKind::TaskCompleted,
            Some("semantic observer failed (timeout)".to_string()),
        ));
        state.mind_injection = Some(MindInjectionPayload {
            status: "pending".to_string(),
            trigger: MindInjectionTriggerKind::Startup,
            scope: "project".to_string(),
            scope_key: t3_scope_id_for_project_root(&cfg.project_root),
            active_tag: Some("master".to_string()),
            reason: Some("session startup baseline handshake".to_string()),
            snapshot_id: Some("hs:test".to_string()),
            payload_hash: Some("hash:test".to_string()),
            token_estimate: Some(210),
            context_pack: None,
            queued_at: Utc::now().to_rfc3339(),
        });

        let source = build_pulse_source(&cfg, &state);
        let root = source.as_object().expect("source should be object");
        assert!(root.contains_key("task_summaries"));
        assert!(root.contains_key("task_summary"));
        assert!(root.contains_key("current_tag"));
        assert!(root.contains_key("diff_summary"));
        assert!(root.contains_key("health"));
        assert!(root.contains_key("insight_detached"));
        assert!(root.contains_key("mind_observer"));
        assert!(root.contains_key("mind_injection"));
        assert!(root.contains_key("worker_snapshot"));
        assert!(root.contains_key("session_overseer"));
        assert_eq!(
            root.get("tab_scope")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            "agent"
        );
        assert_eq!(
            root.get("parser_confidence")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            3
        );
    }

    #[test]
    fn pulse_source_includes_consultation_inbox_and_outbox_sections() {
        let cfg = test_client();
        let mut state = PulseState::new();
        state.consultation_inbox.push(ConsultationInboxEntry {
            consultation_id: "consult-1".to_string(),
            requesting_agent_id: "session-test::24".to_string(),
            summary: Some("Review the migration ordering".to_string()),
            kind: ConsultationPacketKind::Review,
            received_at: Utc::now().to_rfc3339(),
        });
        state.consultation_outbox.push(ConsultationOutboxEntry {
            consultation_id: "consult-1".to_string(),
            requesting_agent_id: "session-test::24".to_string(),
            responding_agent_id: cfg.agent_key.clone(),
            status: PulseConsultationStatus::Completed,
            summary: Some("Current worker context available".to_string()),
            responded_at: Utc::now().to_rfc3339(),
        });

        let source = build_pulse_source(&cfg, &state);
        let root = source.as_object().expect("source should be object");
        assert!(root.contains_key("consultation_inbox"));
        assert!(root.contains_key("consultation_outbox"));
    }

    #[test]
    fn build_pulse_consultation_response_records_inbox_and_outbox() {
        let cfg = test_client();
        let mut state = PulseState::new();
        state.lifecycle = "busy".to_string();
        state.snippet = Some("validating migration order".to_string());
        state.current_tag = Some(CurrentTagPayload {
            tag: "mind".to_string(),
            task_count: 2,
            prd_path: None,
        });
        let envelope = consultation_envelope_for_test(&cfg, "consult-2", "session-test::24");

        let response =
            build_pulse_consultation_response(&cfg, &state, &envelope).expect("response expected");
        let WireMsg::ConsultationResponse(payload) = response.response.msg else {
            panic!("expected consultation response")
        };
        assert_eq!(payload.status, PulseConsultationStatus::Completed);
        assert_eq!(payload.requesting_agent_id, "session-test::24");
        assert_eq!(payload.responding_agent_id, cfg.agent_key);
        assert_eq!(
            payload
                .packet
                .as_ref()
                .and_then(|packet| packet.identity.role.as_deref()),
            Some("peer_worker")
        );
        assert!(payload
            .packet
            .as_ref()
            .and_then(|packet| packet.summary.as_deref())
            .unwrap_or_default()
            .contains("responding to"));
        assert!(response
            .pulse_updates
            .iter()
            .any(|update| matches!(update, PulseUpdate::ConsultationInbox(_))));
        assert!(response
            .pulse_updates
            .iter()
            .any(|update| matches!(update, PulseUpdate::ConsultationOutbox(_))));
    }

    #[test]
    fn current_tag_transition_queues_tag_switch_injection_adapter_update() {
        let cfg = test_client();
        let mut state = PulseState::new();
        state.current_tag = Some(CurrentTagPayload {
            tag: "mind".to_string(),
            task_count: 3,
            prd_path: None,
        });

        apply_pulse_update_with_injection_adapters(
            &cfg,
            &mut state,
            None,
            PulseUpdate::CurrentTag(CurrentTagPayload {
                tag: "ops".to_string(),
                task_count: 2,
                prd_path: None,
            }),
        );

        let injection = state
            .mind_injection
            .as_ref()
            .expect("injection payload should be present");
        assert_eq!(injection.trigger, MindInjectionTriggerKind::TagSwitch);
        assert_eq!(injection.active_tag.as_deref(), Some("ops"));
        assert!(!state.observer_events.is_empty());
        assert_eq!(
            state.observer_events[0].kind,
            ObserverEventKind::ProgressUpdate
        );
    }

    #[test]
    fn task_summary_update_synthesizes_worker_snapshot_event() {
        let cfg = test_client();
        let mut state = PulseState::new();
        state.current_tag = Some(CurrentTagPayload {
            tag: "session-overseer".to_string(),
            task_count: 1,
            prd_path: None,
        });

        let mut payloads = HashMap::new();
        payloads.insert(
            "session-overseer".to_string(),
            TaskSummaryPayload {
                agent_id: cfg.agent_key.clone(),
                tag: "session-overseer".to_string(),
                counts: TaskCounts {
                    total: 1,
                    pending: 0,
                    in_progress: 1,
                    done: 0,
                    blocked: 0,
                },
                active_tasks: Some(vec![ActiveTask {
                    id: "149.2".to_string(),
                    title: "Implement worker progress emission".to_string(),
                    status: "in-progress".to_string(),
                    priority: "high".to_string(),
                    active_agent: true,
                }]),
                error: None,
            },
        );

        apply_pulse_update_with_injection_adapters(
            &cfg,
            &mut state,
            None,
            PulseUpdate::TaskSummaries(payloads),
        );

        let event = state
            .observer_events
            .first()
            .expect("observer event present");
        let snapshot = event.snapshot.as_ref().expect("snapshot present");
        assert_eq!(snapshot.assignment.task_id.as_deref(), Some("149.2"));
        assert_eq!(snapshot.assignment.tag.as_deref(), Some("session-overseer"));
        assert_eq!(snapshot.plan_alignment, PlanAlignment::High);
    }

    #[test]
    fn mind_injection_gate_skips_duplicate_payload_hash() {
        let now = Utc::now();
        let previous = MindInjectionPayload {
            status: "pending".to_string(),
            trigger: MindInjectionTriggerKind::Startup,
            scope: "project".to_string(),
            scope_key: "project:/repo".to_string(),
            active_tag: Some("mind".to_string()),
            reason: None,
            snapshot_id: Some("hs:1".to_string()),
            payload_hash: Some("hash:abc".to_string()),
            token_estimate: Some(210),
            context_pack: None,
            queued_at: now.to_rfc3339(),
        };
        let next = MindInjectionPayload {
            status: "pending".to_string(),
            trigger: MindInjectionTriggerKind::TagSwitch,
            scope: "project".to_string(),
            scope_key: "project:/repo".to_string(),
            active_tag: Some("ops".to_string()),
            reason: Some("tag changed".to_string()),
            snapshot_id: Some("hs:2".to_string()),
            payload_hash: Some("hash:abc".to_string()),
            token_estimate: Some(215),
            context_pack: None,
            queued_at: now.to_rfc3339(),
        };

        let gated = gate_mind_injection_payload(Some(&previous), next, now, 0);
        assert_eq!(gated.status, "skipped_duplicate");
        assert!(gated
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("duplicate payload hash"));
    }

    #[test]
    fn mind_injection_gate_applies_cooldown_for_non_urgent_updates() {
        let now = Utc::now();
        let previous = MindInjectionPayload {
            status: "pending".to_string(),
            trigger: MindInjectionTriggerKind::TagSwitch,
            scope: "project".to_string(),
            scope_key: "project:/repo".to_string(),
            active_tag: Some("mind".to_string()),
            reason: None,
            snapshot_id: Some("hs:1".to_string()),
            payload_hash: Some("hash:one".to_string()),
            token_estimate: Some(200),
            context_pack: None,
            queued_at: (now - chrono::Duration::milliseconds(200)).to_rfc3339(),
        };
        let next = MindInjectionPayload {
            status: "pending".to_string(),
            trigger: MindInjectionTriggerKind::TagSwitch,
            scope: "project".to_string(),
            scope_key: "project:/repo".to_string(),
            active_tag: Some("ops".to_string()),
            reason: Some("tag changed".to_string()),
            snapshot_id: Some("hs:2".to_string()),
            payload_hash: Some("hash:two".to_string()),
            token_estimate: Some(220),
            context_pack: None,
            queued_at: now.to_rfc3339(),
        };

        let gated = gate_mind_injection_payload(Some(&previous), next, now, 0);
        assert_eq!(gated.status, "skipped_cooldown");
    }

    #[test]
    fn mind_injection_gate_suppresses_non_urgent_updates_under_pressure() {
        let now = Utc::now();
        let next = MindInjectionPayload {
            status: "pending".to_string(),
            trigger: MindInjectionTriggerKind::TagSwitch,
            scope: "project".to_string(),
            scope_key: "project:/repo".to_string(),
            active_tag: Some("ops".to_string()),
            reason: Some("tag changed".to_string()),
            snapshot_id: Some("hs:2".to_string()),
            payload_hash: Some("hash:two".to_string()),
            token_estimate: Some(220),
            context_pack: None,
            queued_at: now.to_rfc3339(),
        };

        let gated = gate_mind_injection_payload(None, next, now, 85);
        assert_eq!(gated.status, "suppressed_pressure");
        assert!(gated
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("context pressure"));
    }

    #[test]
    fn pulse_stop_command_returns_ok_for_matching_target() {
        let cfg = test_client();
        let envelope = PulseWireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: cfg.session_id.clone(),
            sender_id: "aoc-mission-control".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some("req-1".to_string()),
            msg: WireMsg::Command(aoc_core::pulse_ipc::CommandPayload {
                command: "stop_agent".to_string(),
                target_agent_id: Some(cfg.agent_key.clone()),
                args: serde_json::json!({}),
            }),
        };

        let command =
            build_pulse_command_response(&cfg, &envelope, None).expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.command, "stop_agent");
        assert_eq!(payload.status, "ok");
        assert!(command.interrupt);
        assert!(command.pulse_updates.is_empty());
    }

    #[test]
    fn pulse_stop_command_rejects_target_mismatch() {
        let cfg = test_client();
        let envelope = PulseWireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: cfg.session_id.clone(),
            sender_id: "aoc-mission-control".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some("req-2".to_string()),
            msg: WireMsg::Command(aoc_core::pulse_ipc::CommandPayload {
                command: "stop_agent".to_string(),
                target_agent_id: Some("session-test::99".to_string()),
                args: serde_json::json!({}),
            }),
        };

        let command =
            build_pulse_command_response(&cfg, &envelope, None).expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "error");
        assert_eq!(
            payload.error.as_ref().map(|err| err.code.as_str()),
            Some("invalid_target")
        );
        assert!(!command.interrupt);
        assert!(command.pulse_updates.is_empty());
    }

    #[test]
    fn pulse_run_observer_command_enqueues_manual_feed_event() {
        let cfg = test_client();
        let envelope = PulseWireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: cfg.session_id.clone(),
            sender_id: "aoc-mission-control".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some("req-3".to_string()),
            msg: WireMsg::Command(aoc_core::pulse_ipc::CommandPayload {
                command: "run_observer".to_string(),
                target_agent_id: Some(cfg.agent_key.clone()),
                args: serde_json::json!({"reason": "pulse_user_request"}),
            }),
        };

        let command =
            build_pulse_command_response(&cfg, &envelope, None).expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");
        assert!(!command.interrupt);
        assert_eq!(command.pulse_updates.len(), 1);
        match &command.pulse_updates[0] {
            PulseUpdate::MindObserverEvent(event) => {
                assert_eq!(event.status, MindObserverFeedStatus::Queued);
                assert_eq!(event.trigger, MindObserverFeedTriggerKind::ManualShortcut);
                assert_eq!(event.reason.as_deref(), Some("pulse_user_request"));
            }
            _ => panic!("expected mind observer feed event"),
        }
    }

    #[test]
    fn pulse_mind_compaction_checkpoint_persists_marker_and_runs_observer() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-compact-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let git_init = std::process::Command::new("git")
            .arg("init")
            .arg(&test_root)
            .status()
            .expect("git init");
        assert!(git_init.success(), "git init should succeed");
        std::fs::write(test_root.join("trail.txt"), "new compaction trail\n")
            .expect("write untracked trail file");

        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let ingest = command_envelope_for_test(
            &cfg,
            "req-mind-ingest-compact",
            "mind_ingest_event",
            serde_json::json!({
                "conversation_id": "conv-compact",
                "event_id": "evt-compact-1",
                "timestamp_ms": 1700000000000i64,
                "body": {
                    "kind": "message",
                    "role": "user",
                    "text": "capture this context before compaction"
                }
            }),
        );
        let _ = build_pulse_command_response(&cfg, &ingest, Some(&mut runtime))
            .expect("ingest response");

        let before = runtime
            .store
            .raw_event_count("conv-compact")
            .expect("raw event count before");

        let checkpoint = command_envelope_for_test(
            &cfg,
            "req-mind-compact-checkpoint",
            "mind_compaction_checkpoint",
            serde_json::json!({
                "conversation_id": "conv-compact",
                "reason": "pi compaction",
                "summary": "Compacted earlier work into a durable summary.",
                "tokens_before": 12345,
                "first_kept_entry_id": "entry-42",
                "compaction_entry_id": "compact-1",
                "from_extension": true
            }),
        );

        let command = build_pulse_command_response(&cfg, &checkpoint, Some(&mut runtime))
            .expect("checkpoint response");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let after = runtime
            .store
            .raw_event_count("conv-compact")
            .expect("raw event count after");
        assert_eq!(after, before + 1, "expected raw compaction marker event");

        let checkpoint = runtime
            .store
            .latest_compaction_checkpoint_for_conversation("conv-compact")
            .expect("load compaction checkpoint")
            .expect("compaction checkpoint exists");
        assert_eq!(checkpoint.compaction_entry_id.as_deref(), Some("compact-1"));
        assert_eq!(checkpoint.first_kept_entry_id.as_deref(), Some("entry-42"));
        assert_eq!(checkpoint.tokens_before, Some(12345));
        assert_eq!(checkpoint.trigger_source, "pi_compact");

        let slice = runtime
            .store
            .latest_compaction_t0_slice_for_conversation("conv-compact")
            .expect("load compaction slice")
            .expect("compaction slice exists");
        assert_eq!(slice.compaction_entry_id.as_deref(), Some("compact-1"));
        assert_eq!(
            slice.checkpoint_id.as_deref(),
            Some(checkpoint.checkpoint_id.as_str())
        );
        assert!(slice.read_files.is_empty());
        assert!(slice.modified_files.iter().any(|path| path == "trail.txt"));

        let t1_artifacts = runtime
            .store
            .artifacts_for_conversation("conv-compact")
            .expect("load artifacts")
            .into_iter()
            .filter(|artifact| artifact.kind == "t1")
            .collect::<Vec<_>>();
        assert!(
            !t1_artifacts.is_empty(),
            "expected a t1 artifact after compaction"
        );
        assert!(t1_artifacts[0]
            .trace_ids
            .iter()
            .any(|trace_id| trace_id == &slice.slice_id));
        let linked_artifacts = runtime
            .store
            .artifacts_with_trace_id("conv-compact", &slice.slice_id)
            .expect("lookup artifacts by slice trace");
        assert!(linked_artifacts
            .iter()
            .any(|artifact| artifact.artifact_id == t1_artifacts[0].artifact_id));
        let links = runtime
            .store
            .artifact_file_links(&t1_artifacts[0].artifact_id)
            .expect("load artifact file links");
        assert!(
            links.iter().any(|link| {
                link.path == "trail.txt"
                    && link.relation == "modified"
                    && link.source == "pi_compaction_git_diff"
                    && link.untracked
            }),
            "expected compaction trail file link on t1 artifact"
        );

        let has_compaction_queue = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::Compaction
                        && event.status == MindObserverFeedStatus::Queued
            )
        });
        assert!(has_compaction_queue, "expected compaction queued event");

        let has_compaction_terminal = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::Compaction
                        && matches!(
                            event.status,
                            MindObserverFeedStatus::Success | MindObserverFeedStatus::Fallback
                        )
            )
        });
        assert!(
            has_compaction_terminal,
            "expected compaction observer completion event"
        );

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_compaction_checkpoint_is_idempotent_for_same_entry_id() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-compact-idempotent-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let checkpoint = || {
            command_envelope_for_test(
                &cfg,
                "req-mind-compact-idempotent",
                "mind_compaction_checkpoint",
                serde_json::json!({
                    "conversation_id": "conv-compact-idempotent",
                    "reason": "pi compaction",
                    "summary": "Compacted earlier work into a durable summary.",
                    "tokens_before": 12345,
                    "first_kept_entry_id": "entry-42",
                    "compaction_entry_id": "compact-fixed",
                    "from_extension": true
                }),
            )
        };

        let before = runtime
            .store
            .raw_event_count("conv-compact-idempotent")
            .expect("raw event count before");

        let first = build_pulse_command_response(&cfg, &checkpoint(), Some(&mut runtime))
            .expect("first checkpoint response");
        let second = build_pulse_command_response(&cfg, &checkpoint(), Some(&mut runtime))
            .expect("second checkpoint response");

        let after = runtime
            .store
            .raw_event_count("conv-compact-idempotent")
            .expect("raw event count after");
        assert_eq!(
            after,
            before + 1,
            "duplicate compaction markers should be ignored"
        );

        let checkpoints = runtime
            .store
            .compaction_checkpoints_for_conversation("conv-compact-idempotent")
            .expect("list compaction checkpoints");
        assert_eq!(checkpoints.len(), 1, "duplicate checkpoints should upsert");
        assert_eq!(
            checkpoints[0].compaction_entry_id.as_deref(),
            Some("compact-fixed")
        );

        let WireMsg::CommandResult(first_payload) = first.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(first_payload.status, "ok");

        let WireMsg::CommandResult(second_payload) = second.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(second_payload.status, "ok");

        let duplicate_reason_seen = second.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::Compaction
                        && event.reason.as_deref() == Some("pi compaction")
            )
        });
        assert!(
            duplicate_reason_seen,
            "expected observer updates for duplicate checkpoint command"
        );

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_compaction_checkpoint_slice_rebuild_is_deterministic_from_checkpoint_marker() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-compact-rebuild-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        std::process::Command::new("git")
            .arg("init")
            .current_dir(&test_root)
            .output()
            .expect("git init");
        std::fs::write(test_root.join("trail.txt"), "changed\n").expect("write trail file");

        let checkpoint = command_envelope_for_test(
            &cfg,
            "req-mind-compact-rebuild",
            "mind_compaction_checkpoint",
            serde_json::json!({
                "conversation_id": "conv-compact-rebuild",
                "reason": "pi compaction",
                "summary": "Compacted earlier work into a durable summary.",
                "tokens_before": 555,
                "first_kept_entry_id": "entry-99",
                "compaction_entry_id": "compact-rebuild",
                "from_extension": true
            }),
        );

        build_pulse_command_response(&cfg, &checkpoint, Some(&mut runtime))
            .expect("checkpoint response");

        let stored_checkpoint = runtime
            .store
            .latest_compaction_checkpoint_for_conversation("conv-compact-rebuild")
            .expect("load checkpoint")
            .expect("checkpoint exists");
        let stored_slice = runtime
            .store
            .compaction_t0_slice_for_checkpoint(&stored_checkpoint.checkpoint_id)
            .expect("slice by checkpoint")
            .expect("stored slice exists");
        let rebuilt =
            rebuild_compaction_t0_slice_from_checkpoint(&runtime.store, &stored_checkpoint)
                .expect("rebuild call")
                .expect("rebuilt slice exists");

        assert_eq!(rebuilt.slice_id, stored_slice.slice_id);
        assert_eq!(rebuilt.slice_hash, stored_slice.slice_hash);
        assert_eq!(rebuilt.checkpoint_id, stored_slice.checkpoint_id);
        assert_eq!(rebuilt.modified_files, stored_slice.modified_files);
        assert!(rebuilt
            .modified_files
            .iter()
            .any(|path| path == "trail.txt"));

        let marker_event = runtime
            .store
            .raw_event_by_id(
                stored_checkpoint
                    .marker_event_id
                    .as_deref()
                    .expect("marker event id"),
            )
            .expect("marker lookup")
            .expect("marker exists");
        let marker_modified_files = marker_event
            .attrs
            .get("mind_compaction_modified_files")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(marker_modified_files
            .iter()
            .any(|value| value.as_str() == Some("trail.txt")));

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_ingest_event_updates_progress() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let envelope = command_envelope_for_test(
            &cfg,
            "req-mind-ingest",
            "mind_ingest_event",
            serde_json::json!({
                "conversation_id": "conv-1",
                "event_id": "evt-1",
                "timestamp_ms": 1700000000000i64,
                "body": {
                    "kind": "message",
                    "role": "user",
                    "text": "plan implementation details for t0 runtime"
                }
            }),
        );

        let command = build_pulse_command_response(&cfg, &envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");
        assert!(!command.pulse_updates.is_empty());
        let has_progress = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::TokenThreshold
                        && event.progress.is_some()
            )
        });
        assert!(has_progress, "expected token-threshold progress update");

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_handoff_runs_observer_flow() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let ingest = command_envelope_for_test(
            &cfg,
            "req-mind-ingest-2",
            "mind_ingest_event",
            serde_json::json!({
                "conversation_id": "conv-2",
                "event_id": "evt-2",
                "timestamp_ms": 1700000001000i64,
                "body": {
                    "kind": "message",
                    "role": "user",
                    "text": "handoff notes ready for implementation"
                }
            }),
        );
        let _ = build_pulse_command_response(&cfg, &ingest, Some(&mut runtime))
            .expect("ingest response");

        let handoff = command_envelope_for_test(
            &cfg,
            "req-mind-handoff",
            "mind_handoff",
            serde_json::json!({
                "conversation_id": "conv-2",
                "reason": "stm handoff"
            }),
        );

        let command = build_pulse_command_response(&cfg, &handoff, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let has_handoff_queue = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::Handoff
                        && event.status == MindObserverFeedStatus::Queued
            )
        });
        assert!(has_handoff_queue, "expected handoff queued event");

        let has_terminal = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::Handoff
                        && matches!(
                            event.status,
                            MindObserverFeedStatus::Success | MindObserverFeedStatus::Fallback
                        )
            )
        });
        assert!(has_terminal, "expected handoff terminal observer event");

        let has_injection = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindInjection(payload)
                    if payload.trigger == MindInjectionTriggerKind::Handoff
                        && payload.status == "pending"
            )
        });
        assert!(has_injection, "expected handoff injection adapter update");

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_resume_queues_resume_injection_adapter_update() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-resume-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let ingest = command_envelope_for_test(
            &cfg,
            "req-mind-ingest-resume",
            "mind_ingest_event",
            serde_json::json!({
                "conversation_id": "conv-resume",
                "event_id": "evt-resume-1",
                "timestamp_ms": 1700000001500i64,
                "body": {
                    "kind": "message",
                    "role": "user",
                    "text": "resume context and continue"
                }
            }),
        );
        let _ = build_pulse_command_response(&cfg, &ingest, Some(&mut runtime))
            .expect("ingest response");

        let resume = command_envelope_for_test(
            &cfg,
            "req-mind-resume",
            "mind_resume",
            serde_json::json!({
                "conversation_id": "conv-resume",
                "reason": "aoc-stm resume"
            }),
        );

        let command = build_pulse_command_response(&cfg, &resume, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let has_resume_injection = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindInjection(payload)
                    if payload.trigger == MindInjectionTriggerKind::Resume
                        && payload.status == "pending"
            )
        });
        assert!(
            has_resume_injection,
            "expected resume injection adapter update"
        );

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_finalize_writes_export_bundle_and_enqueues_t3_job() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-finalize-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let ingest = command_envelope_for_test(
            &cfg,
            "req-mind-ingest-finalize",
            "mind_ingest_event",
            serde_json::json!({
                "conversation_id": "conv-finalize",
                "event_id": "evt-finalize-1",
                "timestamp_ms": 1700000002000i64,
                "body": {
                    "kind": "message",
                    "role": "user",
                    "text": "finalize this session bundle"
                }
            }),
        );
        let _ = build_pulse_command_response(&cfg, &ingest, Some(&mut runtime))
            .expect("ingest response");

        let handoff = command_envelope_for_test(
            &cfg,
            "req-mind-handoff-finalize",
            "mind_handoff",
            serde_json::json!({
                "conversation_id": "conv-finalize",
                "reason": "prepare export"
            }),
        );
        let _ = build_pulse_command_response(&cfg, &handoff, Some(&mut runtime))
            .expect("handoff response");

        let finalize = command_envelope_for_test(
            &cfg,
            "req-mind-finalize",
            "mind_finalize_session",
            serde_json::json!({
                "reason": "manual finalize"
            }),
        );
        let command = build_pulse_command_response(&cfg, &finalize, Some(&mut runtime))
            .expect("finalize response");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let insight_root = test_root.join(".aoc").join("mind").join("insight");
        assert!(
            insight_root.exists(),
            "insight export directory should exist"
        );

        let mut export_dirs = std::fs::read_dir(&insight_root)
            .expect("read insight dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        export_dirs.sort();
        let export_dir = export_dirs.last().expect("at least one export dir");

        assert!(export_dir.join("t1.md").exists());
        assert!(export_dir.join("t2.md").exists());
        assert!(export_dir.join("manifest.json").exists());

        let store = MindStore::open(resolve_mind_store_path_with_override(&cfg, None))
            .expect("open canonical store");
        let watermark = store
            .project_watermark("session:session-test:pane:12")
            .expect("watermark query")
            .expect("watermark exists");
        assert!(watermark.last_artifact_id.is_some());

        let pending_t3_jobs = store.pending_t3_backlog_jobs().expect("count t3 jobs");
        assert!(pending_t3_jobs >= 1);

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_compaction_rebuild_replays_latest_checkpoint_and_requeues_observer() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-compact-requeue-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        std::process::Command::new("git")
            .arg("init")
            .current_dir(&test_root)
            .output()
            .expect("git init");
        std::fs::write(test_root.join("trail.txt"), "changed\n").expect("write trail file");

        let checkpoint = command_envelope_for_test(
            &cfg,
            "req-mind-compact-requeue-seed",
            "mind_compaction_checkpoint",
            serde_json::json!({
                "conversation_id": "conv-compact-requeue",
                "reason": "pi compaction",
                "summary": "Compacted earlier work into a durable summary.",
                "tokens_before": 777,
                "first_kept_entry_id": "entry-101",
                "compaction_entry_id": "compact-requeue",
                "from_extension": true
            }),
        );
        build_pulse_command_response(&cfg, &checkpoint, Some(&mut runtime))
            .expect("seed checkpoint response");

        let rebuild = command_envelope_for_test(
            &cfg,
            "req-mind-compact-requeue",
            "mind_compaction_rebuild",
            serde_json::json!({"reason": "operator compaction rebuild"}),
        );
        let command = build_pulse_command_response(&cfg, &rebuild, Some(&mut runtime))
            .expect("rebuild response");

        let WireMsg::CommandResult(payload) = &command.response.msg else {
            panic!("expected command result");
        };
        assert_eq!(payload.status, "ok");
        assert!(payload
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("compaction rebuilt and requeued"));

        let checkpoint = runtime
            .store
            .latest_compaction_checkpoint_for_conversation("conv-compact-requeue")
            .expect("load checkpoint")
            .expect("checkpoint exists");
        let rebuilt_slice = runtime
            .store
            .compaction_t0_slice_for_checkpoint(&checkpoint.checkpoint_id)
            .expect("slice by checkpoint")
            .expect("slice exists after rebuild");
        assert_eq!(
            rebuilt_slice.checkpoint_id.as_deref(),
            Some(checkpoint.checkpoint_id.as_str())
        );
        assert!(rebuilt_slice
            .modified_files
            .iter()
            .any(|path| path == "trail.txt"));

        let has_manual_success = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::ManualShortcut
                        && event.status == MindObserverFeedStatus::Success
                        && event.reason.as_deref().unwrap_or_default().contains("compaction slice rebuilt")
            )
        });
        assert!(
            has_manual_success,
            "expected manual compaction rebuild success event"
        );

        let has_compaction_queue = command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::Compaction
                        && event.status == MindObserverFeedStatus::Queued
            )
        });
        assert!(has_compaction_queue, "expected compaction requeue event");

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_t3_requeue_reuses_latest_export_manifest() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-requeue-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let ingest = command_envelope_for_test(
            &cfg,
            "req-mind-ingest-requeue",
            "mind_ingest_event",
            serde_json::json!({
                "conversation_id": "conv-requeue",
                "event_id": "evt-requeue-1",
                "timestamp_ms": 1700000002400i64,
                "body": {
                    "kind": "message",
                    "role": "user",
                    "text": "finalize before requeue"
                }
            }),
        );
        let _ = build_pulse_command_response(&cfg, &ingest, Some(&mut runtime))
            .expect("ingest response");

        let handoff = command_envelope_for_test(
            &cfg,
            "req-mind-handoff-requeue",
            "mind_handoff",
            serde_json::json!({
                "conversation_id": "conv-requeue",
                "reason": "prepare requeue export"
            }),
        );
        let _ = build_pulse_command_response(&cfg, &handoff, Some(&mut runtime))
            .expect("handoff response");

        let finalize = command_envelope_for_test(
            &cfg,
            "req-mind-finalize-requeue",
            "mind_finalize_session",
            serde_json::json!({"reason": "seed manifest"}),
        );
        let _ = build_pulse_command_response(&cfg, &finalize, Some(&mut runtime))
            .expect("finalize response");

        let requeue = command_envelope_for_test(
            &cfg,
            "req-mind-t3-requeue",
            "mind_t3_requeue",
            serde_json::json!({"reason": "operator requeue"}),
        );
        let command = build_pulse_command_response(&cfg, &requeue, Some(&mut runtime))
            .expect("requeue response");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");
        assert!(payload
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("t3 requeue"));
        assert!(command
            .pulse_updates
            .iter()
            .any(|update| matches!(update, PulseUpdate::InsightRuntime(_))));

        assert!(
            runtime
                .store
                .pending_t3_backlog_jobs()
                .expect("pending t3 jobs")
                >= 1
        );

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_compaction_rebuild_reports_unavailable_without_marker_provenance() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-compact-rebuild-missing-provenance-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let checkpoint = CompactionCheckpoint {
            checkpoint_id: "cmpchk:conv-missing:compact-1".to_string(),
            conversation_id: "conv-missing".to_string(),
            session_id: cfg.session_id.clone(),
            ts: Utc
                .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
                .single()
                .expect("ts"),
            trigger_source: "pi_compact".to_string(),
            reason: Some("pi compaction".to_string()),
            summary: Some("checkpoint persisted without marker provenance".to_string()),
            tokens_before: Some(222),
            first_kept_entry_id: Some("entry-1".to_string()),
            compaction_entry_id: Some("compact-1".to_string()),
            from_extension: true,
            marker_event_id: None,
            schema_version: 1,
            created_at: Utc
                .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
                .single()
                .expect("ts"),
            updated_at: Utc
                .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
                .single()
                .expect("ts"),
        };
        runtime
            .store
            .upsert_compaction_checkpoint(&checkpoint)
            .expect("upsert checkpoint");

        let rebuild = command_envelope_for_test(
            &cfg,
            "req-mind-compact-rebuild-missing-provenance",
            "mind_compaction_rebuild",
            serde_json::json!({"reason": "operator compaction rebuild"}),
        );
        let command = build_pulse_command_response(&cfg, &rebuild, Some(&mut runtime))
            .expect("rebuild response");

        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command result")
        };
        assert_eq!(payload.status, "error");
        assert_eq!(
            payload.message.as_deref(),
            Some("compaction rebuild unavailable")
        );
        assert_eq!(
            payload.error.as_ref().map(|error| error.code.as_str()),
            Some("mind_compaction_rebuild_unavailable")
        );

        assert!(command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.trigger == MindObserverFeedTriggerKind::ManualShortcut
                        && event.status == MindObserverFeedStatus::Error
                        && event
                            .reason
                            .as_deref()
                            .unwrap_or_default()
                            .contains("marker provenance missing")
            )
        }));

        assert!(runtime
            .store
            .latest_compaction_checkpoint_for_conversation("conv-missing")
            .expect("lookup checkpoint")
            .is_some());
        assert!(runtime
            .store
            .compaction_t0_slice_for_checkpoint("cmpchk:conv-missing:compact-1")
            .expect("lookup slice")
            .is_none());

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_mind_handshake_rebuild_writes_baseline_and_injection_update() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-handshake-rebuild-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let rebuild = command_envelope_for_test(
            &cfg,
            "req-mind-handshake-rebuild",
            "mind_handshake_rebuild",
            serde_json::json!({"active_tag": "mind"}),
        );
        let command = build_pulse_command_response(&cfg, &rebuild, Some(&mut runtime))
            .expect("rebuild response");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let handshake_path = PathBuf::from(&cfg.project_root)
            .join(".aoc")
            .join("mind")
            .join("t3")
            .join("handshake.md");
        assert!(handshake_path.exists());

        assert!(command.pulse_updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindInjection(payload)
                    if payload.trigger == MindInjectionTriggerKind::Startup
            )
        }));

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn t3_worker_claims_backlog_job_and_advances_project_watermark() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-t3-worker-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let ingest = command_envelope_for_test(
            &cfg,
            "req-mind-ingest-t3",
            "mind_ingest_event",
            serde_json::json!({
                "conversation_id": "conv-t3",
                "event_id": "evt-t3-1",
                "timestamp_ms": 1700000002000i64,
                "body": {
                    "kind": "message",
                    "role": "user",
                    "text": "prepare t3 backlog slice"
                }
            }),
        );
        let _ = build_pulse_command_response(&cfg, &ingest, Some(&mut runtime))
            .expect("ingest response");

        let handoff = command_envelope_for_test(
            &cfg,
            "req-mind-handoff-t3",
            "mind_handoff",
            serde_json::json!({
                "conversation_id": "conv-t3",
                "reason": "prepare export"
            }),
        );
        let _ = build_pulse_command_response(&cfg, &handoff, Some(&mut runtime))
            .expect("handoff response");

        let finalize = command_envelope_for_test(
            &cfg,
            "req-mind-finalize-t3",
            "mind_finalize_session",
            serde_json::json!({
                "reason": "manual finalize"
            }),
        );
        let finalize_result = build_pulse_command_response(&cfg, &finalize, Some(&mut runtime))
            .expect("finalize response");
        let WireMsg::CommandResult(finalize_payload) = finalize_result.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(finalize_payload.status, "ok");

        assert!(
            runtime
                .store
                .pending_t3_backlog_jobs()
                .expect("pending count")
                >= 1
        );

        let updates = runtime.tick_t3_runtime();
        assert!(updates.iter().any(|update| {
            matches!(
                update,
                PulseUpdate::MindObserverEvent(event)
                    if event.runtime.as_deref() == Some("t3_backlog")
            )
        }));

        assert_eq!(
            runtime
                .store
                .pending_t3_backlog_jobs()
                .expect("pending count"),
            0
        );

        let watermark = runtime
            .store
            .project_watermark(&t3_scope_id_for_project_root(&cfg.project_root))
            .expect("watermark query")
            .expect("watermark exists");
        assert!(watermark.last_artifact_id.is_some());

        let active_entries = runtime
            .store
            .active_canon_entries(None)
            .expect("active canon query");
        assert!(!active_entries.is_empty());
        for entry in &active_entries {
            assert!(!entry.evidence_refs.is_empty());
            for evidence_ref in &entry.evidence_refs {
                assert!(
                    runtime
                        .store
                        .artifact_by_id(evidence_ref)
                        .expect("artifact lookup")
                        .is_some(),
                    "evidence ref should resolve to a stored artifact"
                );
            }
        }

        let project_mind_path = PathBuf::from(&cfg.project_root)
            .join(".aoc")
            .join("mind")
            .join("t3")
            .join("project_mind.md");
        assert!(project_mind_path.exists());
        let project_mind = std::fs::read_to_string(&project_mind_path)
            .expect("project_mind export should be readable");
        assert!(project_mind.contains("# Project Mind Canon"));
        assert!(project_mind.contains("## Active canon"));

        let handshake_path = PathBuf::from(&cfg.project_root)
            .join(".aoc")
            .join("mind")
            .join("t3")
            .join("handshake.md");
        assert!(handshake_path.exists());
        let handshake =
            std::fs::read_to_string(&handshake_path).expect("handshake export should be readable");
        assert!(handshake.contains("# Mind Handshake Baseline"));

        let handshake_snapshot = runtime
            .store
            .latest_handshake_snapshot("project", &t3_scope_id_for_project_root(&cfg.project_root))
            .expect("snapshot query")
            .expect("handshake snapshot exists");
        assert!(handshake_snapshot.token_estimate <= MIND_T3_HANDSHAKE_TOKEN_BUDGET);
        assert_eq!(handshake_snapshot.payload_text, handshake);

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn multi_session_finalize_stress_drains_t3_backlog_without_duplicates() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-t3-stress-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");

        let session_count = 6usize;
        let mut runtimes = Vec::new();
        for idx in 0..session_count {
            let mut cfg = test_client_with_root(&test_root.to_string_lossy());
            cfg.session_id = format!("session-stress-{idx}");
            cfg.pane_id = format!("{}", 20 + idx);
            cfg.agent_key = format!("{}::{}", cfg.session_id, cfg.pane_id);
            cfg.agent_label = format!("OpenCode-{idx}");
            let runtime = MindRuntime::new(&cfg).expect("mind runtime");
            runtimes.push((cfg, runtime));
        }

        for (idx, (cfg, runtime)) in runtimes.iter_mut().enumerate() {
            let conversation_id = format!("conv-stress-{idx}");
            let ingest = command_envelope_for_test(
                cfg,
                &format!("req-mind-ingest-stress-{idx}"),
                "mind_ingest_event",
                serde_json::json!({
                    "conversation_id": conversation_id,
                    "event_id": format!("evt-stress-{idx}-1"),
                    "timestamp_ms": 1700000003000i64 + (idx as i64 * 100),
                    "body": {
                        "kind": "message",
                        "role": "user",
                        "text": format!("stress finalize payload {idx}")
                    }
                }),
            );
            let _ =
                build_pulse_command_response(cfg, &ingest, Some(runtime)).expect("ingest response");

            let handoff = command_envelope_for_test(
                cfg,
                &format!("req-mind-handoff-stress-{idx}"),
                "mind_handoff",
                serde_json::json!({
                    "conversation_id": conversation_id,
                    "reason": format!("prepare stress export {idx}")
                }),
            );
            let _ = build_pulse_command_response(cfg, &handoff, Some(runtime))
                .expect("handoff response");

            let finalize = command_envelope_for_test(
                cfg,
                &format!("req-mind-finalize-stress-{idx}"),
                "mind_finalize_session",
                serde_json::json!({"reason": format!("stress finalize {idx}")}),
            );
            let finalize_result = build_pulse_command_response(cfg, &finalize, Some(runtime))
                .expect("finalize response");
            let WireMsg::CommandResult(finalize_payload) = finalize_result.response.msg else {
                panic!("expected command_result")
            };
            assert_eq!(finalize_payload.status, "ok");
        }

        let store_path = resolve_mind_store_path(&runtimes[0].0);
        let pending_jobs = || {
            MindStore::open(&store_path)
                .expect("open shared store")
                .pending_t3_backlog_jobs()
                .expect("pending t3 jobs")
        };
        assert_eq!(pending_jobs(), session_count as i64);

        for (idx, (cfg, runtime)) in runtimes.iter_mut().enumerate() {
            let finalize = command_envelope_for_test(
                cfg,
                &format!("req-mind-finalize-stress-repeat-{idx}"),
                "mind_finalize_session",
                serde_json::json!({"reason": "repeat finalize should dedupe"}),
            );
            let finalize_result = build_pulse_command_response(cfg, &finalize, Some(runtime))
                .expect("repeat finalize response");
            let WireMsg::CommandResult(finalize_payload) = finalize_result.response.msg else {
                panic!("expected command_result")
            };
            assert_eq!(finalize_payload.status, "ok");
        }

        assert_eq!(pending_jobs(), session_count as i64);

        for _round in 0..(session_count * 3) {
            if pending_jobs() == 0 {
                break;
            }
            for (_cfg, runtime) in runtimes.iter_mut() {
                runtime.tick_t3_runtime();
            }
        }

        assert_eq!(pending_jobs(), 0);

        let store = MindStore::open(&store_path).expect("open shared store");
        let jobs = store
            .t3_backlog_jobs_for_project_root(&runtimes[0].0.project_root)
            .expect("t3 jobs query");
        assert_eq!(jobs.len(), session_count);
        assert!(jobs
            .iter()
            .all(|job| job.status == T3BacklogJobStatus::Completed));
        let unique_job_ids = jobs
            .iter()
            .map(|job| job.job_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique_job_ids.len(), session_count);

        let active_entries = store
            .active_canon_entries(None)
            .expect("active canon query");
        assert!(active_entries.len() >= session_count);

        let watermark = store
            .project_watermark(&t3_scope_id_for_project_root(&runtimes[0].0.project_root))
            .expect("watermark query")
            .expect("watermark exists");
        assert!(watermark.last_artifact_id.is_some());

        let handshake_snapshot = store
            .latest_handshake_snapshot(
                "project",
                &t3_scope_id_for_project_root(&runtimes[0].0.project_root),
            )
            .expect("handshake query")
            .expect("handshake snapshot exists");
        assert!(handshake_snapshot.token_estimate <= MIND_T3_HANDSHAKE_TOKEN_BUDGET);

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn mind_provenance_graph_projects_lineage_checkpoint_canon_and_handshake() {
        let store = MindStore::open_in_memory().expect("store");
        let now = Utc
            .with_ymd_and_hms(2026, 3, 2, 9, 0, 0)
            .single()
            .expect("ts");

        store
            .insert_raw_event(&RawEvent {
                event_id: "evt-root".to_string(),
                conversation_id: "conv-root".to_string(),
                agent_id: "session-test::24".to_string(),
                ts: now,
                body: RawEventBody::Message(MessageEvent {
                    role: ConversationRole::User,
                    text: "root conversation".to_string(),
                }),
                attrs: canonical_lineage_attrs(&ConversationLineageMetadata {
                    session_id: "session-a".to_string(),
                    parent_conversation_id: None,
                    root_conversation_id: "conv-root".to_string(),
                }),
            })
            .expect("insert root raw event");
        store
            .insert_raw_event(&RawEvent {
                event_id: "evt-child".to_string(),
                conversation_id: "conv-child".to_string(),
                agent_id: "session-test::24".to_string(),
                ts: now + chrono::Duration::seconds(1),
                body: RawEventBody::Message(MessageEvent {
                    role: ConversationRole::Assistant,
                    text: "child conversation".to_string(),
                }),
                attrs: canonical_lineage_attrs(&ConversationLineageMetadata {
                    session_id: "session-a".to_string(),
                    parent_conversation_id: Some("conv-root".to_string()),
                    root_conversation_id: "conv-root".to_string(),
                }),
            })
            .expect("insert child raw event");

        store
            .insert_observation(
                "obs:1",
                "conv-child",
                now + chrono::Duration::seconds(2),
                "observed planner edits",
                &["ref:1".to_string()],
            )
            .expect("insert observation");
        store
            .insert_reflection(
                "ref:1",
                "conv-child",
                now + chrono::Duration::seconds(3),
                "reflection on planner drift",
                &[],
            )
            .expect("insert reflection");
        store
            .upsert_semantic_provenance(&SemanticProvenance {
                artifact_id: "ref:1".to_string(),
                stage: SemanticStage::T2Reflector,
                runtime: SemanticRuntime::Deterministic,
                provider_name: Some("openai".to_string()),
                model_id: Some("gpt-5".to_string()),
                prompt_version: "mind-v2".to_string(),
                input_hash: "in:1".to_string(),
                output_hash: Some("out:1".to_string()),
                latency_ms: Some(240),
                attempt_count: 1,
                fallback_used: false,
                fallback_reason: None,
                failure_kind: None,
                created_at: now + chrono::Duration::seconds(4),
            })
            .expect("upsert provenance");
        store
            .upsert_artifact_file_link(&ArtifactFileLink {
                artifact_id: "obs:1".to_string(),
                path: "crates/aoc-agent-wrap-rs/src/main.rs".to_string(),
                relation: "modified".to_string(),
                source: "test".to_string(),
                additions: Some(10),
                deletions: Some(2),
                staged: false,
                untracked: false,
                created_at: now + chrono::Duration::seconds(5),
                updated_at: now + chrono::Duration::seconds(5),
            })
            .expect("upsert file link");
        store
            .upsert_artifact_task_link(
                &aoc_core::mind_contracts::ArtifactTaskLink::new(
                    "obs:1".to_string(),
                    "132.1".to_string(),
                    aoc_core::mind_contracts::ArtifactTaskRelation::WorkedOn,
                    9200,
                    vec!["evt-child".to_string()],
                    "test".to_string(),
                    now + chrono::Duration::seconds(5),
                    None,
                )
                .expect("task link"),
            )
            .expect("upsert task link");

        let checkpoint = CompactionCheckpoint {
            checkpoint_id: "cmpchk:conv-child:compact-1".to_string(),
            conversation_id: "conv-child".to_string(),
            session_id: "session-a".to_string(),
            ts: now + chrono::Duration::seconds(6),
            trigger_source: "pi_compact".to_string(),
            reason: Some("checkpoint".to_string()),
            summary: Some("compaction checkpoint".to_string()),
            tokens_before: Some(1234),
            first_kept_entry_id: Some("entry-42".to_string()),
            compaction_entry_id: Some("compact-1".to_string()),
            from_extension: true,
            marker_event_id: Some("evt-child".to_string()),
            schema_version: 1,
            created_at: now + chrono::Duration::seconds(6),
            updated_at: now + chrono::Duration::seconds(6),
        };
        store
            .upsert_compaction_checkpoint(&checkpoint)
            .expect("upsert checkpoint");
        let slice = build_compaction_t0_slice(
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
            &["evt-child".to_string()],
            &["README.md".to_string()],
            &["crates/aoc-agent-wrap-rs/src/main.rs".to_string()],
            Some(&checkpoint.checkpoint_id),
            "mind-v2",
        )
        .expect("build slice");
        store
            .upsert_compaction_t0_slice(&slice)
            .expect("upsert slice");

        store
            .upsert_canon_entry_revision(
                "canon:planner",
                Some("mind"),
                "Planner guidance",
                9100,
                8800,
                None,
                &["obs:1".to_string(), "ref:1".to_string()],
                now + chrono::Duration::seconds(7),
            )
            .expect("upsert canon");
        let scope_key = t3_scope_id_for_project_root("/repo");
        store
            .enqueue_t3_backlog_job(
                "/repo",
                "session-a",
                "12",
                Some("mind"),
                Some("evt-root"),
                Some("evt-child"),
                &["obs:1".to_string(), "ref:1".to_string()],
                now + chrono::Duration::seconds(8),
            )
            .expect("enqueue backlog job");
        store
            .advance_project_watermark(
                &scope_key,
                Some(now + chrono::Duration::seconds(3)),
                Some("ref:1"),
                now + chrono::Duration::seconds(8),
            )
            .expect("advance watermark");
        store
            .upsert_handshake_snapshot(
                "project",
                &scope_key,
                "# Mind Handshake Baseline\n\nPlanner guidance",
                "hash:handshake",
                180,
                now + chrono::Duration::seconds(9),
            )
            .expect("upsert handshake");

        let result = compile_mind_provenance_graph(
            &store,
            &MindProvenanceQueryRequest {
                project_root: Some("/repo".to_string()),
                conversation_id: Some("conv-child".to_string()),
                active_tag: Some("mind".to_string()),
                max_nodes: 64,
                max_edges: 128,
                ..Default::default()
            },
        )
        .expect("compile provenance graph");

        assert_eq!(result.status, "ok");
        assert_provenance_graph_integrity(&result);
        assert!(result
            .nodes
            .iter()
            .any(|node| node.node_id == "conversation:conv-child"));
        assert!(result
            .nodes
            .iter()
            .any(|node| node.node_id == "artifact:obs:1"));
        assert!(result
            .nodes
            .iter()
            .any(|node| node.node_id == "checkpoint:cmpchk:conv-child:compact-1"));
        assert!(result
            .nodes
            .iter()
            .any(|node| node.node_id.starts_with("canon:canon:planner#r1")));
        assert!(result
            .nodes
            .iter()
            .any(|node| node.node_id.starts_with("handshake:hs:")));
        assert!(result
            .nodes
            .iter()
            .any(|node| node.node_id.starts_with("backlog:t3j:")));
        assert!(result.edges.iter().any(|edge| {
            edge.kind == MindProvenanceEdgeKind::ConversationArtifact
                && edge.from == "conversation:conv-child"
                && edge.to == "artifact:obs:1"
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.kind == MindProvenanceEdgeKind::CheckpointSlice
                && edge.from == "checkpoint:cmpchk:conv-child:compact-1"
                && edge.to.starts_with("slice:")
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.kind == MindProvenanceEdgeKind::CanonEvidence
                && edge.from == "canon:canon:planner#r1"
                && edge.to == "artifact:obs:1"
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.kind == MindProvenanceEdgeKind::ScopeBacklogJob
                && edge.from == format!("scope:{}", scope_key)
                && edge.to.starts_with("backlog:t3j:")
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.kind == MindProvenanceEdgeKind::BacklogJobArtifact && edge.to == "artifact:obs:1"
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.kind == MindProvenanceEdgeKind::BacklogJobCanon
                && edge.from.starts_with("backlog:t3j:")
                && edge.to == "canon:canon:planner#r1"
        }));
    }

    #[test]
    fn mind_provenance_graph_supports_checkpoint_and_canon_seed_queries() {
        let store = MindStore::open_in_memory().expect("store");
        let now = Utc
            .with_ymd_and_hms(2026, 3, 2, 11, 0, 0)
            .single()
            .expect("ts");

        store
            .insert_raw_event(&RawEvent {
                event_id: "evt-1".to_string(),
                conversation_id: "conv-1".to_string(),
                agent_id: "session-y::1".to_string(),
                ts: now,
                body: RawEventBody::Message(MessageEvent {
                    role: ConversationRole::User,
                    text: "hello".to_string(),
                }),
                attrs: canonical_lineage_attrs(&ConversationLineageMetadata {
                    session_id: "session-y".to_string(),
                    parent_conversation_id: None,
                    root_conversation_id: "conv-1".to_string(),
                }),
            })
            .expect("insert raw event");
        store
            .insert_observation("obs:seed", "conv-1", now, "note", &[])
            .expect("insert observation");
        let checkpoint = CompactionCheckpoint {
            checkpoint_id: "cmpchk:conv-1:compact-1".to_string(),
            conversation_id: "conv-1".to_string(),
            session_id: "session-y".to_string(),
            ts: now + chrono::Duration::seconds(1),
            trigger_source: "pi_compact".to_string(),
            reason: Some("seed checkpoint".to_string()),
            summary: None,
            tokens_before: None,
            first_kept_entry_id: None,
            compaction_entry_id: Some("compact-1".to_string()),
            from_extension: true,
            marker_event_id: Some("evt-1".to_string()),
            schema_version: 1,
            created_at: now + chrono::Duration::seconds(1),
            updated_at: now + chrono::Duration::seconds(1),
        };
        store
            .upsert_compaction_checkpoint(&checkpoint)
            .expect("upsert checkpoint");
        store
            .upsert_canon_entry_revision(
                "canon:seed",
                Some("mind"),
                "Seed canon",
                9000,
                9000,
                None,
                &["obs:seed".to_string()],
                now + chrono::Duration::seconds(2),
            )
            .expect("upsert canon");
        store
            .mark_active_canon_entries_stale(Some("mind"), now + chrono::Duration::seconds(3), &[])
            .expect("mark stale canon");

        let checkpoint_seed = compile_mind_provenance_graph(
            &store,
            &MindProvenanceQueryRequest {
                checkpoint_id: Some("cmpchk:conv-1:compact-1".to_string()),
                max_nodes: 16,
                max_edges: 16,
                ..Default::default()
            },
        )
        .expect("checkpoint seed compile");
        assert_provenance_graph_integrity(&checkpoint_seed);
        assert!(checkpoint_seed
            .nodes
            .iter()
            .any(|node| node.node_id == "checkpoint:cmpchk:conv-1:compact-1"));
        assert!(checkpoint_seed
            .nodes
            .iter()
            .any(|node| node.node_id == "conversation:conv-1"));

        let canon_seed = compile_mind_provenance_graph(
            &store,
            &MindProvenanceQueryRequest {
                canon_entry_id: Some("canon:seed".to_string()),
                include_stale_canon: true,
                max_nodes: 16,
                max_edges: 16,
                ..Default::default()
            },
        )
        .expect("canon seed compile");
        assert_provenance_graph_integrity(&canon_seed);
        assert!(canon_seed
            .nodes
            .iter()
            .any(|node| node.node_id == "canon:canon:seed#r1"));
    }

    fn assert_provenance_graph_integrity(result: &MindProvenanceQueryResult) {
        let node_ids = result
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<Vec<_>>();
        let unique_node_ids = node_ids.iter().cloned().collect::<HashSet<_>>();
        assert_eq!(
            unique_node_ids.len(),
            node_ids.len(),
            "node ids must be unique"
        );

        let edge_ids = result
            .edges
            .iter()
            .map(|edge| edge.edge_id.clone())
            .collect::<Vec<_>>();
        let unique_edge_ids = edge_ids.iter().cloned().collect::<HashSet<_>>();
        assert_eq!(
            unique_edge_ids.len(),
            edge_ids.len(),
            "edge ids must be unique"
        );

        let sorted_node_ids = {
            let mut ids = node_ids.clone();
            ids.sort();
            ids
        };
        assert_eq!(
            node_ids, sorted_node_ids,
            "nodes must be deterministically sorted"
        );

        let sorted_edge_ids = {
            let mut ids = edge_ids.clone();
            ids.sort();
            ids
        };
        assert_eq!(
            edge_ids, sorted_edge_ids,
            "edges must be deterministically sorted"
        );

        for edge in &result.edges {
            assert!(
                unique_node_ids.contains(&edge.from),
                "edge from endpoint missing: {}",
                edge.from
            );
            assert!(
                unique_node_ids.contains(&edge.to),
                "edge to endpoint missing: {}",
                edge.to
            );
        }
    }

    #[test]
    fn mind_provenance_graph_is_deterministic_under_same_seed() {
        let store = MindStore::open_in_memory().expect("store");
        let now = Utc
            .with_ymd_and_hms(2026, 3, 2, 10, 0, 0)
            .single()
            .expect("ts");

        store
            .insert_raw_event(&RawEvent {
                event_id: "evt-1".to_string(),
                conversation_id: "conv-1".to_string(),
                agent_id: "session-z::1".to_string(),
                ts: now,
                body: RawEventBody::Message(MessageEvent {
                    role: ConversationRole::User,
                    text: "hello".to_string(),
                }),
                attrs: canonical_lineage_attrs(&ConversationLineageMetadata {
                    session_id: "session-z".to_string(),
                    parent_conversation_id: None,
                    root_conversation_id: "conv-1".to_string(),
                }),
            })
            .expect("insert raw event");
        store
            .insert_observation("obs:1", "conv-1", now, "note", &[])
            .expect("insert observation");

        let request = MindProvenanceQueryRequest {
            session_id: Some("session-z".to_string()),
            max_nodes: 16,
            max_edges: 16,
            ..Default::default()
        };
        let first = compile_mind_provenance_graph(&store, &request).expect("first compile");
        let second = compile_mind_provenance_graph(&store, &request).expect("second compile");
        assert_provenance_graph_integrity(&first);
        assert_eq!(first, second);
    }

    #[test]
    fn mind_provenance_graph_marks_truncation_and_preserves_valid_edges() {
        let store = MindStore::open_in_memory().expect("store");
        let now = Utc
            .with_ymd_and_hms(2026, 3, 2, 13, 0, 0)
            .single()
            .expect("ts");

        store
            .insert_raw_event(&RawEvent {
                event_id: "evt-many".to_string(),
                conversation_id: "conv-many".to_string(),
                agent_id: "session-many::1".to_string(),
                ts: now,
                body: RawEventBody::Message(MessageEvent {
                    role: ConversationRole::User,
                    text: "hello".to_string(),
                }),
                attrs: canonical_lineage_attrs(&ConversationLineageMetadata {
                    session_id: "session-many".to_string(),
                    parent_conversation_id: None,
                    root_conversation_id: "conv-many".to_string(),
                }),
            })
            .expect("insert raw event");

        for idx in 0..8 {
            store
                .insert_observation(
                    &format!("obs:{idx}"),
                    "conv-many",
                    now + chrono::Duration::seconds(idx + 1),
                    &format!("note {idx}"),
                    &[],
                )
                .expect("insert observation");
        }

        let result = compile_mind_provenance_graph(
            &store,
            &MindProvenanceQueryRequest {
                session_id: Some("session-many".to_string()),
                max_nodes: 3,
                max_edges: 2,
                ..Default::default()
            },
        )
        .expect("compile truncated graph");

        assert!(result.truncated);
        assert!(result.nodes.len() <= 3);
        assert!(result.edges.len() <= 2);
        assert_provenance_graph_integrity(&result);
    }

    #[test]
    fn mind_provenance_export_is_deterministic_and_focuses_seed_nodes() {
        let store = MindStore::open_in_memory().expect("store");
        let now = Utc
            .with_ymd_and_hms(2026, 3, 2, 14, 0, 0)
            .single()
            .expect("ts");

        store
            .insert_raw_event(&RawEvent {
                event_id: "evt-export".to_string(),
                conversation_id: "conv-export".to_string(),
                agent_id: "session-export::1".to_string(),
                ts: now,
                body: RawEventBody::Message(MessageEvent {
                    role: ConversationRole::User,
                    text: "hello export".to_string(),
                }),
                attrs: canonical_lineage_attrs(&ConversationLineageMetadata {
                    session_id: "session-export".to_string(),
                    parent_conversation_id: None,
                    root_conversation_id: "conv-export".to_string(),
                }),
            })
            .expect("insert raw event");
        store
            .insert_observation("obs:export", "conv-export", now, "note", &[])
            .expect("insert observation");

        let request = MindProvenanceQueryRequest {
            conversation_id: Some("conv-export".to_string()),
            max_nodes: 16,
            max_edges: 16,
            ..Default::default()
        };
        let first = compile_mind_provenance_export(&store, request.clone()).expect("first export");
        let second = compile_mind_provenance_export(&store, request).expect("second export");

        assert_eq!(first, second);
        assert_eq!(first.graph.status, "ok");
        assert!(first
            .mission_control
            .focus_node_ids
            .contains(&"conversation:conv-export".to_string()));
    }

    #[test]
    fn insight_retrieve_ranks_session_and_project_sources() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-retrieve-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(test_root.join(".aoc/mind/t3")).expect("create t3 dir");
        let export_root = test_root.join(".aoc/mind/insight/export-a");
        std::fs::create_dir_all(&export_root).expect("create export dir");
        std::fs::write(
            test_root.join(".aoc/mind/t3/project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:entry-alpha r1\n- topic: mind\n- evidence_refs: ref:1, ref:2\n\nCanonical planner guidance and canon drift notes\n\n### canon:entry-beta r1\n- topic: mind\n\nUnrelated deployment checklist\n",
        )
        .expect("write canon");
        std::fs::write(
            export_root.join("t2.md"),
            "# T2 export\n## art:t2 [conv] (2026-03-01T12:00:00Z)\nSession reflection about planner drift\n",
        )
        .expect("write t2");
        std::fs::write(
            export_root.join("t1.md"),
            "# T1 export\n## art:t1 [conv] (2026-03-01T12:00:00Z)\nObserved planner file edits\n",
        )
        .expect("write t1");
        let mut manifest = test_context_pack_manifest(Some("mind"));
        manifest.project_root = test_root.to_string_lossy().to_string();
        manifest.export_dir = export_root.to_string_lossy().to_string();
        std::fs::write(
            export_root.join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");

        let result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner drift".to_string(),
                scope: InsightRetrievalScope::Auto,
                mode: InsightRetrievalMode::Brief,
                active_tag: Some("mind".to_string()),
                max_results: Some(4),
            },
        );
        assert_eq!(result.status, "ok");
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.source_id == "session_t2:art:t2"));
        let canon_hit = result
            .hits
            .iter()
            .find(|hit| hit.source_id == "t3_canon:canon:entry-alpha")
            .expect("canon alpha hit");
        assert!(canon_hit
            .citations
            .iter()
            .any(|citation| citation.reference == "ref:1"));
        assert!(!result.citations.is_empty());

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn insight_retrieve_prefers_matching_canon_entry_over_unrelated_entry() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-retrieve-rank-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(test_root.join(".aoc/mind/t3")).expect("create t3 dir");
        std::fs::write(
            test_root.join(".aoc/mind/t3/project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:planner r1\n- topic: mind\n- evidence_refs: ref:planner\n\nPlanner drift and retrieval ranking guidance\n\n### canon:deploy r1\n- topic: mind\n- evidence_refs: ref:deploy\n\nDeployment window and release checklist\n\n## Stale canon\n\n### canon:old r1\n- topic: mind\n- evidence_refs: ref:old\n\nOld planner wording\n",
        )
        .expect("write canon");

        let result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner ranking".to_string(),
                scope: InsightRetrievalScope::Project,
                mode: InsightRetrievalMode::Brief,
                active_tag: Some("mind".to_string()),
                max_results: Some(2),
            },
        );
        assert_eq!(result.status, "ok");
        assert_eq!(
            result.hits.first().map(|hit| hit.source_id.as_str()),
            Some("t3_canon:canon:planner")
        );
        assert!(result
            .hits
            .iter()
            .all(|hit| !hit.source_id.ends_with("canon:old") || hit.score <= result.hits[0].score));

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn insight_retrieve_mode_budgets_and_drilldown_refs_are_explicit() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-retrieve-mode-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(test_root.join(".aoc/mind/t3")).expect("create t3 dir");
        std::fs::write(
            test_root.join(".aoc/mind/t3/project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:planner r1\n- topic: mind\n- evidence_refs: ref:planner, ref:session:t2\n\nPlanner drift guidance\nExtra supporting planner note\n",
        )
        .expect("write canon");

        let refs_result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner".to_string(),
                scope: InsightRetrievalScope::Project,
                mode: InsightRetrievalMode::Refs,
                active_tag: Some("mind".to_string()),
                max_results: Some(2),
            },
        );
        assert_eq!(refs_result.status, "ok");
        assert_eq!(refs_result.line_budget_per_hit, 0);
        assert!(refs_result.hits.iter().all(|hit| hit.lines.is_empty()));
        assert!(refs_result.hits[0]
            .drilldown_refs
            .iter()
            .any(|item| item.kind == "canon_entry"));
        assert!(refs_result.hits[0]
            .drilldown_refs
            .iter()
            .any(|item| item.kind == "evidence_ref"));

        let brief_result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner".to_string(),
                scope: InsightRetrievalScope::Project,
                mode: InsightRetrievalMode::Brief,
                active_tag: Some("mind".to_string()),
                max_results: Some(2),
            },
        );
        assert_eq!(brief_result.line_budget_per_hit, 2);
        assert!(brief_result.hits[0].lines.len() <= 2);

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn insight_retrieve_refs_mode_is_bounded_and_falls_back_cleanly() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-retrieve-empty-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create root");

        let result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "missing".to_string(),
                scope: InsightRetrievalScope::Project,
                mode: InsightRetrievalMode::Refs,
                active_tag: None,
                max_results: Some(2),
            },
        );
        assert_eq!(result.status, "fallback");
        assert!(result.fallback_used);
        assert!(result.hits.is_empty());

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn insight_retrieve_auto_scope_falls_back_to_project_when_session_tag_mismatches() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-auto-fallback-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(test_root.join(".aoc/mind/t3")).expect("create t3 dir");
        let export_root = test_root.join(".aoc/mind/insight/export-a");
        std::fs::create_dir_all(&export_root).expect("create export dir");
        std::fs::write(
            test_root.join(".aoc/mind/t3/project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:planner r1\n- topic: mind\n\nProject planner canon\n",
        )
        .expect("write canon");
        std::fs::write(
            export_root.join("t2.md"),
            "# T2 export\n## art:t2 [conv] (2026-03-01T12:00:00Z)\nOps-only session reflection\n",
        )
        .expect("write t2");
        let mut manifest = test_context_pack_manifest(Some("ops"));
        manifest.project_root = test_root.to_string_lossy().to_string();
        manifest.export_dir = export_root.to_string_lossy().to_string();
        std::fs::write(
            export_root.join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");

        let result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner".to_string(),
                scope: InsightRetrievalScope::Auto,
                mode: InsightRetrievalMode::Brief,
                active_tag: Some("mind".to_string()),
                max_results: Some(3),
            },
        );
        assert_eq!(result.status, "ok");
        assert_eq!(result.resolved_scope, InsightRetrievalScope::Project);
        assert!(result
            .hits
            .iter()
            .all(|hit| hit.scope == InsightRetrievalScope::Project));

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn insight_retrieve_session_scope_returns_fallback_on_tag_mismatch() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-session-tag-mismatch-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let export_root = test_root.join(".aoc/mind/insight/export-a");
        std::fs::create_dir_all(&export_root).expect("create export dir");
        std::fs::write(export_root.join("t2.md"), "Session planner reflection").expect("write t2");
        let mut manifest = test_context_pack_manifest(Some("ops"));
        manifest.project_root = test_root.to_string_lossy().to_string();
        manifest.export_dir = export_root.to_string_lossy().to_string();
        std::fs::write(
            export_root.join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");

        let result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner".to_string(),
                scope: InsightRetrievalScope::Session,
                mode: InsightRetrievalMode::Brief,
                active_tag: Some("mind".to_string()),
                max_results: Some(2),
            },
        );
        assert_eq!(result.status, "fallback");
        assert!(result.fallback_used);
        assert!(result.hits.is_empty());

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn insight_retrieve_snips_mode_marks_truncation_when_lines_exceed_budget() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-snips-budget-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(test_root.join(".aoc/mind/t3")).expect("create t3 dir");
        std::fs::write(
            test_root.join(".aoc/mind/t3/project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:planner r1\n- topic: mind\n\nPlanner note one\nPlanner note two\nPlanner note three\nPlanner note four\nPlanner note five\nPlanner note six\n",
        )
        .expect("write canon");

        let result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner".to_string(),
                scope: InsightRetrievalScope::Project,
                mode: InsightRetrievalMode::Snips,
                active_tag: Some("mind".to_string()),
                max_results: Some(1),
            },
        );
        assert_eq!(result.status, "ok");
        assert_eq!(result.line_budget_per_hit, 5);
        assert_eq!(result.hits[0].line_budget, 5);
        assert!(result.hits[0].lines_truncated);
        assert!(result.hits[0].lines.len() <= 5);

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn insight_retrieve_whole_file_session_export_fallback_still_cites_session() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-whole-file-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let export_root = test_root.join(".aoc/mind/insight/export-a");
        std::fs::create_dir_all(&export_root).expect("create export dir");
        std::fs::write(
            export_root.join("t2.md"),
            "Loose planner drift note without section headings\nsecond planner line",
        )
        .expect("write t2");
        let mut manifest = test_context_pack_manifest(Some("mind"));
        manifest.project_root = test_root.to_string_lossy().to_string();
        manifest.export_dir = export_root.to_string_lossy().to_string();
        std::fs::write(
            export_root.join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");

        let result = compile_insight_retrieval(
            &test_root.to_string_lossy(),
            InsightRetrievalRequest {
                query: "planner drift".to_string(),
                scope: InsightRetrievalScope::Session,
                mode: InsightRetrievalMode::Brief,
                active_tag: Some("mind".to_string()),
                max_results: Some(2),
            },
        );
        assert_eq!(result.status, "ok");
        assert_eq!(
            result.hits.first().map(|hit| hit.source_id.as_str()),
            Some("session_t2:session-test")
        );
        assert!(result.hits[0]
            .citations
            .iter()
            .any(|citation| citation.source_id == "session:session-test"));
        assert!(result.hits[0]
            .drilldown_refs
            .iter()
            .any(|item| item.kind == "session_export"));

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn pulse_mind_provenance_query_returns_structured_export() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-provenance-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");

        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");
        let now = Utc
            .with_ymd_and_hms(2026, 3, 2, 12, 0, 0)
            .single()
            .expect("ts");

        runtime
            .store
            .insert_raw_event(&RawEvent {
                event_id: "evt-1".to_string(),
                conversation_id: "conv-1".to_string(),
                agent_id: format!("{}::{}", cfg.session_id, cfg.pane_id),
                ts: now,
                body: RawEventBody::Message(MessageEvent {
                    role: ConversationRole::User,
                    text: "hello provenance".to_string(),
                }),
                attrs: canonical_lineage_attrs(&ConversationLineageMetadata {
                    session_id: cfg.session_id.clone(),
                    parent_conversation_id: None,
                    root_conversation_id: "conv-1".to_string(),
                }),
            })
            .expect("insert raw event");
        runtime
            .store
            .insert_observation("obs:1", "conv-1", now, "note", &[])
            .expect("insert observation");

        let envelope = command_envelope_for_test(
            &cfg,
            "req-mind-provenance-query",
            "mind_provenance_query",
            serde_json::json!({
                "project_root": test_root.to_string_lossy(),
                "conversation_id": "conv-1",
                "max_nodes": 16,
                "max_edges": 16
            }),
        );

        let command = build_pulse_command_response(&cfg, &envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let parsed: Value =
            serde_json::from_str(payload.message.as_deref().unwrap_or("{}")).expect("json result");
        assert_eq!(
            parsed.get("schema_version").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            parsed
                .get("request")
                .and_then(|request| request.get("conversation_id"))
                .and_then(Value::as_str),
            Some("conv-1")
        );
        assert_eq!(
            parsed
                .get("graph")
                .and_then(|graph| graph.get("status"))
                .and_then(Value::as_str),
            Some("ok")
        );
        assert!(parsed
            .get("mission_control")
            .and_then(|mc| mc.get("focus_node_ids"))
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn pulse_mind_provenance_query_rejects_invalid_args() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-mind-provenance-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");

        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");
        let envelope = command_envelope_for_test(
            &cfg,
            "req-mind-provenance-query-invalid",
            "mind_provenance_query",
            serde_json::json!({
                "max_nodes": "oops"
            }),
        );

        let command = build_pulse_command_response(&cfg, &envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "error");
        assert_eq!(
            payload.error.as_ref().map(|error| error.code.as_str()),
            Some("invalid_args")
        );

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn pulse_insight_retrieve_returns_structured_result() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(test_root.join(".aoc/mind/t3")).expect("create t3 dir");
        let export_root = test_root.join(".aoc/mind/insight/export-a");
        std::fs::create_dir_all(&export_root).expect("create export dir");
        std::fs::write(
            test_root.join(".aoc/mind/t3/project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:entry r1\n- topic: mind\n\nCanonical planner guidance\n",
        )
        .expect("write canon");
        std::fs::write(export_root.join("t2.md"), "Session planner guidance").expect("write t2");
        let mut manifest = test_context_pack_manifest(Some("mind"));
        manifest.project_root = test_root.to_string_lossy().to_string();
        manifest.export_dir = export_root.to_string_lossy().to_string();
        std::fs::write(
            export_root.join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");

        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");
        let envelope = command_envelope_for_test(
            &cfg,
            "req-insight-retrieve",
            "insight_retrieve",
            serde_json::json!({
                "query": "planner",
                "scope": "auto",
                "mode": "snips",
                "active_tag": "mind",
                "max_results": 3
            }),
        );

        let command = build_pulse_command_response(&cfg, &envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");
        let parsed: Value =
            serde_json::from_str(payload.message.as_deref().unwrap_or("{}")).expect("json result");
        assert_eq!(parsed.get("status").and_then(Value::as_str), Some("ok"));
        assert_eq!(
            parsed.get("line_budget_per_hit").and_then(Value::as_u64),
            Some(5)
        );
        assert!(parsed
            .get("citations")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));
        assert!(parsed
            .get("hits")
            .and_then(Value::as_array)
            .and_then(|hits| hits.first())
            .and_then(|hit| hit.get("drilldown_refs"))
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[test]
    fn pulse_insight_dispatch_returns_structured_result() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        seed_insight_assets(&test_root);

        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let envelope = command_envelope_for_test(
            &cfg,
            "req-insight-dispatch",
            "insight_dispatch",
            serde_json::json!({
                "mode": "dispatch",
                "agent": "insight-t1-observer",
                "input": "summarize insight state"
            }),
        );

        let command = build_pulse_command_response(&cfg, &envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let parsed: Value =
            serde_json::from_str(payload.message.as_deref().unwrap_or("{}")).expect("json result");
        assert_eq!(parsed.get("mode").and_then(Value::as_str), Some("dispatch"));
        assert!(command
            .pulse_updates
            .iter()
            .any(|update| matches!(update, PulseUpdate::InsightRuntime(_))));

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_insight_detached_dispatch_and_status_return_structured_results() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        seed_insight_assets(&test_root);

        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let dispatch_envelope = command_envelope_for_test(
            &cfg,
            "req-insight-detached-dispatch",
            "insight_detached_dispatch",
            serde_json::json!({
                "mode": "dispatch",
                "agent": "insight-t1-observer",
                "input": "summarize insight state"
            }),
        );

        let dispatch = build_pulse_command_response(&cfg, &dispatch_envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = dispatch.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");
        let parsed: Value =
            serde_json::from_str(payload.message.as_deref().unwrap_or("{}")).expect("json result");
        let job_id = parsed
            .get("job")
            .and_then(|job| job.get("job_id"))
            .and_then(Value::as_str)
            .expect("job id")
            .to_string();
        assert!(dispatch
            .pulse_updates
            .iter()
            .any(|update| matches!(update, PulseUpdate::InsightDetached(_))));

        let status_envelope = command_envelope_for_test(
            &cfg,
            "req-insight-detached-status",
            "insight_detached_status",
            serde_json::json!({
                "job_id": job_id
            }),
        );
        let status = build_pulse_command_response(&cfg, &status_envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(status_payload) = status.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(status_payload.status, "ok");
        let parsed_status: Value =
            serde_json::from_str(status_payload.message.as_deref().unwrap_or("{}"))
                .expect("json result");
        assert_eq!(
            parsed_status
                .get("jobs")
                .and_then(Value::as_array)
                .map(|jobs| jobs.len())
                .unwrap_or_default(),
            1
        );

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn pulse_insight_bootstrap_returns_gap_report() {
        let test_root = std::env::temp_dir().join(format!(
            "aoc-insight-wrap-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&test_root).expect("create test root");
        seed_insight_assets(&test_root);

        let cfg = test_client_with_root(&test_root.to_string_lossy());
        let mut runtime = MindRuntime::new(&cfg).expect("mind runtime");

        let envelope = command_envelope_for_test(
            &cfg,
            "req-insight-bootstrap",
            "insight_bootstrap",
            serde_json::json!({
                "dry_run": true,
                "max_gaps": 8
            }),
        );

        let command = build_pulse_command_response(&cfg, &envelope, Some(&mut runtime))
            .expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");

        let parsed: Value =
            serde_json::from_str(payload.message.as_deref().unwrap_or("{}")).expect("json result");
        assert_eq!(parsed.get("dry_run").and_then(Value::as_bool), Some(true));
        assert!(parsed
            .get("gaps")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));

        let _ = std::fs::remove_dir_all(test_root);
    }

    #[test]
    fn repo_policy_keeps_mind_runtime_artifacts_ignored_and_playbook_present() {
        let root = repo_root();
        let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect("read .gitignore");
        assert!(gitignore.contains("/.aoc/mind/"));
        assert!(gitignore.contains("/.aoc/mind/**"));
        assert!(root.join("scripts/verify-mind-runtime-safety.sh").exists());
        assert!(root
            .join("docs/security/mind-secret-incident-response.md")
            .exists());
    }

    #[test]
    fn ensure_safe_export_text_rejects_secret_payloads() {
        let err = ensure_safe_export_text(
            "# Mind Handshake Baseline\n\nAuthorization: Bearer sk-or-v1-abcdefghijklmnop",
            "handshake export",
        )
        .expect_err("unsafe export should fail");
        assert!(err.contains("handshake export contains unredacted secret-bearing content"));
    }

    #[test]
    fn mind_child_env_excludes_ambient_secrets_and_keeps_allowlisted_vars() {
        static ENV_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("env lock");

        let old_path = std::env::var("PATH").ok();
        let old_secret = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::set_var("PATH", "/usr/bin:/bin");
        std::env::set_var("ANTHROPIC_API_KEY", "super-secret-value");

        let env_pairs = mind_child_env(vec![(
            "AOC_INSIGHT_AGENT".to_string(),
            "observer".to_string(),
        )]);
        let env_map: std::collections::BTreeMap<_, _> = env_pairs.into_iter().collect();

        assert_eq!(
            env_map.get("AOC_INSIGHT_AGENT"),
            Some(&"observer".to_string())
        );
        assert_eq!(env_map.get("PATH"), Some(&"/usr/bin:/bin".to_string()));
        assert!(!env_map.contains_key("ANTHROPIC_API_KEY"));

        if let Some(previous) = old_path {
            std::env::set_var("PATH", previous);
        }
        if let Some(previous) = old_secret {
            std::env::set_var("ANTHROPIC_API_KEY", previous);
        } else {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stop_tokio_child_escalates_when_sigint_ignored() {
        let mut child = Command::new("bash")
            .arg("-lc")
            .arg("trap '' INT; sleep 30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn child");
        let pid = child.id();
        let started = Instant::now();

        let _ = stop_tokio_child_with_escalation(&mut child, pid).await;

        assert!(
            started.elapsed() < Duration::from_secs(6),
            "stop escalation took too long"
        );
    }
}
