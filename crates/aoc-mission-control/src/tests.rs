use super::*;
use aoc_core::insight_contracts::InsightDetachedWorkerKind;
use aoc_mind::{project_scope_key, MindHandshakeEntry};
use crossterm::event::KeyModifiers;
use mind_host_render::render_mind_activity_bridge_lines;

fn test_config() -> Config {
    Config {
        session_id: "session-test".to_string(),
        pane_id: "12".to_string(),
        tab_scope: Some("agent".to_string()),
        pulse_socket_path: PathBuf::from("/tmp/pulse-test.sock"),
        mission_theme: MissionThemeMode::Terminal,
        mission_custom_theme: None,
        pulse_vnext_enabled: true,
        overview_enabled: true,
        start_view: None,
        fleet_plane_filter: FleetPlaneFilter::All,
        layout_source: LayoutSource::Hub,
        client_id: "pulse-test".to_string(),
        project_root: PathBuf::from("/tmp"),
        mind_project_scoped: false,
        state_dir: PathBuf::from("/tmp"),
    }
}

fn empty_local() -> LocalSnapshot {
    LocalSnapshot {
        overview: Vec::new(),
        viewer_tab_index: None,
        tab_roster: Vec::new(),
        work: Vec::new(),
        diff: Vec::new(),
        health: HealthSnapshot {
            dependencies: Vec::new(),
            checks: Vec::new(),
            taskmaster_status: "unknown".to_string(),
        },
    }
}

fn fresh_test_mind_store(label: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "{label}-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&root).expect("create test root");
    let store_path = mind_store_path(&root);
    if let Some(parent) = store_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
        std::fs::create_dir_all(parent).expect("create mind dir");
    }
    (root, store_path)
}

fn cleanup_test_mind_store(root: &Path, store_path: &Path) {
    if let Some(parent) = store_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
    let _ = std::fs::remove_dir_all(root);
}

fn hub_state(agent_id: &str, pane_id: &str, project_root: &str) -> AgentState {
    AgentState {
        agent_id: agent_id.to_string(),
        session_id: "session-test".to_string(),
        pane_id: pane_id.to_string(),
        lifecycle: "running".to_string(),
        snippet: Some("working".to_string()),
        last_heartbeat_ms: Some(1),
        last_activity_ms: Some(1),
        updated_at_ms: Some(1),
        source: Some(serde_json::json!({
            "agent_status": {
                "agent_label": "OpenCode",
                "project_root": project_root,
                "pane_id": pane_id,
                "status": "running"
            }
        })),
    }
}

#[test]
fn mind_search_keys_edit_and_browse_results() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.mode = Mode::Mind;
    let mut refresh_requested = false;

    handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        &mut app,
        &mut refresh_requested,
    );
    assert!(app.mind_search_editing);

    handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        &mut app,
        &mut refresh_requested,
    );
    handle_key(
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
        &mut app,
        &mut refresh_requested,
    );
    handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut app,
        &mut refresh_requested,
    );
    assert!(!app.mind_search_editing);
    assert_eq!(app.mind_search_query, "pl");
    assert_eq!(app.mind_search_selected, 0);

    handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &mut app,
        &mut refresh_requested,
    );
    assert_eq!(app.mind_search_selected, 1);

    handle_key(
        KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE),
        &mut app,
        &mut refresh_requested,
    );
    assert_eq!(app.mind_search_selected, 0);
}

#[test]
fn status_payload_prefers_source_metadata() {
    let state = AgentState {
        agent_id: "session-test::12".to_string(),
        session_id: "session-test".to_string(),
        pane_id: "12".to_string(),
        lifecycle: "needs_input".to_string(),
        snippet: Some("awaiting input".to_string()),
        last_heartbeat_ms: Some(1),
        last_activity_ms: Some(1),
        updated_at_ms: Some(1),
        source: Some(serde_json::json!({
            "agent_label": "OpenCode",
            "project_root": "/repo"
        })),
    };

    let payload = status_payload_from_state(&state);
    assert_eq!(payload.status, "needs-input");
    assert_eq!(payload.project_root, "/repo");
    assert_eq!(payload.agent_label.as_deref(), Some("OpenCode"));
    assert_eq!(payload.message.as_deref(), Some("awaiting input"));
    assert_eq!(payload.tab_scope, None);
}

#[test]
fn status_payload_extracts_tab_scope_from_source() {
    let state = AgentState {
        agent_id: "session-test::12".to_string(),
        session_id: "session-test".to_string(),
        pane_id: "12".to_string(),
        lifecycle: "running".to_string(),
        snippet: None,
        last_heartbeat_ms: Some(1),
        last_activity_ms: Some(1),
        updated_at_ms: Some(1),
        source: Some(serde_json::json!({
            "agent_status": {
                "project_root": "/repo",
                "tab_scope": "Agent"
            }
        })),
    };

    let payload = status_payload_from_state(&state);
    assert_eq!(payload.tab_scope.as_deref(), Some("Agent"));
}

#[test]
fn tab_scope_matches_ignores_case_and_whitespace() {
    assert!(tab_scope_matches(Some(" Agent  "), Some("agent")));
    assert!(!tab_scope_matches(Some("agent"), Some("review")));
}

#[test]
fn command_result_clears_pending_on_terminal_status() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.pending_commands.insert(
        "req-1".to_string(),
        PendingCommand {
            command: "stop_agent".to_string(),
            target: "pane-12".to_string(),
        },
    );

    app.apply_hub_event(HubEvent::CommandResult {
        payload: CommandResultPayload {
            command: "stop_agent".to_string(),
            status: "ok".to_string(),
            message: Some("terminated".to_string()),
            error: None,
        },
        request_id: Some("req-1".to_string()),
    });

    assert!(app.pending_commands.is_empty());
    let note = app.status_note.unwrap_or_default();
    assert!(note.contains("stop_agent"));
    assert!(note.contains("terminated"));
}

#[test]
fn command_result_keeps_pending_on_accepted_status() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.pending_commands.insert(
        "req-2".to_string(),
        PendingCommand {
            command: "run_validation".to_string(),
            target: "pane-7".to_string(),
        },
    );

    app.apply_hub_event(HubEvent::CommandResult {
        payload: CommandResultPayload {
            command: "run_validation".to_string(),
            status: "accepted".to_string(),
            message: Some("queued".to_string()),
            error: None,
        },
        request_id: Some("req-2".to_string()),
    });

    assert!(app.pending_commands.contains_key("req-2"));
    assert!(app.status_note.unwrap_or_default().contains("queued"));
}

#[test]
fn command_result_ignores_stale_request_ids() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.status_note = Some("unchanged".to_string());

    app.apply_hub_event(HubEvent::CommandResult {
        payload: CommandResultPayload {
            command: "pause_and_summarize".to_string(),
            status: "ok".to_string(),
            message: Some("late duplicate".to_string()),
            error: None,
        },
        request_id: Some("req-stale".to_string()),
    });

    assert_eq!(app.status_note.as_deref(), Some("unchanged"));
}

#[test]
fn overseer_consultation_queues_review_request_for_peer_worker() {
    let (tx, mut rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Overseer;
    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    pane_id: "12".to_string(),
                    role: Some("builder".to_string()),
                    status: WorkerStatus::Active,
                    summary: Some("implementing transport".to_string()),
                    assignment: aoc_core::session_overseer::WorkerAssignment {
                        task_id: Some("160".to_string()),
                        tag: Some("mind".to_string()),
                        epic_id: None,
                    },
                    plan_alignment: PlanAlignment::Medium,
                    ..Default::default()
                },
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::24".to_string(),
                    pane_id: "24".to_string(),
                    role: Some("reviewer".to_string()),
                    status: WorkerStatus::Active,
                    summary: Some("available for review".to_string()),
                    assignment: aoc_core::session_overseer::WorkerAssignment {
                        task_id: Some("161".to_string()),
                        tag: Some("mind".to_string()),
                        epic_id: None,
                    },
                    plan_alignment: PlanAlignment::High,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    });

    app.request_overseer_consultation(ConsultationPacketKind::Review);

    let outbound = rx.try_recv().expect("consultation outbound queued");
    let WireMsg::ConsultationRequest(payload) = outbound.msg else {
        panic!("expected consultation request")
    };
    assert_eq!(payload.requesting_agent_id, "session-test::12");
    assert_eq!(payload.target_agent_id, "session-test::24");
    assert_eq!(payload.packet.kind, ConsultationPacketKind::Review);
    assert_eq!(
        app.pending_consultations
            .get(&outbound.request_id)
            .map(|value| value.responder.as_str()),
        Some("session-test::24")
    );
}

#[test]
fn consultation_response_clears_pending_on_terminal_status() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.pending_consultations.insert(
        "req-consult".to_string(),
        PendingConsultation {
            kind: ConsultationPacketKind::Review,
            requester: "session-test::12".to_string(),
            responder: "session-test::24".to_string(),
            request_packet: ConsultationPacket {
                packet_id: "packet-request".to_string(),
                kind: ConsultationPacketKind::Review,
                identity: ConsultationIdentity {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    conversation_id: Some("conv-req".to_string()),
                    ..Default::default()
                },
                task_context: ConsultationTaskContext {
                    active_tag: Some("mind".to_string()),
                    task_ids: vec!["165".to_string()],
                    focus_summary: Some("persist consultation outcomes".to_string()),
                },
                summary: Some("request review".to_string()),
                ..Default::default()
            },
        },
    );

    app.apply_hub_event(HubEvent::ConsultationResponse {
        payload: ConsultationResponsePayload {
            consultation_id: "consult-1".to_string(),
            requesting_agent_id: "session-test::12".to_string(),
            responding_agent_id: "session-test::24".to_string(),
            status: ConsultationStatus::Completed,
            packet: None,
            message: Some("review completed".to_string()),
            error: None,
        },
        request_id: Some("req-consult".to_string()),
    });

    assert!(!app.pending_consultations.contains_key("req-consult"));
    assert!(app
        .status_note
        .unwrap_or_default()
        .contains("review completed"));
}

#[test]
fn consultation_response_persists_outcome_into_mind_store() {
    let (test_root, store_path) = fresh_test_mind_store("aoc-mc-consult");

    let mut config = test_config();
    config.project_root = test_root.clone();
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(config, tx, empty_local());
    app.pending_consultations.insert(
        "req-consult".to_string(),
        PendingConsultation {
            kind: ConsultationPacketKind::Review,
            requester: "session-test::12".to_string(),
            responder: "session-test::24".to_string(),
            request_packet: ConsultationPacket {
                packet_id: "packet-request".to_string(),
                kind: ConsultationPacketKind::Review,
                identity: ConsultationIdentity {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    conversation_id: Some("conv-req".to_string()),
                    ..Default::default()
                },
                task_context: ConsultationTaskContext {
                    active_tag: Some("mind".to_string()),
                    task_ids: vec!["165".to_string()],
                    focus_summary: Some("persist consultation outcomes".to_string()),
                },
                summary: Some("request peer review".to_string()),
                evidence_refs: vec![aoc_core::consultation_contracts::ConsultationEvidenceRef {
                    reference: "file:docs/mission-control.md".to_string(),
                    label: Some("mission control docs".to_string()),
                    path: Some("docs/mission-control.md".to_string()),
                    relation: Some("reads".to_string()),
                }],
                ..Default::default()
            },
        },
    );

    app.apply_hub_event(HubEvent::ConsultationResponse {
        payload: ConsultationResponsePayload {
            consultation_id: "consult-1".to_string(),
            requesting_agent_id: "session-test::12".to_string(),
            responding_agent_id: "session-test::24".to_string(),
            status: ConsultationStatus::Completed,
            packet: Some(ConsultationPacket {
                packet_id: "packet-response".to_string(),
                kind: ConsultationPacketKind::Review,
                identity: ConsultationIdentity {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::24".to_string(),
                    conversation_id: Some("conv-resp".to_string()),
                    ..Default::default()
                },
                summary: Some("peer review found one follow-up".to_string()),
                current_plan: vec![aoc_core::consultation_contracts::ConsultationPlanItem {
                    title: "tighten persistence coverage".to_string(),
                    ..Default::default()
                }],
                confidence: ConsultationConfidence {
                    overall_bps: Some(8700),
                    rationale: Some("bounded evidence refs and live worker state".to_string()),
                },
                freshness: ConsultationFreshness {
                    packet_generated_at: Some("2026-03-12T18:00:00Z".to_string()),
                    ..Default::default()
                },
                evidence_refs: vec![aoc_core::consultation_contracts::ConsultationEvidenceRef {
                    reference: "file:crates/aoc-mission-control/src/main.rs".to_string(),
                    label: Some("mission control source".to_string()),
                    path: Some("crates/aoc-mission-control/src/main.rs".to_string()),
                    relation: Some("modified".to_string()),
                }],
                ..Default::default()
            }),
            message: Some("review completed".to_string()),
            error: None,
        },
        request_id: Some("req-consult".to_string()),
    });

    let store = aoc_storage::MindStore::open(&store_path).expect("open store");
    let artifact = store
        .artifact_by_id("consult:consult-1")
        .expect("artifact lookup")
        .expect("consultation artifact persisted");
    assert_eq!(artifact.kind, "t2");
    assert_eq!(artifact.conversation_id, "conv-req");
    assert!(artifact.text.contains("# Consultation outcome"));
    assert!(artifact.text.contains("peer review found one follow-up"));
    assert!(artifact
        .trace_ids
        .iter()
        .any(|value| value == "consultation:consult-1"));
    assert_eq!(
        store
            .artifact_task_links_for_artifact("consult:consult-1")
            .expect("task links")
            .len(),
        1
    );
    assert_eq!(
        store
            .artifact_file_links("consult:consult-1")
            .expect("file links")
            .len(),
        2
    );
    assert_eq!(
        store
            .semantic_provenance_for_artifact("consult:consult-1")
            .expect("semantic provenance")
            .len(),
        1
    );

    drop(store);
    cleanup_test_mind_store(&test_root, &store_path);
}

#[test]
fn orchestrator_tool_surface_marks_spawn_and_delegate_ready() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Overseer;
    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    pane_id: "12".to_string(),
                    status: WorkerStatus::Active,
                    plan_alignment: PlanAlignment::Medium,
                    ..Default::default()
                },
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::24".to_string(),
                    pane_id: "24".to_string(),
                    status: WorkerStatus::Active,
                    plan_alignment: PlanAlignment::High,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    });

    let tools = app.orchestrator_tools();
    assert!(tools
        .iter()
        .any(|tool| tool.id == OrchestratorToolId::WorkerReview
            && tool.status == OrchestratorToolStatus::Ready));
    assert!(tools
        .iter()
        .any(|tool| tool.id == OrchestratorToolId::WorkerSpawn
            && tool.status == OrchestratorToolStatus::Ready
            && tool.shortcut == Some("s")));
    assert!(tools
        .iter()
        .any(|tool| tool.id == OrchestratorToolId::WorkerDelegate
            && tool.status == OrchestratorToolStatus::Ready
            && tool.shortcut == Some("d")));
}

#[test]
fn build_worker_launch_plan_uses_new_tab_in_zellij_and_launch_otherwise() {
    let project_root = PathBuf::from("/tmp/project-root");
    let brief_path = PathBuf::from("/tmp/delegation.md");

    let zellij_plan =
        build_worker_launch_plan(&project_root, "pi", "Worker 3", Some(&brief_path), true);
    assert_eq!(zellij_plan.program, "aoc-new-tab");
    assert_eq!(
        zellij_plan.args,
        vec![
            "--aoc".to_string(),
            "--name".to_string(),
            "Worker 3".to_string(),
            "--cwd".to_string(),
            "/tmp/project-root".to_string(),
        ]
    );
    assert!(zellij_plan
        .env
        .contains(&("AOC_LAUNCH_AGENT_ID".to_string(), "pi".to_string())));
    assert!(zellij_plan.env.contains(&(
        "AOC_DELEGATION_BRIEF_PATH".to_string(),
        "/tmp/delegation.md".to_string()
    )));

    let standalone_plan = build_worker_launch_plan(&project_root, "pi", "Worker 3", None, false);
    assert_eq!(standalone_plan.program, "aoc-launch");
    assert!(standalone_plan.args.is_empty());
}

#[test]
fn delegation_brief_captures_selected_worker_context() {
    let (tx, _rx) = mpsc::channel(4);
    let app = App::new(test_config(), tx, empty_local());
    let worker = WorkerSnapshot {
        session_id: "session-test".to_string(),
        agent_id: "session-test::24".to_string(),
        pane_id: "24".to_string(),
        role: Some("reviewer".to_string()),
        status: WorkerStatus::Blocked,
        summary: Some("waiting on fixture regeneration".to_string()),
        blocker: Some("need fresh snapshot".to_string()),
        assignment: aoc_core::session_overseer::WorkerAssignment {
            task_id: Some("164".to_string()),
            tag: Some("mind".to_string()),
            epic_id: None,
        },
        plan_alignment: PlanAlignment::Medium,
        drift_risk: DriftRisk::Medium,
        ..Default::default()
    };

    let brief = app.render_delegation_brief(&worker);
    assert!(brief.contains("Mission Control delegation brief"));
    assert!(brief.contains("source worker: session-test::24"));
    assert!(brief.contains("task: 164"));
    assert!(brief.contains("need fresh snapshot"));
    assert!(brief.contains("waiting on fixture regeneration"));
}

#[test]
fn orchestration_graph_ir_compiles_reviewable_delegate_path() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Overseer;
    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    pane_id: "12".to_string(),
                    role: Some("implementer".to_string()),
                    status: WorkerStatus::Active,
                    assignment: aoc_core::session_overseer::WorkerAssignment {
                        task_id: Some("166".to_string()),
                        tag: Some("mind".to_string()),
                        epic_id: None,
                    },
                    ..Default::default()
                },
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::24".to_string(),
                    pane_id: "24".to_string(),
                    status: WorkerStatus::Active,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    });

    let graph = app.orchestration_graph_ir();
    assert_eq!(graph.session_id, "session-test");
    assert!(graph
        .nodes
        .iter()
        .any(|node| node.kind == OrchestrationGraphNodeKind::Session));
    assert!(graph
        .nodes
        .iter()
        .any(|node| node.kind == OrchestrationGraphNodeKind::Artifact
            && node.label == "delegation brief"));
    assert!(graph
        .edges
        .iter()
        .any(|edge| edge.kind == OrchestrationGraphEdgeKind::Writes
            && edge.summary.contains("delegation brief")));
    assert!(graph.compile_paths.iter().any(|path| {
        path.entry_tool == OrchestratorToolId::WorkerDelegate
            && path
                .steps
                .iter()
                .any(|step| step.contains("write delegation brief"))
            && path
                .steps
                .iter()
                .any(|step| step.contains("AOC_DELEGATION_BRIEF_PATH"))
    }));
}

#[test]
fn overseer_render_includes_orchestrator_tool_surface() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Overseer;
    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::12".to_string(),
                    pane_id: "12".to_string(),
                    status: WorkerStatus::Active,
                    plan_alignment: PlanAlignment::Medium,
                    ..Default::default()
                },
                WorkerSnapshot {
                    session_id: "session-test".to_string(),
                    agent_id: "session-test::24".to_string(),
                    pane_id: "24".to_string(),
                    status: WorkerStatus::Active,
                    plan_alignment: PlanAlignment::High,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    });

    let rendered = render_overseer_lines(&app, mission_theme(MissionThemeMode::Terminal), false)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Mission Control tools"));
    assert!(rendered.contains("peer review"));
    assert!(rendered.contains("spawn worker"));
    assert!(rendered.contains("Reviewable compile"));
    assert!(rendered.contains("graph "));
    assert!(rendered.contains("plan [ready] delegate task"));
}

#[test]
fn overseer_snapshot_ignores_other_sessions() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());

    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "other-session".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![WorkerSnapshot {
                session_id: "other-session".to_string(),
                agent_id: "other-session::1".to_string(),
                pane_id: "1".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        },
    });

    assert!(app.overseer_snapshot().is_none());
    assert!(app.overseer_workers().is_empty());
}

#[test]
fn overseer_timeline_ignores_other_sessions() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());

    app.apply_hub_event(HubEvent::ObserverTimeline {
        payload: ObserverTimelinePayload {
            session_id: "other-session".to_string(),
            generated_at_ms: Some(1_700_000_000_123),
            entries: vec![ObserverTimelineEntry {
                event_id: "evt-9".to_string(),
                session_id: "other-session".to_string(),
                agent_id: "other-session::1".to_string(),
                ..Default::default()
            }],
        },
    });

    assert!(app.overseer_timeline().is_empty());
}

#[test]
fn overview_sort_prioritizes_tab_position() {
    let rows = vec![
        OverviewRow {
            identity_key: "session-test::2".to_string(),
            label: "needs-input-pane".to_string(),
            lifecycle: "needs-input".to_string(),
            snippet: None,
            pane_id: "2".to_string(),
            tab_index: Some(2),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "hub".to_string(),
            session_title: None,
            chat_title: None,
        },
        OverviewRow {
            identity_key: "session-test::1".to_string(),
            label: "idle-pane".to_string(),
            lifecycle: "idle".to_string(),
            snippet: None,
            pane_id: "1".to_string(),
            tab_index: Some(1),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "hub".to_string(),
            session_title: None,
            chat_title: None,
        },
    ];

    let sorted = sort_overview_rows(rows);
    assert_eq!(sorted[0].identity_key, "session-test::1");
    assert_eq!(sorted[1].identity_key, "session-test::2");
}

#[test]
fn overview_sort_uses_numeric_pane_id_within_tab() {
    let rows = vec![
        OverviewRow {
            identity_key: "session-test::10".to_string(),
            label: "pane-10".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "10".to_string(),
            tab_index: Some(1),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "hub".to_string(),
            session_title: None,
            chat_title: None,
        },
        OverviewRow {
            identity_key: "session-test::2".to_string(),
            label: "pane-2".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "2".to_string(),
            tab_index: Some(1),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "hub".to_string(),
            session_title: None,
            chat_title: None,
        },
    ];

    let sorted = sort_overview_rows(rows);
    assert_eq!(sorted[0].pane_id, "2");
    assert_eq!(sorted[1].pane_id, "10");
}

#[test]
fn overview_attention_sort_prioritizes_blockers_over_layout_order() {
    let rows = vec![
        OverviewRow {
            identity_key: "session-test::1".to_string(),
            label: "pane-1".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "1".to_string(),
            tab_index: Some(1),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "hub".to_string(),
            session_title: None,
            chat_title: None,
        },
        OverviewRow {
            identity_key: "session-test::2".to_string(),
            label: "pane-2".to_string(),
            lifecycle: "needs-input".to_string(),
            snippet: None,
            pane_id: "2".to_string(),
            tab_index: Some(2),
            tab_name: Some("Agent".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "hub".to_string(),
            session_title: None,
            chat_title: None,
        },
    ];

    let sorted = sort_overview_rows_attention(rows);
    assert_eq!(sorted[0].identity_key, "session-test::2");
    assert_eq!(sorted[1].identity_key, "session-test::1");
}

#[test]
fn overview_selection_follows_viewer_tab_by_default() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    let rows = vec![
        OverviewRow {
            identity_key: "session-test::1".to_string(),
            label: "pane-1".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "1".to_string(),
            tab_index: Some(1),
            tab_name: Some("tab-1".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
            session_title: None,
            chat_title: None,
        },
        OverviewRow {
            identity_key: "session-test::2".to_string(),
            label: "pane-2".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "2".to_string(),
            tab_index: Some(2),
            tab_name: Some("tab-2".to_string()),
            tab_focused: false,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
            session_title: None,
            chat_title: None,
        },
    ];

    app.local.viewer_tab_index = Some(2);
    assert_eq!(app.selected_overview_index_for_rows(&rows), 1);

    app.follow_viewer_tab = false;
    app.selected_overview = 0;
    assert_eq!(app.selected_overview_index_for_rows(&rows), 0);
}

#[test]
fn source_confidence_supports_top_level_and_nested_fields() {
    let top = serde_json::json!({"parser_confidence": 3});
    assert_eq!(source_confidence(&Some(top)), Some(3));

    let nested = serde_json::json!({"agent_status": {"lifecycle_confidence": 2}});
    assert_eq!(source_confidence(&Some(nested)), Some(2));
}

#[test]
fn parse_task_summaries_supports_tag_map_counts_shape() {
    let value = serde_json::json!({
        "master": {
            "total": 5,
            "pending": 2,
            "in_progress": 1,
            "blocked": 1,
            "done": 1
        }
    });
    let parsed = parse_task_summaries_from_source(&value, "session-test::12").unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].agent_id, "session-test::12");
    assert_eq!(parsed[0].tag, "master");
    assert_eq!(parsed[0].counts.total, 5);
    assert_eq!(parsed[0].counts.in_progress, 1);
    assert_eq!(parsed[0].counts.blocked, 1);
}

#[test]
fn hub_state_upsert_wires_task_diff_and_health_from_source() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);

    let state = AgentState {
        agent_id: "session-test::12".to_string(),
        session_id: "session-test".to_string(),
        pane_id: "12".to_string(),
        lifecycle: "running".to_string(),
        snippet: None,
        last_heartbeat_ms: Some(1),
        last_activity_ms: Some(1),
        updated_at_ms: Some(1),
        source: Some(serde_json::json!({
            "agent_status": {
                "agent_label": "OpenCode",
                "project_root": "/repo"
            },
            "task_summaries": {
                "master": {
                    "total": 3,
                    "pending": 1,
                    "in_progress": 1,
                    "blocked": 0,
                    "done": 1
                }
            },
            "diff_summary": {
                "repo_root": "/repo",
                "git_available": true,
                "summary": {
                    "staged": {"files": 1, "additions": 2, "deletions": 0},
                    "unstaged": {"files": 1, "additions": 1, "deletions": 1},
                    "untracked": {"files": 0}
                },
                "files": []
            },
            "health": {
                "taskmaster_status": "available",
                "dependencies": [
                    {"name": "git", "available": true, "path": "/usr/bin/git"}
                ],
                "checks": [
                    {"name": "test", "status": "ok", "timestamp": "now", "details": "pass"}
                ]
            },
            "mind_observer": {
                "events": [
                    {
                        "status": "fallback",
                        "trigger": "task_completed",
                        "runtime": "deterministic",
                        "attempt_count": 2,
                        "latency_ms": 95,
                        "reason": "semantic observer failed (timeout)",
                        "failure_kind": "timeout",
                        "conversation_id": "conv-1",
                        "completed_at": "2026-02-25T16:30:00Z"
                    }
                ]
            },
            "insight_runtime": {
                "reflector_enabled": true,
                "reflector_jobs_completed": 2,
                "reflector_jobs_failed": 1,
                "reflector_lock_conflicts": 3,
                "t3_enabled": true,
                "t3_jobs_completed": 4,
                "t3_jobs_failed": 1,
                "t3_jobs_requeued": 2,
                "t3_jobs_dead_lettered": 1,
                "t3_lock_conflicts": 2,
                "t3_queue_depth": 6,
                "supervisor_runs": 4,
                "supervisor_failures": 1,
                "queue_depth": 5
            }
        })),
    };

    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![state],
        },
        event_at: Utc::now(),
    });

    assert_eq!(app.hub.tasks.len(), 1);
    let task = app
        .hub
        .tasks
        .get("session-test::12::master")
        .expect("task payload should exist");
    assert_eq!(task.counts.total, 3);
    assert_eq!(task.counts.in_progress, 1);

    assert_eq!(app.hub.diffs.len(), 1);
    let diff = app
        .hub
        .diffs
        .get("session-test::12")
        .expect("diff payload should exist");
    assert_eq!(diff.repo_root, "/repo");
    assert!(diff.git_available);

    assert_eq!(app.hub.health.len(), 1);
    let health = app
        .hub
        .health
        .get("session-test::12")
        .expect("health payload should exist");
    assert_eq!(health.taskmaster_status, "available");

    assert_eq!(app.hub.mind.len(), 1);
    let mind = app
        .hub
        .mind
        .get("session-test::12")
        .expect("mind observer payload should exist");
    assert_eq!(mind.events.len(), 1);
    assert_eq!(mind.events[0].status, MindObserverFeedStatus::Fallback);

    let insight = app
        .hub
        .insight_runtime
        .get("session-test::12")
        .expect("insight runtime payload should exist");
    assert_eq!(insight.queue_depth, 5);
    assert_eq!(insight.reflector_jobs_completed, 2);
    assert_eq!(insight.t3_queue_depth, 6);
    assert_eq!(insight.t3_jobs_completed, 4);

    app.mode = Mode::Health;
    assert_eq!(app.mode_source(), "hub");
    assert_eq!(app.health_rows().len(), 1);
}

#[test]
fn mind_rows_filter_to_active_tab_scope_and_render_fallback_badge() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);
    app.mode = Mode::Mind;

    let fallback_event = serde_json::json!({
        "status": "fallback",
        "trigger": "task_completed",
        "runtime": "deterministic",
        "attempt_count": 2,
        "latency_ms": 220,
        "reason": "semantic observer failed (timeout)",
        "completed_at": "2026-02-26T06:45:00Z",
        "progress": {
            "t0_estimated_tokens": 7612,
            "t1_target_tokens": 28000,
            "t1_hard_cap_tokens": 32000,
            "tokens_until_next_run": 20388
        }
    });
    let success_event = serde_json::json!({
        "status": "success",
        "trigger": "token_threshold",
        "runtime": "pi-semantic",
        "attempt_count": 1,
        "latency_ms": 80,
        "completed_at": "2026-02-26T06:44:00Z"
    });

    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![
                AgentState {
                    agent_id: "session-test::12".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "OpenCode",
                            "project_root": "/repo",
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [fallback_event]
                        }
                    })),
                },
                AgentState {
                    agent_id: "session-test::99".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "99".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "Other",
                            "project_root": "/repo",
                            "tab_scope": "review"
                        },
                        "mind_observer": {
                            "events": [success_event]
                        }
                    })),
                },
            ],
        },
        event_at: Utc::now(),
    });

    let rows = app.mind_rows();
    assert_eq!(rows.len(), 1, "non-active tab rows should be filtered out");
    assert_eq!(rows[0].pane_id, "12");
    assert_eq!(rows[0].event.status, MindObserverFeedStatus::Fallback);

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("[t1]"));
    assert!(rendered.contains("[fallback]"));
    assert!(rendered.contains("[task]"));
    assert!(rendered.contains("runtime:det"));
    assert!(rendered.contains("t0:7612/28000"));
}

#[test]
fn manual_observer_shortcut_queues_run_observer_command() {
    let (tx, mut rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.set_local(LocalSnapshot {
        overview: vec![OverviewRow {
            identity_key: "session-test::12".to_string(),
            label: "OpenCode".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "12".to_string(),
            tab_index: Some(1),
            tab_name: Some("Agent".to_string()),
            tab_focused: true,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
            session_title: None,
            chat_title: None,
        }],
        viewer_tab_index: Some(1),
        tab_roster: vec![TabMeta {
            index: 1,
            name: "Agent".to_string(),
            focused: true,
        }],
        work: Vec::new(),
        diff: Vec::new(),
        health: empty_local().health,
    });

    app.request_manual_observer_run();
    let command = rx.try_recv().expect("manual command should be queued");
    let WireMsg::Command(payload) = command.msg else {
        panic!("expected command")
    };
    assert_eq!(payload.command, "run_observer");
    assert_eq!(payload.target_agent_id.as_deref(), Some("session-test::12"));
    assert_eq!(
        payload
            .args
            .get("trigger")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        "manual_shortcut"
    );
}

#[test]
fn mind_shortcuts_queue_insight_and_operator_commands() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;
    app.set_local(LocalSnapshot {
        overview: vec![OverviewRow {
            identity_key: "session-test::12".to_string(),
            label: "OpenCode".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "12".to_string(),
            tab_index: Some(1),
            tab_name: Some("Agent".to_string()),
            tab_focused: true,
            project_root: "/repo".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
            session_title: None,
            chat_title: None,
        }],
        viewer_tab_index: Some(1),
        tab_roster: vec![TabMeta {
            index: 1,
            name: "Agent".to_string(),
            focused: true,
        }],
        work: Vec::new(),
        diff: Vec::new(),
        health: empty_local().health,
    });

    app.request_insight_dispatch_chain();
    app.request_insight_bootstrap(true);
    app.request_mind_force_finalize();
    app.request_mind_compaction_rebuild();
    app.request_mind_t3_requeue();
    app.request_mind_handshake_rebuild();

    let first = rx.try_recv().expect("dispatch command");
    let WireMsg::Command(first_payload) = first.msg else {
        panic!("expected command")
    };
    assert_eq!(first_payload.command, "insight_dispatch");
    assert_eq!(
        first_payload.target_agent_id.as_deref(),
        Some("session-test::12")
    );
    assert_eq!(
        first_payload
            .args
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        "chain"
    );

    let second = rx.try_recv().expect("bootstrap command");
    let WireMsg::Command(second_payload) = second.msg else {
        panic!("expected command")
    };
    assert_eq!(second_payload.command, "insight_bootstrap");
    assert_eq!(
        second_payload
            .args
            .get("dry_run")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        true
    );

    let third = rx.try_recv().expect("force finalize command");
    let WireMsg::Command(third_payload) = third.msg else {
        panic!("expected command")
    };
    assert_eq!(third_payload.command, "mind_finalize_session");

    let fourth = rx.try_recv().expect("compaction rebuild command");
    let WireMsg::Command(fourth_payload) = fourth.msg else {
        panic!("expected command")
    };
    assert_eq!(fourth_payload.command, "mind_compaction_rebuild");

    let fifth = rx.try_recv().expect("t3 requeue command");
    let WireMsg::Command(fifth_payload) = fifth.msg else {
        panic!("expected command")
    };
    assert_eq!(fifth_payload.command, "mind_t3_requeue");

    let sixth = rx.try_recv().expect("handshake rebuild command");
    let WireMsg::Command(sixth_payload) = sixth.msg else {
        panic!("expected command")
    };
    assert_eq!(sixth_payload.command, "mind_handshake_rebuild");
}

#[test]
fn mind_lane_toggle_cycles_t0_t1_t2_t3_and_all() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;

    app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
                    agent_id: "session-test::12".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "OpenCode",
                            "project_root": "/repo",
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {
                                    "status":"queued",
                                    "trigger":"token_threshold",
                                    "progress": {
                                        "t0_estimated_tokens": 1200,
                                        "t1_target_tokens": 28000,
                                        "t1_hard_cap_tokens": 32000,
                                        "tokens_until_next_run": 26800
                                    }
                                },
                                {"status":"success","trigger":"token_threshold","runtime":"pi-semantic"},
                                {"status":"success","trigger":"task_completed","runtime":"t2_reflector","reason":"t2 reflector processed 1 job(s)"},
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

    assert_eq!(app.mind_lane, MindLaneFilter::T1);
    assert_eq!(app.mind_rows().len(), 1);

    app.toggle_mind_lane();
    assert_eq!(app.mind_lane, MindLaneFilter::T2);
    assert_eq!(app.mind_rows().len(), 1);

    app.toggle_mind_lane();
    assert_eq!(app.mind_lane, MindLaneFilter::T3);
    assert_eq!(app.mind_rows().len(), 1);

    app.toggle_mind_lane();
    assert_eq!(app.mind_lane, MindLaneFilter::All);
    assert_eq!(app.mind_rows().len(), 4);

    app.toggle_mind_lane();
    assert_eq!(app.mind_lane, MindLaneFilter::T0);
    assert_eq!(app.mind_rows().len(), 1);
}

#[test]
fn render_mind_lines_shows_t3_runtime_rollup() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;
    app.mind_lane = MindLaneFilter::All;

    app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
                    agent_id: "session-test::12".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "OpenCode",
                            "project_root": "/repo",
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "insight_runtime": {
                            "queue_depth": 2,
                            "reflector_jobs_completed": 1,
                            "reflector_jobs_failed": 0,
                            "reflector_lock_conflicts": 0,
                            "t3_queue_depth": 7,
                            "t3_jobs_completed": 4,
                            "t3_jobs_failed": 1,
                            "t3_jobs_requeued": 2,
                            "t3_jobs_dead_lettered": 1,
                            "t3_lock_conflicts": 3
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("t3q:7 done:4 fail:1 rq:2 dlq:1 lock:3"));
    assert!(rendered.contains("[t3]"));
}

#[test]
fn render_mind_lines_shows_detached_subagent_rollup() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;
    app.mind_lane = MindLaneFilter::All;

    app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
                    agent_id: "session-test::12".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "OpenCode",
                            "project_root": "/repo",
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "insight_detached": {
                            "status": "ok",
                            "active_jobs": 1,
                            "fallback_used": false,
                            "jobs": [
                                {
                                    "job_id": "detached-123",
                                    "mode": "dispatch",
                                    "status": "running",
                                    "agent": "reviewer-contracts",
                                    "created_at_ms": 1000,
                                    "started_at_ms": 1500,
                                    "step_count": 1,
                                    "output_excerpt": "reviewing canonical store-first cutover"
                                }
                            ]
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("subagents:"));
    assert!(rendered.contains("reviewer-contracts"));
    assert!(rendered.contains("run:1"));
}

#[test]
fn render_mind_lines_project_scoped_filters_other_projects() {
    let (tx, _rx) = mpsc::channel(4);
    let mut config = test_config();
    config.project_root = PathBuf::from("/repo-a");
    config.mind_project_scoped = true;
    let mut app = App::new(config, tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;
    app.mind_lane = MindLaneFilter::All;

    app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![
                    AgentState {
                        agent_id: "session-test::12".to_string(),
                        session_id: "session-test".to_string(),
                        pane_id: "12".to_string(),
                        lifecycle: "running".to_string(),
                        snippet: None,
                        last_heartbeat_ms: Some(1),
                        last_activity_ms: Some(1),
                        updated_at_ms: Some(1),
                        source: Some(serde_json::json!({
                            "agent_status": {
                                "agent_label": "Repo A",
                                "project_root": "/repo-a",
                                "tab_scope": "agent"
                            },
                            "mind_observer": {
                                "events": [
                                    {"status":"success","trigger":"manual_shortcut","runtime":"observer","reason":"repo a event"}
                                ]
                            }
                        })),
                    },
                    AgentState {
                        agent_id: "session-test::13".to_string(),
                        session_id: "session-test".to_string(),
                        pane_id: "13".to_string(),
                        lifecycle: "running".to_string(),
                        snippet: None,
                        last_heartbeat_ms: Some(1),
                        last_activity_ms: Some(1),
                        updated_at_ms: Some(1),
                        source: Some(serde_json::json!({
                            "agent_status": {
                                "agent_label": "Repo B",
                                "project_root": "/repo-b",
                                "tab_scope": "agent"
                            },
                            "mind_observer": {
                                "events": [
                                    {"status":"success","trigger":"manual_shortcut","runtime":"observer","reason":"repo b event"}
                                ]
                            }
                        })),
                    }
                ],
            },
            event_at: Utc::now(),
        });

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("project: /repo-a [project-scoped]"));
    assert!(rendered.contains("repo a event"));
    assert!(!rendered.contains("repo b event"));
}

#[test]
fn render_mind_lines_search_query_returns_local_hits() {
    let (root, store_path) = fresh_test_mind_store("aoc-mission-control-mind-search");
    let store = aoc_storage::MindStore::open(&store_path).expect("open store");
    let now = Utc::now();
    store
            .upsert_handshake_snapshot(
                "project",
                &project_scope_key(&root),
                "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:planner-drift r1] topic=planner confidence=8800 freshness=95 :: Planner drift contract and routing notes\n",
                "hash:handshake",
                128,
                now,
            )
            .expect("upsert handshake");
    store
        .upsert_canon_entry_revision(
            "canon:planner-drift",
            Some("planner"),
            "Planner drift contract and routing notes",
            8800,
            95,
            None,
            &["obs:planner".to_string()],
            now,
        )
        .expect("upsert canon");

    let (tx, _rx) = mpsc::channel(4);
    let mut config = test_config();
    config.project_root = root.clone();
    config.mind_project_scoped = true;
    let mut app = App::new(config, tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;
    app.mind_lane = MindLaneFilter::All;
    app.mind_search_query = "planner drift".to_string();

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Retrieval / search"));
    assert!(rendered.contains("query: > planner drift"));
    assert!(rendered.contains("selected:1/2"));
    assert!(rendered.contains(">> [canon] planner score:10"));
    assert!(rendered.contains("selected: [canon] planner"));
    assert!(rendered.contains("entry_id: canon:planner-drift"));
    assert!(rendered.contains("revision: 1"));
    assert!(rendered.contains("evidence: 1"));
    assert!(rendered.contains("Planner drift contract and routing notes"));

    drop(store);
    cleanup_test_mind_store(&root, &store_path);
}

#[test]
fn render_mind_lines_without_observer_events_still_shows_artifacts() {
    let (root, store_path) = fresh_test_mind_store("aoc-mission-control-mind-artifacts-only");
    let store = aoc_storage::MindStore::open(&store_path).expect("open store");
    let now = Utc::now();
    store
            .upsert_handshake_snapshot(
                "project",
                &project_scope_key(&root),
                "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:entry-a r1] topic=mind confidence=8800 freshness=95 :: Keep this in startup context\n",
                "hash:handshake",
                128,
                now,
            )
            .expect("upsert handshake");

    let (tx, _rx) = mpsc::channel(4);
    let mut config = test_config();
    config.project_root = root.clone();
    config.mind_project_scoped = true;
    let mut app = App::new(config, tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;
    app.mind_lane = MindLaneFilter::All;

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Observer activity [0 events]"));
    assert!(rendered
        .contains("overview: handshake:1 canon:0 stale:0 latest:none recovery:none detached:0"));
    assert!(rendered.contains("Retrieval / search"));
    assert!(rendered.contains("No observer activity yet for current lane/scope."));
    assert!(rendered.contains("Activity summary [project-local]"));
    assert!(rendered.contains("Mission Control bridge [global follow-up]"));
    assert!(rendered.contains("Knowledge artifacts Artifact drilldown"));
    assert!(rendered.contains("Handshake + canon"));

    drop(store);
    cleanup_test_mind_store(&root, &store_path);
}

#[test]
fn render_mind_activity_summary_and_bridge_for_detached_jobs() {
    let rows = vec![MindObserverRow {
        agent_id: "agent-1".to_string(),
        scope: "project".to_string(),
        pane_id: "11".to_string(),
        tab_scope: Some("agent".to_string()),
        tab_focused: true,
        source: "hub".to_string(),
        event: MindObserverFeedEvent {
            status: MindObserverFeedStatus::Success,
            trigger: MindObserverFeedTriggerKind::ManualShortcut,
            conversation_id: None,
            runtime: Some("observer".to_string()),
            attempt_count: None,
            latency_ms: None,
            reason: Some("repo event".to_string()),
            failure_kind: None,
            enqueued_at: None,
            started_at: None,
            completed_at: Some("2026-03-27T10:00:00Z".to_string()),
            progress: None,
        },
    }];
    let injections = vec![MindInjectionRow {
        scope: "project".to_string(),
        pane_id: "11".to_string(),
        tab_focused: true,
        payload: MindInjectionPayload {
            status: "pending".to_string(),
            trigger: aoc_core::mind_observer_feed::MindInjectionTriggerKind::Startup,
            scope: "project".to_string(),
            scope_key: "project:/repo".to_string(),
            active_tag: Some("env-protec".to_string()),
            reason: Some("seed project context".to_string()),
            snapshot_id: Some("hs:123".to_string()),
            payload_hash: None,
            token_estimate: Some(123),
            context_pack: None,
            queued_at: "2026-03-27T10:01:00Z".to_string(),
        },
    }];
    let jobs = vec![InsightDetachedJob {
        job_id: "job-1".to_string(),
        parent_job_id: None,
        owner_plane: InsightDetachedOwnerPlane::Mind,
        worker_kind: Some(InsightDetachedWorkerKind::T2),
        mode: aoc_core::insight_contracts::InsightDetachedMode::Dispatch,
        status: InsightDetachedJobStatus::Running,
        agent: None,
        team: None,
        chain: None,
        created_at_ms: 1_711_533_000_000,
        started_at_ms: Some(1_711_533_030_000),
        finished_at_ms: None,
        current_step_index: None,
        step_count: None,
        output_excerpt: None,
        stdout_excerpt: None,
        stderr_excerpt: None,
        error: None,
        fallback_used: false,
        step_results: Vec::new(),
    }];
    let snapshot = MindArtifactDrilldown {
        handshake_entries: vec![MindHandshakeEntry {
            entry_id: "canon:entry-a".to_string(),
            revision: 1,
            topic: Some("mind".to_string()),
            summary: "Keep this in startup context".to_string(),
        }],
        ..Default::default()
    };

    let rendered = render_mind_activity_bridge_lines(
        &rows,
        &injections,
        &jobs,
        &snapshot,
        mission_theme(MissionThemeMode::Terminal),
        false,
    )
    .into_iter()
    .map(|line| line.to_string())
    .collect::<Vec<_>>()
    .join("\n");

    assert!(rendered.contains("Activity summary [project-local]"));
    assert!(rendered.contains("latest-event:t1@10:00:00"));
    assert!(rendered.contains("latest-inject:pending@10:01:00"));
    assert!(rendered.contains("detached:1"));
    assert!(rendered.contains("Mission Control bridge [global follow-up]"));
    assert!(rendered.contains("press 4 for Fleet to inspect or cancel them"));
}

#[test]
fn render_mind_lines_shows_injection_rollup_and_store_backed_drilldown() {
    let (root, store_path) = fresh_test_mind_store("aoc-mission-control-mind-v2");
    let store = aoc_storage::MindStore::open(&store_path).expect("open store");
    let now = Utc::now();
    store
            .upsert_handshake_snapshot(
                "project",
                &project_scope_key(&root),
                "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:entry-a r1] topic=mind confidence=8800 freshness=95 :: Keep this in startup context\n",
                "hash:handshake",
                128,
                now,
            )
            .expect("upsert handshake");
    store
        .upsert_canon_entry_revision(
            "canon:entry-a",
            Some("mind"),
            "Consolidated summary",
            8800,
            95,
            None,
            &["obs:1".to_string(), "ref:2".to_string()],
            now,
        )
        .expect("upsert canon");

    let (tx, _rx) = mpsc::channel(4);
    let mut config = test_config();
    config.project_root = root.clone();
    let mut app = App::new(config, tx, empty_local());
    app.connected = true;
    app.mode = Mode::Mind;
    app.mind_lane = MindLaneFilter::All;
    app.mind_show_provenance = true;

    app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
                    agent_id: "session-test::12".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "OpenCode",
                            "project_root": root.to_string_lossy(),
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "mind_injection": {
                            "status": "pending",
                            "trigger": "resume",
                            "scope": "project",
                            "scope_key": project_scope_key(&root),
                            "active_tag": "mind",
                            "reason": "resume handshake refresh",
                            "snapshot_id": "hs:abc123",
                            "payload_hash": "hash:abc123",
                            "token_estimate": 128,
                            "queued_at": "2026-03-01T12:10:00Z"
                        },
                        "insight_runtime": {
                            "queue_depth": 1,
                            "t3_queue_depth": 0
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("overview: handshake:1 canon:1 stale:0"));
    assert!(rendered.contains("inject: [resume] [pending]"));
    assert!(rendered.contains("resume handshake refresh"));
    assert!(rendered.contains("handshake:1 active_canon:1 stale_canon:0"));
    assert!(rendered.contains("trace: handshake -> canon -> evidence[2] obs:1, ref:2"));

    drop(store);
    cleanup_test_mind_store(&root, &store_path);
}

#[test]
fn render_mind_lines_includes_artifact_provenance_drilldown() {
    let (root, store_path) = fresh_test_mind_store("aoc-mission-control-drilldown");
    let t3_dir = root.join(".aoc").join("mind").join("t3");
    let insight_dir = root
        .join(".aoc")
        .join("mind")
        .join("insight")
        .join("session-test_20260301T120000Z_abc123def456");
    std::fs::create_dir_all(&t3_dir).expect("create t3 dir");
    std::fs::create_dir_all(&insight_dir).expect("create insight dir");

    std::fs::write(
            t3_dir.join("handshake.md"),
            "# Mind Handshake Baseline\n\n## Priority canon\n\n- [canon:entry-a r3] topic=mind confidence=8800 freshness=95 :: Keep this in startup context\n",
        )
        .expect("write handshake");

    std::fs::write(
            t3_dir.join("project_mind.md"),
            "# Project Mind Canon\n\n## Active canon\n\n### canon:entry-a r3\n- topic: mind\n- evidence_refs: obs:1, ref:2\n\nConsolidated summary\n\n## Stale canon\n\n### canon:entry-old r1\n- topic: mind\n\nOld summary\n",
        )
        .expect("write project mind");

    std::fs::write(
        insight_dir.join("manifest.json"),
        r#"{
  "session_id": "session-test",
  "active_tag": "mind",
  "export_dir": "/tmp/session-test_20260301T120000Z_abc123def456",
  "t1_count": 2,
  "t2_count": 1,
  "t1_artifact_ids": ["obs:1", "obs:2"],
  "t2_artifact_ids": ["ref:2"],
  "slice_start_id": "obs:1",
  "slice_end_id": "ref:2",
  "t3_job_id": "t3:job:42",
  "exported_at": "2026-03-01T12:00:00Z"
}"#,
    )
    .expect("write manifest");

    let store = aoc_storage::MindStore::open(&store_path).expect("open store");
    let marker_event = aoc_core::mind_contracts::RawEvent {
        event_id: "evt-compaction-session-test-1".to_string(),
        conversation_id: "conv-compact".to_string(),
        agent_id: "agent-1".to_string(),
        ts: Utc
            .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
            .single()
            .expect("ts"),
        body: aoc_core::mind_contracts::RawEventBody::Other {
            payload: serde_json::json!({"type": "compaction"}),
        },
        attrs: std::collections::BTreeMap::from([
            (
                "mind_compaction_modified_files".to_string(),
                serde_json::json!(["src/main.rs", "README.md"]),
            ),
            (
                "pi_detail_read_files".to_string(),
                serde_json::json!(["src/lib.rs"]),
            ),
        ]),
    };
    store
        .insert_raw_event(&marker_event)
        .expect("insert marker");
    let checkpoint = aoc_storage::CompactionCheckpoint {
        checkpoint_id: "cmpchk:conv-compact:compact-1".to_string(),
        conversation_id: "conv-compact".to_string(),
        session_id: "session-test".to_string(),
        ts: marker_event.ts,
        trigger_source: "pi_compaction_checkpoint".to_string(),
        reason: Some("pi compaction".to_string()),
        summary: Some("Compacted prior work into durable summary".to_string()),
        tokens_before: Some(4096),
        first_kept_entry_id: Some("entry-42".to_string()),
        compaction_entry_id: Some("compact-1".to_string()),
        from_extension: true,
        marker_event_id: Some(marker_event.event_id.clone()),
        schema_version: 1,
        created_at: marker_event.ts,
        updated_at: marker_event.ts,
    };
    store
        .upsert_compaction_checkpoint(&checkpoint)
        .expect("upsert checkpoint");
    let slice = aoc_core::mind_contracts::build_compaction_t0_slice(
        &checkpoint.conversation_id,
        &checkpoint.session_id,
        checkpoint.ts,
        &checkpoint.trigger_source,
        checkpoint.reason.as_deref(),
        checkpoint.summary.as_deref(),
        checkpoint.tokens_before,
        checkpoint.first_kept_entry_id.as_deref(),
        checkpoint.compaction_entry_id.as_deref(),
        checkpoint.from_extension,
        "pi_compaction_checkpoint",
        &[marker_event.event_id.clone()],
        &["src/lib.rs".to_string()],
        &["src/main.rs".to_string(), "README.md".to_string()],
        Some(&checkpoint.checkpoint_id),
        "t0.compaction.v1",
    )
    .expect("build slice");
    store
        .upsert_compaction_t0_slice(&slice)
        .expect("upsert slice");

    let (tx, _rx) = mpsc::channel(4);
    let mut cfg = test_config();
    cfg.project_root = root.clone();
    let mut app = App::new(cfg, tx, empty_local());
    app.mode = Mode::Mind;
    app.connected = true;
    app.mind_lane = MindLaneFilter::All;
    app.mind_show_provenance = true;

    app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
                    agent_id: "session-test::12".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "OpenCode",
                            "project_root": root.to_string_lossy().to_string(),
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"success","trigger":"compaction","runtime":"deterministic","conversation_id":"conv-compact","reason":"pi compaction checkpoint","completed_at":"2026-03-01T12:00:02Z"},
                                {"status":"success","trigger":"task_completed","runtime":"t3_backlog","reason":"t3 backlog processed 1 job(s)"}
                            ]
                        },
                        "insight_runtime": {
                            "queue_depth": 1,
                            "t3_queue_depth": 0
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

    let lines = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Artifact drilldown"));
    assert!(rendered.contains("[provenance:on]"));
    assert!(rendered.contains("health: t0:stored replay:ready t1:ok t2q:1 t3q:0"));
    assert!(rendered.contains("evidence: src:1 read:1 modified:2 policy:t0.compaction.v1"));
    assert!(
        rendered.contains("recovery: press 'C' to rebuild/requeue latest compaction checkpoint")
    );
    assert!(rendered.contains("[canon:entry-a r3]"));
    assert!(rendered.contains("trace: handshake -> canon -> evidence[2] obs:1, ref:2"));

    drop(store);
    cleanup_test_mind_store(&root, &store_path);
}

#[test]
fn parse_bool_flag_accepts_rollout_values() {
    assert_eq!(parse_bool_flag("1"), Some(true));
    assert_eq!(parse_bool_flag("on"), Some(true));
    assert_eq!(parse_bool_flag("0"), Some(false));
    assert_eq!(parse_bool_flag("off"), Some(false));
    assert_eq!(parse_bool_flag("maybe"), None);
}

#[test]
fn parse_mission_theme_mode_accepts_known_values() {
    assert_eq!(
        parse_mission_theme_mode("terminal"),
        Some(MissionThemeMode::Terminal)
    );
    assert_eq!(
        parse_mission_theme_mode("AUTO"),
        Some(MissionThemeMode::Terminal)
    );
    assert_eq!(parse_mission_theme_mode("dark"), Some(MissionThemeMode::Dark));
    assert_eq!(parse_mission_theme_mode("light"), Some(MissionThemeMode::Light));
    assert_eq!(parse_mission_theme_mode("solarized"), None);
}

#[test]
fn layout_state_event_updates_local_tab_overlay() {
    let (tx, _rx) = mpsc::channel(4);
    let mut cfg = test_config();
    cfg.overview_enabled = true;
    let mut app = App::new(cfg, tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);
    app.set_local(LocalSnapshot {
        overview: vec![OverviewRow {
            identity_key: "session-test::12".to_string(),
            label: "pane-12".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "12".to_string(),
            tab_index: None,
            tab_name: None,
            tab_focused: false,
            project_root: "/tmp".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
            session_title: None,
            chat_title: None,
        }],
        viewer_tab_index: None,
        tab_roster: Vec::new(),
        work: Vec::new(),
        diff: Vec::new(),
        health: empty_local().health,
    });

    app.apply_hub_event(HubEvent::LayoutState {
        payload: LayoutStatePayload {
            layout_seq: 1,
            session_id: "session-test".to_string(),
            emitted_at_ms: 1,
            tabs: vec![aoc_core::pulse_ipc::LayoutTab {
                index: 3,
                name: "Agent".to_string(),
                focused: true,
            }],
            panes: vec![aoc_core::pulse_ipc::LayoutPane {
                pane_id: "12".to_string(),
                tab_index: 3,
                tab_name: "Agent".to_string(),
                tab_focused: true,
            }],
        },
    });

    assert_eq!(app.viewer_tab_index_from_hub_layout(), Some(3));
    let rows = app.overview_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tab_index, Some(3));
    assert!(rows[0].tab_focused);
}

#[test]
fn hub_layout_source_disables_local_layout_poll_when_connected() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.config.layout_source = LayoutSource::Hub;
    app.connected = true;
    app.hub.layout = Some(HubLayout {
        layout_seq: 2,
        pane_tabs: HashMap::from([(
            "12".to_string(),
            TabMeta {
                index: 1,
                name: "Agent".to_string(),
                focused: true,
            },
        )]),
        focused_tab_index: Some(1),
    });

    assert!(!app.should_poll_local_layout());
}

#[test]
fn hybrid_layout_source_uses_hub_layout_when_connected() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.config.layout_source = LayoutSource::Hybrid;
    app.connected = true;
    app.hub.layout = Some(HubLayout {
        layout_seq: 3,
        pane_tabs: HashMap::from([(
            "12".to_string(),
            TabMeta {
                index: 2,
                name: "Agent".to_string(),
                focused: true,
            },
        )]),
        focused_tab_index: Some(2),
    });

    assert!(!app.should_poll_local_layout());
}

#[test]
fn disconnected_hub_uses_cached_rows_during_grace() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.local.overview.push(OverviewRow {
        identity_key: "session-test::99".to_string(),
        label: "local-only".to_string(),
        lifecycle: "running".to_string(),
        snippet: None,
        pane_id: "99".to_string(),
        tab_index: Some(9),
        tab_name: Some("Agent".to_string()),
        tab_focused: false,
        project_root: "/tmp".to_string(),
        online: true,
        age_secs: Some(1),
        source: "runtime".to_string(),
        session_title: None,
        chat_title: None,
    });

    app.apply_hub_event(HubEvent::Connected);
    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![hub_state("session-test::12", "12", "/repo")],
        },
        event_at: Utc::now(),
    });
    app.apply_hub_event(HubEvent::Disconnected);

    assert_eq!(app.mode_source(), "hub");
    assert_eq!(app.hub_status_label(), "reconnecting");
    let rows = app.overview_rows();
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .any(|row| row.identity_key == "session-test::12"));
    assert!(rows
        .iter()
        .any(|row| row.identity_key == "session-test::99"));
}

#[test]
fn disconnected_hub_falls_back_after_grace_window() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.local.overview.push(OverviewRow {
        identity_key: "session-test::99".to_string(),
        label: "local-only".to_string(),
        lifecycle: "running".to_string(),
        snippet: None,
        pane_id: "99".to_string(),
        tab_index: Some(9),
        tab_name: Some("Agent".to_string()),
        tab_focused: false,
        project_root: "/tmp".to_string(),
        online: true,
        age_secs: Some(1),
        source: "runtime".to_string(),
        session_title: None,
        chat_title: None,
    });

    app.apply_hub_event(HubEvent::Connected);
    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![hub_state("session-test::12", "12", "/repo")],
        },
        event_at: Utc::now(),
    });
    app.apply_hub_event(HubEvent::Disconnected);
    app.hub_disconnected_at =
        Some(Utc::now() - chrono::Duration::seconds(HUB_RECONNECT_GRACE_SECS + 1));

    assert_eq!(app.mode_source(), "local");
    assert_eq!(app.hub_status_label(), "offline");
    let rows = app.overview_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].identity_key, "session-test::99");
}

#[test]
fn disconnect_clears_pending_commands_and_reconnect_restores_hub_mode() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);
    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![hub_state("session-test::12", "12", "/repo")],
        },
        event_at: Utc::now(),
    });
    app.pending_commands.insert(
        "req-reconnect".to_string(),
        PendingCommand {
            command: "run_validation".to_string(),
            target: "pane-12".to_string(),
        },
    );

    app.apply_hub_event(HubEvent::Disconnected);
    assert!(app.pending_commands.is_empty());
    assert_eq!(app.hub_status_label(), "reconnecting");
    assert_eq!(app.mode_source(), "hub");

    app.apply_hub_event(HubEvent::Connected);
    assert_eq!(app.hub_status_label(), "online");
    assert_eq!(app.mode_source(), "hub");
    assert_eq!(app.status_note.as_deref(), Some("hub connected"));
}

#[test]
fn reconnect_followed_by_snapshot_replaces_cached_hub_rows() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);
    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![hub_state("session-test::12", "12", "/repo")],
        },
        event_at: Utc::now(),
    });
    app.apply_hub_event(HubEvent::Disconnected);
    app.apply_hub_event(HubEvent::Connected);
    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 2,
            states: vec![hub_state("session-test::21", "21", "/repo")],
        },
        event_at: Utc::now(),
    });

    let rows = app.overview_rows();
    assert!(rows
        .iter()
        .any(|row| row.identity_key == "session-test::21"));
    assert!(!rows
        .iter()
        .any(|row| row.identity_key == "session-test::12"));
}

#[test]
fn overview_hub_mode_includes_local_only_rows() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.local.overview.push(OverviewRow {
        identity_key: "session-test::99".to_string(),
        label: "local-only".to_string(),
        lifecycle: "running".to_string(),
        snippet: None,
        pane_id: "99".to_string(),
        tab_index: Some(9),
        tab_name: Some("Agent".to_string()),
        tab_focused: false,
        project_root: "/tmp".to_string(),
        online: true,
        age_secs: Some(1),
        source: "runtime".to_string(),
        session_title: None,
        chat_title: None,
    });

    app.apply_hub_event(HubEvent::Connected);
    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![hub_state("session-test::12", "12", "/repo")],
        },
        event_at: Utc::now(),
    });

    let rows = app.overview_rows();
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .any(|row| row.identity_key == "session-test::12"));
    assert!(rows
        .iter()
        .any(|row| row.identity_key == "session-test::99"));
}

#[test]
fn overview_reuses_cached_tab_metadata_when_local_row_lacks_it() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());

    app.set_local(LocalSnapshot {
        overview: vec![OverviewRow {
            identity_key: "session-test::11".to_string(),
            label: "OpenCode".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "11".to_string(),
            tab_index: Some(2),
            tab_name: Some("tab-2".to_string()),
            tab_focused: false,
            project_root: "/tmp/project".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
            session_title: None,
            chat_title: None,
        }],
        viewer_tab_index: Some(2),
        tab_roster: vec![TabMeta {
            index: 2,
            name: "tab-2".to_string(),
            focused: false,
        }],
        work: Vec::new(),
        diff: Vec::new(),
        health: empty_local().health,
    });

    app.set_local(LocalSnapshot {
        overview: vec![OverviewRow {
            identity_key: "session-test::11".to_string(),
            label: "OpenCode".to_string(),
            lifecycle: "running".to_string(),
            snippet: None,
            pane_id: "11".to_string(),
            tab_index: None,
            tab_name: None,
            tab_focused: false,
            project_root: "/tmp/project".to_string(),
            online: true,
            age_secs: Some(1),
            source: "runtime".to_string(),
            session_title: None,
            chat_title: None,
        }],
        viewer_tab_index: Some(2),
        tab_roster: vec![TabMeta {
            index: 2,
            name: "tab-2".to_string(),
            focused: false,
        }],
        work: Vec::new(),
        diff: Vec::new(),
        health: empty_local().health,
    });

    let rows = app.overview_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tab_index, Some(2));
    assert_eq!(rows[0].tab_name.as_deref(), Some("tab-2"));
}

#[test]
fn prune_hub_cache_skips_local_miss_prune_when_overlap_is_low() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.local.overview.push(OverviewRow {
        identity_key: "session-test::1".to_string(),
        label: "OpenCode".to_string(),
        lifecycle: "running".to_string(),
        snippet: None,
        pane_id: "1".to_string(),
        tab_index: Some(1),
        tab_name: Some("Agent".to_string()),
        tab_focused: true,
        project_root: "/tmp/project".to_string(),
        online: true,
        age_secs: Some(1),
        source: "runtime".to_string(),
        session_title: None,
        chat_title: None,
    });

    app.apply_hub_event(HubEvent::Connected);
    app.apply_hub_event(HubEvent::Snapshot {
        payload: SnapshotPayload {
            seq: 1,
            states: vec![
                hub_state("session-test::1", "1", "/tmp/project"),
                hub_state("session-test::2", "2", "/tmp/project"),
                hub_state("session-test::3", "3", "/tmp/project"),
            ],
        },
        event_at: Utc::now() - chrono::Duration::seconds(HUB_LOCAL_MISS_PRUNE_SECS + 1),
    });

    app.prune_hub_cache();
    assert_eq!(app.hub.agents.len(), 3);
}

#[test]
fn extract_layout_pane_ids_supports_pane_id_attribute() {
    let line = r#"pane pane_id="44" name="Agent""#;
    let pane_ids = extract_pane_ids_from_layout_line(line);
    assert_eq!(pane_ids, vec!["44".to_string()]);
}

#[test]
fn extract_layout_pane_ids_supports_flag_and_hyphen_attribute() {
    let line = r#"pane command="runner" args "--pane-id" "55" pane-id="77""#;
    let pane_ids = extract_pane_ids_from_layout_line(line);
    assert_eq!(pane_ids, vec!["55".to_string(), "77".to_string()]);
}

#[test]
fn lifecycle_normalization_and_chips_are_stable() {
    assert_eq!(normalize_lifecycle(" needs_input "), "needs-input");
    assert_eq!(lifecycle_chip_label("needs_input", true), "NEEDS");
}

#[test]
fn overview_presenter_compact_keeps_critical_fields() {
    let row = OverviewRow {
        identity_key: "session-test::991122".to_string(),
        label: "very-long-opencode-agent-label".to_string(),
        lifecycle: "blocked".to_string(),
        snippet: Some("waiting on credentials and operator input".to_string()),
        pane_id: "9911223344".to_string(),
        tab_index: None,
        tab_name: None,
        tab_focused: false,
        project_root: "/tmp/some/project/with/long/path".to_string(),
        online: true,
        age_secs: Some(47),
        source: "hub+runtime".to_string(),
        session_title: None,
        chat_title: None,
    };

    let decorations = OverviewDecorations {
        attention_chip: attention_chip_from_row(&row),
        context: "waiting on credentials and operator input".to_string(),
        task_signal: Some("W:1/4".to_string()),
        git_signal: Some("G:+7/-2 ?1".to_string()),
    };
    let presenter = overview_row_presenter(&row, &decorations, true, 80);
    assert!(presenter.identity.contains("::"));
    assert_eq!(presenter.location_chip, "T?:???");
    assert_eq!(presenter.lifecycle_chip, "[BLOCK]");
    assert_eq!(
        presenter.badge,
        OverviewBadge::Attention(AttentionChip::Blocked)
    );
    assert!(presenter.freshness.contains("47s"));
    assert!(presenter.context.starts_with("M:"));
    assert!(presenter_text_len(&presenter) <= 72);
}

#[test]
fn overview_toggle_methods_update_state() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    assert_eq!(app.overview_sort_mode, OverviewSortMode::Layout);

    app.toggle_overview_sort_mode();
    assert_eq!(app.overview_sort_mode, OverviewSortMode::Attention);
}

#[test]
fn parse_runtime_mode_accepts_primary_labels() {
    assert_eq!(
        parse_runtime_mode("pulse-pane"),
        Some(RuntimeMode::MissionControl)
    );
    assert_eq!(
        parse_runtime_mode("mission-control"),
        Some(RuntimeMode::MissionControl)
    );
    assert_eq!(parse_runtime_mode("mc"), Some(RuntimeMode::MissionControl));
    assert_eq!(parse_runtime_mode("unknown"), None);
}

#[test]
fn parse_start_view_accepts_fleet_and_aliases() {
    assert_eq!(parse_start_view("fleet"), Some(Mode::Fleet));
    assert_eq!(parse_start_view("subagents"), Some(Mode::Fleet));
    assert_eq!(parse_start_view("overview"), Some(Mode::Overview));
    assert_eq!(parse_start_view("unknown"), None);
}

#[test]
fn parse_fleet_plane_filter_accepts_delegated_aliases() {
    assert_eq!(
        parse_fleet_plane_filter("delegated"),
        Some(FleetPlaneFilter::Delegated)
    );
    assert_eq!(
        parse_fleet_plane_filter("subagents"),
        Some(FleetPlaneFilter::Delegated)
    );
    assert_eq!(
        parse_fleet_plane_filter("mind"),
        Some(FleetPlaneFilter::Mind)
    );
    assert_eq!(parse_fleet_plane_filter("unknown"), None);
}

#[test]
fn app_new_honors_start_view_and_fleet_plane_filter() {
    let (tx, _rx) = mpsc::channel(4);
    let mut cfg = test_config();
    cfg.start_view = Some(Mode::Fleet);
    cfg.fleet_plane_filter = FleetPlaneFilter::Delegated;
    let app = App::new(cfg, tx, empty_local());
    assert_eq!(app.mode, Mode::Fleet);
    assert_eq!(app.fleet_plane_filter, FleetPlaneFilter::Delegated);
}

#[test]
fn detached_worker_kind_display_expands_mind_runtime_labels() {
    assert_eq!(
        detached_worker_kind_display(
            InsightDetachedOwnerPlane::Mind,
            Some(InsightDetachedWorkerKind::T2)
        ),
        "t2-reflector"
    );
    assert_eq!(
        detached_worker_kind_display(
            InsightDetachedOwnerPlane::Mind,
            Some(InsightDetachedWorkerKind::T3)
        ),
        "t3-runtime"
    );
    assert_eq!(
        detached_worker_kind_display(
            InsightDetachedOwnerPlane::Delegated,
            Some(InsightDetachedWorkerKind::Specialist)
        ),
        "specialist"
    );
}

#[test]
fn render_fleet_brief_uses_mission_control_followup_for_mind_jobs() {
    let (tx, _rx) = mpsc::channel(4);
    let app = App::new(test_config(), tx, empty_local());
    let job = InsightDetachedJob {
        job_id: "mind-t2-brief-test".to_string(),
        parent_job_id: None,
        owner_plane: InsightDetachedOwnerPlane::Mind,
        worker_kind: Some(InsightDetachedWorkerKind::T2),
        mode: aoc_core::insight_contracts::InsightDetachedMode::Dispatch,
        status: InsightDetachedJobStatus::Stale,
        agent: Some("mind-t2-reflector".to_string()),
        team: None,
        chain: None,
        created_at_ms: 1_700_000_000_000,
        started_at_ms: Some(1_700_000_000_100),
        finished_at_ms: Some(1_700_000_000_200),
        current_step_index: Some(1),
        step_count: Some(1),
        output_excerpt: Some("mind t2 worker marked stale after lease expiry".to_string()),
        stdout_excerpt: None,
        stderr_excerpt: None,
        error: Some(
            "Mind T2 worker lease expired before detached completion was observed".to_string(),
        ),
        fallback_used: false,
        step_results: Vec::new(),
    };
    let row = DetachedFleetRow {
        project_root: "/repo".to_string(),
        owner_plane: InsightDetachedOwnerPlane::Mind,
        jobs: vec![job.clone()],
    };

    let rendered = app.render_fleet_brief(&row, &job, false);
    assert!(rendered.contains("worker_kind: t2-reflector"));
    assert!(rendered.contains("Mission Control Fleet or Mind"));
    assert!(!rendered.contains("/subagent-inspect"));
    assert!(rendered.contains("lost lease or restart continuity"));
}

#[test]
fn pulse_subscribe_includes_overseer_topics() {
    let subscribe = build_pulse_subscribe(&test_config());
    let WireMsg::Subscribe(payload) = subscribe.msg else {
        panic!("expected subscribe envelope")
    };
    assert!(payload
        .topics
        .iter()
        .any(|topic| topic == "observer_snapshot"));
    assert!(payload
        .topics
        .iter()
        .any(|topic| topic == "observer_timeline"));
    assert!(payload
        .topics
        .iter()
        .any(|topic| topic == "consultation_response"));
    assert!(payload.topics.iter().any(|topic| topic == "layout_state"));
}

#[test]
fn overseer_mode_renders_worker_and_timeline_data() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);
    app.mode = Mode::Overseer;

    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![WorkerSnapshot {
                session_id: "session-test".to_string(),
                agent_id: "session-test::12".to_string(),
                pane_id: "12".to_string(),
                role: Some("worker".to_string()),
                status: WorkerStatus::Blocked,
                assignment: aoc_core::session_overseer::WorkerAssignment {
                    task_id: Some("149.5".to_string()),
                    tag: Some("session-overseer".to_string()),
                    epic_id: Some("149".to_string()),
                },
                summary: Some("waiting for Mission Control render wiring".to_string()),
                plan_alignment: PlanAlignment::Medium,
                drift_risk: DriftRisk::High,
                attention: aoc_core::session_overseer::AttentionSignal {
                    level: AttentionLevel::Warn,
                    kind: Some("blocked".to_string()),
                    reason: Some("awaiting operator confirmation".to_string()),
                },
                ..Default::default()
            }],
            timeline: vec![],
            degraded_reason: None,
        },
    });
    app.apply_hub_event(HubEvent::ObserverTimeline {
        payload: ObserverTimelinePayload {
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_123),
            entries: vec![ObserverTimelineEntry {
                event_id: "evt-1".to_string(),
                session_id: "session-test".to_string(),
                agent_id: "session-test::12".to_string(),
                kind: aoc_core::session_overseer::ObserverEventKind::Blocked,
                summary: Some("worker reported blocker".to_string()),
                emitted_at_ms: Some(1_700_000_000_123),
                ..Default::default()
            }],
        },
    });

    assert_eq!(app.mode_source(), "hub");
    let lines = render_overseer_lines(&app, mission_theme(MissionThemeMode::Terminal), false);
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("149.5"));
    assert!(rendered.contains("awaiting operator confirmation"));
    assert!(rendered.contains("worker reported blocker"));
}

#[test]
fn render_mind_lines_shows_partial_compaction_health_when_recovery_is_degraded() {
    let (root, store_path) = fresh_test_mind_store("aoc-mission-control-compaction-degraded");
    let store = aoc_storage::MindStore::open(&store_path).expect("open store");
    let marker_event = aoc_core::mind_contracts::RawEvent {
        event_id: "evt-compaction-degraded-1".to_string(),
        conversation_id: "conv-degraded".to_string(),
        agent_id: "agent-1".to_string(),
        ts: Utc
            .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
            .single()
            .expect("ts"),
        body: aoc_core::mind_contracts::RawEventBody::Other {
            payload: serde_json::json!({"type": "compaction"}),
        },
        attrs: std::collections::BTreeMap::new(),
    };
    store
        .insert_raw_event(&marker_event)
        .expect("insert marker");
    store
        .upsert_compaction_checkpoint(&aoc_storage::CompactionCheckpoint {
            checkpoint_id: "cmpchk:conv-degraded:compact-1".to_string(),
            conversation_id: "conv-degraded".to_string(),
            session_id: "session-test".to_string(),
            ts: marker_event.ts,
            trigger_source: "pi_compaction_checkpoint".to_string(),
            reason: Some("pi compaction".to_string()),
            summary: Some("checkpoint exists but replay provenance is degraded".to_string()),
            tokens_before: Some(1024),
            first_kept_entry_id: Some("entry-7".to_string()),
            compaction_entry_id: Some("compact-1".to_string()),
            from_extension: true,
            marker_event_id: Some(marker_event.event_id.clone()),
            schema_version: 1,
            created_at: marker_event.ts,
            updated_at: marker_event.ts,
        })
        .expect("upsert checkpoint");

    let (tx, _rx) = mpsc::channel(4);
    let mut cfg = test_config();
    cfg.project_root = root.clone();
    let mut app = App::new(cfg, tx, empty_local());
    app.mode = Mode::Mind;
    app.connected = true;
    app.mind_lane = MindLaneFilter::All;

    app.apply_hub_event(HubEvent::Snapshot {
            payload: SnapshotPayload {
                seq: 1,
                states: vec![AgentState {
                    agent_id: "session-test::12".to_string(),
                    session_id: "session-test".to_string(),
                    pane_id: "12".to_string(),
                    lifecycle: "running".to_string(),
                    snippet: None,
                    last_heartbeat_ms: Some(1),
                    last_activity_ms: Some(1),
                    updated_at_ms: Some(1),
                    source: Some(serde_json::json!({
                        "agent_status": {
                            "agent_label": "OpenCode",
                            "project_root": root.to_string_lossy().to_string(),
                            "tab_scope": "agent"
                        },
                        "mind_observer": {
                            "events": [
                                {"status":"error","trigger":"compaction","runtime":"deterministic","conversation_id":"conv-degraded","reason":"semantic stage failed","completed_at":"2026-03-01T12:00:02Z"}
                            ]
                        },
                        "insight_runtime": {
                            "queue_depth": 2,
                            "t3_queue_depth": 1
                        }
                    })),
                }],
            },
            event_at: Utc::now(),
        });

    let rendered = render_mind_lines(&app, mission_theme(MissionThemeMode::Terminal), false)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("health: t0:missing replay:partial t1:error t2q:2 t3q:1"));
    assert!(
        rendered.contains("recovery: press 'C' to rebuild/requeue latest compaction checkpoint")
    );

    drop(store);
    cleanup_test_mind_store(&root, &store_path);
}

#[test]
fn overseer_mode_adds_optional_mind_enrichment_without_blocking_base_render() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);
    app.mode = Mode::Overseer;

    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![WorkerSnapshot {
                session_id: "session-test".to_string(),
                agent_id: "session-test::12".to_string(),
                pane_id: "12".to_string(),
                role: Some("worker".to_string()),
                status: WorkerStatus::Active,
                summary: Some("shipping deterministic overseer baseline".to_string()),
                provenance: Some("heuristic:wrapper+taskmaster".to_string()),
                ..Default::default()
            }],
            timeline: vec![],
            degraded_reason: None,
        },
    });

    let baseline = render_overseer_lines(&app, mission_theme(MissionThemeMode::Terminal), false)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(baseline.contains("shipping deterministic overseer baseline"));
    assert!(baseline.contains("[prov:heuristic:wrapper+taskmaster]"));
    assert!(baseline.contains("mc "));
    assert!(baseline.contains("assign task/tag before further implementation"));

    app.hub.mind.insert(
        "session-test::12".to_string(),
        MindObserverFeedPayload {
            updated_at_ms: Some(1_700_000_000_111),
            events: vec![MindObserverFeedEvent {
                status: MindObserverFeedStatus::Fallback,
                trigger: MindObserverFeedTriggerKind::TaskCompleted,
                conversation_id: None,
                runtime: Some("pi-semantic".to_string()),
                attempt_count: Some(1),
                latency_ms: Some(96),
                reason: Some(
                    "semantic observer timed out; using bounded heuristic summary".to_string(),
                ),
                failure_kind: Some("timeout".to_string()),
                enqueued_at: None,
                started_at: None,
                completed_at: Some("2026-03-09T10:45:00Z".to_string()),
                progress: None,
            }],
        },
    );

    let enriched = render_overseer_lines(&app, mission_theme(MissionThemeMode::Terminal), false)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(enriched.contains("[prov:heuristic:wrapper+taskmaster+mind:t1:fallback]"));
    assert!(enriched.contains("semantic [t1:fallback]"));
    assert!(enriched.contains("semantic observer timed out"));
}

#[test]
fn overseer_mode_renders_mission_control_prompt_for_blocked_worker() {
    let (tx, _rx) = mpsc::channel(4);
    let mut app = App::new(test_config(), tx, empty_local());
    app.apply_hub_event(HubEvent::Connected);
    app.mode = Mode::Overseer;

    app.apply_hub_event(HubEvent::ObserverSnapshot {
        payload: ObserverSnapshot {
            schema_version: 1,
            session_id: "session-test".to_string(),
            generated_at_ms: Some(1_700_000_000_000),
            workers: vec![WorkerSnapshot {
                session_id: "session-test".to_string(),
                agent_id: "session-test::22".to_string(),
                pane_id: "22".to_string(),
                role: Some("reviewer".to_string()),
                status: WorkerStatus::Blocked,
                summary: Some("waiting on design clarification".to_string()),
                blocker: Some("need operator decision on packet shape".to_string()),
                ..Default::default()
            }],
            timeline: vec![],
            degraded_reason: None,
        },
    });

    let rendered = render_overseer_lines(&app, mission_theme(MissionThemeMode::Terminal), false)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("mc "));
    assert!(rendered.contains("ask for unblock plan + evidence-backed next step"));
    assert!(rendered.contains("src:partial"));
}
