use axum::{
    extract::{ws::Message, ws::WebSocket, ws::WebSocketUpgrade, ConnectInfo, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{self, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, Mutex as AsyncMutex, RwLock};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt::writer::BoxMakeWriter, EnvFilter};

const PROTOCOL_VERSION: &str = "1";
const MAX_ENVELOPE_BYTES: usize = 256 * 1024;
const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_FILES_LIST: usize = 500;

#[derive(Clone, Debug)]
struct Config {
    addr: String,
    session_id: String,
    debug: bool,
    stale_seconds: u64,
    ping_interval: Duration,
    write_timeout: Duration,
    log_dir: String,
}

#[derive(Parser, Debug)]
#[command(name = "aoc-hub-rs")]
struct Args {
    #[arg(long, default_value = "")]
    addr: String,
    #[arg(long, default_value = "")]
    session: String,
    #[arg(long, default_value_t = false)]
    debug: bool,
    #[arg(long, default_value_t = 30)]
    stale_seconds: u64,
    #[arg(long, default_value_t = 10)]
    ping_interval: u64,
    #[arg(long, default_value_t = 2)]
    write_timeout: u64,
    #[arg(long, default_value = "")]
    log_dir: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize)]
struct HelloPayload {
    client_id: String,
    role: String,
    capabilities: Vec<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    pane_id: Option<String>,
    #[serde(default)]
    project_root: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentStatusPayload {
    agent_id: String,
    status: String,
    pane_id: String,
    project_root: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiffSummaryPayload {
    agent_id: String,
    repo_root: String,
    git_available: bool,
    summary: Value,
    files: Vec<Value>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct PayloadError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct DiffPatchResponsePayload {
    agent_id: String,
    path: String,
    status: String,
    is_binary: bool,
    #[serde(default)]
    patch: Option<String>,
    #[serde(default)]
    error: Option<PayloadError>,
}

#[derive(Debug, Deserialize)]
struct TaskSummaryPayload {
    agent_id: String,
    tag: String,
    counts: Value,
    #[serde(default)]
    active_tasks: Option<Vec<Value>>,
    #[serde(default)]
    error: Option<PayloadError>,
}

#[derive(Debug, Deserialize)]
struct TaskUpdatePayload {
    agent_id: String,
    tag: String,
    action: String,
    task: Value,
}

#[derive(Debug, Deserialize)]
struct HeartbeatPayload {
    agent_id: String,
    pid: i32,
    cwd: String,
    last_update: String,
}

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    code: String,
    message: String,
}

#[derive(Clone)]
struct Client {
    conn_id: String,
    client_id: String,
    role: String,
    agent_id: String,
    capabilities: Vec<String>,
    sender: mpsc::Sender<Message>,
    last_seen: Arc<AsyncMutex<Instant>>,
}

impl Client {
    async fn touch(&self) {
        let mut last = self.last_seen.lock().await;
        *last = Instant::now();
    }

    async fn last_seen(&self) -> Instant {
        let last = self.last_seen.lock().await;
        *last
    }

    async fn send_text(&self, data: &[u8]) -> bool {
        let text = match std::str::from_utf8(data) {
            Ok(value) => value.to_string(),
            Err(_) => return false,
        };
        self.sender.send(Message::Text(text)).await.is_ok()
    }

    async fn close(&self, reason: &str) {
        let _ = self
            .sender
            .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                code: 1008,
                reason: reason.to_string().into(),
            })))
            .await;
    }
}

struct AgentState {
    last_seen: Instant,
    status: Option<Vec<u8>>,
    diff_summary: Option<Vec<u8>>,
    task_summaries: HashMap<String, Vec<u8>>,
}

struct HubState {
    config: Config,
    conn_counter: AtomicU64,
    clients: RwLock<HashMap<String, Arc<Client>>>,
    subscribers: RwLock<HashMap<String, Arc<Client>>>,
    publishers_by_agent: RwLock<HashMap<String, HashMap<String, Arc<Client>>>>,
    state: RwLock<HashMap<String, AgentState>>,
}

impl HubState {
    fn new(config: Config) -> Self {
        Self {
            config,
            conn_counter: AtomicU64::new(0),
            clients: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashMap::new()),
            publishers_by_agent: RwLock::new(HashMap::new()),
            state: RwLock::new(HashMap::new()),
        }
    }

    fn next_conn_id(&self) -> String {
        let id = self.conn_counter.fetch_add(1, Ordering::SeqCst) + 1;
        format!("conn-{id}")
    }

    async fn register_client(&self, client: Arc<Client>) {
        self.clients
            .write()
            .await
            .insert(client.conn_id.clone(), client.clone());

        if client.role == "subscriber" {
            self.subscribers
                .write()
                .await
                .insert(client.conn_id.clone(), client.clone());
        }

        if client.role == "publisher" {
            let mut pubs = self.publishers_by_agent.write().await;
            let entry = pubs.entry(client.agent_id.clone()).or_default();
            entry.insert(client.conn_id.clone(), client.clone());
        }

        info!(
            event = "client_connected",
            conn_id = %client.conn_id,
            client_id = %client.client_id,
            role = %client.role,
            agent_id = %client.agent_id
        );
    }

    async fn remove_client(&self, client: &Client, reason: &str) {
        client.close(reason).await;
        self.clients.write().await.remove(&client.conn_id);
        self.subscribers.write().await.remove(&client.conn_id);
        let mut offline_event: Option<Vec<u8>> = None;
        let mut prune_agent_state = false;
        if !client.agent_id.is_empty() {
            let mut pubs = self.publishers_by_agent.write().await;
            if let Some(entries) = pubs.get_mut(&client.agent_id) {
                entries.remove(&client.conn_id);
                if entries.is_empty() {
                    pubs.remove(&client.agent_id);
                    prune_agent_state = true;
                }
            }
        }
        if prune_agent_state {
            let previous_status = self
                .state
                .write()
                .await
                .remove(&client.agent_id)
                .and_then(|state| state.status);
            offline_event = build_offline_status_event(
                &self.config.session_id,
                &client.agent_id,
                previous_status.as_deref(),
                reason,
            );
        }
        info!(
            event = "client_disconnected",
            conn_id = %client.conn_id,
            client_id = %client.client_id,
            role = %client.role,
            agent_id = %client.agent_id,
            reason = reason
        );
        if let Some(event) = offline_event {
            self.broadcast_best_effort(&event).await;
        }
    }

    async fn snapshot_subscribers(&self) -> Vec<Arc<Client>> {
        self.subscribers.read().await.values().cloned().collect()
    }

    async fn snapshot_publishers(&self, agent_id: &str) -> Vec<Arc<Client>> {
        self.publishers_by_agent
            .read()
            .await
            .get(agent_id)
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default()
    }

    async fn snapshot_state_messages(&self) -> Vec<Vec<u8>> {
        let state = self.state.read().await;
        let mut ids: Vec<_> = state.keys().cloned().collect();
        ids.sort();
        let mut messages = Vec::new();
        for id in ids {
            if let Some(entry) = state.get(&id) {
                if let Some(payload) = entry.status.as_ref() {
                    messages.push(payload.clone());
                }
                if let Some(payload) = entry.diff_summary.as_ref() {
                    messages.push(payload.clone());
                }
                let mut tags: Vec<_> = entry.task_summaries.keys().cloned().collect();
                tags.sort();
                for tag in tags {
                    if let Some(payload) = entry.task_summaries.get(&tag) {
                        messages.push(payload.clone());
                    }
                }
            }
        }
        messages
    }

    async fn broadcast(&self, raw: &[u8]) {
        let subs = self.snapshot_subscribers().await;
        for sub in subs {
            if !sub.send_text(raw).await {
                warn!(event = "send_error", conn_id = %sub.conn_id);
                self.remove_client(&sub, "send_error").await;
            }
        }
    }

    async fn broadcast_best_effort(&self, raw: &[u8]) {
        let subs = self.snapshot_subscribers().await;
        for sub in subs {
            if !sub.send_text(raw).await {
                warn!(event = "send_error", conn_id = %sub.conn_id);
            }
        }
    }

    async fn forward_to_agent(&self, agent_id: &str, raw: &[u8]) {
        let targets = self.snapshot_publishers(agent_id).await;
        if targets.is_empty() {
            warn!(event = "forward_miss", agent_id = agent_id);
            return;
        }
        for pub_client in targets {
            if !pub_client.send_text(raw).await {
                warn!(event = "send_error", conn_id = %pub_client.conn_id);
                self.remove_client(&pub_client, "send_error").await;
            }
        }
    }

    async fn send_snapshot(&self, client: &Client) {
        let messages = self.snapshot_state_messages().await;
        for msg in &messages {
            if !client.send_text(msg).await {
                warn!(event = "snapshot_error", conn_id = %client.conn_id);
                self.remove_client(client, "snapshot_error").await;
                return;
            }
        }
        info!(event = "snapshot_sent", conn_id = %client.conn_id, count = messages.len());
    }

    async fn update_state(&self, agent_id: &str, kind: &str, raw: &[u8]) {
        if agent_id.is_empty() {
            return;
        }
        let mut state = self.state.write().await;
        let entry = state
            .entry(agent_id.to_string())
            .or_insert_with(|| AgentState {
                last_seen: Instant::now(),
                status: None,
                diff_summary: None,
                task_summaries: HashMap::new(),
            });
        entry.last_seen = Instant::now();
        match kind {
            "agent_status" => entry.status = Some(raw.to_vec()),
            "diff_summary" => entry.diff_summary = Some(raw.to_vec()),
            _ => {}
        }
        info!(event = "state_update", agent_id = agent_id, kind = kind);
    }

    async fn update_task_state(&self, agent_id: &str, tag: &str, raw: &[u8]) {
        if agent_id.is_empty() || tag.is_empty() {
            return;
        }
        let mut state = self.state.write().await;
        let entry = state
            .entry(agent_id.to_string())
            .or_insert_with(|| AgentState {
                last_seen: Instant::now(),
                status: None,
                diff_summary: None,
                task_summaries: HashMap::new(),
            });
        entry.last_seen = Instant::now();
        entry.task_summaries.insert(tag.to_string(), raw.to_vec());
        info!(
            event = "state_update",
            agent_id = agent_id,
            kind = "task_summary",
            tag = tag
        );
    }

    async fn touch_agent(&self, agent_id: &str) {
        if agent_id.is_empty() {
            return;
        }
        let mut state = self.state.write().await;
        let entry = state
            .entry(agent_id.to_string())
            .or_insert_with(|| AgentState {
                last_seen: Instant::now(),
                status: None,
                diff_summary: None,
                task_summaries: HashMap::new(),
            });
        entry.last_seen = Instant::now();
    }

    fn start_stale_reaper(self: Arc<Self>) {
        if self.config.stale_seconds == 0 {
            return;
        }
        let stale_after = Duration::from_secs(self.config.stale_seconds);
        let interval = stale_after / 2;
        let hub = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let clients = hub
                    .clients
                    .read()
                    .await
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                for client in clients {
                    let last_seen = client.last_seen().await;
                    if Instant::now().duration_since(last_seen) > stale_after {
                        warn!(event = "stale_close", conn_id = %client.conn_id);
                        hub.remove_client(&client, "stale").await;
                    }
                }
            }
        });
    }

    fn start_ping(self: Arc<Self>, client: Arc<Client>) {
        if self.config.ping_interval.is_zero() {
            return;
        }
        let interval = self.config.ping_interval;
        let keepalive_touch = client.role == "subscriber";
        let hub = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if client.sender.send(Message::Ping(Vec::new())).await.is_err() {
                    warn!(event = "ping_failed", conn_id = %client.conn_id);
                    hub.remove_client(&client, "ping_failed").await;
                    return;
                }
                if keepalive_touch {
                    client.touch().await;
                }
            }
        });
    }

    async fn handle_message(&self, client: &Client, msg: &Envelope, raw: &[u8]) {
        match msg.r#type.as_str() {
            "agent_status" => {
                let payload = match parse_agent_status(&msg.payload) {
                    Ok(value) => value,
                    Err(err) => {
                        self.send_error(client, "invalid_payload", err, msg.request_id.as_deref())
                            .await;
                        warn!(event = "payload_invalid", error = err);
                        return;
                    }
                };
                if client.role != "publisher"
                    || (!client.agent_id.is_empty() && payload.agent_id != client.agent_id)
                {
                    self.send_error(
                        client,
                        "agent_id_mismatch",
                        "agent id mismatch",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "agent_id_mismatch", conn_id = %client.conn_id);
                    return;
                }
                self.update_state(&payload.agent_id, "agent_status", raw)
                    .await;
                self.broadcast(raw).await;
            }
            "diff_summary" => {
                let payload = match parse_diff_summary(&msg.payload) {
                    Ok(value) => value,
                    Err(err) => {
                        self.send_error(client, "invalid_payload", err, msg.request_id.as_deref())
                            .await;
                        warn!(event = "payload_invalid", error = err);
                        return;
                    }
                };
                if client.role != "publisher"
                    || (!client.agent_id.is_empty() && payload.agent_id != client.agent_id)
                {
                    self.send_error(
                        client,
                        "agent_id_mismatch",
                        "agent id mismatch",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "agent_id_mismatch", conn_id = %client.conn_id);
                    return;
                }
                self.update_state(&payload.agent_id, "diff_summary", raw)
                    .await;
                self.broadcast(raw).await;
            }
            "task_summary" => {
                let payload = match parse_task_summary(&msg.payload) {
                    Ok(value) => value,
                    Err(err) => {
                        self.send_error(client, "invalid_payload", err, msg.request_id.as_deref())
                            .await;
                        warn!(event = "payload_invalid", error = err);
                        return;
                    }
                };
                if client.role != "publisher"
                    || (!client.agent_id.is_empty() && payload.agent_id != client.agent_id)
                {
                    self.send_error(
                        client,
                        "agent_id_mismatch",
                        "agent id mismatch",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "agent_id_mismatch", conn_id = %client.conn_id);
                    return;
                }
                self.update_task_state(&payload.agent_id, &payload.tag, raw)
                    .await;
                self.broadcast(raw).await;
            }
            "task_update" => {
                let payload = match parse_task_update(&msg.payload) {
                    Ok(value) => value,
                    Err(err) => {
                        self.send_error(client, "invalid_payload", err, msg.request_id.as_deref())
                            .await;
                        warn!(event = "payload_invalid", error = err);
                        return;
                    }
                };
                if client.role != "publisher"
                    || (!client.agent_id.is_empty() && payload.agent_id != client.agent_id)
                {
                    self.send_error(
                        client,
                        "agent_id_mismatch",
                        "agent id mismatch",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "agent_id_mismatch", conn_id = %client.conn_id);
                    return;
                }
                self.broadcast(raw).await;
            }
            "diff_patch_request" => {
                let payload = match parse_diff_patch_request(&msg.payload) {
                    Ok(value) => value,
                    Err(err) => {
                        self.send_error(client, "invalid_payload", err, msg.request_id.as_deref())
                            .await;
                        warn!(event = "payload_invalid", error = err);
                        return;
                    }
                };
                if client.role != "subscriber" {
                    self.send_error(
                        client,
                        "role_violation",
                        "subscriber role required",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "role_violation", conn_id = %client.conn_id, role = %client.role);
                    return;
                }
                self.forward_to_agent(&payload.agent_id, raw).await;
            }
            "diff_patch_response" => {
                let payload = match parse_diff_patch_response(&msg.payload) {
                    Ok(value) => value,
                    Err(err) => {
                        self.send_error(client, "invalid_payload", err, msg.request_id.as_deref())
                            .await;
                        warn!(event = "payload_invalid", error = err);
                        return;
                    }
                };
                if let Some(patch) = &payload.patch {
                    if patch.len() > MAX_PATCH_BYTES {
                        self.send_error(
                            client,
                            "patch_too_large",
                            "patch exceeds limit",
                            msg.request_id.as_deref(),
                        )
                        .await;
                        warn!(event = "patch_too_large", conn_id = %client.conn_id, size = patch.len());
                        return;
                    }
                }
                if client.role != "publisher"
                    || (!client.agent_id.is_empty() && payload.agent_id != client.agent_id)
                {
                    self.send_error(
                        client,
                        "agent_id_mismatch",
                        "agent id mismatch",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "agent_id_mismatch", conn_id = %client.conn_id);
                    return;
                }
                self.broadcast(raw).await;
            }
            "heartbeat" => {
                let payload = match parse_heartbeat(&msg.payload) {
                    Ok(value) => value,
                    Err(err) => {
                        self.send_error(client, "invalid_payload", err, msg.request_id.as_deref())
                            .await;
                        warn!(event = "payload_invalid", error = err);
                        return;
                    }
                };
                if client.role != "publisher"
                    || (!client.agent_id.is_empty() && payload.agent_id != client.agent_id)
                {
                    self.send_error(
                        client,
                        "agent_id_mismatch",
                        "agent id mismatch",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "agent_id_mismatch", conn_id = %client.conn_id);
                    return;
                }
                self.touch_agent(&payload.agent_id).await;
            }
            "error" => {
                if parse_error(&msg.payload).is_err() {
                    self.send_error(
                        client,
                        "invalid_payload",
                        "invalid error payload",
                        msg.request_id.as_deref(),
                    )
                    .await;
                    warn!(event = "payload_invalid", conn_id = %client.conn_id);
                    return;
                }
                self.broadcast(raw).await;
            }
            "hello" => {
                self.send_error(
                    client,
                    "unexpected_hello",
                    "unexpected hello",
                    msg.request_id.as_deref(),
                )
                .await;
                warn!(event = "unexpected_hello", conn_id = %client.conn_id);
            }
            other => {
                self.send_error(
                    client,
                    "unknown_message",
                    "unknown message type",
                    msg.request_id.as_deref(),
                )
                .await;
                warn!(event = "unknown_message", conn_id = %client.conn_id, r#type = other);
            }
        }
    }

    async fn send_error(
        &self,
        client: &Client,
        code: &str,
        message: &str,
        request_id: Option<&str>,
    ) {
        let payload = serde_json::json!({
            "code": code,
            "message": message,
            "component": "hub"
        });
        let envelope = serde_json::json!({
            "version": PROTOCOL_VERSION,
            "type": "error",
            "session_id": self.config.session_id.as_str(),
            "sender_id": "aoc-hub",
            "timestamp": Utc::now().to_rfc3339(),
            "payload": payload,
            "request_id": request_id,
        });
        if let Ok(data) = serde_json::to_vec(&envelope) {
            let _ = client.send_text(&data).await;
        }
    }

    async fn handle_socket(self: Arc<Self>, socket: WebSocket, remote: SocketAddr) {
        let (mut ws_sender, mut ws_receiver) = socket.split();
        let (tx, mut rx) = mpsc::channel::<Message>(256);
        let write_timeout = self.config.write_timeout;
        let write_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let send = ws_sender.send(msg);
                if tokio::time::timeout(write_timeout, send).await.is_err() {
                    return;
                }
            }
        });

        let first = match ws_receiver.next().await {
            Some(Ok(msg)) => msg,
            _ => return,
        };

        let data = match message_bytes(first) {
            Some(bytes) => bytes,
            None => return,
        };
        if data.len() > MAX_ENVELOPE_BYTES {
            warn!(event = "hello_too_large", remote = %remote);
            return;
        }

        let msg: Envelope = match serde_json::from_slice(&data) {
            Ok(value) => value,
            Err(err) => {
                warn!(event = "hello_parse", error = %err);
                return;
            }
        };
        if let Err(err) = validate_envelope(&msg) {
            warn!(event = "hello_envelope", error = err);
            return;
        }
        if msg.r#type != "hello" {
            warn!(event = "expected_hello", remote = %remote);
            return;
        }
        if msg.session_id != self.config.session_id {
            warn!(event = "session_id_mismatch", remote = %remote);
            return;
        }
        let payload = match parse_hello(&msg.payload) {
            Ok(value) => value,
            Err(err) => {
                warn!(event = "hello_payload", error = err);
                return;
            }
        };
        if payload.client_id != msg.sender_id {
            warn!(event = "client_id_mismatch", remote = %remote);
            return;
        }

        let conn_id = self.next_conn_id();
        let agent_id = payload.agent_id.clone().unwrap_or_default();
        let client = Arc::new(Client {
            conn_id: conn_id.clone(),
            client_id: payload.client_id,
            role: payload.role,
            agent_id,
            capabilities: payload.capabilities,
            sender: tx.clone(),
            last_seen: Arc::new(AsyncMutex::new(Instant::now())),
        });
        client.touch().await;

        info!(
            event = "handshake_ok",
            conn_id = %client.conn_id,
            client_id = %client.client_id,
            role = %client.role,
            agent_id = %client.agent_id
        );

        self.register_client(client.clone()).await;
        self.clone().start_ping(client.clone());
        if client.role == "subscriber" {
            self.send_snapshot(&client).await;
        }

        while let Some(result) = ws_receiver.next().await {
            let msg = match result {
                Ok(value) => value,
                Err(err) => {
                    warn!(event = "read_error", conn_id = %client.conn_id, error = %err);
                    break;
                }
            };
            let data = match msg {
                Message::Text(text) => text.into_bytes(),
                Message::Binary(bytes) => bytes,
                Message::Close(_) => {
                    info!(event = "client_close", conn_id = %client.conn_id);
                    break;
                }
                Message::Ping(_) | Message::Pong(_) => {
                    client.touch().await;
                    continue;
                }
            };
            if data.len() > MAX_ENVELOPE_BYTES {
                warn!(event = "message_too_large", conn_id = %client.conn_id, size = data.len());
                continue;
            }
            client.touch().await;
            if self.config.debug {
                debug!(event = "message_received", conn_id = %client.conn_id, raw = %String::from_utf8_lossy(&data));
            }
            let msg: Envelope = match serde_json::from_slice(&data) {
                Ok(value) => value,
                Err(err) => {
                    warn!(event = "message_invalid", conn_id = %client.conn_id, error = %err);
                    continue;
                }
            };
            if let Err(err) = validate_envelope(&msg) {
                warn!(event = "message_invalid", conn_id = %client.conn_id, error = err);
                continue;
            }
            if msg.session_id != self.config.session_id {
                warn!(event = "session_mismatch", conn_id = %client.conn_id, msg_session = %msg.session_id);
                self.remove_client(&client, "session_mismatch").await;
                break;
            }
            self.handle_message(&client, &msg, &data).await;
        }

        self.remove_client(&client, "disconnect").await;
        drop(tx);
        let _ = write_task.await;
    }
}

#[tokio::main]
async fn main() {
    let config = load_config();
    let _log_guard = init_logging(&config);
    let addr: SocketAddr = match config.addr.parse() {
        Ok(value) => value,
        Err(err) => {
            error!(event = "invalid_addr", error = %err, addr = %config.addr);
            return;
        }
    };
    if !addr.ip().is_loopback() {
        error!(event = "invalid_addr", addr = %config.addr);
        return;
    }

    let hub = Arc::new(HubState::new(config.clone()));
    hub.clone().start_stale_reaper();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(|| async { "ok" }))
        .with_state(hub.clone());

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(value) => value,
        Err(err) => {
            error!(event = "hub_error", error = %err);
            return;
        }
    };

    info!(event = "hub_start", session_id = %config.session_id, addr = %config.addr);

    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    if let Err(err) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    {
        error!(event = "hub_error", error = %err);
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(hub): State<Arc<HubState>>,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| async move {
        hub.handle_socket(socket, addr).await;
    })
}

fn load_config() -> Config {
    let args = Args::parse();
    let mut session_id = args.session.clone();
    if session_id.is_empty() {
        session_id = resolve_session_id();
    }
    let addr = resolve_addr(&session_id, &args.addr);
    let debug = args.debug || env_true("AOC_HUB_DEBUG");
    let log_dir = resolve_log_dir(&args.log_dir);
    Config {
        addr,
        session_id,
        debug,
        stale_seconds: args.stale_seconds,
        ping_interval: Duration::from_secs(args.ping_interval),
        write_timeout: Duration::from_secs(args.write_timeout),
        log_dir,
    }
}

fn init_logging(config: &Config) -> Option<LogGuard> {
    let level = if config.debug {
        "debug".to_string()
    } else if let Ok(level) = std::env::var("AOC_LOG_LEVEL") {
        level
    } else {
        "info".to_string()
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let writer = match open_log_file(&config.log_dir, &config.session_id) {
        Ok(log_guard) => log_guard,
        Err(err) => {
            eprintln!("log_file_error: {err}");
            LogGuard { file: None }
        }
    };
    let file = writer.file.clone();
    let make_writer = BoxMakeWriter::new(move || MultiWriter::new(file.clone()));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(make_writer)
        .finish();
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        return None;
    }
    Some(writer)
}

struct LogGuard {
    file: Option<Arc<Mutex<std::fs::File>>>,
}

struct MultiWriter {
    stdout: io::Stdout,
    file: Option<Arc<Mutex<std::fs::File>>>,
}

impl MultiWriter {
    fn new(file: Option<Arc<Mutex<std::fs::File>>>) -> Self {
        Self {
            stdout: io::stdout(),
            file,
        }
    }
}

impl Write for MultiWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = self.stdout.write_all(buf);
        if let Some(file) = &self.file {
            let mut file = file.lock().unwrap();
            let _ = file.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = self.stdout.flush();
        if let Some(file) = &self.file {
            let mut file = file.lock().unwrap();
            let _ = file.flush();
        }
        Ok(())
    }
}

fn open_log_file(log_dir: &str, session_id: &str) -> io::Result<LogGuard> {
    if log_dir.trim().is_empty() {
        return Ok(LogGuard { file: None });
    }
    let dir = PathBuf::from(log_dir);
    if std::fs::create_dir_all(&dir).is_err() {
        return Ok(LogGuard { file: None });
    }
    let path = dir.join(format!("aoc-hub-{session_id}.log"));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(path)?;
    Ok(LogGuard {
        file: Some(Arc::new(Mutex::new(file))),
    })
}

fn env_true(key: &str) -> bool {
    match std::env::var(key) {
        Ok(value) => matches!(
            value.trim().to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
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

fn derive_port(session_id: &str) -> u16 {
    let mut hash: u32 = 2166136261;
    for byte in session_id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    42000 + (hash % 2000) as u16
}

fn default_hub_addr(session_id: &str) -> String {
    format!("127.0.0.1:{}", derive_port(session_id))
}

fn resolve_addr(session_id: &str, addr_flag: &str) -> String {
    if !addr_flag.trim().is_empty() {
        return addr_flag.to_string();
    }
    if let Ok(value) = std::env::var("AOC_HUB_ADDR") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    default_hub_addr(session_id)
}

fn resolve_log_dir(log_dir_flag: &str) -> String {
    if !log_dir_flag.trim().is_empty() {
        return log_dir_flag.to_string();
    }
    if let Ok(value) = std::env::var("AOC_LOG_DIR") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    ".aoc/logs".to_string()
}

fn message_bytes(msg: Message) -> Option<Vec<u8>> {
    match msg {
        Message::Text(text) => Some(text.into_bytes()),
        Message::Binary(bytes) => Some(bytes),
        Message::Close(_) => None,
        Message::Ping(_) => None,
        Message::Pong(_) => None,
    }
}

fn validate_envelope(msg: &Envelope) -> Result<(), &'static str> {
    if msg.version.is_empty()
        || msg.r#type.is_empty()
        || msg.session_id.is_empty()
        || msg.sender_id.is_empty()
        || msg.timestamp.is_empty()
    {
        return Err("missing_required_fields");
    }
    if msg.version != PROTOCOL_VERSION {
        return Err("unsupported_version");
    }
    if msg.payload.is_null() {
        return Err("missing_payload");
    }
    if chrono::DateTime::parse_from_rfc3339(&msg.timestamp).is_err() {
        return Err("invalid_timestamp");
    }
    Ok(())
}

fn parse_hello(payload: &Value) -> Result<HelloPayload, &'static str> {
    let value: HelloPayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.client_id.is_empty() || value.role.is_empty() {
        return Err("missing_fields");
    }
    if value.role != "publisher" && value.role != "subscriber" {
        return Err("invalid_role");
    }
    if value.capabilities.is_empty() {
        return Err("missing_capabilities");
    }
    if value.role == "publisher" && value.agent_id.clone().unwrap_or_default().is_empty() {
        return Err("missing_agent_id");
    }
    Ok(value)
}

fn parse_agent_status(payload: &Value) -> Result<AgentStatusPayload, &'static str> {
    let value: AgentStatusPayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.agent_id.is_empty()
        || value.status.is_empty()
        || value.pane_id.is_empty()
        || value.project_root.is_empty()
    {
        return Err("missing_fields");
    }
    Ok(value)
}

fn parse_diff_summary(payload: &Value) -> Result<DiffSummaryPayload, &'static str> {
    let value: DiffSummaryPayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.agent_id.is_empty() || value.repo_root.is_empty() {
        return Err("missing_fields");
    }
    if value.files.len() > MAX_FILES_LIST {
        return Err("files_list_too_large");
    }
    if !value.git_available && value.reason.clone().unwrap_or_default().is_empty() {
        return Err("missing_reason");
    }
    if value.summary.is_null() {
        return Err("missing_summary_or_files");
    }
    Ok(value)
}

fn parse_diff_patch_request(payload: &Value) -> Result<DiffPatchRequestPayload, &'static str> {
    let value: DiffPatchRequestPayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.agent_id.is_empty() || value.path.is_empty() {
        return Err("missing_fields");
    }
    Ok(value)
}

fn parse_diff_patch_response(payload: &Value) -> Result<DiffPatchResponsePayload, &'static str> {
    let value: DiffPatchResponsePayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.agent_id.is_empty() || value.path.is_empty() || value.status.is_empty() {
        return Err("missing_fields");
    }
    Ok(value)
}

fn parse_task_summary(payload: &Value) -> Result<TaskSummaryPayload, &'static str> {
    let value: TaskSummaryPayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.agent_id.is_empty() || value.tag.is_empty() || value.counts.is_null() {
        return Err("missing_fields");
    }
    Ok(value)
}

fn parse_task_update(payload: &Value) -> Result<TaskUpdatePayload, &'static str> {
    let value: TaskUpdatePayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.agent_id.is_empty()
        || value.tag.is_empty()
        || value.action.is_empty()
        || value.task.is_null()
    {
        return Err("missing_fields");
    }
    Ok(value)
}

fn parse_heartbeat(payload: &Value) -> Result<HeartbeatPayload, &'static str> {
    let value: HeartbeatPayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.agent_id.is_empty()
        || value.pid == 0
        || value.cwd.is_empty()
        || value.last_update.is_empty()
    {
        return Err("missing_fields");
    }
    Ok(value)
}

fn parse_error(payload: &Value) -> Result<ErrorPayload, &'static str> {
    let value: ErrorPayload =
        serde_json::from_value(payload.clone()).map_err(|_| "invalid_payload")?;
    if value.code.is_empty() || value.message.is_empty() {
        return Err("missing_fields");
    }
    Ok(value)
}

fn build_offline_status_event(
    session_id: &str,
    agent_id: &str,
    previous_status: Option<&[u8]>,
    reason: &str,
) -> Option<Vec<u8>> {
    let (pane_id, project_root, cwd, agent_label) = offline_metadata(agent_id, previous_status);
    let mut payload = serde_json::Map::new();
    payload.insert("agent_id".to_string(), Value::String(agent_id.to_string()));
    payload.insert("status".to_string(), Value::String("offline".to_string()));
    payload.insert("pane_id".to_string(), Value::String(pane_id));
    payload.insert("project_root".to_string(), Value::String(project_root));
    payload.insert(
        "message".to_string(),
        Value::String(format!("disconnect:{reason}")),
    );
    if let Some(value) = cwd {
        payload.insert("cwd".to_string(), Value::String(value));
    }
    if let Some(value) = agent_label {
        payload.insert("agent_label".to_string(), Value::String(value));
    }

    let envelope = Envelope {
        version: PROTOCOL_VERSION.to_string(),
        r#type: "agent_status".to_string(),
        session_id: session_id.to_string(),
        sender_id: "aoc-hub".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        payload: Value::Object(payload),
        request_id: None,
    };
    serde_json::to_vec(&envelope).ok()
}

fn offline_metadata(
    agent_id: &str,
    previous_status: Option<&[u8]>,
) -> (String, String, Option<String>, Option<String>) {
    let mut pane_id = fallback_pane_id(agent_id);
    let mut project_root = "(unknown)".to_string();
    let mut cwd = None;
    let mut agent_label = None;

    if let Some(raw) = previous_status {
        if let Ok(previous) = serde_json::from_slice::<Envelope>(raw) {
            if let Some(payload) = previous.payload.as_object() {
                if let Some(value) = payload.get("pane_id").and_then(Value::as_str) {
                    if !value.is_empty() {
                        pane_id = value.to_string();
                    }
                }
                if let Some(value) = payload.get("project_root").and_then(Value::as_str) {
                    if !value.is_empty() {
                        project_root = value.to_string();
                    }
                }
                if let Some(value) = payload.get("cwd").and_then(Value::as_str) {
                    if !value.is_empty() {
                        cwd = Some(value.to_string());
                    }
                }
                if let Some(value) = payload.get("agent_label").and_then(Value::as_str) {
                    if !value.is_empty() {
                        agent_label = Some(value.to_string());
                    }
                }
            }
        }
    }

    (pane_id, project_root, cwd, agent_label)
}

fn fallback_pane_id(agent_id: &str) -> String {
    agent_id
        .rsplit_once("::")
        .map(|(_, pane)| pane.to_string())
        .unwrap_or_else(|| agent_id.to_string())
}
