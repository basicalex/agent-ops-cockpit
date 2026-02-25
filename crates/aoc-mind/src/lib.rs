use aoc_core::mind_contracts::{
    build_t2_workstream_batch, canonical_payload_hash, validate_t1_scope, ConversationRole,
    MindContractError, ObservationRef, SemanticProvenance, SemanticRuntime, SemanticStage, T1Batch,
    T1_PARSER_HARD_CAP_TOKENS, T1_PARSER_TARGET_TOKENS,
};
use aoc_storage::{ConversationContextState, MindStore, StorageError, StoredCompactEvent};
use aoc_task_attribution::{AttributionConfig, AttributionError, TaskAttributionEngine};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

const DEFAULT_T1_OUTPUT_MAX_CHARS: usize = 1_200;
const DEFAULT_T2_OUTPUT_MAX_CHARS: usize = 1_400;
const DEFAULT_T2_TRIGGER_TOKENS: u32 = 2_400;

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
        compact_raw_event_to_t0, ConversationRole, MessageEvent, RawEvent, RawEventBody,
        T0CompactionPolicy,
    };
    use chrono::{DateTime, TimeZone, Utc};

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
}
