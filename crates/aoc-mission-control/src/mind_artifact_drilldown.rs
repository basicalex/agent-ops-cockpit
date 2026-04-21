//! Mind artifact drilldown rendering.
//!
//! Host-side composition that builds the "Knowledge artifacts" section for
//! Mission Control's Mind view, including compaction health and handshake→canon
//! provenance traces.

use super::*;

pub(crate) fn render_mind_artifact_drilldown_lines(
    project_root: &Path,
    session_id: &str,
    theme: MissionTheme,
    compact: bool,
    show_provenance: bool,
    observer_rows: &[MindObserverRow],
    runtime: Option<InsightRuntimeSnapshot>,
) -> Vec<Line<'static>> {
    let snapshot = load_mind_artifact_drilldown(project_root, session_id);
    if snapshot.latest_export.is_none()
        && snapshot.latest_compaction_checkpoint.is_none()
        && snapshot.latest_compaction_slice.is_none()
        && snapshot.handshake_entries.is_empty()
        && snapshot.active_canon_entries.is_empty()
        && snapshot.stale_canon_count == 0
    {
        return Vec::new();
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Knowledge artifacts",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("Artifact drilldown", Style::default().fg(theme.muted)),
        Span::raw(" "),
        Span::styled(
            if show_provenance {
                "[provenance:on]"
            } else {
                "[provenance:off]"
            },
            Style::default().fg(theme.muted),
        ),
    ])];

    if let Some(checkpoint) = snapshot.latest_compaction_checkpoint.as_ref() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Compaction / recovery", Style::default().fg(theme.title)),
        ]));
        let mut spans = vec![
            Span::raw("  "),
            Span::styled("compact:", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                ellipsize(
                    checkpoint
                        .compaction_entry_id
                        .as_deref()
                        .unwrap_or(&checkpoint.checkpoint_id),
                    if compact { 24 } else { 40 },
                ),
                Style::default().fg(theme.info),
            ),
        ];
        if let Some(tokens_before) = checkpoint.tokens_before {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("tokens:{}", tokens_before),
                Style::default().fg(theme.muted),
            ));
        }
        if let Some(first_kept) = checkpoint.first_kept_entry_id.as_deref() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("keep:{}", ellipsize(first_kept, 14)),
                Style::default().fg(theme.muted),
            ));
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("@{}", ellipsize(&checkpoint.ts.to_rfc3339(), 20)),
            Style::default().fg(theme.muted),
        ));
        lines.push(Line::from(spans));

        let t1_state = latest_compaction_t1_state(checkpoint, observer_rows);
        let t0_label = if snapshot.latest_compaction_slice.is_some() {
            "stored"
        } else {
            "missing"
        };
        let replay_label = if snapshot.compaction_rebuildable {
            "ready"
        } else if snapshot.compaction_marker_event_available {
            "partial"
        } else {
            "missing"
        };
        let mut health_spans = vec![
            Span::raw("  -> "),
            Span::styled("health:", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                format!("t0:{t0_label}"),
                Style::default().fg(if snapshot.latest_compaction_slice.is_some() {
                    theme.ok
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!("replay:{replay_label}"),
                Style::default().fg(if snapshot.compaction_rebuildable {
                    theme.ok
                } else if snapshot.compaction_marker_event_available {
                    theme.warn
                } else {
                    theme.critical
                }),
            ),
            Span::raw(" "),
            Span::styled(
                format!("t1:{}", t1_state.label),
                Style::default().fg(t1_state.color(theme)),
            ),
        ];
        if let Some(runtime) = runtime.as_ref() {
            health_spans.push(Span::raw(" "));
            health_spans.push(Span::styled(
                format!("t2q:{} t3q:{}", runtime.queue_depth, runtime.t3_queue_depth),
                Style::default().fg(theme.muted),
            ));
        }
        lines.push(Line::from(health_spans));

        if let Some(slice) = snapshot.latest_compaction_slice.as_ref() {
            lines.push(Line::from(vec![
                Span::raw("  -> "),
                Span::styled(
                    format!(
                        "evidence: src:{} read:{} modified:{} policy:{}",
                        slice.source_event_ids.len(),
                        slice.read_files.len(),
                        slice.modified_files.len(),
                        ellipsize(&slice.policy_version, 18)
                    ),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }

        if let Some(summary) = checkpoint
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            lines.push(Line::from(vec![
                Span::raw("  -> "),
                Span::styled(
                    ellipsize(summary, if compact { 52 } else { 88 }),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "recovery: press 'C' to rebuild/requeue latest compaction checkpoint",
                Style::default().fg(theme.warn),
            ),
        ]));
    }

    if let Some(manifest) = snapshot.latest_export.as_ref() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Recent export", Style::default().fg(theme.title)),
        ]));
        let export_leaf = Path::new(&manifest.export_dir)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("(unknown)");
        let mut spans = vec![
            Span::raw("  "),
            Span::styled("latest:", Style::default().fg(theme.muted)),
            Span::raw(" "),
            Span::styled(
                ellipsize(export_leaf, if compact { 30 } else { 48 }),
                Style::default().fg(theme.accent),
            ),
            Span::raw(" "),
            Span::styled(
                format!("session:{}", ellipsize(&manifest.session_id, 18)),
                Style::default().fg(theme.muted),
            ),
            Span::raw(" "),
            Span::styled(
                format!("t1:{} t2:{}", manifest.t1_count, manifest.t2_count),
                Style::default().fg(theme.muted),
            ),
        ];
        if let Some(active_tag) = manifest
            .active_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("tag:{}", ellipsize(active_tag, 16)),
                Style::default().fg(theme.info),
            ));
        }
        if !manifest.exported_at.trim().is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("@{}", ellipsize(&manifest.exported_at, 20)),
                Style::default().fg(theme.muted),
            ));
        }
        if !manifest.t3_job_id.trim().is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("t3:{}", ellipsize(&manifest.t3_job_id, 20)),
                Style::default().fg(theme.warn),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("Handshake + canon", Style::default().fg(theme.title)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!(
                "handshake:{} active_canon:{} stale_canon:{}",
                snapshot.handshake_entries.len(),
                snapshot.active_canon_entries.len(),
                snapshot.stale_canon_count
            ),
            Style::default().fg(theme.muted),
        ),
    ]));

    if !show_provenance {
        lines.push(Line::from(vec![
            Span::raw("  -> "),
            Span::styled(
                "press 'p' to expand handshake → canon → evidence links",
                Style::default().fg(theme.muted),
            ),
        ]));
        return lines;
    }

    let mut canon_by_key = HashMap::new();
    for entry in &snapshot.active_canon_entries {
        canon_by_key.insert(canon_key(&entry.entry_id, entry.revision), entry);
    }

    let limit = if compact { 2 } else { 5 };
    for handshake in snapshot.handshake_entries.iter().take(limit) {
        lines.push(Line::from(vec![
            Span::raw("  ↳ "),
            Span::styled(
                format!("[{} r{}]", handshake.entry_id, handshake.revision),
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                handshake
                    .topic
                    .as_deref()
                    .map(|topic| format!("topic={topic}"))
                    .unwrap_or_else(|| "topic=global".to_string()),
                Style::default().fg(theme.muted),
            ),
        ]));

        let summary = ellipsize(&handshake.summary, if compact { 44 } else { 80 });
        lines.push(Line::from(vec![
            Span::raw("     "),
            Span::styled(summary, Style::default().fg(theme.muted)),
        ]));

        let key = canon_key(&handshake.entry_id, handshake.revision);
        if let Some(canon) = canon_by_key.get(&key) {
            let refs = if canon.evidence_refs.is_empty() {
                "(none)".to_string()
            } else {
                canon
                    .evidence_refs
                    .iter()
                    .take(if compact { 2 } else { 4 })
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let refs_count = canon.evidence_refs.len();
            lines.push(Line::from(vec![
                Span::raw("     trace: "),
                Span::styled("handshake", Style::default().fg(theme.info)),
                Span::raw(" -> "),
                Span::styled("canon", Style::default().fg(theme.accent)),
                Span::raw(" -> "),
                Span::styled(
                    format!("evidence[{refs_count}] {refs}"),
                    Style::default().fg(theme.warn),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("     trace: "),
                Span::styled(
                    "handshake -> canon (missing active entry)",
                    Style::default().fg(theme.critical),
                ),
            ]));
        }
    }

    lines
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CompactionT1State {
    label: &'static str,
}

impl CompactionT1State {
    pub(crate) fn color(self, theme: MissionTheme) -> Color {
        match self.label {
            "ok" => theme.ok,
            "pending" => theme.warn,
            "fallback" => theme.warn,
            "error" => theme.critical,
            _ => theme.muted,
        }
    }
}

pub(crate) fn latest_compaction_t1_state(
    checkpoint: &CompactionCheckpoint,
    observer_rows: &[MindObserverRow],
) -> CompactionT1State {
    fn event_observed_ms(event: &MindObserverFeedEvent) -> Option<i64> {
        event
            .completed_at
            .as_deref()
            .or(event.started_at.as_deref())
            .or(event.enqueued_at.as_deref())
            .and_then(parse_rfc3339_utc)
            .map(|ts| ts.timestamp_millis())
    }

    let checkpoint_ms = checkpoint.ts.timestamp_millis();
    let mut candidates = observer_rows
        .iter()
        .filter(|row| {
            mind_event_lane(&row.event) == MindLaneFilter::T1
                && row.event.trigger == MindObserverFeedTriggerKind::Compaction
                && event_observed_ms(&row.event)
                    .map(|ts| ts >= checkpoint_ms.saturating_sub(1_000))
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|row| event_observed_ms(&row.event).unwrap_or(0));

    let selected = candidates
        .iter()
        .rev()
        .find(|row| {
            row.event
                .conversation_id
                .as_deref()
                .map(|id| id == checkpoint.conversation_id.as_str())
                .unwrap_or(false)
        })
        .copied()
        .or_else(|| candidates.into_iter().last());

    selected
        .map(|row| match row.event.status {
            MindObserverFeedStatus::Success => CompactionT1State { label: "ok" },
            MindObserverFeedStatus::Fallback => CompactionT1State { label: "fallback" },
            MindObserverFeedStatus::Running | MindObserverFeedStatus::Queued => {
                CompactionT1State { label: "pending" }
            }
            MindObserverFeedStatus::Error => CompactionT1State { label: "error" },
        })
        .unwrap_or(CompactionT1State { label: "unknown" })
}

pub(crate) fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|ts| ts.with_timezone(&Utc))
}
