use aoc_core::mind_contracts::{ArtifactTaskLink, ArtifactTaskRelation, MindContractError};
use aoc_storage::{
    ConversationContextState, MindStore, StorageError, StoredArtifact, StoredCompactEvent,
};
use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const CONF_ACTIVE_BPS: u16 = 8_500;
const CONF_MENTIONED_BPS: u16 = 7_200;
const CONF_WORKED_ON_BPS: u16 = 8_800;
const CONF_BACKFILL_BPS: u16 = 9_300;
const CONF_COMPLETED_BPS: u16 = 9_600;

#[derive(Debug, Error)]
pub enum AttributionError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
}

#[derive(Debug, Clone)]
pub struct AttributionConfig {
    pub mention_window_before: Duration,
    pub mention_window_after: Duration,
}

impl Default for AttributionConfig {
    fn default() -> Self {
        Self {
            mention_window_before: Duration::minutes(30),
            mention_window_after: Duration::minutes(5),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AttributionReport {
    pub artifacts_processed: usize,
    pub links_written: usize,
    pub backfilled_links: usize,
}

#[derive(Debug, Clone)]
struct CompletionSignal {
    task_id: String,
    ts: DateTime<Utc>,
    evidence_id: String,
}

#[derive(Debug, Clone)]
struct LinkDraft {
    task_id: String,
    relation: ArtifactTaskRelation,
    confidence_bps: u16,
    source: String,
    evidence_event_ids: BTreeSet<String>,
    end_ts: Option<DateTime<Utc>>,
}

impl LinkDraft {
    fn key(&self) -> (String, String) {
        (self.task_id.clone(), relation_key(self.relation))
    }
}

pub struct TaskAttributionEngine {
    config: AttributionConfig,
}

impl TaskAttributionEngine {
    pub fn new(config: AttributionConfig) -> Self {
        Self { config }
    }

    pub fn attribute_conversation(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<AttributionReport, AttributionError> {
        let artifacts = store.artifacts_for_conversation(conversation_id)?;
        if artifacts.is_empty() {
            return Ok(AttributionReport::default());
        }

        let contexts = store.context_states(conversation_id)?;
        let t0_events = store.t0_events_for_conversation(conversation_id)?;
        let completions = completion_signals(&contexts);

        let mut report = AttributionReport::default();
        let mut context_cursor = 0usize;
        let mut current_context: Option<&ConversationContextState> = None;

        for artifact in artifacts {
            report.artifacts_processed += 1;

            while context_cursor < contexts.len() && contexts[context_cursor].ts <= artifact.ts {
                current_context = Some(&contexts[context_cursor]);
                context_cursor += 1;
            }

            let mut drafts: BTreeMap<(String, String), LinkDraft> = BTreeMap::new();
            let active_tasks = current_context
                .map(|snapshot| snapshot.active_tasks.clone())
                .unwrap_or_default();

            let mut mentioned_tasks = mention_tasks_from_artifact(&artifact);
            let mention_from_t0 = mention_tasks_from_t0(
                artifact.ts,
                &t0_events,
                self.config.mention_window_before,
                self.config.mention_window_after,
            );
            merge_mentions(&mut mentioned_tasks, mention_from_t0);

            for task_id in &active_tasks {
                upsert_draft(
                    &mut drafts,
                    draft(
                        task_id,
                        ArtifactTaskRelation::Active,
                        CONF_ACTIVE_BPS,
                        "context_state",
                        [format!(
                            "ctx:{conversation_id}:{}",
                            artifact.ts.to_rfc3339()
                        )],
                        None,
                    ),
                );
            }

            for (task_id, evidence_ids) in &mentioned_tasks {
                upsert_draft(
                    &mut drafts,
                    draft(
                        task_id,
                        ArtifactTaskRelation::Mentioned,
                        CONF_MENTIONED_BPS,
                        "t0_or_artifact_mentions",
                        evidence_ids.iter().cloned(),
                        None,
                    ),
                );
            }

            let active_set = active_tasks.into_iter().collect::<BTreeSet<String>>();
            let mentioned_set = mentioned_tasks
                .keys()
                .cloned()
                .collect::<BTreeSet<String>>();
            let related_tasks = active_set
                .union(&mentioned_set)
                .cloned()
                .collect::<BTreeSet<String>>();

            for task_id in related_tasks {
                let active = active_set.contains(&task_id);
                let mention_evidence = mentioned_tasks.get(&task_id);

                let mut worked_on_evidence = BTreeSet::new();
                if active {
                    worked_on_evidence.insert(format!(
                        "ctx:{conversation_id}:{}",
                        artifact.ts.to_rfc3339()
                    ));
                }
                if let Some(mention_evidence) = mention_evidence {
                    worked_on_evidence.extend(mention_evidence.iter().cloned());
                }

                upsert_draft(
                    &mut drafts,
                    draft(
                        &task_id,
                        ArtifactTaskRelation::WorkedOn,
                        CONF_WORKED_ON_BPS,
                        "active_or_mentioned",
                        worked_on_evidence,
                        None,
                    ),
                );

                if let Some(completion) =
                    first_completion_after(&completions, &task_id, artifact.ts)
                {
                    upsert_draft(
                        &mut drafts,
                        draft(
                            &task_id,
                            ArtifactTaskRelation::WorkedOn,
                            CONF_BACKFILL_BPS,
                            "completion_backfill",
                            [completion.evidence_id.clone()],
                            Some(completion.ts),
                        ),
                    );
                    report.backfilled_links += 1;
                }

                if let Some(completion) =
                    last_completion_before_or_at(&completions, &task_id, artifact.ts)
                {
                    upsert_draft(
                        &mut drafts,
                        draft(
                            &task_id,
                            ArtifactTaskRelation::Completed,
                            CONF_COMPLETED_BPS,
                            "completion_signal",
                            [completion.evidence_id.clone()],
                            Some(completion.ts),
                        ),
                    );
                }
            }

            for draft in drafts.into_values() {
                let mut evidence_event_ids =
                    draft.evidence_event_ids.into_iter().collect::<Vec<_>>();
                evidence_event_ids.sort();
                let link = ArtifactTaskLink::new(
                    artifact.artifact_id.clone(),
                    draft.task_id,
                    draft.relation,
                    draft.confidence_bps,
                    evidence_event_ids,
                    draft.source,
                    artifact.ts,
                    draft.end_ts,
                )?;
                store.upsert_artifact_task_link(&link)?;
                report.links_written += 1;
            }
        }

        Ok(report)
    }
}

fn completion_signals(contexts: &[ConversationContextState]) -> Vec<CompletionSignal> {
    let mut out = Vec::new();
    for context in contexts {
        let lifecycle = context
            .lifecycle
            .as_deref()
            .unwrap_or_default()
            .to_lowercase();
        let is_completion = lifecycle.contains("done")
            || lifecycle.contains("complete")
            || lifecycle.contains("cancel")
            || lifecycle.contains("closed");
        if !is_completion {
            continue;
        }

        for task_id in &context.signal_task_ids {
            out.push(CompletionSignal {
                task_id: task_id.clone(),
                ts: context.ts,
                evidence_id: format!(
                    "ctx:{}:{}:{}",
                    context.conversation_id,
                    context.ts.to_rfc3339(),
                    context.signal_source
                ),
            });
        }
    }
    out.sort_by(|left, right| {
        left.ts
            .cmp(&right.ts)
            .then(left.task_id.cmp(&right.task_id))
    });
    out
}

fn first_completion_after(
    completions: &[CompletionSignal],
    task_id: &str,
    ts: DateTime<Utc>,
) -> Option<CompletionSignal> {
    completions
        .iter()
        .find(|completion| completion.task_id == task_id && completion.ts > ts)
        .cloned()
}

fn last_completion_before_or_at(
    completions: &[CompletionSignal],
    task_id: &str,
    ts: DateTime<Utc>,
) -> Option<CompletionSignal> {
    completions
        .iter()
        .rev()
        .find(|completion| completion.task_id == task_id && completion.ts <= ts)
        .cloned()
}

fn mention_tasks_from_artifact(artifact: &StoredArtifact) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for task_id in extract_task_ids(&artifact.text) {
        out.entry(task_id)
            .or_insert_with(BTreeSet::new)
            .insert(format!("artifact:{}:text", artifact.artifact_id));
    }
    out
}

fn mention_tasks_from_t0(
    ts: DateTime<Utc>,
    events: &[StoredCompactEvent],
    before: Duration,
    after: Duration,
) -> BTreeMap<String, BTreeSet<String>> {
    let start = ts - before;
    let end = ts + after;
    let mut out = BTreeMap::new();

    for event in events {
        if event.ts < start || event.ts > end {
            continue;
        }

        if let Some(text) = event.text.as_deref() {
            for task_id in extract_task_ids(text) {
                out.entry(task_id)
                    .or_insert_with(BTreeSet::new)
                    .insert(format!("t0:{}", event.compact_id));
            }
        }
    }

    out
}

fn merge_mentions(
    base: &mut BTreeMap<String, BTreeSet<String>>,
    extra: BTreeMap<String, BTreeSet<String>>,
) {
    for (task_id, evidence_ids) in extra {
        base.entry(task_id)
            .or_insert_with(BTreeSet::new)
            .extend(evidence_ids);
    }
}

fn draft<I>(
    task_id: &str,
    relation: ArtifactTaskRelation,
    confidence_bps: u16,
    source: &str,
    evidence_ids: I,
    end_ts: Option<DateTime<Utc>>,
) -> LinkDraft
where
    I: IntoIterator<Item = String>,
{
    LinkDraft {
        task_id: task_id.to_string(),
        relation,
        confidence_bps,
        source: source.to_string(),
        evidence_event_ids: evidence_ids.into_iter().collect(),
        end_ts,
    }
}

fn upsert_draft(drafts: &mut BTreeMap<(String, String), LinkDraft>, draft: LinkDraft) {
    let key = draft.key();
    if let Some(existing) = drafts.get_mut(&key) {
        if draft.confidence_bps > existing.confidence_bps {
            existing.confidence_bps = draft.confidence_bps;
            existing.source = draft.source;
        }
        existing.evidence_event_ids.extend(draft.evidence_event_ids);
        if existing.end_ts.is_none() {
            existing.end_ts = draft.end_ts;
        }
    } else {
        drafts.insert(key, draft);
    }
}

fn relation_key(relation: ArtifactTaskRelation) -> String {
    match relation {
        ArtifactTaskRelation::Active => "active",
        ArtifactTaskRelation::WorkedOn => "worked_on",
        ArtifactTaskRelation::Mentioned => "mentioned",
        ArtifactTaskRelation::Completed => "completed",
    }
    .to_string()
}

fn extract_task_ids(text: &str) -> BTreeSet<String> {
    let patterns = [
        Regex::new(r"(?i)\btask\s*#?([0-9]+)\b").expect("valid regex"),
        Regex::new(r"(?i)\b(?:tm|aoc-task)\s+(?:status|done|start|resume|show)\s+([0-9]+)\b")
            .expect("valid regex"),
        Regex::new(r"\[([0-9]+)\]").expect("valid regex"),
    ];

    let mut task_ids = BTreeSet::new();
    for pattern in patterns {
        for captures in pattern.captures_iter(text) {
            if let Some(task_id) = captures.get(1) {
                task_ids.insert(task_id.as_str().to_string());
            }
        }
    }

    task_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_core::mind_contracts::{
        compact_raw_event_to_t0, ConversationRole, MessageEvent, RawEvent, RawEventBody,
        T0CompactionPolicy,
    };
    use chrono::TimeZone;
    use tempfile::NamedTempFile;

    fn ts(hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 23, hour, min, sec)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn backfills_worked_on_when_completion_arrives_late() {
        let db_file = NamedTempFile::new().expect("temp db");
        let store = MindStore::open(db_file.path()).expect("open store");

        store
            .append_context_state(&ConversationContextState {
                conversation_id: "conv-1".to_string(),
                ts: ts(12, 0, 0),
                active_tag: Some("mind".to_string()),
                active_tasks: vec!["101".to_string()],
                lifecycle: Some("in-progress".to_string()),
                signal_task_ids: vec!["101".to_string()],
                signal_source: "task_lifecycle_command".to_string(),
            })
            .expect("append active");

        store
            .insert_observation(
                "obs-1",
                "conv-1",
                ts(12, 1, 0),
                "working on parser state machine",
                &[],
            )
            .expect("insert artifact");

        store
            .append_context_state(&ConversationContextState {
                conversation_id: "conv-1".to_string(),
                ts: ts(12, 2, 0),
                active_tag: Some("mind".to_string()),
                active_tasks: Vec::new(),
                lifecycle: Some("done".to_string()),
                signal_task_ids: vec!["101".to_string()],
                signal_source: "task_lifecycle_command".to_string(),
            })
            .expect("append done");

        let engine = TaskAttributionEngine::new(AttributionConfig::default());
        let report = engine
            .attribute_conversation(&store, "conv-1")
            .expect("attribute");
        assert_eq!(report.artifacts_processed, 1);
        assert!(report.links_written >= 2);
        assert!(report.backfilled_links >= 1);

        let links = store
            .artifact_task_links_for_artifact("obs-1")
            .expect("load links");
        assert!(links.iter().any(|link| {
            link.task_id == "101"
                && link.relation == ArtifactTaskRelation::WorkedOn
                && link.confidence_bps >= CONF_BACKFILL_BPS
        }));
        assert!(links.iter().any(|link| {
            link.task_id == "101" && link.relation == ArtifactTaskRelation::Active
        }));
    }

    #[test]
    fn uses_t0_and_artifact_mentions_for_task_links() {
        let db_file = NamedTempFile::new().expect("temp db");
        let store = MindStore::open(db_file.path()).expect("open store");

        let raw = RawEvent {
            event_id: "evt-1".to_string(),
            conversation_id: "conv-2".to_string(),
            agent_id: "agent-1".to_string(),
            ts: ts(13, 0, 0),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::Assistant,
                text: "I will fix task 102 before lunch".to_string(),
            }),
            attrs: Default::default(),
        };
        let compact = compact_raw_event_to_t0(&raw, &T0CompactionPolicy::default())
            .expect("compact")
            .expect("kept");
        store
            .upsert_t0_compact_event(&compact)
            .expect("insert compact");

        store
            .insert_reflection(
                "ref-1",
                "conv-2",
                ts(13, 1, 0),
                "summary: task 102 regression and mitigation plan",
                &[],
            )
            .expect("insert reflection");

        let engine = TaskAttributionEngine::new(AttributionConfig::default());
        let report = engine
            .attribute_conversation(&store, "conv-2")
            .expect("attribute");
        assert_eq!(report.artifacts_processed, 1);
        assert!(report.links_written >= 2);

        let links = store
            .artifact_task_links_for_artifact("ref-1")
            .expect("load links");
        assert!(links.iter().any(|link| {
            link.task_id == "102" && link.relation == ArtifactTaskRelation::Mentioned
        }));
        assert!(links.iter().any(|link| {
            link.task_id == "102" && link.relation == ArtifactTaskRelation::WorkedOn
        }));
    }
}
