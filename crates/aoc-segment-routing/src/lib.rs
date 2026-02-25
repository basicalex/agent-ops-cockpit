use aoc_core::mind_contracts::{
    ArtifactTaskLink, ArtifactTaskRelation, MindContractError, RouteOrigin, SegmentCandidate,
    SegmentRoute,
};
use aoc_storage::{ConversationContextState, MindStore, StorageError, StoredArtifact};
use std::cmp::max;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const ROUTE_CONF_TASKMASTER: u16 = 9_600;
const ROUTE_CONF_UNCERTAIN: u16 = 5_300;
const ROUTE_CONF_GLOBAL_FALLBACK: u16 = 5_000;

#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("contract error: {0}")]
    Contract(#[from] MindContractError),
    #[error("invalid override patch for artifact {artifact_id}: {reason}")]
    InvalidOverridePatch { artifact_id: String, reason: String },
}

#[derive(Debug, Clone)]
pub struct SegmentRoutingConfig {
    pub tag_to_segment: BTreeMap<String, String>,
    pub task_to_segment: BTreeMap<String, String>,
    pub segment_keywords: BTreeMap<String, Vec<String>>,
    pub low_confidence_threshold_bps: u16,
    pub ambiguous_delta_bps: u16,
    pub default_global_segment: String,
    pub default_uncertain_segment: String,
    pub max_secondary_segments: usize,
}

impl Default for SegmentRoutingConfig {
    fn default() -> Self {
        let mut tag_to_segment = BTreeMap::new();
        tag_to_segment.insert("mind".to_string(), "mind".to_string());

        let mut segment_keywords = BTreeMap::new();
        segment_keywords.insert(
            "frontend".to_string(),
            vec!["ui".to_string(), "component".to_string(), "css".to_string()],
        );
        segment_keywords.insert(
            "backend".to_string(),
            vec!["api".to_string(), "db".to_string(), "migration".to_string()],
        );
        segment_keywords.insert(
            "mind".to_string(),
            vec![
                "observation".to_string(),
                "reflection".to_string(),
                "taskmaster".to_string(),
            ],
        );

        Self {
            tag_to_segment,
            task_to_segment: BTreeMap::new(),
            segment_keywords,
            low_confidence_threshold_bps: 6_500,
            ambiguous_delta_bps: 350,
            default_global_segment: "global".to_string(),
            default_uncertain_segment: "uncertain".to_string(),
            max_secondary_segments: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteOverridePatch {
    pub patch_id: String,
    pub primary_segment: String,
    pub secondary_segments: Vec<String>,
    pub reason: String,
    pub confidence_bps: u16,
}

impl Default for RouteOverridePatch {
    fn default() -> Self {
        Self {
            patch_id: "manual".to_string(),
            primary_segment: "global".to_string(),
            secondary_segments: Vec::new(),
            reason: "manual override".to_string(),
            confidence_bps: 10_000,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RoutingReport {
    pub artifacts_processed: usize,
    pub routes_written: usize,
    pub routed_taskmaster: usize,
    pub routed_heuristic: usize,
    pub routed_override: usize,
    pub uncertain_fallbacks: usize,
}

#[derive(Debug, Clone)]
struct ScoredSegment {
    segment_id: String,
    confidence_bps: u16,
    reasons: BTreeSet<String>,
}

pub struct SegmentRouter {
    config: SegmentRoutingConfig,
    overrides: BTreeMap<String, RouteOverridePatch>,
}

impl SegmentRouter {
    pub fn new(config: SegmentRoutingConfig) -> Self {
        Self {
            config,
            overrides: BTreeMap::new(),
        }
    }

    pub fn with_overrides(
        config: SegmentRoutingConfig,
        overrides: BTreeMap<String, RouteOverridePatch>,
    ) -> Self {
        Self { config, overrides }
    }

    pub fn route_conversation(
        &self,
        store: &MindStore,
        conversation_id: &str,
    ) -> Result<RoutingReport, RoutingError> {
        let artifacts = store.artifacts_for_conversation(conversation_id)?;
        if artifacts.is_empty() {
            return Ok(RoutingReport::default());
        }

        let contexts = store.context_states(conversation_id)?;
        let mut report = RoutingReport::default();
        let mut context_cursor = 0usize;
        let mut current_context: Option<&ConversationContextState> = None;

        for artifact in artifacts {
            report.artifacts_processed += 1;
            while context_cursor < contexts.len() && contexts[context_cursor].ts <= artifact.ts {
                current_context = Some(&contexts[context_cursor]);
                context_cursor += 1;
            }

            let task_links = store.artifact_task_links_for_artifact(&artifact.artifact_id)?;
            let auto_route = self.compute_auto_route(&artifact, current_context, &task_links)?;
            let route = if let Some(patch) = self.overrides.get(&artifact.artifact_id) {
                self.apply_override(auto_route, patch)?
            } else {
                auto_route
            };

            if eq_segment(
                &route.primary.segment_id,
                self.config.default_uncertain_segment.as_str(),
            ) {
                report.uncertain_fallbacks += 1;
            }

            match route.routed_by {
                RouteOrigin::Taskmaster => report.routed_taskmaster += 1,
                RouteOrigin::Heuristic => report.routed_heuristic += 1,
                RouteOrigin::ManualOverride => report.routed_override += 1,
            }

            store.replace_segment_route(&route)?;
            report.routes_written += 1;
        }

        Ok(report)
    }

    fn compute_auto_route(
        &self,
        artifact: &StoredArtifact,
        context: Option<&ConversationContextState>,
        task_links: &[ArtifactTaskLink],
    ) -> Result<SegmentRoute, RoutingError> {
        if let Some(active_tag) = context
            .and_then(|snapshot| snapshot.active_tag.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some(segment_id) = lookup_segment(&self.config.tag_to_segment, active_tag) {
                let candidate_pool = self.heuristic_candidates(artifact, task_links);
                let mut secondary = Vec::new();
                for candidate in candidate_pool {
                    if secondary.len() >= self.config.max_secondary_segments {
                        break;
                    }
                    if eq_segment(&candidate.segment_id, &segment_id) {
                        continue;
                    }
                    secondary.push(SegmentCandidate::new(
                        candidate.segment_id,
                        candidate.confidence_bps.min(8_800),
                    )?);
                }

                return Ok(SegmentRoute {
                    artifact_id: artifact.artifact_id.clone(),
                    primary: SegmentCandidate::new(segment_id.clone(), ROUTE_CONF_TASKMASTER)?,
                    secondary,
                    routed_by: RouteOrigin::Taskmaster,
                    reason: format!(
                        "taskmaster_tag_map:tag={active_tag}->segment={segment_id}; source=context_state"
                    ),
                    overridden_by: None,
                });
            }
        }

        self.compute_heuristic_route(artifact, task_links)
    }

    fn compute_heuristic_route(
        &self,
        artifact: &StoredArtifact,
        task_links: &[ArtifactTaskLink],
    ) -> Result<SegmentRoute, RoutingError> {
        let candidates = self.heuristic_candidates(artifact, task_links);
        let Some(top) = candidates.first() else {
            return self.uncertain_fallback(
                artifact,
                "fallback:uncertain:no_taskmaster_signal_or_heuristic_match".to_string(),
                &[],
            );
        };

        let ambiguous = candidates.get(1).is_some_and(|second| {
            top.confidence_bps.saturating_sub(second.confidence_bps)
                <= self.config.ambiguous_delta_bps
        });
        let low_confidence = top.confidence_bps < self.config.low_confidence_threshold_bps;

        if low_confidence || ambiguous {
            let reason = format!(
                "fallback:uncertain:top={}({}) low_confidence={} ambiguous={} evidence={}",
                top.segment_id,
                top.confidence_bps,
                low_confidence,
                ambiguous,
                join_reasons(top)
            );
            return self.uncertain_fallback(artifact, reason, &candidates);
        }

        let primary = SegmentCandidate::new(top.segment_id.clone(), top.confidence_bps)?;
        let mut secondary = Vec::new();
        for candidate in candidates.iter().skip(1) {
            if secondary.len() >= self.config.max_secondary_segments {
                break;
            }
            secondary.push(SegmentCandidate::new(
                candidate.segment_id.clone(),
                candidate.confidence_bps,
            )?);
        }

        Ok(SegmentRoute {
            artifact_id: artifact.artifact_id.clone(),
            primary,
            secondary,
            routed_by: RouteOrigin::Heuristic,
            reason: format!(
                "heuristic_route:top={}({}) evidence={}",
                top.segment_id,
                top.confidence_bps,
                join_reasons(top)
            ),
            overridden_by: None,
        })
    }

    fn uncertain_fallback(
        &self,
        artifact: &StoredArtifact,
        reason: String,
        candidates: &[ScoredSegment],
    ) -> Result<SegmentRoute, RoutingError> {
        let uncertain_segment = normalize_segment(self.config.default_uncertain_segment.as_str())
            .unwrap_or_else(|| "uncertain".to_string());
        let global_segment = normalize_segment(self.config.default_global_segment.as_str())
            .unwrap_or_else(|| "global".to_string());

        let mut secondary = Vec::new();
        let mut seen = BTreeSet::new();

        for candidate in candidates {
            if secondary.len() >= self.config.max_secondary_segments {
                break;
            }
            if !seen.insert(normalized_key(candidate.segment_id.as_str())) {
                continue;
            }
            if eq_segment(candidate.segment_id.as_str(), uncertain_segment.as_str()) {
                continue;
            }
            secondary.push(SegmentCandidate::new(
                candidate.segment_id.clone(),
                candidate.confidence_bps,
            )?);
        }

        if !secondary
            .iter()
            .any(|candidate| eq_segment(candidate.segment_id.as_str(), global_segment.as_str()))
        {
            secondary.push(SegmentCandidate::new(
                global_segment,
                ROUTE_CONF_GLOBAL_FALLBACK,
            )?);
        }

        Ok(SegmentRoute {
            artifact_id: artifact.artifact_id.clone(),
            primary: SegmentCandidate::new(uncertain_segment, ROUTE_CONF_UNCERTAIN)?,
            secondary,
            routed_by: RouteOrigin::Heuristic,
            reason,
            overridden_by: None,
        })
    }

    fn heuristic_candidates(
        &self,
        artifact: &StoredArtifact,
        task_links: &[ArtifactTaskLink],
    ) -> Vec<ScoredSegment> {
        let mut scores = BTreeMap::<String, ScoredSegment>::new();

        for link in task_links {
            if let Some(segment_id) = lookup_segment(&self.config.task_to_segment, &link.task_id) {
                let score = task_link_score(link.relation, link.confidence_bps);
                upsert_score(
                    &mut scores,
                    segment_id,
                    score,
                    format!(
                        "task_link:{} relation={} conf={}",
                        link.task_id,
                        relation_label(link.relation),
                        link.confidence_bps
                    ),
                );
            }
        }

        let lower_text = artifact.text.to_lowercase();
        for (segment_id, keywords) in &self.config.segment_keywords {
            let normalized_segment = normalize_segment(segment_id).unwrap_or_default();
            if normalized_segment.is_empty() {
                continue;
            }

            let hits = keywords
                .iter()
                .map(|keyword| keyword.trim().to_lowercase())
                .filter(|keyword| !keyword.is_empty() && lower_text.contains(keyword))
                .collect::<Vec<_>>();

            if hits.is_empty() {
                continue;
            }

            let score = keyword_score(hits.len());
            upsert_score(
                &mut scores,
                normalized_segment,
                score,
                format!("keyword_match:{}", hits.join("+")),
            );
        }

        let mut ordered = scores.into_values().collect::<Vec<_>>();
        ordered.sort_by(|left, right| {
            right
                .confidence_bps
                .cmp(&left.confidence_bps)
                .then(left.segment_id.cmp(&right.segment_id))
        });
        ordered
    }

    fn apply_override(
        &self,
        auto_route: SegmentRoute,
        patch: &RouteOverridePatch,
    ) -> Result<SegmentRoute, RoutingError> {
        let patch_id = patch.patch_id.trim();
        if patch_id.is_empty() {
            return Err(RoutingError::InvalidOverridePatch {
                artifact_id: auto_route.artifact_id,
                reason: "patch_id is required".to_string(),
            });
        }

        let Some(primary_segment) = normalize_segment(patch.primary_segment.as_str()) else {
            return Err(RoutingError::InvalidOverridePatch {
                artifact_id: auto_route.artifact_id,
                reason: "primary_segment is required".to_string(),
            });
        };

        let primary = SegmentCandidate::new(primary_segment.clone(), patch.confidence_bps)?;
        let mut secondary = Vec::new();
        let mut seen = BTreeSet::new();
        seen.insert(normalized_key(primary_segment.as_str()));

        let base_secondary_conf = patch.confidence_bps.saturating_sub(800);
        for (index, segment_id) in patch.secondary_segments.iter().enumerate() {
            if secondary.len() >= self.config.max_secondary_segments {
                break;
            }
            let Some(segment_id) = normalize_segment(segment_id) else {
                continue;
            };
            if !seen.insert(normalized_key(segment_id.as_str())) {
                continue;
            }
            let conf = max(
                ROUTE_CONF_GLOBAL_FALLBACK,
                base_secondary_conf.saturating_sub((index as u16).saturating_mul(300)),
            );
            secondary.push(SegmentCandidate::new(segment_id, conf)?);
        }

        if secondary.len() < self.config.max_secondary_segments
            && seen.insert(normalized_key(auto_route.primary.segment_id.as_str()))
        {
            secondary.push(auto_route.primary.clone());
        }

        for candidate in auto_route.secondary {
            if secondary.len() >= self.config.max_secondary_segments {
                break;
            }
            if seen.insert(normalized_key(candidate.segment_id.as_str())) {
                secondary.push(candidate);
            }
        }

        Ok(SegmentRoute {
            artifact_id: auto_route.artifact_id,
            primary,
            secondary,
            routed_by: RouteOrigin::ManualOverride,
            reason: format!(
                "override_patch:{patch_id}:{}; base={}",
                patch.reason.trim(),
                auto_route.reason
            ),
            overridden_by: Some(patch_id.to_string()),
        })
    }
}

fn lookup_segment(mapping: &BTreeMap<String, String>, key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }
    mapping
        .get(trimmed)
        .or_else(|| mapping.get(trimmed.to_lowercase().as_str()))
        .and_then(|segment_id| normalize_segment(segment_id))
}

fn normalize_segment(value: &str) -> Option<String> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalized_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn eq_segment(left: &str, right: &str) -> bool {
    normalized_key(left) == normalized_key(right)
}

fn keyword_score(hit_count: usize) -> u16 {
    let score = 4_200_u16.saturating_add((hit_count as u16).saturating_mul(550));
    score.min(7_800)
}

fn task_link_score(relation: ArtifactTaskRelation, confidence_bps: u16) -> u16 {
    let relation_boost = match relation {
        ArtifactTaskRelation::Active => 1_300,
        ArtifactTaskRelation::WorkedOn => 1_100,
        ArtifactTaskRelation::Mentioned => 600,
        ArtifactTaskRelation::Completed => 900,
    };
    let weighted = ((u32::from(confidence_bps) * 75) / 100) as u16;
    weighted.saturating_add(relation_boost).min(9_200)
}

fn upsert_score(
    scores: &mut BTreeMap<String, ScoredSegment>,
    segment_id: String,
    confidence_bps: u16,
    reason: String,
) {
    let entry = scores
        .entry(segment_id.clone())
        .or_insert_with(|| ScoredSegment {
            segment_id,
            confidence_bps,
            reasons: BTreeSet::new(),
        });

    if confidence_bps > entry.confidence_bps {
        entry.confidence_bps = confidence_bps;
    }
    entry.reasons.insert(reason);
}

fn relation_label(relation: ArtifactTaskRelation) -> &'static str {
    match relation {
        ArtifactTaskRelation::Active => "active",
        ArtifactTaskRelation::WorkedOn => "worked_on",
        ArtifactTaskRelation::Mentioned => "mentioned",
        ArtifactTaskRelation::Completed => "completed",
    }
}

fn join_reasons(candidate: &ScoredSegment) -> String {
    candidate
        .reasons
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_storage::ConversationContextState;
    use chrono::{DateTime, TimeZone, Utc};

    fn ts(hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 23, hour, min, sec)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn routes_from_taskmaster_tag_map_when_context_is_present() {
        let store = MindStore::open_in_memory().expect("open store");
        store
            .append_context_state(&ConversationContextState {
                conversation_id: "conv-1".to_string(),
                ts: ts(12, 0, 0),
                active_tag: Some("mind".to_string()),
                active_tasks: vec![],
                lifecycle: Some("tag_current".to_string()),
                signal_task_ids: vec![],
                signal_source: "tm_tag_current_json".to_string(),
            })
            .expect("append context");
        store
            .insert_observation(
                "obs-1",
                "conv-1",
                ts(12, 0, 5),
                "observation for parser flow",
                &[],
            )
            .expect("insert observation");

        let router = SegmentRouter::new(SegmentRoutingConfig::default());
        let report = router
            .route_conversation(&store, "conv-1")
            .expect("route conversation");
        assert_eq!(report.artifacts_processed, 1);
        assert_eq!(report.routed_taskmaster, 1);

        let route = store
            .segment_route_for_artifact("obs-1")
            .expect("load route")
            .expect("route exists");
        assert_eq!(route.routed_by, RouteOrigin::Taskmaster);
        assert_eq!(route.primary.segment_id, "mind");
        assert!(route.primary.confidence_bps >= 9_000);
        assert!(route.reason.contains("taskmaster_tag_map"));
    }

    #[test]
    fn ambiguous_heuristics_fall_back_to_uncertain_and_global() {
        let store = MindStore::open_in_memory().expect("open store");
        store
            .insert_observation(
                "obs-2",
                "conv-2",
                ts(13, 0, 0),
                "ui api request parser",
                &[],
            )
            .expect("insert observation");

        let mut config = SegmentRoutingConfig::default();
        config.tag_to_segment.clear();
        config.segment_keywords.clear();
        config
            .segment_keywords
            .insert("frontend".to_string(), vec!["ui".to_string()]);
        config
            .segment_keywords
            .insert("backend".to_string(), vec!["api".to_string()]);

        let router = SegmentRouter::new(config.clone());
        let report = router
            .route_conversation(&store, "conv-2")
            .expect("route conversation");
        assert_eq!(report.routed_heuristic, 1);
        assert_eq!(report.uncertain_fallbacks, 1);

        let route = store
            .segment_route_for_artifact("obs-2")
            .expect("load route")
            .expect("route exists");
        assert_eq!(route.routed_by, RouteOrigin::Heuristic);
        assert_eq!(route.primary.segment_id, config.default_uncertain_segment);
        assert!(route.reason.contains("fallback:uncertain"));
        assert!(route
            .secondary
            .iter()
            .any(|candidate| candidate.segment_id == config.default_global_segment));
    }

    #[test]
    fn override_patch_rewrites_primary_and_keeps_provenance() {
        let store = MindStore::open_in_memory().expect("open store");
        store
            .append_context_state(&ConversationContextState {
                conversation_id: "conv-3".to_string(),
                ts: ts(14, 0, 0),
                active_tag: Some("mind".to_string()),
                active_tasks: vec![],
                lifecycle: Some("tag_current".to_string()),
                signal_task_ids: vec![],
                signal_source: "tm_tag_current_json".to_string(),
            })
            .expect("append context");
        store
            .insert_observation(
                "obs-3",
                "conv-3",
                ts(14, 0, 5),
                "observation for route override testing",
                &[],
            )
            .expect("insert observation");

        let mut overrides = BTreeMap::new();
        overrides.insert(
            "obs-3".to_string(),
            RouteOverridePatch {
                patch_id: "patch-frontend-1".to_string(),
                primary_segment: "frontend".to_string(),
                secondary_segments: vec!["mind".to_string()],
                reason: "manual regroup after review".to_string(),
                confidence_bps: 9_900,
            },
        );

        let router = SegmentRouter::with_overrides(SegmentRoutingConfig::default(), overrides);
        let report = router
            .route_conversation(&store, "conv-3")
            .expect("route conversation");
        assert_eq!(report.routed_override, 1);

        let route = store
            .segment_route_for_artifact("obs-3")
            .expect("load route")
            .expect("route exists");
        assert_eq!(route.routed_by, RouteOrigin::ManualOverride);
        assert_eq!(route.primary.segment_id, "frontend");
        assert_eq!(route.primary.confidence_bps, 9_900);
        assert_eq!(route.overridden_by.as_deref(), Some("patch-frontend-1"));
        assert!(route.reason.contains("override_patch:patch-frontend-1"));
        assert!(route.reason.contains("base=taskmaster_tag_map"));
        assert!(route
            .secondary
            .iter()
            .any(|candidate| candidate.segment_id == "mind"));
    }
}
