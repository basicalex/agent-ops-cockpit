//! App state coordination and behavior.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

impl App {
    pub(crate) fn new(
        config: Config,
        command_tx: mpsc::Sender<HubOutbound>,
        local: LocalSnapshot,
    ) -> Self {
        let tab_cache = seed_tab_cache(&local.overview);
        let last_viewer_tab_index = local.viewer_tab_index;
        let default_mode = if config.overview_enabled {
            Mode::Overview
        } else {
            Mode::Overseer
        };
        let mode = config.start_view.unwrap_or(default_mode);
        let status_note = if config.overview_enabled {
            None
        } else {
            Some("overview disabled; using Overseer/Mind/Work/Diff/Health".to_string())
        };
        let fleet_plane_filter = config.fleet_plane_filter;
        Self {
            config,
            command_tx,
            connected: false,
            hub_disconnected_at: None,
            hub: HubCache::default(),
            local,
            tab_cache,
            mode,
            scroll: 0,
            help_open: false,
            selected_overview: 0,
            selected_fleet: 0,
            selected_fleet_job: 0,
            overview_sort_mode: OverviewSortMode::Layout,
            fleet_sort_mode: FleetSortMode::Project,
            fleet_plane_filter,
            fleet_active_only: false,
            follow_viewer_tab: true,
            last_viewer_tab_index,
            mind_lane: MindLaneFilter::T1,
            mind_show_all_tabs: false,
            mind_show_provenance: false,
            mind_search_query: String::new(),
            mind_search_editing: false,
            mind_search_selected: 0,
            status_note,
            pending_commands: HashMap::new(),
            pending_consultations: HashMap::new(),
            next_request_id: 0,
            pending_render_latency: Vec::new(),
            parser_confidence: HashMap::new(),
            latency_sample_count: 0,
        }
    }

    pub(crate) fn apply_hub_event(&mut self, event: HubEvent) {
        match event {
            HubEvent::Connected => {
                self.connected = true;
                self.hub_disconnected_at = None;
                self.status_note = Some("hub connected".to_string());
            }
            HubEvent::Disconnected => {
                self.connected = false;
                self.hub_disconnected_at = Some(Utc::now());
                self.pending_commands.clear();
                self.pending_consultations.clear();
                self.pending_render_latency.clear();
                self.status_note = Some(if self.has_any_hub_data() {
                    "hub reconnecting; holding last snapshot".to_string()
                } else {
                    "hub offline; local fallback active".to_string()
                });
            }
            HubEvent::Snapshot { payload, event_at } => {
                self.hub.agents.clear();
                self.hub.tasks.clear();
                self.hub.diffs.clear();
                self.hub.health.clear();
                self.hub.mind.clear();
                self.hub.mind_injection.clear();
                self.hub.insight_runtime.clear();
                self.hub.insight_detached.clear();
                self.hub.observer_snapshot = None;
                self.hub.observer_timeline.clear();
                self.hub.last_seq = payload.seq;
                for state in payload.states {
                    self.upsert_hub_state(state, event_at, "snapshot");
                }
            }
            HubEvent::Delta { payload, event_at } => {
                if payload.seq <= self.hub.last_seq {
                    return;
                }
                if self.hub.last_seq > 0 && payload.seq > self.hub.last_seq + 1 {
                    self.hub.agents.clear();
                    self.hub.tasks.clear();
                    self.hub.diffs.clear();
                    self.hub.health.clear();
                    self.hub.mind.clear();
                    self.hub.mind_injection.clear();
                    self.hub.insight_runtime.clear();
                    self.hub.insight_detached.clear();
                    self.hub.observer_snapshot = None;
                    self.hub.observer_timeline.clear();
                    self.status_note = Some("hub delta gap detected; awaiting resync".to_string());
                }
                self.hub.last_seq = payload.seq;
                for change in payload.changes {
                    match change.op {
                        StateChangeOp::Upsert => {
                            if let Some(state) = change.state {
                                self.upsert_hub_state(state, event_at, "delta");
                            }
                        }
                        StateChangeOp::Remove => {
                            self.hub.agents.remove(&change.agent_id);
                            self.hub.diffs.remove(&change.agent_id);
                            self.hub.health.remove(&change.agent_id);
                            self.hub.mind.remove(&change.agent_id);
                            self.hub.mind_injection.remove(&change.agent_id);
                            self.hub.insight_runtime.remove(&change.agent_id);
                            self.hub.insight_detached.remove(&change.agent_id);
                            self.hub.tasks.retain(|key, payload| {
                                if payload.agent_id == change.agent_id {
                                    return false;
                                }
                                key.rsplit_once("::")
                                    .map(|(agent_id, _)| agent_id != change.agent_id)
                                    .unwrap_or(true)
                            });
                        }
                    }
                }
            }
            HubEvent::ObserverSnapshot { payload } => {
                if payload.session_id != self.config.session_id {
                    return;
                }
                self.hub.observer_snapshot = Some(payload);
            }
            HubEvent::ObserverTimeline { payload } => {
                if payload.session_id != self.config.session_id {
                    return;
                }
                self.hub.observer_timeline = payload.entries;
            }
            HubEvent::LayoutState { payload } => {
                if !self.config.overview_enabled {
                    return;
                }
                if payload.session_id != self.config.session_id {
                    return;
                }
                if self
                    .hub
                    .layout
                    .as_ref()
                    .map(|layout| payload.layout_seq <= layout.layout_seq)
                    .unwrap_or(false)
                {
                    return;
                }
                self.hub.layout = Some(hub_layout_from_payload(&payload));
                self.update_viewer_tab_index(self.viewer_tab_index_from_hub_layout());
            }
            HubEvent::Heartbeat { payload, event_at } => {
                let entry = self
                    .hub
                    .agents
                    .entry(payload.agent_id.clone())
                    .or_insert_with(|| HubAgent {
                        status: Some(AgentStatusPayload {
                            agent_id: payload.agent_id.clone(),
                            status: payload
                                .lifecycle
                                .clone()
                                .unwrap_or_else(|| "running".to_string()),
                            pane_id: extract_pane_id(&payload.agent_id),
                            project_root: "(unknown)".to_string(),
                            tab_scope: None,
                            agent_label: Some(extract_label(&payload.agent_id)),
                            message: None,
                            session_title: None,
                            chat_title: None,
                        }),
                        last_seen: event_at,
                        last_heartbeat: None,
                        last_activity: None,
                    });
                entry.last_seen = event_at;
                entry.last_heartbeat = ms_to_datetime(payload.last_heartbeat_ms).or(Some(event_at));
                if let Some(lifecycle) = payload.lifecycle.as_ref() {
                    if let Some(status) = entry.status.as_mut() {
                        status.status = normalize_lifecycle(lifecycle);
                    }
                }
                self.observe_heartbeat_latency(&payload, event_at);
            }
            HubEvent::CommandResult {
                payload,
                request_id,
            } => {
                self.apply_command_result(payload, request_id);
            }
            HubEvent::ConsultationResponse {
                payload,
                request_id,
            } => {
                self.apply_consultation_response(payload, request_id);
            }
        }
    }

    pub(crate) fn latest_compaction_checkpoint(&self) -> Option<CompactionCheckpoint> {
        load_mind_artifact_drilldown(
            Path::new(&self.config.project_root),
            &self.config.session_id,
        )
        .latest_compaction_checkpoint
    }

    pub(crate) fn upsert_hub_state(
        &mut self,
        state: AgentState,
        event_at: DateTime<Utc>,
        channel: &'static str,
    ) {
        let key = state.agent_id.clone();
        self.observe_state_latency(&state, event_at, channel);
        self.observe_parser_confidence_transition(&state.agent_id, &state.source, channel);
        let status = status_payload_from_state(&state);
        let project_root = status.project_root.clone();
        let heartbeat_at = state.last_heartbeat_ms.and_then(ms_to_datetime);
        let activity_at = state.last_activity_ms.and_then(ms_to_datetime);
        let entry = self.hub.agents.entry(key).or_insert(HubAgent {
            status: None,
            last_seen: event_at,
            last_heartbeat: None,
            last_activity: None,
        });
        entry.status = Some(status);
        entry.last_seen = event_at;
        entry.last_heartbeat = heartbeat_at.or(Some(event_at));
        entry.last_activity = activity_at.or(Some(event_at));

        if let Some(source_value) =
            source_value_by_keys(&state.source, &["task_summaries", "task_summary"])
        {
            match parse_task_summaries_from_source(source_value, &state.agent_id) {
                Ok(task_summaries) => {
                    self.hub
                        .tasks
                        .retain(|_, payload| payload.agent_id != state.agent_id);
                    for payload in task_summaries {
                        let key = format!("{}::{}", payload.agent_id, payload.tag);
                        self.hub.tasks.insert(key, payload);
                    }
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "task_summary",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["diff_summary"]) {
            match parse_diff_summary_from_source(source_value, &state.agent_id, &project_root) {
                Ok(Some(payload)) => {
                    self.hub.diffs.insert(state.agent_id.clone(), payload);
                }
                Ok(None) => {
                    self.hub.diffs.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "diff_summary",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        }

        if let Some(source_value) =
            source_value_by_keys(&state.source, &["health", "health_summary"])
        {
            match parse_health_from_source(source_value) {
                Ok(Some(snapshot)) => {
                    self.hub.health.insert(state.agent_id.clone(), snapshot);
                }
                Ok(None) => {
                    self.hub.health.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "health",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["mind_observer"]) {
            match parse_mind_observer_from_source(source_value) {
                Ok(Some(feed)) => {
                    self.hub.mind.insert(state.agent_id.clone(), feed);
                }
                Ok(None) => {
                    self.hub.mind.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "mind_observer",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.mind.remove(&state.agent_id);
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["mind_injection"]) {
            match parse_mind_injection_from_source(source_value) {
                Ok(Some(payload)) => {
                    self.hub
                        .mind_injection
                        .insert(state.agent_id.clone(), payload);
                }
                Ok(None) => {
                    self.hub.mind_injection.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "mind_injection",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.mind_injection.remove(&state.agent_id);
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["insight_runtime"]) {
            match parse_insight_runtime_from_source(source_value) {
                Ok(Some(snapshot)) => {
                    self.hub
                        .insight_runtime
                        .insert(state.agent_id.clone(), snapshot);
                }
                Ok(None) => {
                    self.hub.insight_runtime.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "insight_runtime",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.insight_runtime.remove(&state.agent_id);
        }

        if let Some(source_value) = source_value_by_keys(&state.source, &["insight_detached"]) {
            match parse_insight_detached_from_source(source_value) {
                Ok(Some(snapshot)) => {
                    self.hub
                        .insight_detached
                        .insert(state.agent_id.clone(), snapshot);
                }
                Ok(None) => {
                    self.hub.insight_detached.remove(&state.agent_id);
                }
                Err(err) => {
                    warn!(
                        event = "pulse_source_parse_error",
                        kind = "insight_detached",
                        channel,
                        agent_id = %state.agent_id,
                        error = %err
                    );
                }
            }
        } else {
            self.hub.insight_detached.remove(&state.agent_id);
        }
    }

    pub(crate) fn observe_state_latency(
        &mut self,
        state: &AgentState,
        event_at: DateTime<Utc>,
        channel: &'static str,
    ) {
        let emitted_at_ms = state
            .updated_at_ms
            .or(state.last_activity_ms)
            .or(state.last_heartbeat_ms);
        let Some(emitted_at_ms) = emitted_at_ms else {
            return;
        };
        let hub_event_at_ms = event_at.timestamp_millis();
        let ingest_latency_ms = hub_event_at_ms.saturating_sub(emitted_at_ms);
        self.latency_sample_count = self.latency_sample_count.saturating_add(1);
        let sample_id = self.latency_sample_count;
        if ingest_latency_ms >= PULSE_LATENCY_WARN_MS {
            warn!(
                event = "pulse_end_to_end_latency",
                stage = "hub_ingest",
                sample_id,
                channel,
                agent_id = %state.agent_id,
                emit_ts_ms = emitted_at_ms,
                hub_event_ts_ms = hub_event_at_ms,
                latency_ms = ingest_latency_ms
            );
        } else if sample_id % PULSE_LATENCY_INFO_EVERY == 0 {
            info!(
                event = "pulse_end_to_end_latency",
                stage = "hub_ingest",
                sample_id,
                channel,
                agent_id = %state.agent_id,
                latency_ms = ingest_latency_ms
            );
        }
        self.pending_render_latency.push(PendingRenderLatency {
            sample_id,
            agent_id: state.agent_id.clone(),
            channel,
            emitted_at_ms,
            hub_event_at_ms,
            ingest_latency_ms,
        });
    }

    pub(crate) fn observe_heartbeat_latency(
        &mut self,
        payload: &PulseHeartbeatPayload,
        event_at: DateTime<Utc>,
    ) {
        let ingest_latency_ms = event_at
            .timestamp_millis()
            .saturating_sub(payload.last_heartbeat_ms);
        if ingest_latency_ms >= PULSE_LATENCY_WARN_MS {
            warn!(
                event = "pulse_end_to_end_latency",
                stage = "heartbeat_ingest",
                agent_id = %payload.agent_id,
                latency_ms = ingest_latency_ms
            );
        }
    }

    pub(crate) fn observe_parser_confidence_transition(
        &mut self,
        agent_id: &str,
        source: &Option<Value>,
        channel: &'static str,
    ) {
        let Some(next_confidence) = source_confidence(source) else {
            return;
        };
        let previous = self
            .parser_confidence
            .insert(agent_id.to_string(), next_confidence);
        if previous == Some(next_confidence) {
            return;
        }
        info!(
            event = "pulse_parser_confidence_transition",
            channel,
            agent_id,
            previous = previous.unwrap_or(0),
            next = next_confidence
        );
    }

    pub(crate) fn observe_render_latency(&mut self) {
        if self.pending_render_latency.is_empty() {
            return;
        }
        let now_ms = Utc::now().timestamp_millis();
        for sample in self.pending_render_latency.drain(..) {
            let total_latency_ms = now_ms.saturating_sub(sample.emitted_at_ms);
            let render_latency_ms = now_ms.saturating_sub(sample.hub_event_at_ms);
            if total_latency_ms >= PULSE_LATENCY_WARN_MS {
                warn!(
                    event = "pulse_end_to_end_latency",
                    stage = "render",
                    sample_id = sample.sample_id,
                    channel = sample.channel,
                    agent_id = %sample.agent_id,
                    ingest_latency_ms = sample.ingest_latency_ms,
                    hub_to_render_ms = render_latency_ms,
                    total_latency_ms
                );
            } else if sample.sample_id % PULSE_LATENCY_INFO_EVERY == 0 {
                info!(
                    event = "pulse_end_to_end_latency",
                    stage = "render",
                    sample_id = sample.sample_id,
                    channel = sample.channel,
                    agent_id = %sample.agent_id,
                    ingest_latency_ms = sample.ingest_latency_ms,
                    hub_to_render_ms = render_latency_ms,
                    total_latency_ms
                );
            }
        }
    }

    pub(crate) fn apply_command_result(
        &mut self,
        payload: CommandResultPayload,
        request_id: Option<String>,
    ) {
        let tracked = request_id
            .as_deref()
            .and_then(|id| self.pending_commands.get(id).cloned());
        if request_id.is_some() && tracked.is_none() {
            debug!(
                event = "pulse_command_result_ignored",
                reason = "stale_request_id",
                request_id = request_id.as_deref().unwrap_or_default(),
                command = %payload.command,
                status = %payload.status
            );
            return;
        }

        let done = is_terminal_command_status(&payload.status);
        if done {
            if let Some(id) = request_id.as_deref() {
                self.pending_commands.remove(id);
            }
        }
        let target = tracked
            .as_ref()
            .map(|value| value.target.clone())
            .unwrap_or_else(|| "hub".to_string());
        let command_name = tracked
            .as_ref()
            .map(|value| value.command.clone())
            .unwrap_or_else(|| payload.command.clone());
        let mut message = payload
            .message
            .clone()
            .unwrap_or_else(|| payload.status.clone());
        if let Some(error) = payload.error.as_ref() {
            message = format!("{} ({})", error.message, error.code);
        }
        self.status_note = Some(format!(
            "{} {} -> {}",
            command_name,
            target,
            ellipsize(&message, 72)
        ));
    }

    pub(crate) fn apply_consultation_response(
        &mut self,
        payload: ConsultationResponsePayload,
        request_id: Option<String>,
    ) {
        let tracked = request_id
            .as_deref()
            .and_then(|id| self.pending_consultations.get(id).cloned());
        if request_id.is_some() && tracked.is_none() {
            debug!(
                event = "pulse_consultation_result_ignored",
                reason = "stale_request_id",
                request_id = request_id.as_deref().unwrap_or_default(),
                consultation_id = %payload.consultation_id,
                status = ?payload.status
            );
            return;
        }

        if is_terminal_consultation_status(payload.status) {
            if let Some(id) = request_id.as_deref() {
                self.pending_consultations.remove(id);
            }
        }

        let (requester, responder, kind, request_packet) = tracked
            .map(|value| {
                (
                    value.requester,
                    value.responder,
                    value.kind,
                    Some(value.request_packet),
                )
            })
            .unwrap_or_else(|| {
                (
                    payload.requesting_agent_id.clone(),
                    payload.responding_agent_id.clone(),
                    ConsultationPacketKind::Summary,
                    None,
                )
            });
        if let Some(request_packet) = request_packet.as_ref() {
            if let Err(err) = persist_consultation_outcome(
                &self.config.project_root,
                request_packet,
                &payload,
                kind,
            ) {
                warn!(
                    event = "consultation_outcome_persist_failed",
                    consultation_id = %payload.consultation_id,
                    error = %err
                );
            }
        }
        let mut message = payload
            .message
            .clone()
            .unwrap_or_else(|| format!("{:?}", payload.status).to_ascii_lowercase());
        if let Some(error) = payload.error.as_ref() {
            message = format!("{} ({})", error.message, error.code);
        }
        self.status_note = Some(format!(
            "consult {:?} {} -> {} · {}",
            kind,
            requester,
            responder,
            ellipsize(&message, 72)
        ));
    }

    pub(crate) fn next_command_request_id(&mut self) -> String {
        self.next_request_id = self.next_request_id.saturating_add(1);
        format!("pulse-{}-{}", std::process::id(), self.next_request_id)
    }

    pub(crate) fn queue_hub_command(
        &mut self,
        command: &str,
        target_agent_id: Option<String>,
        args: Value,
        target_label: String,
    ) {
        if !self.connected {
            self.status_note = Some("hub offline; command unavailable".to_string());
            return;
        }
        let request_id = self.next_command_request_id();
        let outbound = HubOutbound {
            request_id: request_id.clone(),
            msg: WireMsg::Command(CommandPayload {
                command: command.to_string(),
                target_agent_id,
                args,
            }),
        };
        match self.command_tx.try_send(outbound) {
            Ok(()) => {
                let queued_target = target_label.clone();
                self.pending_commands.insert(
                    request_id,
                    PendingCommand {
                        command: command.to_string(),
                        target: target_label,
                    },
                );
                self.status_note = Some(format!("{command} queued for {queued_target}"));
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(
                    event = "pulse_command_queue_drop",
                    reason = "queue_full",
                    command,
                    pending = self.pending_commands.len(),
                    capacity = COMMAND_QUEUE_CAPACITY
                );
                self.status_note = Some("hub command queue full".to_string());
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!(
                    event = "pulse_command_queue_drop",
                    reason = "channel_closed",
                    command,
                    pending = self.pending_commands.len()
                );
                self.status_note = Some("hub command channel closed".to_string());
            }
        }
    }

    pub(crate) fn set_local(&mut self, local: LocalSnapshot) {
        let LocalSnapshot {
            overview,
            viewer_tab_index,
            tab_roster,
            work,
            diff,
            health,
        } = local;
        self.set_local_overview(overview, viewer_tab_index);
        self.local.tab_roster = tab_roster;
        self.local.work = work;
        self.local.diff = diff;
        self.local.health = health;
    }

    pub(crate) fn set_local_overview(
        &mut self,
        overview: Vec<OverviewRow>,
        viewer_tab_index: Option<usize>,
    ) {
        let viewer_tab_index = viewer_tab_index.or(self.local.viewer_tab_index);
        merge_tab_cache(&mut self.tab_cache, &overview);
        self.update_viewer_tab_index(viewer_tab_index);
        self.local.overview = overview;
    }

    pub(crate) fn update_viewer_tab_index(&mut self, viewer_tab_index: Option<usize>) {
        let viewer_tab_index = viewer_tab_index.or(self.local.viewer_tab_index);
        if viewer_tab_index != self.last_viewer_tab_index {
            self.follow_viewer_tab = true;
        }
        self.last_viewer_tab_index = viewer_tab_index;
        self.local.viewer_tab_index = viewer_tab_index;
    }

    pub(crate) fn viewer_tab_index_from_hub_layout(&self) -> Option<usize> {
        self.active_hub_layout().and_then(|layout| {
            if self.config.pane_id.trim().is_empty() {
                return layout.focused_tab_index;
            }
            layout
                .pane_tabs
                .get(&self.config.pane_id)
                .map(|meta| meta.index)
                .or(layout.focused_tab_index)
        })
    }

    pub(crate) fn active_hub_layout(&self) -> Option<&HubLayout> {
        if self.config.layout_source == LayoutSource::Local {
            return None;
        }
        if self.connected || self.hub_reconnect_grace_active() {
            return self.hub.layout.as_ref();
        }
        None
    }

    pub(crate) fn should_poll_local_layout(&self) -> bool {
        match self.config.layout_source {
            LayoutSource::Local => true,
            LayoutSource::Hybrid => !self.connected,
            LayoutSource::Hub => false,
        }
    }

    pub(crate) fn refresh_local_layout(&mut self) {
        let (overview, viewer_tab_index, tab_roster) =
            collect_layout_overview(&self.config, &self.local.overview, &self.tab_cache);
        self.set_local_overview(overview, viewer_tab_index);
        self.local.tab_roster = tab_roster;
    }

    pub(crate) fn collect_local_snapshot(&self) -> LocalSnapshot {
        collect_local_with_options(
            &self.config,
            !self.prefer_hub_data(!self.hub.tasks.is_empty()),
            !self.prefer_hub_data(!self.hub.diffs.is_empty()),
            !self.prefer_hub_data(!self.hub.health.is_empty()),
            Some(&self.local),
        )
    }

    pub(crate) fn prune_hub_cache(&mut self) {
        let now = Utc::now();
        let local_online: HashSet<String> = self
            .local
            .overview
            .iter()
            .filter(|row| row.online)
            .map(|row| row.identity_key.clone())
            .collect();
        let hub_agent_ids: HashSet<String> = self.hub.agents.keys().cloned().collect();
        let overlap = hub_agent_ids.intersection(&local_online).count();
        let local_alignment_confident = !hub_agent_ids.is_empty()
            && !local_online.is_empty()
            && (overlap * 100) >= (hub_agent_ids.len() * HUB_LOCAL_ALIGNMENT_MIN_PERCENT);
        self.hub.agents.retain(|agent_id, agent| {
            let age = now
                .signed_duration_since(agent.last_seen)
                .num_seconds()
                .max(0);
            if self.connected
                && local_alignment_confident
                && !local_online.contains(agent_id)
                && age >= HUB_LOCAL_MISS_PRUNE_SECS
            {
                return false;
            }
            let reported_offline = agent
                .status
                .as_ref()
                .map(|status| status.status.eq_ignore_ascii_case("offline"))
                .unwrap_or(false);
            if reported_offline {
                age <= HUB_OFFLINE_GRACE_SECS
            } else {
                age <= HUB_PRUNE_SECS
            }
        });

        let active_agents: HashSet<String> = self.hub.agents.keys().cloned().collect();
        self.hub
            .diffs
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .health
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .mind
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .mind_injection
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .insight_runtime
            .retain(|agent_id, _| active_agents.contains(agent_id));
        self.hub
            .insight_detached
            .retain(|agent_id, _| active_agents.contains(agent_id));
        if let Some(snapshot) = self.hub.observer_snapshot.as_mut() {
            snapshot
                .workers
                .retain(|worker| active_agents.contains(&worker.agent_id));
            snapshot
                .timeline
                .retain(|entry| active_agents.contains(&entry.agent_id));
            if snapshot.workers.is_empty() && snapshot.timeline.is_empty() {
                self.hub.observer_snapshot = None;
            }
        }
        self.hub
            .observer_timeline
            .retain(|entry| active_agents.contains(&entry.agent_id));
        self.hub.tasks.retain(|key, payload| {
            if active_agents.contains(&payload.agent_id) {
                return true;
            }
            key.rsplit_once("::")
                .map(|(agent_id, _)| active_agents.contains(agent_id))
                .unwrap_or(false)
        });
    }

    pub(crate) fn mode_source(&self) -> &'static str {
        match self.mode {
            Mode::Overview => {
                if self.prefer_hub_data(!self.hub.agents.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Overseer => {
                if self.prefer_hub_data(
                    self.hub
                        .observer_snapshot
                        .as_ref()
                        .map(|snapshot| {
                            !snapshot.workers.is_empty() || !snapshot.timeline.is_empty()
                        })
                        .unwrap_or(false)
                        || !self.hub.observer_timeline.is_empty(),
                ) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Mind => {
                if self.prefer_hub_data(
                    !self.hub.mind.is_empty()
                        || !self.hub.insight_runtime.is_empty()
                        || !self.hub.insight_detached.is_empty(),
                ) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Fleet => {
                if self.prefer_hub_data(!self.hub.insight_detached.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Work => {
                if self.prefer_hub_data(!self.hub.tasks.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Diff => {
                if self.prefer_hub_data(!self.hub.diffs.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
            Mode::Health => {
                if self.prefer_hub_data(!self.hub.health.is_empty()) {
                    "hub"
                } else {
                    "local"
                }
            }
        }
    }

    pub(crate) fn has_any_hub_data(&self) -> bool {
        !self.hub.agents.is_empty()
            || !self.hub.tasks.is_empty()
            || !self.hub.diffs.is_empty()
            || !self.hub.health.is_empty()
            || !self.hub.mind.is_empty()
            || !self.hub.insight_runtime.is_empty()
            || !self.hub.insight_detached.is_empty()
            || self.hub.observer_snapshot.is_some()
            || !self.hub.observer_timeline.is_empty()
            || self.hub.layout.is_some()
    }

    #[allow(dead_code)]
    pub(crate) fn hub_status_label(&self) -> &'static str {
        if self.connected {
            "online"
        } else if self.hub_reconnect_grace_active() && self.has_any_hub_data() {
            "reconnecting"
        } else {
            "offline"
        }
    }

    pub(crate) fn hub_reconnect_grace_active(&self) -> bool {
        if self.connected {
            return false;
        }
        let Some(disconnected_at) = self.hub_disconnected_at else {
            return false;
        };
        Utc::now()
            .signed_duration_since(disconnected_at)
            .num_seconds()
            <= HUB_RECONNECT_GRACE_SECS
    }

    pub(crate) fn prefer_hub_data(&self, has_hub_data: bool) -> bool {
        has_hub_data && (self.connected || self.hub_reconnect_grace_active())
    }

    pub(crate) fn overview_rows(&self) -> Vec<OverviewRow> {
        let viewer_scope = self.config.tab_scope.as_deref();
        if self.prefer_hub_data(!self.hub.agents.is_empty()) {
            let now = Utc::now();
            let mut rows: BTreeMap<String, OverviewRow> = BTreeMap::new();
            for (agent_id, agent) in &self.hub.agents {
                let status = agent.status.as_ref();
                let row_tab_scope = status.and_then(|s| s.tab_scope.as_deref());
                let pane_id = status
                    .map(|s| s.pane_id.clone())
                    .unwrap_or_else(|| extract_pane_id(agent_id));
                let label = status
                    .and_then(|s| s.agent_label.clone())
                    .unwrap_or_else(|| extract_label(agent_id));
                let project_root = status
                    .map(|s| s.project_root.clone())
                    .unwrap_or_else(|| "(unknown)".to_string());
                let heartbeat_age_secs = agent
                    .last_heartbeat
                    .map(|dt| now.signed_duration_since(dt).num_seconds().max(0))
                    .or(Some(
                        now.signed_duration_since(agent.last_seen)
                            .num_seconds()
                            .max(0),
                    ));
                let age_secs = agent
                    .last_activity
                    .map(|dt| now.signed_duration_since(dt).num_seconds().max(0))
                    .or(heartbeat_age_secs);
                let reported = status
                    .map(|s| s.status.to_ascii_lowercase())
                    .unwrap_or_else(|| "running".to_string());
                let online = reported != "offline"
                    && heartbeat_age_secs.unwrap_or(HUB_STALE_SECS + 1) <= HUB_STALE_SECS;
                let row = OverviewRow {
                    identity_key: agent_id.clone(),
                    label,
                    lifecycle: status
                        .map(|s| normalize_lifecycle(&s.status))
                        .unwrap_or_else(|| "running".to_string()),
                    snippet: status.and_then(|s| s.message.clone()),
                    pane_id,
                    tab_index: None,
                    tab_name: status.and_then(|s| s.tab_scope.clone()),
                    tab_focused: tab_scope_matches(viewer_scope, row_tab_scope),
                    project_root,
                    online,
                    age_secs,
                    source: "hub".to_string(),
                    session_title: status.and_then(|s| s.session_title.clone()),
                    chat_title: status.and_then(|s| s.chat_title.clone()),
                };
                rows.insert(row.identity_key.clone(), row);
            }

            for local in &self.local.overview {
                if !local.online {
                    continue;
                }
                if let Some(existing) = rows.get_mut(&local.identity_key) {
                    if existing.project_root == "(unknown)" && local.project_root != "(unknown)" {
                        existing.project_root = local.project_root.clone();
                    }
                    if existing.label.starts_with("pane-") && !local.label.starts_with("pane-") {
                        existing.label = local.label.clone();
                    }
                    if existing.source == "hub" {
                        existing.source = "mix".to_string();
                    }
                    if existing.tab_index.is_none() {
                        existing.tab_index = local.tab_index;
                    }
                    if existing.tab_name.is_none() {
                        existing.tab_name = local.tab_name.clone();
                    }
                    if local.tab_focused {
                        existing.tab_focused = true;
                    }
                    if local.online {
                        existing.online = true;
                        existing.age_secs = match (existing.age_secs, local.age_secs) {
                            (Some(left), Some(right)) => Some(left.min(right)),
                            (None, Some(right)) => Some(right),
                            (left, None) => left,
                        };
                        if existing.lifecycle == "offline" {
                            existing.lifecycle = local.lifecycle.clone();
                        }
                    }
                    if existing.session_title.is_none() {
                        existing.session_title = local.session_title.clone();
                    }
                    if existing.chat_title.is_none() {
                        existing.chat_title = local.chat_title.clone();
                    }
                } else {
                    let mut local_row = local.clone();
                    if local_row.source == "runtime" || local_row.source == "proc" {
                        local_row.source = "loc".to_string();
                    }
                    rows.insert(local_row.identity_key.clone(), local_row);
                }
            }

            let mut merged_rows: Vec<OverviewRow> = rows.into_values().collect();
            for row in &mut merged_rows {
                if let Some(meta) = self
                    .active_hub_layout()
                    .and_then(|layout| layout.pane_tabs.get(&row.pane_id))
                {
                    if row.tab_index.is_none() {
                        row.tab_index = Some(meta.index);
                    }
                    if row.tab_name.is_none() {
                        row.tab_name = Some(meta.name.clone());
                    }
                }
                apply_cached_tab_meta(row, &self.tab_cache);
                if !row.tab_focused {
                    row.tab_focused = tab_scope_matches(viewer_scope, row.tab_name.as_deref());
                }
            }
            return self.sort_overview_rows_for_mode(merged_rows);
        }
        let mut local_rows = self.local.overview.clone();
        for row in &mut local_rows {
            if let Some(meta) = self
                .active_hub_layout()
                .and_then(|layout| layout.pane_tabs.get(&row.pane_id))
            {
                if row.tab_index.is_none() {
                    row.tab_index = Some(meta.index);
                }
                if row.tab_name.is_none() {
                    row.tab_name = Some(meta.name.clone());
                }
            }
            apply_cached_tab_meta(row, &self.tab_cache);
            if !row.tab_focused {
                row.tab_focused = tab_scope_matches(viewer_scope, row.tab_name.as_deref());
            }
        }
        self.sort_overview_rows_for_mode(local_rows)
    }

    pub(crate) fn sort_overview_rows_for_mode(&self, rows: Vec<OverviewRow>) -> Vec<OverviewRow> {
        match self.overview_sort_mode {
            OverviewSortMode::Layout => sort_overview_rows(rows),
            OverviewSortMode::Attention => sort_overview_rows_attention(rows),
        }
    }

    pub(crate) fn toggle_overview_sort_mode(&mut self) {
        self.overview_sort_mode = self.overview_sort_mode.toggle();
        self.follow_viewer_tab = true;
        self.selected_overview = 0;
        self.status_note = Some(format!(
            "overview sort: {}",
            self.overview_sort_mode.label()
        ));
    }

    pub(crate) fn cycle_mode(&mut self) {
        self.mode = if self.config.overview_enabled {
            self.mode.next()
        } else {
            match self.mode {
                Mode::Overview => Mode::Overseer,
                Mode::Overseer => Mode::Mind,
                Mode::Mind => Mode::Fleet,
                Mode::Fleet => Mode::Work,
                Mode::Work => Mode::Diff,
                Mode::Diff => Mode::Health,
                Mode::Health => Mode::Overseer,
            }
        };
    }

    pub(crate) fn overview_context_hint(&self, row: &OverviewRow) -> String {
        if let Some(snippet) = row
            .snippet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return snippet.to_string();
        }

        let mut active_titles = self
            .hub
            .tasks
            .values()
            .filter(|payload| payload.agent_id == row.identity_key)
            .flat_map(|payload| payload.active_tasks.clone().unwrap_or_default().into_iter())
            .filter(|task| {
                task.status == "in-progress" || task.status == "in_progress" || task.active_agent
            })
            .map(|task| task.title)
            .collect::<Vec<_>>();
        active_titles.sort();
        if let Some(title) = active_titles.into_iter().next() {
            return title;
        }

        match normalize_lifecycle(&row.lifecycle).as_str() {
            "needs-input" => "awaiting input".to_string(),
            "blocked" => "blocked".to_string(),
            "busy" => "working".to_string(),
            "idle" => "idle".to_string(),
            "error" => "error reported".to_string(),
            _ => "running".to_string(),
        }
    }

    pub(crate) fn overview_task_signal(&self, row: &OverviewRow) -> Option<String> {
        let mut total = 0u32;
        let mut in_progress = 0u32;
        for payload in self
            .hub
            .tasks
            .values()
            .filter(|payload| payload.agent_id == row.identity_key)
        {
            total = total.saturating_add(payload.counts.total);
            in_progress = in_progress.saturating_add(payload.counts.in_progress);
        }
        if total == 0 {
            return None;
        }
        Some(format!("W:{in_progress}/{total}"))
    }

    pub(crate) fn overview_git_signal(&self, row: &OverviewRow) -> Option<String> {
        let diff = self.hub.diffs.get(&row.identity_key)?;
        if !diff.git_available {
            return Some("G:n/a".to_string());
        }
        let additions = diff
            .summary
            .staged
            .additions
            .saturating_add(diff.summary.unstaged.additions);
        let deletions = diff
            .summary
            .staged
            .deletions
            .saturating_add(diff.summary.unstaged.deletions);
        let untracked = diff.summary.untracked.files;
        if additions == 0 && deletions == 0 && untracked == 0 {
            return None;
        }
        Some(if untracked > 0 {
            format!("G:+{additions}/-{deletions} ?{untracked}")
        } else {
            format!("G:+{additions}/-{deletions}")
        })
    }

    pub(crate) fn work_rows(&self) -> Vec<WorkProject> {
        if self.prefer_hub_data(!self.hub.tasks.is_empty()) {
            let mut grouped: BTreeMap<(String, String), BTreeMap<String, WorkTagRow>> =
                BTreeMap::new();
            for payload in self.hub.tasks.values() {
                let project_root = self
                    .hub
                    .agents
                    .get(&payload.agent_id)
                    .and_then(|agent| agent.status.as_ref().map(|s| s.project_root.clone()))
                    .unwrap_or_else(|| "(unknown)".to_string());
                let scope = self
                    .hub
                    .agents
                    .get(&payload.agent_id)
                    .and_then(|agent| agent.status.as_ref().and_then(|s| s.agent_label.clone()))
                    .unwrap_or_else(|| extract_label(&payload.agent_id));
                let in_progress_titles = payload
                    .active_tasks
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|task| task.status == "in-progress" || task.status == "in_progress")
                    .map(|task| format!("#{} {}", task.id, task.title))
                    .collect::<Vec<_>>();
                grouped.entry((project_root, scope)).or_default().insert(
                    payload.tag.clone(),
                    WorkTagRow {
                        tag: payload.tag.clone(),
                        counts: payload.counts.clone(),
                        in_progress_titles,
                    },
                );
            }
            let mut rows = Vec::new();
            for ((project_root, scope), tags) in grouped {
                rows.push(WorkProject {
                    project_root,
                    scope,
                    tags: tags.into_values().collect(),
                });
            }
            return rows;
        }
        self.local.work.clone()
    }

    pub(crate) fn diff_rows(&self) -> Vec<DiffProject> {
        if self.prefer_hub_data(!self.hub.diffs.is_empty()) {
            let mut grouped: BTreeMap<String, (DiffProject, Vec<String>)> = BTreeMap::new();
            for payload in self.hub.diffs.values() {
                let scope = self
                    .hub
                    .agents
                    .get(&payload.agent_id)
                    .and_then(|agent| agent.status.as_ref().and_then(|s| s.agent_label.clone()))
                    .unwrap_or_else(|| extract_label(&payload.agent_id));
                let mut files = payload.files.clone();
                if files.len() > MAX_DIFF_FILES {
                    files.truncate(MAX_DIFF_FILES);
                }
                let key = payload.repo_root.clone();
                let entry = grouped.entry(key.clone()).or_insert_with(|| {
                    (
                        DiffProject {
                            project_root: key,
                            scope: String::new(),
                            git_available: payload.git_available,
                            reason: payload.reason.clone(),
                            summary: payload.summary.clone(),
                            files: files.clone(),
                        },
                        Vec::new(),
                    )
                });
                if !entry.1.iter().any(|value| value == &scope) {
                    entry.1.push(scope);
                }
                if entry.0.files.is_empty() && !files.is_empty() {
                    entry.0.files = files;
                }
                if !entry.0.git_available && payload.git_available {
                    entry.0.git_available = true;
                    entry.0.reason = payload.reason.clone();
                    entry.0.summary = payload.summary.clone();
                }
            }
            let mut rows = Vec::new();
            for (_, (mut row, scopes)) in grouped {
                row.scope = scope_summary(&scopes);
                rows.push(row);
            }
            return rows;
        }
        self.local.diff.clone()
    }

    pub(crate) fn health_rows(&self) -> Vec<HealthRow> {
        if self.prefer_hub_data(!self.hub.health.is_empty()) {
            let mut rows = Vec::new();
            for (agent_id, snapshot) in &self.hub.health {
                let status = self
                    .hub
                    .agents
                    .get(agent_id)
                    .and_then(|agent| agent.status.as_ref());
                let scope = status
                    .and_then(|value| value.agent_label.clone())
                    .unwrap_or_else(|| extract_label(agent_id));
                let project_root = status
                    .map(|value| value.project_root.clone())
                    .unwrap_or_else(|| "(unknown)".to_string());
                rows.push(HealthRow {
                    scope,
                    project_root,
                    snapshot: snapshot.clone(),
                });
            }
            rows.sort_by(|left, right| {
                left.project_root
                    .cmp(&right.project_root)
                    .then_with(|| left.scope.cmp(&right.scope))
            });
            return rows;
        }
        vec![HealthRow {
            scope: "local".to_string(),
            project_root: self.config.project_root.to_string_lossy().to_string(),
            snapshot: self.local.health.clone(),
        }]
    }

    pub(crate) fn overseer_snapshot(&self) -> Option<&ObserverSnapshot> {
        self.hub.observer_snapshot.as_ref().filter(|snapshot| {
            self.prefer_hub_data(!snapshot.workers.is_empty() || !snapshot.timeline.is_empty())
        })
    }

    pub(crate) fn overseer_workers(&self) -> Vec<WorkerSnapshot> {
        let Some(snapshot) = self.overseer_snapshot() else {
            return Vec::new();
        };
        let mut workers = snapshot.workers.clone();
        workers.sort_by(|left, right| {
            overseer_attention_rank(&left.attention.level)
                .cmp(&overseer_attention_rank(&right.attention.level))
                .reverse()
                .then_with(|| {
                    overseer_drift_rank(&left.drift_risk)
                        .cmp(&overseer_drift_rank(&right.drift_risk))
                        .reverse()
                })
                .then_with(|| left.agent_id.cmp(&right.agent_id))
        });
        workers
    }

    pub(crate) fn overseer_timeline(&self) -> Vec<ObserverTimelineEntry> {
        let mut entries = if !self.hub.observer_timeline.is_empty() {
            self.hub.observer_timeline.clone()
        } else {
            self.overseer_snapshot()
                .map(|snapshot| snapshot.timeline.clone())
                .unwrap_or_default()
        };
        entries.sort_by(|left, right| {
            right
                .emitted_at_ms
                .unwrap_or_default()
                .cmp(&left.emitted_at_ms.unwrap_or_default())
                .then_with(|| left.agent_id.cmp(&right.agent_id))
        });
        entries
    }

    pub(crate) fn overseer_mind_event(&self, agent_id: &str) -> Option<&MindObserverFeedEvent> {
        let feed = self.hub.mind.get(agent_id)?;
        feed.events.iter().max_by_key(|event| {
            mind_event_sort_ms(event.completed_at.as_deref())
                .or_else(|| mind_event_sort_ms(event.started_at.as_deref()))
                .or_else(|| mind_event_sort_ms(event.enqueued_at.as_deref()))
                .unwrap_or(0)
        })
    }

    pub(crate) fn insight_runtime_rollup(&self) -> Option<InsightRuntimeSnapshot> {
        if !self.prefer_hub_data(!self.hub.insight_runtime.is_empty()) {
            return None;
        }
        let mut agg = InsightRuntimeSnapshot::default();
        for snapshot in self.hub.insight_runtime.values() {
            agg.reflector_enabled = agg.reflector_enabled || snapshot.reflector_enabled;
            agg.reflector_ticks = agg.reflector_ticks.saturating_add(snapshot.reflector_ticks);
            agg.reflector_lock_conflicts = agg
                .reflector_lock_conflicts
                .saturating_add(snapshot.reflector_lock_conflicts);
            agg.reflector_jobs_completed = agg
                .reflector_jobs_completed
                .saturating_add(snapshot.reflector_jobs_completed);
            agg.reflector_jobs_failed = agg
                .reflector_jobs_failed
                .saturating_add(snapshot.reflector_jobs_failed);
            agg.t3_enabled = agg.t3_enabled || snapshot.t3_enabled;
            agg.t3_ticks = agg.t3_ticks.saturating_add(snapshot.t3_ticks);
            agg.t3_lock_conflicts = agg
                .t3_lock_conflicts
                .saturating_add(snapshot.t3_lock_conflicts);
            agg.t3_jobs_completed = agg
                .t3_jobs_completed
                .saturating_add(snapshot.t3_jobs_completed);
            agg.t3_jobs_failed = agg.t3_jobs_failed.saturating_add(snapshot.t3_jobs_failed);
            agg.t3_jobs_requeued = agg
                .t3_jobs_requeued
                .saturating_add(snapshot.t3_jobs_requeued);
            agg.t3_jobs_dead_lettered = agg
                .t3_jobs_dead_lettered
                .saturating_add(snapshot.t3_jobs_dead_lettered);
            agg.t3_queue_depth = agg
                .t3_queue_depth
                .saturating_add(snapshot.t3_queue_depth.max(0));
            agg.supervisor_runs = agg.supervisor_runs.saturating_add(snapshot.supervisor_runs);
            agg.supervisor_failures = agg
                .supervisor_failures
                .saturating_add(snapshot.supervisor_failures);
            agg.queue_depth = agg.queue_depth.saturating_add(snapshot.queue_depth.max(0));
            if agg.last_tick_ms.is_none() || snapshot.last_tick_ms > agg.last_tick_ms {
                agg.last_tick_ms = snapshot.last_tick_ms;
            }
            if agg.last_error.is_none() {
                agg.last_error = snapshot.last_error.clone();
            }
        }
        Some(agg)
    }

    pub(crate) fn insight_detached_jobs(&self) -> Vec<InsightDetachedJob> {
        if !self.prefer_hub_data(!self.hub.insight_detached.is_empty()) {
            return Vec::new();
        }
        let mut jobs = self
            .hub
            .insight_detached
            .iter()
            .filter(|(agent_id, _)| {
                if !self.config.mind_project_scoped {
                    return true;
                }
                self.hub
                    .agents
                    .get(*agent_id)
                    .and_then(|agent| agent.status.as_ref())
                    .map(|status| self.mind_project_matches(&status.project_root))
                    .unwrap_or(false)
            })
            .flat_map(|(_, snapshot)| snapshot.jobs.clone())
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| {
            right
                .created_at_ms
                .cmp(&left.created_at_ms)
                .then_with(|| left.job_id.cmp(&right.job_id))
        });
        jobs
    }

    pub(crate) fn detached_fleet_rows(&self) -> Vec<DetachedFleetRow> {
        if !self.prefer_hub_data(!self.hub.insight_detached.is_empty()) {
            return Vec::new();
        }
        let mut grouped: BTreeMap<(String, u8), Vec<InsightDetachedJob>> = BTreeMap::new();
        for (agent_id, snapshot) in &self.hub.insight_detached {
            let project_root = self
                .hub
                .agents
                .get(agent_id)
                .and_then(|agent| {
                    agent
                        .status
                        .as_ref()
                        .map(|status| status.project_root.clone())
                })
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "(unknown project)".to_string());
            for job in &snapshot.jobs {
                let plane_rank = match job.owner_plane {
                    InsightDetachedOwnerPlane::Delegated => 0,
                    InsightDetachedOwnerPlane::Mind => 1,
                };
                grouped
                    .entry((project_root.clone(), plane_rank))
                    .or_default()
                    .push(job.clone());
            }
        }

        let mut rows = grouped
            .into_iter()
            .map(|((project_root, _), mut jobs)| {
                jobs.sort_by(|left, right| {
                    right
                        .created_at_ms
                        .cmp(&left.created_at_ms)
                        .then_with(|| left.job_id.cmp(&right.job_id))
                });
                DetachedFleetRow {
                    project_root,
                    owner_plane: jobs
                        .first()
                        .map(|job| job.owner_plane)
                        .unwrap_or(InsightDetachedOwnerPlane::Delegated),
                    jobs,
                }
            })
            .filter(|row| self.fleet_plane_filter.matches(row.owner_plane))
            .filter(|row| {
                !self.fleet_active_only
                    || row.jobs.iter().any(|job| {
                        matches!(
                            job.status,
                            InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
                        )
                    })
            })
            .collect::<Vec<_>>();

        let row_rank = |row: &DetachedFleetRow| -> (usize, usize, usize) {
            let mut active = 0usize;
            let mut errorish = 0usize;
            let mut latest_created = 0i64;
            for job in &row.jobs {
                latest_created = latest_created.max(job.created_at_ms);
                match job.status {
                    InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running => {
                        active += 1
                    }
                    InsightDetachedJobStatus::Error
                    | InsightDetachedJobStatus::Fallback
                    | InsightDetachedJobStatus::Stale => errorish += 1,
                    InsightDetachedJobStatus::Success | InsightDetachedJobStatus::Cancelled => {}
                }
            }
            (active, errorish, latest_created as usize)
        };

        rows.sort_by(|left, right| {
            let left_rank = row_rank(left);
            let right_rank = row_rank(right);
            match self.fleet_sort_mode {
                FleetSortMode::Project => {
                    left.project_root.cmp(&right.project_root).then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    })
                }
                FleetSortMode::Newest => right_rank
                    .2
                    .cmp(&left_rank.2)
                    .then_with(|| left.project_root.cmp(&right.project_root))
                    .then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    }),
                FleetSortMode::ActiveFirst => right_rank
                    .0
                    .cmp(&left_rank.0)
                    .then_with(|| right_rank.2.cmp(&left_rank.2))
                    .then_with(|| left.project_root.cmp(&right.project_root))
                    .then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    }),
                FleetSortMode::ErrorFirst => right_rank
                    .1
                    .cmp(&left_rank.1)
                    .then_with(|| right_rank.0.cmp(&left_rank.0))
                    .then_with(|| right_rank.2.cmp(&left_rank.2))
                    .then_with(|| left.project_root.cmp(&right.project_root))
                    .then_with(|| {
                        detached_owner_plane_label(left.owner_plane)
                            .cmp(detached_owner_plane_label(right.owner_plane))
                    }),
            }
        });
        rows
    }

    pub(crate) fn selected_fleet_index_for_rows(&self, rows: &[DetachedFleetRow]) -> usize {
        self.selected_fleet.min(rows.len().saturating_sub(1))
    }

    pub(crate) fn move_fleet_selection(&mut self, step: i32) {
        let rows = self.detached_fleet_rows();
        if rows.is_empty() {
            self.selected_fleet = 0;
            self.selected_fleet_job = 0;
            return;
        }
        let current = self.selected_fleet_index_for_rows(&rows) as i32;
        let max = rows.len().saturating_sub(1) as i32;
        let next = (current + step).clamp(0, max) as usize;
        self.selected_fleet = next;
        self.selected_fleet_job = 0;
    }

    pub(crate) fn selected_fleet_job_index_for_row(&self, row: &DetachedFleetRow) -> usize {
        self.selected_fleet_job
            .min(row.jobs.len().saturating_sub(1))
    }

    pub(crate) fn move_fleet_job_selection(&mut self, step: i32) {
        let Some(row) = self.selected_fleet_row() else {
            return;
        };
        if row.jobs.is_empty() {
            self.selected_fleet_job = 0;
            return;
        }
        let current = self.selected_fleet_job_index_for_row(&row) as i32;
        let max = row.jobs.len().saturating_sub(1) as i32;
        let next = (current + step).clamp(0, max) as usize;
        self.selected_fleet_job = next;
    }

    pub(crate) fn toggle_fleet_plane_filter(&mut self) {
        self.fleet_plane_filter = self.fleet_plane_filter.next();
        self.selected_fleet = 0;
        self.selected_fleet_job = 0;
        self.scroll = 0;
        self.status_note = Some(format!("fleet plane: {}", self.fleet_plane_filter.label()));
    }

    pub(crate) fn toggle_fleet_active_only(&mut self) {
        self.fleet_active_only = !self.fleet_active_only;
        self.selected_fleet = 0;
        self.selected_fleet_job = 0;
        self.scroll = 0;
        self.status_note = Some(if self.fleet_active_only {
            "fleet scope: active-only".to_string()
        } else {
            "fleet scope: all jobs".to_string()
        });
    }

    pub(crate) fn toggle_fleet_sort_mode(&mut self) {
        self.fleet_sort_mode = self.fleet_sort_mode.next();
        self.selected_fleet = 0;
        self.selected_fleet_job = 0;
        self.scroll = 0;
        self.status_note = Some(format!("fleet sort: {}", self.fleet_sort_mode.label()));
    }

    pub(crate) fn selected_fleet_row(&mut self) -> Option<DetachedFleetRow> {
        if self.mode != Mode::Fleet {
            self.status_note = Some("switch to Fleet mode for detached job actions".to_string());
            return None;
        }
        let rows = self.detached_fleet_rows();
        if rows.is_empty() {
            self.status_note = Some("no detached fleet groups available".to_string());
            return None;
        }
        let selected = self.selected_fleet_index_for_rows(&rows);
        self.selected_fleet = selected;
        let row = rows.get(selected).cloned();
        if let Some(row) = row.as_ref() {
            self.selected_fleet_job = self.selected_fleet_job_index_for_row(row);
        }
        row
    }

    pub(crate) fn selected_fleet_job(&mut self) -> Option<(DetachedFleetRow, InsightDetachedJob)> {
        let row = self.selected_fleet_row()?;
        let Some(job) = row
            .jobs
            .get(self.selected_fleet_job_index_for_row(&row))
            .cloned()
        else {
            self.status_note = Some("selected fleet group has no jobs".to_string());
            return None;
        };
        Some((row, job))
    }

    pub(crate) fn render_fleet_brief(
        &self,
        row: &DetachedFleetRow,
        job: &InsightDetachedJob,
        handoff_only: bool,
    ) -> String {
        let target = job
            .agent
            .as_deref()
            .or(job.chain.as_deref())
            .or(job.team.as_deref())
            .unwrap_or("detached-job");
        let summary = job
            .output_excerpt
            .as_deref()
            .or(job.error.as_deref())
            .unwrap_or("No detached job summary available.");
        let followup = match job.owner_plane {
            InsightDetachedOwnerPlane::Delegated => {
                let action = if handoff_only {
                    "/subagent-handoff"
                } else {
                    "/subagent-inspect"
                };
                format!(
                    "- In the owning Pi session, run: `{action} {}`\n- If the job is still active, optionally cancel from Mission Control Fleet with `x`.\n- If more context is needed, compare against recent jobs in the same project/plane group.",
                    job.job_id,
                )
            }
            InsightDetachedOwnerPlane::Mind => format!(
                "- In the owning Pi session, reopen Mission Control Fleet or Mind for project `{}` and inspect detached job `{}`.\n- If the job is still active, optionally cancel from Mission Control Fleet with `x`.\n- Compare against other recent Mind jobs in the same project group before re-running any upstream workflow.",
                row.project_root,
                job.job_id,
            ),
        };
        let recovery = detached_job_recovery_guidance(job)
            .into_iter()
            .map(|line| format!("- {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "# Mission Control fleet brief\n\n- project: {}\n- job_id: {}\n- target: {}\n- owner_plane: {}\n- worker_kind: {}\n- status: {}\n- fallback_used: {}\n\n## Detached summary\n{}\n\n## Recovery guidance\n{}\n\n## Main session follow-up\n{}\n",
            row.project_root,
            job.job_id,
            target,
            detached_owner_plane_label(job.owner_plane),
            detached_worker_kind_display(job.owner_plane, job.worker_kind),
            detached_job_status_label(job.status),
            if job.fallback_used { "yes" } else { "no" },
            summary,
            recovery,
            followup,
        )
    }

    pub(crate) fn write_fleet_brief(
        &self,
        row: &DetachedFleetRow,
        job: &InsightDetachedJob,
        handoff_only: bool,
    ) -> Result<PathBuf, String> {
        let dir = self.config.state_dir.join("mission-control").join("fleet");
        fs::create_dir_all(&dir).map_err(|err| format!("create fleet brief dir failed: {err}"))?;
        let slug = sanitize_slug(&format!(
            "{}-{}-{}",
            if handoff_only { "handoff" } else { "inspect" },
            job.job_id,
            Utc::now().format("%Y%m%d%H%M%S")
        ));
        let path = dir.join(format!("{slug}.md"));
        fs::write(&path, self.render_fleet_brief(row, job, handoff_only))
            .map_err(|err| format!("write fleet brief failed: {err}"))?;
        Ok(path)
    }

    pub(crate) fn launch_fleet_followup(&mut self, handoff_only: bool) {
        let Some((row, job)) = self.selected_fleet_job() else {
            return;
        };
        let brief_path = match self.write_fleet_brief(&row, &job, handoff_only) {
            Ok(path) => path,
            Err(err) => {
                self.status_note = Some(err);
                return;
            }
        };
        let project_root = PathBuf::from(&row.project_root);
        let launch_root = if project_root.exists() {
            project_root
        } else {
            self.config.project_root.clone()
        };
        let tab_name = if handoff_only {
            format!("Handoff {}", ellipsize(&job.job_id, 18))
        } else {
            format!("Inspect {}", ellipsize(&job.job_id, 18))
        };
        let agent_id = resolve_launch_agent_id();
        let plan = build_worker_launch_plan(
            &launch_root,
            &agent_id,
            &tab_name,
            Some(&brief_path),
            in_zellij_session(),
        );
        match execute_worker_launch_plan(&plan) {
            Ok(()) => {
                self.status_note = Some(format!(
                    "launched {} follow-up for {}; brief: {}",
                    if handoff_only { "handoff" } else { "inspect" },
                    job.job_id,
                    brief_path.display()
                ));
            }
            Err(err) => {
                self.status_note = Some(format!("fleet launch failed: {err}"));
            }
        }
    }

    pub(crate) fn cancel_selected_fleet_job(&mut self) {
        let Some((_row, job)) = self.selected_fleet_job() else {
            return;
        };
        if !matches!(
            job.status,
            InsightDetachedJobStatus::Queued | InsightDetachedJobStatus::Running
        ) {
            self.status_note = Some(format!("selected job {} is not active", job.job_id));
            return;
        }
        self.queue_hub_command(
            "insight_detached_cancel",
            None,
            serde_json::json!({"job_id": job.job_id, "reason": "mission_control_fleet"}),
            format!("detached job {}", job.job_id),
        );
    }

    pub(crate) fn focus_selected_fleet_project(&mut self) {
        let Some(row) = self.selected_fleet_row() else {
            return;
        };
        let candidate = self
            .hub
            .agents
            .iter()
            .filter_map(|(agent_id, agent)| {
                let status = agent.status.as_ref()?;
                if status.project_root != row.project_root {
                    return None;
                }
                let tab_meta = self
                    .active_hub_layout()
                    .and_then(|layout| layout.pane_tabs.get(&status.pane_id))
                    .cloned()
                    .or_else(|| self.tab_cache.get(&status.pane_id).cloned());
                Some((agent_id.clone(), status.pane_id.clone(), tab_meta))
            })
            .next();
        let Some((agent_id, pane_id, tab_meta)) = candidate else {
            self.status_note = Some(format!(
                "no live tab found for project {}",
                row.project_root
            ));
            return;
        };
        self.focus_tab_target(
            &pane_id,
            tab_meta.as_ref().map(|meta| meta.index),
            tab_meta.map(|meta| meta.name),
        );
        self.status_note = Some(format!("focused project tab via {}", agent_id));
    }

    pub(crate) fn mind_project_matches(&self, project_root: &str) -> bool {
        if !self.config.mind_project_scoped {
            return true;
        }
        let candidate = normalized_project_root_key(project_root);
        !candidate.is_empty()
            && candidate == normalized_project_root_key(&self.config.project_root.to_string_lossy())
    }

    pub(crate) fn mind_rows_for_lane(&self, lane_filter: MindLaneFilter) -> Vec<MindObserverRow> {
        if !self.prefer_hub_data(!self.hub.mind.is_empty()) {
            return Vec::new();
        }

        let viewer_scope = self.config.tab_scope.as_deref();
        let mut rows = Vec::new();
        for (agent_id, feed) in &self.hub.mind {
            if feed.events.is_empty() {
                continue;
            }
            let status = self
                .hub
                .agents
                .get(agent_id)
                .and_then(|agent| agent.status.as_ref());
            let scope = status
                .and_then(|value| value.agent_label.clone())
                .unwrap_or_else(|| extract_label(agent_id));
            let pane_id = status
                .map(|value| value.pane_id.clone())
                .unwrap_or_else(|| extract_pane_id(agent_id));
            let tab_scope = status.and_then(|value| value.tab_scope.clone());
            let tab_focused = tab_scope_matches(viewer_scope, tab_scope.as_deref());
            if let Some(project_root) = status.map(|value| value.project_root.as_str()) {
                if !self.mind_project_matches(project_root) {
                    continue;
                }
            } else if self.config.mind_project_scoped {
                continue;
            }
            if !self.mind_show_all_tabs && viewer_scope.is_some() && !tab_focused {
                continue;
            }

            for event in &feed.events {
                let lane = mind_event_lane(event);
                if !mind_lane_matches(lane_filter, lane) {
                    continue;
                }
                rows.push(MindObserverRow {
                    agent_id: agent_id.clone(),
                    scope: scope.clone(),
                    pane_id: pane_id.clone(),
                    tab_scope: tab_scope.clone(),
                    tab_focused,
                    event: event.clone(),
                    source: "hub".to_string(),
                });
            }
        }

        rows.sort_by(|left, right| {
            let left_ts = mind_event_sort_ms(left.event.completed_at.as_deref())
                .or_else(|| mind_event_sort_ms(left.event.started_at.as_deref()))
                .or_else(|| mind_event_sort_ms(left.event.enqueued_at.as_deref()))
                .unwrap_or(0);
            let right_ts = mind_event_sort_ms(right.event.completed_at.as_deref())
                .or_else(|| mind_event_sort_ms(right.event.started_at.as_deref()))
                .or_else(|| mind_event_sort_ms(right.event.enqueued_at.as_deref()))
                .unwrap_or(0);
            right_ts
                .cmp(&left_ts)
                .then_with(|| right.tab_focused.cmp(&left.tab_focused))
                .then_with(|| left.scope.cmp(&right.scope))
                .then_with(|| left.pane_id.cmp(&right.pane_id))
        });
        rows
    }

    pub(crate) fn mind_rows(&self) -> Vec<MindObserverRow> {
        self.mind_rows_for_lane(self.mind_lane)
    }

    pub(crate) fn mind_injection_rows(&self) -> Vec<MindInjectionRow> {
        if !self.prefer_hub_data(!self.hub.mind_injection.is_empty()) {
            return Vec::new();
        }

        let viewer_scope = self.config.tab_scope.as_deref();
        let mut rows = Vec::new();
        for (agent_id, payload) in &self.hub.mind_injection {
            let status = self
                .hub
                .agents
                .get(agent_id)
                .and_then(|agent| agent.status.as_ref());
            let scope = status
                .and_then(|value| value.agent_label.clone())
                .unwrap_or_else(|| extract_label(agent_id));
            let pane_id = status
                .map(|value| value.pane_id.clone())
                .unwrap_or_else(|| extract_pane_id(agent_id));
            let tab_scope = status.and_then(|value| value.tab_scope.clone());
            let tab_focused = tab_scope_matches(viewer_scope, tab_scope.as_deref());
            if let Some(project_root) = status.map(|value| value.project_root.as_str()) {
                if !self.mind_project_matches(project_root) {
                    continue;
                }
            } else if self.config.mind_project_scoped {
                continue;
            }
            if !self.mind_show_all_tabs && viewer_scope.is_some() && !tab_focused {
                continue;
            }
            rows.push(MindInjectionRow {
                scope,
                pane_id,
                tab_focused,
                payload: payload.clone(),
            });
        }

        rows.sort_by(|left, right| {
            let left_ts = mind_event_sort_ms(Some(&left.payload.queued_at)).unwrap_or(0);
            let right_ts = mind_event_sort_ms(Some(&right.payload.queued_at)).unwrap_or(0);
            right_ts
                .cmp(&left_ts)
                .then_with(|| right.tab_focused.cmp(&left.tab_focused))
                .then_with(|| left.scope.cmp(&right.scope))
                .then_with(|| left.pane_id.cmp(&right.pane_id))
        });
        rows
    }

    pub(crate) fn mind_target_agent(&self) -> Option<OverviewRow> {
        let rows = self.overview_rows();
        rows.into_iter()
            .find(|row| row.tab_focused && self.mind_project_matches(&row.project_root))
            .or_else(|| {
                self.hub.agents.iter().find_map(|(agent_id, agent)| {
                    let status = agent.status.as_ref()?;
                    let tab_focused = tab_scope_matches(
                        self.config.tab_scope.as_deref(),
                        status.tab_scope.as_deref(),
                    );
                    if !tab_focused || !self.mind_project_matches(&status.project_root) {
                        return None;
                    }
                    Some(OverviewRow {
                        identity_key: agent_id.clone(),
                        label: status
                            .agent_label
                            .clone()
                            .unwrap_or_else(|| extract_label(agent_id)),
                        lifecycle: status.status.clone(),
                        snippet: status.message.clone(),
                        pane_id: status.pane_id.clone(),
                        tab_index: None,
                        tab_name: status.tab_scope.clone(),
                        tab_focused,
                        project_root: status.project_root.clone(),
                        online: true,
                        age_secs: None,
                        source: "hub".to_string(),
                        session_title: status.session_title.clone(),
                        chat_title: status.chat_title.clone(),
                    })
                })
            })
            .or_else(|| {
                self.hub.agents.iter().find_map(|(agent_id, agent)| {
                    let status = agent.status.as_ref();
                    if self.config.mind_project_scoped
                        && !status
                            .map(|value| self.mind_project_matches(&value.project_root))
                            .unwrap_or(false)
                    {
                        return None;
                    }
                    Some(OverviewRow {
                        identity_key: agent_id.clone(),
                        label: status
                            .and_then(|value| value.agent_label.clone())
                            .unwrap_or_else(|| extract_label(agent_id)),
                        lifecycle: status
                            .map(|value| value.status.clone())
                            .unwrap_or_else(|| "unknown".to_string()),
                        snippet: status.and_then(|value| value.message.clone()),
                        pane_id: status
                            .map(|value| value.pane_id.clone())
                            .unwrap_or_else(|| extract_pane_id(agent_id)),
                        tab_index: None,
                        tab_name: status.and_then(|value| value.tab_scope.clone()),
                        tab_focused: false,
                        project_root: status
                            .map(|value| value.project_root.clone())
                            .unwrap_or_else(|| "(unknown)".to_string()),
                        online: true,
                        age_secs: None,
                        source: "hub".to_string(),
                        session_title: status.and_then(|value| value.session_title.clone()),
                        chat_title: status.and_then(|value| value.chat_title.clone()),
                    })
                })
            })
    }

    pub(crate) fn selected_overseer_worker(&mut self) -> Option<WorkerSnapshot> {
        if self.mode != Mode::Overseer {
            self.status_note = Some("switch to Overseer mode for worker consultation".to_string());
            return None;
        }
        let workers = self.overseer_workers();
        if workers.is_empty() {
            self.status_note = Some("no workers available for consultation".to_string());
            return None;
        }
        let selected = self.selected_overview.min(workers.len().saturating_sub(1));
        self.selected_overview = selected;
        workers.get(selected).cloned()
    }

    pub(crate) fn selected_overview_row(&mut self) -> Option<OverviewRow> {
        let rows = self.overview_rows();
        if rows.is_empty() {
            self.status_note = Some("no agents available".to_string());
            return None;
        }
        let selected = self.selected_overview_index_for_rows(&rows);
        self.selected_overview = selected;
        rows.get(selected).cloned()
    }

    pub(crate) fn selected_pane_target(&mut self) -> Option<(String, String, PathBuf)> {
        match self.mode {
            Mode::Overview => self.selected_overview_row().map(|row| {
                let project_root = if row.project_root.trim().is_empty() {
                    self.config.project_root.clone()
                } else {
                    PathBuf::from(row.project_root)
                };
                (row.pane_id, row.label, project_root)
            }),
            Mode::Overseer => self.selected_overseer_worker().map(|worker| {
                (
                    worker.pane_id,
                    worker.agent_id,
                    self.config.project_root.clone(),
                )
            }),
            Mode::Mind => self.mind_target_agent().map(|row| {
                let project_root = if row.project_root.trim().is_empty() {
                    self.config.project_root.clone()
                } else {
                    PathBuf::from(row.project_root)
                };
                (row.pane_id, row.label, project_root)
            }),
            _ => {
                self.status_note = Some(
                    "pane evidence is available in Overview, Overseer, or Mind mode".to_string(),
                );
                None
            }
        }
    }

    pub(crate) fn capture_selected_pane_evidence(&mut self) {
        let Some((pane_id, label, _project_root)) = self.selected_pane_target() else {
            return;
        };
        let dir = self
            .config
            .state_dir
            .join("mission-control")
            .join("pane-evidence");
        if let Err(err) = fs::create_dir_all(&dir) {
            self.status_note = Some(format!("create evidence dir failed: {err}"));
            return;
        }
        let stamp = Utc::now().format("%Y%m%d%H%M%S");
        let filename = format!(
            "{}-pane-{}-{}.ansi",
            sanitize_slug(&label),
            sanitize_slug(&pane_id),
            stamp
        );
        let path = dir.join(filename);
        match dump_pane_evidence(&self.config.session_id, &pane_id, &path) {
            Ok(()) => {
                self.status_note = Some(format!(
                    "pane evidence saved for {} ({}) -> {}",
                    label,
                    pane_id,
                    path.display()
                ));
            }
            Err(err) => {
                self.status_note = Some(format!("pane evidence failed for {pane_id}: {err}"));
            }
        }
    }

    pub(crate) fn follow_selected_pane_live(&mut self) {
        let Some((pane_id, label, project_root)) = self.selected_pane_target() else {
            return;
        };
        if !in_zellij_session() {
            self.status_note =
                Some("live pane follow requires running Mission Control inside Zellij".to_string());
            return;
        }
        match launch_pane_follow(&self.config.session_id, &pane_id, &label, &project_root) {
            Ok(()) => {
                self.status_note = Some(format!(
                    "live pane follow opened for {} ({})",
                    label, pane_id
                ));
            }
            Err(err) => {
                self.status_note = Some(format!("live pane follow failed for {pane_id}: {err}"));
            }
        }
    }

    pub(crate) fn consultation_peer_for(&self, focal_agent_id: &str) -> Option<WorkerSnapshot> {
        self.overseer_workers()
            .into_iter()
            .filter(|worker| worker.agent_id != focal_agent_id)
            .max_by_key(|worker| {
                let status_rank = match worker.status {
                    WorkerStatus::Active => 4,
                    WorkerStatus::Done => 3,
                    WorkerStatus::Idle => 2,
                    WorkerStatus::NeedsInput | WorkerStatus::Blocked => 1,
                    WorkerStatus::Paused | WorkerStatus::Offline => 0,
                };
                let aligned = matches!(worker.plan_alignment, PlanAlignment::High) as u8;
                (status_rank, aligned)
            })
    }

    pub(crate) fn request_overseer_consultation(&mut self, kind: ConsultationPacketKind) {
        let Some(requester) = self.selected_overseer_worker() else {
            return;
        };
        let Some(responder) = self.consultation_peer_for(&requester.agent_id) else {
            self.status_note = Some("need at least two workers for peer consultation".to_string());
            return;
        };
        if !self.connected {
            self.status_note = Some("hub offline; consultation unavailable".to_string());
            return;
        }

        let checkpoint = self.latest_compaction_checkpoint();
        let mind_event = self.overseer_mind_event(&requester.agent_id);
        let packet =
            derive_overseer_consultation_packet(&requester, checkpoint.as_ref(), mind_event)
                .normalize();
        let request_packet = ConsultationPacket { kind, ..packet };
        let request_id = self.next_command_request_id();
        let consultation_id = format!(
            "{}:{}:{}",
            requester.session_id, requester.agent_id, request_id
        );
        let outbound = HubOutbound {
            request_id: request_id.clone(),
            msg: WireMsg::ConsultationRequest(ConsultationRequestPayload {
                consultation_id,
                requesting_agent_id: requester.agent_id.clone(),
                target_agent_id: responder.agent_id.clone(),
                packet: request_packet.clone(),
            }),
        };
        match self.command_tx.try_send(outbound) {
            Ok(()) => {
                self.pending_consultations.insert(
                    request_id,
                    PendingConsultation {
                        kind,
                        requester: requester.agent_id.clone(),
                        responder: responder.agent_id.clone(),
                        request_packet,
                    },
                );
                self.status_note = Some(format!(
                    "consult {:?} queued {} -> {}",
                    kind, requester.agent_id, responder.agent_id
                ));
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.status_note = Some("hub consultation queue full".to_string());
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.status_note = Some("hub consultation channel closed".to_string());
            }
        }
    }

    pub(crate) fn selected_overseer_worker_ref(&self) -> Option<WorkerSnapshot> {
        let workers = self.overseer_workers();
        workers
            .get(self.selected_overview.min(workers.len().saturating_sub(1)))
            .cloned()
    }

    pub(crate) fn orchestrator_tools(&self) -> Vec<OrchestratorTool> {
        let selected = self.selected_overseer_worker_ref();
        let has_peer = selected
            .as_ref()
            .map(|worker| self.consultation_peer_for(&worker.agent_id).is_some())
            .unwrap_or(false);
        let snapshot_ready = self.overseer_snapshot().is_some() || self.connected;
        let timeline_ready = !self.overseer_timeline().is_empty() || self.connected;
        let launch_ready = self.worker_launch_supported();

        let mut tools = vec![
            OrchestratorTool {
                id: OrchestratorToolId::SessionSnapshot,
                label: "session snapshot",
                scope: "session",
                shortcut: None,
                status: if snapshot_ready {
                    OrchestratorToolStatus::Ready
                } else {
                    OrchestratorToolStatus::Unavailable
                },
                summary: "inspect the current worker snapshot".to_string(),
                reason: (!snapshot_ready).then(|| "waiting for hub snapshot".to_string()),
            },
            OrchestratorTool {
                id: OrchestratorToolId::SessionTimeline,
                label: "session timeline",
                scope: "session",
                shortcut: None,
                status: if timeline_ready {
                    OrchestratorToolStatus::Ready
                } else {
                    OrchestratorToolStatus::Unavailable
                },
                summary: "inspect recent overseer events".to_string(),
                reason: (!timeline_ready).then(|| "waiting for hub timeline".to_string()),
            },
        ];

        if let Some(worker) = selected {
            let worker_target = worker.agent_id.clone();
            tools.extend([
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerFocus,
                    label: "focus worker tab",
                    scope: "worker",
                    shortcut: Some("Enter"),
                    status: OrchestratorToolStatus::Ready,
                    summary: format!("focus {worker_target} in zellij"),
                    reason: None,
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerReview,
                    label: "peer review",
                    scope: "worker",
                    shortcut: Some("c"),
                    status: if self.connected && has_peer {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("request bounded peer review for {worker_target}"),
                    reason: if !self.connected {
                        Some("hub offline".to_string())
                    } else if !has_peer {
                        Some("need another in-session worker".to_string())
                    } else {
                        None
                    },
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerHelp,
                    label: "peer unblock",
                    scope: "worker",
                    shortcut: Some("u"),
                    status: if self.connected && has_peer {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("request unblock/help guidance for {worker_target}"),
                    reason: if !self.connected {
                        Some("hub offline".to_string())
                    } else if !has_peer {
                        Some("need another in-session worker".to_string())
                    } else {
                        None
                    },
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerObserve,
                    label: "run observer",
                    scope: "worker",
                    shortcut: Some("o"),
                    status: if self.connected {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("request fresh observer run for {worker_target}"),
                    reason: (!self.connected).then(|| "hub offline".to_string()),
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerStop,
                    label: "stop worker",
                    scope: "worker",
                    shortcut: Some("x"),
                    status: if self.connected {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!("stop {worker_target} via hub command"),
                    reason: (!self.connected).then(|| "hub offline".to_string()),
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerSpawn,
                    label: "spawn worker",
                    scope: "session",
                    shortcut: Some("s"),
                    status: if launch_ready {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: "launch a fresh worker tab from Mission Control".to_string(),
                    reason: (!launch_ready)
                        .then(|| "project root unavailable for launcher".to_string()),
                },
                OrchestratorTool {
                    id: OrchestratorToolId::WorkerDelegate,
                    label: "delegate task",
                    scope: "worker",
                    shortcut: Some("d"),
                    status: if launch_ready {
                        OrchestratorToolStatus::Ready
                    } else {
                        OrchestratorToolStatus::Unavailable
                    },
                    summary: format!(
                        "spawn a delegated worker with bounded brief for {worker_target}"
                    ),
                    reason: (!launch_ready)
                        .then(|| "project root unavailable for launcher".to_string()),
                },
            ]);
        }

        tools
    }

    pub(crate) fn orchestration_graph_ir(&self) -> OrchestrationGraphIr {
        let workers = self.overseer_workers();
        let selected = self.selected_overseer_worker_ref();
        let tools = self.orchestrator_tools();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let session_id = selected
            .as_ref()
            .map(|worker| worker.session_id.clone())
            .or_else(|| {
                self.overseer_snapshot()
                    .map(|snapshot| snapshot.session_id.clone())
            })
            .unwrap_or_else(|| self.config.session_id.clone());
        let session_node_id = format!("session:{session_id}");
        let mut session_attrs = BTreeMap::new();
        session_attrs.insert(
            "hub".to_string(),
            if self.connected { "online" } else { "offline" }.to_string(),
        );
        session_attrs.insert(
            "mode".to_string(),
            format!("{:?}", self.mode).to_ascii_lowercase(),
        );
        nodes.push(OrchestrationGraphNode {
            id: session_node_id.clone(),
            kind: OrchestrationGraphNodeKind::Session,
            label: "Mission Control session".to_string(),
            status: if self.connected { "online" } else { "offline" }.to_string(),
            attrs: session_attrs,
        });

        for worker in workers {
            let worker_id = format!("worker:{}", worker.agent_id);
            let mut attrs = BTreeMap::new();
            attrs.insert("pane_id".to_string(), worker.pane_id.clone());
            if let Some(role) = worker.role.as_ref() {
                attrs.insert("role".to_string(), role.clone());
            }
            if let Some(task_id) = worker.assignment.task_id.as_ref() {
                attrs.insert("task_id".to_string(), task_id.clone());
            }
            if let Some(tag) = worker.assignment.tag.as_ref() {
                attrs.insert("tag".to_string(), tag.clone());
            }
            if selected
                .as_ref()
                .map(|candidate| candidate.agent_id == worker.agent_id)
                .unwrap_or(false)
            {
                attrs.insert("selected".to_string(), "true".to_string());
            }
            nodes.push(OrchestrationGraphNode {
                id: worker_id.clone(),
                kind: OrchestrationGraphNodeKind::Worker,
                label: worker.agent_id.clone(),
                status: format!("{:?}", worker.status).to_ascii_lowercase(),
                attrs,
            });
            edges.push(OrchestrationGraphEdge {
                from: session_node_id.clone(),
                to: worker_id.clone(),
                kind: if selected
                    .as_ref()
                    .map(|candidate| candidate.agent_id == worker.agent_id)
                    .unwrap_or(false)
                {
                    OrchestrationGraphEdgeKind::Selects
                } else {
                    OrchestrationGraphEdgeKind::Enumerates
                },
                summary: if selected
                    .as_ref()
                    .map(|candidate| candidate.agent_id == worker.agent_id)
                    .unwrap_or(false)
                {
                    "selected worker in current overseer view".to_string()
                } else {
                    "worker snapshot in current session".to_string()
                },
            });
        }

        for tool in &tools {
            let tool_id = format!("tool:{}", orchestrator_tool_id_slug(tool.id));
            let mut attrs = BTreeMap::new();
            attrs.insert("scope".to_string(), tool.scope.to_string());
            if let Some(shortcut) = tool.shortcut {
                attrs.insert("shortcut".to_string(), shortcut.to_string());
            }
            nodes.push(OrchestrationGraphNode {
                id: tool_id.clone(),
                kind: OrchestrationGraphNodeKind::Tool,
                label: tool.label.to_string(),
                status: match tool.status {
                    OrchestratorToolStatus::Ready => "ready",
                    OrchestratorToolStatus::Unavailable => "blocked",
                }
                .to_string(),
                attrs,
            });
            edges.push(OrchestrationGraphEdge {
                from: session_node_id.clone(),
                to: tool_id.clone(),
                kind: OrchestrationGraphEdgeKind::Enumerates,
                summary: "tool surfaced in Mission Control".to_string(),
            });

            if let Some(worker) = selected.as_ref() {
                if tool.scope == "worker" {
                    edges.push(OrchestrationGraphEdge {
                        from: tool_id.clone(),
                        to: format!("worker:{}", worker.agent_id),
                        kind: OrchestrationGraphEdgeKind::OperatesOn,
                        summary: tool.summary.clone(),
                    });
                }
                if tool.id == OrchestratorToolId::WorkerDelegate {
                    let artifact_id = format!(
                        "artifact:delegation-brief:{}",
                        sanitize_slug(&worker.agent_id)
                    );
                    let mut attrs = BTreeMap::new();
                    attrs.insert(
                        "path".to_string(),
                        self.config
                            .state_dir
                            .join("mission-control")
                            .join("delegations")
                            .to_string_lossy()
                            .to_string(),
                    );
                    nodes.push(OrchestrationGraphNode {
                        id: artifact_id.clone(),
                        kind: OrchestrationGraphNodeKind::Artifact,
                        label: "delegation brief".to_string(),
                        status: match tool.status {
                            OrchestratorToolStatus::Ready => "ready",
                            OrchestratorToolStatus::Unavailable => "blocked",
                        }
                        .to_string(),
                        attrs,
                    });
                    edges.push(OrchestrationGraphEdge {
                        from: tool_id.clone(),
                        to: artifact_id.clone(),
                        kind: OrchestrationGraphEdgeKind::Writes,
                        summary: "write bounded delegation brief before launch".to_string(),
                    });
                    edges.push(OrchestrationGraphEdge {
                        from: artifact_id,
                        to: format!("worker:{}", worker.agent_id),
                        kind: OrchestrationGraphEdgeKind::DelegatesFrom,
                        summary: "delegated worker inherits bounded source context".to_string(),
                    });
                }
                if matches!(
                    tool.id,
                    OrchestratorToolId::WorkerSpawn | OrchestratorToolId::WorkerDelegate
                ) {
                    edges.push(OrchestrationGraphEdge {
                        from: tool_id,
                        to: session_node_id.clone(),
                        kind: OrchestrationGraphEdgeKind::Launches,
                        summary: "compile path launches a fresh worker tab".to_string(),
                    });
                }
            }
        }

        let compile_paths = tools
            .iter()
            .filter(|tool| {
                matches!(
                    tool.id,
                    OrchestratorToolId::WorkerReview
                        | OrchestratorToolId::WorkerHelp
                        | OrchestratorToolId::WorkerObserve
                        | OrchestratorToolId::WorkerStop
                        | OrchestratorToolId::WorkerSpawn
                        | OrchestratorToolId::WorkerDelegate
                )
            })
            .map(|tool| self.compile_orchestration_path(tool, selected.as_ref()))
            .collect();

        OrchestrationGraphIr {
            session_id,
            selected_worker_id: selected.as_ref().map(|worker| worker.agent_id.clone()),
            nodes,
            edges,
            compile_paths,
        }
    }

    pub(crate) fn compile_orchestration_path(
        &self,
        tool: &OrchestratorTool,
        selected: Option<&WorkerSnapshot>,
    ) -> OrchestrationCompilePath {
        let selected_label = selected
            .map(|worker| worker.agent_id.clone())
            .unwrap_or_else(|| "selected worker".to_string());
        let steps = match tool.id {
            OrchestratorToolId::SessionSnapshot => {
                vec!["inspect current session snapshot".to_string()]
            }
            OrchestratorToolId::SessionTimeline => {
                vec!["inspect recent overseer timeline".to_string()]
            }
            OrchestratorToolId::WorkerFocus => vec![
                format!("resolve tab target for {selected_label}"),
                "focus target tab in zellij when metadata is present".to_string(),
            ],
            OrchestratorToolId::WorkerReview => vec![
                format!("select requester {selected_label}"),
                "resolve a peer worker in the same session".to_string(),
                "queue bounded peer review consultation through hub".to_string(),
            ],
            OrchestratorToolId::WorkerHelp => vec![
                format!("select requester {selected_label}"),
                "resolve a peer worker in the same session".to_string(),
                "queue bounded unblock/help consultation through hub".to_string(),
            ],
            OrchestratorToolId::WorkerObserve => vec![
                format!("select target {selected_label}"),
                "queue run_observer command through hub".to_string(),
            ],
            OrchestratorToolId::WorkerStop => vec![
                format!("select target {selected_label}"),
                "queue stop_agent command through hub".to_string(),
            ],
            OrchestratorToolId::WorkerSpawn => vec![
                format!(
                    "resolve launch agent from env/current agent for {}",
                    self.config.session_id
                ),
                format!("compile worker tab name {}", self.next_worker_tab_name()),
                "launch fresh worker tab via aoc-new-tab or aoc-launch".to_string(),
                "return focus to Mission Control tab when possible".to_string(),
            ],
            OrchestratorToolId::WorkerDelegate => vec![
                format!("select source worker {selected_label}"),
                "render bounded delegation brief from worker snapshot".to_string(),
                "write delegation brief under state_dir/mission-control/delegations".to_string(),
                format!(
                    "compile delegated tab name {}",
                    selected
                        .map(App::delegation_tab_name)
                        .unwrap_or_else(|| "delegated-worker".to_string())
                ),
                "launch delegated worker tab and export AOC_DELEGATION_BRIEF_PATH".to_string(),
                "return focus to Mission Control tab when possible".to_string(),
            ],
        };
        OrchestrationCompilePath {
            entry_tool: tool.id,
            review_label: tool.label.to_string(),
            status: tool.status,
            steps,
        }
    }

    pub(crate) fn request_manual_observer_run(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for observer run".to_string());
            return;
        };
        self.queue_hub_command(
            "run_observer",
            Some(target.identity_key.clone()),
            serde_json::json!({"trigger": "manual_shortcut", "reason": "pulse_user_request"}),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    pub(crate) fn request_insight_dispatch_chain(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for insight dispatch".to_string());
            return;
        };
        self.queue_hub_command(
            "insight_dispatch",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "mode": "chain",
                "chain": "insight-handoff",
                "reason": "pulse_mind_action",
                "input": "Mind panel dispatch (T1 -> T2)"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    pub(crate) fn request_insight_bootstrap(&mut self, dry_run: bool) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for insight bootstrap".to_string());
            return;
        };
        self.queue_hub_command(
            "insight_bootstrap",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "dry_run": dry_run,
                "max_gaps": 12
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    pub(crate) fn request_mind_force_finalize(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for force finalize".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_finalize_session",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator force finalize"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    pub(crate) fn request_mind_t3_requeue(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for t3 requeue".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_t3_requeue",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator requeue"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    pub(crate) fn request_mind_handshake_rebuild(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for handshake rebuild".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_handshake_rebuild",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator rebuild"}
            ),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    pub(crate) fn request_mind_compaction_rebuild(&mut self) {
        let Some(target) = self.mind_target_agent() else {
            self.status_note = Some("no target pane for compaction rebuild".to_string());
            return;
        };
        self.queue_hub_command(
            "mind_compaction_rebuild",
            Some(target.identity_key.clone()),
            serde_json::json!({
                "reason": "operator compaction rebuild"
            }),
            format!("{}::{}", target.label, target.pane_id),
        );
    }

    pub(crate) fn toggle_mind_lane(&mut self) {
        self.mind_lane = self.mind_lane.next();
        self.scroll = 0;
        self.status_note = Some(format!("mind lane: {}", self.mind_lane.label()));
    }

    pub(crate) fn toggle_mind_scope(&mut self) {
        self.mind_show_all_tabs = !self.mind_show_all_tabs;
        self.scroll = 0;
        self.status_note = Some(if self.mind_show_all_tabs {
            "mind scope: all tabs".to_string()
        } else {
            "mind scope: active tab".to_string()
        });
    }

    pub(crate) fn toggle_mind_provenance(&mut self) {
        self.mind_show_provenance = !self.mind_show_provenance;
        self.scroll = 0;
        self.status_note = Some(if self.mind_show_provenance {
            "mind provenance: expanded".to_string()
        } else {
            "mind provenance: compact".to_string()
        });
    }

    pub(crate) fn viewer_tab_overview_index(
        rows: &[OverviewRow],
        tab_index: Option<usize>,
    ) -> Option<usize> {
        let tab_index = tab_index?;
        rows.iter().position(|row| row.tab_index == Some(tab_index))
    }

    pub(crate) fn selected_overview_index_for_rows(&self, rows: &[OverviewRow]) -> usize {
        if rows.is_empty() {
            return 0;
        }
        if self.follow_viewer_tab {
            let viewer_tab = self
                .viewer_tab_index_from_hub_layout()
                .or(self.local.viewer_tab_index);
            if let Some(index) = Self::viewer_tab_overview_index(rows, viewer_tab) {
                return index;
            }
            if let Some(index) = rows.iter().position(|row| row.tab_focused) {
                return index;
            }
        }
        self.selected_overview.min(rows.len().saturating_sub(1))
    }

    pub(crate) fn move_overview_selection(&mut self, step: i32) {
        let rows = self.overview_rows();
        let len = rows.len();
        if len == 0 {
            self.selected_overview = 0;
            return;
        }
        let current = self.selected_overview_index_for_rows(&rows) as i32;
        let max = len.saturating_sub(1) as i32;
        let next = (current + step).clamp(0, max) as usize;
        self.follow_viewer_tab = false;
        self.selected_overview = next;
    }

    pub(crate) fn focus_tab_target(
        &mut self,
        pane_id: &str,
        tab_index: Option<usize>,
        tab_name: Option<String>,
    ) {
        if self.connected {
            let mut args = serde_json::Map::new();
            if let Some(tab_index) = tab_index {
                args.insert("tab_index".to_string(), Value::from(tab_index as u64));
            }
            if let Some(tab_name) = tab_name.as_ref().filter(|value| !value.trim().is_empty()) {
                args.insert("tab_name".to_string(), Value::String(tab_name.clone()));
            }
            if args.is_empty() {
                self.status_note = Some(format!("no tab id/name for pane {pane_id}"));
                return;
            }
            self.queue_hub_command(
                "focus_tab",
                None,
                Value::Object(args),
                format!("pane {pane_id}"),
            );
            return;
        }

        let Some(tab_index) = tab_index else {
            self.status_note = Some(format!("no tab id/name for pane {pane_id}"));
            return;
        };
        if let Err(err) = go_to_tab(&self.config.session_id, tab_index) {
            self.status_note = Some(format!("focus failed: {err}"));
        } else {
            self.status_note = Some(format!("focused tab {tab_index} for pane {pane_id}"));
        }
    }

    pub(crate) fn current_tab_index(&self) -> Option<usize> {
        self.active_hub_layout()
            .and_then(|layout| {
                layout
                    .pane_tabs
                    .get(&self.config.pane_id)
                    .map(|meta| meta.index)
                    .or(layout.focused_tab_index)
            })
            .or(self.local.viewer_tab_index)
    }

    pub(crate) fn worker_launch_supported(&self) -> bool {
        self.config.project_root.exists()
    }

    pub(crate) fn next_worker_tab_name(&self) -> String {
        format!("Worker {}", self.overseer_workers().len().saturating_add(1))
    }

    pub(crate) fn delegation_tab_name(worker: &WorkerSnapshot) -> String {
        if let Some(task_id) = worker
            .assignment
            .task_id
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            return format!("Delegate {task_id}");
        }
        if let Some(tag) = worker
            .assignment
            .tag
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            return format!("Delegate {tag}");
        }
        if let Some(role) = worker
            .role
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            return format!("Delegate {role}");
        }
        "Delegated Worker".to_string()
    }

    pub(crate) fn render_delegation_brief(&self, worker: &WorkerSnapshot) -> String {
        let summary = worker
            .summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("No bounded worker summary available.");
        let task = worker.assignment.task_id.as_deref().unwrap_or("unassigned");
        let tag = worker.assignment.tag.as_deref().unwrap_or("unscoped");
        let role = worker.role.as_deref().unwrap_or("worker");
        let blocker = worker.blocker.as_deref().unwrap_or("none reported");
        format!(
            "# Mission Control delegation brief\n\n- session: {}\n- source worker: {}\n- pane: {}\n- role: {}\n- task: {}\n- tag: {}\n- status: {:?}\n- plan alignment: {:?}\n- drift risk: {:?}\n- blocker: {}\n\n## Focus summary\n{}\n\n## Operator guidance\n- Use this as bounded context only; re-observe before making major plan changes.\n- Prefer explicit task/tag alignment and a narrow validation goal.\n- Request peer review or unblock consultation if uncertainty remains.\n",
            worker.session_id,
            worker.agent_id,
            worker.pane_id,
            role,
            task,
            tag,
            worker.status,
            worker.plan_alignment,
            worker.drift_risk,
            blocker,
            summary,
        )
    }

    pub(crate) fn write_delegation_brief(
        &self,
        worker: &WorkerSnapshot,
    ) -> Result<PathBuf, String> {
        let dir = self
            .config
            .state_dir
            .join("mission-control")
            .join("delegations");
        fs::create_dir_all(&dir).map_err(|err| format!("create delegation dir failed: {err}"))?;
        let slug = sanitize_slug(&format!(
            "{}-{}-{}",
            worker.agent_id,
            worker.assignment.task_id.as_deref().unwrap_or("worker"),
            Utc::now().format("%Y%m%d%H%M%S")
        ));
        let path = dir.join(format!("{slug}.md"));
        fs::write(&path, self.render_delegation_brief(worker))
            .map_err(|err| format!("write delegation brief failed: {err}"))?;
        Ok(path)
    }

    pub(crate) fn launch_worker_tab(
        &mut self,
        tab_name: &str,
        brief_path: Option<&Path>,
    ) -> Result<String, String> {
        if !self.worker_launch_supported() {
            return Err("project root unavailable for worker launch".to_string());
        }
        let in_zellij = in_zellij_session();
        let return_tab = self.current_tab_index();
        let agent_id = resolve_launch_agent_id();
        let plan = build_worker_launch_plan(
            &self.config.project_root,
            &agent_id,
            tab_name,
            brief_path,
            in_zellij,
        );
        execute_worker_launch_plan(&plan)?;
        if in_zellij {
            if let Some(tab_index) = return_tab {
                let _ = go_to_tab(&self.config.session_id, tab_index);
            }
        }
        Ok(agent_id)
    }

    pub(crate) fn request_spawn_worker(&mut self) {
        if self.mode != Mode::Overseer {
            self.status_note = Some("switch to Overseer mode to spawn workers".to_string());
            return;
        }
        let tab_name = self.next_worker_tab_name();
        match self.launch_worker_tab(&tab_name, None) {
            Ok(agent_id) => {
                self.status_note = Some(format!("spawned {tab_name} with agent {agent_id}"));
            }
            Err(err) => {
                self.status_note = Some(format!("spawn failed: {err}"));
            }
        }
    }

    pub(crate) fn request_delegate_worker(&mut self) {
        let Some(worker) = self.selected_overseer_worker() else {
            return;
        };
        let tab_name = Self::delegation_tab_name(&worker);
        match self.write_delegation_brief(&worker) {
            Ok(brief_path) => match self.launch_worker_tab(&tab_name, Some(&brief_path)) {
                Ok(agent_id) => {
                    self.status_note = Some(format!(
                        "delegated {} via {tab_name} ({agent_id}); brief: {}",
                        worker.agent_id,
                        brief_path.display()
                    ));
                }
                Err(err) => {
                    self.status_note = Some(format!("delegate failed: {err}"));
                }
            },
            Err(err) => {
                self.status_note = Some(format!("delegate failed: {err}"));
            }
        }
    }

    pub(crate) fn focus_selected_overview_tab(&mut self) {
        match self.mode {
            Mode::Overview => {
                let rows = self.overview_rows();
                if rows.is_empty() {
                    self.status_note = Some("no agents to focus".to_string());
                    return;
                }
                let selected = self.selected_overview_index_for_rows(&rows);
                self.selected_overview = selected;
                let row = &rows[selected];
                self.focus_tab_target(&row.pane_id, row.tab_index, row.tab_name.clone());
            }
            Mode::Overseer => {
                let Some(worker) = self.selected_overseer_worker() else {
                    return;
                };
                let tab_meta = self
                    .active_hub_layout()
                    .and_then(|layout| layout.pane_tabs.get(&worker.pane_id))
                    .cloned()
                    .or_else(|| self.tab_cache.get(&worker.pane_id).cloned());
                self.focus_tab_target(
                    &worker.pane_id,
                    tab_meta.as_ref().map(|meta| meta.index),
                    tab_meta.map(|meta| meta.name),
                );
            }
            _ => {}
        }
    }

    pub(crate) fn stop_selected_overview_agent(&mut self) {
        match self.mode {
            Mode::Overview => {
                let rows = self.overview_rows();
                if rows.is_empty() {
                    self.status_note = Some("no agents to stop".to_string());
                    return;
                }
                let selected = self.selected_overview_index_for_rows(&rows);
                self.selected_overview = selected;
                let row = &rows[selected];
                self.queue_hub_command(
                    "stop_agent",
                    Some(row.identity_key.clone()),
                    serde_json::json!({"reason": "pulse_user_request"}),
                    format!("{}::{}", row.label, row.pane_id),
                );
            }
            Mode::Overseer => {
                let Some(worker) = self.selected_overseer_worker() else {
                    return;
                };
                self.queue_hub_command(
                    "stop_agent",
                    Some(worker.agent_id.clone()),
                    serde_json::json!({"reason": "pulse_user_request"}),
                    format!("{}::{}", worker.agent_id, worker.pane_id),
                );
            }
            _ => {}
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct RuntimeSnapshot {
    pub(crate) session_id: String,
    pub(crate) pane_id: String,
    pub(crate) agent_id: String,
    pub(crate) agent_label: String,
    pub(crate) project_root: String,
    #[serde(default)]
    pub(crate) tab_scope: Option<String>,
    #[serde(default)]
    pub(crate) session_title: Option<String>,
    #[serde(default)]
    pub(crate) chat_title: Option<String>,
    pub(crate) pid: i32,
    pub(crate) status: String,
    pub(crate) last_update: String,
}

#[derive(Clone, Debug)]
pub(crate) struct GitStatusEntry {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) staged: bool,
    pub(crate) unstaged: bool,
    pub(crate) untracked: bool,
}
