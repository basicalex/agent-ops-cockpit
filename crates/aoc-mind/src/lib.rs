mod observer_runtime;
mod reflector_runtime;

pub use observer_runtime::{
    ClaimedObserverRun, ObserverQueueConfig, ObserverTrigger, ObserverTriggerKind,
    ObserverTriggerPriority, SessionObserverQueue,
};
pub use reflector_runtime::{
    DetachedReflectorWorker, ReflectorRuntimeConfig, ReflectorRuntimeError, ReflectorTickReport,
};

use aoc_core::{
    mind_contracts::{
        build_t2_workstream_batch, canonical_json, canonical_payload_hash, validate_t1_scope,
        ConversationRole, MindContractError, ObservationRef, ObserverAdapter, ObserverInput,
        ObserverOutput, SemanticAdapterError, SemanticFailureKind, SemanticGuardrails,
        SemanticModelProfile, SemanticProvenance, SemanticRuntime, SemanticRuntimeMode,
        SemanticStage, T1Batch, T1_PARSER_HARD_CAP_TOKENS, T1_PARSER_TARGET_TOKENS,
    },
    mind_observer_feed::{
        MindObserverFeedEvent, MindObserverFeedProgress, MindObserverFeedStatus,
        MindObserverFeedTriggerKind,
    },
};
use aoc_storage::{ConversationContextState, MindStore, StorageError, StoredCompactEvent};
use aoc_task_attribution::{AttributionConfig, AttributionError, TaskAttributionEngine};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

const DEFAULT_T1_OUTPUT_MAX_CHARS: usize = 1_200;
const DEFAULT_T2_OUTPUT_MAX_CHARS: usize = 1_400;
const DEFAULT_T2_TRIGGER_TOKENS: u32 = 2_400;
const DEFAULT_PI_OBSERVER_PROVIDER: &str = "pi";
const DEFAULT_PI_OBSERVER_MODEL: &str = "small-background";
const DEFAULT_PI_OBSERVER_PROMPT_VERSION: &str = "pi.observer.v1";
const DEFAULT_PI_REFLECTOR_PROVIDER: &str = "pi";
const DEFAULT_PI_REFLECTOR_MODEL: &str = "small-background";
const DEFAULT_PI_REFLECTOR_PROMPT_VERSION: &str = "pi.reflector.v1";
const DEFAULT_SEMANTIC_COST_MICROS_PER_TOKEN: u64 = 100;

#[derive(Debug, Error)]
pub enum DistillationError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
    #[error("attribution error: {0}")]
    Attribution(#[from] AttributionError),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct DistillationConfig {
    pub t1_target_tokens: u32,
    pub t1_hard_cap_tokens: u32,
    pub t2_trigger_tokens: u32,
    pub t1_output_max_chars: usize,
    pub t2_output_max_chars: usize,
    pub enable_attribution: bool,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            t1_target_tokens: T1_PARSER_TARGET_TOKENS,
            t1_hard_cap_tokens: T1_PARSER_HARD_CAP_TOKENS,
            t2_trigger_tokens: DEFAULT_T2_TRIGGER_TOKENS,
            t1_output_max_chars: DEFAULT_T1_OUTPUT_MAX_CHARS,
            t2_output_max_chars: DEFAULT_T2_OUTPUT_MAX_CHARS,
            enable_attribution: true,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DistillationReport {
    pub t0_events_processed: usize,
    pub t1_batches_planned: usize,
    pub t1_artifacts_written: usize,
    pub t2_artifacts_written: usize,
    pub chunked_t1: bool,
    pub attribution_links_written: usize,
}

#[derive(Debug, Clone)]
struct ProducedObservation {
    artifact_id: String,
    ts: chrono::DateTime<chrono::Utc>,
    active_tag: String,
    text: String,
    estimated_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct SemanticObserverConfig {
    pub mode: SemanticRuntimeMode,
    pub profile: SemanticModelProfile,
    pub guardrails: SemanticGuardrails,
}

impl Default for SemanticObserverConfig {
    fn default() -> Self {
        Self {
            mode: SemanticRuntimeMode::SemanticWithFallback,
            profile: default_pi_observer_profile(),
            guardrails: SemanticGuardrails::default(),
        }
    }
}

pub fn default_pi_observer_profile() -> SemanticModelProfile {
    SemanticModelProfile {
        provider_name: DEFAULT_PI_OBSERVER_PROVIDER.to_string(),
        model_id: DEFAULT_PI_OBSERVER_MODEL.to_string(),
        prompt_version: DEFAULT_PI_OBSERVER_PROMPT_VERSION.to_string(),
        max_input_tokens: 4_096,
        max_output_tokens: 768,
    }
}

pub fn default_pi_reflector_profile() -> SemanticModelProfile {
    SemanticModelProfile {
        provider_name: DEFAULT_PI_REFLECTOR_PROVIDER.to_string(),
        model_id: DEFAULT_PI_REFLECTOR_MODEL.to_string(),
        prompt_version: DEFAULT_PI_REFLECTOR_PROMPT_VERSION.to_string(),
        max_input_tokens: 8_192,
        max_output_tokens: 1_024,
    }
}

pub trait PiObserverInvoker {
    fn invoke_observer(
        &self,
        canonical_input_json: &str,
        profile: &SemanticModelProfile,
        guardrails: &SemanticGuardrails,
    ) -> Result<String, SemanticAdapterError>;
}

#[derive(Debug, Default)]
pub struct NoopPiObserverInvoker;

impl PiObserverInvoker for NoopPiObserverInvoker {
    fn invoke_observer(
        &self,
        _canonical_input_json: &str,
        _profile: &SemanticModelProfile,
        _guardrails: &SemanticGuardrails,
    ) -> Result<String, SemanticAdapterError> {
        Err(SemanticAdapterError::new(
            SemanticFailureKind::ProviderError,
            "pi observer runtime is not configured",
        ))
    }
}

#[derive(Debug)]
pub struct PiObserverAdapter<I = NoopPiObserverInvoker> {
    invoker: I,
}

impl Default for PiObserverAdapter<NoopPiObserverInvoker> {
    fn default() -> Self {
        Self {
            invoker: NoopPiObserverInvoker,
        }
    }
}

impl<I> PiObserverAdapter<I> {
    pub fn new(invoker: I) -> Self {
        Self { invoker }
    }
}

impl<I> ObserverAdapter for PiObserverAdapter<I>
where
    I: PiObserverInvoker,
{
    fn observe_t1(
        &self,
        input: &ObserverInput,
        profile: &SemanticModelProfile,
        guardrails: &SemanticGuardrails,
    ) -> Result<ObserverOutput, SemanticAdapterError> {
        let canonical_input_json = canonical_json(input).map_err(|err| {
            SemanticAdapterError::new(
                SemanticFailureKind::InvalidOutput,
                format!("failed to serialize observer input: {err}"),
            )
        })?;

        let raw = self
            .invoker
            .invoke_observer(&canonical_input_json, profile, guardrails)?;

        ObserverOutput::parse_json(&raw).map_err(|err| {
            SemanticAdapterError::new(
                SemanticFailureKind::InvalidOutput,
                format!("failed to parse observer output: {err}"),
            )
        })
    }
}

pub struct SemanticObserverDistiller<A: ObserverAdapter> {
    config: DistillationConfig,
    semantic: SemanticObserverConfig,
    adapter: A,
}

impl<A: ObserverAdapter> SemanticObserverDistiller<A> {
    pub fn new(config: DistillationConfig, semantic: SemanticObserverConfig, adapter: A) -> Self {
        Self {
            config,
            semantic,
            adapter,
        }
    }

    pub fn distill_conversation(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<DistillationReport, DistillationError> {
        if self.semantic.mode == SemanticRuntimeMode::DeterministicOnly {
            return DeterministicDistiller::new(self.config.clone())
                .distill_conversation(store, conversation_id);
        }

        self.distill_with_semantic_t1(store, conversation_id)
    }

    fn observe_t1_with_guardrails(
        &self,
        input: &ObserverInput,
    ) -> (
        Result<ObserverOutput, SemanticAdapterError>,
        u16,
        Option<u64>,
    ) {
        let max_attempts = u16::from(self.semantic.guardrails.max_retries)
            .saturating_add(1)
            .max(1);
        let mut attempt = 1_u16;

        loop {
            if let Err(error) = enforce_observer_budget_guardrails(
                input.estimated_tokens,
                None,
                &self.semantic.profile,
                &self.semantic.guardrails,
            ) {
                return (Err(error), attempt, None);
            }

            let started_at = Utc::now();
            let observed =
                self.adapter
                    .observe_t1(input, &self.semantic.profile, &self.semantic.guardrails);
            let latency_ms = (Utc::now() - started_at).num_milliseconds().max(0) as u64;

            let guarded = observed.and_then(|output| {
                enforce_observer_budget_guardrails(
                    input.estimated_tokens,
                    Some(estimate_observer_output_tokens(&output)),
                    &self.semantic.profile,
                    &self.semantic.guardrails,
                )?;

                if self.semantic.guardrails.timeout_ms > 0
                    && latency_ms > self.semantic.guardrails.timeout_ms
                {
                    return Err(SemanticAdapterError::new(
                        SemanticFailureKind::Timeout,
                        format!(
                            "semantic observer exceeded timeout guardrail: {latency_ms}ms > {}ms",
                            self.semantic.guardrails.timeout_ms
                        ),
                    ));
                }

                Ok(output)
            });

            match guarded {
                Ok(output) => return (Ok(output), attempt, Some(latency_ms)),
                Err(error) => {
                    let retryable = matches!(
                        error.kind,
                        SemanticFailureKind::Timeout
                            | SemanticFailureKind::ProviderError
                            | SemanticFailureKind::LockConflict
                    );

                    if retryable && attempt < max_attempts {
                        attempt = attempt.saturating_add(1);
                        continue;
                    }

                    return (Err(error), attempt, Some(latency_ms));
                }
            }
        }
    }

    fn distill_with_semantic_t1(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<DistillationReport, DistillationError> {
        let t0_events = store.t0_events_for_conversation(conversation_id)?;
        if t0_events.is_empty() {
            return Ok(DistillationReport::default());
        }
        let context_states = store.context_states(conversation_id)?;

        let batches = plan_t1_batches(
            &t0_events,
            self.config.t1_target_tokens,
            self.config.t1_hard_cap_tokens,
        )?;
        let event_lookup = t0_events
            .iter()
            .map(|event| (event.compact_id.clone(), event))
            .collect::<BTreeMap<_, _>>();

        let mut report = DistillationReport {
            t0_events_processed: t0_events.len(),
            t1_batches_planned: batches.len(),
            chunked_t1: batches.len() > 1,
            ..DistillationReport::default()
        };

        let mut observations = Vec::new();

        for (batch_index, batch) in batches.iter().enumerate() {
            let mut batch_events = Vec::with_capacity(batch.compact_event_ids.len());
            for compact_id in &batch.compact_event_ids {
                let event = event_lookup.get(compact_id).ok_or_else(|| {
                    DistillationError::Internal(format!("missing compact event: {compact_id}"))
                })?;
                batch_events.push(*event);
            }

            let ts = batch_events
                .last()
                .map(|event| event.ts)
                .ok_or_else(|| DistillationError::Internal("empty T1 batch".to_string()))?;
            let active_tag = active_tag_for_ts(&context_states, ts)
                .unwrap_or_else(|| "global".to_string())
                .to_lowercase();
            let artifact_id = deterministic_artifact_id(
                "obs",
                conversation_id,
                &batch.compact_event_ids,
                self.config.t1_output_max_chars as u64,
            );

            let observer_input = ObserverInput::new(
                conversation_id,
                active_tag.clone(),
                batch.compact_event_ids.clone(),
                observer_payload_lines(&batch_events),
                batch.estimated_tokens,
                self.semantic.profile.prompt_version.clone(),
            )?;

            let (semantic_result, semantic_attempts, latency_ms) =
                self.observe_t1_with_guardrails(&observer_input);

            match semantic_result {
                Ok(output) => {
                    let text = synthesize_semantic_observation_text(
                        &output,
                        self.config.t1_output_max_chars,
                    );
                    store.insert_observation(
                        &artifact_id,
                        conversation_id,
                        ts,
                        &text,
                        &batch.compact_event_ids,
                    )?;

                    store.upsert_semantic_provenance(&SemanticProvenance {
                        artifact_id: artifact_id.clone(),
                        stage: SemanticStage::T1Observer,
                        runtime: SemanticRuntime::PiSemantic,
                        provider_name: Some(self.semantic.profile.provider_name.clone()),
                        model_id: Some(self.semantic.profile.model_id.clone()),
                        prompt_version: self.semantic.profile.prompt_version.clone(),
                        input_hash: observer_input.input_hash,
                        output_hash: Some(canonical_payload_hash(&output)?),
                        latency_ms,
                        attempt_count: semantic_attempts,
                        fallback_used: false,
                        fallback_reason: None,
                        failure_kind: None,
                        created_at: ts,
                    })?;

                    observations.push(ProducedObservation {
                        artifact_id,
                        ts,
                        active_tag,
                        estimated_tokens: estimate_tokens(&text),
                        text,
                    });
                }
                Err(error) => {
                    let fallback_text = synthesize_observation_text(
                        conversation_id,
                        batch_index + 1,
                        batches.len(),
                        batch,
                        &batch_events,
                        self.config.t1_output_max_chars,
                    );
                    store.insert_observation(
                        &artifact_id,
                        conversation_id,
                        ts,
                        &fallback_text,
                        &batch.compact_event_ids,
                    )?;

                    store.upsert_semantic_provenance(&SemanticProvenance {
                        artifact_id: artifact_id.clone(),
                        stage: SemanticStage::T1Observer,
                        runtime: SemanticRuntime::PiSemantic,
                        provider_name: Some(self.semantic.profile.provider_name.clone()),
                        model_id: Some(self.semantic.profile.model_id.clone()),
                        prompt_version: self.semantic.profile.prompt_version.clone(),
                        input_hash: observer_input.input_hash.clone(),
                        output_hash: None,
                        latency_ms,
                        attempt_count: semantic_attempts,
                        fallback_used: true,
                        fallback_reason: Some(error.message.clone()),
                        failure_kind: Some(error.kind),
                        created_at: ts,
                    })?;

                    store.upsert_semantic_provenance(&SemanticProvenance {
                        artifact_id: artifact_id.clone(),
                        stage: SemanticStage::T1Observer,
                        runtime: SemanticRuntime::Deterministic,
                        provider_name: None,
                        model_id: None,
                        prompt_version: "deterministic.observer.v1".to_string(),
                        input_hash: canonical_payload_hash(&(
                            conversation_id,
                            &batch.compact_event_ids,
                            batch.estimated_tokens,
                        ))?,
                        output_hash: Some(canonical_payload_hash(&fallback_text)?),
                        latency_ms: None,
                        attempt_count: semantic_attempts.saturating_add(1),
                        fallback_used: true,
                        fallback_reason: Some(format!(
                            "semantic observer failed ({})",
                            error.kind.as_str()
                        )),
                        failure_kind: Some(error.kind),
                        created_at: ts,
                    })?;

                    observations.push(ProducedObservation {
                        artifact_id,
                        ts,
                        active_tag,
                        estimated_tokens: estimate_tokens(&fallback_text),
                        text: fallback_text,
                    });
                }
            }

            report.t1_artifacts_written += 1;
        }

        let deterministic = DeterministicDistiller::new(self.config.clone());
        report.t2_artifacts_written = deterministic.emit_reflections(
            store,
            conversation_id,
            &observations,
            self.config.t2_trigger_tokens,
            self.config.t2_output_max_chars,
        )?;

        if self.config.enable_attribution {
            let attribution = TaskAttributionEngine::new(AttributionConfig::default())
                .attribute_conversation(store, conversation_id)?;
            report.attribution_links_written = attribution.links_written;
        }

        Ok(report)
    }
}

#[derive(Debug)]
pub struct SessionObserverRunOutcome {
    pub session_id: String,
    pub conversation_id: String,
    pub trigger: ObserverTrigger,
    pub enqueued_at: chrono::DateTime<chrono::Utc>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub report: Result<DistillationReport, DistillationError>,
    pub progress: Option<MindObserverFeedProgress>,
}

pub fn observer_feed_event_from_outcome(
    store: &MindStore,
    outcome: &SessionObserverRunOutcome,
    completed_at: chrono::DateTime<chrono::Utc>,
) -> MindObserverFeedEvent {
    let mut event = MindObserverFeedEvent {
        status: MindObserverFeedStatus::Success,
        trigger: observer_feed_trigger(outcome.trigger.kind),
        conversation_id: Some(outcome.conversation_id.clone()),
        runtime: None,
        attempt_count: None,
        latency_ms: None,
        reason: None,
        failure_kind: None,
        enqueued_at: Some(outcome.enqueued_at.to_rfc3339()),
        started_at: Some(outcome.started_at.to_rfc3339()),
        completed_at: Some(completed_at.to_rfc3339()),
        progress: outcome.progress.clone(),
    };

    match &outcome.report {
        Err(error) => {
            event.status = MindObserverFeedStatus::Error;
            event.reason = Some(error.to_string());
            event
        }
        Ok(report) => {
            if report.t1_artifacts_written == 0 {
                event.reason = Some("observer run produced no t1 artifacts".to_string());
                return event;
            }

            let Some((runtime, attempt_count, latency_ms, fallback_used, reason, failure_kind)) =
                latest_t1_provenance_summary(store, &outcome.conversation_id)
            else {
                return event;
            };

            event.runtime = Some(runtime);
            event.attempt_count = Some(attempt_count);
            event.latency_ms = latency_ms;
            event.reason = reason;
            event.failure_kind = failure_kind;
            event.status = if fallback_used {
                MindObserverFeedStatus::Fallback
            } else {
                MindObserverFeedStatus::Success
            };
            event
        }
    }
}

fn observer_feed_progress(
    store: &MindStore,
    conversation_id: &str,
    config: &DistillationConfig,
) -> Option<MindObserverFeedProgress> {
    let t0_events = store.t0_events_for_conversation(conversation_id).ok()?;
    let t0_estimated_tokens = t0_events.iter().fold(0_u32, |total, event| {
        total.saturating_add(estimate_t0_event_tokens(event))
    });

    Some(MindObserverFeedProgress {
        t0_estimated_tokens,
        t1_target_tokens: config.t1_target_tokens,
        t1_hard_cap_tokens: config.t1_hard_cap_tokens,
        tokens_until_next_run: config.t1_target_tokens.saturating_sub(t0_estimated_tokens),
    })
}

fn latest_t1_provenance_summary(
    store: &MindStore,
    conversation_id: &str,
) -> Option<(
    String,
    u16,
    Option<u64>,
    bool,
    Option<String>,
    Option<String>,
)> {
    let artifacts = store.artifacts_for_conversation(conversation_id).ok()?;
    let artifact_id = artifacts
        .iter()
        .rev()
        .find(|artifact| artifact.kind == "t1")?
        .artifact_id
        .clone();
    let provenance = store.semantic_provenance_for_artifact(&artifact_id).ok()?;
    if provenance.is_empty() {
        return None;
    }

    let attempt_count = provenance.last().map(|row| row.attempt_count).unwrap_or(1);
    let runtime = provenance
        .last()
        .map(|row| row.runtime.as_str().to_string())
        .unwrap_or_else(|| SemanticRuntime::Deterministic.as_str().to_string());
    let latency_ms = provenance.iter().rev().find_map(|row| row.latency_ms);
    let fallback_used = provenance.iter().any(|row| row.fallback_used);
    let reason = provenance
        .iter()
        .rev()
        .find_map(|row| row.fallback_reason.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let failure_kind = provenance
        .iter()
        .rev()
        .find_map(|row| row.failure_kind.map(|kind| kind.as_str().to_string()));

    Some((
        runtime,
        attempt_count,
        latency_ms,
        fallback_used,
        reason,
        failure_kind,
    ))
}

fn observer_feed_trigger(kind: ObserverTriggerKind) -> MindObserverFeedTriggerKind {
    match kind {
        ObserverTriggerKind::TokenThreshold => MindObserverFeedTriggerKind::TokenThreshold,
        ObserverTriggerKind::TaskCompleted => MindObserverFeedTriggerKind::TaskCompleted,
        ObserverTriggerKind::ManualShortcut => MindObserverFeedTriggerKind::ManualShortcut,
    }
}

pub struct SessionObserverSidecar<A: ObserverAdapter> {
    queue: SessionObserverQueue,
    distiller: SemanticObserverDistiller<A>,
}

impl<A: ObserverAdapter> SessionObserverSidecar<A> {
    pub fn new(config: DistillationConfig, semantic: SemanticObserverConfig, adapter: A) -> Self {
        let queue = SessionObserverQueue::new(ObserverQueueConfig {
            debounce_ms: semantic.guardrails.queue_debounce_ms,
        });
        let distiller = SemanticObserverDistiller::new(config, semantic, adapter);
        Self { queue, distiller }
    }

    pub fn enqueue_turn(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.enqueue_token_threshold(session_id, conversation_id, now);
    }

    pub fn enqueue_token_threshold(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::token_threshold(),
            now,
        );
    }

    pub fn enqueue_task_completed(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::task_completed(),
            now,
        );
    }

    pub fn enqueue_manual(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        self.queue.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::manual_shortcut(),
            now,
        );
    }

    pub fn run_ready(
        &mut self,
        store: &MindStore,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Vec<SessionObserverRunOutcome> {
        let mut outcomes = Vec::new();

        while let Some(run) = self.queue.claim_ready(now) {
            let progress =
                observer_feed_progress(store, &run.conversation_id, &self.distiller.config);
            let report = self
                .distiller
                .distill_conversation(store, &run.conversation_id);
            self.queue.complete_run(&run, now);
            self.enqueue_branch_backfill(store, &run.session_id, &run.conversation_id, now);
            outcomes.push(SessionObserverRunOutcome {
                session_id: run.session_id,
                conversation_id: run.conversation_id,
                trigger: run.trigger,
                enqueued_at: run.enqueued_at,
                started_at: run.started_at,
                report,
                progress,
            });
        }

        outcomes
    }

    fn enqueue_branch_backfill(
        &mut self,
        store: &MindStore,
        session_id: &str,
        conversation_id: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        let conversation_ids = match store.session_tree_conversations(session_id, conversation_id) {
            Ok(ids) => ids,
            Err(_) => return,
        };

        for candidate in conversation_ids {
            if candidate == conversation_id {
                continue;
            }
            match store.conversation_needs_observer_run(&candidate) {
                Ok(true) => self.queue.enqueue_with_trigger(
                    session_id.to_string(),
                    candidate,
                    ObserverTrigger {
                        kind: ObserverTriggerKind::TokenThreshold,
                        priority: ObserverTriggerPriority::Normal,
                        bypass_debounce: true,
                    },
                    now,
                ),
                Ok(false) | Err(_) => {}
            }
        }
    }

    pub fn queue(&self) -> &SessionObserverQueue {
        &self.queue
    }
}

pub struct DeterministicDistiller {
    config: DistillationConfig,
}

impl DeterministicDistiller {
    pub fn new(config: DistillationConfig) -> Self {
        Self { config }
    }

    pub fn distill_conversation(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<DistillationReport, DistillationError> {
        let t0_events = store.t0_events_for_conversation(conversation_id)?;
        if t0_events.is_empty() {
            return Ok(DistillationReport::default());
        }
        let context_states = store.context_states(conversation_id)?;

        let batches = plan_t1_batches(
            &t0_events,
            self.config.t1_target_tokens,
            self.config.t1_hard_cap_tokens,
        )?;
        let event_lookup = t0_events
            .iter()
            .map(|event| (event.compact_id.clone(), event))
            .collect::<BTreeMap<_, _>>();

        let mut report = DistillationReport {
            t0_events_processed: t0_events.len(),
            t1_batches_planned: batches.len(),
            chunked_t1: batches.len() > 1,
            ..DistillationReport::default()
        };

        let mut observations = Vec::new();

        for (batch_index, batch) in batches.iter().enumerate() {
            let mut batch_events = Vec::with_capacity(batch.compact_event_ids.len());
            for compact_id in &batch.compact_event_ids {
                let event = event_lookup.get(compact_id).ok_or_else(|| {
                    DistillationError::Internal(format!("missing compact event: {compact_id}"))
                })?;
                batch_events.push(*event);
            }

            let text = synthesize_observation_text(
                conversation_id,
                batch_index + 1,
                batches.len(),
                batch,
                &batch_events,
                self.config.t1_output_max_chars,
            );
            let artifact_id = deterministic_artifact_id(
                "obs",
                conversation_id,
                &batch.compact_event_ids,
                self.config.t1_output_max_chars as u64,
            );
            let ts = batch_events
                .last()
                .map(|event| event.ts)
                .ok_or_else(|| DistillationError::Internal("empty T1 batch".to_string()))?;

            store.insert_observation(
                &artifact_id,
                conversation_id,
                ts,
                &text,
                &batch.compact_event_ids,
            )?;
            persist_deterministic_provenance(
                store,
                &artifact_id,
                SemanticStage::T1Observer,
                "deterministic.observer.v1",
                canonical_payload_hash(&(
                    conversation_id,
                    &batch.compact_event_ids,
                    batch.estimated_tokens,
                ))?,
                Some(canonical_payload_hash(&text)?),
                ts,
            )?;

            let active_tag = active_tag_for_ts(&context_states, ts)
                .unwrap_or_else(|| "global".to_string())
                .to_lowercase();

            observations.push(ProducedObservation {
                artifact_id,
                ts,
                active_tag,
                estimated_tokens: estimate_tokens(&text),
                text,
            });
            report.t1_artifacts_written += 1;
        }

        report.t2_artifacts_written = self.emit_reflections(
            store,
            conversation_id,
            &observations,
            self.config.t2_trigger_tokens,
            self.config.t2_output_max_chars,
        )?;

        if self.config.enable_attribution {
            let attribution = TaskAttributionEngine::new(AttributionConfig::default())
                .attribute_conversation(store, conversation_id)?;
            report.attribution_links_written = attribution.links_written;
        }

        Ok(report)
    }

    fn emit_reflections(
        &self,
        store: &MindStore,
        conversation_id: &str,
        observations: &[ProducedObservation],
        trigger_tokens: u32,
        max_chars: usize,
    ) -> Result<usize, DistillationError> {
        if observations.is_empty() || trigger_tokens == 0 {
            return Ok(0);
        }

        let mut by_tag = BTreeMap::<String, Vec<&ProducedObservation>>::new();
        for observation in observations {
            by_tag
                .entry(observation.active_tag.clone())
                .or_default()
                .push(observation);
        }

        let mut created = 0usize;
        for (tag, tagged_observations) in by_tag {
            let refs = tagged_observations
                .iter()
                .map(|observation| ObservationRef {
                    artifact_id: observation.artifact_id.clone(),
                    conversation_id: conversation_id.to_string(),
                    active_tag: tag.clone(),
                })
                .collect::<Vec<_>>();
            let _ = build_t2_workstream_batch(&tag, &refs)?;

            let total_tokens = tagged_observations
                .iter()
                .map(|observation| observation.estimated_tokens)
                .sum::<u32>();
            if total_tokens < trigger_tokens {
                continue;
            }

            let chunks = chunk_observations_for_t2(&tagged_observations, trigger_tokens);
            for (chunk_index, chunk) in chunks.iter().enumerate() {
                let obs_ids = chunk
                    .iter()
                    .map(|observation| observation.artifact_id.clone())
                    .collect::<Vec<_>>();
                let text = synthesize_reflection_text(
                    &tag,
                    chunk_index + 1,
                    chunks.len(),
                    chunk,
                    max_chars,
                );
                let ts = chunk
                    .last()
                    .map(|observation| observation.ts)
                    .ok_or_else(|| DistillationError::Internal("empty T2 chunk".to_string()))?;
                let artifact_id =
                    deterministic_artifact_id("ref", conversation_id, &obs_ids, max_chars as u64);

                store.insert_reflection(&artifact_id, conversation_id, ts, &text, &obs_ids)?;
                persist_deterministic_provenance(
                    store,
                    &artifact_id,
                    SemanticStage::T2Reflector,
                    "deterministic.reflector.v1",
                    canonical_payload_hash(&(conversation_id, &tag, &obs_ids))?,
                    Some(canonical_payload_hash(&text)?),
                    ts,
                )?;
                created += 1;
            }
        }

        Ok(created)
    }
}

fn plan_t1_batches(
    t0_events: &[StoredCompactEvent],
    target_tokens: u32,
    hard_cap_tokens: u32,
) -> Result<Vec<T1Batch>, DistillationError> {
    if t0_events.is_empty() {
        return Ok(Vec::new());
    }

    let first_conversation = t0_events[0].conversation_id.clone();
    let mut total_tokens = 0_u32;
    let mut compact_ids = Vec::with_capacity(t0_events.len());

    for event in t0_events {
        if event.conversation_id != first_conversation {
            return Err(DistillationError::Contract(
                MindContractError::T1CrossConversation {
                    expected: first_conversation.clone(),
                    found: event.conversation_id.clone(),
                },
            ));
        }
        let event_tokens = estimate_t0_event_tokens(event);
        total_tokens = total_tokens.saturating_add(event_tokens);
        compact_ids.push(event.compact_id.clone());
    }
    if total_tokens <= target_tokens {
        let batch = T1Batch {
            conversation_id: first_conversation,
            compact_event_ids: compact_ids,
            estimated_tokens: total_tokens,
        };
        batch.validate_hard_cap(hard_cap_tokens)?;
        return Ok(vec![batch]);
    }

    let mut batches = Vec::new();
    let mut current_ids = Vec::new();
    let mut current_tokens = 0_u32;

    for event in t0_events {
        let event_tokens = estimate_t0_event_tokens(event);
        if event_tokens > hard_cap_tokens {
            return Err(DistillationError::Contract(
                MindContractError::T1OverHardCap {
                    estimated_tokens: event_tokens,
                    hard_cap: hard_cap_tokens,
                },
            ));
        }

        if !current_ids.is_empty() && current_tokens.saturating_add(event_tokens) > target_tokens {
            batches.push(T1Batch {
                conversation_id: event.conversation_id.clone(),
                compact_event_ids: current_ids,
                estimated_tokens: current_tokens,
            });
            current_ids = Vec::new();
            current_tokens = 0;
        }

        current_ids.push(event.compact_id.clone());
        current_tokens = current_tokens.saturating_add(event_tokens);
    }

    if !current_ids.is_empty() {
        batches.push(T1Batch {
            conversation_id: t0_events[0].conversation_id.clone(),
            compact_event_ids: current_ids,
            estimated_tokens: current_tokens,
        });
    }

    validate_t1_scope(&batches)?;
    for batch in &batches {
        batch.validate_hard_cap(hard_cap_tokens)?;
    }
    Ok(batches)
}

fn chunk_observations_for_t2<'a>(
    observations: &'a [&'a ProducedObservation],
    trigger_tokens: u32,
) -> Vec<Vec<&'a ProducedObservation>> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_tokens = 0_u32;

    for observation in observations {
        let tokens = observation.estimated_tokens.max(1);
        if !current.is_empty() && current_tokens.saturating_add(tokens) > trigger_tokens {
            chunks.push(current);
            current = Vec::new();
            current_tokens = 0;
        }

        current.push(*observation);
        current_tokens = current_tokens.saturating_add(tokens);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn synthesize_observation_text(
    conversation_id: &str,
    chunk_index: usize,
    chunk_count: usize,
    batch: &T1Batch,
    events: &[&StoredCompactEvent],
    max_chars: usize,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "T1 observation {chunk_index}/{chunk_count} for {conversation_id}; tokens={} events={}",
        batch.estimated_tokens,
        events.len()
    ));

    for event in events {
        if let Some(text) = event.text.as_deref() {
            let role = event.role.map(role_label).unwrap_or("message");
            lines.push(format!("{role}: {}", normalize_text(text)));
            continue;
        }

        if let Some(tool_meta) = event.tool_meta.as_ref() {
            lines.push(format!(
                "tool:{} status={:?} latency_ms={} exit_code={} output_bytes={} redacted={}",
                tool_meta.tool_name,
                tool_meta.status,
                tool_meta
                    .latency_ms
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta
                    .exit_code
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta.output_bytes,
                tool_meta.redacted
            ));
        }
    }

    truncate_chars(lines.join("\n"), max_chars)
}

fn observer_payload_lines(events: &[&StoredCompactEvent]) -> Vec<String> {
    let mut lines = Vec::new();

    for event in events {
        if let Some(text) = event.text.as_deref() {
            let role = event.role.map(role_label).unwrap_or("message");
            lines.push(format!("{role}: {}", normalize_text(text)));
            continue;
        }

        if let Some(tool_meta) = event.tool_meta.as_ref() {
            lines.push(format!(
                "tool:{} status={:?} latency_ms={} exit_code={} output_bytes={} redacted={}",
                tool_meta.tool_name,
                tool_meta.status,
                tool_meta
                    .latency_ms
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta
                    .exit_code
                    .map_or("na".to_string(), |value| value.to_string()),
                tool_meta.output_bytes,
                tool_meta.redacted
            ));
        }
    }

    lines
}

fn synthesize_semantic_observation_text(output: &ObserverOutput, max_chars: usize) -> String {
    let mut lines = Vec::new();
    lines.push(output.summary.trim().to_string());

    for point in &output.key_points {
        lines.push(format!("- {}", point.trim()));
    }

    if !output.citations.is_empty() {
        lines.push(format!("citations: {}", output.citations.join(", ")));
    }

    truncate_chars(lines.join("\n"), max_chars)
}

fn estimate_observer_output_tokens(output: &ObserverOutput) -> u32 {
    let mut chars = output.summary.chars().count();
    chars += output
        .key_points
        .iter()
        .map(|point| point.chars().count() + 2)
        .sum::<usize>();
    chars += output
        .citations
        .iter()
        .map(|citation| citation.chars().count() + 2)
        .sum::<usize>();
    ((chars as u32) / 4).max(1)
}

fn estimate_semantic_cost_micros(tokens: u32) -> u64 {
    u64::from(tokens).saturating_mul(DEFAULT_SEMANTIC_COST_MICROS_PER_TOKEN)
}

fn enforce_observer_budget_guardrails(
    input_tokens: u32,
    output_tokens: Option<u32>,
    profile: &SemanticModelProfile,
    guardrails: &SemanticGuardrails,
) -> Result<(), SemanticAdapterError> {
    if input_tokens > profile.max_input_tokens {
        return Err(SemanticAdapterError::new(
            SemanticFailureKind::BudgetExceeded,
            format!(
                "observer input tokens exceed profile limit: {input_tokens} > {}",
                profile.max_input_tokens
            ),
        ));
    }

    if let Some(output_tokens) = output_tokens {
        if output_tokens > profile.max_output_tokens {
            return Err(SemanticAdapterError::new(
                SemanticFailureKind::BudgetExceeded,
                format!(
                    "observer output tokens exceed profile limit: {output_tokens} > {}",
                    profile.max_output_tokens
                ),
            ));
        }
    }

    let budget_tokens = if guardrails.max_budget_tokens == 0 {
        u32::MAX
    } else {
        guardrails.max_budget_tokens
    };

    let observed_tokens = input_tokens.saturating_add(output_tokens.unwrap_or(0));
    if observed_tokens > budget_tokens {
        return Err(SemanticAdapterError::new(
            SemanticFailureKind::BudgetExceeded,
            format!("observer token budget exceeded: {observed_tokens} > {budget_tokens}"),
        ));
    }

    if guardrails.max_budget_cost_micros > 0 {
        let projected_output = output_tokens.unwrap_or(profile.max_output_tokens);
        let projected_tokens = input_tokens.saturating_add(projected_output);
        let projected_cost = estimate_semantic_cost_micros(projected_tokens);
        if projected_cost > guardrails.max_budget_cost_micros {
            return Err(SemanticAdapterError::new(
                SemanticFailureKind::BudgetExceeded,
                format!(
                    "observer cost budget exceeded: {projected_cost} > {} micros",
                    guardrails.max_budget_cost_micros
                ),
            ));
        }
    }

    Ok(())
}

fn synthesize_reflection_text(
    tag: &str,
    chunk_index: usize,
    chunk_count: usize,
    observations: &[&ProducedObservation],
    max_chars: usize,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "T2 reflection {chunk_index}/{chunk_count} for tag={tag}; observations={}",
        observations.len()
    ));

    for observation in observations {
        let preview = truncate_chars(normalize_text(&observation.text), 180);
        lines.push(format!("{}: {}", observation.artifact_id, preview));
    }

    truncate_chars(lines.join("\n"), max_chars)
}

fn deterministic_artifact_id(
    prefix: &str,
    conversation_id: &str,
    trace_ids: &[String],
    budget: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(conversation_id.as_bytes());
    hasher.update(b"|");
    hasher.update(prefix.as_bytes());
    hasher.update(b"|");
    hasher.update(budget.to_string().as_bytes());

    let mut sorted = trace_ids.to_vec();
    sorted.sort();
    for trace_id in sorted {
        hasher.update(b"|");
        hasher.update(trace_id.as_bytes());
    }

    let digest = hasher.finalize();
    format!("{prefix}:{}", hex_prefix(&digest, 16))
}

fn hex_prefix(digest: &[u8], bytes: usize) -> String {
    let mut output = String::with_capacity(bytes * 2);
    for byte in digest.iter().take(bytes) {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn persist_deterministic_provenance(
    store: &MindStore,
    artifact_id: &str,
    stage: SemanticStage,
    prompt_version: &str,
    input_hash: String,
    output_hash: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), DistillationError> {
    store.upsert_semantic_provenance(&SemanticProvenance {
        artifact_id: artifact_id.to_string(),
        stage,
        runtime: SemanticRuntime::Deterministic,
        provider_name: None,
        model_id: None,
        prompt_version: prompt_version.to_string(),
        input_hash,
        output_hash,
        latency_ms: None,
        attempt_count: 1,
        fallback_used: false,
        fallback_reason: None,
        failure_kind: None,
        created_at,
    })?;
    Ok(())
}

fn role_label(role: ConversationRole) -> &'static str {
    match role {
        ConversationRole::System => "system",
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    }
}

fn estimate_t0_event_tokens(event: &StoredCompactEvent) -> u32 {
    let text_tokens = event.text.as_deref().map_or(0, estimate_tokens);
    let tool_tokens = event
        .tool_meta
        .as_ref()
        .map_or(0, |meta| 14 + ((meta.output_bytes as u32) / 180));
    (text_tokens + tool_tokens).max(1)
}

fn estimate_tokens(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    (chars / 4).max(1)
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: String, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    if text.chars().count() <= max_chars {
        return text;
    }

    let mut out = text
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn active_tag_for_ts(
    context_states: &[ConversationContextState],
    ts: chrono::DateTime<chrono::Utc>,
) -> Option<String> {
    let mut active_tag = None;
    for context in context_states {
        if context.ts > ts {
            break;
        }
        if let Some(tag) = context
            .active_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            active_tag = Some(tag.to_string());
        }
    }
    active_tag
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_core::mind_contracts::{
        canonical_lineage_attrs, compact_raw_event_to_t0, ConversationLineageMetadata,
        ConversationRole, MessageEvent, ObserverAdapter, ObserverInput, ObserverOutput, RawEvent,
        RawEventBody, SemanticAdapterError, SemanticFailureKind, SemanticGuardrails,
        SemanticModelProfile, T0CompactionPolicy,
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

    fn raw_message(
        event_id: &str,
        conversation_id: &str,
        ts: DateTime<Utc>,
        text: &str,
    ) -> RawEvent {
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
}
