use aoc_core::{
    mind_contracts::text_contains_unredacted_secret,
    mind_observer_feed::MindObserverFeedTriggerKind,
    provenance_contracts::MindProvenanceQueryRequest,
};
use aoc_mind::{
    compile_mind_context_pack, compile_mind_provenance_export, default_pi_session_root,
    discover_latest_pi_session_file, mind_progress_for_conversation, open_project_store,
    parse_mind_context_pack_mode, prepare_session_finalize_execution,
    read_mind_service_health_snapshot, read_mind_service_lease, summarize_mind_service_status,
    sync_latest_pi_session_into_project_store, sync_session_file_into_project_store,
    DistillationConfig, MindContextPackProfile, MindContextPackRequest, MindProjectPaths,
    MindRuntimeConfig, MindRuntimeCore, SessionFinalizePreparationOutcome,
};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

const DEFAULT_AGENT_ID: &str = "aoc-mind-standalone";

#[derive(Debug, Parser)]
#[command(name = "aoc-mind-service")]
#[command(about = "Project-scoped standalone Mind ingest/runtime helper")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print resolved runtime/store/session discovery state for a project.
    Status {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Ingest one Pi session JSONL file (or the latest discovered file) into the project Mind store.
    SyncPi {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        session_file: Option<PathBuf>,
        #[arg(long, default_value = DEFAULT_AGENT_ID)]
        agent_id: String,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Run one detached T2 reflector worker tick.
    ReflectorRun {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long, default_value = "standalone")]
        session_id: String,
        #[arg(long, default_value = "service")]
        pane_id: String,
        #[arg(long, default_value = DEFAULT_AGENT_ID)]
        agent_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Run one detached T3 canon worker tick.
    T3Run {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long, default_value = "standalone")]
        session_id: String,
        #[arg(long, default_value = "service")]
        pane_id: String,
        #[arg(long, default_value = DEFAULT_AGENT_ID)]
        agent_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Audit Mind store, Pi session sync, service, and pipeline health.
    Doctor {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Poll for the latest Pi session JSONL and keep ingesting into the project Mind store.
    WatchPi {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        session_file: Option<PathBuf>,
        #[arg(long, default_value = DEFAULT_AGENT_ID)]
        agent_id: String,
        #[arg(long, default_value_t = 1000)]
        interval_ms: u64,
        #[arg(long)]
        json: bool,
    },
    /// Compile a project-scoped Mind context pack without Pulse/wrapper transport.
    ContextPack {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        mode: Option<String>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = false)]
        detail: bool,
        #[arg(long)]
        active_tag: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run a manual project-scoped Mind observer cycle without Pulse/wrapper transport.
    ObserverRun {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        pane_id: String,
        #[arg(long)]
        conversation_id: Option<String>,
        #[arg(long, default_value = DEFAULT_AGENT_ID)]
        agent_id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Compile a project-scoped Mind provenance export without Pulse/wrapper transport.
    ProvenanceQuery {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        conversation_id: Option<String>,
        #[arg(long)]
        artifact_id: Option<String>,
        #[arg(long)]
        checkpoint_id: Option<String>,
        #[arg(long)]
        canon_entry_id: Option<String>,
        #[arg(long)]
        task_id: Option<String>,
        #[arg(long)]
        file_path: Option<String>,
        #[arg(long)]
        active_tag: Option<String>,
        #[arg(long, default_value_t = false)]
        include_stale_canon: bool,
        #[arg(long, default_value_t = 64)]
        max_nodes: usize,
        #[arg(long, default_value_t = 128)]
        max_edges: usize,
        #[arg(long)]
        json: bool,
    },
    /// Finalize the current project-scoped Mind session slice without Pulse/wrapper transport.
    FinalizeSession {
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        pane_id: String,
        #[arg(long)]
        conversation_id: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let args = Args::parse();
    let code = match args.command {
        Command::Status { project_root, json } => run_status(&project_root, json),
        Command::SyncPi {
            project_root,
            session_file,
            agent_id,
            all,
            json,
        } => run_sync_pi(&project_root, session_file.as_deref(), &agent_id, all, json),
        Command::ReflectorRun {
            project_root,
            session_id,
            pane_id,
            agent_id,
            json,
        } => run_reflector_run(&project_root, &session_id, &pane_id, &agent_id, json),
        Command::T3Run {
            project_root,
            session_id,
            pane_id,
            agent_id,
            json,
        } => run_t3_run(&project_root, &session_id, &pane_id, &agent_id, json),
        Command::Doctor { project_root, json } => run_doctor(&project_root, json),
        Command::WatchPi {
            project_root,
            session_file,
            agent_id,
            interval_ms,
            json,
        } => run_watch_pi(
            &project_root,
            session_file.as_deref(),
            &agent_id,
            interval_ms,
            json,
        ),
        Command::ContextPack {
            project_root,
            mode,
            role,
            reason,
            detail,
            active_tag,
            json,
        } => run_context_pack(&project_root, mode.as_deref(), role, reason, detail, active_tag, json),
        Command::ObserverRun {
            project_root,
            session_id,
            pane_id,
            conversation_id,
            agent_id,
            reason,
            json,
        } => run_observer_run(
            &project_root,
            &session_id,
            &pane_id,
            conversation_id.as_deref(),
            &agent_id,
            reason,
            json,
        ),
        Command::ProvenanceQuery {
            project_root,
            session_id,
            conversation_id,
            artifact_id,
            checkpoint_id,
            canon_entry_id,
            task_id,
            file_path,
            active_tag,
            include_stale_canon,
            max_nodes,
            max_edges,
            json,
        } => run_provenance_query(
            &project_root,
            MindProvenanceQueryRequest {
                project_root: Some(project_root.display().to_string()),
                session_id,
                conversation_id,
                artifact_id,
                checkpoint_id,
                canon_entry_id,
                task_id,
                file_path,
                active_tag,
                include_stale_canon,
                max_nodes,
                max_edges,
            },
            json,
        ),
        Command::FinalizeSession {
            project_root,
            session_id,
            pane_id,
            conversation_id,
            reason,
            json,
        } => run_finalize_session(
            &project_root,
            &session_id,
            &pane_id,
            conversation_id.as_deref(),
            reason,
            json,
        ),
    };
    std::process::exit(code);
}

fn run_status(project_root: &PathBuf, as_json: bool) -> i32 {
    let paths = MindProjectPaths::for_project_root(project_root);
    let discovered = discover_latest_pi_session_file(project_root);
    let lease = read_mind_service_lease(project_root).ok().flatten();
    let health = read_mind_service_health_snapshot(project_root)
        .ok()
        .flatten();
    let service_status = summarize_mind_service_status(
        lease.as_ref(),
        health.as_ref(),
        chrono::Utc::now().timestamp_millis(),
    );
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "project_root": project_root,
                "runtime_root": paths.runtime_root,
                "store_path": paths.store_path,
                "legacy_root": paths.legacy_root,
                "locks_dir": paths.locks_dir,
                "reflector_lock_path": paths.reflector_lock_path,
                "t3_lock_path": paths.t3_lock_path,
                "reflector_dispatch_lock_path": paths.reflector_dispatch_lock_path,
                "t3_dispatch_lock_path": paths.t3_dispatch_lock_path,
                "service_lock_path": paths.service_lock_path,
                "health_snapshot_path": paths.health_snapshot_path,
                "store_exists": paths.store_path.exists(),
                "latest_pi_session_file": discovered,
                "service_lease": lease,
                "health_snapshot": health,
                "service_status": service_status,
            }))
            .expect("status json")
        );
    } else {
        println!("project_root: {}", project_root.display());
        println!("runtime_root: {}", paths.runtime_root.display());
        println!("store_path: {}", paths.store_path.display());
        println!("store_exists: {}", paths.store_path.exists());
        println!("service_lock_path: {}", paths.service_lock_path.display());
        println!(
            "health_snapshot_path: {}",
            paths.health_snapshot_path.display()
        );
        println!(
            "latest_pi_session_file: {}",
            discovered
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        );
        println!(
            "service_lease_owner: {}",
            lease
                .as_ref()
                .map(|lease| lease.owner_id.as_str())
                .unwrap_or("<none>")
        );
        println!(
            "health_lifecycle: {}",
            health
                .as_ref()
                .map(|health| health.lifecycle.as_str())
                .filter(|value| !value.is_empty())
                .unwrap_or("<none>")
        );
        println!("service_state: {}", service_status.state);
        println!("service_stale: {}", if service_status.stale { "yes" } else { "no" });
        if let Some(blocker) = service_status.blocker.as_deref() {
            println!("service_blocker: {}", blocker);
        }
    }
    0
}

fn run_sync_pi(
    project_root: &PathBuf,
    session_file: Option<&std::path::Path>,
    agent_id: &str,
    all: bool,
    as_json: bool,
) -> i32 {
    if all {
        return run_sync_pi_all(project_root, agent_id, as_json);
    }

    let result = if let Some(session_file) = session_file {
        sync_session_file_into_project_store(project_root, agent_id, session_file).map(Some)
    } else {
        sync_latest_pi_session_into_project_store(project_root, agent_id)
    };

    match result {
        Ok(Some(sync)) => {
            let distill = DistillationConfig::default();
            let progress = open_project_store(project_root, "", "", None)
                .ok()
                .and_then(|opened| {
                    mind_progress_for_conversation(
                        &opened.store,
                        &sync.report.conversation_id,
                        distill.t1_target_tokens,
                        distill.t1_hard_cap_tokens,
                    )
                });
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "session_file": sync.session_file,
                        "conversation_id": sync.report.conversation_id,
                        "processed_raw_events": sync.report.processed_raw_events,
                        "produced_t0_events": sync.report.produced_t0_events,
                        "persisted_compaction_checkpoints": sync.report.persisted_compaction_checkpoints,
                        "skipped_corrupt_lines": sync.report.skipped_corrupt_lines,
                        "deferred_partial_line": sync.report.deferred_partial_line,
                        "reset_due_to_truncation": sync.report.reset_due_to_truncation,
                        "raw_cursor": sync.report.raw_cursor,
                        "t0_cursor": sync.report.t0_cursor,
                        "progress": progress,
                    }))
                    .expect("sync json")
                );
            } else {
                println!("ingested: {}", sync.session_file.display());
                println!("conversation_id: {}", sync.report.conversation_id);
                println!("processed_raw_events: {}", sync.report.processed_raw_events);
                println!("produced_t0_events: {}", sync.report.produced_t0_events);
                println!(
                    "persisted_compaction_checkpoints: {}",
                    sync.report.persisted_compaction_checkpoints
                );
                println!("raw_cursor: {}", sync.report.raw_cursor);
            }
            0
        }
        Ok(None) => {
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "session_file": null,
                        "message": "no Pi session file discovered",
                    }))
                    .expect("empty json")
                );
            } else {
                println!("no Pi session file discovered");
            }
            0
        }
        Err(err) => {
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": false,
                        "error": err.to_string(),
                    }))
                    .expect("error json")
                );
            } else {
                eprintln!("sync failed: {err}");
            }
            1
        }
    }
}

fn discover_pi_session_files(project_root: &PathBuf) -> Vec<PathBuf> {
    let Some(root) = default_pi_session_root(project_root) else {
        return Vec::new();
    };
    let Ok(read_dir) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files = read_dir
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn conversation_id_from_session_file(path: &std::path::Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let id = stem.rsplit_once('_').map(|(_, id)| id).unwrap_or(stem);
    if id.len() == 36 && id.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-') {
        Some(format!("pi:{id}"))
    } else {
        None
    }
}

fn session_sync_audit(project_root: &PathBuf) -> serde_json::Value {
    let files = discover_pi_session_files(project_root);
    let checkpoints = open_project_store(project_root, "standalone", "service", None)
        .ok()
        .and_then(|opened| {
            files.iter().try_fold(
                (0_u64, 0_u64),
                |(missing, partial), path| -> Result<(u64, u64), ()> {
                    let Some(conversation_id) = conversation_id_from_session_file(path) else {
                        return Ok((missing + 1, partial));
                    };
                    let size = path.metadata().map(|meta| meta.len()).unwrap_or(0);
                    let checkpoint = opened
                        .store
                        .checkpoint(&conversation_id)
                        .map_err(|_| ())?;
                    match checkpoint {
                        None => Ok((missing + 1, partial)),
                        Some(checkpoint) if checkpoint.raw_cursor < size => Ok((missing, partial + 1)),
                        Some(_) => Ok((missing, partial)),
                    }
                },
            )
            .ok()
        });
    let (missing, partial) = checkpoints.unwrap_or((files.len() as u64, 0));
    json!({
        "session_files": files.len(),
        "missing": missing,
        "partial": partial,
    })
}

fn run_sync_pi_all(project_root: &PathBuf, agent_id: &str, as_json: bool) -> i32 {
    let files = discover_pi_session_files(project_root);
    let mut files_synced = 0_u64;
    let mut files_failed = 0_u64;
    let mut processed_raw_events = 0_u64;
    let mut produced_t0_events = 0_u64;
    let mut persisted_compaction_checkpoints = 0_u64;
    let mut errors = Vec::new();

    for path in &files {
        match sync_session_file_into_project_store(project_root, agent_id, path) {
            Ok(sync) => {
                files_synced += 1;
                processed_raw_events += sync.report.processed_raw_events as u64;
                produced_t0_events += sync.report.produced_t0_events as u64;
                persisted_compaction_checkpoints +=
                    sync.report.persisted_compaction_checkpoints as u64;
            }
            Err(err) => {
                files_failed += 1;
                errors.push(json!({
                    "session_file": path,
                    "error": err.to_string(),
                }));
            }
        }
    }

    let audit = session_sync_audit(project_root);
    let ok = files_failed == 0;
    if as_json {
        print_json(json!({
            "ok": ok,
            "files_seen": files.len(),
            "files_synced": files_synced,
            "files_failed": files_failed,
            "processed_raw_events": processed_raw_events,
            "produced_t0_events": produced_t0_events,
            "persisted_compaction_checkpoints": persisted_compaction_checkpoints,
            "sync_audit": audit,
            "errors": errors,
        }));
    } else {
        println!("files_seen: {}", files.len());
        println!("files_synced: {files_synced}");
        println!("files_failed: {files_failed}");
        println!("processed_raw_events: {processed_raw_events}");
        println!("sync_audit: {audit}");
    }
    if ok { 0 } else { 1 }
}

fn print_json(value: serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(&value).expect("json output"));
}

fn run_context_pack(
    project_root: &PathBuf,
    mode: Option<&str>,
    role: Option<String>,
    reason: Option<String>,
    detail: bool,
    active_tag: Option<String>,
    as_json: bool,
) -> i32 {
    let store = open_project_store(project_root, "standalone", "service", None)
        .ok()
        .map(|opened| opened.store);
    let request = MindContextPackRequest {
        mode: parse_mind_context_pack_mode(mode),
        profile: if detail {
            MindContextPackProfile::Expanded
        } else {
            MindContextPackProfile::Compact
        },
        active_tag,
        reason,
        role,
    };
    match compile_mind_context_pack(
        &project_root.display().to_string(),
        store.as_ref(),
        request,
        None,
    ) {
        Ok(pack) => {
            if as_json {
                print_json(json!({ "ok": true, "pack": pack }));
            } else {
                println!("{}", pack.rendered_lines.join("\n"));
            }
            0
        }
        Err(err) => {
            if as_json {
                print_json(json!({ "ok": false, "error": err }));
            } else {
                eprintln!("context pack failed: {err}");
            }
            1
        }
    }
}

fn build_runtime(
    project_root: &PathBuf,
    session_id: &str,
    pane_id: &str,
    agent_id: &str,
) -> Result<MindRuntimeCore, String> {
    let paths = MindProjectPaths::for_project_root(project_root);
    MindRuntimeCore::new(MindRuntimeConfig {
        project_root: project_root.display().to_string(),
        session_id: session_id.to_string(),
        pane_id: pane_id.to_string(),
        agent_key: agent_id.to_string(),
        store_path_override: None,
        reflector_lock_path: paths.reflector_lock_path,
        t3_lock_path: paths.t3_lock_path,
        debounce_run_ms: 250,
        t3_max_attempts: 3,
    })
}

fn run_observer_run(
    project_root: &PathBuf,
    session_id: &str,
    pane_id: &str,
    conversation_id: Option<&str>,
    agent_id: &str,
    reason: Option<String>,
    as_json: bool,
) -> i32 {
    let mut runtime = match build_runtime(project_root, session_id, pane_id, agent_id) {
        Ok(runtime) => runtime,
        Err(err) => {
            if as_json {
                print_json(json!({ "ok": false, "error": err }));
            } else {
                eprintln!("observer run failed: {err}");
            }
            return 1;
        }
    };
    let conversation_id = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .or_else(|| runtime.store().conversation_ids_for_session(session_id).ok()?.into_iter().last());
    let reason = reason.unwrap_or_else(|| "pi shortcut".to_string());
    if let Some(conversation_id) = conversation_id {
        runtime.set_latest_conversation_id(Some(conversation_id.clone()));
        let events = runtime.enqueue_observer_events(
            &conversation_id,
            MindObserverFeedTriggerKind::ManualShortcut,
            Some(reason.clone()),
        );
        if as_json {
            print_json(json!({
                "ok": true,
                "message": "observer trigger queued",
                "conversation_id": conversation_id,
                "events": events,
            }));
        } else {
            println!("observer trigger queued");
        }
        0
    } else {
        if as_json {
            print_json(json!({
                "ok": true,
                "message": "observer trigger queued",
                "conversation_id": null,
                "events": [],
            }));
        } else {
            println!("observer trigger queued");
        }
        0
    }
}

fn tick_report_json_reflector(report: &aoc_mind::ReflectorTickReport) -> serde_json::Value {
    json!({
        "file_lock_acquired": report.file_lock_acquired,
        "lease_acquired": report.lease_acquired,
        "lock_conflict": report.lock_conflict,
        "jobs_claimed": report.jobs_claimed,
        "jobs_completed": report.jobs_completed,
        "jobs_failed": report.jobs_failed,
    })
}

fn tick_report_json_t3(report: &aoc_mind::T3TickReport) -> serde_json::Value {
    json!({
        "file_lock_acquired": report.file_lock_acquired,
        "lease_acquired": report.lease_acquired,
        "lock_conflict": report.lock_conflict,
        "jobs_claimed": report.jobs_claimed,
        "jobs_completed": report.jobs_completed,
        "jobs_failed": report.jobs_failed,
        "jobs_requeued": report.jobs_requeued,
        "jobs_dead_lettered": report.jobs_dead_lettered,
    })
}

fn run_reflector_run(
    project_root: &PathBuf,
    session_id: &str,
    pane_id: &str,
    agent_id: &str,
    as_json: bool,
) -> i32 {
    let mut runtime = match build_runtime(project_root, session_id, pane_id, agent_id) {
        Ok(runtime) => runtime,
        Err(err) => {
            if as_json { print_json(json!({ "ok": false, "error": err })); } else { eprintln!("reflector run failed: {err}"); }
            return 1;
        }
    };
    match runtime.run_reflector_tick(chrono::Utc::now()) {
        Ok(report) => {
            if as_json { print_json(json!({ "ok": true, "report": tick_report_json_reflector(&report) })); }
            else { println!("reflector jobs: claimed={} completed={} failed={}", report.jobs_claimed, report.jobs_completed, report.jobs_failed); }
            0
        }
        Err(err) => {
            if as_json { print_json(json!({ "ok": false, "error": err })); } else { eprintln!("reflector run failed: {err}"); }
            1
        }
    }
}

fn run_t3_run(
    project_root: &PathBuf,
    session_id: &str,
    pane_id: &str,
    agent_id: &str,
    as_json: bool,
) -> i32 {
    let mut runtime = match build_runtime(project_root, session_id, pane_id, agent_id) {
        Ok(runtime) => runtime,
        Err(err) => {
            if as_json { print_json(json!({ "ok": false, "error": err })); } else { eprintln!("t3 run failed: {err}"); }
            return 1;
        }
    };
    match runtime.run_t3_tick(chrono::Utc::now(), |_store, _project_root, _active_tag, _now| Ok(())) {
        Ok(report) => {
            if as_json { print_json(json!({ "ok": true, "report": tick_report_json_t3(&report) })); }
            else { println!("t3 jobs: claimed={} completed={} failed={}", report.jobs_claimed, report.jobs_completed, report.jobs_failed); }
            0
        }
        Err(err) => {
            if as_json { print_json(json!({ "ok": false, "error": err })); } else { eprintln!("t3 run failed: {err}"); }
            1
        }
    }
}

fn table_counts_json(project_root: &PathBuf) -> serde_json::Value {
    let tables = [
        "raw_events",
        "compact_events_t0",
        "compaction_checkpoints",
        "compaction_slices_t0",
        "observations_t1",
        "reflections_t2",
        "project_canon_revisions",
        "ingestion_checkpoints",
        "reflector_jobs_t2",
        "t3_backlog_jobs",
        "detached_insight_jobs",
        "handshake_snapshots",
    ];
    let Ok(opened) = open_project_store(project_root, "standalone", "service", None) else {
        return json!({});
    };
    let mut map = serde_json::Map::new();
    for table in tables {
        map.insert(table.to_string(), json!(opened.store.table_count(table).unwrap_or(0)));
    }
    json!(map)
}

fn memory_scope_json(project_root: &PathBuf) -> serde_json::Value {
    let path = std::process::Command::new("aoc-mem")
        .arg("path")
        .current_dir(project_root)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.trim().to_string())
        .unwrap_or_default();
    let head = std::process::Command::new("aoc-mem")
        .arg("read")
        .current_dir(project_root)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.lines().take(3).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    let expected_prefix = project_root.join(".aoc").display().to_string();
    let path_ok = path.starts_with(&expected_prefix);
    let header_mismatch = head.contains("Project: agent-ops-cockpit")
        && project_root.file_name().and_then(|v| v.to_str()) != Some("agent-ops-cockpit");
    json!({
        "path": path,
        "path_ok": path_ok,
        "header_mismatch": header_mismatch,
        "head": head,
    })
}

fn run_doctor(project_root: &PathBuf, as_json: bool) -> i32 {
    let paths = MindProjectPaths::for_project_root(project_root);
    let counts = table_counts_json(project_root);
    let sync_audit = session_sync_audit(project_root);
    let memory = memory_scope_json(project_root);
    let lease = read_mind_service_lease(project_root).ok().flatten();
    let health = read_mind_service_health_snapshot(project_root).ok().flatten();
    let service_status = summarize_mind_service_status(
        lease.as_ref(),
        health.as_ref(),
        chrono::Utc::now().timestamp_millis(),
    );
    let counts_obj = counts.as_object();
    let t1 = counts_obj.and_then(|m| m.get("observations_t1")).and_then(|v| v.as_i64()).unwrap_or(0);
    let t2 = counts_obj.and_then(|m| m.get("reflections_t2")).and_then(|v| v.as_i64()).unwrap_or(0);
    let t3 = counts_obj.and_then(|m| m.get("project_canon_revisions")).and_then(|v| v.as_i64()).unwrap_or(0);
    let missing = sync_audit.get("missing").and_then(|v| v.as_u64()).unwrap_or(0);
    let partial = sync_audit.get("partial").and_then(|v| v.as_u64()).unwrap_or(0);
    let memory_bad = memory.get("path_ok").and_then(|v| v.as_bool()) == Some(false)
        || memory.get("header_mismatch").and_then(|v| v.as_bool()) == Some(true);
    let degraded = missing > 0 || partial > 0 || t1 == 0 || t2 == 0 || t3 == 0 || memory_bad;
    let status = if degraded { "degraded" } else { "healthy" };
    let payload = json!({
        "ok": !degraded,
        "status": status,
        "project_root": project_root,
        "store_path": paths.store_path,
        "store_exists": paths.store_path.exists(),
        "service_status": service_status,
        "sync": sync_audit,
        "counts": counts,
        "memory": memory,
        "checks": {
            "ingestion": if missing == 0 && partial == 0 { "ok" } else { "degraded" },
            "t1": if t1 > 0 { "ok" } else { "missing" },
            "t2": if t2 > 0 { "ok" } else { "missing" },
            "t3": if t3 > 0 { "ok" } else { "missing" },
            "memory": if memory_bad { "degraded" } else { "ok" },
            "service": service_status.state,
        }
    });
    if as_json {
        print_json(payload);
    } else {
        println!("Mind: {status}");
        println!("store: {}", paths.store_path.display());
        println!("sync: {sync_audit}");
        println!("counts: {counts}");
        println!("memory: {memory}");
    }
    if degraded { 1 } else { 0 }
}

fn run_provenance_query(
    project_root: &PathBuf,
    request: MindProvenanceQueryRequest,
    as_json: bool,
) -> i32 {
    let store = match open_project_store(project_root, "standalone", "service", None) {
        Ok(opened) => opened.store,
        Err(err) => {
            if as_json {
                print_json(json!({ "ok": false, "error": format!("mind store open failed: {err}") }));
            } else {
                eprintln!("provenance query failed: mind store open failed: {err}");
            }
            return 1;
        }
    };

    match compile_mind_provenance_export(&store, request) {
        Ok(export) => {
            if as_json {
                print_json(json!({ "ok": true, "export": export }));
            } else {
                println!("{}", serde_json::to_string_pretty(&export).expect("json output"));
            }
            0
        }
        Err(err) => {
            if as_json {
                print_json(json!({ "ok": false, "error": err }));
            } else {
                eprintln!("provenance query failed: {err}");
            }
            1
        }
    }
}

fn ensure_safe_export_text(payload: &str, label: &str) -> Result<(), String> {
    if text_contains_unredacted_secret(payload) {
        return Err(format!("{label} contains unredacted secret-bearing content"));
    }
    Ok(())
}

fn run_finalize_session(
    project_root: &PathBuf,
    session_id: &str,
    pane_id: &str,
    conversation_id: Option<&str>,
    reason: Option<String>,
    as_json: bool,
) -> i32 {
    let opened = match open_project_store(project_root, session_id, pane_id, None) {
        Ok(opened) => opened,
        Err(err) => {
            if as_json {
                print_json(json!({ "ok": false, "error": format!("mind store open failed: {err}") }));
            } else {
                eprintln!("finalize failed: mind store open failed: {err}");
            }
            return 1;
        }
    };
    let finalize_reason = reason.unwrap_or_else(|| "pi command".to_string());
    let latest_conversation_id = conversation_id.map(str::trim).filter(|value| !value.is_empty());
    let prepared = match prepare_session_finalize_execution(
        &opened.store,
        &project_root.display().to_string(),
        session_id,
        pane_id,
        latest_conversation_id,
        &finalize_reason,
        chrono::Utc::now(),
    ) {
        Ok(SessionFinalizePreparationOutcome::Skip(message)) => {
            if as_json {
                print_json(json!({ "ok": true, "message": message.reason, "skipped": true }));
            } else {
                println!("{}", message.reason);
            }
            return 0;
        }
        Ok(SessionFinalizePreparationOutcome::Ready(prepared)) => prepared,
        Err(message) => {
            if as_json {
                print_json(json!({ "ok": false, "error": message.reason }));
            } else {
                eprintln!("{}", message.reason);
            }
            return 1;
        }
    };

    let export_dir = PathBuf::from(&prepared.export_dir);
    if let Err(err) = std::fs::create_dir_all(&export_dir) {
        let message = aoc_mind::session_finalize_error("manifest_write", err).reason;
        if as_json {
            print_json(json!({ "ok": false, "error": message }));
        } else {
            eprintln!("{message}");
        }
        return 1;
    }

    for file in &prepared.host_plan.export_files {
        if let Err(err) = ensure_safe_export_text(&file.contents, file.safety_label) {
            if as_json {
                print_json(json!({ "ok": false, "error": format!("finalize failed: {err}") }));
            } else {
                eprintln!("finalize failed: {err}");
            }
            return 1;
        }
        if let Err(err) = std::fs::write(export_dir.join(file.file_name), &file.contents) {
            let message = aoc_mind::session_finalize_error(file.write_stage, err).reason;
            if as_json {
                print_json(json!({ "ok": false, "error": message }));
            } else {
                eprintln!("{message}");
            }
            return 1;
        }
    }

    if let Err(err) = opened.store.advance_project_watermark(
        &prepared.scope_key,
        Some(prepared.host_plan.watermark_ts),
        Some(&prepared.host_plan.watermark_artifact_id),
        chrono::Utc::now(),
    ) {
        let message = aoc_mind::session_finalize_error("watermark_write", err).reason;
        if as_json {
            print_json(json!({ "ok": false, "error": message }));
        } else {
            eprintln!("{message}");
        }
        return 1;
    }

    if as_json {
        print_json(json!({
            "ok": true,
            "message": prepared.host_plan.success.reason,
            "manifest": prepared.host_plan.manifest,
            "export_dir": prepared.export_dir,
            "t3_job_inserted": prepared.t3_job_inserted,
        }));
    } else {
        println!("{}", prepared.host_plan.success.reason);
    }
    0
}

fn run_watch_pi(
    project_root: &PathBuf,
    session_file: Option<&std::path::Path>,
    agent_id: &str,
    interval_ms: u64,
    as_json: bool,
) -> i32 {
    loop {
        let code = run_sync_pi(project_root, session_file, agent_id, false, as_json);
        if code != 0 {
            return code;
        }
        thread::sleep(Duration::from_millis(interval_ms.max(100)));
    }
}
