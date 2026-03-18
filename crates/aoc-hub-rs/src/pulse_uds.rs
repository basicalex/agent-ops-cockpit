use aoc_core::{
    pulse_ipc::{
        decode_frame, encode_frame, AgentState, CommandError, CommandPayload, CommandResultPayload,
        ConsultationRequestPayload, ConsultationResponsePayload, ConsultationStatus, DeltaPayload,
        HeartbeatPayload, HelloPayload, LayoutPane, LayoutStatePayload, LayoutTab,
        ObserverTimelinePayload, ProtocolVersion, SnapshotPayload, StateChange, StateChangeOp,
        SubscribePayload, WireEnvelope, WireMsg, CURRENT_PROTOCOL_VERSION, DEFAULT_MAX_FRAME_BYTES,
    },
    session_overseer::{
        AttentionLevel, AttentionSignal, DriftRisk, DuplicateWorkSignal, ManagerCommand,
        ManagerCommandError, ManagerCommandResult, ManagerCommandStatus, ObserverEvent,
        ObserverEventKind, ObserverSnapshot, ObserverTimelineEntry, OverseerRetentionPolicy,
        OverseerSourceKind, PlanAlignment, WorkerSnapshot, WorkerStatus,
    },
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
const LAYOUT_WATCH_INTERVAL_MS_DEFAULT: u64 = 3000;
const LAYOUT_WATCH_INTERVAL_MS_MIN: u64 = 500;
const LAYOUT_WATCH_INTERVAL_MS_MAX: u64 = 30000;
const LAYOUT_WATCH_IDLE_INTERVAL_MS_DEFAULT: u64 = 12000;
const LAYOUT_WATCH_IDLE_INTERVAL_MS_MIN: u64 = 1000;
const LAYOUT_WATCH_IDLE_INTERVAL_MS_MAX: u64 = 60000;

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
    if resolve_layout_watch_enabled() {
        hub.clone().spawn_layout_watcher(shutdown.clone());
    } else {
        info!(
            event = "pulse_layout_watcher_disabled",
            session_id = %config.session_id,
            reason = "AOC_PULSE_LAYOUT_WATCH_ENABLED=0"
        );
    }

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
    ObserverSnapshot,
    ObserverTimeline,
    CommandResult,
    ConsultationRequest,
    ConsultationResponse,
    LayoutState,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct TopicFilter {
    agent_state: bool,
    observer_snapshot: bool,
    observer_timeline: bool,
    command_result: bool,
    consultation_request: bool,
    consultation_response: bool,
    layout_state: bool,
}

#[cfg(unix)]
impl TopicFilter {
    fn baseline() -> Self {
        Self {
            agent_state: true,
            observer_snapshot: false,
            observer_timeline: false,
            command_result: true,
            consultation_request: false,
            consultation_response: false,
            layout_state: false,
        }
    }

    fn from_subscribe(payload: &SubscribePayload) -> Self {
        if payload.topics.is_empty() {
            return Self::baseline();
        }

        let mut filter = Self {
            agent_state: false,
            observer_snapshot: false,
            observer_timeline: false,
            command_result: false,
            consultation_request: false,
            consultation_response: false,
            layout_state: false,
        };
        for topic in &payload.topics {
            match topic.trim().to_ascii_lowercase().as_str() {
                "agent_state" | "snapshot" | "delta" => filter.agent_state = true,
                "observer_snapshot" => filter.observer_snapshot = true,
                "observer_timeline" => filter.observer_timeline = true,
                "command_result" => filter.command_result = true,
                "consultation_request" => filter.consultation_request = true,
                "consultation_response" => filter.consultation_response = true,
                "layout_state" => filter.layout_state = true,
                _ => {}
            }
        }
        filter
    }

    fn allows(&self, topic: PulseTopic) -> bool {
        match topic {
            PulseTopic::AgentState => self.agent_state,
            PulseTopic::ObserverSnapshot => self.observer_snapshot,
            PulseTopic::ObserverTimeline => self.observer_timeline,
            PulseTopic::CommandResult => self.command_result,
            PulseTopic::ConsultationRequest => self.consultation_request,
            PulseTopic::ConsultationResponse => self.consultation_response,
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
    overseer_retention: OverseerRetentionPolicy,
    observer_timeline: RwLock<VecDeque<ObserverTimelineEntry>>,
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
            overseer_retention: OverseerRetentionPolicy::default(),
            observer_timeline: RwLock::new(VecDeque::new()),
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

    async fn build_observer_snapshot_envelope(&self) -> WireEnvelope {
        let payload = self.build_observer_snapshot().await;
        self.make_envelope("aoc-hub", None, WireMsg::ObserverSnapshot(payload))
    }

    async fn build_observer_timeline_envelope(&self) -> WireEnvelope {
        let entries = {
            let timeline = self.observer_timeline.read().await;
            timeline.iter().cloned().collect::<Vec<_>>()
        };
        self.make_envelope(
            "aoc-hub",
            None,
            WireMsg::ObserverTimeline(ObserverTimelinePayload {
                session_id: self.config.session_id.clone(),
                generated_at_ms: Some(now_ms()),
                entries,
            }),
        )
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

        if filter.observer_snapshot {
            let snapshot = self.build_observer_snapshot_envelope().await;
            let _ = self.send_to_conn(conn_id, snapshot).await;
        }

        if filter.observer_timeline {
            let timeline = self.build_observer_timeline_envelope().await;
            let _ = self.send_to_conn(conn_id, timeline).await;
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

        if filter.observer_snapshot {
            let snapshot = self.build_observer_snapshot_envelope().await;
            let _ = self.send_to_conn(conn_id, snapshot).await;
        }

        if filter.observer_timeline {
            let timeline = self.build_observer_timeline_envelope().await;
            let _ = self.send_to_conn(conn_id, timeline).await;
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

    async fn build_observer_snapshot(&self) -> ObserverSnapshot {
        let now = now_ms();
        let mut workers = {
            let state = self.state.read().await;
            let mut workers = state
                .values()
                .filter_map(|record| {
                    worker_snapshot_from_agent_state(&record.state)
                        .map(|snapshot| enrich_worker_snapshot(snapshot, &record.state, now))
                })
                .collect::<Vec<_>>();
            apply_duplicate_work_heuristics(&mut workers);
            workers
        };
        workers.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        let timeline = {
            let timeline = self.observer_timeline.read().await;
            timeline.iter().cloned().collect::<Vec<_>>()
        };

        ObserverSnapshot {
            schema_version: 1,
            session_id: self.config.session_id.clone(),
            generated_at_ms: Some(now),
            workers,
            timeline,
            degraded_reason: None,
        }
    }

    async fn emit_observer_updates(&self) {
        let snapshot = self.build_observer_snapshot_envelope().await;
        self.broadcast_to_subscribers(snapshot).await;
        let timeline = self.build_observer_timeline_envelope().await;
        self.broadcast_to_subscribers(timeline).await;
    }

    async fn append_observer_events(&self, events: &[ObserverEvent]) {
        if events.is_empty() {
            return;
        }

        let mut timeline = self.observer_timeline.write().await;
        for (index, event) in events.iter().enumerate() {
            timeline.push_front(observer_timeline_entry_from_event(event, index));
        }
        while timeline.len() > self.overseer_retention.max_timeline_entries {
            timeline.pop_back();
        }
    }

    async fn append_command_result_event(
        &self,
        target_agent_id: &str,
        command: ManagerCommand,
        payload: &CommandResultPayload,
    ) {
        let snapshot = {
            let state = self.state.read().await;
            state
                .get(target_agent_id)
                .and_then(|record| worker_snapshot_from_agent_state(&record.state))
        };
        let pane_id = snapshot
            .as_ref()
            .map(|value| value.pane_id.clone())
            .unwrap_or_else(|| pane_from_agent_id(target_agent_id));
        let status = match payload.status.trim().to_ascii_lowercase().as_str() {
            "accepted" => ManagerCommandStatus::Accepted,
            "ok" | "completed" => ManagerCommandStatus::Completed,
            "rejected" => ManagerCommandStatus::Rejected,
            _ => ManagerCommandStatus::Failed,
        };
        let error = payload.error.as_ref().map(|err| ManagerCommandError {
            code: err.code.clone(),
            message: err.message.clone(),
        });
        let event = ObserverEvent {
            schema_version: 1,
            kind: ObserverEventKind::CommandResult,
            session_id: self.config.session_id.clone(),
            agent_id: target_agent_id.to_string(),
            pane_id,
            source: OverseerSourceKind::Hub,
            summary: payload.message.clone(),
            reason: None,
            snapshot,
            command: Some(command),
            command_result: Some(ManagerCommandResult {
                status,
                message: payload.message.clone(),
                error,
            }),
            emitted_at_ms: Some(now_ms()),
        };
        self.append_observer_events(&[event]).await;
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
            self.emit_observer_updates().await;
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
        let idle_interval_ms = resolve_layout_watch_idle_interval_ms(interval_ms);
        let idle_interval = Duration::from_millis(idle_interval_ms);
        let session_id = self.config.session_id.clone();
        tokio::spawn(async move {
            let jitter_ms = session_interval_jitter_ms(&session_id, interval_ms);
            let mut ticker = tokio::time::interval_at(
                tokio::time::Instant::now() + Duration::from_millis(jitter_ms),
                interval,
            );
            let mut previous_tick = Instant::now();
            let mut last_layout_poll = Instant::now() - idle_interval;
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
                idle_interval_ms,
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

                        let should_poll_fast = self.has_layout_state_subscribers().await;
                        let target_interval = if should_poll_fast {
                            interval
                        } else {
                            idle_interval
                        };
                        if now.duration_since(last_layout_poll) < target_interval {
                            continue;
                        }
                        last_layout_poll = now;

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

    async fn has_layout_state_subscribers(&self) -> bool {
        self.subscribers
            .read()
            .await
            .values()
            .any(|entry| entry.topics.allows(PulseTopic::LayoutState))
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
        self.broadcast_delta(changes.clone()).await;
        if !changes.is_empty() {
            self.emit_observer_updates().await;
        }
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
        let mut observer_events = Vec::new();
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
                        observer_events.extend(observer_events_from_agent_state(&next_state));
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
        self.broadcast_delta(outgoing.clone()).await;
        if !outgoing.is_empty() {
            self.append_observer_events(&observer_events).await;
            self.emit_observer_updates().await;
        }
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
            "stop_agent"
            | "run_observer"
            | "mind_ingest_event"
            | "mind_compaction_checkpoint"
            | "mind_handoff"
            | "mind_resume"
            | "mind_finalize"
            | "mind_finalize_session"
            | "mind_t3_requeue"
            | "mind_handshake_rebuild"
            | "insight_ingest"
            | "insight_handoff"
            | "insight_resume"
            | "insight_status"
            | "insight_dispatch"
            | "insight_bootstrap"
            | "insight_detached_dispatch"
            | "insight_detached_status"
            | "insight_detached_cancel"
            | "request_status_update"
            | "request_handoff"
            | "pause_and_summarize"
            | "run_validation"
            | "switch_focus"
            | "finalize_and_report" => {
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

    async fn route_consultation_request(
        &self,
        source_conn_id: &str,
        envelope: WireEnvelope,
        payload: ConsultationRequestPayload,
    ) {
        if payload.consultation_id.trim().is_empty() {
            self.send_consultation_response(
                source_conn_id,
                envelope.request_id,
                ConsultationResponsePayload {
                    consultation_id: String::new(),
                    requesting_agent_id: payload.requesting_agent_id,
                    responding_agent_id: payload.target_agent_id,
                    status: ConsultationStatus::Failed,
                    packet: None,
                    message: Some("consultation_id is required".to_string()),
                    error: Some(CommandError {
                        code: "invalid_consultation_id".to_string(),
                        message: "consultation_id is required".to_string(),
                    }),
                },
            )
            .await;
            return;
        }

        if payload.target_agent_id.trim().is_empty()
            || !agent_in_session(&self.config.session_id, &payload.target_agent_id)
        {
            self.send_consultation_response(
                source_conn_id,
                envelope.request_id,
                ConsultationResponsePayload {
                    consultation_id: payload.consultation_id,
                    requesting_agent_id: payload.requesting_agent_id,
                    responding_agent_id: payload.target_agent_id,
                    status: ConsultationStatus::Failed,
                    packet: None,
                    message: Some("target_agent_id is required and must match session".to_string()),
                    error: Some(CommandError {
                        code: "invalid_target".to_string(),
                        message: "target_agent_id is required and must match session".to_string(),
                    }),
                },
            )
            .await;
            return;
        }

        if payload.requesting_agent_id.trim().is_empty()
            || !agent_in_session(&self.config.session_id, &payload.requesting_agent_id)
        {
            self.send_consultation_response(
                source_conn_id,
                envelope.request_id,
                ConsultationResponsePayload {
                    consultation_id: payload.consultation_id,
                    requesting_agent_id: payload.requesting_agent_id,
                    responding_agent_id: payload.target_agent_id,
                    status: ConsultationStatus::Failed,
                    packet: None,
                    message: Some(
                        "requesting_agent_id is required and must match session".to_string(),
                    ),
                    error: Some(CommandError {
                        code: "invalid_requester".to_string(),
                        message: "requesting_agent_id is required and must match session"
                            .to_string(),
                    }),
                },
            )
            .await;
            return;
        }

        if payload.packet.identity.session_id != self.config.session_id
            || payload.packet.identity.agent_id != payload.requesting_agent_id
        {
            self.send_consultation_response(
                source_conn_id,
                envelope.request_id,
                ConsultationResponsePayload {
                    consultation_id: payload.consultation_id,
                    requesting_agent_id: payload.requesting_agent_id,
                    responding_agent_id: payload.target_agent_id,
                    status: ConsultationStatus::Failed,
                    packet: None,
                    message: Some(
                        "consultation packet identity must match session and requesting agent"
                            .to_string(),
                    ),
                    error: Some(CommandError {
                        code: "invalid_packet_identity".to_string(),
                        message:
                            "consultation packet identity must match session and requesting agent"
                                .to_string(),
                    }),
                },
            )
            .await;
            return;
        }

        let targets = {
            let publishers = self.publishers.read().await;
            publishers
                .get(&payload.target_agent_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>()
        };
        if targets.is_empty() {
            self.send_consultation_response(
                source_conn_id,
                envelope.request_id,
                ConsultationResponsePayload {
                    consultation_id: payload.consultation_id,
                    requesting_agent_id: payload.requesting_agent_id,
                    responding_agent_id: payload.target_agent_id,
                    status: ConsultationStatus::Failed,
                    packet: None,
                    message: Some("target publisher is not connected".to_string()),
                    error: Some(CommandError {
                        code: "publisher_missing".to_string(),
                        message: "target publisher is not connected".to_string(),
                    }),
                },
            )
            .await;
            return;
        }

        let mut delivered = false;
        for conn_id in targets {
            delivered |= self
                .send_to_conn(
                    &conn_id,
                    self.make_envelope(
                        &envelope.sender_id,
                        envelope.request_id.clone(),
                        WireMsg::ConsultationRequest(payload.clone()),
                    ),
                )
                .await;
        }
        if !delivered {
            self.send_consultation_response(
                source_conn_id,
                envelope.request_id,
                ConsultationResponsePayload {
                    consultation_id: payload.consultation_id,
                    requesting_agent_id: payload.requesting_agent_id,
                    responding_agent_id: payload.target_agent_id,
                    status: ConsultationStatus::Failed,
                    packet: None,
                    message: Some("failed to deliver consultation request".to_string()),
                    error: Some(CommandError {
                        code: "publisher_unavailable".to_string(),
                        message: "failed to deliver consultation request".to_string(),
                    }),
                },
            )
            .await;
            return;
        }

        self.send_consultation_response(
            source_conn_id,
            envelope.request_id,
            ConsultationResponsePayload {
                consultation_id: payload.consultation_id,
                requesting_agent_id: payload.requesting_agent_id,
                responding_agent_id: payload.target_agent_id,
                status: ConsultationStatus::Accepted,
                packet: None,
                message: Some("consultation request forwarded".to_string()),
                error: None,
            },
        )
        .await;
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

    async fn send_consultation_response(
        &self,
        conn_id: &str,
        request_id: Option<String>,
        payload: ConsultationResponsePayload,
    ) {
        let envelope = self.make_envelope(
            "aoc-hub",
            request_id,
            WireMsg::ConsultationResponse(payload),
        );
        let _ = self.send_to_conn(conn_id, envelope).await;
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
                    if let Some(publisher_agent) = agent_id.as_deref() {
                        let overseer_command = match payload.command.as_str() {
                            "request_status_update" => Some(ManagerCommand::RequestStatusUpdate),
                            "request_handoff" => Some(ManagerCommand::RequestHandoff),
                            "pause_and_summarize" => Some(ManagerCommand::PauseAndSummarize),
                            "run_validation" => Some(ManagerCommand::RunValidation),
                            "switch_focus" => Some(ManagerCommand::SwitchFocus(Default::default())),
                            "finalize_and_report" => Some(ManagerCommand::FinalizeAndReport),
                            _ => None,
                        };
                        if let Some(command) = overseer_command {
                            self.append_command_result_event(publisher_agent, command, &payload)
                                .await;
                            self.emit_observer_updates().await;
                        }
                    }
                    let forwarded = self.make_envelope(
                        &envelope.sender_id,
                        envelope.request_id,
                        WireMsg::CommandResult(payload),
                    );
                    self.broadcast_to_subscribers(forwarded).await;
                }
                (ClientRole::Publisher, WireMsg::ConsultationResponse(payload)) => {
                    let forwarded = self.make_envelope(
                        &envelope.sender_id,
                        envelope.request_id,
                        WireMsg::ConsultationResponse(payload),
                    );
                    self.broadcast_to_subscribers(forwarded).await;
                }
                (ClientRole::Subscriber, WireMsg::Command(payload)) => {
                    self.route_command(&conn_id, envelope, payload).await;
                }
                (ClientRole::Subscriber, WireMsg::ConsultationRequest(payload)) => {
                    self.route_consultation_request(&conn_id, envelope, payload)
                        .await;
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

fn resolve_layout_watch_enabled() -> bool {
    std::env::var("AOC_PULSE_LAYOUT_WATCH_ENABLED")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn resolve_layout_watch_idle_interval_ms(active_interval_ms: u64) -> u64 {
    let fallback = active_interval_ms
        .saturating_mul(4)
        .max(LAYOUT_WATCH_IDLE_INTERVAL_MS_DEFAULT)
        .min(LAYOUT_WATCH_IDLE_INTERVAL_MS_MAX);
    std::env::var("AOC_PULSE_LAYOUT_IDLE_WATCH_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| {
            value.clamp(
                LAYOUT_WATCH_IDLE_INTERVAL_MS_MIN,
                LAYOUT_WATCH_IDLE_INTERVAL_MS_MAX,
            )
        })
        .unwrap_or(fallback)
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
        WireMsg::ObserverSnapshot(_) => Some(PulseTopic::ObserverSnapshot),
        WireMsg::ObserverTimeline(_) => Some(PulseTopic::ObserverTimeline),
        WireMsg::CommandResult(_) => Some(PulseTopic::CommandResult),
        WireMsg::ConsultationRequest(_) => Some(PulseTopic::ConsultationRequest),
        WireMsg::ConsultationResponse(_) => Some(PulseTopic::ConsultationResponse),
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

#[cfg(unix)]
fn worker_snapshot_from_agent_state(state: &AgentState) -> Option<WorkerSnapshot> {
    state
        .source
        .as_ref()
        .and_then(|source| {
            source
                .get("worker_snapshot")
                .or_else(|| source.get("session_overseer"))
        })
        .and_then(|value| serde_json::from_value::<WorkerSnapshot>(value.clone()).ok())
}

#[cfg(unix)]
fn current_tag_from_agent_state(state: &AgentState) -> Option<String> {
    state
        .source
        .as_ref()
        .and_then(|source| source.get("current_tag"))
        .and_then(|value| value.get("tag"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

#[cfg(unix)]
fn active_task_ids_from_agent_state(state: &AgentState) -> Vec<String> {
    let mut task_ids = Vec::new();
    let Some(source) = state.source.as_ref() else {
        return task_ids;
    };
    let Some(task_summaries) = source
        .get("task_summaries")
        .and_then(|value| value.as_object())
    else {
        return task_ids;
    };
    for payload in task_summaries.values() {
        let active = payload
            .get("active_tasks")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten();
        for task in active {
            let is_active_agent = task
                .get("active_agent")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if !is_active_agent {
                continue;
            }
            let Some(task_id) = task.get("id").and_then(|value| value.as_str()) else {
                continue;
            };
            let task_id = task_id.trim();
            if task_id.is_empty() || task_ids.iter().any(|existing| existing == task_id) {
                continue;
            }
            task_ids.push(task_id.to_string());
        }
    }
    task_ids
}

#[cfg(unix)]
fn enrich_worker_snapshot(
    mut snapshot: WorkerSnapshot,
    state: &AgentState,
    now_ms: i64,
) -> WorkerSnapshot {
    if snapshot.assignment.tag.is_none() {
        snapshot.assignment.tag = current_tag_from_agent_state(state);
    }
    if snapshot.assignment.task_id.is_none() {
        snapshot.assignment.task_id = active_task_ids_from_agent_state(state).into_iter().next();
    }
    if snapshot.last_update_at_ms.is_none() {
        snapshot.last_update_at_ms = state
            .updated_at_ms
            .or(state.last_activity_ms)
            .or(state.last_heartbeat_ms);
    }
    if snapshot.last_meaningful_progress_at_ms.is_none() {
        snapshot.last_meaningful_progress_at_ms = state.last_activity_ms.or(state.updated_at_ms);
    }

    snapshot.plan_alignment = derive_plan_alignment(&snapshot, state);
    snapshot.drift_risk = derive_drift_risk(&snapshot, now_ms);
    snapshot.attention = derive_attention_signal(&snapshot, now_ms);
    snapshot
}

#[cfg(unix)]
fn derive_plan_alignment(snapshot: &WorkerSnapshot, state: &AgentState) -> PlanAlignment {
    let current_tag = current_tag_from_agent_state(state);
    let active_task_ids = active_task_ids_from_agent_state(state);
    match (
        snapshot.assignment.task_id.as_deref(),
        snapshot.assignment.tag.as_deref(),
    ) {
        (Some(task_id), Some(tag))
            if current_tag.as_deref() == Some(tag)
                && active_task_ids.iter().any(|candidate| candidate == task_id) =>
        {
            PlanAlignment::High
        }
        (Some(_), Some(tag)) if current_tag.as_deref() == Some(tag) => PlanAlignment::Medium,
        (Some(_), None) => PlanAlignment::Medium,
        (None, Some(tag)) if current_tag.as_deref() == Some(tag) => PlanAlignment::Medium,
        (Some(_), Some(_)) => PlanAlignment::Low,
        (None, None) if snapshot.status == WorkerStatus::Offline => PlanAlignment::Unknown,
        _ => PlanAlignment::Unassigned,
    }
}

#[cfg(unix)]
fn derive_drift_risk(snapshot: &WorkerSnapshot, now_ms: i64) -> DriftRisk {
    if matches!(
        snapshot.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        return DriftRisk::High;
    }
    let stale_after = snapshot.stale_after_ms.unwrap_or(5 * 60 * 1000) as i64;
    let last_progress = snapshot
        .last_meaningful_progress_at_ms
        .or(snapshot.last_update_at_ms)
        .unwrap_or(now_ms);
    let age = now_ms.saturating_sub(last_progress);
    if age >= stale_after {
        return DriftRisk::High;
    }
    if snapshot.assignment.task_id.is_none() && snapshot.assignment.tag.is_none() {
        return DriftRisk::Medium;
    }
    if age >= stale_after / 2
        || matches!(
            snapshot.plan_alignment,
            PlanAlignment::Low | PlanAlignment::Unassigned
        )
    {
        return DriftRisk::Medium;
    }
    DriftRisk::Low
}

#[cfg(unix)]
fn derive_attention_signal(snapshot: &WorkerSnapshot, now_ms: i64) -> AttentionSignal {
    if matches!(
        snapshot.status,
        WorkerStatus::Blocked | WorkerStatus::NeedsInput
    ) {
        return AttentionSignal {
            level: AttentionLevel::Warn,
            kind: Some("blocked".to_string()),
            reason: snapshot
                .blocker
                .clone()
                .or_else(|| snapshot.summary.clone()),
        };
    }
    let stale_after = snapshot.stale_after_ms.unwrap_or(5 * 60 * 1000) as i64;
    let last_progress = snapshot
        .last_meaningful_progress_at_ms
        .or(snapshot.last_update_at_ms)
        .unwrap_or(now_ms);
    if now_ms.saturating_sub(last_progress) >= stale_after {
        return AttentionSignal {
            level: AttentionLevel::Warn,
            kind: Some("stale".to_string()),
            reason: Some(
                "worker has not reported meaningful progress within stale window".to_string(),
            ),
        };
    }
    if matches!(
        snapshot.plan_alignment,
        PlanAlignment::Low | PlanAlignment::Unassigned
    ) {
        return AttentionSignal {
            level: AttentionLevel::Info,
            kind: Some("plan_alignment".to_string()),
            reason: Some(
                "worker progress is weakly correlated with active task/tag context".to_string(),
            ),
        };
    }
    AttentionSignal::default()
}

#[cfg(unix)]
fn apply_duplicate_work_heuristics(workers: &mut [WorkerSnapshot]) {
    let mut task_to_agents: HashMap<String, Vec<String>> = HashMap::new();
    let mut file_to_agents: HashMap<String, Vec<String>> = HashMap::new();

    for worker in workers.iter() {
        if let Some(task_id) = worker.assignment.task_id.as_ref() {
            task_to_agents
                .entry(task_id.clone())
                .or_default()
                .push(worker.agent_id.clone());
        }
        for file in &worker.files_touched {
            file_to_agents
                .entry(file.clone())
                .or_default()
                .push(worker.agent_id.clone());
        }
    }

    for worker in workers.iter_mut() {
        let mut overlapping_files = worker
            .files_touched
            .iter()
            .filter(|file| {
                file_to_agents
                    .get(*file)
                    .map(|agents| agents.len() > 1)
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();
        overlapping_files.sort();
        overlapping_files.dedup();

        let mut overlapping_task_ids = Vec::new();
        if let Some(task_id) = worker.assignment.task_id.as_ref() {
            if task_to_agents
                .get(task_id)
                .map(|agents| agents.len() > 1)
                .unwrap_or(false)
            {
                overlapping_task_ids.push(task_id.clone());
            }
        }

        let mut other_agents = Vec::new();
        if let Some(task_id) = worker.assignment.task_id.as_ref() {
            if let Some(agents) = task_to_agents.get(task_id) {
                other_agents.extend(
                    agents
                        .iter()
                        .filter(|agent| *agent != &worker.agent_id)
                        .cloned(),
                );
            }
        }
        for file in &overlapping_files {
            if let Some(agents) = file_to_agents.get(file) {
                other_agents.extend(
                    agents
                        .iter()
                        .filter(|agent| *agent != &worker.agent_id)
                        .cloned(),
                );
            }
        }
        other_agents.sort();
        other_agents.dedup();

        if !overlapping_files.is_empty() || !overlapping_task_ids.is_empty() {
            worker.duplicate_work = Some(DuplicateWorkSignal {
                overlapping_files,
                overlapping_task_ids,
                other_agents,
            });
            if worker.drift_risk == DriftRisk::Low {
                worker.drift_risk = DriftRisk::Medium;
            }
            if worker.attention == AttentionSignal::default() {
                worker.attention = AttentionSignal {
                    level: AttentionLevel::Info,
                    kind: Some("duplicate_work".to_string()),
                    reason: Some("potential overlapping task/file ownership detected".to_string()),
                };
            }
        }
    }
}

#[cfg(unix)]
fn observer_events_from_agent_state(state: &AgentState) -> Vec<ObserverEvent> {
    state
        .source
        .as_ref()
        .and_then(|source| source.get("observer_events"))
        .and_then(|value| serde_json::from_value::<Vec<ObserverEvent>>(value.clone()).ok())
        .unwrap_or_default()
}

#[cfg(unix)]
fn observer_timeline_entry_from_event(
    event: &ObserverEvent,
    ordinal: usize,
) -> ObserverTimelineEntry {
    ObserverTimelineEntry {
        event_id: format!(
            "{}:{}:{}:{:?}",
            event.agent_id,
            event.emitted_at_ms.unwrap_or_default(),
            ordinal,
            event.kind
        ),
        session_id: event.session_id.clone(),
        agent_id: event.agent_id.clone(),
        kind: event.kind,
        source: event.source,
        summary: event.summary.clone(),
        reason: event.reason.clone(),
        attention: event
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.attention.clone())
            .and_then(|attention| {
                if attention == AttentionSignal::default() {
                    None
                } else {
                    Some(attention)
                }
            }),
        emitted_at_ms: event.emitted_at_ms,
    }
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

    fn consultation_request_envelope(
        session: &str,
        sender: &str,
        request_id: &str,
        consultation_id: &str,
        requesting_agent_id: &str,
        target_agent_id: &str,
    ) -> WireEnvelope {
        WireEnvelope {
            version: ProtocolVersion::CURRENT,
            session_id: session.to_string(),
            sender_id: sender.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some(request_id.to_string()),
            msg: WireMsg::ConsultationRequest(ConsultationRequestPayload {
                consultation_id: consultation_id.to_string(),
                requesting_agent_id: requesting_agent_id.to_string(),
                target_agent_id: target_agent_id.to_string(),
                packet: aoc_core::consultation_contracts::ConsultationPacket {
                    packet_id: format!("packet-{consultation_id}"),
                    identity: aoc_core::consultation_contracts::ConsultationIdentity {
                        session_id: session.to_string(),
                        agent_id: requesting_agent_id.to_string(),
                        pane_id: Some(pane_from_agent_id(requesting_agent_id)),
                        conversation_id: None,
                        role: Some("builder".to_string()),
                    },
                    summary: Some("Need peer review on bounded consultation transport".to_string()),
                    ..Default::default()
                },
            }),
        }
    }

    fn consultation_response_envelope(
        session: &str,
        sender: &str,
        request_id: &str,
        consultation_id: &str,
        requesting_agent_id: &str,
        responding_agent_id: &str,
    ) -> WireEnvelope {
        WireEnvelope {
            version: ProtocolVersion::CURRENT,
            session_id: session.to_string(),
            sender_id: sender.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: Some(request_id.to_string()),
            msg: WireMsg::ConsultationResponse(ConsultationResponsePayload {
                consultation_id: consultation_id.to_string(),
                requesting_agent_id: requesting_agent_id.to_string(),
                responding_agent_id: responding_agent_id.to_string(),
                status: ConsultationStatus::Completed,
                packet: Some(aoc_core::consultation_contracts::ConsultationPacket {
                    packet_id: format!("response-{consultation_id}"),
                    identity: aoc_core::consultation_contracts::ConsultationIdentity {
                        session_id: session.to_string(),
                        agent_id: responding_agent_id.to_string(),
                        pane_id: Some(pane_from_agent_id(responding_agent_id)),
                        conversation_id: None,
                        role: Some("reviewer".to_string()),
                    },
                    summary: Some(
                        "Use request/response topics with strict session scoping".to_string(),
                    ),
                    ..Default::default()
                }),
                message: Some("peer consultation complete".to_string()),
                error: None,
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
        assert!(!baseline.allows(PulseTopic::ObserverSnapshot));
        assert!(!baseline.allows(PulseTopic::ObserverTimeline));
        assert!(!baseline.allows(PulseTopic::ConsultationRequest));
        assert!(!baseline.allows(PulseTopic::ConsultationResponse));
        assert!(!baseline.allows(PulseTopic::LayoutState));

        let selective = TopicFilter::from_subscribe(&SubscribePayload {
            topics: vec![
                "layout_state".to_string(),
                "observer_snapshot".to_string(),
                "consultation_response".to_string(),
            ],
            since_seq: None,
        });
        assert!(!selective.allows(PulseTopic::AgentState));
        assert!(selective.allows(PulseTopic::ObserverSnapshot));
        assert!(!selective.allows(PulseTopic::ObserverTimeline));
        assert!(!selective.allows(PulseTopic::ConsultationRequest));
        assert!(selective.allows(PulseTopic::ConsultationResponse));
        assert!(selective.allows(PulseTopic::LayoutState));
    }

    #[test]
    fn enrich_worker_snapshot_correlates_plan_and_staleness() {
        let now = now_ms();
        let state = AgentState {
            agent_id: "sess::12".to_string(),
            session_id: "sess".to_string(),
            pane_id: "12".to_string(),
            lifecycle: "running".to_string(),
            snippet: Some("working".to_string()),
            last_heartbeat_ms: Some(now),
            last_activity_ms: Some(now - 400_000),
            updated_at_ms: Some(now - 400_000),
            source: Some(serde_json::json!({
                "current_tag": {"tag": "session-overseer"},
                "task_summaries": {
                    "session-overseer": {
                        "active_tasks": [{"id": "149.4", "active_agent": true}]
                    }
                }
            })),
        };
        let snapshot = enrich_worker_snapshot(
            WorkerSnapshot {
                session_id: "sess".to_string(),
                agent_id: "sess::12".to_string(),
                pane_id: "12".to_string(),
                status: WorkerStatus::Active,
                stale_after_ms: Some(300_000),
                ..Default::default()
            },
            &state,
            now,
        );

        assert_eq!(snapshot.assignment.task_id.as_deref(), Some("149.4"));
        assert_eq!(snapshot.assignment.tag.as_deref(), Some("session-overseer"));
        assert_eq!(snapshot.plan_alignment, PlanAlignment::High);
        assert_eq!(snapshot.drift_risk, DriftRisk::High);
        assert_eq!(snapshot.attention.kind.as_deref(), Some("stale"));
    }

    #[test]
    fn duplicate_work_heuristics_flag_overlapping_tasks_and_files() {
        let mut workers = vec![
            WorkerSnapshot {
                agent_id: "sess::12".to_string(),
                pane_id: "12".to_string(),
                session_id: "sess".to_string(),
                assignment: aoc_core::session_overseer::WorkerAssignment {
                    task_id: Some("149.4".to_string()),
                    tag: Some("session-overseer".to_string()),
                    epic_id: None,
                },
                files_touched: vec!["crates/aoc-hub-rs/src/pulse_uds.rs".to_string()],
                drift_risk: DriftRisk::Low,
                ..Default::default()
            },
            WorkerSnapshot {
                agent_id: "sess::19".to_string(),
                pane_id: "19".to_string(),
                session_id: "sess".to_string(),
                assignment: aoc_core::session_overseer::WorkerAssignment {
                    task_id: Some("149.4".to_string()),
                    tag: Some("session-overseer".to_string()),
                    epic_id: None,
                },
                files_touched: vec!["crates/aoc-hub-rs/src/pulse_uds.rs".to_string()],
                drift_risk: DriftRisk::Low,
                ..Default::default()
            },
        ];

        apply_duplicate_work_heuristics(&mut workers);

        for worker in workers {
            let duplicate = worker.duplicate_work.expect("duplicate work flagged");
            assert_eq!(duplicate.overlapping_task_ids, vec!["149.4".to_string()]);
            assert_eq!(
                duplicate.overlapping_files,
                vec!["crates/aoc-hub-rs/src/pulse_uds.rs".to_string()]
            );
            assert_eq!(worker.drift_risk, DriftRisk::Medium);
            assert_eq!(worker.attention.kind.as_deref(), Some("duplicate_work"));
        }
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
    async fn observer_snapshot_and_timeline_bootstrap_from_worker_source() {
        let session = "pulse-overseer-session";
        let (path, shutdown_tx, handle) =
            launch_hub("observer-bootstrap", session, Some(Duration::from_secs(2))).await;
        let agent = format!("{session}::12");

        let (_pub_reader, mut pub_writer) = connect_client(
            &path,
            hello_envelope(session, "pub-1", "publisher", Some(&agent)),
        )
        .await;

        let worker_snapshot = WorkerSnapshot {
            session_id: session.to_string(),
            agent_id: agent.clone(),
            pane_id: "12".to_string(),
            role: Some("worker".to_string()),
            status: aoc_core::session_overseer::WorkerStatus::Active,
            progress: Default::default(),
            assignment: aoc_core::session_overseer::WorkerAssignment {
                task_id: Some("149.3".to_string()),
                tag: Some("session-overseer".to_string()),
                epic_id: None,
            },
            summary: Some("aggregating overseer state".to_string()),
            blocker: None,
            files_touched: vec!["crates/aoc-hub-rs/src/pulse_uds.rs".to_string()],
            plan_alignment: aoc_core::session_overseer::PlanAlignment::High,
            drift_risk: Default::default(),
            attention: Default::default(),
            duplicate_work: None,
            branch: None,
            last_update_at_ms: Some(now_ms()),
            last_meaningful_progress_at_ms: Some(now_ms()),
            stale_after_ms: Some(300_000),
            source: OverseerSourceKind::Wrapper,
            provenance: Some("test".to_string()),
        };
        let observer_event = ObserverEvent {
            schema_version: 1,
            kind: ObserverEventKind::ProgressUpdate,
            session_id: session.to_string(),
            agent_id: agent.clone(),
            pane_id: "12".to_string(),
            source: OverseerSourceKind::Wrapper,
            summary: Some("task context updated: 149.3".to_string()),
            reason: None,
            snapshot: Some(worker_snapshot.clone()),
            command: None,
            command_result: None,
            emitted_at_ms: Some(now_ms()),
        };
        let delta = WireEnvelope {
            version: ProtocolVersion::CURRENT,
            session_id: session.to_string(),
            sender_id: "pub-1".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            request_id: None,
            msg: WireMsg::Delta(DeltaPayload {
                seq: 0,
                changes: vec![StateChange {
                    op: StateChangeOp::Upsert,
                    agent_id: agent.clone(),
                    state: Some(AgentState {
                        agent_id: agent.clone(),
                        session_id: session.to_string(),
                        pane_id: "12".to_string(),
                        lifecycle: "running".to_string(),
                        snippet: Some("working".to_string()),
                        last_heartbeat_ms: Some(now_ms()),
                        last_activity_ms: Some(now_ms()),
                        updated_at_ms: Some(now_ms()),
                        source: Some(serde_json::json!({
                            "worker_snapshot": worker_snapshot,
                            "observer_events": [observer_event],
                        })),
                    }),
                }],
            }),
        };
        send_frame(&mut pub_writer, &delta).await;

        let (mut sub_reader, mut sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let _ = read_frame(&mut sub_reader).await;
        send_frame(
            &mut sub_writer,
            &WireEnvelope {
                version: ProtocolVersion::CURRENT,
                session_id: session.to_string(),
                sender_id: "sub-1".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                request_id: None,
                msg: WireMsg::Subscribe(SubscribePayload {
                    topics: vec![
                        "observer_snapshot".to_string(),
                        "observer_timeline".to_string(),
                    ],
                    since_seq: None,
                }),
            },
        )
        .await;

        let mut saw_snapshot = false;
        let mut saw_timeline = false;
        for _ in 0..4 {
            let envelope = read_frame(&mut sub_reader).await;
            match envelope.msg {
                WireMsg::ObserverSnapshot(payload) => {
                    saw_snapshot = true;
                    assert_eq!(payload.workers.len(), 1);
                    assert_eq!(
                        payload.workers[0].assignment.task_id.as_deref(),
                        Some("149.3")
                    );
                }
                WireMsg::ObserverTimeline(payload) => {
                    saw_timeline = true;
                    assert_eq!(payload.entries.len(), 1);
                    assert_eq!(payload.entries[0].kind, ObserverEventKind::ProgressUpdate);
                }
                WireMsg::Snapshot(_) | WireMsg::Delta(_) => {}
                other => panic!("unexpected message after observer subscribe: {other:?}"),
            }
            if saw_snapshot && saw_timeline {
                break;
            }
        }
        assert!(saw_snapshot);
        assert!(saw_timeline);

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
    async fn insight_dispatch_command_routes_and_acks() {
        let session = "pulse-insight-command-session";
        let (path, shutdown_tx, handle) = launch_hub(
            "insight-command-route",
            session,
            Some(Duration::from_secs(2)),
        )
        .await;
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
            "req-insight-dispatch",
            "insight_dispatch",
            Some(&agent),
            serde_json::json!({"mode": "dispatch", "input": "status"}),
        );
        send_frame(&mut sub_writer, &command).await;

        let routed = read_frame(&mut pub_reader).await;
        let WireMsg::Command(payload) = routed.msg else {
            panic!("expected command routed to publisher")
        };
        assert_eq!(payload.command, "insight_dispatch");

        let ack = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = ack.msg else {
            panic!("expected command_result ack")
        };
        assert_eq!(payload.command, "insight_dispatch");
        assert_eq!(payload.status, "accepted");

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mind_resume_command_routes_and_acks() {
        let session = "pulse-mind-resume-command-session";
        let (path, shutdown_tx, handle) = launch_hub(
            "mind-resume-command-route",
            session,
            Some(Duration::from_secs(2)),
        )
        .await;
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
            "req-mind-resume",
            "mind_resume",
            Some(&agent),
            serde_json::json!({"reason": "aoc-stm resume"}),
        );
        send_frame(&mut sub_writer, &command).await;

        let routed = read_frame(&mut pub_reader).await;
        let WireMsg::Command(payload) = routed.msg else {
            panic!("expected command routed to publisher")
        };
        assert_eq!(payload.command, "mind_resume");

        let ack = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = ack.msg else {
            panic!("expected command_result ack")
        };
        assert_eq!(payload.command, "mind_resume");
        assert_eq!(payload.status, "accepted");

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mind_t3_requeue_command_routes_and_acks() {
        let session = "pulse-mind-t3-requeue-command-session";
        let (path, shutdown_tx, handle) = launch_hub(
            "mind-t3-requeue-command-route",
            session,
            Some(Duration::from_secs(2)),
        )
        .await;
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
            "req-mind-t3-requeue",
            "mind_t3_requeue",
            Some(&agent),
            serde_json::json!({"reason": "operator requeue"}),
        );
        send_frame(&mut sub_writer, &command).await;

        let routed = read_frame(&mut pub_reader).await;
        let WireMsg::Command(payload) = routed.msg else {
            panic!("expected command routed to publisher")
        };
        assert_eq!(payload.command, "mind_t3_requeue");

        let ack = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = ack.msg else {
            panic!("expected command_result ack")
        };
        assert_eq!(payload.command, "mind_t3_requeue");
        assert_eq!(payload.status, "accepted");

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mind_handshake_rebuild_command_routes_and_acks() {
        let session = "pulse-mind-handshake-rebuild-command-session";
        let (path, shutdown_tx, handle) = launch_hub(
            "mind-handshake-rebuild-command-route",
            session,
            Some(Duration::from_secs(2)),
        )
        .await;
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
            "req-mind-handshake-rebuild",
            "mind_handshake_rebuild",
            Some(&agent),
            serde_json::json!({"reason": "operator rebuild"}),
        );
        send_frame(&mut sub_writer, &command).await;

        let routed = read_frame(&mut pub_reader).await;
        let WireMsg::Command(payload) = routed.msg else {
            panic!("expected command routed to publisher")
        };
        assert_eq!(payload.command, "mind_handshake_rebuild");

        let ack = read_frame(&mut sub_reader).await;
        let WireMsg::CommandResult(payload) = ack.msg else {
            panic!("expected command_result ack")
        };
        assert_eq!(payload.command, "mind_handshake_rebuild");
        assert_eq!(payload.status, "accepted");

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn consultation_request_routes_and_acknowledges() {
        let session = "pulse-consultation-session";
        let (path, shutdown_tx, handle) =
            launch_hub("consultation-route", session, Some(Duration::from_secs(2))).await;
        let requester = format!("{session}::12");
        let responder = format!("{session}::24");

        let (mut responder_reader, _responder_writer) = connect_client(
            &path,
            hello_envelope(session, "pub-24", "publisher", Some(&responder)),
        )
        .await;
        let (mut sub_reader, mut sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let _ = read_frame(&mut sub_reader).await;
        send_frame(
            &mut sub_writer,
            &WireEnvelope {
                version: ProtocolVersion::CURRENT,
                session_id: session.to_string(),
                sender_id: "sub-1".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                request_id: None,
                msg: WireMsg::Subscribe(SubscribePayload {
                    topics: vec!["consultation_response".to_string()],
                    since_seq: None,
                }),
            },
        )
        .await;

        let request = consultation_request_envelope(
            session,
            "sub-1",
            "req-consult-1",
            "consult-1",
            &requester,
            &responder,
        );
        send_frame(&mut sub_writer, &request).await;

        let routed = read_frame(&mut responder_reader).await;
        let WireMsg::ConsultationRequest(payload) = routed.msg else {
            panic!("expected consultation request routed to publisher")
        };
        assert_eq!(payload.consultation_id, "consult-1");
        assert_eq!(payload.requesting_agent_id, requester);
        assert_eq!(payload.target_agent_id, responder);

        let ack = read_frame(&mut sub_reader).await;
        let WireMsg::ConsultationResponse(payload) = ack.msg else {
            panic!("expected consultation_response ack")
        };
        assert_eq!(payload.consultation_id, "consult-1");
        assert_eq!(payload.status, ConsultationStatus::Accepted);
        assert_eq!(payload.responding_agent_id, responder);
        assert!(payload.packet.is_none());

        let _ = shutdown_tx.send(true);
        let result = handle.await.expect("join hub");
        assert!(result.is_ok(), "hub returned error: {result:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn consultation_response_broadcasts_to_subscribers() {
        let session = "pulse-consultation-response-session";
        let (path, shutdown_tx, handle) = launch_hub(
            "consultation-response-route",
            session,
            Some(Duration::from_secs(2)),
        )
        .await;
        let requester = format!("{session}::12");
        let responder = format!("{session}::24");

        let (_responder_reader, mut responder_writer) = connect_client(
            &path,
            hello_envelope(session, "pub-24", "publisher", Some(&responder)),
        )
        .await;
        let (mut sub_reader, mut sub_writer) =
            connect_client(&path, hello_envelope(session, "sub-1", "subscriber", None)).await;
        let _ = read_frame(&mut sub_reader).await;
        send_frame(
            &mut sub_writer,
            &WireEnvelope {
                version: ProtocolVersion::CURRENT,
                session_id: session.to_string(),
                sender_id: "sub-1".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                request_id: None,
                msg: WireMsg::Subscribe(SubscribePayload {
                    topics: vec!["consultation_response".to_string()],
                    since_seq: None,
                }),
            },
        )
        .await;
        tokio::time::sleep(Duration::from_millis(25)).await;

        let response = consultation_response_envelope(
            session,
            "pub-24",
            "req-consult-2",
            "consult-2",
            &requester,
            &responder,
        );
        send_frame(&mut responder_writer, &response).await;

        let forwarded = read_frame(&mut sub_reader).await;
        let WireMsg::ConsultationResponse(payload) = forwarded.msg else {
            panic!("expected consultation response broadcast")
        };
        assert_eq!(payload.consultation_id, "consult-2");
        assert_eq!(payload.status, ConsultationStatus::Completed);
        assert_eq!(payload.responding_agent_id, responder);
        assert_eq!(
            payload
                .packet
                .as_ref()
                .and_then(|packet| packet.summary.as_deref()),
            Some("Use request/response topics with strict session scoping")
        );

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
