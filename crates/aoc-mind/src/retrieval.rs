use crate::SessionExportManifest;
use aoc_core::insight_contracts::{
    InsightRetrievalCitation, InsightRetrievalDrilldownRef, InsightRetrievalHit,
    InsightRetrievalMode, InsightRetrievalRequest, InsightRetrievalResult, InsightRetrievalScope,
};
use std::path::{Path, PathBuf};

const INSIGHT_RETRIEVAL_MAX_RESULTS_DEFAULT: usize = 4;
const INSIGHT_RETRIEVAL_MAX_RESULTS_CAP: usize = 8;
const INSIGHT_RETRIEVAL_SESSION_EXPORT_SCAN_LIMIT: usize = 6;
const INSIGHT_RETRIEVAL_BRIEF_LINE_BUDGET: usize = 2;
const INSIGHT_RETRIEVAL_REFS_LINE_BUDGET: usize = 0;
const INSIGHT_RETRIEVAL_SNIPS_LINE_BUDGET: usize = 5;

#[derive(Debug, Clone)]
struct InsightRetrievalSource {
    source_id: String,
    scope: InsightRetrievalScope,
    label: String,
    reference: String,
    lines: Vec<String>,
    citations: Vec<InsightRetrievalCitation>,
    drilldown_refs: Vec<InsightRetrievalDrilldownRef>,
    score_bias: i64,
}

pub fn compile_insight_retrieval(
    project_root: &str,
    request: InsightRetrievalRequest,
) -> InsightRetrievalResult {
    let active_tag = request
        .active_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let resolved_scope = match request.scope {
        InsightRetrievalScope::Session => InsightRetrievalScope::Session,
        InsightRetrievalScope::Project => InsightRetrievalScope::Project,
        InsightRetrievalScope::Auto => {
            if load_session_export_manifests(
                project_root,
                INSIGHT_RETRIEVAL_SESSION_EXPORT_SCAN_LIMIT,
            )
            .ok()
            .map(|manifests| {
                manifests.into_iter().any(|manifest| {
                    export_matches_active_tag(manifest.active_tag.as_deref(), active_tag.as_deref())
                })
            })
            .unwrap_or(false)
            {
                InsightRetrievalScope::Auto
            } else {
                InsightRetrievalScope::Project
            }
        }
    };

    let max_results = request
        .max_results
        .unwrap_or(INSIGHT_RETRIEVAL_MAX_RESULTS_DEFAULT)
        .clamp(1, INSIGHT_RETRIEVAL_MAX_RESULTS_CAP);
    let sources =
        collect_insight_retrieval_sources(project_root, resolved_scope, active_tag.as_deref());
    let mut hits = sources
        .into_iter()
        .filter_map(|source| {
            rank_insight_retrieval_source(&request.query, &request.mode, max_results, source)
        })
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    if hits.len() > max_results {
        hits.truncate(max_results);
    }

    let citations = hits
        .iter()
        .flat_map(|hit| hit.citations.iter().cloned())
        .collect::<Vec<_>>();

    let fallback_used = hits.is_empty();
    let status = if fallback_used { "fallback" } else { "ok" }.to_string();
    let summary_lines = if fallback_used {
        vec![format!(
            "no retrieval hits for query '{}' in {:?} scope",
            request.query, resolved_scope
        )]
    } else {
        match request.mode {
            InsightRetrievalMode::Brief => hits
                .iter()
                .map(|hit| format!("{} [{}]", hit.label, hit.reference))
                .collect(),
            InsightRetrievalMode::Refs => hits
                .iter()
                .map(|hit| {
                    let drilldown = hit
                        .drilldown_refs
                        .iter()
                        .map(|item| format!("{}:{}", item.kind, item.reference))
                        .collect::<Vec<_>>()
                        .join(", ");
                    if drilldown.is_empty() {
                        format!("{} -> {}", hit.label, hit.reference)
                    } else {
                        format!("{} -> {} ({})", hit.label, hit.reference, drilldown)
                    }
                })
                .collect(),
            InsightRetrievalMode::Snips => hits
                .iter()
                .map(|hit| {
                    let preview = hit.lines.first().cloned().unwrap_or_default();
                    format!("{} -> {}", hit.label, preview)
                })
                .collect(),
        }
    };
    let line_budget_per_hit = insight_retrieval_line_budget(&request.mode, max_results);

    InsightRetrievalResult {
        query: request.query,
        scope: request.scope,
        resolved_scope,
        mode: request.mode,
        status,
        summary_lines,
        hits,
        citations,
        fallback_used,
        hit_budget: max_results,
        line_budget_per_hit,
    }
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

fn collect_insight_retrieval_sources(
    project_root: &str,
    scope: InsightRetrievalScope,
    active_tag: Option<&str>,
) -> Vec<InsightRetrievalSource> {
    let mut sources = Vec::new();

    if matches!(
        scope,
        InsightRetrievalScope::Project | InsightRetrievalScope::Auto
    ) {
        if let Some(text) = read_optional_text(
            &PathBuf::from(project_root)
                .join(".aoc")
                .join("mind")
                .join("t3")
                .join("project_mind.md"),
        ) {
            sources.extend(parse_project_mind_retrieval_sources(&text, active_tag));
        }
    }

    if matches!(
        scope,
        InsightRetrievalScope::Session | InsightRetrievalScope::Auto
    ) {
        if let Ok(manifests) =
            load_session_export_manifests(project_root, INSIGHT_RETRIEVAL_SESSION_EXPORT_SCAN_LIMIT)
        {
            for (index, manifest) in manifests.into_iter().enumerate() {
                if !export_matches_active_tag(manifest.active_tag.as_deref(), active_tag) {
                    continue;
                }
                let export_dir = PathBuf::from(&manifest.export_dir);
                let recency_bias =
                    (INSIGHT_RETRIEVAL_SESSION_EXPORT_SCAN_LIMIT.saturating_sub(index)) as i64;
                if let Some(text) = read_optional_text(&export_dir.join("t2.md")) {
                    sources.extend(parse_session_export_retrieval_sources(
                        &text,
                        "t2",
                        &manifest,
                        &format!("{}/t2.md", manifest.export_dir),
                        recency_bias,
                    ));
                }
                if let Some(text) = read_optional_text(&export_dir.join("t1.md")) {
                    sources.extend(parse_session_export_retrieval_sources(
                        &text,
                        "t1",
                        &manifest,
                        &format!("{}/t1.md", manifest.export_dir),
                        recency_bias,
                    ));
                }
            }
        }
    }

    sources
}

fn load_session_export_manifests(
    project_root: &str,
    limit: usize,
) -> Result<Vec<SessionExportManifest>, String> {
    let insight_root = PathBuf::from(project_root)
        .join(".aoc")
        .join("mind")
        .join("insight");
    let entries = std::fs::read_dir(&insight_root)
        .map_err(|err| format!("read insight export dir failed: {err}"))?;

    let mut manifests = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .filter_map(|dir| {
            let manifest_path = dir.join("manifest.json");
            let payload = std::fs::read_to_string(&manifest_path).ok()?;
            let manifest = serde_json::from_str::<SessionExportManifest>(&payload).ok()?;
            Some((dir, manifest))
        })
        .collect::<Vec<_>>();

    manifests.sort_by(|left, right| {
        right
            .1
            .exported_at
            .cmp(&left.1.exported_at)
            .then_with(|| right.0.cmp(&left.0))
    });

    Ok(manifests
        .into_iter()
        .take(limit.max(1))
        .map(|(_, manifest)| manifest)
        .collect())
}

fn parse_project_mind_retrieval_sources(
    text: &str,
    active_tag: Option<&str>,
) -> Vec<InsightRetrievalSource> {
    let requested = active_tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let mut sources = Vec::new();
    let mut state = "active";
    let mut heading: Option<String> = None;
    let mut topic: Option<String> = None;
    let mut evidence_refs: Vec<String> = Vec::new();
    let mut body_lines: Vec<String> = Vec::new();

    let flush =
        |sources: &mut Vec<InsightRetrievalSource>,
         heading: &mut Option<String>,
         topic: &mut Option<String>,
         evidence_refs: &mut Vec<String>,
         body_lines: &mut Vec<String>,
         state: &str,
         requested: Option<&String>| {
            let Some(entry_heading) = heading.take() else {
                topic.take();
                evidence_refs.clear();
                body_lines.clear();
                return;
            };
            let topic_value = topic.take();
            let include = match (requested, topic_value.as_deref()) {
                (None, _) => true,
                (Some(requested), Some(topic)) => {
                    let topic = topic.trim().to_ascii_lowercase();
                    topic == *requested || topic == "global"
                }
                (Some(_), None) => false,
            };
            if !include {
                evidence_refs.clear();
                body_lines.clear();
                return;
            }

            let mut lines = vec![entry_heading.clone()];
            if let Some(topic) = topic_value.as_deref() {
                lines.push(format!("- topic: {topic}"));
            }
            lines.extend(body_lines.iter().cloned());
            let lines = lines
                .into_iter()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>();
            if lines.is_empty() {
                evidence_refs.clear();
                body_lines.clear();
                return;
            }

            let entry_id = entry_heading
                .trim_start_matches("### ")
                .split_whitespace()
                .next()
                .unwrap_or("canon-entry")
                .to_string();
            let reference = format!(".aoc/mind/t3/project_mind.md#{entry_id}");
            let mut citations = vec![InsightRetrievalCitation {
                source_id: format!("t3_canon:{entry_id}"),
                label: format!("Canon entry {entry_id}"),
                reference: reference.clone(),
                score: 0,
            }];
            citations.extend(
                evidence_refs
                    .iter()
                    .map(|evidence| InsightRetrievalCitation {
                        source_id: evidence.clone(),
                        label: format!("Evidence {evidence}"),
                        reference: evidence.clone(),
                        score: 0,
                    }),
            );

            let mut drilldown_refs = vec![InsightRetrievalDrilldownRef {
                kind: "canon_entry".to_string(),
                label: format!("Canon entry {entry_id}"),
                reference: reference.clone(),
            }];
            drilldown_refs.extend(evidence_refs.iter().map(|evidence| {
                InsightRetrievalDrilldownRef {
                    kind: "evidence_ref".to_string(),
                    label: format!("Evidence {evidence}"),
                    reference: evidence.clone(),
                }
            }));

            sources.push(InsightRetrievalSource {
                source_id: format!("t3_canon:{entry_id}"),
                scope: InsightRetrievalScope::Project,
                label: format!("Project canon {entry_id} ({state})"),
                reference,
                lines,
                citations,
                drilldown_refs,
                score_bias: if state == "active" { 20 } else { -10 },
            });
            evidence_refs.clear();
            body_lines.clear();
        };

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("## Active canon") {
            flush(
                &mut sources,
                &mut heading,
                &mut topic,
                &mut evidence_refs,
                &mut body_lines,
                state,
                requested.as_ref(),
            );
            state = "active";
            continue;
        }
        if line.starts_with("## Stale canon") {
            flush(
                &mut sources,
                &mut heading,
                &mut topic,
                &mut evidence_refs,
                &mut body_lines,
                state,
                requested.as_ref(),
            );
            state = "stale";
            continue;
        }
        if line.starts_with("### ") {
            flush(
                &mut sources,
                &mut heading,
                &mut topic,
                &mut evidence_refs,
                &mut body_lines,
                state,
                requested.as_ref(),
            );
            heading = Some(line.to_string());
            continue;
        }
        if heading.is_none() || line.is_empty() || line == "(none)" {
            continue;
        }
        if let Some(value) = line.strip_prefix("- topic:") {
            topic = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("- evidence_refs:") {
            evidence_refs = value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .collect();
            continue;
        }
        if line.starts_with('-') {
            body_lines.push(line.to_string());
            continue;
        }
        body_lines.push(line.to_string());
    }

    flush(
        &mut sources,
        &mut heading,
        &mut topic,
        &mut evidence_refs,
        &mut body_lines,
        state,
        requested.as_ref(),
    );
    sources
}

fn parse_session_export_retrieval_sources(
    text: &str,
    kind: &str,
    manifest: &SessionExportManifest,
    reference: &str,
    recency_bias: i64,
) -> Vec<InsightRetrievalSource> {
    let mut sources = Vec::new();
    let mut heading: Option<String> = None;
    let mut body_lines = Vec::new();

    let flush = |sources: &mut Vec<InsightRetrievalSource>,
                 heading: &mut Option<String>,
                 body_lines: &mut Vec<String>| {
        let title = heading.take();
        let lines = body_lines
            .iter()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && line != "(empty)")
            .collect::<Vec<_>>();
        body_lines.clear();
        if lines.is_empty() {
            return;
        }

        let label = match title.as_deref() {
            Some(value) => format!(
                "Session {} {}",
                kind.to_uppercase(),
                value.trim_start_matches("## ")
            ),
            None => format!("Session {} export", kind.to_uppercase()),
        };
        let source_id = match title.as_deref() {
            Some(value) => {
                let artifact_id = value
                    .trim_start_matches("## ")
                    .split_whitespace()
                    .next()
                    .unwrap_or(kind);
                format!("session_{}:{}", kind, artifact_id)
            }
            None => format!("session_{}:{}", kind, manifest.session_id),
        };
        let mut citations = vec![InsightRetrievalCitation {
            source_id: source_id.clone(),
            label: label.clone(),
            reference: reference.to_string(),
            score: 0,
        }];
        citations.push(InsightRetrievalCitation {
            source_id: format!("session:{}", manifest.session_id),
            label: format!("Session {}", manifest.session_id),
            reference: manifest.export_dir.clone(),
            score: 0,
        });
        let drilldown_refs = vec![
            InsightRetrievalDrilldownRef {
                kind: "export_file".to_string(),
                label: format!("{} export file", kind.to_uppercase()),
                reference: reference.to_string(),
            },
            InsightRetrievalDrilldownRef {
                kind: "session_export".to_string(),
                label: format!("Session {} export dir", manifest.session_id),
                reference: manifest.export_dir.clone(),
            },
        ];
        sources.push(InsightRetrievalSource {
            source_id,
            scope: InsightRetrievalScope::Session,
            label,
            reference: reference.to_string(),
            lines,
            citations,
            drilldown_refs,
            score_bias: if kind == "t2" {
                8 + recency_bias
            } else {
                4 + recency_bias
            },
        });
    };

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("# ") {
            continue;
        }
        if line.starts_with("## ") {
            flush(&mut sources, &mut heading, &mut body_lines);
            heading = Some(line.to_string());
            continue;
        }
        if line.is_empty() {
            flush(&mut sources, &mut heading, &mut body_lines);
            continue;
        }
        body_lines.push(line.to_string());
    }
    flush(&mut sources, &mut heading, &mut body_lines);

    if sources.is_empty() {
        let lines = extract_nonempty_lines(text, 48);
        if !lines.is_empty() {
            sources.push(InsightRetrievalSource {
                source_id: format!("session_{}:{}", kind, manifest.session_id),
                scope: InsightRetrievalScope::Session,
                label: format!("Session {} export", kind.to_uppercase()),
                reference: reference.to_string(),
                lines,
                citations: vec![InsightRetrievalCitation {
                    source_id: format!("session:{}", manifest.session_id),
                    label: format!("Session {}", manifest.session_id),
                    reference: manifest.export_dir.clone(),
                    score: 0,
                }],
                drilldown_refs: vec![InsightRetrievalDrilldownRef {
                    kind: "session_export".to_string(),
                    label: format!("Session {} export dir", manifest.session_id),
                    reference: manifest.export_dir.clone(),
                }],
                score_bias: if kind == "t2" {
                    8 + recency_bias
                } else {
                    4 + recency_bias
                },
            });
        }
    }

    sources
}

fn insight_retrieval_line_budget(mode: &InsightRetrievalMode, max_results: usize) -> usize {
    match mode {
        InsightRetrievalMode::Brief => INSIGHT_RETRIEVAL_BRIEF_LINE_BUDGET,
        InsightRetrievalMode::Refs => INSIGHT_RETRIEVAL_REFS_LINE_BUDGET,
        InsightRetrievalMode::Snips => INSIGHT_RETRIEVAL_SNIPS_LINE_BUDGET,
    }
    .min(
        max_results
            .saturating_mul(INSIGHT_RETRIEVAL_SNIPS_LINE_BUDGET)
            .max(1),
    )
}

fn rank_insight_retrieval_source(
    query: &str,
    mode: &InsightRetrievalMode,
    max_results: usize,
    source: InsightRetrievalSource,
) -> Option<InsightRetrievalHit> {
    let terms = query
        .split_whitespace()
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let score_line = |line: &str| {
        let normalized = line.to_ascii_lowercase();
        let term_hits = terms
            .iter()
            .map(|term| {
                if normalized.contains(term) {
                    10 + term.len() as i64
                } else {
                    0
                }
            })
            .sum::<i64>();
        let heading_bonus = if normalized.starts_with("### ") || normalized.starts_with("## ") {
            6
        } else {
            0
        };
        term_hits + heading_bonus
    };

    let mut matched = source
        .lines
        .iter()
        .filter_map(|line| {
            let score = if terms.is_empty() { 1 } else { score_line(line) };
            if score > 0 {
                Some((line.clone(), score))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if matched.is_empty() {
        return None;
    }
    matched.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let line_budget = insight_retrieval_line_budget(mode, max_results);

    let lines = match mode {
        InsightRetrievalMode::Refs => Vec::new(),
        _ => matched
            .iter()
            .take(line_budget)
            .map(|(line, _)| line.clone())
            .collect(),
    };
    let lines_truncated =
        matched.len() > line_budget && !matches!(mode, InsightRetrievalMode::Refs);
    let score = source.score_bias
        + matched
            .iter()
            .take(line_budget.max(1))
            .map(|(_, score)| *score)
            .sum::<i64>();
    let citations = source
        .citations
        .into_iter()
        .map(|citation| InsightRetrievalCitation { score, ..citation })
        .collect();

    Some(InsightRetrievalHit {
        source_id: source.source_id,
        scope: source.scope,
        label: source.label,
        reference: source.reference,
        score,
        lines,
        citations,
        drilldown_refs: source.drilldown_refs,
        line_budget,
        lines_truncated,
    })
}

fn extract_nonempty_lines(text: &str, max_lines: usize) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| *line != "(none)" && *line != "(empty)")
        .filter(|line| !line.starts_with("generated_at:") && !line.starts_with("_generated_at:"))
        .take(max_lines)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
}

fn read_optional_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
