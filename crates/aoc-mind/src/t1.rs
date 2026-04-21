use crate::ingest::mind_progress_for_conversation;
use aoc_core::mind_observer_feed::MindObserverFeedProgress;
use aoc_storage::{MindStore, StorageError};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum T1ThresholdDecision {
    NoProgress,
    BelowTarget {
        progress: MindObserverFeedProgress,
    },
    AlreadySatisfied {
        progress: MindObserverFeedProgress,
    },
    NeedsRun {
        progress: MindObserverFeedProgress,
        reason: String,
    },
}

#[derive(Debug, Error)]
pub enum T1ThresholdError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

pub fn evaluate_t1_token_threshold(
    store: &MindStore,
    conversation_id: &str,
    t1_target_tokens: u32,
    t1_hard_cap_tokens: u32,
) -> Result<T1ThresholdDecision, T1ThresholdError> {
    let Some(progress) = mind_progress_for_conversation(
        store,
        conversation_id,
        t1_target_tokens,
        t1_hard_cap_tokens,
    ) else {
        return Ok(T1ThresholdDecision::NoProgress);
    };

    if progress.t0_estimated_tokens == 0 {
        return Ok(T1ThresholdDecision::NoProgress);
    }

    if progress.t0_estimated_tokens < t1_target_tokens {
        return Ok(T1ThresholdDecision::BelowTarget { progress });
    }

    if store.conversation_needs_observer_run(conversation_id)? {
        return Ok(T1ThresholdDecision::NeedsRun {
            progress,
            reason: "t0 target reached".to_string(),
        });
    }

    Ok(T1ThresholdDecision::AlreadySatisfied { progress })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_core::mind_contracts::{
        compact_raw_event_to_t0, ConversationRole, MessageEvent, RawEvent, RawEventBody,
        T0CompactionPolicy,
    };
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn raw(event_id: &str, text: &str) -> RawEvent {
        RawEvent {
            event_id: event_id.to_string(),
            conversation_id: "conv-t1".to_string(),
            agent_id: "agent-1".to_string(),
            ts: Utc::now(),
            body: RawEventBody::Message(MessageEvent {
                role: ConversationRole::Assistant,
                text: text.to_string(),
            }),
            attrs: BTreeMap::new(),
        }
    }

    fn insert_t0(store: &MindStore, event_id: &str, text: &str) {
        let raw = raw(event_id, text);
        store.insert_raw_event(&raw).expect("raw");
        let compact = compact_raw_event_to_t0(&raw, &T0CompactionPolicy::default())
            .expect("compact")
            .expect("compact event");
        store
            .upsert_t0_compact_event(&compact)
            .expect("upsert compact");
    }

    #[test]
    fn threshold_returns_no_progress_for_empty_conversation() {
        let store = MindStore::open_in_memory().expect("store");
        let decision = evaluate_t1_token_threshold(&store, "conv-t1", 10, 20).expect("decision");
        assert_eq!(decision, T1ThresholdDecision::NoProgress);
    }

    #[test]
    fn threshold_returns_below_target_when_tokens_are_insufficient() {
        let store = MindStore::open_in_memory().expect("store");
        insert_t0(&store, "evt-1", "short");
        let decision = evaluate_t1_token_threshold(&store, "conv-t1", 100, 120).expect("decision");
        match decision {
            T1ThresholdDecision::BelowTarget { progress } => {
                assert!(progress.t0_estimated_tokens < 100);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn threshold_returns_needs_run_when_target_is_reached_and_no_observer_run_exists() {
        let store = MindStore::open_in_memory().expect("store");
        insert_t0(&store, "evt-1", &"x".repeat(200));
        let decision = evaluate_t1_token_threshold(&store, "conv-t1", 10, 20).expect("decision");
        match decision {
            T1ThresholdDecision::NeedsRun { progress, reason } => {
                assert!(progress.t0_estimated_tokens >= 10);
                assert_eq!(reason, "t0 target reached");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn threshold_returns_already_satisfied_after_observation_exists() {
        let store = MindStore::open_in_memory().expect("store");
        insert_t0(&store, "evt-1", &"x".repeat(200));
        store
            .insert_observation(
                "obs-1",
                "conv-t1",
                Utc::now(),
                "observer output",
                &["evt-1".to_string()],
            )
            .expect("insert observation");
        let decision = evaluate_t1_token_threshold(&store, "conv-t1", 10, 20).expect("decision");
        match decision {
            T1ThresholdDecision::AlreadySatisfied { progress } => {
                assert!(progress.t0_estimated_tokens >= 10);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }
}
