use serde::{Deserialize, Serialize};

pub const CONSULTATION_PACKET_SCHEMA_VERSION: u16 = 1;
pub const CONSULTATION_PACKET_MAX_SUMMARY_CHARS: usize = 480;
pub const CONSULTATION_PACKET_MAX_PLAN_ITEMS: usize = 8;
pub const CONSULTATION_PACKET_MAX_PLAN_ITEM_SUMMARY_CHARS: usize = 160;
pub const CONSULTATION_PACKET_MAX_BLOCKERS: usize = 4;
pub const CONSULTATION_PACKET_MAX_BLOCKER_SUMMARY_CHARS: usize = 200;
pub const CONSULTATION_PACKET_MAX_EVIDENCE_REFS: usize = 12;
pub const CONSULTATION_PACKET_MAX_EVIDENCE_LABEL_CHARS: usize = 96;
pub const CONSULTATION_PACKET_MAX_HELP_REQUEST_CHARS: usize = 220;
pub const CONSULTATION_PACKET_MAX_DEGRADED_INPUTS: usize = 8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConsultationPacketKind {
    #[default]
    Summary,
    Plan,
    Blockers,
    Review,
    Align,
    CheckpointStatus,
    HelpRequest,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConsultationSourceStatus {
    #[default]
    Complete,
    Partial,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationIdentity {
    pub session_id: String,
    pub agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationTaskContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tag: Option<String>,
    #[serde(default)]
    pub task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationPlanItem {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationBlocker {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationCheckpointRef {
    pub checkpoint_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationArtifactRef {
    pub artifact_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationEvidenceRef {
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationFreshness {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_after_ms: Option<u64>,
    #[serde(default)]
    pub source_status: ConsultationSourceStatus,
    #[serde(default)]
    pub degraded_inputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationConfidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationHelpRequest {
    pub kind: String,
    pub question: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urgency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConsultationPacket {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub packet_id: String,
    #[serde(default)]
    pub kind: ConsultationPacketKind,
    pub identity: ConsultationIdentity,
    #[serde(default)]
    pub task_context: ConsultationTaskContext,
    #[serde(default)]
    pub current_plan: Vec<ConsultationPlanItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub blockers: Vec<ConsultationBlocker>,
    #[serde(default)]
    pub checkpoint: Option<ConsultationCheckpointRef>,
    #[serde(default)]
    pub artifact_refs: Vec<ConsultationArtifactRef>,
    #[serde(default)]
    pub evidence_refs: Vec<ConsultationEvidenceRef>,
    #[serde(default)]
    pub freshness: ConsultationFreshness,
    #[serde(default)]
    pub confidence: ConsultationConfidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help_request: Option<ConsultationHelpRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

impl ConsultationPacket {
    pub fn normalize(mut self) -> Self {
        if self.schema_version == 0 {
            self.schema_version = CONSULTATION_PACKET_SCHEMA_VERSION;
        }

        self.summary = self
            .summary
            .take()
            .map(|value| truncate_chars(value.trim(), CONSULTATION_PACKET_MAX_SUMMARY_CHARS))
            .filter(|value| !value.is_empty());

        self.task_context.focus_summary = self
            .task_context
            .focus_summary
            .take()
            .map(|value| {
                truncate_chars(
                    value.trim(),
                    CONSULTATION_PACKET_MAX_PLAN_ITEM_SUMMARY_CHARS,
                )
            })
            .filter(|value| !value.is_empty());

        self.task_context
            .task_ids
            .retain(|value| !value.trim().is_empty());

        self.current_plan
            .truncate(CONSULTATION_PACKET_MAX_PLAN_ITEMS);
        for item in &mut self.current_plan {
            item.title = truncate_chars(
                item.title.trim(),
                CONSULTATION_PACKET_MAX_PLAN_ITEM_SUMMARY_CHARS,
            );
            item.summary = item
                .summary
                .take()
                .map(|value| {
                    truncate_chars(
                        value.trim(),
                        CONSULTATION_PACKET_MAX_PLAN_ITEM_SUMMARY_CHARS,
                    )
                })
                .filter(|value| !value.is_empty());
            item.evidence_refs =
                normalize_string_refs(&item.evidence_refs, CONSULTATION_PACKET_MAX_EVIDENCE_REFS);
        }
        self.current_plan.retain(|item| !item.title.is_empty());

        self.blockers.truncate(CONSULTATION_PACKET_MAX_BLOCKERS);
        for blocker in &mut self.blockers {
            blocker.summary = truncate_chars(
                blocker.summary.trim(),
                CONSULTATION_PACKET_MAX_BLOCKER_SUMMARY_CHARS,
            );
            blocker.evidence_refs = normalize_string_refs(
                &blocker.evidence_refs,
                CONSULTATION_PACKET_MAX_EVIDENCE_REFS,
            );
        }
        self.blockers.retain(|blocker| !blocker.summary.is_empty());

        self.artifact_refs
            .retain(|artifact| !artifact.artifact_id.trim().is_empty());

        self.evidence_refs
            .truncate(CONSULTATION_PACKET_MAX_EVIDENCE_REFS);
        for evidence in &mut self.evidence_refs {
            evidence.reference = truncate_chars(
                evidence.reference.trim(),
                CONSULTATION_PACKET_MAX_EVIDENCE_LABEL_CHARS,
            );
            evidence.label = evidence
                .label
                .take()
                .map(|value| {
                    truncate_chars(value.trim(), CONSULTATION_PACKET_MAX_EVIDENCE_LABEL_CHARS)
                })
                .filter(|value| !value.is_empty());
        }
        self.evidence_refs
            .retain(|evidence| !evidence.reference.trim().is_empty());

        self.freshness.degraded_inputs = normalize_string_refs(
            &self.freshness.degraded_inputs,
            CONSULTATION_PACKET_MAX_DEGRADED_INPUTS,
        );

        self.confidence.rationale = self
            .confidence
            .rationale
            .take()
            .map(|value| {
                truncate_chars(
                    value.trim(),
                    CONSULTATION_PACKET_MAX_PLAN_ITEM_SUMMARY_CHARS,
                )
            })
            .filter(|value| !value.is_empty());

        if let Some(help_request) = self.help_request.as_mut() {
            help_request.kind = truncate_chars(
                help_request.kind.trim(),
                CONSULTATION_PACKET_MAX_EVIDENCE_LABEL_CHARS,
            );
            help_request.question = truncate_chars(
                help_request.question.trim(),
                CONSULTATION_PACKET_MAX_HELP_REQUEST_CHARS,
            );
            if help_request.kind.is_empty() || help_request.question.is_empty() {
                self.help_request = None;
            }
        }

        self.degraded_reason = self
            .degraded_reason
            .take()
            .map(|value| {
                truncate_chars(
                    value.trim(),
                    CONSULTATION_PACKET_MAX_PLAN_ITEM_SUMMARY_CHARS,
                )
            })
            .filter(|value| !value.is_empty());

        self
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded_reason.is_some() || !self.freshness.degraded_inputs.is_empty()
    }
}

fn default_schema_version() -> u16 {
    CONSULTATION_PACKET_SCHEMA_VERSION
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn normalize_string_refs(values: &[String], max_items: usize) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .take(max_items)
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long_text(prefix: &str, count: usize) -> String {
        let mut value = String::new();
        value.push_str(prefix);
        value.push(':');
        value.push_str(&"x".repeat(count));
        value
    }

    #[test]
    fn consultation_packet_json_round_trip_uses_schema_defaults() {
        let packet = ConsultationPacket {
            schema_version: 0,
            packet_id: "pkt-1".to_string(),
            identity: ConsultationIdentity {
                session_id: "sess-1".to_string(),
                agent_id: "sess-1::pane-3".to_string(),
                conversation_id: Some("pi:sess-1".to_string()),
                ..Default::default()
            },
            task_context: ConsultationTaskContext {
                active_tag: Some("mind".to_string()),
                task_ids: vec!["156".to_string()],
                focus_summary: Some("define packet".to_string()),
            },
            summary: Some("bounded summary".to_string()),
            checkpoint: Some(ConsultationCheckpointRef {
                checkpoint_id: "cmpchk:1".to_string(),
                conversation_id: Some("pi:sess-1".to_string()),
                ..Default::default()
            }),
            freshness: ConsultationFreshness {
                packet_generated_at: Some("2026-03-09T10:00:00Z".to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
        .normalize();

        let json = serde_json::to_value(&packet).expect("serialize packet");
        assert_eq!(json["schema_version"], CONSULTATION_PACKET_SCHEMA_VERSION);
        let round_trip: ConsultationPacket =
            serde_json::from_value(json).expect("deserialize packet");
        assert_eq!(round_trip.packet_id, "pkt-1");
        assert_eq!(round_trip.summary.as_deref(), Some("bounded summary"));
        assert_eq!(
            round_trip.identity.conversation_id.as_deref(),
            Some("pi:sess-1")
        );
    }

    #[test]
    fn consultation_packet_normalize_truncates_and_caps_payloads() {
        let packet = ConsultationPacket {
            schema_version: 0,
            packet_id: "pkt-2".to_string(),
            kind: ConsultationPacketKind::Review,
            identity: ConsultationIdentity {
                session_id: "sess-1".to_string(),
                agent_id: "sess-1::pane-5".to_string(),
                ..Default::default()
            },
            task_context: ConsultationTaskContext {
                task_ids: vec!["156".to_string(), "  ".to_string(), "157".to_string()],
                focus_summary: Some(long_text("focus", 400)),
                ..Default::default()
            },
            current_plan: (0..12)
                .map(|index| ConsultationPlanItem {
                    title: format!("step-{index}-{}", "y".repeat(220)),
                    summary: Some(long_text("step", 320)),
                    evidence_refs: vec!["ev:1".to_string(), " ".to_string(), "ev:2".to_string()],
                    ..Default::default()
                })
                .collect(),
            summary: Some(long_text("summary", 1000)),
            blockers: (0..7)
                .map(|index| ConsultationBlocker {
                    summary: long_text(&format!("blocker-{index}"), 400),
                    evidence_refs: vec!["ev:blocker".to_string(); 16],
                    ..Default::default()
                })
                .collect(),
            evidence_refs: (0..20)
                .map(|index| ConsultationEvidenceRef {
                    reference: format!("ref-{index}-{}", "z".repeat(200)),
                    label: Some(long_text("label", 200)),
                    ..Default::default()
                })
                .collect(),
            help_request: Some(ConsultationHelpRequest {
                kind: "review".to_string(),
                question: long_text("question", 600),
                ..Default::default()
            }),
            ..Default::default()
        }
        .normalize();

        assert_eq!(packet.schema_version, CONSULTATION_PACKET_SCHEMA_VERSION);
        assert_eq!(
            packet.current_plan.len(),
            CONSULTATION_PACKET_MAX_PLAN_ITEMS
        );
        assert_eq!(packet.blockers.len(), CONSULTATION_PACKET_MAX_BLOCKERS);
        assert_eq!(
            packet.evidence_refs.len(),
            CONSULTATION_PACKET_MAX_EVIDENCE_REFS
        );
        assert_eq!(
            packet.summary.as_ref().unwrap().chars().count(),
            CONSULTATION_PACKET_MAX_SUMMARY_CHARS
        );
        assert_eq!(packet.task_context.task_ids, vec!["156", "157"]);
        assert!(packet
            .current_plan
            .iter()
            .all(|item| item.title.chars().count()
                <= CONSULTATION_PACKET_MAX_PLAN_ITEM_SUMMARY_CHARS));
        assert!(packet
            .blockers
            .iter()
            .all(|item| item.summary.chars().count()
                <= CONSULTATION_PACKET_MAX_BLOCKER_SUMMARY_CHARS));
        assert!(packet
            .evidence_refs
            .iter()
            .all(|item| item.reference.chars().count()
                <= CONSULTATION_PACKET_MAX_EVIDENCE_LABEL_CHARS));
        assert!(packet.help_request.is_some());
        assert!(
            packet
                .help_request
                .as_ref()
                .unwrap()
                .question
                .chars()
                .count()
                <= CONSULTATION_PACKET_MAX_HELP_REQUEST_CHARS
        );
    }

    #[test]
    fn consultation_packet_marks_degraded_inputs_without_failing() {
        let packet = ConsultationPacket {
            schema_version: 0,
            packet_id: "pkt-3".to_string(),
            identity: ConsultationIdentity {
                session_id: "sess-2".to_string(),
                agent_id: "sess-2::pane-1".to_string(),
                ..Default::default()
            },
            freshness: ConsultationFreshness {
                source_status: ConsultationSourceStatus::Partial,
                degraded_inputs: vec![
                    "mind.t2".to_string(),
                    "pulse.timeline".to_string(),
                    " ".to_string(),
                ],
                ..Default::default()
            },
            degraded_reason: Some(
                "Mind unavailable; packet derived from checkpoint + runtime state".to_string(),
            ),
            ..Default::default()
        }
        .normalize();

        assert!(packet.is_degraded());
        assert_eq!(
            packet.freshness.degraded_inputs,
            vec!["mind.t2", "pulse.timeline"]
        );
        assert_eq!(
            packet.freshness.source_status,
            ConsultationSourceStatus::Partial
        );
        assert_eq!(
            packet.degraded_reason.as_deref(),
            Some("Mind unavailable; packet derived from checkpoint + runtime state")
        );
    }

    #[test]
    fn consultation_packet_drops_invalid_help_request() {
        let packet = ConsultationPacket {
            schema_version: 0,
            packet_id: "pkt-4".to_string(),
            identity: ConsultationIdentity {
                session_id: "sess-3".to_string(),
                agent_id: "sess-3::pane-9".to_string(),
                ..Default::default()
            },
            help_request: Some(ConsultationHelpRequest {
                kind: " ".to_string(),
                question: " ".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }
        .normalize();

        assert!(packet.help_request.is_none());
    }
}
