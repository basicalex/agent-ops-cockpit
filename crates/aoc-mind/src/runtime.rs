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
    insight_contracts::{
        InsightDetachedJob, InsightDetachedJobStatus, InsightDetachedMode,
        InsightDetachedOwnerPlane, InsightDetachedWorkerKind, InsightDispatchStepResult,
    },
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

pub struct MindTickEffects {
    pub observer_events: Vec<MindObserverFeedEvent>,
}

pub struct MindDetachedJobOutcome {
    pub status: InsightDetachedJobStatus,
    pub summary: String,
    pub exit_code: i32,
}

pub struct MindDetachedDispatchDecision {
    pub should_dispatch: bool,
    pub emit_status_update: bool,
    pub stale_jobs: Vec<InsightDetachedJob>,
    pub job_id: Option<String>,
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

const DETACHED_JOB_OUTPUT_MAX_CHARS: usize = 320;
const DETACHED_JOB_ERROR_MAX_CHARS: usize = 240;

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

    pub fn refresh_queue_depths(&self, snapshot: &mut MindServiceHealthSnapshot) {
        snapshot.queue_depth = self.pending_reflector_jobs();
        snapshot.t3_queue_depth = self.pending_t3_backlog_jobs();
    }

    pub fn reconcile_stale_detached_jobs(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        jobs: Vec<InsightDetachedJob>,
        lease_active: bool,
        now: chrono::DateTime<chrono::Utc>,
        stale_after_ms: i64,
    ) -> Vec<InsightDetachedJob> {
        if lease_active {
            return Vec::new();
        }

        let stale_before = self.stale_detached_cutoff_ms(now, stale_after_ms);
        jobs.into_iter()
            .filter(|job| {
                job.worker_kind == Some(worker_kind)
                    && matches!(
                        job.status,
                        InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
                    )
                    && job.created_at_ms <= stale_before
            })
            .map(|job| self.mark_detached_job_stale(job, now.timestamp_millis()))
            .collect()
    }

    pub fn has_active_detached_job(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        jobs: &[InsightDetachedJob],
    ) -> bool {
        jobs.iter().any(|job| {
            job.worker_kind == Some(worker_kind)
                && matches!(
                    job.status,
                    InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
                )
        })
    }

    pub fn next_detached_job_id(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        now: chrono::DateTime<chrono::Utc>,
    ) -> String {
        let kind = match worker_kind {
            InsightDetachedWorkerKind::T2 => "t2",
            InsightDetachedWorkerKind::T3 => "t3",
            InsightDetachedWorkerKind::T1 => "t1",
            _ => "runtime",
        };
        format!(
            "mind-{}-{}-{}",
            kind,
            now.timestamp_millis().max(0),
            std::process::id()
        )
    }

    pub fn detached_dispatch_decision(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        queue_depth: i64,
        jobs: Vec<InsightDetachedJob>,
        lease_active: bool,
        now: chrono::DateTime<chrono::Utc>,
        stale_after_ms: i64,
    ) -> MindDetachedDispatchDecision {
        if queue_depth <= 0 {
            return MindDetachedDispatchDecision {
                should_dispatch: false,
                emit_status_update: false,
                stale_jobs: Vec::new(),
                job_id: None,
            };
        }

        let stale_jobs = self.reconcile_stale_detached_jobs(
            worker_kind,
            jobs.clone(),
            lease_active,
            now,
            stale_after_ms,
        );
        if self.has_active_detached_job(worker_kind, &jobs) {
            return MindDetachedDispatchDecision {
                should_dispatch: false,
                emit_status_update: !stale_jobs.is_empty(),
                stale_jobs,
                job_id: None,
            };
        }

        MindDetachedDispatchDecision {
            should_dispatch: true,
            emit_status_update: true,
            stale_jobs,
            job_id: Some(self.next_detached_job_id(worker_kind, now)),
        }
    }

    pub fn detached_spawned_pid_note(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        pid: u32,
    ) -> String {
        format!("{} queued (pid {pid})", detached_mind_worker_label(worker_kind))
    }

    pub fn detached_spawn_fallback_note(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        spawn_err: &str,
    ) -> String {
        format!(
            "{} fallback inline after spawn failure: {spawn_err}",
            detached_mind_worker_label(worker_kind)
        )
    }

    pub fn new_detached_job(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        job_id: String,
        created_at_ms: i64,
    ) -> InsightDetachedJob {
        InsightDetachedJob {
            job_id,
            parent_job_id: None,
            owner_plane: InsightDetachedOwnerPlane::Mind,
            worker_kind: Some(worker_kind),
            mode: InsightDetachedMode::Dispatch,
            status: InsightDetachedJobStatus::Queued,
            agent: Some(detached_mind_worker_label(worker_kind).to_string()),
            team: None,
            chain: None,
            created_at_ms,
            started_at_ms: None,
            finished_at_ms: None,
            current_step_index: Some(0),
            step_count: Some(1),
            output_excerpt: Some(format!("{} queued", detached_mind_worker_label(worker_kind))),
            stdout_excerpt: None,
            stderr_excerpt: None,
            error: None,
            fallback_used: false,
            step_results: Vec::new(),
        }
    }

    pub fn mark_detached_job_running(
        &self,
        mut job: InsightDetachedJob,
        started_at_ms: i64,
        note: Option<String>,
    ) -> InsightDetachedJob {
        let worker_kind = job.worker_kind.unwrap_or(InsightDetachedWorkerKind::T2);
        job.status = InsightDetachedJobStatus::Running;
        job.started_at_ms = Some(started_at_ms);
        job.output_excerpt = note.or_else(|| {
            Some(format!("{} running", detached_mind_worker_label(worker_kind)))
        });
        job
    }

    pub fn mark_detached_job_stale(
        &self,
        mut job: InsightDetachedJob,
        finished_at_ms: i64,
    ) -> InsightDetachedJob {
        let worker_kind = job.worker_kind.unwrap_or(InsightDetachedWorkerKind::T2);
        job.status = InsightDetachedJobStatus::Stale;
        job.finished_at_ms = Some(finished_at_ms);
        job.error.get_or_insert_with(|| {
            format!(
                "{} lease expired before detached completion was observed",
                detached_mind_worker_title(worker_kind)
            )
        });
        if job.output_excerpt.is_none() {
            job.output_excerpt = Some(format!(
                "{} marked stale after lease expiry",
                detached_mind_worker_label(worker_kind)
            ));
        }
        job
    }

    pub fn finalize_detached_job(
        &self,
        mut job: InsightDetachedJob,
        status: InsightDetachedJobStatus,
        summary: String,
        error: Option<String>,
        fallback_used: bool,
        finished_at_ms: i64,
    ) -> InsightDetachedJob {
        if job.status == InsightDetachedJobStatus::Cancelled {
            job.finished_at_ms.get_or_insert(finished_at_ms);
            if job.output_excerpt.is_none() {
                job.output_excerpt = Some(truncate_chars(summary, DETACHED_JOB_OUTPUT_MAX_CHARS));
            }
            if job.stderr_excerpt.is_none() {
                job.stderr_excerpt = error
                    .as_ref()
                    .map(|value| truncate_chars(value.clone(), DETACHED_JOB_ERROR_MAX_CHARS));
            }
            if job.error.is_none() {
                job.error = error;
            }
            return job;
        }

        let worker_kind = job.worker_kind.unwrap_or(InsightDetachedWorkerKind::T2);
        let step_status = match status {
            InsightDetachedJobStatus::Success => "success",
            InsightDetachedJobStatus::Cancelled => "cancelled",
            InsightDetachedJobStatus::Error => "error",
            InsightDetachedJobStatus::Fallback => "fallback",
            InsightDetachedJobStatus::Stale => "error",
            InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running => "success",
        }
        .to_string();
        let output_excerpt = truncate_chars(summary.clone(), DETACHED_JOB_OUTPUT_MAX_CHARS);
        let stderr_excerpt = error
            .as_ref()
            .map(|value| truncate_chars(value.clone(), DETACHED_JOB_ERROR_MAX_CHARS));

        job.status = status;
        job.started_at_ms.get_or_insert(finished_at_ms);
        job.finished_at_ms = Some(finished_at_ms);
        job.current_step_index = Some(1);
        job.step_count = Some(1);
        job.output_excerpt = Some(output_excerpt.clone());
        job.stdout_excerpt = Some(output_excerpt.clone());
        job.stderr_excerpt = stderr_excerpt.clone();
        job.error = error.clone();
        job.fallback_used = fallback_used;
        job.step_results = vec![InsightDispatchStepResult {
            agent: detached_mind_worker_label(worker_kind).to_string(),
            status: step_status,
            output_excerpt: Some(output_excerpt),
            stdout_excerpt: None,
            stderr_excerpt,
            error,
        }];
        job
    }

    pub fn reflector_completion_outcome(
        &self,
        report: &ReflectorTickReport,
        fallback_used: bool,
    ) -> MindDetachedJobOutcome {
        let status = if report.jobs_failed > 0 {
            if fallback_used {
                InsightDetachedJobStatus::Fallback
            } else {
                InsightDetachedJobStatus::Error
            }
        } else {
            InsightDetachedJobStatus::Success
        };
        let summary = if fallback_used {
            format!(
                "mind t2 inline fallback processed claimed={} completed={} failed={}",
                report.jobs_claimed, report.jobs_completed, report.jobs_failed
            )
        } else {
            format!(
                "mind t2 detached worker processed claimed={} completed={} failed={}",
                report.jobs_claimed, report.jobs_completed, report.jobs_failed
            )
        };
        MindDetachedJobOutcome {
            exit_code: if report.jobs_failed > 0 { 1 } else { 0 },
            status,
            summary,
        }
    }

    pub fn t3_completion_outcome(
        &self,
        report: &T3TickReport,
        fallback_used: bool,
    ) -> MindDetachedJobOutcome {
        let status = if report.jobs_failed > 0 {
            if fallback_used {
                InsightDetachedJobStatus::Fallback
            } else {
                InsightDetachedJobStatus::Error
            }
        } else {
            InsightDetachedJobStatus::Success
        };
        let summary = if fallback_used {
            format!(
                "mind t3 inline fallback processed claimed={} completed={} failed={} requeued={} dead_lettered={}",
                report.jobs_claimed,
                report.jobs_completed,
                report.jobs_failed,
                report.jobs_requeued,
                report.jobs_dead_lettered
            )
        } else {
            format!(
                "mind t3 detached worker processed claimed={} completed={} failed={} requeued={} dead_lettered={}",
                report.jobs_claimed,
                report.jobs_completed,
                report.jobs_failed,
                report.jobs_requeued,
                report.jobs_dead_lettered
            )
        };
        MindDetachedJobOutcome {
            exit_code: if report.jobs_failed > 0 { 1 } else { 0 },
            status,
            summary,
        }
    }

    pub fn apply_reflector_runtime_failure(
        &self,
        snapshot: &mut MindServiceHealthSnapshot,
        runtime_err: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> MindTickEffects {
        snapshot.last_error = Some(format!("reflector tick failed: {runtime_err}"));
        MindTickEffects {
            observer_events: vec![MindObserverFeedEvent {
                status: MindObserverFeedStatus::Error,
                trigger: MindObserverFeedTriggerKind::TaskCompleted,
                conversation_id: self.latest_conversation_id().map(str::to_string),
                runtime: Some("t2_reflector".to_string()),
                attempt_count: Some(1),
                latency_ms: None,
                reason: Some("reflector tick failed".to_string()),
                failure_kind: Some("runtime_error".to_string()),
                enqueued_at: None,
                started_at: None,
                completed_at: Some(now.to_rfc3339()),
                progress: None,
            }],
        }
    }

    pub fn apply_t3_runtime_failure(
        &self,
        snapshot: &mut MindServiceHealthSnapshot,
        runtime_err: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> MindTickEffects {
        snapshot.last_error = Some(format!("t3 backlog tick failed: {runtime_err}"));
        MindTickEffects {
            observer_events: vec![MindObserverFeedEvent {
                status: MindObserverFeedStatus::Error,
                trigger: MindObserverFeedTriggerKind::TaskCompleted,
                conversation_id: self.latest_conversation_id().map(str::to_string),
                runtime: Some("t3_backlog".to_string()),
                attempt_count: Some(1),
                latency_ms: None,
                reason: Some("t3 backlog tick failed".to_string()),
                failure_kind: Some("runtime_error".to_string()),
                enqueued_at: None,
                started_at: None,
                completed_at: Some(now.to_rfc3339()),
                progress: None,
            }],
        }
    }

    pub fn runtime_failure_job_summary(
        &self,
        worker_kind: InsightDetachedWorkerKind,
        fallback_used: bool,
    ) -> String {
        match (worker_kind, fallback_used) {
            (InsightDetachedWorkerKind::T2, true) => "mind t2 inline fallback failed".to_string(),
            (InsightDetachedWorkerKind::T2, false) => "mind t2 detached worker failed".to_string(),
            (InsightDetachedWorkerKind::T3, true) => "mind t3 inline fallback failed".to_string(),
            (InsightDetachedWorkerKind::T3, false) => "mind t3 detached worker failed".to_string(),
            (_, true) => "mind runtime inline fallback failed".to_string(),
            (_, false) => "mind detached runtime worker failed".to_string(),
        }
    }

    pub fn compose_runtime_failure_error(
        &self,
        spawn_err: Option<&str>,
        runtime_err: &str,
    ) -> String {
        match spawn_err {
            Some(spawn_err) => format!("spawn failed: {spawn_err}; runtime failed: {runtime_err}"),
            None => runtime_err.to_string(),
        }
    }

    pub fn begin_reflector_tick(
        &self,
        snapshot: &mut MindServiceHealthSnapshot,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        snapshot.reflector_ticks = snapshot.reflector_ticks.saturating_add(1);
        snapshot.last_tick_ms = Some(now.timestamp_millis());
    }

    pub fn begin_t3_tick(
        &self,
        snapshot: &mut MindServiceHealthSnapshot,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        snapshot.t3_ticks = snapshot.t3_ticks.saturating_add(1);
        snapshot.last_tick_ms = Some(now.timestamp_millis());
    }

    pub fn reflector_scope_key(&self) -> String {
        reflector_scope_id_for_project_root(&self.project_root.to_string_lossy())
    }

    pub fn t3_scope_key(&self) -> String {
        t3_scope_id_for_project_root(&self.project_root.to_string_lossy())
    }

    pub fn reflector_lease_active_at(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.store
            .reflector_lease(&self.reflector_scope_key())
            .ok()
            .flatten()
            .map(|lease| lease.expires_at >= now)
            .unwrap_or(false)
    }

    pub fn t3_runtime_lease_active_at(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.store
            .t3_runtime_lease(&self.t3_scope_key())
            .ok()
            .flatten()
            .map(|lease| lease.expires_at >= now)
            .unwrap_or(false)
    }

    pub fn stale_detached_cutoff_ms(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        stale_after_ms: i64,
    ) -> i64 {
        now.timestamp_millis() - stale_after_ms
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

    pub fn apply_reflector_tick_effects(
        &self,
        snapshot: &mut MindServiceHealthSnapshot,
        report: &ReflectorTickReport,
        now: chrono::DateTime<chrono::Utc>,
    ) -> MindTickEffects {
        if report.lock_conflict {
            snapshot.reflector_lock_conflicts = snapshot.reflector_lock_conflicts.saturating_add(1);
        }
        snapshot.reflector_jobs_completed = snapshot
            .reflector_jobs_completed
            .saturating_add(report.jobs_completed as u64);
        snapshot.reflector_jobs_failed = snapshot
            .reflector_jobs_failed
            .saturating_add(report.jobs_failed as u64);

        if report.jobs_failed == 0 {
            snapshot.last_error = None;
        }

        let conversation_id = self.latest_conversation_id().map(str::to_string);
        let mut observer_events = Vec::new();
        if report.jobs_completed > 0 {
            observer_events.push(MindObserverFeedEvent {
                status: MindObserverFeedStatus::Success,
                trigger: MindObserverFeedTriggerKind::TaskCompleted,
                conversation_id: conversation_id.clone(),
                runtime: Some("t2_reflector".to_string()),
                attempt_count: Some(1),
                latency_ms: None,
                reason: Some(format!("t2 reflector processed {} job(s)", report.jobs_completed)),
                failure_kind: None,
                enqueued_at: None,
                started_at: None,
                completed_at: Some(now.to_rfc3339()),
                progress: None,
            });
        }
        if report.jobs_failed > 0 {
            observer_events.push(MindObserverFeedEvent {
                status: MindObserverFeedStatus::Error,
                trigger: MindObserverFeedTriggerKind::TaskCompleted,
                conversation_id,
                runtime: Some("t2_reflector".to_string()),
                attempt_count: Some(1),
                latency_ms: None,
                reason: Some(format!("t2 reflector failed {} job(s)", report.jobs_failed)),
                failure_kind: Some("runtime_error".to_string()),
                enqueued_at: None,
                started_at: None,
                completed_at: Some(now.to_rfc3339()),
                progress: None,
            });
        }

        MindTickEffects { observer_events }
    }

    pub fn apply_t3_tick_effects(
        &self,
        snapshot: &mut MindServiceHealthSnapshot,
        report: &T3TickReport,
        now: chrono::DateTime<chrono::Utc>,
    ) -> MindTickEffects {
        if report.lock_conflict {
            snapshot.t3_lock_conflicts = snapshot.t3_lock_conflicts.saturating_add(1);
        }
        snapshot.t3_jobs_completed = snapshot
            .t3_jobs_completed
            .saturating_add(report.jobs_completed as u64);
        snapshot.t3_jobs_failed = snapshot.t3_jobs_failed.saturating_add(report.jobs_failed as u64);
        snapshot.t3_jobs_requeued = snapshot
            .t3_jobs_requeued
            .saturating_add(report.jobs_requeued as u64);
        snapshot.t3_jobs_dead_lettered = snapshot
            .t3_jobs_dead_lettered
            .saturating_add(report.jobs_dead_lettered as u64);

        if report.jobs_failed == 0 {
            snapshot.last_error = None;
        }

        let conversation_id = self.latest_conversation_id().map(str::to_string);
        let mut observer_events = Vec::new();
        if report.jobs_completed > 0 {
            observer_events.push(MindObserverFeedEvent {
                status: MindObserverFeedStatus::Success,
                trigger: MindObserverFeedTriggerKind::TaskCompleted,
                conversation_id: conversation_id.clone(),
                runtime: Some("t3_backlog".to_string()),
                attempt_count: Some(1),
                latency_ms: None,
                reason: Some(format!("t3 backlog processed {} job(s)", report.jobs_completed)),
                failure_kind: None,
                enqueued_at: None,
                started_at: None,
                completed_at: Some(now.to_rfc3339()),
                progress: None,
            });
        }
        if report.jobs_failed > 0 {
            observer_events.push(MindObserverFeedEvent {
                status: MindObserverFeedStatus::Error,
                trigger: MindObserverFeedTriggerKind::TaskCompleted,
                conversation_id,
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
            });
        }

        MindTickEffects { observer_events }
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

fn detached_mind_worker_label(kind: InsightDetachedWorkerKind) -> &'static str {
    match kind {
        InsightDetachedWorkerKind::T2 => "mind t2 worker",
        InsightDetachedWorkerKind::T3 => "mind t3 worker",
        InsightDetachedWorkerKind::T1 => "mind t1 worker",
        _ => "mind runtime worker",
    }
}

fn detached_mind_worker_title(kind: InsightDetachedWorkerKind) -> &'static str {
    match kind {
        InsightDetachedWorkerKind::T2 => "Mind T2 worker",
        InsightDetachedWorkerKind::T3 => "Mind T3 worker",
        InsightDetachedWorkerKind::T1 => "Mind T1 worker",
        _ => "Mind runtime worker",
    }
}

fn truncate_chars(text: String, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text;
    }
    let mut truncated = text.chars().take(max_chars.saturating_sub(1)).collect::<String>();
    truncated.push('…');
    truncated
}

fn reflector_scope_id_for_project_root(project_root: &str) -> String {
    project_scope_key(Path::new(project_root))
}
