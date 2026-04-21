use aoc_core::{
    mind_contracts::{
        compact_raw_event_to_t0, sanitize_raw_event_for_storage, MindContractError, RawEvent,
        T0CompactionPolicy,
    },
    mind_observer_feed::MindObserverFeedProgress,
};
use aoc_storage::{MindStore, StorageError, StoredCompactEvent};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T0IngestConfig {
    pub policy: T0CompactionPolicy,
    pub t1_target_tokens: u32,
    pub t1_hard_cap_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T0IngestReport {
    pub inserted_raw: bool,
    pub produced_compact: bool,
    pub progress: MindObserverFeedProgress,
}

#[derive(Debug, Error)]
pub enum T0IngestError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
}

pub fn ingest_raw_event(
    store: &MindStore,
    raw: &RawEvent,
    config: &T0IngestConfig,
) -> Result<T0IngestReport, T0IngestError> {
    let raw = sanitize_raw_event_for_storage(raw);
    let inserted_raw = store.insert_raw_event(&raw)?;
    let produced_compact = if let Some(compact) = compact_raw_event_to_t0(&raw, &config.policy)? {
        store.upsert_t0_compact_event(&compact)?;
        true
    } else {
        false
    };
    let progress = mind_progress_for_conversation(
        store,
        &raw.conversation_id,
        config.t1_target_tokens,
        config.t1_hard_cap_tokens,
    )
    .ok_or_else(|| StorageError::Serialization("mind progress unavailable".to_string()))?;

    Ok(T0IngestReport {
        inserted_raw,
        produced_compact,
        progress,
    })
}

pub fn mind_progress_for_conversation(
    store: &MindStore,
    conversation_id: &str,
    t1_target_tokens: u32,
    t1_hard_cap_tokens: u32,
) -> Option<MindObserverFeedProgress> {
    let t0_events = store.t0_events_for_conversation(conversation_id).ok()?;
    let t0_estimated_tokens = t0_events.iter().fold(0_u32, |total, event| {
        total.saturating_add(estimate_compact_tokens(event))
    });
    Some(MindObserverFeedProgress {
        t0_estimated_tokens,
        t1_target_tokens,
        t1_hard_cap_tokens,
        tokens_until_next_run: t1_target_tokens.saturating_sub(t0_estimated_tokens),
    })
}

pub fn estimate_compact_tokens(event: &StoredCompactEvent) -> u32 {
    let text_tokens = event
        .text
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or_default();
    let tool_tokens = event
        .tool_meta
        .as_ref()
        .map(|meta| 14 + ((meta.output_bytes as u32) / 180))
        .unwrap_or_default();
    (text_tokens + tool_tokens).max(1)
}

fn estimate_text_tokens(text: &str) -> u32 {
    (text.chars().count() as u32 / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_core::mind_contracts::{ConversationRole, MessageEvent, RawEventBody};
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn sample_raw_event() -> RawEvent {
        RawEvent {
            event_id: "evt-1".to_string(),
            conversation_id: "conv-1".to_string(),
            agent_id: "agent-1".to_string(),
            ts: Utc::now(),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::Assistant,
                text: "hello from t0 ingest".to_string(),
            }),
            attrs: BTreeMap::new(),
        }
    }

    #[test]
    fn ingest_raw_event_persists_raw_and_compact_and_reports_progress() {
        let store = MindStore::open_in_memory().expect("store");
        let report = ingest_raw_event(
            &store,
            &sample_raw_event(),
            &T0IngestConfig {
                policy: T0CompactionPolicy::default(),
                t1_target_tokens: 100,
                t1_hard_cap_tokens: 120,
            },
        )
        .expect("ingest");

        assert!(report.inserted_raw);
        assert!(report.produced_compact);
        assert!(report.progress.t0_estimated_tokens >= 1);
        assert_eq!(report.progress.t1_target_tokens, 100);
        assert_eq!(
            store
                .raw_event_by_id("evt-1")
                .expect("raw query")
                .expect("raw present")
                .conversation_id,
            "conv-1"
        );
        assert_eq!(
            store
                .t0_events_for_conversation("conv-1")
                .expect("compact query")
                .len(),
            1
        );
    }

    #[test]
    fn progress_for_conversation_sums_compact_tokens() {
        let store = MindStore::open_in_memory().expect("store");
        let _ = ingest_raw_event(
            &store,
            &sample_raw_event(),
            &T0IngestConfig {
                policy: T0CompactionPolicy::default(),
                t1_target_tokens: 10,
                t1_hard_cap_tokens: 20,
            },
        )
        .expect("ingest");
        let progress = mind_progress_for_conversation(&store, "conv-1", 10, 20).expect("progress");
        assert!(progress.t0_estimated_tokens >= 1);
        assert_eq!(progress.t1_hard_cap_tokens, 20);
    }
}
