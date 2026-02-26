use aoc_core::mind_contracts::SemanticGuardrails;
use aoc_storage::{MindStore, ReflectorJob};
use chrono::{DateTime, Duration, Utc};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ReflectorRuntimeConfig {
    pub scope_id: String,
    pub owner_id: String,
    pub owner_pid: Option<i64>,
    pub lock_path: PathBuf,
    pub lease_ttl_ms: u64,
    pub max_jobs_per_tick: usize,
    pub requeue_on_error: bool,
}

impl ReflectorRuntimeConfig {
    pub fn with_lock_path(
        scope_id: impl Into<String>,
        owner_id: impl Into<String>,
        lock_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            scope_id: scope_id.into(),
            owner_id: owner_id.into(),
            owner_pid: Some(std::process::id() as i64),
            lock_path: lock_path.into(),
            lease_ttl_ms: 30_000,
            max_jobs_per_tick: 4,
            requeue_on_error: false,
        }
    }

    pub fn with_guardrails(
        scope_id: impl Into<String>,
        owner_id: impl Into<String>,
        lock_path: impl Into<PathBuf>,
        guardrails: &SemanticGuardrails,
    ) -> Self {
        let mut config = Self::with_lock_path(scope_id, owner_id, lock_path);
        config.lease_ttl_ms = guardrails.reflector_lease_ttl_ms.max(1);
        config
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReflectorTickReport {
    pub file_lock_acquired: bool,
    pub lease_acquired: bool,
    pub lock_conflict: bool,
    pub jobs_claimed: usize,
    pub jobs_completed: usize,
    pub jobs_failed: usize,
}

#[derive(Debug, Error)]
pub enum ReflectorRuntimeError {
    #[error("storage error: {0}")]
    Storage(#[from] aoc_storage::StorageError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

struct AdvisoryReflectorFileLock {
    file: File,
}

impl AdvisoryReflectorFileLock {
    fn try_acquire(
        path: &Path,
        owner_id: &str,
        owner_pid: Option<i64>,
        now: DateTime<Utc>,
        ttl_ms: u64,
    ) -> Result<Option<Self>, std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        if file.try_lock_exclusive().is_err() {
            return Ok(None);
        }

        let expires_at = now + Duration::milliseconds(ttl_ms.min(i64::MAX as u64) as i64);
        let metadata = format!(
            "owner_id={owner_id}\nowner_pid={}\nacquired_at={}\nexpires_at={}\n",
            owner_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "na".to_string()),
            now.to_rfc3339(),
            expires_at.to_rfc3339(),
        );
        file.set_len(0)?;
        file.write_all(metadata.as_bytes())?;
        file.flush()?;

        Ok(Some(Self { file }))
    }
}

impl Drop for AdvisoryReflectorFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub struct DetachedReflectorWorker {
    config: ReflectorRuntimeConfig,
}

impl DetachedReflectorWorker {
    pub fn new(config: ReflectorRuntimeConfig) -> Self {
        Self { config }
    }

    pub fn run_once<F>(
        &self,
        store: &MindStore,
        now: DateTime<Utc>,
        mut handler: F,
    ) -> Result<ReflectorTickReport, ReflectorRuntimeError>
    where
        F: FnMut(&MindStore, &ReflectorJob) -> Result<(), String>,
    {
        let mut report = ReflectorTickReport::default();

        let Some(_file_guard) = AdvisoryReflectorFileLock::try_acquire(
            &self.config.lock_path,
            &self.config.owner_id,
            self.config.owner_pid,
            now,
            self.config.lease_ttl_ms,
        )?
        else {
            report.lock_conflict = true;
            return Ok(report);
        };
        report.file_lock_acquired = true;

        let lease_acquired = store.try_acquire_reflector_lease(
            &self.config.scope_id,
            &self.config.owner_id,
            self.config.owner_pid,
            now,
            self.config.lease_ttl_ms,
        )?;
        if !lease_acquired {
            report.lock_conflict = true;
            return Ok(report);
        }
        report.lease_acquired = true;

        for _ in 0..self.config.max_jobs_per_tick.max(1) {
            let Some(job) = store.claim_next_reflector_job(
                &self.config.scope_id,
                &self.config.owner_id,
                now,
            )?
            else {
                break;
            };

            report.jobs_claimed += 1;

            match handler(store, &job) {
                Ok(()) => {
                    store.complete_reflector_job(&job.job_id, &self.config.owner_id, now)?;
                    report.jobs_completed += 1;
                }
                Err(message) => {
                    store.fail_reflector_job(
                        &job.job_id,
                        &self.config.owner_id,
                        &message,
                        now,
                        self.config.requeue_on_error,
                    )?;
                    report.jobs_failed += 1;
                }
            }

            let _ = store.heartbeat_reflector_lease(
                &self.config.scope_id,
                &self.config.owner_id,
                now,
                self.config.lease_ttl_ms,
            )?;
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(offset_ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(1_707_335_222_000 + offset_ms)
            .single()
            .expect("valid ts")
    }

    fn temp_lock_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "aoc-mind-reflector-runtime-{name}-{}-{}.lock",
            std::process::id(),
            Utc::now().timestamp_millis()
        ));
        path
    }

    #[test]
    fn worker_reports_lock_conflict_when_file_lock_is_busy() {
        let store = MindStore::open_in_memory().expect("db");
        let lock_path = temp_lock_path("busy");

        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .expect("lock file");
        lock_file.lock_exclusive().expect("hold file lock");

        let worker = DetachedReflectorWorker::new(ReflectorRuntimeConfig {
            scope_id: "scope-a".to_string(),
            owner_id: "owner-a".to_string(),
            owner_pid: Some(1),
            lock_path: lock_path.clone(),
            lease_ttl_ms: 1_000,
            max_jobs_per_tick: 1,
            requeue_on_error: false,
        });

        let report = worker
            .run_once(&store, ts(0), |_store, _job| Ok(()))
            .expect("run");
        assert!(report.lock_conflict);
        assert!(!report.file_lock_acquired);

        lock_file.unlock().expect("unlock");
        let _ = std::fs::remove_file(lock_path);
    }

    #[test]
    fn worker_takes_over_after_stale_lease_and_completes_job() {
        let store = MindStore::open_in_memory().expect("db");
        let lock_path = temp_lock_path("takeover");

        let now = ts(0);
        store
            .try_acquire_reflector_lease("scope-a", "owner-old", Some(1), now, 500)
            .expect("seed lease");

        store
            .enqueue_reflector_job(
                "mind",
                &["obs:1".to_string()],
                &["conv-1".to_string()],
                20,
                now,
            )
            .expect("job");

        let worker = DetachedReflectorWorker::new(ReflectorRuntimeConfig {
            scope_id: "scope-a".to_string(),
            owner_id: "owner-new".to_string(),
            owner_pid: Some(2),
            lock_path,
            lease_ttl_ms: 1_000,
            max_jobs_per_tick: 2,
            requeue_on_error: false,
        });

        let report = worker
            .run_once(&store, now + Duration::milliseconds(700), |_store, _job| {
                Ok(())
            })
            .expect("run");

        assert!(report.file_lock_acquired);
        assert!(report.lease_acquired);
        assert_eq!(report.jobs_claimed, 1);
        assert_eq!(report.jobs_completed, 1);
        assert_eq!(store.pending_reflector_jobs().expect("pending"), 0);
    }

    #[test]
    fn worker_requeues_failures_when_enabled() {
        let store = MindStore::open_in_memory().expect("db");
        let lock_path = temp_lock_path("requeue");

        let now = ts(0);
        store
            .try_acquire_reflector_lease("scope-a", "owner-a", Some(1), now, 1_000)
            .expect("lease");
        store
            .enqueue_reflector_job(
                "mind",
                &["obs:1".to_string()],
                &["conv-1".to_string()],
                20,
                now,
            )
            .expect("job");

        let worker = DetachedReflectorWorker::new(ReflectorRuntimeConfig {
            scope_id: "scope-a".to_string(),
            owner_id: "owner-a".to_string(),
            owner_pid: Some(1),
            lock_path,
            lease_ttl_ms: 1_000,
            max_jobs_per_tick: 1,
            requeue_on_error: true,
        });

        let report = worker
            .run_once(&store, now + Duration::milliseconds(10), |_store, _job| {
                Err("provider timeout".to_string())
            })
            .expect("run");

        assert_eq!(report.jobs_failed, 1);
        assert_eq!(store.pending_reflector_jobs().expect("pending"), 1);
    }
}
