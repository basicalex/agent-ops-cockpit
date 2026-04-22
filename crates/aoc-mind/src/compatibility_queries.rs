use crate::{load_latest_session_export_manifest, project_scope_key, SessionExportManifest};
use aoc_core::{
    mind_contracts::{compose_context_pack, ContextLayer, ContextPackInput},
    mind_observer_feed::MindInjectionTriggerKind,
    provenance_contracts::{
        MindProvenanceEdge, MindProvenanceEdgeKind, MindProvenanceExport, MindProvenanceNode,
        MindProvenanceNodeKind, MindProvenanceQueryRequest, MindProvenanceQueryResult,
    },
};
use aoc_storage::{CanonRevisionState, CompactionCheckpoint, MindStore, StoredArtifact};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

const MIND_CONTEXT_PACK_SCHEMA_VERSION: u16 = 1;
const MIND_CONTEXT_PACK_COMPACT_MAX_LINES: usize = 24;
const MIND_CONTEXT_PACK_EXPANDED_MAX_LINES: usize = 48;
const MIND_CONTEXT_PACK_COMPACT_SOURCE_MAX_LINES: usize = 5;
const MIND_CONTEXT_PACK_EXPANDED_SOURCE_MAX_LINES: usize = 10;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MindContextPackMode {
    Startup,
    TagSwitch,
    Resume,
    Handoff,
    Dispatch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MindContextPackProfile {
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindContextPackCitation {
    pub source_id: String,
    pub label: String,
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindContextPackSection {
    pub source_id: String,
    pub layer: ContextLayer,
    pub title: String,
    pub citation: String,
    pub lines: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MindContextPack {
    pub schema_version: u16,
    pub mode: MindContextPackMode,
    pub profile: MindContextPackProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub line_budget: usize,
    pub truncated: bool,
    pub rendered_lines: Vec<String>,
    pub sections: Vec<MindContextPackSection>,
    pub citations: Vec<MindContextPackCitation>,
    pub generated_at: String,
}

#[derive(Debug, Clone)]
pub struct MindContextPackRequest {
    pub mode: MindContextPackMode,
    pub profile: MindContextPackProfile,
    pub active_tag: Option<String>,
    pub reason: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MindContextPackSourceOverrides {
    pub aoc_mem: Option<String>,
    pub aoc_stm_current: Option<String>,
    pub aoc_stm_resume: Option<String>,
    pub handshake_markdown: Option<String>,
    pub project_mind_markdown: Option<String>,
    pub latest_export_manifest: Option<SessionExportManifest>,
    pub latest_t1_markdown: Option<String>,
    pub latest_t2_markdown: Option<String>,
}

pub fn parse_mind_context_pack_mode(value: Option<&str>) -> MindContextPackMode {
    match value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("startup") => MindContextPackMode::Startup,
        Some("tag_switch") | Some("tag-switch") => MindContextPackMode::TagSwitch,
        Some("resume") => MindContextPackMode::Resume,
        Some("dispatch") => MindContextPackMode::Dispatch,
        _ => MindContextPackMode::Handoff,
    }
}

pub fn parse_mind_context_pack_request(args: &Value) -> MindContextPackRequest {
    let mode = args
        .as_object()
        .and_then(|value| value.get("mode"))
        .and_then(Value::as_str);
    let detail = args
        .as_object()
        .and_then(|value| value.get("detail"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let active_tag = args
        .as_object()
        .and_then(|value| value.get("active_tag"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let role = args
        .as_object()
        .and_then(|value| value.get("role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let reason = args
        .as_object()
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    MindContextPackRequest {
        mode: parse_mind_context_pack_mode(mode),
        profile: if detail {
            MindContextPackProfile::Expanded
        } else {
            MindContextPackProfile::Compact
        },
        active_tag,
        reason,
        role,
    }
}

pub fn mind_context_pack_mode_for_trigger(
    trigger: MindInjectionTriggerKind,
) -> MindContextPackMode {
    match trigger {
        MindInjectionTriggerKind::Startup => MindContextPackMode::Startup,
        MindInjectionTriggerKind::TagSwitch => MindContextPackMode::TagSwitch,
        MindInjectionTriggerKind::Resume => MindContextPackMode::Resume,
        MindInjectionTriggerKind::Handoff => MindContextPackMode::Handoff,
    }
}

pub fn compile_mind_context_pack(
    project_root: &str,
    store: Option<&MindStore>,
    request: MindContextPackRequest,
    overrides: Option<&MindContextPackSourceOverrides>,
) -> Result<MindContextPack, String> {
    let active_tag = request
        .active_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let role = request
        .role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let profile = request.profile;
    let line_budget = match profile {
        MindContextPackProfile::Compact => MIND_CONTEXT_PACK_COMPACT_MAX_LINES,
        MindContextPackProfile::Expanded => MIND_CONTEXT_PACK_EXPANDED_MAX_LINES,
    };
    let source_line_limit = match profile {
        MindContextPackProfile::Compact => MIND_CONTEXT_PACK_COMPACT_SOURCE_MAX_LINES,
        MindContextPackProfile::Expanded => MIND_CONTEXT_PACK_EXPANDED_SOURCE_MAX_LINES,
    };

    let mut sections = Vec::new();
    let mut citations = Vec::new();

    if let Some(text) = overrides
        .and_then(|value| value.aoc_mem.clone())
        .or_else(|| load_context_cli_output(project_root, "aoc-mem", &["read"]))
    {
        push_context_pack_section(
            &mut sections,
            &mut citations,
            ContextLayer::AocMem,
            "aoc_mem",
            "AOC memory",
            "cmd:aoc-mem read",
            extract_nonempty_lines(&text, source_line_limit),
        );
    }

    let stm_source = match request.mode {
        MindContextPackMode::Resume => overrides
            .and_then(|value| value.aoc_stm_resume.clone())
            .or_else(|| load_context_cli_output(project_root, "aoc-stm", &["resume"]))
            .or_else(|| overrides.and_then(|value| value.aoc_stm_current.clone()))
            .or_else(|| load_context_cli_output(project_root, "aoc-stm", &[])),
        _ => overrides
            .and_then(|value| value.aoc_stm_current.clone())
            .or_else(|| load_context_cli_output(project_root, "aoc-stm", &[])),
    };
    if let Some(text) = stm_source {
        let label = match request.mode {
            MindContextPackMode::Resume => "cmd:aoc-stm resume",
            _ => "cmd:aoc-stm",
        };
        push_context_pack_section(
            &mut sections,
            &mut citations,
            ContextLayer::AocStm,
            "aoc_stm",
            "AOC short-term memory",
            label,
            extract_nonempty_lines(&text, source_line_limit),
        );
    }

    let handshake_text = overrides
        .and_then(|value| value.handshake_markdown.clone())
        .or_else(|| {
            store.and_then(|store| {
                store
                    .latest_handshake_snapshot(
                        "project",
                        &t3_scope_id_for_project_root(project_root),
                    )
                    .ok()
                    .flatten()
                    .map(|snapshot| snapshot.payload_text)
            })
        })
        .or_else(|| {
            read_optional_text(
                &PathBuf::from(project_root)
                    .join(".aoc")
                    .join("mind")
                    .join("t3")
                    .join("handshake.md"),
            )
        });
    if let Some(text) = handshake_text {
        push_context_pack_section(
            &mut sections,
            &mut citations,
            ContextLayer::AocMind,
            "t3_handshake",
            "Mind handshake canon",
            ".aoc/mind/t3/handshake.md",
            extract_nonempty_lines(&text, source_line_limit),
        );
    }

    if matches!(profile, MindContextPackProfile::Expanded) {
        if let Some(text) = overrides
            .and_then(|value| value.project_mind_markdown.clone())
            .or_else(|| {
                read_optional_text(
                    &PathBuf::from(project_root)
                        .join(".aoc")
                        .join("mind")
                        .join("t3")
                        .join("project_mind.md"),
                )
            })
        {
            push_context_pack_section(
                &mut sections,
                &mut citations,
                ContextLayer::AocMind,
                "t3_canon",
                "Project mind canon",
                ".aoc/mind/t3/project_mind.md",
                extract_project_mind_lines(&text, active_tag.as_deref(), source_line_limit),
            );
        }
    }

    let export_manifest = overrides
        .and_then(|value| value.latest_export_manifest.clone())
        .or_else(|| load_latest_session_export_manifest(project_root).ok());
    if let Some(manifest) = export_manifest.filter(|manifest| {
        export_matches_active_tag(manifest.active_tag.as_deref(), active_tag.as_deref())
    }) {
        let export_dir = PathBuf::from(&manifest.export_dir);
        let t2_text = overrides
            .and_then(|value| value.latest_t2_markdown.clone())
            .or_else(|| read_optional_text(&export_dir.join("t2.md")));
        if let Some(text) = t2_text {
            push_context_pack_section(
                &mut sections,
                &mut citations,
                ContextLayer::AocMind,
                "session_t2",
                "Session reflections",
                &format!("{}/t2.md", manifest.export_dir),
                extract_nonempty_lines(&text, source_line_limit),
            );
        }
        let t1_text = overrides
            .and_then(|value| value.latest_t1_markdown.clone())
            .or_else(|| read_optional_text(&export_dir.join("t1.md")));
        if let Some(text) = t1_text {
            push_context_pack_section(
                &mut sections,
                &mut citations,
                ContextLayer::AocMind,
                "session_t1",
                "Session observations",
                &format!("{}/t1.md", manifest.export_dir),
                extract_nonempty_lines(&text, source_line_limit),
            );
        }
    }

    if sections.is_empty() {
        return Err("no context-pack sources available".to_string());
    }

    let inputs = sections
        .iter()
        .map(|section| ContextPackInput {
            layer: section.layer,
            lines: render_context_pack_section(section),
        })
        .collect::<Vec<_>>();
    let composed = compose_context_pack(&inputs, line_budget)
        .map_err(|err| format!("compose context pack failed: {err}"))?;
    let section_truncated = sections.iter().any(|section| section.truncated);

    Ok(MindContextPack {
        schema_version: MIND_CONTEXT_PACK_SCHEMA_VERSION,
        mode: request.mode,
        profile,
        role,
        active_tag,
        reason,
        line_budget,
        truncated: composed.truncated || section_truncated,
        rendered_lines: composed.lines,
        sections,
        citations,
        generated_at: Utc::now().to_rfc3339(),
    })
}

fn push_context_pack_section(
    sections: &mut Vec<MindContextPackSection>,
    citations: &mut Vec<MindContextPackCitation>,
    layer: ContextLayer,
    source_id: &str,
    title: &str,
    reference: &str,
    extracted: (Vec<String>, bool),
) {
    let (lines, truncated) = extracted;
    if lines.is_empty() {
        return;
    }
    let citation = format!("[{source_id}]");
    sections.push(MindContextPackSection {
        source_id: source_id.to_string(),
        layer,
        title: title.to_string(),
        citation: citation.clone(),
        lines,
        truncated,
    });
    citations.push(MindContextPackCitation {
        source_id: source_id.to_string(),
        label: title.to_string(),
        reference: reference.to_string(),
    });
}

fn render_context_pack_section(section: &MindContextPackSection) -> Vec<String> {
    let mut lines = vec![format!("{} {}", section.citation, section.title)];
    lines.extend(section.lines.iter().cloned());
    lines
}

fn extract_nonempty_lines(text: &str, max_lines: usize) -> (Vec<String>, bool) {
    let cleaned = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| *line != "(none)" && *line != "(empty)")
        .filter(|line| !line.starts_with("generated_at:") && !line.starts_with("_generated_at:"))
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let truncated = cleaned.len() > max_lines;
    (cleaned.into_iter().take(max_lines).collect(), truncated)
}

fn extract_project_mind_lines(
    text: &str,
    active_tag: Option<&str>,
    max_lines: usize,
) -> (Vec<String>, bool) {
    let requested = active_tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let mut selected = Vec::new();
    let mut block = Vec::new();
    let mut include_block = requested.is_none();
    let flush_block = |selected: &mut Vec<String>, block: &mut Vec<String>, include_block: bool| {
        if include_block {
            selected.extend(block.iter().cloned());
        }
        block.clear();
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("## ") {
            flush_block(&mut selected, &mut block, include_block);
            include_block = false;
            continue;
        }
        if line.starts_with("### ") {
            flush_block(&mut selected, &mut block, include_block);
            include_block = requested.is_none();
            block.push(line.to_string());
            continue;
        }
        if line.is_empty() || line == "(none)" {
            continue;
        }
        if let Some(requested) = requested.as_ref() {
            if let Some(topic) = line.strip_prefix("- topic:") {
                let topic = topic.trim().to_ascii_lowercase();
                include_block = topic == *requested || topic == "global";
            }
        }
        block.push(line.to_string());
    }
    flush_block(&mut selected, &mut block, include_block);
    let truncated = selected.len() > max_lines;
    (selected.into_iter().take(max_lines).collect(), truncated)
}

fn export_matches_active_tag(export_tag: Option<&str>, requested_tag: Option<&str>) -> bool {
    match (
        export_tag.map(str::trim).filter(|value| !value.is_empty()),
        requested_tag
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (_, None) => true,
        (Some(export_tag), Some(requested_tag)) => export_tag == requested_tag,
        (None, Some(_)) => false,
    }
}

fn load_context_cli_output(project_root: &str, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

fn read_optional_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn edge_kind_key(kind: MindProvenanceEdgeKind) -> &'static str {
    match kind {
        MindProvenanceEdgeKind::ScopeSession => "scope_session",
        MindProvenanceEdgeKind::ScopeHandshake => "scope_handshake",
        MindProvenanceEdgeKind::ScopeBacklogJob => "scope_backlog_job",
        MindProvenanceEdgeKind::SessionConversation => "session_conversation",
        MindProvenanceEdgeKind::ConversationParent => "conversation_parent",
        MindProvenanceEdgeKind::ConversationRoot => "conversation_root",
        MindProvenanceEdgeKind::ConversationArtifact => "conversation_artifact",
        MindProvenanceEdgeKind::ConversationCheckpoint => "conversation_checkpoint",
        MindProvenanceEdgeKind::ArtifactTrace => "artifact_trace",
        MindProvenanceEdgeKind::ArtifactSemanticProvenance => "artifact_semantic_provenance",
        MindProvenanceEdgeKind::ArtifactFileLink => "artifact_file_link",
        MindProvenanceEdgeKind::ArtifactTaskLink => "artifact_task_link",
        MindProvenanceEdgeKind::CheckpointSlice => "checkpoint_slice",
        MindProvenanceEdgeKind::SliceFileRead => "slice_file_read",
        MindProvenanceEdgeKind::SliceFileModified => "slice_file_modified",
        MindProvenanceEdgeKind::CanonSupersedes => "canon_supersedes",
        MindProvenanceEdgeKind::CanonEvidence => "canon_evidence",
        MindProvenanceEdgeKind::HandshakeCanon => "handshake_canon",
        MindProvenanceEdgeKind::BacklogJobArtifact => "backlog_job_artifact",
        MindProvenanceEdgeKind::BacklogJobCanon => "backlog_job_canon",
    }
}

struct MindProvenanceGraphBuilder {
    max_nodes: usize,
    max_edges: usize,
    nodes: Vec<MindProvenanceNode>,
    edges: Vec<MindProvenanceEdge>,
    node_ids: HashSet<String>,
    edge_ids: HashSet<String>,
    truncated: bool,
}

impl MindProvenanceGraphBuilder {
    fn new(max_nodes: usize, max_edges: usize) -> Self {
        Self {
            max_nodes: max_nodes.max(1),
            max_edges: max_edges.max(1),
            nodes: Vec::new(),
            edges: Vec::new(),
            node_ids: HashSet::new(),
            edge_ids: HashSet::new(),
            truncated: false,
        }
    }
    fn add_node(&mut self, node: MindProvenanceNode) {
        if self.node_ids.contains(&node.node_id) {
            return;
        }
        if self.nodes.len() >= self.max_nodes {
            self.truncated = true;
            return;
        }
        self.node_ids.insert(node.node_id.clone());
        self.nodes.push(node);
    }
    fn add_edge(
        &mut self,
        kind: MindProvenanceEdgeKind,
        from: impl Into<String>,
        to: impl Into<String>,
        label: Option<String>,
        attrs: std::collections::BTreeMap<String, serde_json::Value>,
    ) {
        let from = from.into();
        let to = to.into();
        if !self.node_ids.contains(&from) || !self.node_ids.contains(&to) {
            return;
        }
        let edge_id = format!(
            "{}:{}:{}:{}",
            edge_kind_key(kind),
            from,
            to,
            label.as_deref().unwrap_or_default()
        );
        if self.edge_ids.contains(&edge_id) {
            return;
        }
        if self.edges.len() >= self.max_edges {
            self.truncated = true;
            return;
        }
        self.edge_ids.insert(edge_id.clone());
        self.edges.push(MindProvenanceEdge {
            edge_id,
            kind,
            from,
            to,
            label,
            attrs,
        });
    }
    fn finish(
        mut self,
        status: &str,
        summary: String,
        seed_refs: Vec<String>,
    ) -> MindProvenanceQueryResult {
        self.nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        self.edges.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
        MindProvenanceQueryResult {
            status: status.to_string(),
            summary,
            seed_refs,
            nodes: self.nodes,
            edges: self.edges,
            truncated: self.truncated,
        }
    }
}

fn json_string_attr(
    attrs: &mut std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
    value: impl Into<String>,
) {
    attrs.insert(key.to_string(), serde_json::Value::String(value.into()));
}
fn json_bool_attr(
    attrs: &mut std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
    value: bool,
) {
    attrs.insert(key.to_string(), serde_json::Value::Bool(value));
}
fn json_u64_attr(
    attrs: &mut std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
    value: u64,
) {
    attrs.insert(
        key.to_string(),
        serde_json::Value::Number(serde_json::Number::from(value)),
    );
}

pub fn compile_mind_provenance_graph(
    store: &MindStore,
    request: &MindProvenanceQueryRequest,
) -> Result<MindProvenanceQueryResult, String> {
    let mut seed_refs = Vec::new();
    if let Some(project_root) = request.project_root.as_ref() {
        seed_refs.push(format!("project:{}", project_root));
    }
    if let Some(session_id) = request.session_id.as_ref() {
        seed_refs.push(format!("session:{}", session_id));
    }
    if let Some(conversation_id) = request.conversation_id.as_ref() {
        seed_refs.push(format!("conversation:{}", conversation_id));
    }
    if let Some(artifact_id) = request.artifact_id.as_ref() {
        seed_refs.push(format!("artifact:{}", artifact_id));
    }
    if let Some(checkpoint_id) = request.checkpoint_id.as_ref() {
        seed_refs.push(format!("checkpoint:{}", checkpoint_id));
    }
    if let Some(canon_entry_id) = request.canon_entry_id.as_ref() {
        seed_refs.push(format!("canon:{}", canon_entry_id));
    }
    if let Some(task_id) = request.task_id.as_ref() {
        seed_refs.push(format!("task:{}", task_id));
    }
    if let Some(file_path) = request.file_path.as_ref() {
        seed_refs.push(format!("file:{}", file_path));
    }

    let mut graph = MindProvenanceGraphBuilder::new(request.max_nodes, request.max_edges);
    let mut conversation_ids = HashSet::<String>::new();
    let mut artifact_ids = HashSet::<String>::new();
    let mut canon_entry_ids = HashSet::<String>::new();

    let project_scope_id = if let Some(project_root) = request.project_root.as_ref() {
        let scope_key = t3_scope_id_for_project_root(project_root);
        let mut attrs = std::collections::BTreeMap::new();
        json_string_attr(&mut attrs, "project_root", project_root.clone());
        json_string_attr(&mut attrs, "scope_key", scope_key.clone());
        if let Some(watermark) = store
            .project_watermark(&scope_key)
            .map_err(|err| format!("load project watermark failed: {err}"))?
        {
            if let Some(last_artifact_id) = watermark.last_artifact_id.as_ref() {
                json_string_attr(
                    &mut attrs,
                    "watermark_last_artifact_id",
                    last_artifact_id.clone(),
                );
            }
            if let Some(last_artifact_ts) = watermark.last_artifact_ts.as_ref() {
                json_string_attr(
                    &mut attrs,
                    "watermark_last_artifact_ts",
                    last_artifact_ts.to_rfc3339(),
                );
            }
        }
        graph.add_node(MindProvenanceNode {
            node_id: format!("scope:{}", scope_key),
            kind: MindProvenanceNodeKind::ProjectScope,
            label: project_root.clone(),
            reference: Some(scope_key.clone()),
            attrs,
        });
        Some(scope_key)
    } else {
        None
    };

    if let Some(session_id) = request.session_id.as_ref() {
        graph.add_node(MindProvenanceNode {
            node_id: format!("session:{}", session_id),
            kind: MindProvenanceNodeKind::Session,
            label: format!("Session {session_id}"),
            reference: Some(session_id.clone()),
            attrs: std::collections::BTreeMap::new(),
        });
        if let Some(scope_key) = project_scope_id.as_ref() {
            graph.add_edge(
                MindProvenanceEdgeKind::ScopeSession,
                format!("scope:{}", scope_key),
                format!("session:{}", session_id),
                Some("contains".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
        for conversation_id in store
            .conversation_ids_for_session(session_id)
            .map_err(|err| format!("list conversation lineage failed: {err}"))?
        {
            conversation_ids.insert(conversation_id);
        }
    }
    if let Some(conversation_id) = request.conversation_id.as_ref() {
        conversation_ids.insert(conversation_id.clone());
    }
    if let Some(artifact_id) = request.artifact_id.as_ref() {
        artifact_ids.insert(artifact_id.clone());
        if let Some(artifact) = store
            .artifact_by_id(artifact_id)
            .map_err(|err| format!("load artifact failed: {err}"))?
        {
            conversation_ids.insert(artifact.conversation_id);
        }
    }
    if let Some(checkpoint_id) = request.checkpoint_id.as_ref() {
        if let Some(checkpoint) = store
            .compaction_checkpoint_by_id(checkpoint_id)
            .map_err(|err| format!("load checkpoint failed: {err}"))?
        {
            conversation_ids.insert(checkpoint.conversation_id.clone());
            add_provenance_checkpoint_branch(store, &mut graph, &checkpoint)?;
        }
    }
    if let Some(task_id) = request.task_id.as_ref() {
        graph.add_node(MindProvenanceNode {
            node_id: format!("task:{}", task_id),
            kind: MindProvenanceNodeKind::Task,
            label: task_id.clone(),
            reference: Some(task_id.clone()),
            attrs: std::collections::BTreeMap::new(),
        });
        for artifact_id in store
            .artifact_ids_for_task_id(task_id)
            .map_err(|err| format!("load task-linked artifacts failed: {err}"))?
        {
            artifact_ids.insert(artifact_id);
        }
    }
    if let Some(file_path) = request.file_path.as_ref() {
        add_provenance_file_node(&mut graph, file_path, Some("seed"));
        for artifact_id in store
            .artifact_ids_for_file_path(file_path)
            .map_err(|err| format!("load file-linked artifacts failed: {err}"))?
        {
            artifact_ids.insert(artifact_id);
        }
    }

    for conversation_id in conversation_ids.iter() {
        let lineage = store
            .conversation_lineage(conversation_id)
            .map_err(|err| format!("load conversation lineage failed: {err}"))?;
        let mut attrs = std::collections::BTreeMap::new();
        if let Some(lineage) = lineage.as_ref() {
            json_string_attr(&mut attrs, "session_id", lineage.session_id.clone());
            json_string_attr(
                &mut attrs,
                "root_conversation_id",
                lineage.root_conversation_id.clone(),
            );
            if let Some(parent) = lineage.parent_conversation_id.as_ref() {
                json_string_attr(&mut attrs, "parent_conversation_id", parent.clone());
            }
        }
        graph.add_node(MindProvenanceNode {
            node_id: format!("conversation:{}", conversation_id),
            kind: MindProvenanceNodeKind::Conversation,
            label: format!("Conversation {conversation_id}"),
            reference: Some(conversation_id.clone()),
            attrs,
        });
        if let Some(lineage) = lineage.as_ref() {
            graph.add_node(MindProvenanceNode {
                node_id: format!("session:{}", lineage.session_id),
                kind: MindProvenanceNodeKind::Session,
                label: format!("Session {}", lineage.session_id),
                reference: Some(lineage.session_id.clone()),
                attrs: std::collections::BTreeMap::new(),
            });
            if let Some(scope_key) = project_scope_id.as_ref() {
                graph.add_edge(
                    MindProvenanceEdgeKind::ScopeSession,
                    format!("scope:{}", scope_key),
                    format!("session:{}", lineage.session_id),
                    Some("contains".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
            graph.add_edge(
                MindProvenanceEdgeKind::SessionConversation,
                format!("session:{}", lineage.session_id),
                format!("conversation:{}", conversation_id),
                Some("contains".to_string()),
                std::collections::BTreeMap::new(),
            );
            if let Some(parent) = lineage.parent_conversation_id.as_ref() {
                graph.add_node(MindProvenanceNode {
                    node_id: format!("conversation:{}", parent),
                    kind: MindProvenanceNodeKind::Conversation,
                    label: format!("Conversation {parent}"),
                    reference: Some(parent.clone()),
                    attrs: std::collections::BTreeMap::new(),
                });
                graph.add_edge(
                    MindProvenanceEdgeKind::ConversationParent,
                    format!("conversation:{}", conversation_id),
                    format!("conversation:{}", parent),
                    Some("parent".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
            if lineage.root_conversation_id != *conversation_id {
                graph.add_node(MindProvenanceNode {
                    node_id: format!("conversation:{}", lineage.root_conversation_id),
                    kind: MindProvenanceNodeKind::Conversation,
                    label: format!("Conversation {}", lineage.root_conversation_id),
                    reference: Some(lineage.root_conversation_id.clone()),
                    attrs: std::collections::BTreeMap::new(),
                });
                graph.add_edge(
                    MindProvenanceEdgeKind::ConversationRoot,
                    format!("conversation:{}", conversation_id),
                    format!("conversation:{}", lineage.root_conversation_id),
                    Some("root".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
        for artifact in store
            .artifacts_for_conversation(conversation_id)
            .map_err(|err| format!("list conversation artifacts failed: {err}"))?
        {
            artifact_ids.insert(artifact.artifact_id.clone());
            add_provenance_artifact_node(&mut graph, &artifact);
            graph.add_edge(
                MindProvenanceEdgeKind::ConversationArtifact,
                format!("conversation:{}", conversation_id),
                format!("artifact:{}", artifact.artifact_id),
                Some(artifact.kind.clone()),
                std::collections::BTreeMap::new(),
            );
        }
        for checkpoint in store
            .compaction_checkpoints_for_conversation(conversation_id)
            .map_err(|err| format!("list conversation checkpoints failed: {err}"))?
        {
            add_provenance_checkpoint_branch(store, &mut graph, &checkpoint)?;
        }
    }

    let mut artifact_queue = artifact_ids.iter().cloned().collect::<Vec<_>>();
    artifact_queue.sort();
    for artifact_id in artifact_queue {
        let Some(artifact) = store
            .artifact_by_id(&artifact_id)
            .map_err(|err| format!("load artifact by id failed: {err}"))?
        else {
            continue;
        };
        add_provenance_artifact_node(&mut graph, &artifact);
        for trace_id in &artifact.trace_ids {
            if let Some(traced) = store
                .artifact_by_id(trace_id)
                .map_err(|err| format!("load traced artifact failed: {err}"))?
            {
                add_provenance_artifact_node(&mut graph, &traced);
                graph.add_edge(
                    MindProvenanceEdgeKind::ArtifactTrace,
                    format!("artifact:{}", artifact.artifact_id),
                    format!("artifact:{}", traced.artifact_id),
                    Some("trace".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
        for entry in store
            .semantic_provenance_for_artifact(&artifact.artifact_id)
            .map_err(|err| format!("load semantic provenance failed: {err}"))?
        {
            let node_id = format!(
                "semantic:{}:{}:{}",
                artifact.artifact_id, entry.prompt_version, entry.attempt_count
            );
            let mut attrs = std::collections::BTreeMap::new();
            json_string_attr(
                &mut attrs,
                "stage",
                format!("{:?}", entry.stage).to_lowercase(),
            );
            json_string_attr(
                &mut attrs,
                "runtime",
                format!("{:?}", entry.runtime).to_lowercase(),
            );
            json_string_attr(&mut attrs, "prompt_version", entry.prompt_version.clone());
            json_bool_attr(&mut attrs, "fallback_used", entry.fallback_used);
            json_u64_attr(&mut attrs, "attempt_count", entry.attempt_count as u64);
            graph.add_node(MindProvenanceNode {
                node_id: node_id.clone(),
                kind: MindProvenanceNodeKind::SemanticProvenance,
                label: format!("{} attempt {}", artifact.artifact_id, entry.attempt_count),
                reference: Some(artifact.artifact_id.clone()),
                attrs,
            });
            graph.add_edge(
                MindProvenanceEdgeKind::ArtifactSemanticProvenance,
                format!("artifact:{}", artifact.artifact_id),
                node_id,
                Some("semantic_provenance".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
        for file_link in store
            .artifact_file_links(&artifact.artifact_id)
            .map_err(|err| format!("load artifact file links failed: {err}"))?
        {
            add_provenance_file_node(&mut graph, &file_link.path, Some(&file_link.relation));
            let mut attrs = std::collections::BTreeMap::new();
            json_string_attr(&mut attrs, "relation", file_link.relation.clone());
            json_string_attr(&mut attrs, "source", file_link.source.clone());
            graph.add_edge(
                MindProvenanceEdgeKind::ArtifactFileLink,
                format!("artifact:{}", artifact.artifact_id),
                format!("file:{}", file_link.path),
                Some(file_link.relation.clone()),
                attrs,
            );
        }
        for task_link in store
            .artifact_task_links_for_artifact(&artifact.artifact_id)
            .map_err(|err| format!("load artifact task links failed: {err}"))?
        {
            let task_id = format!("task:{}", task_link.task_id);
            let mut task_attrs = std::collections::BTreeMap::new();
            json_u64_attr(
                &mut task_attrs,
                "confidence_bps",
                task_link.confidence_bps as u64,
            );
            graph.add_node(MindProvenanceNode {
                node_id: task_id.clone(),
                kind: MindProvenanceNodeKind::Task,
                label: task_link.task_id.clone(),
                reference: Some(task_link.task_id.clone()),
                attrs: task_attrs,
            });
            let mut edge_attrs = std::collections::BTreeMap::new();
            json_string_attr(
                &mut edge_attrs,
                "relation",
                format!("{:?}", task_link.relation).to_lowercase(),
            );
            json_string_attr(&mut edge_attrs, "source", task_link.source.clone());
            graph.add_edge(
                MindProvenanceEdgeKind::ArtifactTaskLink,
                format!("artifact:{}", artifact.artifact_id),
                task_id,
                Some(format!("{:?}", task_link.relation).to_lowercase()),
                edge_attrs,
            );
        }
    }

    let canon_topic = request.active_tag.as_deref();
    let mut canon_revisions = if let Some(seed_entry) = request.canon_entry_id.as_ref() {
        store
            .canon_entry_revisions(seed_entry)
            .map_err(|err| format!("load canon revisions failed: {err}"))?
    } else {
        let mut revisions = store
            .active_canon_entries(canon_topic)
            .map_err(|err| format!("load active canon failed: {err}"))?;
        if request.include_stale_canon {
            revisions.extend(
                store
                    .canon_entries_by_state(CanonRevisionState::Stale, canon_topic)
                    .map_err(|err| format!("load stale canon failed: {err}"))?,
            );
        }
        revisions
    };
    if !request.include_stale_canon {
        canon_revisions.retain(|revision| revision.state != CanonRevisionState::Stale);
    }
    canon_revisions.sort_by(|a, b| {
        a.entry_id
            .cmp(&b.entry_id)
            .then_with(|| b.revision.cmp(&a.revision))
    });
    for revision in canon_revisions {
        let node_id = format!("canon:{}#r{}", revision.entry_id, revision.revision);
        canon_entry_ids.insert(node_id.clone());
        let mut attrs = std::collections::BTreeMap::new();
        json_u64_attr(&mut attrs, "revision", revision.revision as u64);
        json_u64_attr(&mut attrs, "confidence_bps", revision.confidence_bps as u64);
        json_u64_attr(
            &mut attrs,
            "freshness_score",
            revision.freshness_score as u64,
        );
        if let Some(topic) = revision.topic.as_ref() {
            json_string_attr(&mut attrs, "topic", topic.clone());
        }
        json_string_attr(
            &mut attrs,
            "state",
            format!("{:?}", revision.state).to_lowercase(),
        );
        graph.add_node(MindProvenanceNode {
            node_id: node_id.clone(),
            kind: MindProvenanceNodeKind::CanonEntryRevision,
            label: revision.summary.clone(),
            reference: Some(format!("{}.r{}", revision.entry_id, revision.revision)),
            attrs,
        });
        if let Some(previous) = revision.supersedes_entry_id.as_ref() {
            for prior in store
                .canon_entry_revisions(previous)
                .map_err(|err| format!("load superseded canon failed: {err}"))?
                .into_iter()
                .take(1)
            {
                let prior_id = format!("canon:{}#r{}", prior.entry_id, prior.revision);
                graph.add_node(MindProvenanceNode {
                    node_id: prior_id.clone(),
                    kind: MindProvenanceNodeKind::CanonEntryRevision,
                    label: prior.summary.clone(),
                    reference: Some(format!("{}.r{}", prior.entry_id, prior.revision)),
                    attrs: std::collections::BTreeMap::new(),
                });
                graph.add_edge(
                    MindProvenanceEdgeKind::CanonSupersedes,
                    node_id.clone(),
                    prior_id,
                    Some("supersedes".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
        for evidence_ref in &revision.evidence_refs {
            if let Some(artifact) = store
                .artifact_by_id(evidence_ref)
                .map_err(|err| format!("load canon evidence artifact failed: {err}"))?
            {
                add_provenance_artifact_node(&mut graph, &artifact);
                graph.add_edge(
                    MindProvenanceEdgeKind::CanonEvidence,
                    node_id.clone(),
                    format!("artifact:{}", artifact.artifact_id),
                    Some("evidence".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
    }

    if let (Some(project_root), Some(scope_key)) =
        (request.project_root.as_ref(), project_scope_id.as_ref())
    {
        for job in store
            .t3_backlog_jobs_for_project_root(project_root)
            .map_err(|err| format!("load t3 backlog jobs failed: {err}"))?
        {
            let mut attrs = std::collections::BTreeMap::new();
            json_string_attr(&mut attrs, "session_id", job.session_id.clone());
            json_string_attr(&mut attrs, "pane_id", job.pane_id.clone());
            json_string_attr(
                &mut attrs,
                "status",
                format!("{:?}", job.status).to_lowercase(),
            );
            json_u64_attr(&mut attrs, "attempts", job.attempts as u64);
            if let Some(active_tag) = job.active_tag.as_ref() {
                json_string_attr(&mut attrs, "active_tag", active_tag.clone());
            }
            if let Some(slice_start_id) = job.slice_start_id.as_ref() {
                json_string_attr(&mut attrs, "slice_start_id", slice_start_id.clone());
            }
            if let Some(slice_end_id) = job.slice_end_id.as_ref() {
                json_string_attr(&mut attrs, "slice_end_id", slice_end_id.clone());
            }
            graph.add_node(MindProvenanceNode {
                node_id: format!("backlog:{}", job.job_id),
                kind: MindProvenanceNodeKind::T3BacklogJob,
                label: job.job_id.clone(),
                reference: Some(job.job_id.clone()),
                attrs,
            });
            graph.add_edge(
                MindProvenanceEdgeKind::ScopeBacklogJob,
                format!("scope:{}", scope_key),
                format!("backlog:{}", job.job_id),
                Some("queued".to_string()),
                std::collections::BTreeMap::new(),
            );
            for artifact_id in &job.artifact_refs {
                if let Some(artifact) = store
                    .artifact_by_id(artifact_id)
                    .map_err(|err| format!("load backlog artifact failed: {err}"))?
                {
                    add_provenance_artifact_node(&mut graph, &artifact);
                    graph.add_edge(
                        MindProvenanceEdgeKind::BacklogJobArtifact,
                        format!("backlog:{}", job.job_id),
                        format!("artifact:{}", artifact.artifact_id),
                        Some("input".to_string()),
                        std::collections::BTreeMap::new(),
                    );
                }
            }
            for canon_id in canon_entry_ids.iter().cloned().collect::<Vec<_>>() {
                graph.add_edge(
                    MindProvenanceEdgeKind::BacklogJobCanon,
                    format!("backlog:{}", job.job_id),
                    canon_id,
                    Some("targets".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
        if let Some(snapshot) = store
            .latest_handshake_snapshot("project", scope_key)
            .map_err(|err| format!("load handshake snapshot failed: {err}"))?
        {
            let mut attrs = std::collections::BTreeMap::new();
            json_u64_attr(&mut attrs, "token_estimate", snapshot.token_estimate as u64);
            json_string_attr(&mut attrs, "scope", snapshot.scope.clone());
            graph.add_node(MindProvenanceNode {
                node_id: format!("handshake:{}", snapshot.snapshot_id),
                kind: MindProvenanceNodeKind::HandshakeSnapshot,
                label: format!("Handshake {}", project_root),
                reference: Some(".aoc/mind/t3/handshake.md".to_string()),
                attrs,
            });
            graph.add_edge(
                MindProvenanceEdgeKind::ScopeHandshake,
                format!("scope:{}", scope_key),
                format!("handshake:{}", snapshot.snapshot_id),
                Some("latest".to_string()),
                std::collections::BTreeMap::new(),
            );
            for canon_id in canon_entry_ids.iter().cloned().collect::<Vec<_>>() {
                graph.add_edge(
                    MindProvenanceEdgeKind::HandshakeCanon,
                    format!("handshake:{}", snapshot.snapshot_id),
                    canon_id,
                    Some("renders".to_string()),
                    std::collections::BTreeMap::new(),
                );
            }
        }
    }

    let node_count = graph.nodes.len();
    let edge_count = graph.edges.len();
    Ok(graph.finish(
        "ok",
        format!(
            "{} nodes, {} edges across lineage, artifacts, checkpoints, canon, and handshake",
            node_count, edge_count
        ),
        seed_refs,
    ))
}

fn add_provenance_artifact_node(graph: &mut MindProvenanceGraphBuilder, artifact: &StoredArtifact) {
    let mut attrs = std::collections::BTreeMap::new();
    json_string_attr(
        &mut attrs,
        "conversation_id",
        artifact.conversation_id.clone(),
    );
    json_string_attr(&mut attrs, "kind", artifact.kind.clone());
    json_u64_attr(&mut attrs, "trace_count", artifact.trace_ids.len() as u64);
    graph.add_node(MindProvenanceNode {
        node_id: format!("artifact:{}", artifact.artifact_id),
        kind: MindProvenanceNodeKind::Artifact,
        label: artifact.artifact_id.clone(),
        reference: Some(artifact.artifact_id.clone()),
        attrs,
    });
}

fn add_provenance_file_node(
    graph: &mut MindProvenanceGraphBuilder,
    path: &str,
    relation: Option<&str>,
) {
    let mut attrs = std::collections::BTreeMap::new();
    if let Some(relation) = relation {
        json_string_attr(&mut attrs, "relation", relation.to_string());
    }
    graph.add_node(MindProvenanceNode {
        node_id: format!("file:{path}"),
        kind: MindProvenanceNodeKind::File,
        label: path.to_string(),
        reference: Some(path.to_string()),
        attrs,
    });
}

pub fn compile_mind_provenance_export(
    store: &MindStore,
    request: MindProvenanceQueryRequest,
) -> Result<MindProvenanceExport, String> {
    let graph = compile_mind_provenance_graph(store, &request)?;
    Ok(MindProvenanceExport::new(request, graph))
}

fn add_provenance_checkpoint_branch(
    store: &MindStore,
    graph: &mut MindProvenanceGraphBuilder,
    checkpoint: &CompactionCheckpoint,
) -> Result<(), String> {
    let mut attrs = std::collections::BTreeMap::new();
    json_string_attr(&mut attrs, "session_id", checkpoint.session_id.clone());
    json_string_attr(
        &mut attrs,
        "trigger_source",
        checkpoint.trigger_source.clone(),
    );
    if let Some(reason) = checkpoint.reason.as_ref() {
        json_string_attr(&mut attrs, "reason", reason.clone());
    }
    graph.add_node(MindProvenanceNode {
        node_id: format!("checkpoint:{}", checkpoint.checkpoint_id),
        kind: MindProvenanceNodeKind::CompactionCheckpoint,
        label: checkpoint.checkpoint_id.clone(),
        reference: Some(checkpoint.checkpoint_id.clone()),
        attrs,
    });
    graph.add_node(MindProvenanceNode {
        node_id: format!("conversation:{}", checkpoint.conversation_id),
        kind: MindProvenanceNodeKind::Conversation,
        label: format!("Conversation {}", checkpoint.conversation_id),
        reference: Some(checkpoint.conversation_id.clone()),
        attrs: std::collections::BTreeMap::new(),
    });
    graph.add_edge(
        MindProvenanceEdgeKind::ConversationCheckpoint,
        format!("conversation:{}", checkpoint.conversation_id),
        format!("checkpoint:{}", checkpoint.checkpoint_id),
        Some("checkpoint".to_string()),
        std::collections::BTreeMap::new(),
    );
    if let Some(slice) = store
        .compaction_t0_slice_for_checkpoint(&checkpoint.checkpoint_id)
        .map_err(|err| format!("load compaction t0 slice failed: {err}"))?
    {
        let mut slice_attrs = std::collections::BTreeMap::new();
        json_string_attr(&mut slice_attrs, "source_kind", slice.source_kind.clone());
        json_u64_attr(
            &mut slice_attrs,
            "schema_version",
            slice.schema_version as u64,
        );
        graph.add_node(MindProvenanceNode {
            node_id: format!("slice:{}", slice.slice_id),
            kind: MindProvenanceNodeKind::CompactionT0Slice,
            label: slice.slice_id.clone(),
            reference: Some(slice.slice_id.clone()),
            attrs: slice_attrs,
        });
        graph.add_edge(
            MindProvenanceEdgeKind::CheckpointSlice,
            format!("checkpoint:{}", checkpoint.checkpoint_id),
            format!("slice:{}", slice.slice_id),
            Some("materializes".to_string()),
            std::collections::BTreeMap::new(),
        );
        for path in &slice.read_files {
            add_provenance_file_node(graph, path, Some("read"));
            graph.add_edge(
                MindProvenanceEdgeKind::SliceFileRead,
                format!("slice:{}", slice.slice_id),
                format!("file:{path}"),
                Some("read".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
        for path in &slice.modified_files {
            add_provenance_file_node(graph, path, Some("modified"));
            graph.add_edge(
                MindProvenanceEdgeKind::SliceFileModified,
                format!("slice:{}", slice.slice_id),
                format!("file:{path}"),
                Some("modified".to_string()),
                std::collections::BTreeMap::new(),
            );
        }
    }
    Ok(())
}

fn t3_scope_id_for_project_root(project_root: &str) -> String {
    project_scope_key(Path::new(project_root))
}
