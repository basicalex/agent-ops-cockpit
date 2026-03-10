use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use std::{
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aoc_core::{
    pulse_ipc::{
        decode_frame, encode_frame, CommandPayload, HelloPayload, ObserverTimelinePayload,
        ProtocolVersion, SubscribePayload, WireEnvelope, WireMsg, CURRENT_PROTOCOL_VERSION,
        DEFAULT_MAX_FRAME_BYTES,
    },
    session_overseer::{ObserverSnapshot, OVERSEER_SNAPSHOT_TOPIC, OVERSEER_TIMELINE_TOPIC},
};

#[derive(Subcommand, Debug)]
pub enum OverseerCommand {
    /// Read the current overseer snapshot for a session
    Snapshot(OverseerQueryArgs),
    /// Read recent overseer timeline entries for a session
    Timeline(TimelineArgs),
    /// Send a manager-style overseer command to a worker
    Command(CommandArgs),
}

#[derive(Args, Debug)]
pub struct OverseerQueryArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
}

#[derive(Args, Debug)]
pub struct TimelineArgs {
    #[command(flatten)]
    pub query: OverseerQueryArgs,
    /// Maximum number of entries to print.
    #[arg(long, default_value_t = 12)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct CommandArgs {
    /// Session id to inspect. Falls back to AOC_SESSION_ID.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Target worker agent id.
    #[arg(long)]
    pub target_agent_id: String,
    /// Socket path override. Falls back to AOC_PULSE_SOCK or the default runtime path.
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    /// Read timeout in milliseconds.
    #[arg(long, default_value_t = 3000)]
    pub timeout_ms: u64,
    /// Print raw JSON payload.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(value_enum)]
    pub kind: OverseerCommandKindArg,
    /// Task id to switch focus to (switch-focus only).
    #[arg(long)]
    pub task_id: Option<String>,
    /// Operator/manager summary for switch-focus.
    #[arg(long)]
    pub summary: Option<String>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OverseerCommandKindArg {
    RequestStatusUpdate,
    RequestHandoff,
    PauseAndSummarize,
    RunValidation,
    SwitchFocus,
    FinalizeAndReport,
}

pub fn handle_overseer_command(command: OverseerCommand) -> Result<()> {
    match command {
        OverseerCommand::Snapshot(args) => handle_snapshot(args),
        OverseerCommand::Timeline(args) => handle_timeline(args),
        OverseerCommand::Command(args) => handle_command(args),
    }
}

fn handle_snapshot(args: OverseerQueryArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path);
    let snapshot = request_snapshot(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
    )?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }

    println!(
        "session={} workers={} timeline={} generated_at_ms={}",
        snapshot.session_id,
        snapshot.workers.len(),
        snapshot.timeline.len(),
        snapshot.generated_at_ms.unwrap_or_default()
    );
    for worker in snapshot.workers {
        let task = worker
            .assignment
            .task_id
            .or(worker.assignment.tag)
            .unwrap_or_else(|| "unassigned".to_string());
        let attention = format!("{:?}", worker.attention.level).to_ascii_lowercase();
        let drift = format!("{:?}", worker.drift_risk).to_ascii_lowercase();
        let status = format!("{:?}", worker.status).to_ascii_lowercase();
        let summary = worker.summary.unwrap_or_default();
        println!(
            "- {} pane={} status={} task={} attention={} drift={} {}",
            worker.agent_id, worker.pane_id, status, task, attention, drift, summary
        );
    }
    Ok(())
}

fn handle_timeline(args: TimelineArgs) -> Result<()> {
    let session_id = resolve_session_id(args.query.session_id)?;
    let socket_path = resolve_pulse_socket_path(&session_id, args.query.socket_path);
    let payload = request_timeline(
        &session_id,
        &socket_path,
        Duration::from_millis(args.query.timeout_ms),
    )?;
    if args.query.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!(
        "session={} timeline={} generated_at_ms={}",
        payload.session_id,
        payload.entries.len(),
        payload.generated_at_ms.unwrap_or_default()
    );
    for entry in payload.entries.into_iter().take(args.limit) {
        let kind = format!("{:?}", entry.kind).to_ascii_lowercase();
        let summary = entry.summary.unwrap_or_default();
        println!(
            "- {} {} {} {}",
            entry.emitted_at_ms.unwrap_or_default(),
            entry.agent_id,
            kind,
            summary
        );
    }
    Ok(())
}

fn handle_command(args: CommandArgs) -> Result<()> {
    let session_id = resolve_session_id(args.session_id.clone())?;
    let command_args = command_args_json(&args);
    let socket_path = resolve_pulse_socket_path(&session_id, args.socket_path.clone());
    let request_id = format!("overseer-cli-{}", now_ms());
    let command = command_name(&args.kind);
    let result = request_command_result(
        &session_id,
        &socket_path,
        Duration::from_millis(args.timeout_ms),
        &request_id,
        &args.target_agent_id,
        command,
        command_args,
    )?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    println!(
        "command={} target={} status={} code={} message={}",
        result.command,
        result.target_agent_id.unwrap_or_default(),
        result.status,
        result.error_code.unwrap_or_default(),
        result.message.unwrap_or_default()
    );
    Ok(())
}

#[derive(serde::Serialize)]
struct CommandResultView {
    command: String,
    target_agent_id: Option<String>,
    status: String,
    error_code: Option<String>,
    message: Option<String>,
}

fn request_snapshot(
    session_id: &str,
    socket_path: &PathBuf,
    timeout: Duration,
) -> Result<ObserverSnapshot> {
    let mut stream =
        connect_subscriber(session_id, socket_path, &[OVERSEER_SNAPSHOT_TOPIC], timeout)?;
    loop {
        match read_wire_envelope(&mut stream, timeout)?.msg {
            WireMsg::ObserverSnapshot(payload) => return Ok(payload),
            WireMsg::Snapshot(_) | WireMsg::Delta(_) | WireMsg::Heartbeat(_) => continue,
            other => {
                bail!("unexpected pulse message while waiting for observer snapshot: {other:?}")
            }
        }
    }
}

fn request_timeline(
    session_id: &str,
    socket_path: &PathBuf,
    timeout: Duration,
) -> Result<ObserverTimelinePayload> {
    let mut stream =
        connect_subscriber(session_id, socket_path, &[OVERSEER_TIMELINE_TOPIC], timeout)?;
    loop {
        match read_wire_envelope(&mut stream, timeout)?.msg {
            WireMsg::ObserverTimeline(payload) => return Ok(payload),
            WireMsg::Snapshot(_) | WireMsg::Delta(_) | WireMsg::Heartbeat(_) => continue,
            other => {
                bail!("unexpected pulse message while waiting for observer timeline: {other:?}")
            }
        }
    }
}

fn request_command_result(
    session_id: &str,
    socket_path: &PathBuf,
    timeout: Duration,
    request_id: &str,
    target_agent_id: &str,
    command: &str,
    args: serde_json::Value,
) -> Result<CommandResultView> {
    let mut stream = connect_subscriber(session_id, socket_path, &["command_result"], timeout)?;
    let envelope = WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: session_id.to_string(),
        sender_id: client_id(),
        timestamp: now_rfc3339(),
        request_id: Some(request_id.to_string()),
        msg: WireMsg::Command(CommandPayload {
            command: command.to_string(),
            target_agent_id: Some(target_agent_id.to_string()),
            args,
        }),
    };
    send_wire_envelope(&mut stream, &envelope)?;

    loop {
        let envelope = read_wire_envelope(&mut stream, timeout)?;
        if envelope.request_id.as_deref() != Some(request_id) {
            continue;
        }
        match envelope.msg {
            WireMsg::CommandResult(payload) => {
                return Ok(CommandResultView {
                    command: payload.command,
                    target_agent_id: Some(target_agent_id.to_string()),
                    status: payload.status,
                    error_code: payload.error.as_ref().map(|err| err.code.clone()),
                    message: payload
                        .message
                        .or_else(|| payload.error.as_ref().map(|err| err.message.clone())),
                });
            }
            other => bail!("unexpected pulse message while waiting for command result: {other:?}"),
        }
    }
}

#[cfg(unix)]
fn connect_subscriber(
    session_id: &str,
    socket_path: &PathBuf,
    topics: &[&str],
    timeout: Duration,
) -> Result<std::os::unix::net::UnixStream> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket_path).with_context(|| {
        format!(
            "failed to connect to pulse socket {}",
            socket_path.display()
        )
    })?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    let hello = WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: session_id.to_string(),
        sender_id: client_id(),
        timestamp: now_rfc3339(),
        request_id: None,
        msg: WireMsg::Hello(HelloPayload {
            client_id: client_id(),
            role: "subscriber".to_string(),
            capabilities: vec![
                "snapshot".to_string(),
                "delta".to_string(),
                "command_result".to_string(),
            ],
            agent_id: None,
            pane_id: None,
            project_root: None,
        }),
    };
    send_wire_envelope(&mut stream, &hello)?;

    let subscribe = WireEnvelope {
        version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        session_id: session_id.to_string(),
        sender_id: client_id(),
        timestamp: now_rfc3339(),
        request_id: None,
        msg: WireMsg::Subscribe(SubscribePayload {
            topics: topics.iter().map(|topic| (*topic).to_string()).collect(),
            since_seq: None,
        }),
    };
    send_wire_envelope(&mut stream, &subscribe)?;
    Ok(stream)
}

#[cfg(not(unix))]
fn connect_subscriber(
    _session_id: &str,
    _socket_path: &PathBuf,
    _topics: &[&str],
    _timeout: Duration,
) -> Result<()> {
    bail!("overseer CLI currently requires unix domain sockets")
}

#[cfg(unix)]
fn send_wire_envelope(
    stream: &mut std::os::unix::net::UnixStream,
    envelope: &WireEnvelope,
) -> Result<()> {
    let frame = encode_frame(envelope, DEFAULT_MAX_FRAME_BYTES)?;
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

#[cfg(unix)]
fn read_wire_envelope(
    stream: &mut std::os::unix::net::UnixStream,
    timeout: Duration,
) -> Result<WireEnvelope> {
    stream.set_read_timeout(Some(timeout))?;
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0u8; 8192];
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            bail!("pulse socket closed before response arrived");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            let frame = buffer.drain(..=newline).collect::<Vec<u8>>();
            return decode_frame::<WireEnvelope>(&frame, DEFAULT_MAX_FRAME_BYTES)
                .map_err(|err| anyhow!(err.to_string()));
        }
    }
}

fn command_name(kind: &OverseerCommandKindArg) -> &'static str {
    match kind {
        OverseerCommandKindArg::RequestStatusUpdate => "request_status_update",
        OverseerCommandKindArg::RequestHandoff => "request_handoff",
        OverseerCommandKindArg::PauseAndSummarize => "pause_and_summarize",
        OverseerCommandKindArg::RunValidation => "run_validation",
        OverseerCommandKindArg::SwitchFocus => "switch_focus",
        OverseerCommandKindArg::FinalizeAndReport => "finalize_and_report",
    }
}

fn command_args_json(args: &CommandArgs) -> serde_json::Value {
    match args.kind {
        OverseerCommandKindArg::SwitchFocus => serde_json::json!({
            "task_id": args.task_id,
            "summary": args.summary,
        }),
        _ => serde_json::json!({}),
    }
}

fn resolve_session_id(value: Option<String>) -> Result<String> {
    if let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(value);
    }
    if let Ok(value) = std::env::var("AOC_SESSION_ID") {
        let value = value.trim();
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }
    bail!("missing session id; pass --session-id or set AOC_SESSION_ID")
}

fn resolve_pulse_socket_path(session_id: &str, override_path: Option<PathBuf>) -> PathBuf {
    if let Some(path) = override_path {
        return path;
    }
    if let Ok(value) = std::env::var("AOC_PULSE_SOCK") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    let runtime_dir = if let Ok(value) = std::env::var("XDG_RUNTIME_DIR") {
        if !value.trim().is_empty() {
            PathBuf::from(value)
        } else {
            PathBuf::from("/tmp")
        }
    } else if let Ok(uid) = std::env::var("UID") {
        PathBuf::from(format!("/run/user/{uid}"))
    } else {
        PathBuf::from("/tmp")
    };
    runtime_dir
        .join("aoc")
        .join(session_slug(session_id))
        .join("pulse.sock")
}

fn session_slug(session_id: &str) -> String {
    let mut slug = String::with_capacity(session_id.len());
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            slug.push(ch);
        } else {
            slug.push('-');
        }
    }
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-').to_string();
    let base = if slug.is_empty() {
        "session".to_string()
    } else {
        slug
    };
    let hash = stable_hash_hex(session_id);
    let short = if base.len() > 48 {
        &base[..48]
    } else {
        base.as_str()
    };
    format!("{short}-{hash}")
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in input.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("{hash:08x}")
}

fn client_id() -> String {
    format!("aoc-overseer-cli-{}", std::process::id())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}
