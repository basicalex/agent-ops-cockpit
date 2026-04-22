use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const MIND_PROVENANCE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MindProvenanceNodeKind {
    ProjectScope,
    Session,
    Conversation,
    Artifact,
    SemanticProvenance,
    CompactionCheckpoint,
    CompactionT0Slice,
    CanonEntryRevision,
    HandshakeSnapshot,
    T3BacklogJob,
    File,
    Task,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MindProvenanceEdgeKind {
    ScopeSession,
    ScopeHandshake,
    ScopeBacklogJob,
    SessionConversation,
    ConversationParent,
    ConversationRoot,
    ConversationArtifact,
    ConversationCheckpoint,
    ArtifactTrace,
    ArtifactSemanticProvenance,
    ArtifactFileLink,
    ArtifactTaskLink,
    CheckpointSlice,
    SliceFileRead,
    SliceFileModified,
    CanonSupersedes,
    CanonEvidence,
    HandshakeCanon,
    BacklogJobArtifact,
    BacklogJobCanon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindProvenanceNode {
    pub node_id: String,
    pub kind: MindProvenanceNodeKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default)]
    pub attrs: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindProvenanceEdge {
    pub edge_id: String,
    pub kind: MindProvenanceEdgeKind,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub attrs: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MindProvenanceQueryRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canon_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tag: Option<String>,
    #[serde(default)]
    pub include_stale_canon: bool,
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
    #[serde(default = "default_max_edges")]
    pub max_edges: usize,
}

fn default_max_nodes() -> usize {
    64
}

fn default_max_edges() -> usize {
    128
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MindProvenanceQueryResult {
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub seed_refs: Vec<String>,
    #[serde(default)]
    pub nodes: Vec<MindProvenanceNode>,
    #[serde(default)]
    pub edges: Vec<MindProvenanceEdge>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum MindProvenanceCommand {
    Query(MindProvenanceQueryRequest),
}

impl MindProvenanceCommand {
    pub fn parse(command: &str, args: serde_json::Value) -> Result<Self, String> {
        match command {
            "mind_provenance_query" => serde_json::from_value(args)
                .map(Self::Query)
                .map_err(|err| format!("invalid mind_provenance_query args: {err}")),
            other => Err(format!("unsupported provenance command: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindProvenanceExport {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub request: MindProvenanceQueryRequest,
    pub graph: MindProvenanceQueryResult,
    pub mission_control: MindProvenanceMissionControlView,
}

impl MindProvenanceExport {
    pub fn new(request: MindProvenanceQueryRequest, graph: MindProvenanceQueryResult) -> Self {
        let mission_control = MindProvenanceMissionControlView::from_graph(&graph);
        Self {
            schema_version: MIND_PROVENANCE_SCHEMA_VERSION,
            request,
            graph,
            mission_control,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MindProvenanceMissionControlView {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub summary: String,
    #[serde(default)]
    pub seed_refs: Vec<String>,
    #[serde(default)]
    pub focus_node_ids: Vec<String>,
    pub node_count: usize,
    pub edge_count: usize,
    #[serde(default)]
    pub truncated: bool,
}

impl MindProvenanceMissionControlView {
    pub fn from_graph(graph: &MindProvenanceQueryResult) -> Self {
        let focus_node_ids = graph
            .nodes
            .iter()
            .filter(|node| {
                graph.seed_refs.iter().any(|seed| {
                    node.node_id == *seed
                        || node.reference.as_deref() == Some(seed.as_str())
                        || seed
                            .strip_prefix("project:")
                            .map(|value| node.reference.as_deref() == Some(value))
                            .unwrap_or(false)
                        || seed
                            .strip_prefix("session:")
                            .map(|value| node.reference.as_deref() == Some(value))
                            .unwrap_or(false)
                        || seed
                            .strip_prefix("conversation:")
                            .map(|value| node.reference.as_deref() == Some(value))
                            .unwrap_or(false)
                        || seed
                            .strip_prefix("artifact:")
                            .map(|value| node.reference.as_deref() == Some(value))
                            .unwrap_or(false)
                        || seed
                            .strip_prefix("checkpoint:")
                            .map(|value| node.reference.as_deref() == Some(value))
                            .unwrap_or(false)
                        || seed
                            .strip_prefix("canon:")
                            .map(|value| {
                                node.reference
                                    .as_deref()
                                    .map(|reference| reference.starts_with(value))
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                        || seed
                            .strip_prefix("task:")
                            .map(|value| node.reference.as_deref() == Some(value))
                            .unwrap_or(false)
                        || seed
                            .strip_prefix("file:")
                            .map(|value| node.reference.as_deref() == Some(value))
                            .unwrap_or(false)
                })
            })
            .map(|node| node.node_id.clone())
            .collect::<Vec<_>>();

        Self {
            schema_version: MIND_PROVENANCE_SCHEMA_VERSION,
            summary: graph.summary.clone(),
            seed_refs: graph.seed_refs.clone(),
            focus_node_ids,
            node_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
            truncated: graph.truncated,
        }
    }
}

fn default_schema_version() -> u16 {
    MIND_PROVENANCE_SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_request_defaults_are_bounded() {
        let request: MindProvenanceQueryRequest =
            serde_json::from_value(serde_json::json!({})).expect("request parse");
        assert_eq!(request.max_nodes, 64);
        assert_eq!(request.max_edges, 128);
        assert!(!request.include_stale_canon);
    }

    #[test]
    fn provenance_result_round_trips_graph_shapes() {
        let result = MindProvenanceQueryResult {
            status: "ok".to_string(),
            summary: "project scope and session lineage".to_string(),
            seed_refs: vec!["conversation:conv-1".to_string()],
            nodes: vec![MindProvenanceNode {
                node_id: "conversation:conv-1".to_string(),
                kind: MindProvenanceNodeKind::Conversation,
                label: "Conversation conv-1".to_string(),
                reference: Some("conv-1".to_string()),
                attrs: BTreeMap::new(),
            }],
            edges: vec![MindProvenanceEdge {
                edge_id: "edge:1".to_string(),
                kind: MindProvenanceEdgeKind::SessionConversation,
                from: "session:s1".to_string(),
                to: "conversation:conv-1".to_string(),
                label: Some("contains".to_string()),
                attrs: BTreeMap::new(),
            }],
            truncated: false,
        };

        let value = serde_json::to_value(&result).expect("serialize");
        let parsed: MindProvenanceQueryResult = serde_json::from_value(value).expect("parse");
        assert_eq!(parsed, result);
    }

    #[test]
    fn parse_provenance_query_defaults_bounds() {
        let command = MindProvenanceCommand::parse(
            "mind_provenance_query",
            serde_json::json!({"conversation_id": "conv-1", "task_id": "141", "file_path": "docs/mission-control.md"}),
        )
        .expect("parse provenance query");

        let MindProvenanceCommand::Query(request) = command;
        assert_eq!(request.conversation_id.as_deref(), Some("conv-1"));
        assert_eq!(request.task_id.as_deref(), Some("141"));
        assert_eq!(
            request.file_path.as_deref(),
            Some("docs/mission-control.md")
        );
        assert_eq!(request.max_nodes, 64);
        assert_eq!(request.max_edges, 128);
    }

    #[test]
    fn provenance_export_derives_mission_control_view() {
        let graph = MindProvenanceQueryResult {
            status: "ok".to_string(),
            summary: "graph ready".to_string(),
            seed_refs: vec![
                "conversation:conv-1".to_string(),
                "artifact:obs:1".to_string(),
                "task:141".to_string(),
                "file:docs/mission-control.md".to_string(),
            ],
            nodes: vec![
                MindProvenanceNode {
                    node_id: "conversation:conv-1".to_string(),
                    kind: MindProvenanceNodeKind::Conversation,
                    label: "Conversation conv-1".to_string(),
                    reference: Some("conv-1".to_string()),
                    attrs: BTreeMap::new(),
                },
                MindProvenanceNode {
                    node_id: "artifact:obs:1".to_string(),
                    kind: MindProvenanceNodeKind::Artifact,
                    label: "obs:1".to_string(),
                    reference: Some("obs:1".to_string()),
                    attrs: BTreeMap::new(),
                },
                MindProvenanceNode {
                    node_id: "task:141".to_string(),
                    kind: MindProvenanceNodeKind::Task,
                    label: "141".to_string(),
                    reference: Some("141".to_string()),
                    attrs: BTreeMap::new(),
                },
                MindProvenanceNode {
                    node_id: "file:docs/mission-control.md".to_string(),
                    kind: MindProvenanceNodeKind::File,
                    label: "docs/mission-control.md".to_string(),
                    reference: Some("docs/mission-control.md".to_string()),
                    attrs: BTreeMap::new(),
                },
            ],
            edges: vec![],
            truncated: true,
        };

        let export = MindProvenanceExport::new(
            MindProvenanceQueryRequest {
                conversation_id: Some("conv-1".to_string()),
                ..Default::default()
            },
            graph,
        );

        assert_eq!(export.schema_version, MIND_PROVENANCE_SCHEMA_VERSION);
        assert_eq!(export.mission_control.node_count, 4);
        assert_eq!(export.mission_control.edge_count, 0);
        assert!(export.mission_control.truncated);
        assert!(export
            .mission_control
            .focus_node_ids
            .contains(&"conversation:conv-1".to_string()));
        assert!(export
            .mission_control
            .focus_node_ids
            .contains(&"artifact:obs:1".to_string()));
        assert!(export
            .mission_control
            .focus_node_ids
            .contains(&"task:141".to_string()));
        assert!(export
            .mission_control
            .focus_node_ids
            .contains(&"file:docs/mission-control.md".to_string()));
    }
}
