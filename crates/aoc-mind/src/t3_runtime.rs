use aoc_storage::{MindStore, T3BacklogJob};
use chrono::{DateTime, Duration, Utc};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct T3RuntimeConfig {
    pub scope_id: String,
    pub owner_id: String,
    pub owner_pid: Option<i64>,
    pub lock_path: PathBuf,
    pub lease_ttl_ms: u64,
    pub stale_claim_after_ms: i64,
    pub max_jobs_per_tick: usize,
    pub requeue_on_error: bool,
    pub max_attempts: u16,
}

impl T3RuntimeConfig {
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
            stale_claim_after_ms: 60_000,
            max_jobs_per_tick: 4,
            requeue_on_error: true,
            max_attempts: 3,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct T3TickReport {
    pub file_lock_acquired: bool,
    pub lease_acquired: bool,
    pub lock_conflict: bool,
    pub jobs_claimed: usize,
    pub jobs_completed: usize,
    pub jobs_failed: usize,
    pub jobs_requeued: usize,
    pub jobs_dead_lettered: usize,
}

#[derive(Debug, Error)]
pub enum T3RuntimeError {
    #[error("storage error: {0}")]
    Storage(#[from] aoc_storage::StorageError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

struct AdvisoryT3FileLock {
    file: File,
}

impl AdvisoryT3FileLock {
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

impl Drop for AdvisoryT3FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub struct DetachedT3Worker {
    config: T3RuntimeConfig,
}

impl DetachedT3Worker {
    pub fn new(config: T3RuntimeConfig) -> Self {
        Self { config }
    }

    pub fn run_once<F>(
        &self,
        store: &MindStore,
        now: DateTime<Utc>,
        mut handler: F,
    ) -> Result<T3TickReport, T3RuntimeError>
    where
        F: FnMut(&MindStore, &T3BacklogJob) -> Result<(), String>,
    {
        let mut report = T3TickReport::default();

        let Some(_file_guard) = AdvisoryT3FileLock::try_acquire(
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

        let lease_acquired = store.try_acquire_t3_runtime_lease(
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

        let lease_ttl_ms = self.config.lease_ttl_ms.min(i64::MAX as u64) as i64;
        let stale_claim_after_ms = self.config.stale_claim_after_ms.max(lease_ttl_ms).max(1);

        for _ in 0..self.config.max_jobs_per_tick.max(1) {
            let Some(job) = store.claim_next_t3_backlog_job(
                &self.config.scope_id,
                &self.config.owner_id,
                now,
                stale_claim_after_ms,
            )?
            else {
                break;
            };

            report.jobs_claimed += 1;

            match handler(store, &job) {
                Ok(()) => {
                    store.complete_t3_backlog_job(&job.job_id, &self.config.owner_id, now)?;
                    report.jobs_completed += 1;
                }
                Err(message) => {
                    let will_requeue = self.config.requeue_on_error
                        && job.attempts < self.config.max_attempts.max(1);
                    store.fail_t3_backlog_job(
                        &job.job_id,
                        &self.config.owner_id,
                        &message,
                        now,
                        self.config.requeue_on_error,
                        self.config.max_attempts,
                    )?;
                    report.jobs_failed += 1;
                    if will_requeue {
                        report.jobs_requeued += 1;
                    } else {
                        report.jobs_dead_lettered += 1;
                    }
                }
            }

            let _ = store.heartbeat_t3_runtime_lease(
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
            "aoc-mind-t3-runtime-{name}-{}-{}.lock",
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

        let worker = DetachedT3Worker::new(T3RuntimeConfig {
            scope_id: "project:/repo".to_string(),
            owner_id: "owner-a".to_string(),
            owner_pid: Some(1),
            lock_path: lock_path.clone(),
            lease_ttl_ms: 1_000,
            stale_claim_after_ms: 1_000,
            max_jobs_per_tick: 1,
            requeue_on_error: true,
            max_attempts: 3,
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
            .try_acquire_t3_runtime_lease("project:/repo", "owner-old", Some(1), now, 500)
            .expect("seed lease");
        store
            .enqueue_t3_backlog_job(
                "/repo",
                "session-a",
                "12",
                Some("mind"),
                Some("obs:1"),
                Some("ref:1"),
                &["obs:1".to_string()],
                now,
            )
            .expect("enqueue");

        let worker = DetachedT3Worker::new(T3RuntimeConfig {
            scope_id: "project:/repo".to_string(),
            owner_id: "owner-new".to_string(),
            owner_pid: Some(2),
            lock_path,
            lease_ttl_ms: 1_000,
            stale_claim_after_ms: 1_000,
            max_jobs_per_tick: 2,
            requeue_on_error: true,
            max_attempts: 3,
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
        assert_eq!(store.pending_t3_backlog_jobs().expect("pending"), 0);
    }

    #[test]
    fn worker_requeues_then_dead_letters_after_max_attempts() {
        let store = MindStore::open_in_memory().expect("db");
        let lock_path = temp_lock_path("dead-letter");

        let now = ts(0);
        let (job_id, inserted) = store
            .enqueue_t3_backlog_job(
                "/repo",
                "session-a",
                "12",
                Some("mind"),
                Some("obs:1"),
                Some("ref:1"),
                &["obs:1".to_string()],
                now,
            )
            .expect("enqueue");
        assert!(inserted);

        let worker = DetachedT3Worker::new(T3RuntimeConfig {
            scope_id: "project:/repo".to_string(),
            owner_id: "owner-a".to_string(),
            owner_pid: Some(1),
            lock_path,
            lease_ttl_ms: 1_000,
            stale_claim_after_ms: 1_000,
            max_jobs_per_tick: 1,
            requeue_on_error: true,
            max_attempts: 2,
        });

        let first = worker
            .run_once(&store, now + Duration::milliseconds(10), |_store, _job| {
                Err("provider timeout".to_string())
            })
            .expect("first run");
        assert_eq!(first.jobs_failed, 1);
        assert_eq!(first.jobs_requeued, 1);
        assert_eq!(store.pending_t3_backlog_jobs().expect("pending"), 1);

        let second = worker
            .run_once(&store, now + Duration::milliseconds(20), |_store, _job| {
                Err("provider timeout".to_string())
            })
            .expect("second run");
        assert_eq!(second.jobs_failed, 1);
        assert_eq!(second.jobs_dead_lettered, 1);

        let job = store
            .t3_backlog_job_by_id(&job_id)
            .expect("load")
            .expect("job exists");
        assert_eq!(job.attempts, 2);
        assert_eq!(job.status, aoc_storage::T3BacklogJobStatus::Failed);
    }
}
