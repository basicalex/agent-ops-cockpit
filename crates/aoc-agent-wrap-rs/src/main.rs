use aoc_core::{ProjectData, Task, TaskStatus};
use chrono::Utc;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	env,
	fs::OpenOptions,
	io::{self, IsTerminal, Read, Write},
	path::{Path, PathBuf},
	process::Stdio,
	sync::{Arc, Mutex as StdMutex},
	time::Duration,
};
use tokio::{
	fs,
	io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
	process::Command,
	sync::{mpsc, oneshot, Mutex},
};
use tokio_tungstenite::connect_async;
use tracing::{error, warn};
use tracing_subscriber::{fmt::writer::BoxMakeWriter, EnvFilter};
use url::Url;

const PROTOCOL_VERSION: &str = "1";
const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_FILES_LIST: usize = 500;
const TASK_DEBOUNCE_MS: u64 = 500;
const DIFF_INTERVAL_SECS: u64 = 2;
const STATUS_THROTTLE_MS: u64 = 500;
const DISABLE_MOUSE_SEQ: &str = "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1007l\x1b[?1004l";

#[derive(Parser, Debug)]
#[command(name = "aoc-agent-wrap-rs")]
struct Args {
	#[arg(long, default_value = "")]
	session: String,
	#[arg(long, default_value = "")]
	pane_id: String,
	#[arg(long, default_value = "")]
	agent_id: String,
	#[arg(long, default_value = "")]
	project_root: String,
	#[arg(long, default_value = "")]
	hub_addr: String,
	#[arg(long, default_value = "")]
	hub_url: String,
	#[arg(long, default_value = "")]
	log_dir: String,
	#[arg(long, default_value_t = 10)]
	heartbeat_interval: u64,
	#[arg(last = true)]
	cmd: Vec<String>,
}

#[derive(Clone, Debug)]
struct ClientConfig {
	session_id: String,
	agent_id: String,
	pane_id: String,
	project_root: String,
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
	version: String,
	#[serde(rename = "type")]
	r#type: String,
	session_id: String,
	sender_id: String,
	timestamp: String,
	payload: T,
	#[serde(skip_serializing_if = "Option::is_none")]
	request_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct HelloPayload {
	client_id: String,
	role: String,
	capabilities: Vec<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	agent_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pane_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	project_root: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AgentStatusPayload {
	agent_id: String,
	status: String,
	pane_id: String,
	project_root: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	cwd: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	message: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct HeartbeatPayload {
	agent_id: String,
	pid: i32,
	cwd: String,
	last_update: String,
}

#[derive(Serialize, Deserialize)]
struct DiffPatchRequestPayload {
	agent_id: String,
	path: String,
	#[serde(default)]
	context_lines: Option<i32>,
	#[serde(default)]
	include_untracked: Option<bool>,
	#[serde(default)]
	request_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PayloadError {
	code: String,
	message: String,
}

#[derive(Serialize, Deserialize)]
struct DiffPatchResponsePayload {
	agent_id: String,
	path: String,
	status: String,
	is_binary: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	patch: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<PayloadError>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct DiffCounts {
	files: u32,
	additions: u32,
	deletions: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct UntrackedCounts {
	files: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct DiffSummaryCounts {
	staged: DiffCounts,
	unstaged: DiffCounts,
	untracked: UntrackedCounts,
}

#[derive(Serialize, Deserialize, Clone)]
struct DiffFile {
	path: String,
	status: String,
	additions: u32,
	deletions: u32,
	staged: bool,
	untracked: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct DiffSummaryPayload {
	agent_id: String,
	repo_root: String,
	git_available: bool,
	summary: DiffSummaryCounts,
	files: Vec<DiffFile>,
	#[serde(skip_serializing_if = "Option::is_none")]
	reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct TaskCounts {
	total: u32,
	pending: u32,
	in_progress: u32,
	done: u32,
	blocked: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct ActiveTask {
	id: String,
	title: String,
	status: String,
	priority: String,
	active_agent: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct TaskSummaryPayload {
	agent_id: String,
	tag: String,
	counts: TaskCounts,
	#[serde(skip_serializing_if = "Option::is_none")]
	active_tasks: Option<Vec<ActiveTask>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<PayloadError>,
}

#[derive(Clone)]
struct RuntimeConfig {
	client: ClientConfig,
	hub_url: Url,
	heartbeat_interval: Duration,
	cmd: Vec<String>,
	log_dir: String,
	log_stdout: bool,
}

#[derive(Default)]
struct CachedMessages {
	status: Option<String>,
	diff_summary: Option<String>,
	task_summary: HashMap<String, String>,
}

struct LogGuard {
	file: Option<Arc<StdMutex<std::fs::File>>>,
}

struct MultiWriter {
	stdout_enabled: bool,
	file: Option<Arc<StdMutex<std::fs::File>>>,
}

enum GitError {
	Missing,
	NotRepo,
	Error(String),
}

enum TaskError {
	Missing,
	Malformed(String),
	Io(String),
}

struct GitStatusEntry {
	path: String,
	status: String,
	staged: bool,
	unstaged: bool,
	untracked: bool,
}

#[tokio::main]
async fn main() {
	let args = Args::parse();
	let config = load_config(args);
	let _log_guard = init_logging(&config);

	if config.cmd.is_empty() {
		error!("missing command to wrap");
		std::process::exit(1);
	}

	let (tx, rx) = mpsc::channel::<String>(256);
	let cache = Arc::new(Mutex::new(CachedMessages::default()));
	let hub_cfg = config.client.clone();
	let hub_url = config.hub_url.clone();
	let cache_clone = cache.clone();
	let mut hub_rx = rx;
	let hub_task = tokio::spawn(async move {
		hub_loop(hub_cfg, hub_url, &mut hub_rx, cache_clone).await;
	});

	let online = build_agent_status(&config.client, "running", None);
	{
		let mut cached = cache.lock().await;
		cached.status = Some(online.clone());
	}
	let _ = tx.send(online).await;

	let heartbeat_cfg = config.clone();
	let heartbeat_tx = tx.clone();
	let heartbeat_task = tokio::spawn(async move {
		let mut ticker = tokio::time::interval(heartbeat_cfg.heartbeat_interval);
		loop {
			ticker.tick().await;
			let msg = build_heartbeat(&heartbeat_cfg.client);
			if heartbeat_tx.send(msg).await.is_err() {
				break;
			}
		}
	});

	let task_cfg = config.client.clone();
	let task_tx = tx.clone();
	let task_cache = cache.clone();
	let task_task = tokio::spawn(async move {
		task_summary_loop(task_cfg, task_tx, task_cache).await;
	});

	let diff_cfg = config.client.clone();
	let diff_tx = tx.clone();
	let diff_cache = cache.clone();
	let diff_task = tokio::spawn(async move {
		diff_summary_loop(diff_cfg, diff_tx, diff_cache).await;
	});

	let (line_tx, line_rx) = mpsc::channel::<String>(512);
	let status_cfg = config.client.clone();
	let status_tx = tx.clone();
	let status_cache = cache.clone();
	let status_task = tokio::spawn(async move {
		status_message_loop(status_cfg, status_tx, status_cache, line_rx).await;
	});

	let use_pty = resolve_use_pty();
	let exit_code = if use_pty {
		match run_child_pty(&config.cmd, line_tx.clone()).await {
			Ok(code) => code,
			Err(err) => {
				warn!("pty_spawn_failed: {err}; falling back to pipes");
				run_child_piped(&config.cmd, line_tx.clone()).await
			}
		}
	} else {
		run_child_piped(&config.cmd, line_tx.clone()).await
	};

	drop(line_tx);

	let _ = status_task.await;

	let offline = build_agent_status(&config.client, "offline", Some("exit"));
	{
		let mut cached = cache.lock().await;
		cached.status = Some(offline.clone());
	}
	let _ = tx.send(offline).await;
	drop(tx);
	let _ = heartbeat_task.await;
	let _ = task_task.await;
	let _ = diff_task.await;
	let _ = hub_task.await;
	std::process::exit(exit_code);
}

fn load_config(args: Args) -> RuntimeConfig {
	let session_id = if !args.session.trim().is_empty() {
		args.session
	} else {
		resolve_session_id()
	};
	let project_root = resolve_project_root(&args.project_root);
	let agent_id = resolve_agent_id(&args.agent_id, &project_root);
	let pane_id = resolve_pane_id(&args.pane_id);
	let hub_url = resolve_hub_url(&args.hub_url, &args.hub_addr, &session_id);
	let log_dir = resolve_log_dir(&args.log_dir);
	let log_stdout = resolve_log_stdout();
	RuntimeConfig {
		client: ClientConfig {
			session_id,
			agent_id,
			pane_id,
			project_root,
		},
		hub_url,
		heartbeat_interval: Duration::from_secs(args.heartbeat_interval),
		cmd: args.cmd,
		log_dir,
		log_stdout,
	}
}

async fn hub_loop(
	cfg: ClientConfig,
	hub_url: Url,
	rx: &mut mpsc::Receiver<String>,
	cache: Arc<Mutex<CachedMessages>>,
) {
	let mut backoff = Duration::from_secs(1);
	loop {
		let connect = connect_async(hub_url.clone()).await;
		let (mut ws, _) = match connect {
			Ok(value) => value,
			Err(err) => {
				warn!("hub_connect_error: {err}");
				tokio::time::sleep(backoff).await;
				backoff = next_backoff(backoff);
				continue;
			}
		};
		backoff = Duration::from_secs(1);

		let hello = build_hello(&cfg);
		if ws.send(tokio_tungstenite::tungstenite::Message::Text(hello)).await.is_err() {
			warn!("hub_hello_error");
			let _ = ws.close(None).await;
			continue;
		}

		let (cached_status, cached_diff, cached_tasks) = {
			let cache = cache.lock().await;
			(
				cache.status.clone(),
				cache.diff_summary.clone(),
				cache.task_summary.clone(),
			)
		};
		if let Some(status) = cached_status {
			let _ = ws.send(tokio_tungstenite::tungstenite::Message::Text(status)).await;
		}
		if let Some(diff) = cached_diff {
			let _ = ws.send(tokio_tungstenite::tungstenite::Message::Text(diff)).await;
		}
		if !cached_tasks.is_empty() {
			let mut tags: Vec<_> = cached_tasks.keys().cloned().collect();
			tags.sort();
			for tag in tags {
				if let Some(msg) = cached_tasks.get(&tag) {
					let _ = ws
						.send(tokio_tungstenite::tungstenite::Message::Text(msg.clone()))
						.await;
				}
			}
		}

		loop {
			tokio::select! {
				Some(msg) = ws.next() => {
					match msg {
						Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
							if let Some(response) = handle_incoming(&cfg, &text).await {
								let _ = ws.send(tokio_tungstenite::tungstenite::Message::Text(response)).await;
							}
						}
						Ok(_) => {}
						Err(_) => break,
					}
				}
				Some(out) = rx.recv() => {
					if ws.send(tokio_tungstenite::tungstenite::Message::Text(out)).await.is_err() {
						break;
					}
				}
				else => break,
			}
		}
		let _ = ws.close(None).await;
	}
}

fn build_hello(cfg: &ClientConfig) -> String {
	let payload = HelloPayload {
		client_id: cfg.agent_id.clone(),
		role: "publisher".to_string(),
		capabilities: vec![
			"agent_status".to_string(),
			"diff_summary".to_string(),
			"diff_patch_response".to_string(),
			"task_summary".to_string(),
			"task_update".to_string(),
			"heartbeat".to_string(),
			"error".to_string(),
		],
		agent_id: Some(cfg.agent_id.clone()),
		pane_id: Some(cfg.pane_id.clone()),
		project_root: Some(cfg.project_root.clone()),
	};
	build_envelope("hello", &cfg.session_id, &cfg.agent_id, payload, None)
}

fn build_agent_status(cfg: &ClientConfig, status: &str, message: Option<&str>) -> String {
	let payload = AgentStatusPayload {
		agent_id: cfg.agent_id.clone(),
		status: status.to_string(),
		pane_id: cfg.pane_id.clone(),
		project_root: cfg.project_root.clone(),
		cwd: env::current_dir().ok().map(|p| p.to_string_lossy().to_string()),
		message: message.map(|m| m.to_string()),
	};
	build_envelope("agent_status", &cfg.session_id, &cfg.agent_id, payload, None)
}

fn build_heartbeat(cfg: &ClientConfig) -> String {
	let payload = HeartbeatPayload {
		agent_id: cfg.agent_id.clone(),
		pid: std::process::id() as i32,
		cwd: env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
		last_update: Utc::now().to_rfc3339(),
	};
	build_envelope("heartbeat", &cfg.session_id, &cfg.agent_id, payload, None)
}

async fn handle_incoming(cfg: &ClientConfig, text: &str) -> Option<String> {
	let envelope: Envelope<serde_json::Value> = serde_json::from_str(text).ok()?;
	if envelope.session_id != cfg.session_id {
		return None;
	}
	if envelope.r#type != "diff_patch_request" {
		return None;
	}
	let payload: DiffPatchRequestPayload = serde_json::from_value(envelope.payload).ok()?;
	if payload.agent_id != cfg.agent_id {
		return None;
	}
	let request_id = payload
		.request_id
		.as_deref()
		.or(envelope.request_id.as_deref());
	let response = build_diff_patch_response(cfg, &payload).await;
	Some(build_envelope(
		"diff_patch_response",
		&cfg.session_id,
		&cfg.agent_id,
		response,
		request_id,
	))
}

fn build_envelope<T: Serialize>(
	kind: &str,
	session_id: &str,
	sender_id: &str,
	payload: T,
	request_id: Option<&str>,
) -> String {
	let envelope = Envelope {
		version: PROTOCOL_VERSION.to_string(),
		r#type: kind.to_string(),
		session_id: session_id.to_string(),
		sender_id: sender_id.to_string(),
		timestamp: Utc::now().to_rfc3339(),
		payload,
		request_id: request_id.map(|id| id.to_string()),
	};
	serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string())
}

fn resolve_use_pty() -> bool {
	if let Ok(value) = env::var("AOC_AGENT_PTY") {
		if let Some(parsed) = parse_bool_env(&value) {
			return parsed;
		}
	}
	if let Ok(value) = env::var("AOC_PTY") {
		if let Some(parsed) = parse_bool_env(&value) {
			return parsed;
		}
	}
	io::stdout().is_terminal()
}

fn parse_bool_env(value: &str) -> Option<bool> {
	match value.trim() {
		"1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON" => Some(true),
		"0" | "false" | "FALSE" | "no" | "NO" | "off" | "OFF" => Some(false),
		_ => None,
	}
}

fn resolve_filter_mouse() -> bool {
	if let Ok(value) = env::var("AOC_AGENT_FILTER_MOUSE") {
		if let Some(parsed) = parse_bool_env(&value) {
			return parsed;
		}
	}
	true
}

fn disable_mouse_reporting_stdout() {
	let mut stdout = io::stdout();
	let _ = stdout.write_all(DISABLE_MOUSE_SEQ.as_bytes());
	let _ = stdout.flush();
}

fn filter_mouse_input(input: &[u8], carry: &mut Vec<u8>) -> (Vec<u8>, bool) {
	let mut data = Vec::with_capacity(carry.len() + input.len());
	if !carry.is_empty() {
		data.extend_from_slice(carry);
		carry.clear();
	}
	data.extend_from_slice(input);

	let mut output = Vec::with_capacity(data.len());
	let mut dropped = false;
	let mut i = 0;
	while i < data.len() {
		if data[i] == 0x1b {
			if i + 1 >= data.len() {
				carry.extend_from_slice(&data[i..]);
				break;
			}
			if data[i + 1] != b'[' {
				output.push(data[i]);
				i += 1;
				continue;
			}
			if i + 2 >= data.len() {
				carry.extend_from_slice(&data[i..]);
				break;
			}
			if data[i + 2] != b'<' {
				output.push(data[i]);
				i += 1;
				continue;
			}
			let mut j = i + 3;
			let mut found = false;
			while j < data.len() {
				let byte = data[j];
				if byte == b'M' || byte == b'm' {
					found = true;
					j += 1;
					break;
				}
				j += 1;
			}
			if found {
				dropped = true;
				i = j;
				continue;
			}
			carry.extend_from_slice(&data[i..]);
			break;
		}
		if data[i] == 0x1b && i + 2 < data.len() && data[i + 1] == b'[' && data[i + 2] == b'M' {
			if i + 5 <= data.len() {
				dropped = true;
				i += 5;
				continue;
			}
			carry.extend_from_slice(&data[i..]);
			break;
		}
		output.push(data[i]);
		i += 1;
	}

	(output, dropped)
}

fn filter_mouse_output(input: &[u8], carry: &mut Vec<u8>) -> (Vec<u8>, bool) {
	let mut data = Vec::with_capacity(carry.len() + input.len());
	if !carry.is_empty() {
		data.extend_from_slice(carry);
		carry.clear();
	}
	data.extend_from_slice(input);

	let mut output = Vec::with_capacity(data.len());
	let mut dropped = false;
	let mut i = 0;
	while i < data.len() {
		if data[i] == 0x1b && i + 2 < data.len() && data[i + 1] == b'[' && data[i + 2] == b'?' {
			let mut j = i + 3;
			let mut numbers: Vec<u32> = Vec::new();
			let mut current: u32 = 0;
			let mut has_digit = false;
			let mut found = false;
			let mut terminator = 0u8;
			while j < data.len() {
				let byte = data[j];
				if byte.is_ascii_digit() {
					has_digit = true;
					current = current.saturating_mul(10).saturating_add((byte - b'0') as u32);
					j += 1;
					continue;
				}
				if byte == b';' {
					if has_digit {
						numbers.push(current);
						current = 0;
						has_digit = false;
					}
					j += 1;
					continue;
				}
				if byte == b'h' || byte == b'l' {
					if has_digit {
						numbers.push(current);
					}
					terminator = byte;
					found = true;
					j += 1;
					break;
				}
				break;
			}
			if found {
				let mut is_mouse = false;
				for num in &numbers {
					if matches!(
						num,
						1000 | 1002 | 1003 | 1004 | 1005 | 1006 | 1007 | 1015
					) {
						is_mouse = true;
						break;
					}
				}
				if is_mouse {
					dropped = true;
					i = j;
					continue;
				}
			}
			if !found {
				carry.extend_from_slice(&data[i..]);
				break;
			}
		}
		output.push(data[i]);
		i += 1;
	}

	(output, dropped)
}

fn resolve_pty_size() -> PtySize {
	let cols = env::var("COLUMNS")
		.ok()
		.and_then(|value| value.parse::<u16>().ok())
		.unwrap_or(80);
	let rows = env::var("LINES")
		.ok()
		.and_then(|value| value.parse::<u16>().ok())
		.unwrap_or(24);
	PtySize {
		rows: rows.max(1),
		cols: cols.max(1),
		pixel_width: 0,
		pixel_height: 0,
	}
}

async fn run_child_piped(cmd: &[String], line_tx: mpsc::Sender<String>) -> i32 {
	let mut child = Command::new(&cmd[0]);
	child
		.args(&cmd[1..])
		.stdin(Stdio::inherit())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	let mut child = match child.spawn() {
		Ok(proc) => proc,
		Err(err) => {
			error!("failed to spawn child: {err}");
			return 1;
		}
	};

	let stdout_task = if let Some(stdout) = child.stdout.take() {
		let line_tx = line_tx.clone();
		Some(tokio::spawn(async move {
			forward_stream(stdout, tokio::io::stdout(), line_tx).await;
		}))
	} else {
		None
	};

	let stderr_task = if let Some(stderr) = child.stderr.take() {
		let line_tx = line_tx.clone();
		Some(tokio::spawn(async move {
			forward_stream(stderr, tokio::io::stderr(), line_tx).await;
		}))
	} else {
		None
	};

	drop(line_tx);

	let exit_code = tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			let _ = child.kill().await;
			1
		}
		status = child.wait() => {
			match status {
				Ok(status) => status.code().unwrap_or(0),
				Err(_) => 1,
			}
		}
	};

	if let Some(task) = stdout_task {
		let _ = task.await;
	}
	if let Some(task) = stderr_task {
		let _ = task.await;
	}

	exit_code
}

async fn run_child_pty(cmd: &[String], line_tx: mpsc::Sender<String>) -> io::Result<i32> {
	let pty_system = native_pty_system();
	let pair = pty_system
		.openpty(resolve_pty_size())
		.map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;

	let mut builder = CommandBuilder::new(&cmd[0]);
	builder.args(&cmd[1..]);
	if env::var("TERM").is_err() {
		builder.env("TERM", "xterm-256color");
	}

	let mut child = pair
		.slave
		.spawn_command(builder)
		.map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
	drop(pair.slave);

	let mut reader = pair
		.master
		.try_clone_reader()
		.map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
	let writer = Arc::new(StdMutex::new(
		pair
			.master
			.take_writer()
			.map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?,
	));

	let filter_mouse = resolve_filter_mouse();
	if filter_mouse {
		disable_mouse_reporting_stdout();
	}
	let writer_clone = writer.clone();
	let stdin_task = tokio::task::spawn_blocking(move || {
		let mut stdin = io::stdin();
		let mut buffer = [0u8; 4096];
		let mut mouse_carry: Vec<u8> = Vec::new();
		loop {
			let read = match stdin.read(&mut buffer) {
				Ok(0) => break,
				Ok(count) => count,
				Err(_) => break,
			};
			let payload: Vec<u8>;
			let mut dropped_mouse = false;
			let slice = &buffer[..read];
			let out = if filter_mouse {
				let (filtered, dropped) = filter_mouse_input(slice, &mut mouse_carry);
				payload = filtered;
				dropped_mouse = dropped;
				payload.as_slice()
			} else {
				slice
			};
			if out.is_empty() {
				continue;
			}
			if dropped_mouse {
				disable_mouse_reporting_stdout();
			}
			if let Ok(mut writer) = writer_clone.lock() {
				let _ = writer.write_all(out);
				let _ = writer.flush();
			}
		}
	});

	let line_sender = line_tx.clone();
	let stdout_task = tokio::task::spawn_blocking(move || {
		let mut stdout = io::stdout();
		let mut buffer = [0u8; 8192];
		let mut line_buf: Vec<u8> = Vec::new();
		let mut mouse_carry: Vec<u8> = Vec::new();
		loop {
			let read = match reader.read(&mut buffer) {
				Ok(0) => break,
				Ok(count) => count,
				Err(_) => break,
			};
			let mut dropped_mouse = false;
			let (filtered, dropped) = if filter_mouse {
				let (filtered, dropped) = filter_mouse_output(&buffer[..read], &mut mouse_carry);
				(filtered, dropped)
			} else {
				(buffer[..read].to_vec(), false)
			};
			dropped_mouse = dropped;
			if dropped_mouse {
				disable_mouse_reporting_stdout();
			}
			if filtered.is_empty() {
				continue;
			}
			let _ = stdout.write_all(&filtered);
			let _ = stdout.flush();
			for byte in &filtered {
				if *byte == b'\n' {
					if !line_buf.is_empty() {
						let line = String::from_utf8_lossy(&line_buf).to_string();
						let line = line.trim_end_matches('\r').to_string();
						if !line.trim().is_empty() {
							let _ = line_sender.blocking_send(line);
						}
						line_buf.clear();
					} else {
						line_buf.clear();
					}
				} else {
					line_buf.push(*byte);
				}
			}
		}
		if !line_buf.is_empty() {
			let line = String::from_utf8_lossy(&line_buf).to_string();
			let line = line.trim_end_matches('\r').to_string();
			if !line.trim().is_empty() {
				let _ = line_sender.blocking_send(line);
			}
		}
	});

	let (exit_tx, mut exit_rx) = oneshot::channel::<i32>();
	std::thread::spawn(move || {
		let code = match child.wait() {
			Ok(status) => status.exit_code() as i32,
			Err(_) => 1,
		};
		let _ = exit_tx.send(code);
	});

	let exit_code = tokio::select! {
		status = &mut exit_rx => status.unwrap_or(1),
		_ = tokio::signal::ctrl_c() => {
			send_ctrl_c(&writer);
			exit_rx.await.unwrap_or(1)
		}
	};

	let _ = stdout_task.await;
	stdin_task.abort();

	Ok(exit_code)
}

fn send_ctrl_c(writer: &Arc<StdMutex<Box<dyn Write + Send>>>) {
	if let Ok(mut writer) = writer.lock() {
		let _ = writer.write_all(&[0x03]);
		let _ = writer.flush();
	}
}

async fn forward_stream<R, W>(reader: R, mut writer: W, line_tx: mpsc::Sender<String>)
where
	R: tokio::io::AsyncRead + Unpin,
	W: tokio::io::AsyncWrite + Unpin,
{
	let mut lines = BufReader::new(reader).lines();
	while let Ok(Some(line)) = lines.next_line().await {
		if writer.write_all(line.as_bytes()).await.is_err() {
			break;
		}
		if writer.write_all(b"\n").await.is_err() {
			break;
		}
		let _ = writer.flush().await;
		let _ = line_tx.try_send(line);
	}
}

async fn status_message_loop(
	cfg: ClientConfig,
	tx: mpsc::Sender<String>,
	cache: Arc<Mutex<CachedMessages>>,
	mut line_rx: mpsc::Receiver<String>,
) {
	let mut pending: Option<String> = None;
	let mut last_sent = String::new();
	let mut closed = false;
	let mut ticker = tokio::time::interval(Duration::from_millis(STATUS_THROTTLE_MS));
	loop {
		tokio::select! {
			maybe_line = line_rx.recv() => {
				match maybe_line {
					Some(line) => {
						if !line.trim().is_empty() {
							pending = Some(line);
						}
					}
					None => {
						closed = true;
						if pending.is_none() {
							break;
						}
					}
				}
			}
			_ = ticker.tick() => {
				if let Some(line) = pending.take() {
					if line != last_sent {
						let _ = send_status_update(&cfg, &tx, &cache, "running", Some(&line)).await;
						last_sent = line;
					}
				} else if closed {
					break;
				}
			}
		}
	}
}

async fn send_status_update(
	cfg: &ClientConfig,
	tx: &mpsc::Sender<String>,
	cache: &Arc<Mutex<CachedMessages>>,
	status: &str,
	message: Option<&str>,
) -> Result<(), ()> {
	let msg = build_agent_status(cfg, status, message);
	let mut cached = cache.lock().await;
	if cached.status.as_ref() == Some(&msg) {
		return Ok(());
	}
	cached.status = Some(msg.clone());
	drop(cached);
	let _ = tx.send(msg).await;
	Ok(())
}

async fn task_summary_loop(cfg: ClientConfig, tx: mpsc::Sender<String>, cache: Arc<Mutex<CachedMessages>>) {
	let tasks_path = tasks_file_path(&cfg.project_root);
	let state_path = state_file_path(&cfg.project_root);
	let (event_tx, mut event_rx) = mpsc::unbounded_channel::<()>();
	let mut watcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
		if res.is_ok() {
			let _ = event_tx.send(());
		}
	}) {
		Ok(watcher) => watcher,
		Err(err) => {
			warn!("task_watch_failed: {err}");
			return;
		}
	};
	watch_path(&mut watcher, &tasks_path);
	watch_path(&mut watcher, &state_path);
	let _ = send_task_summaries(&cfg, &tx, &cache).await;
	let mut pending = false;
	let debounce = Duration::from_millis(TASK_DEBOUNCE_MS);
	loop {
		tokio::select! {
			_ = tx.closed() => break,
			Some(_) = event_rx.recv() => {
				pending = true;
			}
			_ = tokio::time::sleep(debounce), if pending => {
				pending = false;
				let _ = send_task_summaries(&cfg, &tx, &cache).await;
			}
		}
	}
}

async fn send_task_summaries(
	cfg: &ClientConfig,
	tx: &mpsc::Sender<String>,
	cache: &Arc<Mutex<CachedMessages>>,
) -> Result<(), ()> {
	let tasks_path = tasks_file_path(&cfg.project_root);
	match load_tasks(&tasks_path).await {
		Ok(data) => {
			let mut tags: Vec<_> = data.tags.into_iter().collect();
			tags.sort_by(|a, b| a.0.cmp(&b.0));
			let mut messages: HashMap<String, String> = HashMap::new();
			for (tag, ctx) in tags {
				let payload = build_task_summary_payload(cfg, &tag, &ctx.tasks, None);
				let msg = build_envelope("task_summary", &cfg.session_id, &cfg.agent_id, payload, None);
				messages.insert(tag, msg);
			}
			let mut cache = cache.lock().await;
			let mut to_send = Vec::new();
			for (tag, msg) in &messages {
				if cache.task_summary.get(tag).map(|value| value.as_str()) != Some(msg) {
					to_send.push(msg.clone());
				}
			}
			cache.task_summary = messages;
			drop(cache);
			for msg in to_send {
				let _ = tx.send(msg).await;
			}
			Ok(())
		}
		Err(err) => {
			let (code, message) = match err {
				TaskError::Missing => (
					"tasks_missing".to_string(),
					format!("tasks.json not found at {}", tasks_path.display()),
				),
				TaskError::Malformed(msg) => ("tasks_malformed".to_string(), msg),
				TaskError::Io(msg) => ("tasks_error".to_string(), msg),
			};
			let error = PayloadError { code, message };
			let payload = build_task_summary_payload(cfg, "default", &[], Some(error));
			let msg = build_envelope("task_summary", &cfg.session_id, &cfg.agent_id, payload, None);
			let mut cache = cache.lock().await;
			let should_send = cache.task_summary.get("default").map(|value| value.as_str()) != Some(&msg);
			cache.task_summary.clear();
			cache.task_summary.insert("default".to_string(), msg.clone());
			drop(cache);
			if should_send {
				let _ = tx.send(msg).await;
			}
			Err(())
		}
	}
}

fn build_task_summary_payload(
	cfg: &ClientConfig,
	tag: &str,
	tasks: &[Task],
	error: Option<PayloadError>,
) -> TaskSummaryPayload {
	let mut counts = TaskCounts {
		total: tasks.len() as u32,
		..TaskCounts::default()
	};
	let mut active_tasks = Vec::new();
	for task in tasks {
		match task.status {
			TaskStatus::Pending => counts.pending += 1,
			TaskStatus::InProgress => counts.in_progress += 1,
			TaskStatus::Blocked => counts.blocked += 1,
			TaskStatus::Done | TaskStatus::Cancelled => counts.done += 1,
			_ => {}
		}
		if task.active_agent {
			active_tasks.push(ActiveTask {
				id: task.id.clone(),
				title: task.title.clone(),
				status: task.status.as_str().to_string(),
				priority: task.priority.as_str().to_string(),
				active_agent: task.active_agent,
			});
		}
	}
	let active_tasks = if active_tasks.is_empty() { None } else { Some(active_tasks) };
	TaskSummaryPayload {
		agent_id: cfg.agent_id.clone(),
		tag: tag.to_string(),
		counts,
		active_tasks,
		error,
	}
}

async fn diff_summary_loop(cfg: ClientConfig, tx: mpsc::Sender<String>, cache: Arc<Mutex<CachedMessages>>) {
	let _ = send_diff_summary(&cfg, &tx, &cache).await;
	let mut ticker = tokio::time::interval(Duration::from_secs(DIFF_INTERVAL_SECS));
	loop {
		tokio::select! {
			_ = tx.closed() => break,
			_ = ticker.tick() => {
				let _ = send_diff_summary(&cfg, &tx, &cache).await;
			}
		}
	}
}

async fn send_diff_summary(
	cfg: &ClientConfig,
	tx: &mpsc::Sender<String>,
	cache: &Arc<Mutex<CachedMessages>>,
) -> Result<(), ()> {
	let payload = build_diff_summary_payload(cfg).await;
	let msg = build_envelope("diff_summary", &cfg.session_id, &cfg.agent_id, payload, None);
	let mut cached = cache.lock().await;
	if cached.diff_summary.as_ref().map(|value| value.as_str()) == Some(&msg) {
		return Ok(());
	}
	cached.diff_summary = Some(msg.clone());
	drop(cached);
	let _ = tx.send(msg).await;
	Ok(())
}

async fn build_diff_summary_payload(cfg: &ClientConfig) -> DiffSummaryPayload {
	let project_root = PathBuf::from(&cfg.project_root);
	match git_repo_root(&project_root).await {
		Ok(repo_root) => match collect_git_summary(&repo_root).await {
			Ok((summary, files)) => DiffSummaryPayload {
				agent_id: cfg.agent_id.clone(),
				repo_root: repo_root.to_string_lossy().to_string(),
				git_available: true,
				summary,
				files,
				reason: None,
			},
		Err(_err) => DiffSummaryPayload {
			agent_id: cfg.agent_id.clone(),
			repo_root: repo_root.to_string_lossy().to_string(),
			git_available: false,
			summary: DiffSummaryCounts::default(),
			files: Vec::new(),
				reason: Some("error".to_string()),
			},
		},
		Err(err) => {
			let reason = match err {
				GitError::Missing => "git_missing",
				GitError::NotRepo => "not_git_repo",
				GitError::Error(_) => "error",
			};
			DiffSummaryPayload {
				agent_id: cfg.agent_id.clone(),
				repo_root: cfg.project_root.clone(),
				git_available: false,
				summary: DiffSummaryCounts::default(),
				files: Vec::new(),
				reason: Some(reason.to_string()),
			}
		}
	}
}

async fn collect_git_summary(repo_root: &Path) -> Result<(DiffSummaryCounts, Vec<DiffFile>), String> {
	let staged_raw = run_git(repo_root, &["diff", "--numstat", "--cached"]).await?;
	let (staged_counts, staged_map) = parse_numstat(&staged_raw);
	let unstaged_raw = run_git(repo_root, &["diff", "--numstat"]).await?;
	let (unstaged_counts, unstaged_map) = parse_numstat(&unstaged_raw);
	let status_raw = run_git(repo_root, &["status", "--porcelain=v1", "-u"]).await?;
	let status_entries = parse_status_entries(&status_raw);
	let mut files = Vec::new();
	for entry in status_entries {
		let (additions, deletions) = if entry.untracked {
			(0, 0)
		} else {
			let staged_stats = staged_map.get(&entry.path).copied().unwrap_or((0, 0));
			let unstaged_stats = unstaged_map.get(&entry.path).copied().unwrap_or((0, 0));
			if entry.staged && entry.unstaged {
				(staged_stats.0 + unstaged_stats.0, staged_stats.1 + unstaged_stats.1)
			} else if entry.staged {
				staged_stats
			} else {
				unstaged_stats
			}
		};
		files.push(DiffFile {
			path: entry.path,
			status: entry.status,
			additions,
			deletions,
			staged: entry.staged,
			untracked: entry.untracked,
		});
	}
	if files.len() > MAX_FILES_LIST {
		files.truncate(MAX_FILES_LIST);
		warn!("diff_summary_truncated");
	}
	let untracked_count = files.iter().filter(|entry| entry.untracked).count() as u32;
	let summary = DiffSummaryCounts {
		staged: staged_counts,
		unstaged: unstaged_counts,
		untracked: UntrackedCounts { files: untracked_count },
	};
	Ok((summary, files))
}

async fn build_diff_patch_response(
	cfg: &ClientConfig,
	request: &DiffPatchRequestPayload,
) -> DiffPatchResponsePayload {
	let context_lines = request.context_lines.unwrap_or(3).max(0) as u32;
	let include_untracked = request.include_untracked.unwrap_or(true);
	let project_root = PathBuf::from(&cfg.project_root);
	let repo_root = match git_repo_root(&project_root).await {
		Ok(root) => root,
		Err(err) => {
			let error = PayloadError {
				code: match err {
					GitError::Missing => "git_missing",
					GitError::NotRepo => "not_git_repo",
					GitError::Error(_) => "error",
				}
				.to_string(),
				message: "unable to locate git repo".to_string(),
			};
			return DiffPatchResponsePayload {
				agent_id: cfg.agent_id.clone(),
				path: request.path.clone(),
				status: "modified".to_string(),
				is_binary: false,
				patch: None,
				error: Some(error),
			};
		}
	};

	let rel_path = normalize_rel_path(&repo_root, &request.path);
	let status_entry = match git_status_entry(&repo_root, &rel_path).await {
		Ok(entry) => entry,
		Err(_) => None,
	};
	let status = status_entry
		.as_ref()
		.map(|entry| entry.status.clone())
		.unwrap_or_else(|| "modified".to_string());
	let untracked = status_entry.as_ref().map(|entry| entry.untracked).unwrap_or(false);

	if untracked && !include_untracked {
		return DiffPatchResponsePayload {
			agent_id: cfg.agent_id.clone(),
			path: request.path.clone(),
			status,
			is_binary: false,
			patch: None,
			error: Some(PayloadError {
				code: "untracked_excluded".to_string(),
				message: "untracked file excluded".to_string(),
			}),
		};
	}

	let abs_path = if Path::new(&request.path).is_absolute() {
		PathBuf::from(&request.path)
	} else {
		repo_root.join(&request.path)
	};

	if untracked {
		return diff_patch_for_untracked(&cfg.agent_id, request, &abs_path, context_lines, &status).await;
	}

	diff_patch_for_tracked(&cfg.agent_id, request, &repo_root, &rel_path, context_lines, &status).await
}

async fn diff_patch_for_untracked(
	agent_id: &str,
	request: &DiffPatchRequestPayload,
	path: &Path,
	context_lines: u32,
	status: &str,
) -> DiffPatchResponsePayload {
	if !path.exists() {
		return DiffPatchResponsePayload {
			agent_id: agent_id.to_string(),
			path: request.path.clone(),
			status: status.to_string(),
			is_binary: false,
			patch: None,
			error: Some(PayloadError {
				code: "not_found".to_string(),
				message: "file does not exist".to_string(),
			}),
		};
	}

	if is_binary_file(path).await {
		return DiffPatchResponsePayload {
			agent_id: agent_id.to_string(),
			path: request.path.clone(),
			status: status.to_string(),
			is_binary: true,
			patch: None,
			error: Some(PayloadError {
				code: "patch_unavailable".to_string(),
				message: "binary file".to_string(),
			}),
		};
	}

	let path_str = path.to_string_lossy().to_string();
	let args = [
		"diff",
		"--no-index",
		&format!("--unified={context_lines}"),
		"--",
		"/dev/null",
		path_str.as_str(),
	];
	let patch = match run_git(path.parent().unwrap_or(Path::new(".")), &args).await {
		Ok(output) => output,
		Err(err) => {
			return DiffPatchResponsePayload {
				agent_id: agent_id.to_string(),
				path: request.path.clone(),
				status: status.to_string(),
				is_binary: false,
				patch: None,
				error: Some(PayloadError {
					code: "patch_unavailable".to_string(),
					message: format!("failed to build patch: {err}"),
				}),
			};
		}
	};
	if patch.as_bytes().len() > MAX_PATCH_BYTES {
		return DiffPatchResponsePayload {
			agent_id: agent_id.to_string(),
			path: request.path.clone(),
			status: status.to_string(),
			is_binary: false,
			patch: None,
			error: Some(PayloadError {
				code: "patch_unavailable".to_string(),
				message: "patch too large".to_string(),
			}),
		};
	}
	DiffPatchResponsePayload {
		agent_id: agent_id.to_string(),
		path: request.path.clone(),
		status: status.to_string(),
		is_binary: false,
		patch: Some(patch),
		error: None,
	}
}

async fn diff_patch_for_tracked(
	agent_id: &str,
	request: &DiffPatchRequestPayload,
	repo_root: &Path,
	rel_path: &str,
	context_lines: u32,
	status: &str,
) -> DiffPatchResponsePayload {
	if is_binary_git(repo_root, rel_path).await {
		return DiffPatchResponsePayload {
			agent_id: agent_id.to_string(),
			path: request.path.clone(),
			status: status.to_string(),
			is_binary: true,
			patch: None,
			error: Some(PayloadError {
				code: "patch_unavailable".to_string(),
				message: "binary file".to_string(),
			}),
		};
	}

	let args = [
		"diff",
		&format!("--unified={context_lines}"),
		"HEAD",
		"--",
		rel_path,
	];
	let patch = match run_git(repo_root, &args).await {
		Ok(output) => output,
		Err(err) => {
			return DiffPatchResponsePayload {
				agent_id: agent_id.to_string(),
				path: request.path.clone(),
				status: status.to_string(),
				is_binary: false,
				patch: None,
				error: Some(PayloadError {
					code: "patch_unavailable".to_string(),
					message: format!("failed to build patch: {err}"),
				}),
			};
		}
	};
	if patch.is_empty() {
		return DiffPatchResponsePayload {
			agent_id: agent_id.to_string(),
			path: request.path.clone(),
			status: status.to_string(),
			is_binary: false,
			patch: None,
			error: Some(PayloadError {
				code: "patch_unavailable".to_string(),
				message: "no changes".to_string(),
			}),
		};
	}
	if patch.as_bytes().len() > MAX_PATCH_BYTES {
		return DiffPatchResponsePayload {
			agent_id: agent_id.to_string(),
			path: request.path.clone(),
			status: status.to_string(),
			is_binary: false,
			patch: None,
			error: Some(PayloadError {
				code: "patch_unavailable".to_string(),
				message: "patch too large".to_string(),
			}),
		};
	}
	DiffPatchResponsePayload {
		agent_id: agent_id.to_string(),
		path: request.path.clone(),
		status: status.to_string(),
		is_binary: false,
		patch: Some(patch),
		error: None,
	}
}

async fn load_tasks(path: &Path) -> Result<ProjectData, TaskError> {
	match fs::read_to_string(path).await {
		Ok(contents) => serde_json::from_str(&contents).map_err(|err| TaskError::Malformed(err.to_string())),
		Err(err) => {
			if err.kind() == io::ErrorKind::NotFound {
				Err(TaskError::Missing)
			} else {
				Err(TaskError::Io(err.to_string()))
			}
		}
	}
}

async fn git_repo_root(project_root: &Path) -> Result<PathBuf, GitError> {
	let output = Command::new("git")
		.arg("-C")
		.arg(project_root)
		.arg("rev-parse")
		.arg("--show-toplevel")
		.output()
		.await;
	let output = match output {
		Ok(value) => value,
		Err(err) => {
			if err.kind() == io::ErrorKind::NotFound {
				return Err(GitError::Missing);
			}
			return Err(GitError::Error(err.to_string()));
		}
	};
	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr).to_string();
		if stderr.contains("not a git repository") {
			return Err(GitError::NotRepo);
		}
		return Err(GitError::Error(stderr));
	}
	let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
	if root.is_empty() {
		return Err(GitError::Error("empty git root".to_string()));
	}
	Ok(PathBuf::from(root))
}

async fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
	let output = Command::new("git")
		.args(args)
		.current_dir(cwd)
		.output()
		.await
		.map_err(|err| err.to_string())?;
	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
		return Err(stderr);
	}
	Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_numstat(output: &str) -> (DiffCounts, HashMap<String, (u32, u32)>) {
	let mut counts = DiffCounts::default();
	let mut map = HashMap::new();
	for line in output.lines() {
		let mut parts = line.splitn(3, '\t');
		let additions = parts.next().unwrap_or("0");
		let deletions = parts.next().unwrap_or("0");
		let path = parts.next().unwrap_or("");
		if path.is_empty() {
			continue;
		}
		let add_count = additions.parse::<u32>().unwrap_or(0);
		let del_count = deletions.parse::<u32>().unwrap_or(0);
		counts.files += 1;
		counts.additions += add_count;
		counts.deletions += del_count;
		map.insert(path.to_string(), (add_count, del_count));
	}
	(counts, map)
}

fn parse_status_entries(output: &str) -> Vec<GitStatusEntry> {
	let mut entries = Vec::new();
	for line in output.lines() {
		if let Some(entry) = parse_status_line(line) {
			entries.push(entry);
		}
	}
	entries
}

fn parse_status_line(line: &str) -> Option<GitStatusEntry> {
	if line.len() < 3 {
		return None;
	}
	if line.starts_with("?? ") {
		return Some(GitStatusEntry {
			path: line[3..].trim().to_string(),
			status: "untracked".to_string(),
			staged: false,
			unstaged: false,
			untracked: true,
		});
	}
	let mut chars = line.chars();
	let x = chars.next()?;
	let y = chars.next()?;
	let mut path = line[3..].trim().to_string();
	if let Some((_, new_path)) = path.split_once("->") {
		path = new_path.trim().to_string();
	}
	let staged = x != ' ' && x != '?';
	let unstaged = y != ' ' && y != '?';
	let status = if matches!(x, 'A' | 'C') || matches!(y, 'A' | 'C') {
		"added"
	} else if x == 'D' || y == 'D' {
		"deleted"
	} else if x == 'R' || y == 'R' {
		"renamed"
	} else {
		"modified"
	};
	Some(GitStatusEntry {
		path,
		status: status.to_string(),
		staged,
		unstaged,
		untracked: false,
	})
}

async fn git_status_entry(repo_root: &Path, path: &str) -> Result<Option<GitStatusEntry>, GitError> {
	let output = Command::new("git")
		.args(["status", "--porcelain=v1", "-u", "--", path])
		.current_dir(repo_root)
		.output()
		.await;
	let output = match output {
		Ok(value) => value,
		Err(err) => {
			if err.kind() == io::ErrorKind::NotFound {
				return Err(GitError::Missing);
			}
			return Err(GitError::Error(err.to_string()));
		}
	};
	if !output.status.success() {
		return Err(GitError::Error(String::from_utf8_lossy(&output.stderr).to_string()));
	}
	let raw = String::from_utf8_lossy(&output.stdout).to_string();
	Ok(raw.lines().next().and_then(parse_status_line))
}

async fn is_binary_git(repo_root: &Path, path: &str) -> bool {
	let output = run_git(repo_root, &["diff", "--numstat", "HEAD", "--", path]).await;
	match output {
		Ok(text) => text
			.lines()
			.next()
			.and_then(|line| line.split('\t').next())
			.map(|first| first.trim() == "-")
			.unwrap_or(false),
		Err(_) => false,
	}
}

async fn is_binary_file(path: &Path) -> bool {
	let mut file = match fs::File::open(path).await {
		Ok(file) => file,
		Err(_) => return false,
	};
	let mut buffer = [0u8; 8192];
	let read = match file.read(&mut buffer).await {
		Ok(count) => count,
		Err(_) => return false,
	};
	buffer[..read].iter().any(|byte| *byte == 0)
}

fn normalize_rel_path(repo_root: &Path, input: &str) -> String {
	let path = PathBuf::from(input);
	if path.is_absolute() {
		if let Ok(stripped) = path.strip_prefix(repo_root) {
			return stripped.to_string_lossy().to_string();
		}
	}
	input.to_string()
}

fn tasks_file_path(project_root: &str) -> PathBuf {
	PathBuf::from(project_root)
		.join(".taskmaster")
		.join("tasks")
		.join("tasks.json")
}

fn state_file_path(project_root: &str) -> PathBuf {
	PathBuf::from(project_root).join(".taskmaster").join("state.json")
}

fn watch_path(watcher: &mut RecommendedWatcher, path: &Path) {
	if let Some(parent) = path.parent() {
		if parent.exists() {
			if let Err(err) = watcher.watch(parent, RecursiveMode::NonRecursive) {
				warn!("watch_failed: {err}");
			}
		}
	}
	if path.exists() {
		if let Err(err) = watcher.watch(path, RecursiveMode::NonRecursive) {
			warn!("watch_failed: {err}");
		}
	}
}

fn init_logging(config: &RuntimeConfig) -> Option<LogGuard> {
	let level = if let Ok(level) = env::var("AOC_LOG_LEVEL") {
		level
	} else {
		"info".to_string()
	};
	let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
	let writer = match open_log_file(&config.log_dir, &config.client.session_id, &config.client.agent_id) {
		Ok(log_guard) => log_guard,
		Err(err) => {
			eprintln!("log_file_error: {err}");
			LogGuard { file: None }
		}
	};
	let file = writer.file.clone();
	let stdout_enabled = config.log_stdout;
	let make_writer = BoxMakeWriter::new(move || MultiWriter::new(file.clone(), stdout_enabled));
	let subscriber = tracing_subscriber::fmt()
		.with_env_filter(filter)
		.with_writer(make_writer)
		.finish();
	if tracing::subscriber::set_global_default(subscriber).is_err() {
		return None;
	}
	Some(writer)
}

impl MultiWriter {
	fn new(file: Option<Arc<StdMutex<std::fs::File>>>, stdout_enabled: bool) -> Self {
		Self {
			stdout_enabled,
			file,
		}
	}
}

impl Write for MultiWriter {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		if self.stdout_enabled {
			let _ = io::stdout().write_all(buf);
		}
		if let Some(file) = &self.file {
			let mut file = file.lock().unwrap();
			let _ = file.write_all(buf);
		}
		Ok(buf.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		if self.stdout_enabled {
			let _ = io::stdout().flush();
		}
		if let Some(file) = &self.file {
			let mut file = file.lock().unwrap();
			let _ = file.flush();
		}
		Ok(())
	}
}

fn open_log_file(log_dir: &str, session_id: &str, agent_id: &str) -> io::Result<LogGuard> {
	if log_dir.trim().is_empty() {
		return Ok(LogGuard { file: None });
	}
	let dir = PathBuf::from(log_dir);
	if std::fs::create_dir_all(&dir).is_err() {
		return Ok(LogGuard { file: None });
	}
	let safe_session = sanitize_component(session_id);
	let safe_agent = sanitize_component(agent_id);
	let path = dir.join(format!("aoc-agent-wrap-{safe_session}-{safe_agent}.log"));
	let file = OpenOptions::new()
		.create(true)
		.append(true)
		.write(true)
		.open(path)?;
	Ok(LogGuard {
		file: Some(Arc::new(StdMutex::new(file))),
	})
}

fn sanitize_component(input: &str) -> String {
	input
		.chars()
		.map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
		.collect()
}

fn resolve_session_id() -> String {
	if let Ok(value) = env::var("AOC_SESSION_ID") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	if let Ok(value) = env::var("ZELLIJ_SESSION_NAME") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	format!("pid-{}", std::process::id())
}

fn resolve_project_root(flag: &str) -> String {
	if !flag.trim().is_empty() {
		return flag.to_string();
	}
	if let Ok(value) = env::var("AOC_PROJECT_ROOT") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	env::current_dir()
		.unwrap_or_else(|_| PathBuf::from("."))
		.to_string_lossy()
		.to_string()
}

fn resolve_agent_id(flag: &str, project_root: &str) -> String {
	if !flag.trim().is_empty() {
		return flag.to_string();
	}
	if let Ok(value) = env::var("AOC_AGENT_ID") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	let base = PathBuf::from(project_root)
		.file_name()
		.map(|s| s.to_string_lossy().to_string())
		.unwrap_or_else(|| "".to_string());
	if !base.is_empty() {
		return base;
	}
	format!("pid-{}", std::process::id())
}

fn resolve_pane_id(flag: &str) -> String {
	if !flag.trim().is_empty() {
		return flag.to_string();
	}
	if let Ok(value) = env::var("AOC_PANE_ID") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	if let Ok(value) = env::var("ZELLIJ_PANE_ID") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	std::process::id().to_string()
}

fn resolve_hub_url(flag_url: &str, flag_addr: &str, session_id: &str) -> Url {
	if !flag_url.trim().is_empty() {
		return Url::parse(flag_url).expect("invalid hub url");
	}
	if let Ok(value) = env::var("AOC_HUB_URL") {
		if !value.trim().is_empty() {
			return Url::parse(&value).expect("invalid hub url");
		}
	}
	let addr = if !flag_addr.trim().is_empty() {
		flag_addr.to_string()
	} else if let Ok(value) = env::var("AOC_HUB_ADDR") {
		if !value.trim().is_empty() {
			value
		} else {
			default_hub_addr(session_id)
		}
	} else {
		default_hub_addr(session_id)
	};
	Url::parse(&format!("ws://{addr}/ws")).expect("invalid hub addr")
}

fn default_hub_addr(session_id: &str) -> String {
	format!("127.0.0.1:{}", derive_port(session_id))
}

fn derive_port(session_id: &str) -> u16 {
	let mut hash: u32 = 2166136261;
	for byte in session_id.as_bytes() {
		hash ^= *byte as u32;
		hash = hash.wrapping_mul(16777619);
	}
	42000 + (hash % 2000) as u16
}

fn resolve_log_dir(flag: &str) -> String {
	if !flag.trim().is_empty() {
		return flag.to_string();
	}
	if let Ok(value) = env::var("AOC_LOG_DIR") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	".aoc/logs".to_string()
}

fn resolve_log_stdout() -> bool {
	if let Ok(value) = env::var("AOC_LOG_STDOUT") {
		match value.trim() {
			"1" | "true" | "TRUE" | "yes" | "YES" => return true,
			"0" | "false" | "FALSE" | "no" | "NO" => return false,
			_ => {}
		}
	}
	false
}

fn next_backoff(current: Duration) -> Duration {
	let next = current + current;
	if next > Duration::from_secs(10) {
		Duration::from_secs(10)
	} else {
		next
	}
}
