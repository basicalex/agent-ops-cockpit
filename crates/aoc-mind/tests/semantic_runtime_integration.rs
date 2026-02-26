use aoc_core::mind_contracts::{
    compact_raw_event_to_t0, ConversationRole, MessageEvent, ObserverAdapter, ObserverInput,
    ObserverOutput, RawEvent, RawEventBody, SemanticAdapterError, SemanticFailureKind,
    SemanticGuardrails, SemanticModelProfile, T0CompactionPolicy,
};
use aoc_mind::{
    DetachedReflectorWorker, DistillationConfig, ReflectorRuntimeConfig, SemanticObserverConfig,
    SessionObserverSidecar,
};
use aoc_storage::{MindStore, ReflectorJobStatus};
use chrono::{DateTime, Duration, TimeZone, Utc};
use fs2::FileExt;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::PathBuf;

fn ts(offset_ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(1_708_995_600_000 + offset_ms)
        .single()
        .expect("valid timestamp")
}

fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "aoc-mind-{prefix}-{}-{}.{}",
        std::process::id(),
        Utc::now().timestamp_micros(),
        suffix
    ));
    path
}

fn insert_t0(
    store: &MindStore,
    event_id: &str,
    conversation_id: &str,
    ts: DateTime<Utc>,
    text: &str,
) {
    let raw = RawEvent {
        event_id: event_id.to_string(),
        conversation_id: conversation_id.to_string(),
        agent_id: "agent-1".to_string(),
        ts,
        body: RawEventBody::Message(MessageEvent {
            role: ConversationRole::User,
            text: text.to_string(),
        }),
        attrs: Default::default(),
    };
    let compact = compact_raw_event_to_t0(&raw, &T0CompactionPolicy::default())
        .expect("compact")
        .expect("keep message");
    store.upsert_t0_compact_event(&compact).expect("insert t0");
}

struct RoutingObserverAdapter {
    by_conversation: BTreeMap<String, Result<ObserverOutput, SemanticAdapterError>>,
}

impl ObserverAdapter for RoutingObserverAdapter {
    fn observe_t1(
        &self,
        input: &ObserverInput,
        _profile: &SemanticModelProfile,
        _guardrails: &SemanticGuardrails,
    ) -> Result<ObserverOutput, SemanticAdapterError> {
        self.by_conversation
            .get(&input.conversation_id)
            .cloned()
            .unwrap_or_else(|| {
                Err(SemanticAdapterError::new(
                    SemanticFailureKind::ProviderError,
                    format!("no scripted result for {}", input.conversation_id),
                ))
            })
    }
}

#[test]
fn multi_session_sidecar_processes_each_session_independently() {
    let store = MindStore::open_in_memory().expect("open db");
    insert_t0(&store, "e1", "conv-a", ts(0), "session a observer input");
    insert_t0(&store, "e2", "conv-b", ts(10), "session b observer input");

    let adapter = RoutingObserverAdapter {
        by_conversation: BTreeMap::from([
            (
                "conv-a".to_string(),
                Ok(ObserverOutput {
                    summary: "semantic summary A".to_string(),
                    key_points: vec!["a1".to_string()],
                    citations: vec![],
                }),
            ),
            (
                "conv-b".to_string(),
                Ok(ObserverOutput {
                    summary: "semantic summary B".to_string(),
                    key_points: vec!["b1".to_string()],
                    citations: vec![],
                }),
            ),
        ]),
    };

    let mut distill = DistillationConfig::default();
    distill.enable_attribution = false;
    distill.t2_trigger_tokens = 9_999;

    let mut sidecar =
        SessionObserverSidecar::new(distill, SemanticObserverConfig::default(), adapter);

    let now = ts(1000);
    sidecar.enqueue_turn("session-a", "conv-a", now);
    sidecar.enqueue_turn("session-b", "conv-b", now + Duration::milliseconds(10));

    let outcomes = sidecar.run_ready(&store, now + Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|outcome| outcome.report.is_ok()));

    let conv_a = store
        .artifacts_for_conversation("conv-a")
        .expect("conv-a artifacts");
    let conv_b = store
        .artifacts_for_conversation("conv-b")
        .expect("conv-b artifacts");

    assert_eq!(conv_a.len(), 1);
    assert_eq!(conv_b.len(), 1);
    assert!(conv_a[0].text.contains("semantic summary A"));
    assert!(conv_b[0].text.contains("semantic summary B"));
}

#[test]
fn semantic_timeout_fails_open_to_deterministic_with_provenance() {
    let store = MindStore::open_in_memory().expect("open db");
    insert_t0(
        &store,
        "e1",
        "conv-timeout",
        ts(0),
        "provider timeout should still produce deterministic artifact",
    );

    let adapter = RoutingObserverAdapter {
        by_conversation: BTreeMap::from([(
            "conv-timeout".to_string(),
            Err(SemanticAdapterError::new(
                SemanticFailureKind::Timeout,
                "scripted timeout",
            )),
        )]),
    };

    let mut distill = DistillationConfig::default();
    distill.enable_attribution = false;
    distill.t2_trigger_tokens = 9_999;

    let mut semantic = SemanticObserverConfig::default();
    semantic.guardrails.max_retries = 0;

    let mut sidecar = SessionObserverSidecar::new(distill, semantic, adapter);
    let now = ts(1000);
    sidecar.enqueue_turn("session-timeout", "conv-timeout", now);

    let outcomes = sidecar.run_ready(&store, now + Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].report.is_ok());

    let artifacts = store
        .artifacts_for_conversation("conv-timeout")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].text.starts_with("T1 observation"));

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 2);
    assert_eq!(
        provenance[0].failure_kind,
        Some(SemanticFailureKind::Timeout)
    );
    assert_eq!(provenance[0].attempt_count, 1);
    assert_eq!(provenance[1].attempt_count, 2);
}

#[test]
fn reflector_lock_contention_then_stale_lease_takeover_processes_job_once() {
    let db_path = unique_temp_path("reflector", "db");
    let lock_path = unique_temp_path("reflector", "lock");
    let store = MindStore::open(&db_path).expect("open db");

    let now = ts(0);
    store
        .try_acquire_reflector_lease("scope-a", "owner-old", Some(1), now, 500)
        .expect("seed old lease");
    let job_id = store
        .enqueue_reflector_job(
            "mind",
            &["obs:1".to_string()],
            &["conv-1".to_string()],
            32,
            now,
        )
        .expect("enqueue job");

    let external_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .expect("open lock");
    external_lock.lock_exclusive().expect("hold lock");

    let mut runtime = ReflectorRuntimeConfig::with_guardrails(
        "scope-a",
        "owner-new",
        lock_path.clone(),
        &SemanticGuardrails {
            reflector_lease_ttl_ms: 1_000,
            ..SemanticGuardrails::default()
        },
    );
    runtime.owner_pid = Some(42);
    runtime.max_jobs_per_tick = 2;

    let worker = DetachedReflectorWorker::new(runtime);

    let report_conflict = worker
        .run_once(&store, now + Duration::milliseconds(100), |_store, _job| {
            Ok(())
        })
        .expect("run conflict");
    assert!(report_conflict.lock_conflict);
    assert_eq!(report_conflict.jobs_claimed, 0);

    external_lock.unlock().expect("unlock");

    let report_takeover = worker
        .run_once(&store, now + Duration::milliseconds(700), |_store, _job| {
            Ok(())
        })
        .expect("run takeover");
    assert!(report_takeover.lease_acquired);
    assert_eq!(report_takeover.jobs_claimed, 1);
    assert_eq!(report_takeover.jobs_completed, 1);

    let report_noop = worker
        .run_once(&store, now + Duration::milliseconds(900), |_store, _job| {
            Ok(())
        })
        .expect("run noop");
    assert_eq!(report_noop.jobs_claimed, 0);

    let job = store
        .reflector_job_by_id(&job_id)
        .expect("load job")
        .expect("job exists");
    assert_eq!(job.status, ReflectorJobStatus::Completed);

    let _ = std::fs::remove_file(lock_path);
    let _ = std::fs::remove_file(db_path);
}
