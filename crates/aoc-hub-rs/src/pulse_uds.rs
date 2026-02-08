use aoc_core::pulse_ipc::{
    decode_frame, encode_frame, AgentState, CommandError, CommandPayload, CommandResultPayload,
    DeltaPayload, HeartbeatPayload, HelloPayload, LayoutPane, LayoutStatePayload, LayoutTab,
    ProtocolVersion, SnapshotPayload, StateChange, StateChangeOp, SubscribePayload, WireEnvelope,
    WireMsg, CURRENT_PROTOCOL_VERSION, DEFAULT_MAX_FRAME_BYTES,
};
use chrono::Utc;
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt};
#[cfg(unix)]
use tokio::net::{
    unix::{OwnedReadHalf, OwnedWriteHalf},
    UnixListener, UnixStream,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{mpsc, watch, RwLock},
};
use tracing::{debug, info, warn};

const COMMAND_CACHE_MAX: usize = 512;
const COMMAND_CACHE_TTL: Duration = Duration::from_secs(30);
const PULSE_LATENCY_WARN_MS: i64 = 1500;
const PULSE_LATENCY_INFO_EVERY: u64 = 50;
const LAYOUT_HEALTH_EVERY_TICKS: u64 = 20;
const LAYOUT_WATCH_INTERVAL_MS_DEFAULT: u64 = 1000;
const LAYOUT_WATCH_INTERVAL_MS_MIN: u64 = 250;
const LAYOUT_WATCH_INTERVAL_MS_MAX: u64 = 5000;

#[derive(Clone, Debug)]
pub struct PulseUdsConfig {
    pub session_id: String,
    pub socket_path: PathBuf,
    pub stale_after: Option<Duration>,
    pub write_timeout: Duration,
    pub queue_capacity: usize,
}

#[cfg(not(unix))]
pub async fn run(_config: PulseUdsConfig, mut shutdown: watch::Receiver<bool>) -> io::Result<()> {
    let _ = shutdown.changed().await;
    Ok(())
}

#[cfg(unix)]
pub async fn run(config: PulseUdsConfig, mut shutdown: watch::Receiver<bool>) -> io::Result<()> {
    if let Some(parent) = config.socket_path.parent() {
        fs::create_dir_all(parent)?;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }

    if config.socket_path.exists() {
        let _ = fs::remove_file(&config.socket_path);
    }

    let listener = UnixListener::bind(&config.socket_path)?;
    let _ = fs::set_permissions(&config.socket_path, fs::Permissions::from_mode(0o600));

    let hub = Arc::new(PulseUdsHub::new(config.clone()));
    hub.clone().spawn_stale_reaper(shutdown.clone());
    hub.clone().spawn_layout_watcher(shutdown.clone());

    info!(
        event = "pulse_uds_start",
        session_id = %config.session_id,
        socket = %config.socket_path.display(),
        queue_capacity = config.queue_capacity
    );

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_ok() && *shutdown.borrow() {
                    break;
                }
            }
            accept = listener.accept() => {
                match accept {
                    Ok((stream, _addr)) => {
                        let hub = hub.clone();
                        tokio::spawn(async move {
                            hub.handle_connection(stream).await;
                        });
                    }
                    Err(err) => {
                        warn!(event = "pulse_uds_accept_error", error = %err);
                    }
                }
            }
        }
    }

    let _ = fs::remove_file(&config.socket_path);
    info!(event = "pulse_uds_stop", session_id = %config.session_id);
    Ok(())
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClientRole {
    Publisher,
    Subscriber,
}

#[cfg(unix)]
enum FocusAction {
    ByIndex(u64),
    ByName(String),
}

#[cfg(unix)]
#[derive(Clone)]
struct ClientEntry {
    conn_id: String,
    role: ClientRole,
    agent_id: Option<String>,
    sender: mpsc::Sender<WireEnvelope>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PulseTopic {
    AgentState,
    CommandResult,
    LayoutState,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct TopicFilter {
    agent_state: bool,
    command_result: bool,
    layout_state: bool,
}

#[cfg(unix)]
impl TopicFilter {
    fn baseline() -> Self {
        Self {
            agent_state: true,
            command_result: true,
            layout_state: false,
        }
    }

    fn from_subscribe(payload: &SubscribePayload) -> Self {
        if payload.topics.is_empty() {
            return Self::baseline();
        }

        let mut filter = Self {
            agent_state: false,
            command_result: false,
            layout_state: false,
        };
        for topic in &payload.topics {
            match topic.trim().to_ascii_lowercase().as_str() {
                "agent_state" | "snapshot" | "delta" => filter.agent_state = true,
                "command_result" => filter.command_result = true,
                "layout_state" => filter.layout_state = true,
                _ => {}
            }
        }
        filter
    }

    fn allows(&self, topic: PulseTopic) -> bool {
        match topic {
            PulseTopic::AgentState => self.agent_state,
            PulseTopic::CommandResult => self.command_result,
            PulseTopic::LayoutState => self.layout_state,
        }
    }
}

#[cfg(unix)]
#[derive(Clone)]
struct SubscriberEntry {
    sender: mpsc::Sender<WireEnvelope>,
    topics: TopicFilter,
}

#[cfg(unix)]
struct AgentRecord {
    state: AgentState,
    last_heartbeat: Instant,
}

#[cfg(unix)]
struct CommandCacheEntry {
    envelope: WireEnvelope,
    stored_at: Instant,
}

#[cfg(unix)]
#[derive(Clone)]
struct LayoutCacheEntry {
    signature: u64,
    payload: LayoutStatePayload,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct LayoutSnapshot {
    pane_ids: HashSet<String>,
    tabs: Vec<LayoutTab>,
    panes: Vec<LayoutPane>,
}

#[cfg(unix)]
struct PulseUdsHub {
    config: PulseUdsConfig,
    conn_counter: AtomicU64,
    seq: AtomicU64,
    layout_seq: AtomicU64,
    latency_sample_count: AtomicU64,
    layout_poll_count: AtomicU64,
    layout_emit_count: AtomicU64,
    layout_drop_count: AtomicU64,
    queue_drop_count: AtomicU64,
    backpressure_count: AtomicU64,
    clients: RwLock<HashMap<String, ClientEntry>>,
    subscribers: RwLock<HashMap<String, SubscriberEntry>>,
    publishers: RwLock<HashMap<String, HashSet<String>>>,
    state: RwLock<HashMap<String, AgentRecord>>,
    active_panes: RwLock<HashSet<String>>,
    layout_state: RwLock<Option<LayoutCacheEntry>>,
    command_cache: RwLock<HashMap<String, CommandCacheEntry>>,
    command_cache_order: RwLock<VecDeque<String>>,
}

#[cfg(unix)]
impl PulseUdsHub {
    fn new(config: PulseUdsConfig) -> Self {
        Self {
            config,
            conn_counter: AtomicU64::new(0),
            seq: AtomicU64::new(0),
            layout_seq: AtomicU64::new(0),
            latency_sample_count: AtomicU64::new(0),
            layout_poll_count: AtomicU64::new(0),
            layout_emit_count: AtomicU64::new(0),
            layout_drop_count: AtomicU64::new(0),
            queue_drop_count: AtomicU64::new(0),
            backpressure_count: AtomicU64::new(0),
            clients: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashMap::new()),
            publishers: RwLock::new(HashMap::new()),
            state: RwLock::new(HashMap::new()),
            active_panes: RwLock::new(HashSet::new()),
            layout_state: RwLock::new(None),
            command_cache: RwLock::new(HashMap::new()),
            command_cache_order: RwLock::new(VecDeque::new()),
        }
    }

    fn next_conn_id(&self) -> String {
        let id = self.conn_counter.fetch_add(1, Ordering::SeqCst) + 1;
        format!("pulse-conn-{id}")
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    fn next_layout_seq(&self) -> u64 {
        self.layout_seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn make_envelope(
        &self,
        sender_id: &str,
        request_id: Option<String>,
        msg: WireMsg,
    ) -> WireEnvelope {
        WireEnvelope {
            version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
            session_id: self.config.session_id.clone(),
            sender_id: sender_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id,
            msg,
        }
    }

    async fn register_client(&self, client: ClientEntry) {
        self.clients
            .write()
            .await
            .insert(client.conn_id.clone(), client.clone());

        match client.role {
            ClientRole::Subscriber => {
                self.subscribers.write().await.insert(
                    client.conn_id.clone(),
                    SubscriberEntry {
                        sender: client.sender.clone(),
                        topics: TopicFilter::baseline(),
                    },
                );
            }
            ClientRole::Publisher => {
                if let Some(agent_id) = &client.agent_id {
                    let mut publishers = self.publishers.write().await;
                    publishers
                        .entry(agent_id.clone())
                        .or_default()
                        .insert(client.conn_id.clone());
                }
            }
        }

        info!(
            event = "pulse_client_connected",
            conn_id = %client.conn_id,
            role = ?client.role,
            agent_id = client.agent_id.as_deref().unwrap_or_default(),
        );
    }

    async fn unregister_client(&self, conn_id: &str) {
        let client = self.clients.write().await.remove(conn_id);
        if let Some(client) = client {
            self.subscribers.write().await.remove(conn_id);
            if let Some(agent_id) = client.agent_id {
                let mut publishers = self.publishers.write().await;
                if let Some(set) = publishers.get_mut(&agent_id) {
                    set.remove(conn_id);
                    if set.is_empty() {
                        publishers.remove(&agent_id);
                    }
                }
            }
            info!(event = "pulse_client_disconnected", conn_id = conn_id);
        }
    }

    async fn build_snapshot_envelope(&self) -> WireEnvelope {
        let mut states = {
            let state = self.state.read().await;
            state
                .values()
                .map(|record| record.state.clone())
                .collect::<Vec<_>>()
        };
        states.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        self.make_envelope(
            "aoc-hub",
            None,
            WireMsg::Snapshot(SnapshotPayload {
                seq: self.current_seq(),
                states,
            }),
        )
    }

    async fn broadcast_delta(&self, changes: Vec<StateChange>) {
        if changes.is_empty() {
            return;
        }
        let envelope = self.make_envelope(
            "aoc-hub",
            None,
            WireMsg::Delta(DeltaPayload {
                seq: self.next_seq(),
                changes,
            }),
        );
        self.broadcast_to_subscribers(envelope).await;
    }

    async fn build_layout_envelope(&self) -> Option<WireEnvelope> {
        let payload = {
            let state = self.layout_state.read().await;
            state.as_ref().map(|entry| entry.payload.clone())
        }?;
        Some(self.make_envelope("aoc-hub", None, WireMsg::LayoutState(payload)))
    }

    async fn set_subscriber_topics(&self, conn_id: &str, payload: SubscribePayload) {
        let filter = TopicFilter::from_subscribe(&payload);
        let updated = {
            let mut subscribers = self.subscribers.write().await;
            if let Some(entry) = subscribers.get_mut(conn_id) {
                entry.topics = filter.clone();
                true
            } else {
                false
            }
        };

        if !updated {
            return;
        }

        if filter.agent_state {
            let snapshot = self.build_snapshot_envelope().await;
            let _ = self.send_to_conn(conn_id, snapshot).await;
        }

        if filter.layout_state {
            if let Some(layout) = self.build_layout_envelope().await {
                let _ = self.send_to_conn(conn_id, layout).await;
            }
        }
    }

    async fn send_subscriber_bootstrap(&self, conn_id: &str) {
        let filter = {
            let subscribers = self.subscribers.read().await;
            subscribers
                .get(conn_id)
                .map(|entry| entry.topics.clone())
                .unwrap_or_else(TopicFilter::baseline)
        };

        if filter.agent_state {
            let snapshot = self.build_snapshot_envelope().await;
            let _ = self.send_to_conn(conn_id, snapshot).await;
        }

        if filter.layout_state {
            if let Some(layout) = self.build_layout_envelope().await {
                let _ = self.send_to_conn(conn_id, layout).await;
            }
        }
    }

    async fn broadcast_to_subscribers(&self, envelope: WireEnvelope) {
        let topic = wire_topic(&envelope.msg);
        let subscribers = self.subscribers.read().await.clone();
        let mut slow = Vec::new();

        for (conn_id, entry) in subscribers {
            if let Some(topic) = topic {
                if !entry.topics.allows(topic) {
                    continue;
                }
            }

            match entry.sender.try_send(envelope.clone()) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    let dropped = self.queue_drop_count.fetch_add(1, Ordering::Relaxed) + 1;
                    if topic == Some(PulseTopic::LayoutState) {
                        self.layout_drop_count.fetch_add(1, Ordering::Relaxed);
                    }
                    warn!(
                        event = "pulse_queue_drop",
                        reason = "channel_closed",
                        conn_id = %conn_id,
                        dropped,
                        queue_capacity = self.config.queue_capacity
                    );
                    slow.push(conn_id);
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    let dropped = self.queue_drop_count.fetch_add(1, Ordering::Relaxed) + 1;
                    if topic == Some(PulseTopic::LayoutState) {
                        self.layout_drop_count.fetch_add(1, Ordering::Relaxed);
                    }
                    warn!(
                        event = "pulse_queue_drop",
                        reason = "slow_consumer",
                        conn_id = %conn_id,
                        dropped,
                        queue_capacity = self.config.queue_capacity
                    );
                    slow.push(conn_id);
                }
            }
        }

        for conn_id in slow {
            self.unregister_client(&conn_id).await;
        }
    }

    async fn send_to_conn(&self, conn_id: &str, envelope: WireEnvelope) -> bool {
        let sender = {
            let clients = self.clients.read().await;
            clients.get(conn_id).map(|c| c.sender.clone())
        };
        let Some(sender) = sender else {
            return false;
        };

        match sender.try_send(envelope) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Closed(_)) => {
                let dropped = self.queue_drop_count.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(
                    event = "pulse_queue_drop",
                    reason = "send_closed",
                    conn_id = %conn_id,
                    dropped,
                    queue_capacity = self.config.queue_capacity
                );
                self.unregister_client(conn_id).await;
                false
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                let dropped = self.queue_drop_count.fetch_add(1, Ordering::Relaxed) + 1;
                let backpressure = self.backpressure_count.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(
                    event = "pulse_send_backpressure",
                    conn_id = %conn_id,
                    dropped,
                    backpressure,
                    queue_capacity = self.config.queue_capacity
                );
                self.unregister_client(conn_id).await;
                false
            }
        }
    }

    fn observe_ingest_latency(
        &self,
        stage: &'static str,
        agent_id: &str,
        emitted_at_ms: Option<i64>,
    ) {
        let Some(emitted_at_ms) = emitted_at_ms else {
            return;
        };
        let now = now_ms();
        let latency_ms = now.saturating_sub(emitted_at_ms);
        let sample_id = self.latency_sample_count.fetch_add(1, Ordering::Relaxed) + 1;
        if latency_ms >= PULSE_LATENCY_WARN_MS {
            warn!(
                event = "pulse_end_to_end_latency",
                stage,
                sample_id,
                agent_id,
                emit_ts_ms = emitted_at_ms,
                hub_ingest_ts_ms = now,
                latency_ms
            );
        } else if sample_id % PULSE_LATENCY_INFO_EVERY == 0 {
            info!(
                event = "pulse_end_to_end_latency",
                stage, sample_id, agent_id, latency_ms
            );
        }
    }

    async fn stale_reap_once(&self) {
        let Some(stale_after) = self.config.stale_after else {
            return;
        };

        let mut changes = Vec::new();
        let now = Instant::now();
        {
            let mut state = self.state.write().await;
            let expired = state
                .iter()
                .filter_map(|(agent_id, record)| {
                    if now.duration_since(record.last_heartbeat) > stale_after {
                        Some(agent_id.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            for agent_id in expired {
                state.remove(&agent_id);
                changes.push(StateChange {
                    op: StateChangeOp::Remove,
                    agent_id,
                    state: None,
                });
            }
        }

        if !changes.is_empty() {
            self.broadcast_delta(changes).await;
        }
    }

    fn spawn_stale_reaper(self: Arc<Self>, mut shutdown: watch::Receiver<bool>) {
        let Some(stale_after) = self.config.stale_after else {
            return;
        };
        let tick = std::cmp::max(Duration::from_millis(100), stale_after / 2);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tick);
            loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_ok() && *shutdown.borrow() {
                            break;
                        }
                    }
                    _ = ticker.tick() => {
                        self.stale_reap_once().await;
                    }
                }
            }
        });
    }

    fn spawn_layout_watcher(self: Arc<Self>, mut shutdown: watch::Receiver<bool>) {
        let interval_ms = resolve_layout_watch_interval_ms();
        let interval = Duration::from_millis(interval_ms);
        let session_id = self.config.session_id.clone();
        tokio::spawn(async move {
            let jitter_ms = session_interval_jitter_ms(&session_id, interval_ms);
            let mut ticker = tokio::time::interval_at(
                tokio::time::Instant::now() + Duration::from_millis(jitter_ms),
                interval,
            );
            let mut previous_tick = Instant::now();
            let mut failure_streak: u32 = 0;
            let mut tick_count: u64 = 0;
            let mut slow_cycles: u64 = 0;
            let mut opened_total: u64 = 0;
            let mut closed_total: u64 = 0;
            let mut last_health_at = Instant::now();
            let mut last_poll_total = 0u64;
            let mut last_emit_total = 0u64;
            let mut last_layout_drop_total = 0u64;
            let mut last_queue_drop_total = 0u64;

            info!(
                event = "pulse_layout_watcher_start",
                session_id = %session_id,
                interval_ms,
                jitter_ms
            );

            loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_ok() && *shutdown.borrow() {
                            break;
                        }
                    }
                    _ = ticker.tick() => {
                        tick_count = tick_count.saturating_add(1);
                        let now = Instant::now();
                        let elapsed = now.duration_since(previous_tick);
                        previous_tick = now;
                        let jitter = elapsed.abs_diff(interval);
                        let jitter_ms = jitter.as_millis() as u64;
                        if jitter > Duration::from_millis(150) {
                            warn!(event = "pulse_layout_watcher_jitter", jitter_ms);
                        }

                        let started = Instant::now();
                        self.layout_poll_count.fetch_add(1, Ordering::Relaxed);
                        let layout_snapshot = match collect_layout_snapshot(&session_id).await {
                            Ok(snapshot) => snapshot,
                            Err(err) => {
                                failure_streak = failure_streak.saturating_add(1);
                                let backoff_ms = 150u64.saturating_mul(2u64.saturating_pow(failure_streak.min(4)));
                                warn!(
                                    event = "pulse_layout_watcher_error",
                                    error = %err,
                                    failure_streak,
                                    backoff_ms
                                );
                                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                                continue;
                            }
                        };

                        if failure_streak > 0 {
                            info!(event = "pulse_layout_watcher_recovered", failure_streak);
                        }
                        failure_streak = 0;

                        let (opened, closed) = self
                            .reconcile_layout_panes(layout_snapshot.pane_ids.clone())
                            .await;
                        opened_total = opened_total.saturating_add(opened.len() as u64);
                        closed_total = closed_total.saturating_add(closed.len() as u64);
                        if !opened.is_empty() {
                            info!(event = "pulse_pane_opened", count = opened.len());
                        }
                        if !closed.is_empty() {
                            info!(event = "pulse_pane_closed", count = closed.len());
                            self.prune_closed_panes(closed).await;
                        }

                        self.emit_layout_state_if_changed(layout_snapshot).await;

                        let elapsed_ms = started.elapsed().as_millis() as u64;
                        if elapsed_ms > 500 {
                            slow_cycles = slow_cycles.saturating_add(1);
                            warn!(event = "pulse_layout_watcher_slow", elapsed_ms);
                        }
                        if tick_count % LAYOUT_HEALTH_EVERY_TICKS == 0 {
                            let now = Instant::now();
                            let window_secs = now.duration_since(last_health_at).as_secs_f64().max(0.001);
                            last_health_at = now;

                            let poll_total = self.layout_poll_count.load(Ordering::Relaxed);
                            let emit_total = self.layout_emit_count.load(Ordering::Relaxed);
                            let layout_drop_total = self.layout_drop_count.load(Ordering::Relaxed);
                            let queue_drop_total = self.queue_drop_count.load(Ordering::Relaxed);
                            let polls_per_sec = (poll_total.saturating_sub(last_poll_total) as f64) / window_secs;
                            let emits_per_sec = (emit_total.saturating_sub(last_emit_total) as f64) / window_secs;
                            let layout_drops = layout_drop_total.saturating_sub(last_layout_drop_total);
                            let queue_drops = queue_drop_total.saturating_sub(last_queue_drop_total);

                            last_poll_total = poll_total;
                            last_emit_total = emit_total;
                            last_layout_drop_total = layout_drop_total;
                            last_queue_drop_total = queue_drop_total;

                            let active_panes = self.active_panes.read().await.len();
                            info!(
                                event = "pulse_layout_watcher_health",
                                session_id = %session_id,
                                tick = tick_count,
                                active_panes,
                                failure_streak,
                                slow_cycles,
                                opened_total,
                                closed_total,
                                last_cycle_ms = elapsed_ms,
                                jitter_ms,
                                layout_polls_per_sec = polls_per_sec,
                                layout_emits_per_sec = emits_per_sec,
                                dropped_layout_events = layout_drops,
                                subscriber_queue_drops = queue_drops,
                                layout_polls_total = poll_total,
                                layout_emits_total = emit_total,
                                layout_drop_total,
                                queue_drop_total,
                                backpressure = self.backpressure_count.load(Ordering::Relaxed)
                            );
                        }
                    }
                }
            }
        });
    }

    async fn emit_layout_state_if_changed(&self, snapshot: LayoutSnapshot) {
        let signature = layout_signature(&snapshot.tabs, &snapshot.panes);
        let mut layout_state = self.layout_state.write().await;
        if layout_state
            .as_ref()
            .map(|entry| entry.signature == signature)
            .unwrap_or(false)
        {
            return;
        }

        let payload = LayoutStatePayload {
            layout_seq: self.next_layout_seq(),
            session_id: self.config.session_id.clone(),
            emitted_at_ms: now_ms(),
            tabs: snapshot.tabs,
            panes: snapshot.panes,
        };
        *layout_state = Some(LayoutCacheEntry {
            signature,
            payload: payload.clone(),
        });
        drop(layout_state);

        self.layout_emit_count.fetch_add(1, Ordering::Relaxed);
        let envelope = self.make_envelope("aoc-hub", None, WireMsg::LayoutState(payload));
        self.broadcast_to_subscribers(envelope).await;
    }

    async fn reconcile_layout_panes(&self, latest: HashSet<String>) -> (Vec<String>, Vec<String>) {
        let mut panes = self.active_panes.write().await;
        let opened = latest.difference(&*panes).cloned().collect::<Vec<_>>();
        let closed = panes.difference(&latest).cloned().collect::<Vec<_>>();
        *panes = latest;
        (opened, closed)
    }

    async fn prune_closed_panes(&self, closed: Vec<String>) {
        if closed.is_empty() {
            return;
        }
        let closed_set = closed.into_iter().collect::<HashSet<_>>();
        let mut changes = Vec::new();
        {
            let mut state = self.state.write().await;
            let stale_agents = state
                .iter()
                .filter_map(|(agent_id, record)| {
                    if closed_set.contains(record.state.pane_id.as_str()) {
                        Some(agent_id.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            for agent_id in stale_agents {
                state.remove(&agent_id);
                changes.push(StateChange {
                    op: StateChangeOp::Remove,
                    agent_id,
                    state: None,
                });
            }
        }
        self.broadcast_delta(changes).await;
    }

    async fn apply_heartbeat(&self, publisher_agent: &str, payload: HeartbeatPayload) {
        if payload.agent_id != publisher_agent {
            return;
        }

        let pane_id = pane_from_agent_id(publisher_agent);
        let mut state = self.state.write().await;
        let entry = state
            .entry(publisher_agent.to_string())
            .or_insert_with(|| AgentRecord {
                state: AgentState {
                    agent_id: publisher_agent.to_string(),
                    session_id: self.config.session_id.clone(),
                    pane_id,
                    lifecycle: payload
                        .lifecycle
                        .clone()
                        .unwrap_or_else(|| "running".to_string()),
                    snippet: None,
                    last_heartbeat_ms: Some(payload.last_heartbeat_ms),
                    last_activity_ms: None,
                    updated_at_ms: Some(payload.last_heartbeat_ms),
                    source: None,
                },
                last_heartbeat: Instant::now(),
            });

        entry.last_heartbeat = Instant::now();
        entry.state.last_heartbeat_ms = Some(payload.last_heartbeat_ms);
        entry.state.updated_at_ms = Some(payload.last_heartbeat_ms);
        if let Some(lifecycle) = payload.lifecycle {
            entry.state.lifecycle = lifecycle;
        }
        self.observe_ingest_latency(
            "heartbeat_ingest",
            publisher_agent,
            Some(payload.last_heartbeat_ms),
        );
    }

    async fn apply_delta(&self, publisher_agent: &str, payload: DeltaPayload) {
        let mut outgoing = Vec::new();
        {
            let mut state = self.state.write().await;
            for mut change in payload.changes {
                if change.agent_id != publisher_agent {
                    continue;
                }

                match change.op {
                    StateChangeOp::Upsert => {
                        let Some(mut next_state) = change.state.take() else {
                            continue;
                        };
                        next_state.agent_id = publisher_agent.to_string();
                        next_state.session_id = self.config.session_id.clone();
                        if next_state.pane_id.is_empty() {
                            next_state.pane_id = pane_from_agent_id(publisher_agent);
                        }
                        if next_state.updated_at_ms.is_none() {
                            next_state.updated_at_ms = Some(now_ms());
                        }
                        self.observe_ingest_latency(
                            "delta_ingest",
                            publisher_agent,
                            next_state
                                .updated_at_ms
                                .or(next_state.last_activity_ms)
                                .or(next_state.last_heartbeat_ms),
                        );
                        let record = AgentRecord {
                            state: next_state.clone(),
                            last_heartbeat: Instant::now(),
                        };
                        state.insert(publisher_agent.to_string(), record);

                        outgoing.push(StateChange {
                            op: StateChangeOp::Upsert,
                            agent_id: publisher_agent.to_string(),
                            state: Some(next_state),
                        });
                    }
                    StateChangeOp::Remove => {
                        if state.remove(publisher_agent).is_some() {
                            outgoing.push(StateChange {
                                op: StateChangeOp::Remove,
                                agent_id: publisher_agent.to_string(),
                                state: None,
                            });
                        }
                    }
                }
            }
        }
        self.broadcast_delta(outgoing).await;
    }

    async fn route_command(
        &self,
        source_conn_id: &str,
        envelope: WireEnvelope,
        payload: CommandPayload,
    ) {
        if self.client_role(source_conn_id).await != Some(ClientRole::Subscriber) {
            self.send_command_error(
                source_conn_id,
                envelope.request_id,
                &payload.command,
                "role_violation",
                "subscriber role required",
            )
            .await;
            return;
        }

        if let Some(request_id) = envelope.request_id.as_deref() {
            if let Some(cached) = self.cached_command_result(source_conn_id, request_id).await {
                let _ = self.send_to_conn(source_conn_id, cached).await;
                return;
            }
        }

        match payload.command.as_str() {
            "focus_tab" => {
                self.handle_focus_tab_command(source_conn_id, envelope.request_id, payload)
                    .await;
            }
            "stop_agent" => {
                self.route_stop_agent_command(source_conn_id, envelope, payload)
                    .await;
            }
            _ => {
                self.send_command_error(
                    source_conn_id,
                    envelope.request_id,
                    &payload.command,
                    "unsupported_command",
                    "unsupported command",
                )
                .await;
            }
        }
    }

    async fn route_stop_agent_command(
        &self,
        source_conn_id: &str,
        envelope: WireEnvelope,
        payload: CommandPayload,
    ) {
        let target = payload.target_agent_id.unwrap_or_default();
        if target.is_empty() || !agent_in_session(&self.config.session_id, &target) {
            self.send_command_error(
                source_conn_id,
                envelope.request_id,
                &payload.command,
                "invalid_target",
                "target_agent_id is required and must match session",
            )
            .await;
            return;
        }

        let targets = {
            let publishers = self.publishers.read().await;
            publishers
                .get(&target)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>()
        };
        if targets.is_empty() {
            self.send_command_error(
                source_conn_id,
                envelope.request_id,
                &payload.command,
                "publisher_missing",
                "target publisher is not connected",
            )
            .await;
            return;
        }

        let mut delivered = false;
        for conn_id in targets {
            delivered |= self.send_to_conn(&conn_id, envelope.clone()).await;
        }
        if !delivered {
            self.send_command_error(
                source_conn_id,
                envelope.request_id,
                &payload.command,
                "publisher_unavailable",
                "failed to deliver command",
            )
            .await;
            return;
        }

        self.send_command_result(
            source_conn_id,
            envelope.request_id,
            &payload.command,
            "accepted",
            "command forwarded",
            None,
        )
        .await;
    }

    async fn handle_focus_tab_command(
        &self,
        source_conn_id: &str,
        request_id: Option<String>,
        payload: CommandPayload,
    ) {
        match self.execute_focus_tab(&payload.args).await {
            Ok(message) => {
                self.send_command_result(
                    source_conn_id,
                    request_id,
                    &payload.command,
                    "ok",
                    &message,
                    None,
                )
                .await;
            }
            Err(err) => {
                let message = err.message.clone();
                self.send_command_result(
                    source_conn_id,
                    request_id,
                    &payload.command,
                    "error",
                    &message,
                    Some(err),
                )
                .await;
            }
        }
    }

    async fn execute_focus_tab(&self, args: &serde_json::Value) -> Result<String, CommandError> {
        let tab_index = args.get("tab_index").and_then(|value| value.as_i64());
        let tab_name = args.get("tab_name").and_then(|value| value.as_str());

        let action = if let Some(index) = tab_index {
            if index < 1 {
                return Err(CommandError {
                    code: "invalid_args".to_string(),
                    message: "tab_index must be >= 1".to_string(),
                });
            }
            FocusAction::ByIndex(index as u64)
        } else if let Some(name) = tab_name {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(CommandError {
                    code: "invalid_args".to_string(),
                    message: "tab_name cannot be empty".to_string(),
                });
            }
            FocusAction::ByName(trimmed.to_string())
        } else {
            return Err(CommandError {
                code: "invalid_args".to_string(),
                message: "focus_tab requires tab_index or tab_name".to_string(),
            });
        };

        let session_id = self.config.session_id.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("zellij");
            cmd.arg("--session").arg(&session_id).arg("action");
            match action {
                FocusAction::ByIndex(index) => {
                    cmd.arg("go-to-tab").arg(index.to_string());
                }
                FocusAction::ByName(name) => {
                    cmd.arg("go-to-tab-name").arg(name);
                }
            }
            cmd.output()
        })
        .await
        .map_err(|err| CommandError {
            code: "focus_failed".to_string(),
            message: format!("failed to execute focus command: {err}"),
        })?
        .map_err(|err| CommandError {
            code: "focus_failed".to_string(),
            message: format!("failed to spawn zellij: {err}"),
        })?;

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                format!("focus command failed: {}", result.status)
            } else {
                stderr
            };
            return Err(CommandError {
                code: "focus_failed".to_string(),
                message,
            });
        }

        Ok("tab focus updated".to_string())
    }

    async fn send_command_error(
        &self,
        conn_id: &str,
        request_id: Option<String>,
        command: &str,
        code: &str,
        message: &str,
    ) {
        self.send_command_result(
            conn_id,
            request_id,
            command,
            "error",
            message,
            Some(CommandError {
                code: code.to_string(),
                message: message.to_string(),
            }),
        )
        .await;
    }

    async fn send_command_result(
        &self,
        conn_id: &str,
        request_id: Option<String>,
        command: &str,
        status: &str,
        message: &str,
        error: Option<CommandError>,
    ) {
        let envelope = self.make_envelope(
            "aoc-hub",
            request_id.clone(),
            WireMsg::CommandResult(CommandResultPayload {
                command: command.to_string(),
                status: status.to_string(),
                message: Some(message.to_string()),
                error,
            }),
        );
        if self.send_to_conn(conn_id, envelope.clone()).await {
            if let Some(request_id) = request_id.as_deref() {
                self.cache_command_result(conn_id, request_id, envelope)
                    .await;
            }
        }
    }

    async fn client_role(&self, conn_id: &str) -> Option<ClientRole> {
        self.clients
            .read()
            .await
            .get(conn_id)
            .map(|entry| entry.role)
    }

    fn command_cache_key(conn_id: &str, request_id: &str) -> String {
        format!("{conn_id}:{request_id}")
    }

    async fn cached_command_result(&self, conn_id: &str, request_id: &str) -> Option<WireEnvelope> {
        let key = Self::command_cache_key(conn_id, request_id);
        {
            let cache = self.command_cache.read().await;
            if let Some(entry) = cache.get(&key) {
                if entry.stored_at.elapsed() <= COMMAND_CACHE_TTL {
                    return Some(entry.envelope.clone());
                }
            }
        }
        self.command_cache.write().await.remove(&key);
        None
    }

    async fn cache_command_result(&self, conn_id: &str, request_id: &str, envelope: WireEnvelope) {
        let key = Self::command_cache_key(conn_id, request_id);
        {
            let mut cache = self.command_cache.write().await;
            cache.insert(
                key.clone(),
                CommandCacheEntry {
                    envelope,
                    stored_at: Instant::now(),
                },
            );
        }
        let mut evicted = Vec::new();
        {
            let mut order = self.command_cache_order.write().await;
            order.push_back(key.clone());
            while order.len() > COMMAND_CACHE_MAX {
                if let Some(oldest) = order.pop_front() {
                    evicted.push(oldest);
                }
            }
        }
        if !evicted.is_empty() {
            let mut cache = self.command_cache.write().await;
            for oldest in evicted {
                cache.remove(&oldest);
            }
        }
        let expired = {
            let cache = self.command_cache.read().await;
            cache
                .iter()
                .filter_map(|(cache_key, entry)| {
                    if entry.stored_at.elapsed() > COMMAND_CACHE_TTL {
                        Some(cache_key.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        if !expired.is_empty() {
            let mut cache = self.command_cache.write().await;
            for cache_key in expired {
                cache.remove(&cache_key);
            }
        }
    }

    async fn handle_connection(self: Arc<Self>, stream: UnixStream) {
        let conn_id = self.next_conn_id();
        let (reader_half, writer_half) = stream.into_split();
        let mut reader = BufReader::new(reader_half);

        let Some(hello) = read_next_valid_frame(&mut reader).await else {
            return;
        };

        if hello.version.0 > CURRENT_PROTOCOL_VERSION {
            warn!(
                event = "pulse_uds_unsupported_version",
                conn_id = %conn_id,
                version = hello.version.0
            );
            return;
        }
        if hello.session_id != self.config.session_id {
            warn!(
                event = "pulse_uds_session_mismatch",
                conn_id = %conn_id,
                msg_session = %hello.session_id,
                expected_session = %self.config.session_id
            );
            return;
        }

        let WireMsg::Hello(payload) = hello.msg else {
            warn!(event = "pulse_uds_expected_hello", conn_id = %conn_id);
            return;
        };

        let role = match payload.role.as_str() {
            "publisher" => ClientRole::Publisher,
            "subscriber" => ClientRole::Subscriber,
            _ => {
                warn!(event = "pulse_uds_invalid_role", conn_id = %conn_id, role = %payload.role);
                return;
            }
        };

        let agent_id = match role {
            ClientRole::Publisher => {
                match normalize_publisher_agent_id(&self.config.session_id, &payload) {
                    Some(value) => Some(value),
                    None => {
                        warn!(event = "pulse_uds_missing_agent_id", conn_id = %conn_id);
                        return;
                    }
                }
            }
            ClientRole::Subscriber => None,
        };

        let (tx, rx) = mpsc::channel::<WireEnvelope>(self.config.queue_capacity);
        let write_timeout = self.config.write_timeout;
        let conn_for_writer = conn_id.clone();
        let writer_task = tokio::spawn(async move {
            writer_loop(conn_for_writer, writer_half, rx, write_timeout).await;
        });

        let client = ClientEntry {
            conn_id: conn_id.clone(),
            role,
            agent_id: agent_id.clone(),
            sender: tx.clone(),
        };
        self.register_client(client).await;

        if role == ClientRole::Subscriber {
            self.send_subscriber_bootstrap(&conn_id).await;
        }

        loop {
            let Some(envelope) = read_next_valid_frame(&mut reader).await else {
                break;
            };
            if envelope.version.0 > CURRENT_PROTOCOL_VERSION {
                warn!(
                    event = "pulse_uds_skip_version",
                    conn_id = %conn_id,
                    version = envelope.version.0
                );
                continue;
            }
            if envelope.session_id != self.config.session_id {
                warn!(
                    event = "pulse_uds_message_session_mismatch",
                    conn_id = %conn_id,
                    msg_session = %envelope.session_id,
                    expected_session = %self.config.session_id
                );
                break;
            }

            match (role, envelope.msg.clone()) {
                (ClientRole::Publisher, WireMsg::Heartbeat(payload)) => {
                    if let Some(publisher_agent) = agent_id.as_deref() {
                        self.apply_heartbeat(publisher_agent, payload).await;
                    }
                }
                (ClientRole::Publisher, WireMsg::Delta(payload)) => {
                    if let Some(publisher_agent) = agent_id.as_deref() {
                        self.apply_delta(publisher_agent, payload).await;
                    }
                }
                (ClientRole::Publisher, WireMsg::Snapshot(payload)) => {
                    if let Some(publisher_agent) = agent_id.as_deref() {
                        let changes = payload
                            .states
                            .into_iter()
                            .map(|state| StateChange {
                                op: StateChangeOp::Upsert,
                                agent_id: publisher_agent.to_string(),
                                state: Some(state),
                            })
                            .collect::<Vec<_>>();
                        self.apply_delta(
                            publisher_agent,
                            DeltaPayload {
                                seq: payload.seq,
                                changes,
                            },
                        )
                        .await;
                    }
                }
                (ClientRole::Publisher, WireMsg::CommandResult(payload)) => {
                    let forwarded = self.make_envelope(
                        &envelope.sender_id,
                        envelope.request_id,
                        WireMsg::CommandResult(payload),
                    );
                    self.broadcast_to_subscribers(forwarded).await;
                }
                (ClientRole::Subscriber, WireMsg::Command(payload)) => {
                    self.route_command(&conn_id, envelope, payload).await;
                }
                (ClientRole::Subscriber, WireMsg::Subscribe(payload)) => {
                    self.set_subscriber_topics(&conn_id, payload).await;
                }
                (_, WireMsg::Hello(_)) => {
                    warn!(event = "pulse_uds_unexpected_hello", conn_id = %conn_id);
                }
                _ => {
                    debug!(event = "pulse_uds_ignored_message", conn_id = %conn_id);
                }
            }
        }

        self.unregister_client(&conn_id).await;
        drop(tx);
        let _ = writer_task.await;
    }
}

fn resolve_layout_watch_interval_ms() -> u64 {
    std::env::var("AOC_PULSE_LAYOUT_WATCH_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| value.clamp(LAYOUT_WATCH_INTERVAL_MS_MIN, LAYOUT_WATCH_INTERVAL_MS_MAX))
        .unwrap_or(LAYOUT_WATCH_INTERVAL_MS_DEFAULT)
}

fn session_interval_jitter_ms(session_id: &str, interval_ms: u64) -> u64 {
    if interval_ms <= 1 {
        return 0;
    }
    let seed = session_id.bytes().fold(0u64, |acc, b| {
        acc.wrapping_mul(131).wrapping_add(u64::from(b))
    });
    seed % interval_ms
}

#[cfg(unix)]
async fn writer_loop(
    conn_id: String,
    mut writer: OwnedWriteHalf,
    mut rx: mpsc::Receiver<WireEnvelope>,
    write_timeout: Duration,
) {
    while let Some(envelope) = rx.recv().await {
        let frame = match encode_frame(&envelope, DEFAULT_MAX_FRAME_BYTES) {
            Ok(value) => value,
            Err(err) => {
                warn!(event = "pulse_uds_encode_error", conn_id = %conn_id, error = %err);
                continue;
            }
        };
        let send = async {
            writer.write_all(&frame).await?;
            writer.flush().await
        };
        if tokio::time::timeout(write_timeout, send).await.is_err() {
            warn!(event = "pulse_uds_write_timeout", conn_id = %conn_id);
            break;
        }
    }
}

#[cfg(unix)]
async fn read_next_valid_frame(reader: &mut BufReader<OwnedReadHalf>) -> Option<WireEnvelope> {
    loop {
        let mut line = Vec::new();
        let n = match reader.read_until(b'\n', &mut line).await {
            Ok(value) => value,
            Err(err) => {
                warn!(event = "pulse_uds_read_error", error = %err);
                return None;
            }
        };
        if n == 0 {
            return None;
        }
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        match decode_frame::<WireEnvelope>(&line, DEFAULT_MAX_FRAME_BYTES) {
            Ok(envelope) => return Some(envelope),
            Err(err) => {
                warn!(event = "pulse_uds_decode_error", error = %err);
                continue;
            }
        }
    }
}

#[cfg(unix)]
fn wire_topic(msg: &WireMsg) -> Option<PulseTopic> {
    match msg {
        WireMsg::Snapshot(_) | WireMsg::Delta(_) => Some(PulseTopic::AgentState),
        WireMsg::CommandResult(_) => Some(PulseTopic::CommandResult),
        WireMsg::LayoutState(_) => Some(PulseTopic::LayoutState),
        _ => None,
    }
}

#[cfg(unix)]
async fn collect_layout_snapshot(session_id: &str) -> Result<LayoutSnapshot, String> {
    if session_id.trim().is_empty() {
        return Ok(LayoutSnapshot {
            pane_ids: HashSet::new(),
            tabs: Vec::new(),
            panes: Vec::new(),
        });
    }
    let session = session_id.to_string();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("zellij")
            .arg("--session")
            .arg(session)
            .arg("action")
            .arg("dump-layout")
            .output()
    })
    .await
    .map_err(|err| format!("join_error:{err}"))
    .and_then(|result| result.map_err(|err| format!("spawn_error:{err}")))?;

    if !output.status.success() {
        return Err(format!("dump_layout_exit:{}", output.status));
    }

    let layout = String::from_utf8(output.stdout).map_err(|err| format!("utf8:{err}"))?;
    Ok(parse_layout_snapshot(&layout))
}

#[cfg(unix)]
fn parse_layout_snapshot(layout: &str) -> LayoutSnapshot {
    let mut pane_ids = HashSet::new();
    let mut tabs = Vec::new();
    let mut panes = HashMap::new();
    let mut current_tab_index = 0u64;
    let mut current_tab_name = String::new();
    let mut current_tab_focused = false;

    for line in layout.lines() {
        if line_is_tab_decl(line) {
            current_tab_index = current_tab_index.saturating_add(1);
            current_tab_name = extract_layout_attr(line, "name")
                .unwrap_or_else(|| format!("tab-{current_tab_index}"));
            current_tab_focused = line.contains("focus=true") || line.contains("focus true");
            tabs.push(LayoutTab {
                index: current_tab_index,
                name: current_tab_name.clone(),
                focused: current_tab_focused,
            });
        }

        for pane_id in extract_pane_ids_from_layout_line(line) {
            pane_ids.insert(pane_id.clone());
            if current_tab_index > 0 {
                panes.insert(
                    pane_id.clone(),
                    LayoutPane {
                        pane_id,
                        tab_index: current_tab_index,
                        tab_name: current_tab_name.clone(),
                        tab_focused: current_tab_focused,
                    },
                );
            }
        }
    }

    tabs.sort_by(|left, right| left.index.cmp(&right.index));
    let mut pane_entries = panes.into_values().collect::<Vec<_>>();
    pane_entries.sort_by(|left, right| {
        left.tab_index
            .cmp(&right.tab_index)
            .then_with(|| {
                pane_id_number_u64(&left.pane_id)
                    .unwrap_or(u64::MAX)
                    .cmp(&pane_id_number_u64(&right.pane_id).unwrap_or(u64::MAX))
            })
            .then_with(|| left.pane_id.cmp(&right.pane_id))
    });

    LayoutSnapshot {
        pane_ids,
        tabs,
        panes: pane_entries,
    }
}

#[cfg(unix)]
fn parse_layout_pane_ids(layout: &str) -> HashSet<String> {
    parse_layout_snapshot(layout).pane_ids
}

#[cfg(unix)]
fn extract_pane_ids_from_layout_line(line: &str) -> Vec<String> {
    let mut pane_ids = extract_quoted_flag_values(line, "--pane-id");
    pane_ids.extend(extract_legacy_flag_values(line, "--pane-id\""));
    pane_ids.extend(extract_attr_values(line, "pane_id"));
    pane_ids.extend(extract_attr_values(line, "pane-id"));
    pane_ids.sort();
    pane_ids.dedup();
    pane_ids
}

#[cfg(unix)]
fn line_is_tab_decl(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("tab ") || trimmed == "tab" || trimmed.starts_with("tab\t")
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
fn extract_legacy_flag_values(line: &str, marker: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = line;
    while let Some(idx) = cursor.find(marker) {
        let tail = &cursor[idx + marker.len()..];
        let Some(start_quote) = tail.find('"') else {
            break;
        };
        let value_tail = &tail[start_quote + 1..];
        let Some(end_quote) = value_tail.find('"') else {
            break;
        };
        let value = value_tail[..end_quote].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        cursor = &value_tail[end_quote + 1..];
    }
    values
}

#[cfg(unix)]
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

#[cfg(unix)]
fn pane_id_number_u64(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

#[cfg(unix)]
fn layout_signature(tabs: &[LayoutTab], panes: &[LayoutPane]) -> u64 {
    let mut hasher = DefaultHasher::new();
    tabs.hash(&mut hasher);
    panes.hash(&mut hasher);
    hasher.finish()
}

#[cfg(unix)]
fn normalize_publisher_agent_id(session_id: &str, hello: &HelloPayload) -> Option<String> {
    let mut candidate = hello.agent_id.clone().unwrap_or_default();
    if candidate.trim().is_empty() {
        let pane = hello.pane_id.clone().unwrap_or_default();
        if pane.trim().is_empty() {
            return None;
        }
        candidate = format!("{session_id}::{pane}");
    }
    if !agent_in_session(session_id, &candidate) {
        return None;
    }
    Some(candidate)
}

#[cfg(unix)]
fn agent_in_session(session_id: &str, agent_id: &str) -> bool {
    let prefix = format!("{session_id}::");
    agent_id.starts_with(&prefix)
}

#[cfg(unix)]
fn pane_from_agent_id(agent_id: &str) -> String {
    agent_id
        .split_once("::")
        .map(|(_, pane)| pane.to_string())
        .unwrap_or_default()
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use tokio::net::{
        unix::{OwnedReadHalf, OwnedWriteHalf},
        UnixStream,
    };

    fn test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("aoc-pulse-hub-test-{name}-{nanos}"))
            .join("pulse.sock")
    }

    async fn wait_for_socket(path: &Path) {
        for _ in 0..100 {
            if path.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("socket did not appear: {}", path.display());
    }

    fn hello_envelope(
        session: &str,
        sender: &str,
        role: &str,
        agent_id: Option<&str>,
    ) -> WireEnvelope {
        WireEnvelope {
            version: ProtocolVersion::CURRENT,
            session_id: session.to_string(),
            sender_id: sender.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: None,
            msg: WireMsg::Hello(HelloPayload {
                client_id: sender.to_string(),
                role: role.to_string(),
                capabilities: vec!["pulse".to_string()],
                agent_id: agent_id.map(str::to_string),
                pane_id: Some("12".to_string()),
                project_root: Some("/tmp/repo".to_string()),
            }),
        }
    }

    fn upsert_delta(session: &str, sender: &str, agent_id: &str, lifecycle: &str) -> WireEnvelope {
        WireEnvelope {
            version: ProtocolVersion::CURRENT,
            session_id: session.to_string(),
            sender_id: sender.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: None,
            msg: WireMsg::Delta(DeltaPayload {
                seq: 0,
                changes: vec![StateChange {
                    op: StateChangeOp::Upsert,
                    agent_id: agent_id.to_string(),
                    state: Some(AgentState {
                        agent_id: agent_id.to_string(),
                        session_id: session.to_string(),
                        pane_id: "12".to_string(),
                        lifecycle: lifecycle.to_string(),
                        snippet: Some("working".to_string()),
                        last_heartbeat_ms: Some(now_ms()),
                        last_activity_ms: Some(now_ms()),
                        updated_at_ms: Some(now_ms()),
                        source: None,
                    }),
                }],
            }),
        }
    }

    fn command_envelope(
        session: &str,
        sender: &str,
        request_id: &str,
        command: &str,
        target_agent_id: Option<&str>,
        args: serde_json::Value,
    ) -> WireEnvelope {
        WireEnvelope {
            version: ProtocolVersion::CURRENT,
            session_id: session.to_string(),
            sender_id: sender.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some(request_id.to_string()),
            msg: WireMsg::Command(CommandPayload {
                command: command.to_string(),
                target_agent_id: target_agent_id.map(str::to_string),
                args,
            }),
        }
    }

    async fn connect_client(
        path: &Path,
        hello: WireEnvelope,
    ) -> (BufReader<OwnedReadHalf>, OwnedWriteHalf) {
        let stream = UnixStream::connect(path)
            .await
            .unwrap_or_else(|err| panic!("connect failed: {err}"));
        let (reader, mut writer) = stream.into_split();
        let frame = encode_frame(&hello, DEFAULT_MAX_FRAME_BYTES).expect("hello encode");
        writer.write_all(&frame).await.expect("hello write");
        writer.flush().await.expect("hello flush");
        (BufReader::new(reader), writer)
    }

    async fn send_frame(writer: &mut OwnedWriteHalf, envelope: &WireEnvelope) {
        let frame = encode_frame(envelope, DEFAULT_MAX_FRAME_BYTES).expect("encode");
        writer.write_all(&frame).await.expect("write");
        writer.flush().await.expect("flush");
    }

    async fn read_frame(reader: &mut BufReader<OwnedReadHalf>) -> WireEnvelope {
        let mut line = Vec::new();
        let read =
            tokio::time::timeout(Duration::from_secs(3), reader.read_until(b'\n', &mut line))
                .await
                .expect("read timeout")
                .expect("read error");
        assert!(read > 0, "unexpected EOF");
        decode_frame(&line, DEFAULT_MAX_FRAME_BYTES).expect("decode")
    }

    async fn read_frame_timeout(
        reader: &mut BufReader<OwnedReadHalf>,
        timeout: Duration,
    ) -> Option<WireEnvelope> {
        let mut line = Vec::new();
        let read = match tokio::time::timeout(timeout, reader.read_until(b'\n', &mut line)).await {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => return None,
            Err(_) => return None,
        };
        if read == 0 {
            return None;
        }
        decode_frame(&line, DEFAULT_MAX_FRAME_BYTES).ok()
    }

    async fn launch_hub(
        name: &str,
        session: &str,
        stale_after: Option<Duration>,
    ) -> (
        PathBuf,
        watch::Sender<bool>,
        tokio::task::JoinHandle<io::Result<()>>,
    ) {
        let path = test_path(name);
        let cfg = PulseUdsConfig {
            session_id: session.to_string(),
            socket_path: path.clone(),
            stale_after,
            write_timeout: Duration::from_secs(1),
            queue_capacity: 32,
        };
        let (tx, rx) = watch::channel(false);
        let handle = tokio::spawn(run(cfg, rx));
        wait_for_socket(&path).await;
        (path, tx, handle)
    }

    #[test]
    fn parse_layout_extracts_pane_ids() {
        let layout = r#"
tab name="AOC"
pane command="bash" args "-lc" "something" --pane-id" "12"
pane pane_id="44" name="Agent"
"#;
        let panes = parse_layout_pane_ids(layout);
        assert!(panes.contains("12"));
        assert!(panes.contains("44"));
    }

    #[test]
    fn parse_layout_snapshot_tracks_tabs_and_focus() {
        let layout = r#"
tab name="Agent" focus=true
 pane pane_id="12" name="Agent [core]"
tab name="Review"
 pane command="runner" args "--pane-id" "19"
"#;

        let snapshot = parse_layout_snapshot(layout);
        assert_eq!(snapshot.tabs.len(), 2);
        assert_eq!(snapshot.tabs[0].index, 1);
        assert!(snapshot.tabs[0].focused);
        assert_eq!(snapshot.panes.len(), 2);
        assert_eq!(snapshot.panes[0].pane_id, "12");
        assert_eq!(snapshot.panes[0].tab_index, 1);
        assert!(snapshot.panes[0].tab_focused);
    }

    #[test]
    fn topic_filter_defaults_and_selective_subscribe() {
        let baseline = TopicFilter::from_subscribe(&SubscribePayload {
            topics: Vec::new(),
            since_seq: None,
        });
        assert!(baseline.allows(PulseTopic::AgentState));
        assert!(baseline.allows(PulseTopic::CommandResult));
        assert!(!baseline.allows(PulseTopic::LayoutState));

        let selective = TopicFilter::from_subscribe(&SubscribePayload {
            topics: vec!["layout_state".to_string()],
            since_seq: None,
        });
        assert!(!selective.allows(PulseTopic::AgentState));
        assert!(selective.allows(PulseTopic::LayoutState));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn prune_closed_panes_removes_state_immediately() {
        let hub = Arc::new(PulseUdsHub::new(PulseUdsConfig {
            session_id: "session-prune".to_string(),
            socket_path: test_path("prune-state"),
            stale_after: Some(Duration::from_secs(10)),
            write_timeout: Duration::from_secs(1),
            queue_capacity: 8,
        }));

        {
            let mut state = hub.state.write().await;
            state.insert(
                "session-prune::12".to_string(),
                AgentRecord {
                    state: AgentState {
                        agent_id: "session-prune::12".to_string(),
                        session_id: "session-prune".to_string(),
                        pane_id: "12".to_string(),
                        lifecycle: "running".to_string(),
                        snippet: None,
                        last_heartbeat_ms: Some(now_ms()),
                        last_activity_ms: None,
                        updated_at_ms: Some(now_ms()),
                        source: None,
                    },
                    last_heartbeat: Instant::now(),
                },
            );
            state.insert(
                "session-prune::99".to_string(),
                AgentRecord {
                    state: AgentState {
                        agent_id: "session-prune::99".to_string(),
                        session_id: "session-prune".to_string(),
                        pane_id: "99".to_string(),
                        lifecycle: "running".to_string(),
                        snippet: None,
                        last_heartbeat_ms: Some(now_ms()),
                        last_activity_ms: None,
                        updated_at_ms: Some(now_ms()),
                        source: None,
                    },
                    last_heartbeat: Instant::now(),
                },
            );
        }

        let (_, closed_first) = hub
            .reconcile_layout_panes(HashSet::from(["12".to_string(), "99".to_string()]))
            .await;
        assert!(closed_first.is_empty());

        let (_, closed_second) = hub
            .reconcile_layout_panes(HashSet::from(["12".to_string()]))
            .await;
        assert_eq!(closed_second, vec!["99".to_string()]);

        hub.prune_closed_panes(closed_second).await;
        let snapshot = hub.build_snapshot_envelope().await;
        let WireMsg::Snapshot(payload) = snapshot.msg else {
            panic!("expected snapshot")
        };
        assert_eq!(payload.states.len(), 1);
        assert_eq!(payload.states[0].pane_id, "12");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn snapshot_on_connect_and_ordered_deltas() {
        let session = "pulse-test-session";
        let (path, shutdown_tx, handle) =
            launch_hub("snapshot-delta", session, Some(Duration::from_secs(2))).await;
        let agent = format!("{session}::12");

        let (_pub_reader, mut pub_writer) = connect_client(
            &path,
            hello_envelope(session, "pub-1", "publisher", Some(&agent)),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let first = upsert_delta(session, "pub-1", &agent, "running");
        send_frame(&mut pub_writer, &first).await;

        let (mut sub_reader, _sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;

        let snapshot = read_frame(&mut sub_reader).await;
        let WireMsg::Snapshot(snapshot_payload) = snapshot.msg else {
            panic!("expected snapshot")
        };
        assert_eq!(snapshot_payload.seq, 1);
        assert_eq!(snapshot_payload.states.len(), 1);
        assert_eq!(snapshot_payload.states[0].agent_id, agent);
        assert_eq!(snapshot_payload.states[0].lifecycle, "running");

        let second = upsert_delta(session, "pub-1", &agent, "needs_input");
        send_frame(&mut pub_writer, &second).await;

        let delta = read_frame(&mut sub_reader).await;
        let WireMsg::Delta(delta_payload) = delta.msg else {
            panic!("expected delta")
        };
        assert_eq!(delta_payload.seq, 2);
        assert_eq!(delta_payload.changes.len(), 1);
        assert_eq!(delta_payload.changes[0].op, StateChangeOp::Upsert);
        assert_eq!(delta_payload.changes[0].agent_id, agent);
        assert_eq!(
            delta_payload.changes[0]
                .state
                .as_ref()
                .expect("upsert state")
                .lifecycle,
            "needs_input"
        );

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop_agent_command_routes_and_acks() {
        let session = "pulse-command-session";
        let (path, shutdown_tx, handle) =
            launch_hub("command-route", session, Some(Duration::from_secs(2))).await;
        let agent = format!("{session}::12");

        let (mut pub_reader, _pub_writer) = connect_client(
            &path,
            hello_envelope(session, "pub-1", "publisher", Some(&agent)),
        )
        .await;
        let (mut sub_reader, mut sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let _ = read_frame(&mut sub_reader).await;

        let command = command_envelope(
            session,
            "sub-1",
            "req-stop-1",
            "stop_agent",
            Some(&agent),
            serde_json::json!({"reason": "user_request"}),
        );
        send_frame(&mut sub_writer, &command).await;

        let routed = read_frame(&mut pub_reader).await;
        let WireMsg::Command(payload) = routed.msg else {
            panic!("expected command routed to publisher")
        };
        assert_eq!(payload.command, "stop_agent");
        assert_eq!(payload.target_agent_id.as_deref(), Some(agent.as_str()));

        let ack = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = ack.msg else {
            panic!("expected command_result ack")
        };
        assert_eq!(payload.command, "stop_agent");
        assert_eq!(payload.status, "accepted");

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn duplicate_request_id_is_idempotent() {
        let session = "pulse-command-idempotent";
        let (path, shutdown_tx, handle) =
            launch_hub("command-idempotent", session, Some(Duration::from_secs(2))).await;
        let agent = format!("{session}::12");

        let (mut pub_reader, _pub_writer) = connect_client(
            &path,
            hello_envelope(session, "pub-1", "publisher", Some(&agent)),
        )
        .await;
        let (mut sub_reader, mut sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let _ = read_frame(&mut sub_reader).await;

        let command = command_envelope(
            session,
            "sub-1",
            "req-stop-dup",
            "stop_agent",
            Some(&agent),
            serde_json::json!({}),
        );
        send_frame(&mut sub_writer, &command).await;
        let _ = read_frame(&mut pub_reader).await;
        let _ = read_frame(&mut sub_reader).await;

        send_frame(&mut sub_writer, &command).await;
        let duplicate_ack = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = duplicate_ack.msg else {
            panic!("expected command_result for duplicate")
        };
        assert_eq!(payload.status, "accepted");

        let second = read_frame_timeout(&mut pub_reader, Duration::from_millis(250)).await;
        assert!(second.is_none(), "duplicate request should not be rerouted");

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn command_errors_include_code_and_message() {
        let session = "pulse-command-errors";
        let (path, shutdown_tx, handle) =
            launch_hub("command-errors", session, Some(Duration::from_secs(2))).await;

        let (mut sub_reader, mut sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let _ = read_frame(&mut sub_reader).await;

        let missing_target = command_envelope(
            session,
            "sub-1",
            "req-missing-target",
            "stop_agent",
            None,
            serde_json::json!({}),
        );
        send_frame(&mut sub_writer, &missing_target).await;
        let error = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = error.msg else {
            panic!("expected command_result error")
        };
        assert_eq!(payload.status, "error");
        assert_eq!(
            payload.error.as_ref().map(|value| value.code.as_str()),
            Some("invalid_target")
        );

        let focus_without_args = command_envelope(
            session,
            "sub-1",
            "req-focus-invalid",
            "focus_tab",
            None,
            serde_json::json!({}),
        );
        send_frame(&mut sub_writer, &focus_without_args).await;
        let focus_error = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = focus_error.msg else {
            panic!("expected focus command_result error")
        };
        assert_eq!(payload.status, "error");
        assert_eq!(
            payload.error.as_ref().map(|value| value.code.as_str()),
            Some("invalid_args")
        );

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stale_eviction_emits_remove_delta() {
        let session = "pulse-stale-session";
        let (path, shutdown_tx, handle) =
            launch_hub("stale-evict", session, Some(Duration::from_millis(200))).await;
        let agent = format!("{session}::12");

        let (mut sub_reader, _sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let initial = read_frame(&mut sub_reader).await;
        match initial.msg {
            WireMsg::Snapshot(payload) => assert!(payload.states.is_empty()),
            _ => panic!("expected initial snapshot"),
        }

        let (_pub_reader, mut pub_writer) = connect_client(
            &path,
            hello_envelope(session, "pub-1", "publisher", Some(&agent)),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        send_frame(
            &mut pub_writer,
            &upsert_delta(session, "pub-1", &agent, "running"),
        )
        .await;

        let upsert = read_frame(&mut sub_reader).await;
        let WireMsg::Delta(payload) = upsert.msg else {
            panic!("expected delta upsert")
        };
        assert_eq!(payload.changes.len(), 1);
        assert_eq!(payload.changes[0].op, StateChangeOp::Upsert);

        let remove = read_frame(&mut sub_reader).await;
        let WireMsg::Delta(payload) = remove.msg else {
            panic!("expected delta remove")
        };
        assert!(payload
            .changes
            .iter()
            .any(|change| change.op == StateChangeOp::Remove && change.agent_id == agent));

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rejects_cross_session_publishers() {
        let session = "pulse-main-session";
        let (path, shutdown_tx, handle) =
            launch_hub("session-scope", session, Some(Duration::from_secs(2))).await;

        let (_rogue_reader, mut rogue_writer) = connect_client(
            &path,
            hello_envelope(
                "different-session",
                "rogue-pub",
                "publisher",
                Some("different-session::12"),
            ),
        )
        .await;

        let rogue_delta = upsert_delta(
            "different-session",
            "rogue-pub",
            "different-session::12",
            "running",
        );
        if let Ok(frame) = encode_frame(&rogue_delta, DEFAULT_MAX_FRAME_BYTES) {
            let _ = rogue_writer.write_all(&frame).await;
            let _ = rogue_writer.flush().await;
        }

        let (mut sub_reader, _sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let snapshot = read_frame(&mut sub_reader).await;
        let WireMsg::Snapshot(payload) = snapshot.msg else {
            panic!("expected snapshot")
        };
        assert!(payload.states.is_empty());

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn pulse_stress_agents_tab_churn_benchmark() {
        let session = "pulse-bench-session";
        let hub = Arc::new(PulseUdsHub::new(PulseUdsConfig {
            session_id: session.to_string(),
            socket_path: test_path("bench-churn"),
            stale_after: Some(Duration::from_secs(5)),
            write_timeout: Duration::from_secs(1),
            queue_capacity: 256,
        }));

        for agents in [5usize, 10, 20] {
            let start = Instant::now();
            for tick in 0..120usize {
                let mut open_panes = HashSet::new();
                for idx in 0..agents {
                    let pane = (idx + 1).to_string();
                    if (tick + idx) % 4 != 0 {
                        open_panes.insert(pane.clone());
                    }
                    let agent_id = format!("{session}::{pane}");
                    let state = AgentState {
                        agent_id: agent_id.clone(),
                        session_id: session.to_string(),
                        pane_id: pane.clone(),
                        lifecycle: if (tick + idx) % 11 == 0 {
                            "needs_input".to_string()
                        } else {
                            "running".to_string()
                        },
                        snippet: Some(format!("tick-{tick}-agent-{idx}")),
                        last_heartbeat_ms: Some(now_ms()),
                        last_activity_ms: Some(now_ms()),
                        updated_at_ms: Some(now_ms()),
                        source: Some(serde_json::json!({
                            "benchmark": true,
                            "parser_confidence": (idx % 3) + 1
                        })),
                    };
                    hub.apply_delta(
                        &agent_id,
                        DeltaPayload {
                            seq: tick as u64,
                            changes: vec![StateChange {
                                op: StateChangeOp::Upsert,
                                agent_id: agent_id.clone(),
                                state: Some(state),
                            }],
                        },
                    )
                    .await;
                }

                let (_, closed) = hub.reconcile_layout_panes(open_panes).await;
                if !closed.is_empty() {
                    hub.prune_closed_panes(closed).await;
                }
            }

            let elapsed = start.elapsed();
            let remaining = hub.state.read().await.len();
            assert!(
                elapsed < Duration::from_secs(6),
                "scenario agents={agents} exceeded budget: {elapsed:?}"
            );
            assert!(
                remaining <= agents,
                "state leak detected: {remaining} > {agents}"
            );
            println!(
                "bench scenario agents={} elapsed_ms={} remaining_state={} queue_drops={} backpressure={}",
                agents,
                elapsed.as_millis(),
                remaining,
                hub.queue_drop_count.load(Ordering::Relaxed),
                hub.backpressure_count.load(Ordering::Relaxed)
            );
        }
    }
}
