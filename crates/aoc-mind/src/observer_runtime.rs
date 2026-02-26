use chrono::{DateTime, Duration, Utc};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserverQueueConfig {
    pub debounce_ms: u64,
}

impl Default for ObserverQueueConfig {
    fn default() -> Self {
        Self { debounce_ms: 250 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObserverTriggerPriority {
    Normal,
    Elevated,
    Urgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObserverTriggerKind {
    TokenThreshold,
    TaskCompleted,
    ManualShortcut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserverTrigger {
    pub kind: ObserverTriggerKind,
    pub priority: ObserverTriggerPriority,
    pub bypass_debounce: bool,
}

impl ObserverTrigger {
    pub fn token_threshold() -> Self {
        Self {
            kind: ObserverTriggerKind::TokenThreshold,
            priority: ObserverTriggerPriority::Normal,
            bypass_debounce: false,
        }
    }

    pub fn task_completed() -> Self {
        Self {
            kind: ObserverTriggerKind::TaskCompleted,
            priority: ObserverTriggerPriority::Elevated,
            bypass_debounce: false,
        }
    }

    pub fn manual_shortcut() -> Self {
        Self {
            kind: ObserverTriggerKind::ManualShortcut,
            priority: ObserverTriggerPriority::Urgent,
            bypass_debounce: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedObserverRun {
    pub session_id: String,
    pub conversation_id: String,
    pub trigger: ObserverTrigger,
    pub enqueued_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingConversation {
    conversation_id: String,
    trigger: ObserverTrigger,
    enqueued_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct SessionQueueState {
    pending: VecDeque<PendingConversation>,
    active_run: bool,
    next_eligible_at: DateTime<Utc>,
}

impl SessionQueueState {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            pending: VecDeque::new(),
            active_run: false,
            next_eligible_at: now,
        }
    }
}

#[derive(Debug, Default)]
pub struct SessionObserverQueue {
    config: ObserverQueueConfig,
    sessions: BTreeMap<String, SessionQueueState>,
}

impl SessionObserverQueue {
    pub fn new(config: ObserverQueueConfig) -> Self {
        Self {
            config,
            sessions: BTreeMap::new(),
        }
    }

    pub fn enqueue(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        now: DateTime<Utc>,
    ) {
        self.enqueue_with_trigger(
            session_id,
            conversation_id,
            ObserverTrigger::token_threshold(),
            now,
        );
    }

    pub fn enqueue_with_trigger(
        &mut self,
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        trigger: ObserverTrigger,
        now: DateTime<Utc>,
    ) {
        let session_id = session_id.into();
        let conversation_id = conversation_id.into();
        let debounce = self.debounce_duration();

        let state = self
            .sessions
            .entry(session_id)
            .or_insert_with(|| SessionQueueState::new(now));

        if let Some(existing) = state
            .pending
            .iter_mut()
            .find(|pending| pending.conversation_id == conversation_id)
        {
            if trigger.priority > existing.trigger.priority {
                existing.trigger = trigger;
            }
            if trigger.bypass_debounce && !state.active_run {
                state.next_eligible_at = now;
            }
            return;
        }

        let pending = PendingConversation {
            conversation_id,
            trigger,
            enqueued_at: now,
        };

        if trigger.priority == ObserverTriggerPriority::Urgent {
            state.pending.push_front(pending);
        } else {
            state.pending.push_back(pending);
        }

        if !state.active_run {
            state.next_eligible_at = if trigger.bypass_debounce {
                now
            } else {
                now + debounce
            };
        }
    }

    pub fn claim_ready(&mut self, now: DateTime<Utc>) -> Option<ClaimedObserverRun> {
        let mut selected_session: Option<String> = None;
        let mut selected_priority = ObserverTriggerPriority::Normal;
        let mut selected_eligible_at: Option<DateTime<Utc>> = None;
        let mut selected_enqueued_at: Option<DateTime<Utc>> = None;

        for (session_id, state) in &self.sessions {
            if state.active_run || state.pending.is_empty() || state.next_eligible_at > now {
                continue;
            }

            let pending = state
                .pending
                .front()
                .expect("pending queue has at least one item");

            let should_select = if selected_session.is_none() {
                true
            } else if pending.trigger.priority > selected_priority {
                true
            } else if pending.trigger.priority < selected_priority {
                false
            } else if state.next_eligible_at
                < selected_eligible_at.expect("selected session has eligible_at")
            {
                true
            } else if state.next_eligible_at
                > selected_eligible_at.expect("selected session has eligible_at")
            {
                false
            } else {
                pending.enqueued_at
                    < selected_enqueued_at.expect("selected session has enqueued_at")
            };

            if should_select {
                selected_priority = pending.trigger.priority;
                selected_eligible_at = Some(state.next_eligible_at);
                selected_enqueued_at = Some(pending.enqueued_at);
                selected_session = Some(session_id.clone());
            }
        }

        let session_id = selected_session?;
        let state = self.sessions.get_mut(&session_id)?;
        let next = state.pending.pop_front()?;
        state.active_run = true;

        Some(ClaimedObserverRun {
            session_id,
            conversation_id: next.conversation_id,
            trigger: next.trigger,
            enqueued_at: next.enqueued_at,
            started_at: now,
        })
    }

    pub fn complete_run(&mut self, run: &ClaimedObserverRun, now: DateTime<Utc>) {
        let debounce = self.debounce_duration();
        let Some(state) = self.sessions.get_mut(&run.session_id) else {
            return;
        };

        state.active_run = false;
        state.next_eligible_at = if let Some(next_pending) = state.pending.front() {
            if next_pending.trigger.bypass_debounce {
                now
            } else {
                now + debounce
            }
        } else {
            now
        };
    }

    pub fn pending_count(&self, session_id: &str) -> usize {
        self.sessions
            .get(session_id)
            .map(|state| state.pending.len())
            .unwrap_or(0)
    }

    pub fn has_active_run(&self, session_id: &str) -> bool {
        self.sessions
            .get(session_id)
            .map(|state| state.active_run)
            .unwrap_or(false)
    }

    fn debounce_duration(&self) -> Duration {
        Duration::milliseconds(self.config.debounce_ms.min(i64::MAX as u64) as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(1_700_000_000_000 + ms)
            .single()
            .expect("valid test timestamp")
    }

    #[test]
    fn queue_debounces_before_claiming() {
        let mut queue = SessionObserverQueue::new(ObserverQueueConfig { debounce_ms: 200 });

        queue.enqueue("session-a", "conv-1", ts(0));
        queue.enqueue("session-a", "conv-1", ts(50));

        assert!(queue.claim_ready(ts(100)).is_none());

        let claimed = queue.claim_ready(ts(260)).expect("run must be claimable");
        assert_eq!(claimed.session_id, "session-a");
        assert_eq!(claimed.conversation_id, "conv-1");
        assert_eq!(claimed.trigger.kind, ObserverTriggerKind::TokenThreshold);
        assert!(queue.has_active_run("session-a"));
    }

    #[test]
    fn queue_enforces_single_active_run_per_session() {
        let mut queue = SessionObserverQueue::new(ObserverQueueConfig { debounce_ms: 50 });
        queue.enqueue("session-a", "conv-1", ts(0));
        queue.enqueue("session-a", "conv-2", ts(10));

        let first = queue.claim_ready(ts(100)).expect("first run");
        assert_eq!(first.conversation_id, "conv-1");
        assert!(queue.claim_ready(ts(100)).is_none());

        queue.complete_run(&first, ts(120));
        assert!(queue.claim_ready(ts(140)).is_none());

        let second = queue.claim_ready(ts(180)).expect("second run");
        assert_eq!(second.conversation_id, "conv-2");
    }

    #[test]
    fn queue_claims_oldest_eligible_session_first() {
        let mut queue = SessionObserverQueue::new(ObserverQueueConfig { debounce_ms: 100 });

        queue.enqueue("session-b", "conv-b", ts(0));
        queue.enqueue("session-a", "conv-a", ts(20));

        let first = queue.claim_ready(ts(120)).expect("first claim");
        assert_eq!(first.session_id, "session-b");
        queue.complete_run(&first, ts(130));

        let second = queue.claim_ready(ts(180)).expect("second claim");
        assert_eq!(second.session_id, "session-a");
    }

    #[test]
    fn manual_trigger_bypasses_debounce() {
        let mut queue = SessionObserverQueue::new(ObserverQueueConfig { debounce_ms: 500 });
        queue.enqueue_with_trigger(
            "session-a",
            "conv-1",
            ObserverTrigger::manual_shortcut(),
            ts(0),
        );

        let claimed = queue
            .claim_ready(ts(0))
            .expect("manual should claim immediately");
        assert_eq!(claimed.trigger.kind, ObserverTriggerKind::ManualShortcut);
    }

    #[test]
    fn manual_trigger_priority_wins_across_sessions() {
        let mut queue = SessionObserverQueue::new(ObserverQueueConfig { debounce_ms: 100 });
        queue.enqueue("session-a", "conv-a", ts(0));
        queue.enqueue_with_trigger(
            "session-b",
            "conv-b",
            ObserverTrigger::manual_shortcut(),
            ts(10),
        );

        let claimed = queue.claim_ready(ts(110)).expect("one run should be ready");
        assert_eq!(claimed.session_id, "session-b");
        assert_eq!(claimed.trigger.kind, ObserverTriggerKind::ManualShortcut);
    }

    #[test]
    fn task_completed_upgrades_existing_pending_trigger() {
        let mut queue = SessionObserverQueue::new(ObserverQueueConfig { debounce_ms: 100 });
        queue.enqueue("session-a", "conv-1", ts(0));
        queue.enqueue_with_trigger(
            "session-a",
            "conv-1",
            ObserverTrigger::task_completed(),
            ts(10),
        );

        let claimed = queue.claim_ready(ts(110)).expect("run should be ready");
        assert_eq!(claimed.trigger.kind, ObserverTriggerKind::TaskCompleted);
        assert_eq!(claimed.trigger.priority, ObserverTriggerPriority::Elevated);
    }
}
