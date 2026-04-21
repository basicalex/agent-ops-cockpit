//! Mind store queries: artifact drilldown, search, and persistence access.
//!
//! Extracted from `aoc-mission-control/src/main.rs` so that Mind queries
//! can be reused by any consumer (Mission Control, Mind TUI, CLI tools).

use aoc_storage::{CanonRevisionState, CompactionCheckpoint, StoredCompactionT0Slice};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

// --- Re-export types needed by query module ---

/// Typed export manifest from the Mind store.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct MindSessionExportManifest {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub active_tag: Option<String>,
    #[serde(default)]
    pub export_dir: String,
    #[serde(default)]
    pub t1_count: usize,
    #[serde(default)]
    pub t2_count: usize,
    #[serde(default)]
    pub t3_job_id: String,
    #[serde(default)]
    pub exported_at: String,
}

/// Handshake entry from the Mind store.
#[derive(Clone, Debug, Default)]
pub struct MindHandshakeEntry {
    pub entry_id: String,
    pub revision: u32,
    pub topic: Option<String>,
    pub summary: String,
}

/// Canon entry from the Mind store.
#[derive(Clone, Debug, Default)]
pub struct MindCanonEntry {
    pub entry_id: String,
    pub revision: u32,
    pub topic: Option<String>,
    pub evidence_refs: Vec<String>,
    pub summary: String,
}

/// Aggregated artifact snapshot for the current project.
#[derive(Clone, Debug, Default)]
pub struct MindArtifactDrilldown {
    pub latest_export: Option<MindSessionExportManifest>,
    pub latest_compaction_checkpoint: Option<CompactionCheckpoint>,
    pub latest_compaction_slice: Option<StoredCompactionT0Slice>,
    pub compaction_marker_event_available: bool,
    pub compaction_rebuildable: bool,
    pub handshake_entries: Vec<MindHandshakeEntry>,
    pub active_canon_entries: Vec<MindCanonEntry>,
    pub stale_canon_count: usize,
}

/// Search result within Mind artifacts.
#[derive(Clone, Debug)]
pub struct MindSearchHit {
    pub kind: &'static str,
    pub title: String,
    pub summary: String,
    pub detail: Vec<String>,
    pub score: usize,
}

/// Returns the path to the project Mind SQLite store.
pub fn mind_store_path(project_root: &Path) -> PathBuf {
    resolve_aoc_state_home()
        .join("aoc")
        .join("mind")
        .join("projects")
        .join(sanitize_runtime_component(&project_root.to_string_lossy()))
        .join("project.sqlite")
}

/// Compute the canonical key for a canon entry.
pub fn canon_key(entry_id: &str, revision: u32) -> String {
    format!("{}#{}", entry_id.trim(), revision)
}

/// Determine compaction rebuildability from stored attributes.
pub fn compaction_rebuildable_from_attrs(attrs: &BTreeMap<String, serde_json::Value>) -> bool {
    attrs.contains_key("mind_compaction_modified_files")
        || attrs.contains_key("project_file")
        || attrs.contains_key("project_url")
        || attrs.contains_key("project_text")
        || attrs.contains_key("project_snippet")
        || attrs.contains_key("pi_detail_read_files")
        || attrs.contains_key("pi_detail_modified_files")
}

/// Full artifact drilldown: export manifest, compaction state, handshake, canon.
pub fn load_mind_artifact_drilldown(
    project_root: &Path,
    session_id: &str,
) -> MindArtifactDrilldown {
    let mut snapshot = MindArtifactDrilldown::default();

    let insight_dir = project_root.join(".aoc").join("mind").join("insight");
    if let Some(manifest) = load_latest_session_export_manifest(&insight_dir) {
        snapshot.latest_export = Some(manifest);
    }

    let compatibility = mind_feed_compatibility_mode();
    let store_path = mind_store_path(project_root);
    if compatibility != MindFeedCompatibilityMode::Legacy && store_path.exists() {
        if let Ok(store) = aoc_storage::MindStore::open(&store_path) {
            snapshot.latest_compaction_checkpoint = store
                .latest_compaction_checkpoint_for_session(session_id)
                .ok()
                .flatten();
            snapshot.latest_compaction_slice = store
                .latest_compaction_t0_slice_for_session(session_id)
                .ok()
                .flatten();
            if let Some(checkpoint) = snapshot.latest_compaction_checkpoint.as_ref() {
                if let Some(marker_event_id) = checkpoint.marker_event_id.as_deref() {
                    if let Ok(marker_event) = store.raw_event_by_id(marker_event_id) {
                        snapshot.compaction_marker_event_available = marker_event.is_some();
                        snapshot.compaction_rebuildable = marker_event
                            .as_ref()
                            .map(|event| compaction_rebuildable_from_attrs(&event.attrs))
                            .unwrap_or(false);
                    }
                }
                if snapshot.latest_compaction_slice.is_none() {
                    snapshot.latest_compaction_slice = store
                        .compaction_t0_slice_for_checkpoint(&checkpoint.checkpoint_id)
                        .ok()
                        .flatten();
                }
            }

            let scope_key = project_scope_key(project_root);
            if let Ok(Some(handshake)) = store.latest_handshake_snapshot("project", &scope_key) {
                snapshot.handshake_entries = parse_handshake_entries(&handshake.payload_text);
            }
            if let Ok(active) = store.active_canon_entries(None) {
                snapshot.active_canon_entries = active
                    .into_iter()
                    .map(|entry| MindCanonEntry {
                        entry_id: entry.entry_id,
                        revision: entry.revision.max(0) as u32,
                        topic: entry.topic,
                        evidence_refs: entry.evidence_refs,
                        summary: entry.summary,
                    })
                    .collect();
            }
            if let Ok(stale) = store.canon_entries_by_state(CanonRevisionState::Stale, None) {
                snapshot.stale_canon_count = stale.len();
            }
        }
    }

    let should_fallback_legacy = compatibility != MindFeedCompatibilityMode::Canonical
        && (snapshot.handshake_entries.is_empty()
            || snapshot.active_canon_entries.is_empty() && snapshot.stale_canon_count == 0);
    if should_fallback_legacy {
        load_legacy_mind_artifact_drilldown(project_root, &mut snapshot);
    }

    snapshot
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MindFeedCompatibilityMode {
    Canonical,
    Hybrid,
    Legacy,
}

fn mind_feed_compatibility_mode() -> MindFeedCompatibilityMode {
    match env::var("AOC_MIND_FEED_COMPAT")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("canonical") | Some("v2") | Some("store") => MindFeedCompatibilityMode::Canonical,
        Some("legacy") | Some("v1") | Some("files") => MindFeedCompatibilityMode::Legacy,
        _ => MindFeedCompatibilityMode::Hybrid,
    }
}

pub fn project_scope_key(project_root: &Path) -> String {
    format!("project:{}", project_root.to_string_lossy())
}

fn load_legacy_mind_artifact_drilldown(project_root: &Path, snapshot: &mut MindArtifactDrilldown) {
    let t3_dir = project_root.join(".aoc").join("mind").join("t3");
    if snapshot.handshake_entries.is_empty() {
        let handshake_path = t3_dir.join("handshake.md");
        if let Ok(payload) = fs::read_to_string(&handshake_path) {
            snapshot.handshake_entries = parse_handshake_entries(&payload);
        }
    }

    if snapshot.active_canon_entries.is_empty() && snapshot.stale_canon_count == 0 {
        let canon_path = t3_dir.join("project_mind.md");
        if let Ok(payload) = fs::read_to_string(&canon_path) {
            let (active, stale_count) = parse_project_canon_entries(&payload);
            snapshot.active_canon_entries = active;
            snapshot.stale_canon_count = stale_count;
        }
    }
}

fn resolve_aoc_state_home() -> PathBuf {
    if let Ok(value) = env::var("XDG_STATE_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".local/state")
}

fn sanitize_runtime_component(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Load the most recent session export manifest from the Mind insights directory.
fn load_latest_session_export_manifest(insight_dir: &Path) -> Option<MindSessionExportManifest> {
    let entries = fs::read_dir(insight_dir).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    let latest = dirs.pop()?;
    let payload = fs::read_to_string(latest.join("manifest.json")).ok()?;
    serde_json::from_str::<MindSessionExportManifest>(&payload).ok()
}

/// Parse handshake entries from JSON or legacy markdown export payloads.
pub fn parse_handshake_entries(payload: &str) -> Vec<MindHandshakeEntry> {
    let trimmed = payload.trim();
    if trimmed.starts_with('[') {
        let mut entries = Vec::new();
        let Ok(entries_arr): Result<Vec<serde_json::Value>, _> = serde_json::from_str(payload)
        else {
            return entries;
        };
        for val in entries_arr {
            let entry_id = val
                .get("entry_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let revision = val.get("revision").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let topic = val
                .get("topic")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let summary = val
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            entries.push(MindHandshakeEntry {
                entry_id,
                revision,
                topic,
                summary,
            });
        }
        return entries;
    }

    let mut entries = Vec::new();
    for line in payload.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("- [") else {
            continue;
        };
        let Some(end_bracket) = rest.find(']') else {
            continue;
        };
        let head = rest[..end_bracket].trim();
        let Some((entry_id, revision_raw)) = head.rsplit_once(" r") else {
            continue;
        };
        let Ok(revision) = revision_raw.trim().parse::<u32>() else {
            continue;
        };

        let tail = rest[end_bracket + 1..].trim();
        let topic = tail
            .split_whitespace()
            .find_map(|segment| segment.strip_prefix("topic="))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let summary = tail
            .split_once("::")
            .map(|(_, text)| text.trim().to_string())
            .unwrap_or_default();

        entries.push(MindHandshakeEntry {
            entry_id: entry_id.trim().to_string(),
            revision,
            topic,
            summary,
        });
    }
    entries
}

/// Parse project canon entries from JSON or legacy markdown payloads.
pub fn parse_project_canon_entries(payload: &str) -> (Vec<MindCanonEntry>, usize) {
    let trimmed = payload.trim();
    if trimmed.starts_with('[') {
        let mut entries = Vec::new();
        let mut stale = 0;
        let Ok(items): Result<Vec<serde_json::Value>, _> = serde_json::from_str(payload) else {
            return (entries, stale);
        };
        for val in items {
            let entry_id = val
                .get("entry_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let revision = val.get("revision").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let topic = val
                .get("topic")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let evidence_refs = val
                .get("evidence_refs")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let summary = val
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let is_stale = val.get("stale").and_then(|v| v.as_bool()).unwrap_or(false);
            if is_stale {
                stale += 1;
            } else {
                entries.push(MindCanonEntry {
                    entry_id,
                    revision,
                    topic,
                    evidence_refs,
                    summary,
                });
            }
        }
        return (entries, stale);
    }

    enum Section {
        None,
        Active,
        Stale,
    }

    let mut section = Section::None;
    let mut active_entries = Vec::new();
    let mut stale_count = 0usize;
    let mut current: Option<MindCanonEntry> = None;

    let flush_current = |section: &Section,
                         current: &mut Option<MindCanonEntry>,
                         active_entries: &mut Vec<MindCanonEntry>,
                         stale_count: &mut usize| {
        let Some(entry) = current.take() else {
            return;
        };
        match section {
            Section::Active => active_entries.push(entry),
            Section::Stale => *stale_count += 1,
            Section::None => {}
        }
    };

    for raw_line in payload.lines() {
        let line = raw_line.trim();

        if line == "## Active canon" {
            flush_current(
                &section,
                &mut current,
                &mut active_entries,
                &mut stale_count,
            );
            section = Section::Active;
            continue;
        }
        if line == "## Stale canon" {
            flush_current(
                &section,
                &mut current,
                &mut active_entries,
                &mut stale_count,
            );
            section = Section::Stale;
            continue;
        }

        if let Some(header) = line.strip_prefix("### ") {
            flush_current(
                &section,
                &mut current,
                &mut active_entries,
                &mut stale_count,
            );
            let Some((entry_id, revision_raw)) = header.rsplit_once(" r") else {
                current = None;
                continue;
            };
            let Ok(revision) = revision_raw.trim().parse::<u32>() else {
                current = None;
                continue;
            };
            current = Some(MindCanonEntry {
                entry_id: entry_id.trim().to_string(),
                revision,
                topic: None,
                evidence_refs: Vec::new(),
                summary: String::new(),
            });
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };

        if let Some(topic) = line.strip_prefix("- topic:") {
            let topic = topic.trim();
            if !topic.is_empty() {
                entry.topic = Some(topic.to_string());
            }
            continue;
        }

        if let Some(refs) = line.strip_prefix("- evidence_refs:") {
            entry.evidence_refs = refs
                .split(',')
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            continue;
        }

        if !line.is_empty() && !line.starts_with('-') && entry.summary.is_empty() {
            entry.summary = line.to_string();
        }
    }

    flush_current(
        &section,
        &mut current,
        &mut active_entries,
        &mut stale_count,
    );

    active_entries.sort_by(|left, right| {
        left.entry_id
            .cmp(&right.entry_id)
            .then_with(|| left.revision.cmp(&right.revision))
    });

    (active_entries, stale_count)
}

/// Search across Mind artifacts for a query string.
pub fn collect_mind_search_hits(
    snapshot: &MindArtifactDrilldown,
    query: &str,
) -> Vec<MindSearchHit> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();

    // Search handshake entries
    for entry in &snapshot.handshake_entries {
        if entry.summary.to_lowercase().contains(&q)
            || entry.entry_id.to_lowercase().contains(&q)
            || entry
                .topic
                .as_ref()
                .map(|t| t.to_lowercase())
                .unwrap_or_default()
                .contains(&q)
        {
            hits.push(MindSearchHit {
                kind: "handshake",
                title: entry
                    .topic
                    .clone()
                    .unwrap_or_else(|| entry.entry_id.clone()),
                summary: truncate(&entry.summary, 120),
                detail: vec![
                    format!("entry_id: {}", entry.entry_id),
                    format!("revision: {}", entry.revision),
                ],
                score: score_hit(&q, &[&entry.summary, entry.topic.as_deref().unwrap_or("")]),
            });
        }
    }

    // Search canon entries
    for entry in &snapshot.active_canon_entries {
        if entry.summary.to_lowercase().contains(&q)
            || entry.entry_id.to_lowercase().contains(&q)
            || entry
                .topic
                .as_ref()
                .map(|t| t.to_lowercase())
                .unwrap_or_default()
                .contains(&q)
        {
            hits.push(MindSearchHit {
                kind: "canon",
                title: entry
                    .topic
                    .clone()
                    .unwrap_or_else(|| entry.entry_id.clone()),
                summary: truncate(&entry.summary, 120),
                detail: vec![
                    format!("entry_id: {}", entry.entry_id),
                    format!("revision: {}", entry.revision),
                    format!("evidence: {}", entry.evidence_refs.len()),
                ],
                score: score_hit(&q, &[&entry.summary, entry.topic.as_deref().unwrap_or("")]) * 2,
            });
        }
    }

    // Search export manifest
    if let Some(export) = &snapshot.latest_export {
        if export.t3_job_id.to_lowercase().contains(&q)
            || export.session_id.to_lowercase().contains(&q)
        {
            hits.push(MindSearchHit {
                kind: "export",
                title: format!("export (t3:{})", export.t3_job_id),
                summary: format!(
                    "Session {} exported at {}",
                    export.session_id, export.exported_at
                ),
                detail: vec![
                    format!("t1_count: {}", export.t1_count),
                    format!("t2_count: {}", export.t2_count),
                ],
                score: 1,
            });
        }
    }

    hits.sort_by_key(|h| std::cmp::Reverse(h.score));
    hits
}

/// Score a hit by matching frequency.
fn score_hit(query: &str, fields: &[&str]) -> usize {
    let mut total = 0;
    for field in fields {
        if field.is_empty() {
            continue;
        }
        let fl = field.to_lowercase();
        if fl == query {
            total += 10;
        } else if fl.starts_with(query) {
            total += 5;
        } else if fl.contains(query) {
            total += 1;
        }
    }
    total
}

/// Stale canon entries from the snapshot (accessor for render module).
pub fn canon_stale_entries(_snapshot: &MindArtifactDrilldown) -> Vec<MindCanonEntry> {
    // Note: stale entries are not stored separately in this module; this is a stub
    // for the render search function. The actual stale count is tracked in
    // MindArtifactDrilldown::stale_canon_count.
    // In the full implementation, stale canon would be loaded from a separate path.
    vec![]
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canon_key_format() {
        assert_eq!(canon_key("abc", 3), "abc#3");
    }

    #[test]
    fn parse_handshake_empty() {
        assert!(parse_handshake_entries("[]").is_empty());
    }

    #[test]
    fn parse_handshake_single() {
        let payload = r#"[{"entry_id":"h1","revision":1,"topic":"test","summary":"a b c"}]"#;
        let entries = parse_handshake_entries(payload);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_id, "h1");
        assert_eq!(entries[0].revision, 1);
    }

    #[test]
    fn parse_handshake_markdown_single() {
        let payload = "- [h1 r3] topic=arch :: system architecture summary";
        let entries = parse_handshake_entries(payload);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_id, "h1");
        assert_eq!(entries[0].revision, 3);
        assert_eq!(entries[0].topic.as_deref(), Some("arch"));
        assert_eq!(entries[0].summary, "system architecture summary");
    }

    #[test]
    fn parse_canon_with_stale() {
        let payload = r#"[
            {"entry_id":"c1","revision":1,"topic":"x","summary":"s1","stale":false},
            {"entry_id":"c2","revision":2,"topic":"y","summary":"s2","stale":true}
        ]"#;
        let (entries, stale) = parse_project_canon_entries(payload);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_id, "c1");
        assert_eq!(stale, 1);
    }

    #[test]
    fn parse_canon_markdown_with_stale() {
        let payload = r#"
## Active canon
### c1 r1
- topic: architecture
- evidence_refs: e1, e2
Primary summary

## Stale canon
### c2 r2
- topic: old
Stale summary
"#;
        let (entries, stale) = parse_project_canon_entries(payload);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_id, "c1");
        assert_eq!(entries[0].revision, 1);
        assert_eq!(entries[0].topic.as_deref(), Some("architecture"));
        assert_eq!(entries[0].evidence_refs, vec!["e1", "e2"]);
        assert_eq!(entries[0].summary, "Primary summary");
        assert_eq!(stale, 1);
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let snap = MindArtifactDrilldown::default();
        let hits = collect_mind_search_hits(&snap, "");
        assert!(hits.is_empty());
    }

    #[test]
    fn search_finds_handshake_entry() {
        let snap = MindArtifactDrilldown {
            handshake_entries: vec![MindHandshakeEntry {
                entry_id: "h1".into(),
                revision: 1,
                topic: Some("Architecture".into()),
                summary: "System architecture overview".into(),
            }],
            ..Default::default()
        };
        let hits = collect_mind_search_hits(&snap, "architecture");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].kind, "handshake");
    }
}
