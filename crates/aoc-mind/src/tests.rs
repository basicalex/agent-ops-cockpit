use super::*;
use aoc_core::mind_contracts::{
    canonical_lineage_attrs, compact_raw_event_to_t0, ConversationLineageMetadata,
    ConversationRole, MessageEvent, ObserverAdapter, ObserverInput, ObserverOutput, RawEvent,
    RawEventBody, SemanticAdapterError, SemanticFailureKind, SemanticGuardrails,
    SemanticModelProfile, T0CompactionPolicy,
};
use aoc_core::mind_observer_feed::MindInjectionTriggerKind;
use aoc_storage::{
    MindStore, ReflectorJob, ReflectorJobStatus, StoredArtifact, T3BacklogJob, T3BacklogJobStatus,
};
use chrono::{DateTime, TimeZone, Utc};
use std::cell::RefCell;
use std::thread;
use std::time::Duration;

fn ts(hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 2, 23, hour, min, sec)
        .single()
        .expect("valid timestamp")
}

fn raw_message(event_id: &str, conversation_id: &str, ts: DateTime<Utc>, text: &str) -> RawEvent {
    RawEvent {
        event_id: event_id.to_string(),
        conversation_id: conversation_id.to_string(),
        agent_id: "agent-1".to_string(),
        ts,
        body: RawEventBody::Message(MessageEvent {
            role: ConversationRole::User,
            text: text.to_string(),
        }),
        attrs: Default::default(),
    }
}

fn insert_t0(
    store: &MindStore,
    event_id: &str,
    conversation_id: &str,
    ts: DateTime<Utc>,
    text: &str,
) {
    let raw = raw_message(event_id, conversation_id, ts, text);
    let compact = compact_raw_event_to_t0(&raw, &T0CompactionPolicy::default())
        .expect("compact")
        .expect("kept");
    store.upsert_t0_compact_event(&compact).expect("insert t0");
}

#[derive(Clone)]
struct StaticObserverAdapter {
    result: Result<ObserverOutput, SemanticAdapterError>,
}

impl ObserverAdapter for StaticObserverAdapter {
    fn observe_t1(
        &self,
        _input: &ObserverInput,
        _profile: &SemanticModelProfile,
        _guardrails: &SemanticGuardrails,
    ) -> Result<ObserverOutput, SemanticAdapterError> {
        self.result.clone()
    }
}

struct SequenceObserverAdapter {
    scripted_results: RefCell<Vec<Result<ObserverOutput, SemanticAdapterError>>>,
    delay_ms: u64,
}

impl ObserverAdapter for SequenceObserverAdapter {
    fn observe_t1(
        &self,
        _input: &ObserverInput,
        _profile: &SemanticModelProfile,
        _guardrails: &SemanticGuardrails,
    ) -> Result<ObserverOutput, SemanticAdapterError> {
        if self.delay_ms > 0 {
            thread::sleep(Duration::from_millis(self.delay_ms));
        }

        let mut scripted = self.scripted_results.borrow_mut();
        if scripted.is_empty() {
            return Err(SemanticAdapterError::new(
                SemanticFailureKind::ProviderError,
                "no scripted observer result",
            ));
        }

        scripted.remove(0)
    }
}

#[test]
fn under_budget_runs_single_pass_t1() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(&store, "e1", "conv-1", ts(12, 0, 0), "short one");
    insert_t0(&store, "e2", "conv-1", ts(12, 0, 1), "short two");

    let mut config = DistillationConfig::default();
    config.enable_attribution = false;
    let distiller = DeterministicDistiller::new(config);
    let report = distiller
        .distill_conversation(&store, "conv-1")
        .expect("distill");

    assert_eq!(report.t1_batches_planned, 1);
    assert_eq!(report.t1_artifacts_written, 1);
    assert!(!report.chunked_t1);

    let artifacts = store
        .artifacts_for_conversation("conv-1")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].kind, "t1");
    assert_eq!(artifacts[0].trace_ids.len(), 2);

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 1);
    assert_eq!(provenance[0].runtime, SemanticRuntime::Deterministic);
    assert_eq!(provenance[0].stage, SemanticStage::T1Observer);
}

#[test]
fn over_budget_chunks_with_deterministic_order_and_traceability() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-2",
        ts(12, 10, 0),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    insert_t0(
        &store,
        "e2",
        "conv-2",
        ts(12, 10, 1),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    insert_t0(
        &store,
        "e3",
        "conv-2",
        ts(12, 10, 2),
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    );

    let mut config = DistillationConfig::default();
    config.t1_target_tokens = 20;
    config.t1_hard_cap_tokens = 32;
    config.enable_attribution = false;
    let distiller = DeterministicDistiller::new(config.clone());

    let first = distiller
        .distill_conversation(&store, "conv-2")
        .expect("first distill");
    assert_eq!(first.t1_batches_planned, 3);
    assert!(first.chunked_t1);

    let first_artifacts = store
        .artifacts_for_conversation("conv-2")
        .expect("first artifacts")
        .into_iter()
        .filter(|artifact| artifact.kind == "t1")
        .collect::<Vec<_>>();

    let second = distiller
        .distill_conversation(&store, "conv-2")
        .expect("second distill");
    assert_eq!(second.t1_batches_planned, 3);

    let second_artifacts = store
        .artifacts_for_conversation("conv-2")
        .expect("second artifacts")
        .into_iter()
        .filter(|artifact| artifact.kind == "t1")
        .collect::<Vec<_>>();

    assert_eq!(first_artifacts.len(), 3);
    assert_eq!(first_artifacts, second_artifacts);

    let conv2_sources = store
        .t0_events_for_conversation("conv-2")
        .expect("conv2 t0")
        .into_iter()
        .map(|event| event.compact_id)
        .collect::<std::collections::BTreeSet<_>>();

    for artifact in first_artifacts {
        for trace_id in artifact.trace_ids {
            assert!(conv2_sources.contains(&trace_id));
        }
    }
}

#[test]
fn planner_rejects_cross_conversation_mixing() {
    let events = vec![
        StoredCompactEvent {
            compact_id: "t0:a".to_string(),
            conversation_id: "conv-a".to_string(),
            ts: ts(13, 0, 0),
            role: Some(ConversationRole::User),
            text: Some("alpha".to_string()),
            tool_meta: None,
            source_event_ids: vec!["e1".to_string()],
            policy_version: "t0.v1".to_string(),
        },
        StoredCompactEvent {
            compact_id: "t0:b".to_string(),
            conversation_id: "conv-b".to_string(),
            ts: ts(13, 0, 1),
            role: Some(ConversationRole::User),
            text: Some("beta".to_string()),
            tool_meta: None,
            source_event_ids: vec!["e2".to_string()],
            policy_version: "t0.v1".to_string(),
        },
    ];

    let err = plan_t1_batches(&events, 4, 32).expect_err("must fail");
    assert!(matches!(
        err,
        DistillationError::Contract(MindContractError::T1CrossConversation { .. })
    ));
}

#[test]
fn emits_t2_reflection_when_t1_block_exceeds_threshold() {
    let store = MindStore::open_in_memory().expect("open");
    store
        .append_context_state(&ConversationContextState {
            conversation_id: "conv-3".to_string(),
            ts: ts(14, 0, 0),
            active_tag: Some("mind".to_string()),
            active_tasks: vec!["107".to_string()],
            lifecycle: Some("in-progress".to_string()),
            signal_task_ids: vec!["107".to_string()],
            signal_source: "task_lifecycle_command".to_string(),
        })
        .expect("context");

    insert_t0(
        &store,
        "e1",
        "conv-3",
        ts(14, 0, 1),
        "observation runtime deterministic output keeps trace ids stable",
    );
    insert_t0(
        &store,
        "e2",
        "conv-3",
        ts(14, 0, 2),
        "reflection threshold should trigger for grouped observations by tag",
    );
    insert_t0(
        &store,
        "e3",
        "conv-3",
        ts(14, 0, 3),
        "chunk ordering remains deterministic when running again",
    );

    let mut config = DistillationConfig::default();
    config.t1_target_tokens = 10;
    config.t2_trigger_tokens = 10;
    config.enable_attribution = false;
    let distiller = DeterministicDistiller::new(config.clone());

    let report = distiller
        .distill_conversation(&store, "conv-3")
        .expect("distill");
    assert!(report.t1_artifacts_written >= 2);
    assert!(report.t2_artifacts_written >= 1);

    let artifacts = store
        .artifacts_for_conversation("conv-3")
        .expect("artifacts");
    let reflections = artifacts
        .iter()
        .filter(|artifact| artifact.kind == "t2")
        .collect::<Vec<_>>();
    assert!(!reflections.is_empty());

    for reflection in reflections {
        assert!(reflection.text.chars().count() <= config.t2_output_max_chars);
        for trace_id in &reflection.trace_ids {
            assert!(trace_id.starts_with("obs:"));
        }
        let provenance = store
            .semantic_provenance_for_artifact(&reflection.artifact_id)
            .expect("provenance");
        assert!(!provenance.is_empty());
        assert_eq!(provenance[0].stage, SemanticStage::T2Reflector);
        assert_eq!(provenance[0].runtime, SemanticRuntime::Deterministic);
    }
}

#[test]
fn oversized_single_event_respects_hard_cap() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
            &store,
            "e1",
            "conv-4",
            ts(15, 0, 0),
            "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
        );

    let mut config = DistillationConfig::default();
    config.t1_target_tokens = 28;
    config.t1_hard_cap_tokens = 32;
    config.enable_attribution = false;

    let distiller = DeterministicDistiller::new(config);
    let err = distiller
        .distill_conversation(&store, "conv-4")
        .expect_err("hard cap must fail");

    assert!(matches!(
        err,
        DistillationError::Contract(MindContractError::T1OverHardCap { .. })
    ));
}

#[test]
fn session_sidecar_runs_semantic_t1_after_debounce() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-sem",
        ts(16, 0, 0),
        "build semantic observer queue and debounce behavior",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let adapter = StaticObserverAdapter {
        result: Ok(ObserverOutput {
            summary: "semantic observer summary".to_string(),
            key_points: vec!["point a".to_string(), "point b".to_string()],
            citations: vec!["t0:e1".to_string()],
        }),
    };
    let mut sidecar =
        SessionObserverSidecar::new(distill_config, SemanticObserverConfig::default(), adapter);

    let now = ts(16, 5, 0);
    sidecar.enqueue_turn("session-1", "conv-sem", now);
    assert!(sidecar.run_ready(&store, now).is_empty());

    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    assert_eq!(
        outcomes[0].trigger.kind,
        ObserverTriggerKind::TokenThreshold
    );
    let report = outcomes[0].report.as_ref().expect("distillation report");
    assert_eq!(report.t1_artifacts_written, 1);

    let artifacts = store
        .artifacts_for_conversation("conv-sem")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].text.contains("semantic observer summary"));

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 1);
    assert_eq!(provenance[0].runtime, SemanticRuntime::PiSemantic);
    assert_eq!(provenance[0].stage, SemanticStage::T1Observer);
}

#[test]
fn semantic_failure_falls_back_to_deterministic_t1() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-fallback",
        ts(16, 10, 0),
        "semantic provider failure should not block artifact creation",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let adapter = StaticObserverAdapter {
        result: Err(SemanticAdapterError::new(
            SemanticFailureKind::Timeout,
            "observer timed out",
        )),
    };
    let mut sidecar =
        SessionObserverSidecar::new(distill_config, SemanticObserverConfig::default(), adapter);

    let now = ts(16, 12, 0);
    sidecar.enqueue_turn("session-2", "conv-fallback", now);
    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    assert_eq!(
        outcomes[0].trigger.kind,
        ObserverTriggerKind::TokenThreshold
    );
    outcomes[0].report.as_ref().expect("report");

    let artifacts = store
        .artifacts_for_conversation("conv-fallback")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].text.starts_with("T1 observation"));

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 2);
    assert_eq!(provenance[0].runtime, SemanticRuntime::PiSemantic);
    assert_eq!(
        provenance[0].failure_kind,
        Some(SemanticFailureKind::Timeout)
    );
    assert_eq!(provenance[0].attempt_count, 2);
    assert!(provenance[0].fallback_used);
    assert_eq!(provenance[1].runtime, SemanticRuntime::Deterministic);
    assert_eq!(provenance[1].attempt_count, 3);
    assert!(provenance[1].fallback_used);
}

#[test]
fn semantic_observer_retries_and_persists_attempt_count_on_success() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-retry",
        ts(16, 14, 0),
        "retry semantic observer on provider hiccup",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let mut semantic_config = SemanticObserverConfig::default();
    semantic_config.guardrails.max_retries = 1;

    let adapter = SequenceObserverAdapter {
        scripted_results: RefCell::new(vec![
            Err(SemanticAdapterError::new(
                SemanticFailureKind::ProviderError,
                "temporary provider outage",
            )),
            Ok(ObserverOutput {
                summary: "retry succeeded".to_string(),
                key_points: vec!["attempt two".to_string()],
                citations: vec![],
            }),
        ]),
        delay_ms: 0,
    };
    let mut sidecar = SessionObserverSidecar::new(distill_config, semantic_config, adapter);

    let now = ts(16, 14, 30);
    sidecar.enqueue_turn("session-retry", "conv-retry", now);
    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    outcomes[0].report.as_ref().expect("report");

    let artifacts = store
        .artifacts_for_conversation("conv-retry")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].text.contains("retry succeeded"));

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 1);
    assert_eq!(provenance[0].runtime, SemanticRuntime::PiSemantic);
    assert_eq!(provenance[0].attempt_count, 2);
    assert!(!provenance[0].fallback_used);
}

#[test]
fn guardrail_budget_exceeded_falls_back_to_deterministic_t1() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-budget",
        ts(16, 16, 0),
        "this line is intentionally long enough to exceed a tiny budget guardrail",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let mut semantic_config = SemanticObserverConfig::default();
    semantic_config.guardrails.max_budget_tokens = 8;
    semantic_config.guardrails.max_retries = 2;

    let adapter = StaticObserverAdapter {
        result: Ok(ObserverOutput {
            summary: "should never run due to budget preflight".to_string(),
            key_points: vec![],
            citations: vec![],
        }),
    };
    let mut sidecar = SessionObserverSidecar::new(distill_config, semantic_config, adapter);

    let now = ts(16, 16, 30);
    sidecar.enqueue_turn("session-budget", "conv-budget", now);
    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    outcomes[0].report.as_ref().expect("report");

    let artifacts = store
        .artifacts_for_conversation("conv-budget")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].text.starts_with("T1 observation"));

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 2);
    assert_eq!(
        provenance[0].failure_kind,
        Some(SemanticFailureKind::BudgetExceeded)
    );
    assert_eq!(provenance[0].attempt_count, 1);
    assert_eq!(provenance[1].runtime, SemanticRuntime::Deterministic);
    assert_eq!(provenance[1].attempt_count, 2);
}

#[test]
fn guardrail_cost_budget_exceeded_falls_back_to_deterministic_t1() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-cost",
        ts(16, 17, 0),
        "cost guardrail should reject expensive projected observer call",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let mut semantic_config = SemanticObserverConfig::default();
    semantic_config.guardrails.max_budget_tokens = 10_000;
    semantic_config.guardrails.max_budget_cost_micros = 100;

    let adapter = StaticObserverAdapter {
        result: Ok(ObserverOutput {
            summary: "should not execute".to_string(),
            key_points: vec![],
            citations: vec![],
        }),
    };
    let mut sidecar = SessionObserverSidecar::new(distill_config, semantic_config, adapter);

    let now = ts(16, 17, 30);
    sidecar.enqueue_turn("session-cost", "conv-cost", now);
    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    outcomes[0].report.as_ref().expect("report");

    let artifacts = store
        .artifacts_for_conversation("conv-cost")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].text.starts_with("T1 observation"));

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 2);
    assert_eq!(
        provenance[0].failure_kind,
        Some(SemanticFailureKind::BudgetExceeded)
    );
    assert_eq!(provenance[1].runtime, SemanticRuntime::Deterministic);
}

#[test]
fn guardrail_timeout_converts_slow_success_to_fallback() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-timeout",
        ts(16, 18, 0),
        "slow semantic response should be treated as timeout",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let mut semantic_config = SemanticObserverConfig::default();
    semantic_config.guardrails.timeout_ms = 1;
    semantic_config.guardrails.max_retries = 0;

    let adapter = SequenceObserverAdapter {
        scripted_results: RefCell::new(vec![Ok(ObserverOutput {
            summary: "too slow".to_string(),
            key_points: vec![],
            citations: vec![],
        })]),
        delay_ms: 20,
    };

    let mut sidecar = SessionObserverSidecar::new(distill_config, semantic_config, adapter);

    let now = ts(16, 18, 30);
    sidecar.enqueue_turn("session-timeout", "conv-timeout", now);
    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    outcomes[0].report.as_ref().expect("report");

    let artifacts = store
        .artifacts_for_conversation("conv-timeout")
        .expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].text.starts_with("T1 observation"));

    let provenance = store
        .semantic_provenance_for_artifact(&artifacts[0].artifact_id)
        .expect("provenance");
    assert_eq!(provenance.len(), 2);
    assert_eq!(provenance[0].runtime, SemanticRuntime::PiSemantic);
    assert_eq!(
        provenance[0].failure_kind,
        Some(SemanticFailureKind::Timeout)
    );
    assert!(provenance[0].fallback_used);
    assert_eq!(provenance[1].runtime, SemanticRuntime::Deterministic);
}

#[test]
fn manual_trigger_runs_immediately_and_is_reported() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-manual",
        ts(16, 20, 0),
        "manual shortcut should run observer immediately",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let adapter = StaticObserverAdapter {
        result: Ok(ObserverOutput {
            summary: "manual semantic run".to_string(),
            key_points: vec!["fast path".to_string()],
            citations: vec![],
        }),
    };
    let mut sidecar =
        SessionObserverSidecar::new(distill_config, SemanticObserverConfig::default(), adapter);

    let now = ts(16, 21, 0);
    sidecar.enqueue_manual("session-3", "conv-manual", now);

    let outcomes = sidecar.run_ready(&store, now);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(
        outcomes[0].trigger.kind,
        ObserverTriggerKind::ManualShortcut
    );
    outcomes[0].report.as_ref().expect("report");
}

#[test]
fn task_completed_trigger_upgrades_pending_turn_trigger() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-task",
        ts(16, 30, 0),
        "task completion trigger should be visible in outcomes",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let adapter = StaticObserverAdapter {
        result: Ok(ObserverOutput {
            summary: "task complete semantic run".to_string(),
            key_points: vec![],
            citations: vec![],
        }),
    };
    let mut sidecar =
        SessionObserverSidecar::new(distill_config, SemanticObserverConfig::default(), adapter);

    let now = ts(16, 31, 0);
    sidecar.enqueue_turn("session-4", "conv-task", now);
    sidecar.enqueue_task_completed(
        "session-4",
        "conv-task",
        now + chrono::Duration::milliseconds(20),
    );

    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].trigger.kind, ObserverTriggerKind::TaskCompleted);
    outcomes[0].report.as_ref().expect("report");
}

#[test]
fn session_sidecar_backfills_branch_conversations_within_same_session_tree() {
    let store = MindStore::open_in_memory().expect("open");

    let mut root = raw_message(
        "e-root",
        "conv-root",
        ts(16, 35, 0),
        "root conversation needs observer processing",
    );
    root.agent_id = "session-tree::12".to_string();
    store.insert_raw_event(&root).expect("insert root raw");
    let root_compact = compact_raw_event_to_t0(&root, &T0CompactionPolicy::default())
        .expect("compact root")
        .expect("root kept");
    store
        .upsert_t0_compact_event(&root_compact)
        .expect("insert root t0");

    let mut branch = raw_message(
        "e-branch",
        "conv-branch",
        ts(16, 35, 1),
        "branch conversation should be backfilled in same session",
    );
    branch.agent_id = "session-tree::12".to_string();
    branch.attrs = canonical_lineage_attrs(&ConversationLineageMetadata {
        session_id: "session-tree".to_string(),
        parent_conversation_id: Some("conv-root".to_string()),
        root_conversation_id: "conv-root".to_string(),
    });
    store.insert_raw_event(&branch).expect("insert branch raw");
    let branch_compact = compact_raw_event_to_t0(&branch, &T0CompactionPolicy::default())
        .expect("compact branch")
        .expect("branch kept");
    store
        .upsert_t0_compact_event(&branch_compact)
        .expect("insert branch t0");

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;

    let adapter = StaticObserverAdapter {
        result: Ok(ObserverOutput {
            summary: "semantic observer summary".to_string(),
            key_points: vec!["point".to_string()],
            citations: vec![],
        }),
    };
    let mut sidecar =
        SessionObserverSidecar::new(distill_config, SemanticObserverConfig::default(), adapter);

    let now = ts(16, 36, 0);
    sidecar.enqueue_turn("session-tree", "conv-root", now);
    let outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(300));

    assert_eq!(outcomes.len(), 2);
    let conversations = outcomes
        .iter()
        .map(|outcome| outcome.conversation_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        conversations,
        std::collections::BTreeSet::from(["conv-branch", "conv-root"])
    );

    let root_artifacts = store
        .artifacts_for_conversation("conv-root")
        .expect("root artifacts")
        .into_iter()
        .filter(|artifact| artifact.kind == "t1")
        .count();
    let branch_artifacts = store
        .artifacts_for_conversation("conv-branch")
        .expect("branch artifacts")
        .into_iter()
        .filter(|artifact| artifact.kind == "t1")
        .count();
    assert_eq!(root_artifacts, 1);
    assert_eq!(branch_artifacts, 1);
}

#[test]
fn observer_feed_event_maps_trigger_and_fallback_metadata() {
    let store = MindStore::open_in_memory().expect("open");
    insert_t0(
        &store,
        "e1",
        "conv-feed",
        ts(16, 40, 0),
        "observer fallback metadata should be visible in feed",
    );

    let mut distill_config = DistillationConfig::default();
    distill_config.enable_attribution = false;
    distill_config.t2_trigger_tokens = 9_999;
    let expected_target_tokens = distill_config.t1_target_tokens;
    let expected_hard_cap_tokens = distill_config.t1_hard_cap_tokens;

    let mut semantic_config = SemanticObserverConfig::default();
    semantic_config.guardrails.timeout_ms = 1;
    semantic_config.guardrails.max_retries = 0;

    let adapter = SequenceObserverAdapter {
        scripted_results: RefCell::new(vec![Ok(ObserverOutput {
            summary: "slow semantic output".to_string(),
            key_points: vec![],
            citations: vec![],
        })]),
        delay_ms: 20,
    };

    let mut sidecar = SessionObserverSidecar::new(distill_config, semantic_config, adapter);
    let now = ts(16, 41, 0);
    sidecar.enqueue_task_completed("session-5", "conv-feed", now);

    let mut outcomes = sidecar.run_ready(&store, now + chrono::Duration::milliseconds(250));
    assert_eq!(outcomes.len(), 1);

    let event = observer_feed_event_from_outcome(
        &store,
        &outcomes.remove(0),
        now + chrono::Duration::milliseconds(260),
    );
    assert_eq!(event.trigger, MindObserverFeedTriggerKind::TaskCompleted);
    assert_eq!(event.status, MindObserverFeedStatus::Fallback);
    assert_eq!(event.runtime.as_deref(), Some("deterministic"));
    assert_eq!(event.attempt_count, Some(2));
    assert_eq!(event.failure_kind.as_deref(), Some("timeout"));

    let progress = event.progress.expect("mind progress");
    assert_eq!(progress.t1_target_tokens, expected_target_tokens);
    assert_eq!(progress.t1_hard_cap_tokens, expected_hard_cap_tokens);
    let t0_events = store
        .t0_events_for_conversation("conv-feed")
        .expect("conv-feed events");
    let expected_t0_tokens = t0_events.iter().fold(0_u32, |total, event| {
        total.saturating_add(estimate_t0_event_tokens(event))
    });
    assert_eq!(progress.t0_estimated_tokens, expected_t0_tokens);
    assert_eq!(
        progress.tokens_until_next_run,
        expected_target_tokens.saturating_sub(expected_t0_tokens)
    );
}

#[test]
fn process_reflector_job_materializes_t2_reflection() {
    let store = MindStore::open_in_memory().expect("open");
    store
        .insert_observation(
            "obs-r1",
            "conv-reflector",
            ts(16, 55, 0),
            "first observation for reflector",
            &[],
        )
        .expect("observation one");
    store
        .insert_observation(
            "obs-r2",
            "conv-reflector",
            ts(16, 55, 1),
            "second observation for reflector",
            &[],
        )
        .expect("observation two");

    let job = ReflectorJob {
        job_id: "rj-1".to_string(),
        active_tag: "mind".to_string(),
        observation_ids: vec!["obs-r1".to_string(), "obs-r2".to_string()],
        conversation_ids: vec!["conv-reflector".to_string()],
        estimated_tokens: 120,
        status: ReflectorJobStatus::Pending,
        claimed_by: None,
        claimed_at: None,
        attempts: 0,
        last_error: None,
        created_at: ts(16, 55, 2),
        updated_at: ts(16, 55, 2),
    };

    process_reflector_job(&store, &job, ts(16, 55, 3)).expect("process reflector job");

    let artifacts = store
        .artifacts_for_conversation("conv-reflector")
        .expect("artifacts");
    let reflection = artifacts
        .iter()
        .find(|artifact| artifact.kind == "t2")
        .expect("t2 reflection written");
    assert!(reflection
        .text
        .contains("T2 runtime reflection for tag=mind observations=2"));
    assert!(reflection.trace_ids.contains(&"obs-r1".to_string()));
    assert!(reflection.trace_ids.contains(&"obs-r2".to_string()));
}

#[test]
fn process_t3_backlog_job_updates_canon_and_watermark() {
    let store = MindStore::open_in_memory().expect("open");
    store
        .insert_observation(
            "obs-t3",
            "conv-t3",
            ts(16, 58, 0),
            "observation for canon",
            &[],
        )
        .expect("t1 observation");
    store
        .insert_reflection(
            "ref-t3",
            "conv-t3",
            ts(16, 58, 1),
            "reflection for canon",
            &["obs-t3".to_string()],
        )
        .expect("t2 reflection");

    let job = T3BacklogJob {
        job_id: "t3j-1".to_string(),
        project_root: "/repo".to_string(),
        session_id: "session-t3".to_string(),
        pane_id: "12".to_string(),
        active_tag: Some("mind".to_string()),
        slice_start_id: Some("obs-t3".to_string()),
        slice_end_id: Some("ref-t3".to_string()),
        artifact_refs: vec!["obs-t3".to_string(), "ref-t3".to_string()],
        status: T3BacklogJobStatus::Pending,
        attempts: 0,
        last_error: None,
        claimed_by: None,
        claimed_at: None,
        created_at: ts(16, 58, 2),
        updated_at: ts(16, 58, 2),
    };

    process_t3_backlog_job(
        &store,
        &job,
        ts(16, 58, 3),
        |_store, _project_root, _active_tag, _now| Ok(()),
    )
    .expect("process t3 backlog job");

    let active = store
        .active_canon_entries(Some("mind"))
        .expect("active canon");
    assert_eq!(active.len(), 2);
    assert!(active
        .iter()
        .any(|entry| entry.evidence_refs.contains(&"obs-t3".to_string())));
    assert!(active
        .iter()
        .any(|entry| entry.evidence_refs.contains(&"ref-t3".to_string())));

    let watermark = store
        .project_watermark("project:/repo")
        .expect("watermark")
        .expect("watermark row");
    assert_eq!(watermark.last_artifact_id.as_deref(), Some("ref-t3"));
    assert_eq!(watermark.last_artifact_ts, Some(ts(16, 58, 1)));
}

#[test]
fn build_handshake_export_prefers_active_tag_and_respects_budget() {
    let store = MindStore::open_in_memory().expect("open");
    let now = ts(17, 4, 0);

    store
        .upsert_canon_entry_revision(
            "canon-mind",
            Some("mind"),
            "mind focus with remaining follow-up work",
            8200,
            7600,
            None,
            &["obs-mind".to_string()],
            now,
        )
        .expect("mind canon");
    store
        .upsert_canon_entry_revision(
            "canon-other",
            Some("other"),
            "other topic stable summary",
            7000,
            6500,
            None,
            &["obs-other".to_string()],
            now - chrono::Duration::minutes(1),
        )
        .expect("other canon");

    let snapshot = HandshakeProjectSnapshot {
        workstreams: vec![HandshakeWorkstreamSummary {
            tag: "mind".to_string(),
            counts: HandshakeTaskCounts {
                total: 2,
                pending: 1,
                in_progress: 1,
                blocked: 0,
                done: 0,
            },
            prd_backed_open: 1,
        }],
        priority_tasks: vec![HandshakeTaskSummary {
            id: "190".to_string(),
            tag: "mind".to_string(),
            title: "Standalone Mind export move".to_string(),
            status: "in-progress".to_string(),
            priority: "high".to_string(),
            prd_source: Some("task-prd"),
            active_agent: true,
        }],
    };

    let bundle = build_handshake_export(&store, Some(&snapshot), Some("mind"), now)
        .expect("handshake export");

    assert!(bundle.payload.contains("active_tag: mind"));
    assert!(bundle.payload.contains("tag mind :: [canon-mind r1]"));
    assert!(bundle.token_estimate <= MIND_T3_HANDSHAKE_TOKEN_BUDGET);
    assert!(!bundle.payload_hash.is_empty());
}

#[test]
fn canonical_mind_command_name_normalizes_legacy_aliases() {
    assert_eq!(
        canonical_mind_command_name("mind_ingest_event"),
        Some("mind_ingest_event")
    );
    assert_eq!(
        canonical_mind_command_name("insight_ingest"),
        Some("mind_ingest_event")
    );
    assert_eq!(
        canonical_mind_command_name("insight_handoff"),
        Some("mind_handoff")
    );
    assert_eq!(
        canonical_mind_command_name("insight_resume"),
        Some("mind_resume")
    );
    assert_eq!(
        canonical_mind_command_name("mind_finalize"),
        Some("mind_finalize_session")
    );
    assert_eq!(canonical_mind_command_name("unknown_command"), None);
}

#[test]
fn prepare_handoff_resume_command_authors_queue_and_injection_policy() {
    let handoff = prepare_handoff_resume_command("mind_handoff", None);
    assert_eq!(handoff.result.status, "ok");
    assert_eq!(
        handoff.result.reason,
        "handoff/resume observer trigger queued"
    );
    assert_eq!(
        handoff.followup.queue_reason.as_deref(),
        Some("stm handoff")
    );
    assert_eq!(
        handoff.followup.injection_trigger,
        Some(MindInjectionTriggerKind::Handoff)
    );
    assert_eq!(
        handoff.followup.injection_reason.as_deref(),
        Some("stm handoff")
    );

    let resume = prepare_handoff_resume_command("mind_resume", Some("resume latest handoff"));
    assert_eq!(
        resume.followup.injection_trigger,
        Some(MindInjectionTriggerKind::Resume)
    );
    assert_eq!(
        resume.followup.queue_reason.as_deref(),
        Some("resume latest handoff")
    );
}

#[test]
fn mind_command_policy_helpers_shape_manual_shortcut_results() {
    let success = prepare_compaction_rebuild_success("checkpoint-1", "operator request");
    assert_eq!(success.result.status, "ok");
    assert_eq!(
        success.followup.queue_reason.as_deref(),
        Some("compaction rebuild requested (operator request): checkpoint-1")
    );
    assert_eq!(
        success
            .result
            .observer_event
            .as_ref()
            .map(|event| &event.reason),
        Some(&"compaction slice rebuilt: checkpoint-1".to_string())
    );

    let missing = mind_compaction_rebuild_checkpoint_missing();
    assert_eq!(
        missing.error_code,
        Some("mind_compaction_checkpoint_missing")
    );
    assert_eq!(
        missing.observer_event.as_ref().map(|event| &event.reason),
        Some(&"compaction rebuild failed: no compaction checkpoint found".to_string())
    );

    let t3 = mind_t3_requeue_success("job-1", true, "operator requeue request");
    assert_eq!(t3.reason, "t3 requeue job-1 (inserted)");
    assert_eq!(
        t3.observer_event.as_ref().map(|event| event.status),
        Some(MindObserverFeedStatus::Queued)
    );

    let handshake = prepare_handshake_rebuild_success();
    assert_eq!(handshake.result.reason, "handshake baseline rebuilt");
    assert_eq!(
        handshake.followup.injection_trigger,
        Some(MindInjectionTriggerKind::Startup)
    );
    assert_eq!(
        handshake.followup.injection_reason.as_deref(),
        Some("handshake rebuild")
    );
}

#[test]
fn execute_manual_compaction_rebuild_reports_missing_checkpoint() {
    let test_root = std::env::temp_dir().join(format!(
        "aoc-mind-manual-compaction-test-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&test_root).expect("create test root");
    let store = MindStore::open(test_root.join("project.sqlite")).expect("open store");

    match execute_manual_compaction_rebuild(&store, "session-missing", "operator request") {
        ManualCompactionRebuildOutcome::Complete { result } => {
            assert_eq!(
                result.error_code,
                Some("mind_compaction_checkpoint_missing")
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(test_root);
}

#[test]
fn execute_manual_t3_requeue_from_manifest_reports_missing_export() {
    let project_root = std::env::temp_dir().join(format!(
        "aoc-mind-manual-t3-requeue-test-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&project_root).expect("create test root");
    let store = MindStore::open(project_root.join("project.sqlite")).expect("open store");

    let outcome = execute_manual_t3_requeue_from_manifest(
        &store,
        project_root.to_str().expect("utf8 path"),
        "operator request",
    );
    assert_eq!(outcome.result.status, "error");
    assert_eq!(outcome.result.error_code, Some("mind_t3_requeue_failed"));
    assert!(outcome.pending_jobs.is_none());
}

#[test]
fn evaluate_idle_finalize_requires_timeout_and_respects_throttle() {
    let now = Utc::now();

    assert_eq!(
        evaluate_idle_finalize(None, None, now, 60_000, 5_000),
        IdleFinalizeDecision::NoLastIngest
    );
    assert_eq!(
        evaluate_idle_finalize(Some(now), None, now, 0, 5_000),
        IdleFinalizeDecision::Disabled
    );
    assert_eq!(
        evaluate_idle_finalize(
            Some(now - chrono::Duration::milliseconds(10_000)),
            None,
            now,
            60_000,
            5_000,
        ),
        IdleFinalizeDecision::WaitingForIdleTimeout
    );
    assert_eq!(
        evaluate_idle_finalize(
            Some(now - chrono::Duration::milliseconds(60_000)),
            Some(now - chrono::Duration::milliseconds(1_000)),
            now,
            60_000,
            5_000,
        ),
        IdleFinalizeDecision::Throttled
    );
    assert_eq!(
        evaluate_idle_finalize(
            Some(now - chrono::Duration::milliseconds(60_000)),
            Some(now - chrono::Duration::milliseconds(6_000)),
            now,
            60_000,
            5_000,
        ),
        IdleFinalizeDecision::Finalize {
            reason: "idle timeout finalize"
        }
    );
}

#[test]
fn evaluate_finalize_drain_reports_settled_and_timeout() {
    let now = Utc::now();
    assert_eq!(
        evaluate_finalize_drain(true, 0, now, now + chrono::Duration::seconds(1)),
        FinalizeDrainDecision::Settled
    );
    assert_eq!(
        evaluate_finalize_drain(false, 1, now, now),
        FinalizeDrainDecision::TimedOut {
            observer_reason: "finalize drain timeout reached; exporting current slice"
        }
    );
    assert_eq!(
        evaluate_finalize_drain(false, 1, now, now + chrono::Duration::seconds(1)),
        FinalizeDrainDecision::Continue
    );
}

#[test]
fn session_finalize_messages_format_error_and_success() {
    let err = session_finalize_error("planning", "boom");
    assert_eq!(err.status, "error");
    assert_eq!(err.reason, "finalize failed: planning error: boom");
    assert_eq!(err.observer_reason, "finalize planning failed: boom");

    let manifest = SessionExportManifest {
        schema_version: 1,
        session_id: "s1".to_string(),
        pane_id: "p1".to_string(),
        project_root: "/tmp/project".to_string(),
        active_tag: Some("core".to_string()),
        conversation_ids: vec!["c1".to_string()],
        export_dir: "/tmp/project/.aoc/mind/s1".to_string(),
        t1_count: 2,
        t2_count: 1,
        t1_artifact_ids: vec!["a1".to_string()],
        t2_artifact_ids: vec!["a2".to_string()],
        slice_start_id: "a1".to_string(),
        slice_end_id: "a2".to_string(),
        slice_hash: "deadbeef".to_string(),
        exported_at: Utc::now().to_rfc3339(),
        last_artifact_ts: Utc::now().to_rfc3339(),
        watermark_scope: "session:s1:pane:p1".to_string(),
        t3_job_id: "job-1".to_string(),
        t3_job_inserted: true,
    };
    let ok = session_finalize_success("idle timeout finalize", &manifest.export_dir, &manifest);
    assert_eq!(ok.status, "ok");
    assert_eq!(
        ok.reason,
        "idle timeout finalize: session export finalized at /tmp/project/.aoc/mind/s1"
    );
    assert_eq!(
        ok.observer_reason,
        "idle timeout finalize: session export finalized: t1=2 t2=1 t3_job_inserted=true"
    );
}

#[test]
fn prepare_session_finalize_plan_skips_when_no_new_artifacts() {
    let store = MindStore::open_in_memory().expect("open");
    let outcome = prepare_session_finalize_plan(
        &store,
        "session-empty",
        "12",
        None,
        "session:session-empty:pane:12",
    )
    .expect("finalize plan outcome");

    match outcome {
        SessionFinalizePlanOutcome::Skip {
            observer_reason,
            outcome_reason_suffix,
        } => {
            assert_eq!(observer_reason, "finalize skipped: no new artifacts");
            assert_eq!(outcome_reason_suffix, "no new finalized artifacts");
        }
        SessionFinalizePlanOutcome::Ready(_) => panic!("expected skip outcome"),
    }
}

#[test]
fn prepare_session_finalize_export_location_sanitizes_session_and_builds_path() {
    let now = Utc::now();
    let plan = FinalizeArtifactPlan {
        conversation_ids: vec!["c1".to_string()],
        active_tag: Some("core".to_string()),
        delta_artifacts: vec![StoredArtifact {
            artifact_id: "a2".to_string(),
            conversation_id: "c1".to_string(),
            ts: now,
            text: "artifact content".to_string(),
            kind: "t2.reflector".to_string(),
            trace_ids: vec![],
        }],
        t1_artifacts: vec![],
        t2_artifacts: vec![],
        slice_start_id: "a2".to_string(),
        slice_end_id: "a2".to_string(),
        slice_hash: "deadbeefcafebabe".to_string(),
        artifact_ids: vec!["a2".to_string()],
        last_artifact_ts: now,
        watermark_scope: "session:s1:pane:p1".to_string(),
    };

    let location =
        prepare_session_finalize_export_location("/tmp/project", "session / weird: id", &plan);
    assert!(location.export_dir_name.starts_with("session_weird_id_"));
    assert!(location.export_dir_name.ends_with("_deadbeefcafe"));
    assert!(location
        .export_dir
        .ends_with(&format!("/.aoc/mind/insight/{}", location.export_dir_name)));
}

#[test]
fn prepare_session_finalize_host_plan_builds_files_and_watermark() {
    let now = Utc::now();
    let artifact = StoredArtifact {
        artifact_id: "a2".to_string(),
        conversation_id: "c1".to_string(),
        ts: now,
        text: "artifact content".to_string(),
        kind: "t2.reflector".to_string(),
        trace_ids: vec![],
    };
    let plan = FinalizeArtifactPlan {
        conversation_ids: vec!["c1".to_string()],
        active_tag: Some("core".to_string()),
        delta_artifacts: vec![artifact.clone()],
        t1_artifacts: vec![],
        t2_artifacts: vec![artifact],
        slice_start_id: "a2".to_string(),
        slice_end_id: "a2".to_string(),
        slice_hash: "deadbeefcafebabe".to_string(),
        artifact_ids: vec!["a2".to_string()],
        last_artifact_ts: now,
        watermark_scope: "session:s1:pane:p1".to_string(),
    };

    let host = prepare_session_finalize_host_plan(
        &plan,
        "s1",
        "p1",
        "/tmp/project",
        "/tmp/project/.aoc/mind/insight/export1",
        "job-1",
        true,
        "manual finalize",
    )
    .expect("host plan");

    assert_eq!(host.export_files.len(), 3);
    assert_eq!(host.export_files[0].file_name, "t1.md");
    assert_eq!(host.export_files[1].file_name, "t2.md");
    assert_eq!(host.export_files[2].file_name, "manifest.json");
    assert_eq!(host.watermark_artifact_id, "a2");
    assert_eq!(host.watermark_ts, now);
    assert_eq!(host.success.status, "ok");
    assert!(host
        .success
        .reason
        .contains("manual finalize: session export finalized at"));
    assert_eq!(host.manifest.t3_job_id, "job-1");
}

#[test]
fn session_export_bundle_renders_markdown_and_manifest() {
    let plan = FinalizeArtifactPlan {
        conversation_ids: vec!["conv-export".to_string()],
        active_tag: Some("mind".to_string()),
        delta_artifacts: vec![],
        t1_artifacts: vec![StoredArtifact {
            artifact_id: "obs-export".to_string(),
            conversation_id: "conv-export".to_string(),
            ts: ts(17, 5, 0),
            text: "exported observation".to_string(),
            kind: "t1".to_string(),
            trace_ids: vec![],
        }],
        t2_artifacts: vec![StoredArtifact {
            artifact_id: "ref-export".to_string(),
            conversation_id: "conv-export".to_string(),
            ts: ts(17, 5, 1),
            text: "exported reflection".to_string(),
            kind: "t2".to_string(),
            trace_ids: vec!["obs-export".to_string()],
        }],
        slice_start_id: "obs-export".to_string(),
        slice_end_id: "ref-export".to_string(),
        slice_hash: "slicehash123".to_string(),
        artifact_ids: vec!["obs-export".to_string(), "ref-export".to_string()],
        last_artifact_ts: ts(17, 5, 1),
        watermark_scope: "session:session-export:pane:12".to_string(),
    };

    let bundle = build_session_export_bundle(
        &plan,
        "session-export",
        "12",
        "/repo",
        "/repo/.aoc/mind/insight/export-dir",
        "t3j-export",
        true,
    )
    .expect("bundle");

    assert!(bundle.t1_markdown.contains("# T1 export"));
    assert!(bundle.t2_markdown.contains("# T2 export"));
    assert!(bundle.manifest_json.contains("session-export"));
    assert_eq!(bundle.manifest.t1_count, 1);
    assert_eq!(bundle.manifest.t2_count, 1);
    assert_eq!(bundle.manifest.t3_job_id, "t3j-export");
}

#[test]
fn finalize_planner_selects_new_t1_t2_artifacts_after_watermark() {
    let store = MindStore::open_in_memory().expect("open");
    let mut attrs = canonical_lineage_attrs(&ConversationLineageMetadata {
        session_id: "session-finalize".to_string(),
        parent_conversation_id: None,
        root_conversation_id: "conv-finalize".to_string(),
    });
    attrs.insert(
        "project_root".to_string(),
        serde_json::Value::String("/repo".to_string()),
    );
    store
        .insert_raw_event(&RawEvent {
            event_id: "evt-finalize-1".to_string(),
            conversation_id: "conv-finalize".to_string(),
            agent_id: "agent-1".to_string(),
            ts: ts(17, 0, 0),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::User,
                text: "finalize this session".to_string(),
            }),
            attrs,
        })
        .expect("raw event");
    store
        .append_context_state(&ConversationContextState {
            conversation_id: "conv-finalize".to_string(),
            ts: ts(17, 0, 1),
            active_tag: Some("mind".to_string()),
            active_tasks: vec![],
            lifecycle: Some("in-progress".to_string()),
            signal_task_ids: vec![],
            signal_source: "test".to_string(),
        })
        .expect("context state");
    store
        .insert_observation(
            "obs-old",
            "conv-finalize",
            ts(17, 0, 2),
            "old observation",
            &[],
        )
        .expect("old observation");
    store
        .advance_project_watermark(
            "session:session-finalize:pane:12",
            Some(ts(17, 0, 2)),
            Some("obs-old"),
            ts(17, 0, 3),
        )
        .expect("watermark");
    store
        .insert_observation(
            "obs-new",
            "conv-finalize",
            ts(17, 0, 4),
            "new observation",
            &[],
        )
        .expect("new observation");
    store
        .insert_reflection(
            "ref-new",
            "conv-finalize",
            ts(17, 0, 5),
            "new reflection",
            &[],
        )
        .expect("new reflection");

    let plan = plan_session_finalize_artifacts(
        &store,
        "session-finalize",
        "12",
        Some("conv-finalize"),
        "session:session-finalize:pane:12",
    )
    .expect("plan");

    let FinalizeArtifactSelection::Ready(plan) = plan else {
        panic!("expected ready finalize plan")
    };
    assert_eq!(plan.active_tag.as_deref(), Some("mind"));
    assert_eq!(plan.t1_artifacts.len(), 1);
    assert_eq!(plan.t2_artifacts.len(), 1);
    assert_eq!(plan.slice_start_id, "obs-new");
    assert_eq!(plan.slice_end_id, "ref-new");
    assert_eq!(plan.artifact_ids, vec!["obs-new", "ref-new"]);
}
