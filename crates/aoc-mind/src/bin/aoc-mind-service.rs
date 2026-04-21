use aoc_core::{
    mind_contracts::text_contains_unredacted_secret,
    mind_observer_feed::MindObserverFeedTriggerKind,
};
use aoc_mind::{
    compile_mind_context_pack, discover_latest_pi_session_file,
    mind_progress_for_conversation, open_project_store, prepare_session_finalize_export_location,
    prepare_session_finalize_host_plan, prepare_session_finalize_plan,
    read_mind_service_health_snapshot, read_mind_service_lease,
    sync_latest_pi_session_into_project_store, sync_session_file_into_project_store,
    DistillationConfig, MindContextPackMode, MindContextPackProfile, MindContextPackRequest,
    MindProjectPaths, MindRuntimeConfig, MindRuntimeCore, SessionFinalizePlanOutcome,
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
            json,
        } => run_sync_pi(&project_root, session_file.as_deref(), &agent_id, json),
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
    }
    0
}

fn run_sync_pi(
    project_root: &PathBuf,
    session_file: Option<&std::path::Path>,
    agent_id: &str,
    as_json: bool,
) -> i32 {
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

fn parse_context_pack_mode(value: Option<&str>) -> MindContextPackMode {
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
        mode: parse_context_pack_mode(mode),
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
    let scope_key = format!("session:{}:pane:{}", session_id, pane_id);
    let latest_conversation_id = conversation_id.map(str::trim).filter(|value| !value.is_empty());
    let plan = match prepare_session_finalize_plan(
        &opened.store,
        session_id,
        pane_id,
        latest_conversation_id,
        &scope_key,
    ) {
        Ok(SessionFinalizePlanOutcome::Skip { outcome_reason_suffix, .. }) => {
            let message = format!("{}: {}", finalize_reason, outcome_reason_suffix);
            if as_json {
                print_json(json!({ "ok": true, "message": message, "skipped": true }));
            } else {
                println!("{message}");
            }
            return 0;
        }
        Ok(SessionFinalizePlanOutcome::Ready(plan)) => plan,
        Err(err) => {
            let message = format!("{}", aoc_mind::session_finalize_error("planning", err).reason);
            if as_json {
                print_json(json!({ "ok": false, "error": message }));
            } else {
                eprintln!("{message}");
            }
            return 1;
        }
    };

    let export_location = prepare_session_finalize_export_location(
        &project_root.display().to_string(),
        session_id,
        &plan,
    );
    let export_dir = PathBuf::from(&export_location.export_dir);
    if let Err(err) = std::fs::create_dir_all(&export_dir) {
        let message = aoc_mind::session_finalize_error("manifest_write", err).reason;
        if as_json {
            print_json(json!({ "ok": false, "error": message }));
        } else {
            eprintln!("{message}");
        }
        return 1;
    }

    let (t3_job_id, t3_job_inserted) = match opened.store.enqueue_t3_backlog_job(
        &project_root.display().to_string(),
        session_id,
        pane_id,
        plan.active_tag.as_deref(),
        Some(&plan.slice_start_id),
        Some(&plan.slice_end_id),
        &plan.artifact_ids,
        chrono::Utc::now(),
    ) {
        Ok(result) => result,
        Err(err) => {
            let message = aoc_mind::session_finalize_error("t3_enqueue", err).reason;
            if as_json {
                print_json(json!({ "ok": false, "error": message }));
            } else {
                eprintln!("{message}");
            }
            return 1;
        }
    };

    let host_plan = match prepare_session_finalize_host_plan(
        &plan,
        session_id,
        pane_id,
        &project_root.display().to_string(),
        &export_location.export_dir,
        &t3_job_id,
        t3_job_inserted,
        &finalize_reason,
    ) {
        Ok(host_plan) => host_plan,
        Err(err) => {
            let message = aoc_mind::session_finalize_error("export_bundle", err).reason;
            if as_json {
                print_json(json!({ "ok": false, "error": message }));
            } else {
                eprintln!("{message}");
            }
            return 1;
        }
    };

    for file in &host_plan.export_files {
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
        &scope_key,
        Some(host_plan.watermark_ts),
        Some(&host_plan.watermark_artifact_id),
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
            "message": host_plan.success.reason,
            "manifest": host_plan.manifest,
            "export_dir": export_location.export_dir,
            "t3_job_inserted": t3_job_inserted,
        }));
    } else {
        println!("{}", host_plan.success.reason);
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
        let code = run_sync_pi(project_root, session_file, agent_id, as_json);
        if code != 0 {
            return code;
        }
        thread::sleep(Duration::from_millis(interval_ms.max(100)));
    }
}
