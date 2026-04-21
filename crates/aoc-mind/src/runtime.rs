use crate::{
    drain_observer_state, enqueue_observer_and_run_events, evaluate_finalize_drain,
    evaluate_idle_finalize, evaluate_t1_token_threshold, open_project_store, process_reflector_job,
    process_t3_backlog_job, project_scope_key, t3_scope_id_for_project_root,
    write_mind_service_health_snapshot, DetachedReflectorWorker, DetachedT3Worker,
    DistillationConfig, FinalizeDrainDecision, IdleFinalizeDecision, MindServiceHealthSnapshot,
    MindServiceLeaseGuard, ObserverDrainState, PiObserverAdapter, ReflectorRuntimeConfig,
    ReflectorTickReport, SemanticObserverConfig, SessionObserverSidecar, T1ThresholdDecision,
    T3RuntimeConfig, T3TickReport,
};
use aoc_core::{
    mind_contracts::SemanticRuntimeMode,
    mind_observer_feed::{
        MindObserverFeedEvent, MindObserverFeedStatus, MindObserverFeedTriggerKind,
    },
};
use aoc_storage::MindStore;
use chrono::Utc;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MindRuntimeConfig {
    pub project_root: String,
    pub session_id: String,
    pub pane_id: String,
    pub agent_key: String,
    pub store_path_override: Option<String>,
    pub reflector_lock_path: PathBuf,
    pub t3_lock_path: PathBuf,
    pub debounce_run_ms: i64,
    pub t3_max_attempts: u16,
}

pub struct MindFinalizeDrainOutcome {
    pub observer_events: Vec<MindObserverFeedEvent>,
    pub settled: bool,
    pub timed_out_reason: Option<String>,
}

pub struct MindRuntimeCore {
    store: MindStore,
    sidecar: SessionObserverSidecar<PiObserverAdapter>,
    distill: DistillationConfig,
    session_id: String,
    pane_id: String,
    project_root: PathBuf,
    latest_conversation_id: Option<String>,
    last_ingest_at: Option<chrono::DateTime<chrono::Utc>>,
    last_idle_finalize_check: Option<chrono::DateTime<chrono::Utc>>,
    reflector_worker: DetachedReflectorWorker,
    t3_worker: DetachedT3Worker,
    debounce_run_ms: i64,
    service_lease: MindServiceLeaseGuard,
}

impl MindRuntimeCore {
    pub fn new(cfg: MindRuntimeConfig) -> Result<Self, String> {
        let opened = open_project_store(
            Path::new(&cfg.project_root),
            &cfg.session_id,
            &cfg.pane_id,
            cfg.store_path_override.as_deref(),
        )
        .map_err(|err| format!("mind store open failed: {err}"))?;
        let store = opened.store;

        let mut distill = DistillationConfig::default();
        let mut semantic = SemanticObserverConfig::default();
        semantic.mode = SemanticRuntimeMode::DeterministicOnly;
        let semantic_input_limit = semantic.profile.max_input_tokens.max(1);
        distill.t1_target_tokens = distill.t1_target_tokens.min(semantic_input_limit);
        distill.t1_hard_cap_tokens = distill.t1_hard_cap_tokens.min(semantic_input_limit);
        let sidecar =
            SessionObserverSidecar::new(distill.clone(), semantic, PiObserverAdapter::default());
        let service_lease = MindServiceLeaseGuard::acquire(
            Path::new(&cfg.project_root),
            &cfg.agent_key,
            &cfg.session_id,
            &cfg.pane_id,
            30_000,
        )
        .map_err(|err| format!("mind service lease acquire failed: {err}"))?;
        let reflector_worker = DetachedReflectorWorker::new(ReflectorRuntimeConfig {
            scope_id: reflector_scope_id_for_project_root(&cfg.project_root),
            owner_id: cfg.agent_key.clone(),
            owner_pid: Some(std::process::id() as i64),
            lock_path: cfg.reflector_lock_path,
            lease_ttl_ms: 30_000,
            max_jobs_per_tick: 2,
            requeue_on_error: true,
        });
        let t3_worker = DetachedT3Worker::new(T3RuntimeConfig {
            scope_id: t3_scope_id_for_project_root(&cfg.project_root),
            owner_id: cfg.agent_key,
            owner_pid: Some(std::process::id() as i64),
            lock_path: cfg.t3_lock_path,
            lease_ttl_ms: 30_000,
            stale_claim_after_ms: 60_000,
            max_jobs_per_tick: 4,
            requeue_on_error: true,
            max_attempts: cfg.t3_max_attempts,
        });

        Ok(Self {
            store,
            sidecar,
            distill,
            session_id: cfg.session_id,
            pane_id: cfg.pane_id,
            project_root: PathBuf::from(cfg.project_root),
            latest_conversation_id: None,
            last_ingest_at: None,
            last_idle_finalize_check: None,
            reflector_worker,
            t3_worker,
            debounce_run_ms: cfg.debounce_run_ms,
            service_lease,
        })
    }

    pub fn store(&self) -> &MindStore {
        &self.store
    }

    pub fn sidecar_mut(&mut self) -> &mut SessionObserverSidecar<PiObserverAdapter> {
        &mut self.sidecar
    }

    pub fn distill(&self) -> &DistillationConfig {
        &self.distill
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn pane_id(&self) -> &str {
        &self.pane_id
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn latest_conversation_id(&self) -> Option<&str> {
        self.latest_conversation_id.as_deref()
    }

    pub fn set_latest_conversation_id(&mut self, conversation_id: Option<String>) {
        self.latest_conversation_id = conversation_id;
    }

    pub fn last_ingest_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.last_ingest_at
    }

    pub fn set_last_ingest_at(&mut self, at: Option<chrono::DateTime<chrono::Utc>>) {
        self.last_ingest_at = at;
    }

    pub fn last_idle_finalize_check(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.last_idle_finalize_check
    }

    pub fn set_last_idle_finalize_check(&mut self, at: Option<chrono::DateTime<chrono::Utc>>) {
        self.last_idle_finalize_check = at;
    }

    pub fn reflector_worker(&self) -> &DetachedReflectorWorker {
        &self.reflector_worker
    }

    pub fn t3_worker(&self) -> &DetachedT3Worker {
        &self.t3_worker
    }

    pub fn drain_observer_state(
        &mut self,
        session_id: Option<&str>,
        run_at: chrono::DateTime<chrono::Utc>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> ObserverDrainState {
        drain_observer_state(&mut self.sidecar, &self.store, session_id, run_at, now)
    }

    pub fn heartbeat_service(
        &mut self,
        snapshot: &mut MindServiceHealthSnapshot,
    ) -> Result<(), String> {
        self.service_lease
            .heartbeat(30_000)
            .map_err(|err| format!("mind service lease heartbeat failed: {err}"))?;
        snapshot.owner_id = self.service_lease.lease().owner_id.clone();
        snapshot.owner_pid = self.service_lease.lease().owner_pid;
        snapshot.session_id = self.session_id.clone();
        snapshot.pane_id = self.pane_id.clone();
        if snapshot.lifecycle.is_empty() {
            snapshot.lifecycle = "running".to_string();
        }
        snapshot.last_heartbeat_ms = Some(Utc::now().timestamp_millis());
        snapshot.lease_expires_at_ms =
            Some(self.service_lease.lease().expires_at.timestamp_millis());
        write_mind_service_health_snapshot(&self.project_root, snapshot)
            .map_err(|err| format!("mind service health snapshot write failed: {err}"))
    }

    pub fn pending_reflector_jobs(&self) -> i64 {
        self.store.pending_reflector_jobs().unwrap_or_default()
    }

    pub fn pending_t3_backlog_jobs(&self) -> i64 {
        self.store.pending_t3_backlog_jobs().unwrap_or_default()
    }

    pub fn session_watermark_scope_key(&self) -> String {
        format!("session:{}:pane:{}", self.session_id, self.pane_id)
    }

    pub fn take_idle_finalize_reason(
        &mut self,
        now: chrono::DateTime<chrono::Utc>,
        idle_timeout_ms: i64,
        throttle_ms: i64,
    ) -> Option<String> {
        match evaluate_idle_finalize(
            self.last_ingest_at,
            self.last_idle_finalize_check,
            now,
            idle_timeout_ms,
            throttle_ms,
        ) {
            IdleFinalizeDecision::Finalize { reason } => {
                self.last_idle_finalize_check = Some(now);
                Some(reason.to_string())
            }
            IdleFinalizeDecision::NoLastIngest
            | IdleFinalizeDecision::Disabled
            | IdleFinalizeDecision::WaitingForIdleTimeout
            | IdleFinalizeDecision::Throttled => None,
        }
    }

    pub fn finalize_drain_step(
        &mut self,
        session_id: Option<&str>,
        deadline: chrono::DateTime<chrono::Utc>,
        reflector_pending: i64,
    ) -> MindFinalizeDrainOutcome {
        let now = Utc::now();
        let run_at = now + chrono::Duration::milliseconds(self.debounce_run_ms + 1);
        let observer = self.drain_observer_state(session_id, run_at, now);
        let observer_idle = observer.is_idle();
        let observer_events = observer.events;

        match evaluate_finalize_drain(observer_idle, reflector_pending, Utc::now(), deadline) {
            FinalizeDrainDecision::Settled => MindFinalizeDrainOutcome {
                observer_events,
                settled: true,
                timed_out_reason: None,
            },
            FinalizeDrainDecision::TimedOut { observer_reason } => MindFinalizeDrainOutcome {
                observer_events,
                settled: false,
                timed_out_reason: Some(observer_reason.to_string()),
            },
            FinalizeDrainDecision::Continue => MindFinalizeDrainOutcome {
                observer_events,
                settled: false,
                timed_out_reason: None,
            },
        }
    }

    pub fn run_reflector_tick(
        &mut self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<ReflectorTickReport, String> {
        self.reflector_worker
            .run_once(&self.store, now, |store, job| {
                process_reflector_job(store, job, now).map_err(|err| err.to_string())
            })
            .map_err(|err| err.to_string())
    }

    pub fn run_t3_tick<F>(
        &mut self,
        now: chrono::DateTime<chrono::Utc>,
        export_writer: F,
    ) -> Result<T3TickReport, String>
    where
        F: Fn(&MindStore, &str, Option<&str>, chrono::DateTime<chrono::Utc>) -> Result<(), String>,
    {
        self.t3_worker
            .run_once(&self.store, now, |store, job| {
                process_t3_backlog_job(store, job, now, |store, project_root, active_tag, now| {
                    export_writer(store, project_root, active_tag, now)
                })
                .map_err(|err| err.to_string())
            })
            .map_err(|err| err.to_string())
    }

    pub fn resolve_conversation_id(&self, args: &serde_json::Value) -> Option<String> {
        args.as_object()
            .and_then(|value| value.get("conversation_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .or_else(|| self.latest_conversation_id.clone())
    }

    pub fn enqueue_observer_events(
        &mut self,
        conversation_id: &str,
        trigger: MindObserverFeedTriggerKind,
        reason: Option<String>,
    ) -> Vec<MindObserverFeedEvent> {
        enqueue_observer_and_run_events(
            &mut self.sidecar,
            &self.store,
            &self.session_id,
            conversation_id,
            trigger,
            reason,
            Utc::now(),
            self.debounce_run_ms,
        )
    }

    pub fn maybe_run_token_threshold_events(
        &mut self,
        conversation_id: &str,
    ) -> Vec<MindObserverFeedEvent> {
        match evaluate_t1_token_threshold(
            &self.store,
            conversation_id,
            self.distill.t1_target_tokens,
            self.distill.t1_hard_cap_tokens,
        ) {
            Ok(T1ThresholdDecision::NoProgress)
            | Ok(T1ThresholdDecision::BelowTarget { .. })
            | Ok(T1ThresholdDecision::AlreadySatisfied { .. }) => Vec::new(),
            Ok(T1ThresholdDecision::NeedsRun { reason, .. }) => self.enqueue_observer_events(
                conversation_id,
                MindObserverFeedTriggerKind::TokenThreshold,
                Some(reason),
            ),
            Err(err) => vec![MindObserverFeedEvent {
                status: MindObserverFeedStatus::Error,
                trigger: MindObserverFeedTriggerKind::TokenThreshold,
                conversation_id: None,
                runtime: None,
                attempt_count: None,
                latency_ms: None,
                reason: Some(format!("mind threshold check failed: {err}")),
                failure_kind: None,
                enqueued_at: None,
                started_at: None,
                completed_at: None,
                progress: None,
            }],
        }
    }
}

fn reflector_scope_id_for_project_root(project_root: &str) -> String {
    project_scope_key(Path::new(project_root))
}
