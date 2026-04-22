use aoc_pi_adapter::{IngestionOptions, IngestionReport, PiAdapterError, PiSessionIngestor};
use aoc_storage::{LegacyImportReport, MindStore, StorageError};
use chrono::{DateTime, Duration, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindProjectPaths {
    pub project_root: PathBuf,
    pub runtime_root: PathBuf,
    pub store_path: PathBuf,
    pub legacy_root: PathBuf,
    pub locks_dir: PathBuf,
    pub reflector_lock_path: PathBuf,
    pub t3_lock_path: PathBuf,
    pub reflector_dispatch_lock_path: PathBuf,
    pub t3_dispatch_lock_path: PathBuf,
    pub service_lock_path: PathBuf,
    pub health_snapshot_path: PathBuf,
}

impl MindProjectPaths {
    pub fn for_project_root(project_root: impl AsRef<Path>) -> Self {
        let project_root = project_root.as_ref().to_path_buf();
        let runtime_root = mind_runtime_root(&project_root);
        let locks_dir = runtime_root.join("locks");
        Self {
            project_root,
            store_path: runtime_root.join("project.sqlite"),
            legacy_root: runtime_root.join("legacy"),
            reflector_lock_path: locks_dir.join("reflector.lock"),
            t3_lock_path: locks_dir.join("t3.lock"),
            reflector_dispatch_lock_path: locks_dir.join("reflector-dispatch.lock"),
            t3_dispatch_lock_path: locks_dir.join("t3-dispatch.lock"),
            service_lock_path: locks_dir.join("service.lock"),
            health_snapshot_path: runtime_root.join("service-health.json"),
            locks_dir,
            runtime_root,
        }
    }

    pub fn legacy_store_path_for_session(&self, session_id: &str, pane_id: &str) -> PathBuf {
        self.legacy_root
            .join(sanitize_runtime_component(session_id))
            .join(format!("{}.sqlite", sanitize_runtime_component(pane_id)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandalonePiSyncReport {
    pub session_file: PathBuf,
    pub report: IngestionReport,
}

pub struct OpenedMindProjectStore {
    pub store: MindStore,
    pub store_path: PathBuf,
    pub legacy_path: Option<PathBuf>,
    pub legacy_import_report: LegacyImportReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindServiceLease {
    pub owner_id: String,
    pub owner_pid: Option<i64>,
    pub session_id: String,
    pub pane_id: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub const DEFAULT_MIND_SERVICE_STALE_AFTER_MS: i64 = 90_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MindServiceHealthSnapshot {
    #[serde(default)]
    pub owner_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_pid: Option<i64>,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub pane_id: String,
    #[serde(default)]
    pub lifecycle: String,
    #[serde(default)]
    pub reflector_enabled: bool,
    #[serde(default)]
    pub reflector_ticks: u64,
    #[serde(default)]
    pub reflector_lock_conflicts: u64,
    #[serde(default)]
    pub reflector_jobs_completed: u64,
    #[serde(default)]
    pub reflector_jobs_failed: u64,
    #[serde(default)]
    pub t3_enabled: bool,
    #[serde(default)]
    pub t3_ticks: u64,
    #[serde(default)]
    pub t3_lock_conflicts: u64,
    #[serde(default)]
    pub t3_jobs_completed: u64,
    #[serde(default)]
    pub t3_jobs_failed: u64,
    #[serde(default)]
    pub t3_jobs_requeued: u64,
    #[serde(default)]
    pub t3_jobs_dead_lettered: u64,
    #[serde(default)]
    pub t3_queue_depth: i64,
    #[serde(default)]
    pub supervisor_runs: u64,
    #[serde(default)]
    pub supervisor_failures: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_tick_ms: Option<i64>,
    #[serde(default)]
    pub queue_depth: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindServiceStatusSummary {
    pub state: String,
    pub stale: bool,
    pub lease_active: bool,
    pub heartbeat_fresh: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
}

pub fn summarize_mind_service_status(
    lease: Option<&MindServiceLease>,
    health: Option<&MindServiceHealthSnapshot>,
    now_ms: i64,
) -> MindServiceStatusSummary {
    let lease_expiry_ms = lease
        .map(|lease| lease.expires_at.timestamp_millis())
        .or_else(|| health.and_then(|snapshot| snapshot.lease_expires_at_ms));
    let lease_active = lease_expiry_ms.map(|value| value >= now_ms).unwrap_or(false);
    let heartbeat_fresh = health
        .and_then(|snapshot| snapshot.last_heartbeat_ms)
        .map(|value| now_ms.saturating_sub(value) <= DEFAULT_MIND_SERVICE_STALE_AFTER_MS)
        .unwrap_or(false);
    let lifecycle = health
        .map(|snapshot| snapshot.lifecycle.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let last_error = health
        .and_then(|snapshot| snapshot.last_error.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if lease_expiry_ms.map(|value| value < now_ms).unwrap_or(false) {
        return MindServiceStatusSummary {
            state: "stale".to_string(),
            stale: true,
            lease_active: false,
            heartbeat_fresh,
            blocker: Some("service lease expired".to_string()),
        };
    }

    if health.is_some() && !heartbeat_fresh {
        return MindServiceStatusSummary {
            state: "stale".to_string(),
            stale: true,
            lease_active,
            heartbeat_fresh: false,
            blocker: Some("service heartbeat stale".to_string()),
        };
    }

    if lease_active {
        if matches!(lifecycle, "error" | "needs-input" | "blocked") || last_error.is_some() {
            return MindServiceStatusSummary {
                state: "degraded".to_string(),
                stale: false,
                lease_active: true,
                heartbeat_fresh,
                blocker: last_error
                    .map(|value| value.to_string())
                    .or_else(|| Some(format!("service lifecycle {lifecycle}"))),
            };
        }
        return MindServiceStatusSummary {
            state: "running".to_string(),
            stale: false,
            lease_active: true,
            heartbeat_fresh,
            blocker: None,
        };
    }

    if health.is_some() || lease.is_some() {
        return MindServiceStatusSummary {
            state: if lifecycle == "idle" { "idle" } else { "inactive" }.to_string(),
            stale: false,
            lease_active: false,
            heartbeat_fresh,
            blocker: last_error.map(|value| value.to_string()),
        };
    }

    MindServiceStatusSummary {
        state: "cold".to_string(),
        stale: false,
        lease_active: false,
        heartbeat_fresh: false,
        blocker: None,
    }
}

pub struct MindServiceLeaseGuard {
    file: File,
    path: PathBuf,
    lease: MindServiceLease,
}

#[derive(Debug, Error)]
pub enum StandaloneMindError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("pi adapter error: {0}")]
    PiAdapter(#[from] PiAdapterError),
}

pub fn mind_runtime_root(project_root: &Path) -> PathBuf {
    resolve_aoc_state_home()
        .join("aoc")
        .join("mind")
        .join("projects")
        .join(sanitize_runtime_component(&project_root.to_string_lossy()))
}

pub fn mind_store_path_with_override(project_root: &Path, override_path: Option<&str>) -> PathBuf {
    if let Some(path) = override_path.map(str::trim).filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    MindProjectPaths::for_project_root(project_root).store_path
}

pub fn legacy_mind_store_path(project_root: &Path, session_id: &str, pane_id: &str) -> PathBuf {
    MindProjectPaths::for_project_root(project_root)
        .legacy_store_path_for_session(session_id, pane_id)
}

pub fn reflector_lock_path_with_override(
    project_root: &Path,
    override_path: Option<&str>,
) -> PathBuf {
    if let Some(path) = override_path.map(str::trim).filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    MindProjectPaths::for_project_root(project_root).reflector_lock_path
}

pub fn t3_lock_path_with_override(project_root: &Path, override_path: Option<&str>) -> PathBuf {
    if let Some(path) = override_path.map(str::trim).filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    MindProjectPaths::for_project_root(project_root).t3_lock_path
}

pub fn reflector_dispatch_lock_path(project_root: &Path) -> PathBuf {
    MindProjectPaths::for_project_root(project_root).reflector_dispatch_lock_path
}

pub fn t3_dispatch_lock_path(project_root: &Path) -> PathBuf {
    MindProjectPaths::for_project_root(project_root).t3_dispatch_lock_path
}

pub fn default_pi_session_root(project_root: &Path) -> Option<PathBuf> {
    let settings_parent = env::var("AOC_PI_SETTINGS_PATH")
        .ok()
        .map(PathBuf::from)
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let home = env::var("HOME").ok().map(PathBuf::from);
    default_pi_session_root_with_env(project_root, settings_parent.as_deref(), home.as_deref())
}

pub fn latest_pi_session_file(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut newest: Option<(SystemTime, PathBuf)> = None;

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file()
                || path.extension().and_then(|ext| ext.to_str()) != Some("jsonl")
            {
                continue;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            match &newest {
                Some((current_modified, current_path)) => {
                    if modified > *current_modified
                        || (modified == *current_modified && path > *current_path)
                    {
                        newest = Some((modified, path));
                    }
                }
                None => newest = Some((modified, path)),
            }
        }
    }

    newest.map(|(_, path)| path)
}

pub fn discover_latest_pi_session_file(project_root: &Path) -> Option<PathBuf> {
    if let Ok(session_dir) = env::var("AOC_PI_SESSION_DIR") {
        let session_dir = PathBuf::from(session_dir.trim());
        if !session_dir.as_os_str().is_empty() {
            if let Some(path) = latest_pi_session_file(&session_dir) {
                return Some(path);
            }
        }
    }

    default_pi_session_root(project_root).and_then(|root| latest_pi_session_file(&root))
}

pub fn open_project_store(
    project_root: &Path,
    session_id: &str,
    pane_id: &str,
    store_override: Option<&str>,
) -> Result<OpenedMindProjectStore, StandaloneMindError> {
    let store_path = mind_store_path_with_override(project_root, store_override);
    if let Some(parent) = store_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let store = MindStore::open(&store_path)?;

    let mut legacy_path = None;
    let mut legacy_import_report = LegacyImportReport::default();
    if store_override
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .is_none()
    {
        let candidate = legacy_mind_store_path(project_root, session_id, pane_id);
        if candidate != store_path && candidate.exists() {
            legacy_import_report = store.import_legacy_store(&candidate)?;
            legacy_path = Some(candidate);
        }
    }

    Ok(OpenedMindProjectStore {
        store,
        store_path,
        legacy_path,
        legacy_import_report,
    })
}

impl MindServiceLeaseGuard {
    pub fn acquire(
        project_root: &Path,
        owner_id: &str,
        session_id: &str,
        pane_id: &str,
        ttl_ms: u64,
    ) -> Result<Self, StandaloneMindError> {
        let paths = MindProjectPaths::for_project_root(project_root);
        if let Some(parent) = paths.service_lock_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&paths.service_lock_path)?;
        file.try_lock_exclusive().map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!(
                    "mind service already active for {}: {err}",
                    project_root.display()
                ),
            )
        })?;

        let now = Utc::now();
        let lease = MindServiceLease {
            owner_id: owner_id.to_string(),
            owner_pid: Some(std::process::id() as i64),
            session_id: session_id.to_string(),
            pane_id: pane_id.to_string(),
            acquired_at: now,
            expires_at: now + Duration::milliseconds(ttl_ms.min(i64::MAX as u64) as i64),
        };
        let mut guard = Self {
            file,
            path: paths.service_lock_path,
            lease,
        };
        guard.write_metadata()?;
        Ok(guard)
    }

    pub fn lease(&self) -> &MindServiceLease {
        &self.lease
    }

    pub fn heartbeat(&mut self, ttl_ms: u64) -> Result<(), StandaloneMindError> {
        self.lease.expires_at =
            Utc::now() + Duration::milliseconds(ttl_ms.min(i64::MAX as u64) as i64);
        self.write_metadata()
    }

    fn write_metadata(&mut self) -> Result<(), StandaloneMindError> {
        let body = serde_json::to_vec_pretty(&self.lease)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&body)?;
        self.file.flush()?;
        Ok(())
    }
}

impl Drop for MindServiceLeaseGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = self.file.unlock();
    }
}

pub fn read_mind_service_lease(
    project_root: &Path,
) -> Result<Option<MindServiceLease>, StandaloneMindError> {
    let path = MindProjectPaths::for_project_root(project_root).service_lock_path;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    let lease = serde_json::from_slice(&bytes)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(Some(lease))
}

pub fn write_mind_service_health_snapshot(
    project_root: &Path,
    snapshot: &MindServiceHealthSnapshot,
) -> Result<(), StandaloneMindError> {
    let path = MindProjectPaths::for_project_root(project_root).health_snapshot_path;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(snapshot)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    fs::write(path, body)?;
    Ok(())
}

pub fn read_mind_service_health_snapshot(
    project_root: &Path,
) -> Result<Option<MindServiceHealthSnapshot>, StandaloneMindError> {
    let path = MindProjectPaths::for_project_root(project_root).health_snapshot_path;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    let snapshot = serde_json::from_slice(&bytes)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(Some(snapshot))
}

pub fn sync_session_file_into_project_store(
    project_root: &Path,
    agent_id: &str,
    session_file: &Path,
) -> Result<StandalonePiSyncReport, StandaloneMindError> {
    let opened = open_project_store(project_root, "standalone", "service", None)?;
    let ingestor = PiSessionIngestor::new(IngestionOptions::default());
    let report = ingestor.ingest_session_file(&opened.store, agent_id, session_file)?;
    Ok(StandalonePiSyncReport {
        session_file: session_file.to_path_buf(),
        report,
    })
}

pub fn sync_latest_pi_session_into_project_store(
    project_root: &Path,
    agent_id: &str,
) -> Result<Option<StandalonePiSyncReport>, StandaloneMindError> {
    let Some(session_file) = discover_latest_pi_session_file(project_root) else {
        return Ok(None);
    };
    sync_session_file_into_project_store(project_root, agent_id, &session_file).map(Some)
}

fn default_pi_session_root_with_env(
    project_root: &Path,
    settings_parent: Option<&Path>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    let agent_root = settings_parent
        .map(Path::to_path_buf)
        .or_else(|| home.map(|home| home.join(".pi").join("agent")))?;
    let bucket = format!(
        "--{}--",
        project_root
            .to_string_lossy()
            .replace('/', "-")
            .trim_matches('-')
    );
    Some(agent_root.join("sessions").join(bucket))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration as StdDuration;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        env_lock().lock().unwrap_or_else(|err| err.into_inner())
    }

    fn temp_path(label: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "aoc-mind-standalone-test-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    #[test]
    fn project_paths_match_expected_layout() {
        let project_root = PathBuf::from("/tmp/example/project");
        let paths = MindProjectPaths::for_project_root(&project_root);
        assert!(paths.store_path.ends_with("project.sqlite"));
        assert!(paths.reflector_lock_path.ends_with("locks/reflector.lock"));
        assert!(paths
            .t3_dispatch_lock_path
            .ends_with("locks/t3-dispatch.lock"));
        assert!(paths
            .legacy_store_path_for_session("session-test", "12")
            .ends_with("legacy/session-test/12.sqlite"));
    }

    #[test]
    fn latest_pi_session_file_prefers_newest_jsonl() {
        let dir = temp_path("latest");
        let nested = dir.join("nested");
        fs::create_dir_all(&nested).expect("nested dir");
        let older = nested.join("older.jsonl");
        let newer = nested.join("newer.jsonl");
        fs::write(&older, b"{}\n").expect("older");
        std::thread::sleep(StdDuration::from_millis(10));
        fs::write(&newer, b"{}\n").expect("newer");
        assert_eq!(
            latest_pi_session_file(&dir).as_deref(),
            Some(newer.as_path())
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_session_file_into_project_store_ingests_pi_jsonl() {
        let _guard = env_guard();
        let project_root = temp_path("project");
        let state_home = temp_path("state");
        let session_file = project_root.join("session.jsonl");
        fs::write(
            &session_file,
            concat!(
                "{\"type\":\"session\",\"version\":3,\"id\":\"sess-standalone\",\"timestamp\":\"2024-12-03T14:00:00.000Z\",\"cwd\":\"/tmp/proj\"}\n",
                "{\"type\":\"message\",\"id\":\"a1\",\"parentId\":null,\"timestamp\":\"2024-12-03T14:00:02.000Z\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"hello from standalone\"}]}}\n"
            ),
        )
        .expect("session file");

        let previous_state = env::var("XDG_STATE_HOME").ok();
        env::set_var("XDG_STATE_HOME", &state_home);
        let store_path = mind_runtime_root(&project_root).join("project.sqlite");
        let result = sync_session_file_into_project_store(
            &project_root,
            "mind-standalone-test",
            &session_file,
        )
        .expect("sync");
        if let Some(value) = previous_state {
            env::set_var("XDG_STATE_HOME", value);
        } else {
            env::remove_var("XDG_STATE_HOME");
        }

        assert_eq!(result.report.conversation_id, "pi:sess-standalone");
        assert_eq!(result.report.processed_raw_events, 1);
        assert_eq!(result.report.produced_t0_events, 1);

        let store = MindStore::open(store_path).expect("open store");
        let events = store
            .t0_events_for_conversation("pi:sess-standalone")
            .expect("t0 events");
        assert_eq!(events.len(), 1);

        let _ = fs::remove_dir_all(&project_root);
        let _ = fs::remove_dir_all(&state_home);
    }

    #[test]
    fn open_project_store_imports_legacy_when_present() {
        let _guard = env_guard();
        let project_root = temp_path("legacy-project");
        let state_home = temp_path("legacy-state");
        let previous_state = env::var("XDG_STATE_HOME").ok();
        env::set_var("XDG_STATE_HOME", &state_home);

        let paths = MindProjectPaths::for_project_root(&project_root);
        let legacy_path = paths.legacy_store_path_for_session("session-test", "12");
        fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
        let legacy = MindStore::open(&legacy_path).expect("legacy store");
        let raw = aoc_core::mind_contracts::RawEvent {
            event_id: "legacy-evt-1".to_string(),
            conversation_id: "legacy-conv".to_string(),
            agent_id: "legacy-agent".to_string(),
            ts: chrono::Utc::now(),
            body: aoc_core::mind_contracts::RawEventBody::Message(
                aoc_core::mind_contracts::MessageEvent {
                    role: aoc_core::mind_contracts::ConversationRole::Assistant,
                    text: "legacy imported raw event".to_string(),
                },
            ),
            attrs: std::collections::BTreeMap::new(),
        };
        legacy
            .insert_raw_event(&raw)
            .expect("insert legacy raw event");

        let opened = open_project_store(&project_root, "session-test", "12", None)
            .expect("open project store");

        if let Some(value) = previous_state {
            env::set_var("XDG_STATE_HOME", value);
        } else {
            env::remove_var("XDG_STATE_HOME");
        }

        assert_eq!(opened.legacy_path.as_deref(), Some(legacy_path.as_path()));
        assert!(opened.legacy_import_report.rows_imported >= 1);
        let imported = opened
            .store
            .raw_event_by_id("legacy-evt-1")
            .expect("raw event query")
            .expect("legacy imported");
        assert_eq!(imported.conversation_id, "legacy-conv");

        let _ = fs::remove_dir_all(&project_root);
        let _ = fs::remove_dir_all(&state_home);
    }

    #[test]
    fn default_pi_session_root_uses_settings_parent() {
        let root = default_pi_session_root_with_env(
            Path::new("/tmp/proj"),
            Some(Path::new("/tmp/pi-agent")),
            None,
        )
        .expect("root");
        assert!(root.ends_with("sessions/--tmp-proj--"));
    }

    #[test]
    fn explicit_overrides_and_legacy_paths_are_supported() {
        let project_root = Path::new("/repo");
        assert_eq!(
            mind_store_path_with_override(project_root, Some("/tmp/custom-mind.sqlite")),
            PathBuf::from("/tmp/custom-mind.sqlite")
        );
        assert_eq!(
            reflector_lock_path_with_override(project_root, Some("/tmp/custom-reflector.lock")),
            PathBuf::from("/tmp/custom-reflector.lock")
        );
        assert_eq!(
            t3_lock_path_with_override(project_root, Some("/tmp/custom-t3.lock")),
            PathBuf::from("/tmp/custom-t3.lock")
        );
        assert_eq!(
            legacy_mind_store_path(project_root, "session-test", "12"),
            mind_runtime_root(project_root)
                .join("legacy")
                .join("session-test")
                .join("12.sqlite")
        );
        assert_eq!(
            reflector_dispatch_lock_path(project_root),
            mind_runtime_root(project_root)
                .join("locks")
                .join("reflector-dispatch.lock")
        );
        assert_eq!(
            t3_dispatch_lock_path(project_root),
            mind_runtime_root(project_root)
                .join("locks")
                .join("t3-dispatch.lock")
        );
    }

    #[test]
    fn summarize_mind_service_status_reports_running_and_stale_states() {
        let now_ms = Utc::now().timestamp_millis();
        let running_lease = MindServiceLease {
            owner_id: "agent-test".to_string(),
            owner_pid: Some(7),
            session_id: "session-test".to_string(),
            pane_id: "pane-test".to_string(),
            acquired_at: Utc::now(),
            expires_at: Utc::now() + Duration::milliseconds(30_000),
        };
        let running_health = MindServiceHealthSnapshot {
            lifecycle: "running".to_string(),
            last_heartbeat_ms: Some(now_ms),
            lease_expires_at_ms: Some(running_lease.expires_at.timestamp_millis()),
            ..MindServiceHealthSnapshot::default()
        };
        let running = summarize_mind_service_status(
            Some(&running_lease),
            Some(&running_health),
            now_ms,
        );
        assert_eq!(running.state, "running");
        assert!(!running.stale);
        assert!(running.lease_active);
        assert!(running.heartbeat_fresh);
        assert!(running.blocker.is_none());

        let stale_health = MindServiceHealthSnapshot {
            lifecycle: "running".to_string(),
            last_heartbeat_ms: Some(now_ms - (DEFAULT_MIND_SERVICE_STALE_AFTER_MS + 1_000)),
            lease_expires_at_ms: Some(now_ms + 30_000),
            ..MindServiceHealthSnapshot::default()
        };
        let stale = summarize_mind_service_status(None, Some(&stale_health), now_ms);
        assert_eq!(stale.state, "stale");
        assert!(stale.stale);
        assert_eq!(stale.blocker.as_deref(), Some("service heartbeat stale"));
    }

    #[test]
    fn service_lease_guard_persists_metadata_and_snapshot_roundtrips() {
        let _guard = env_guard();
        let project_root = temp_path("service-lease");
        let state_home = temp_path("service-state");
        let previous_state = env::var("XDG_STATE_HOME").ok();
        env::set_var("XDG_STATE_HOME", &state_home);

        let mut lease = MindServiceLeaseGuard::acquire(
            &project_root,
            "agent-test",
            "session-test",
            "pane-test",
            30_000,
        )
        .expect("acquire service lease");
        lease.heartbeat(30_000).expect("heartbeat lease");

        let persisted_lease = read_mind_service_lease(&project_root)
            .expect("read lease")
            .expect("lease present");
        assert_eq!(persisted_lease.owner_id, "agent-test");
        assert_eq!(persisted_lease.session_id, "session-test");
        assert_eq!(persisted_lease.pane_id, "pane-test");

        let snapshot = MindServiceHealthSnapshot {
            owner_id: "agent-test".to_string(),
            session_id: "session-test".to_string(),
            pane_id: "pane-test".to_string(),
            lifecycle: "running".to_string(),
            queue_depth: 2,
            t3_queue_depth: 1,
            last_heartbeat_ms: Some(Utc::now().timestamp_millis()),
            lease_expires_at_ms: Some(persisted_lease.expires_at.timestamp_millis()),
            ..MindServiceHealthSnapshot::default()
        };
        write_mind_service_health_snapshot(&project_root, &snapshot).expect("write snapshot");
        let roundtrip = read_mind_service_health_snapshot(&project_root)
            .expect("read snapshot")
            .expect("snapshot present");
        assert_eq!(roundtrip.lifecycle, "running");
        assert_eq!(roundtrip.queue_depth, 2);
        assert_eq!(roundtrip.t3_queue_depth, 1);

        drop(lease);
        assert!(read_mind_service_lease(&project_root)
            .expect("read released")
            .is_none());

        if let Some(value) = previous_state {
            env::set_var("XDG_STATE_HOME", value);
        } else {
            env::remove_var("XDG_STATE_HOME");
        }
        let _ = fs::remove_dir_all(&project_root);
        let _ = fs::remove_dir_all(&state_home);
    }
}
