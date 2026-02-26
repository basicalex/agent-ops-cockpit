use aoc_core::{
    mind_observer_feed::{
        MindObserverFeedEvent, MindObserverFeedPayload, MindObserverFeedStatus,
        MindObserverFeedTriggerKind,
    },
    pulse_ipc::{
        decode_frame, encode_frame, AgentState as PulseAgentState,
        CommandError as PulseCommandError, CommandResultPayload as PulseCommandResultPayload,
        DeltaPayload as PulseDeltaPayload, HeartbeatPayload as PulseHeartbeatPayload,
        HelloPayload as PulseHelloPayload, ProtocolVersion, StateChange as PulseStateChange,
        StateChangeOp, WireEnvelope as PulseWireEnvelope, WireMsg, CURRENT_PROTOCOL_VERSION,
        DEFAULT_MAX_FRAME_BYTES,
    },
    ProjectData, Task, TaskStatus,
};
use chrono::Utc;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    collections::HashMap,
    env,
    fs::OpenOptions,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex as StdMutex, OnceLock},
    time::Duration,
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
const REDACTED_SECRET: &str = "[REDACTED]";
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
    MindObserverEvent(MindObserverFeedEvent),
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
    mind_observer: MindObserverFeedPayload,
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
            mind_observer: MindObserverFeedPayload::default(),
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
    Error(String),
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
    let mut last_state_hash: Option<u64> = None;
    let mut backoff = Duration::from_secs(1);

    loop {
        let stream = match UnixStream::connect(&socket_path).await {
            Ok(stream) => stream,
            Err(err) => {
                warn!("pulse_connect_error: {err}");
                let mut sleep = tokio::time::sleep(backoff);
                tokio::pin!(sleep);
                loop {
                    tokio::select! {
                        _ = &mut sleep => break,
                        update = rx.recv() => {
                            match update {
                                Some(PulseUpdate::Shutdown) | None => return,
                                Some(PulseUpdate::Remove) => {
                                    last_state_hash = None;
                                }
                                Some(PulseUpdate::Heartbeat { lifecycle }) => {
                                    state.last_heartbeat_ms = Some(Utc::now().timestamp_millis());
                                    if let Some(lifecycle) = lifecycle {
                                        state.lifecycle = normalize_lifecycle_status(&lifecycle);
                                    }
                                }
                                Some(other) => {
                                    apply_pulse_update(&mut state, other);
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

        last_state_hash = None;
        if send_pulse_upsert(&cfg, &state, &mut writer_half, &mut last_state_hash)
            .await
            .is_err()
        {
            continue;
        }

        let mut reader = BufReader::new(reader_half);
        loop {
            tokio::select! {
                update = rx.recv() => {
                    match update {
                        Some(PulseUpdate::Shutdown) => {
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
                            apply_pulse_update(&mut state, other);
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
                inbound = read_next_pulse_frame(&mut reader) => {
                    let Some(envelope) = inbound else {
                        break;
                    };
                    if envelope.session_id != cfg.session_id || envelope.version.0 > CURRENT_PROTOCOL_VERSION {
                        continue;
                    }
                    if let Some(command) = build_pulse_command_response(&cfg, &envelope) {
                        if send_pulse_envelope(&mut writer_half, &command.response).await.is_err() {
                            break;
                        }
                        if let Some(update) = command.pulse_update {
                            apply_pulse_update(&mut state, update);
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
        PulseUpdate::Heartbeat { lifecycle } => {
            state.last_heartbeat_ms = Some(now);
            if let Some(lifecycle) = lifecycle {
                state.lifecycle = normalize_lifecycle_status(&lifecycle);
            }
        }
        PulseUpdate::Remove | PulseUpdate::Shutdown => {}
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
    if !state.mind_observer.events.is_empty() {
        if let Ok(value) = serde_json::to_value(&state.mind_observer) {
            source.insert("mind_observer".to_string(), value);
        }
    }
    serde_json::Value::Object(source)
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
    pulse_update: Option<PulseUpdate>,
}

fn build_pulse_command_response(
    cfg: &ClientConfig,
    envelope: &PulseWireEnvelope,
) -> Option<PulseCommandHandling> {
    let WireMsg::Command(payload) = envelope.msg.clone() else {
        return None;
    };

    let command = payload.command.clone();
    let result = |status: &str,
                  message: Option<String>,
                  error: Option<PulseCommandError>,
                  interrupt: bool,
                  pulse_update: Option<PulseUpdate>| {
        PulseCommandHandling {
            response: PulseWireEnvelope {
                version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
                session_id: cfg.session_id.clone(),
                sender_id: cfg.agent_key.clone(),
                timestamp: Utc::now().to_rfc3339(),
                request_id: envelope.request_id.clone(),
                msg: WireMsg::CommandResult(PulseCommandResultPayload {
                    command: command.clone(),
                    status: status.to_string(),
                    message,
                    error,
                }),
            },
            interrupt,
            pulse_update,
        }
    };

    match payload.command.as_str() {
        "stop_agent" => {
            if payload.target_agent_id.as_deref() != Some(cfg.agent_key.as_str()) {
                return Some(result(
                    "error",
                    Some("target mismatch".to_string()),
                    Some(PulseCommandError {
                        code: "invalid_target".to_string(),
                        message: "target_agent_id does not match publisher".to_string(),
                    }),
                    false,
                    None,
                ));
            }
            Some(result(
                "ok",
                Some("stop signal dispatched".to_string()),
                None,
                true,
                None,
            ))
        }
        "run_observer" => {
            if payload.target_agent_id.as_deref() != Some(cfg.agent_key.as_str()) {
                return Some(result(
                    "error",
                    Some("target mismatch".to_string()),
                    Some(PulseCommandError {
                        code: "invalid_target".to_string(),
                        message: "target_agent_id does not match publisher".to_string(),
                    }),
                    false,
                    None,
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
            Some(result(
                "ok",
                Some("observer trigger queued".to_string()),
                None,
                false,
                Some(PulseUpdate::MindObserverEvent(mind_observer_event(
                    MindObserverFeedStatus::Queued,
                    MindObserverFeedTriggerKind::ManualShortcut,
                    Some(reason),
                ))),
            ))
        }
        _ => Some(result(
            "error",
            Some("unsupported command".to_string()),
            Some(PulseCommandError {
                code: "unsupported_command".to_string(),
                message: "unsupported command".to_string(),
            }),
            false,
            None,
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
            let mut terminator = 0u8;
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
                    terminator = byte;
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
            let mut dropped_mouse = false;
            let (filtered, dropped) = if filter_mouse {
                let (filtered, dropped) = filter_mouse_output(&buffer[..read], &mut mouse_carry);
                (filtered, dropped)
            } else {
                (buffer[..read].to_vec(), false)
            };
            dropped_mouse = dropped;
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
        Ok(repo_root) => match collect_git_summary(&repo_root).await {
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
        },
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
    let dependencies = [
        "git",
        "zellij",
        "aoc-hub-rs",
        "aoc-agent-wrap-rs",
        "aoc-taskmaster",
    ]
    .iter()
    .map(|name| dependency_status(name))
    .collect();
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
            return Err(GitError::Error(err.to_string()));
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("not a git repository") {
            return Err(GitError::NotRepo);
        }
        return Err(GitError::Error(stderr));
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err(GitError::Error("empty git root".to_string()));
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
            return Err(GitError::Error(err.to_string()));
        }
    };
    if !output.status.success() {
        return Err(GitError::Error(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
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

fn runtime_snapshot_path(session_id: &str, pane_id: &str) -> PathBuf {
    let state_home = if let Ok(value) = env::var("XDG_STATE_HOME") {
        if !value.trim().is_empty() {
            PathBuf::from(value)
        } else {
            PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".local/state")
        }
    } else {
        PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".local/state")
    };
    state_home
        .join("aoc")
        .join("telemetry")
        .join(sanitize_component(session_id))
        .join(format!("{}.json", sanitize_component(pane_id)))
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
    use serde_json::Value;
    use std::time::{Duration, Instant};

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
        state.mind_observer.events.push(mind_observer_event(
            MindObserverFeedStatus::Fallback,
            MindObserverFeedTriggerKind::TaskCompleted,
            Some("semantic observer failed (timeout)".to_string()),
        ));

        let source = build_pulse_source(&cfg, &state);
        let root = source.as_object().expect("source should be object");
        assert!(root.contains_key("task_summaries"));
        assert!(root.contains_key("task_summary"));
        assert!(root.contains_key("current_tag"));
        assert!(root.contains_key("diff_summary"));
        assert!(root.contains_key("health"));
        assert!(root.contains_key("mind_observer"));
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

        let command = build_pulse_command_response(&cfg, &envelope).expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.command, "stop_agent");
        assert_eq!(payload.status, "ok");
        assert!(command.interrupt);
        assert!(command.pulse_update.is_none());
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

        let command = build_pulse_command_response(&cfg, &envelope).expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "error");
        assert_eq!(
            payload.error.as_ref().map(|err| err.code.as_str()),
            Some("invalid_target")
        );
        assert!(!command.interrupt);
        assert!(command.pulse_update.is_none());
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

        let command = build_pulse_command_response(&cfg, &envelope).expect("response expected");
        let WireMsg::CommandResult(payload) = command.response.msg else {
            panic!("expected command_result")
        };
        assert_eq!(payload.status, "ok");
        assert!(!command.interrupt);
        match command.pulse_update {
            Some(PulseUpdate::MindObserverEvent(event)) => {
                assert_eq!(event.status, MindObserverFeedStatus::Queued);
                assert_eq!(event.trigger, MindObserverFeedTriggerKind::ManualShortcut);
                assert_eq!(event.reason.as_deref(), Some("pulse_user_request"));
            }
            _ => panic!("expected mind observer feed event"),
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
