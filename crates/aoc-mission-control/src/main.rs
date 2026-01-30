use chrono::{DateTime, Utc};
use crossterm::{
	event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
	backend::CrosstermBackend,
	layout::{Constraint, Direction, Layout},
	style::{Color, Modifier, Style},
	text::{Line, Text},
	widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
	Terminal,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
	collections::{BTreeMap, HashMap},
	error::Error,
	io,
	time::Duration,
};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use url::Url;

const PROTOCOL_VERSION: &str = "1";

#[derive(Clone, Debug)]
struct Config {
	session_id: String,
	hub_url: Url,
	client_id: String,
}

#[derive(Deserialize, Serialize)]
struct Envelope {
	version: String,
	#[serde(rename = "type")]
	r#type: String,
	session_id: String,
	sender_id: String,
	timestamp: String,
	payload: Value,
	#[serde(default)]
	request_id: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
struct HelloPayload {
	client_id: String,
	role: String,
	capabilities: Vec<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	agent_id: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct AgentStatusPayload {
	agent_id: String,
	status: String,
	pane_id: String,
	project_root: String,
	#[serde(default)]
	cwd: Option<String>,
	#[serde(default)]
	message: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffCounts {
	files: u32,
	additions: u32,
	deletions: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct UntrackedCounts {
	files: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffSummaryCounts {
	#[serde(default)]
	staged: DiffCounts,
	#[serde(default)]
	unstaged: DiffCounts,
	#[serde(default)]
	untracked: UntrackedCounts,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffFile {
	path: String,
	#[serde(default)]
	status: String,
	#[serde(default)]
	additions: u32,
	#[serde(default)]
	deletions: u32,
	#[serde(default)]
	staged: bool,
	#[serde(default)]
	untracked: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
struct DiffSummaryPayload {
	agent_id: String,
	repo_root: String,
	#[serde(default)]
	git_available: bool,
	#[serde(default)]
	summary: DiffSummaryCounts,
	#[serde(default)]
	files: Vec<DiffFile>,
	#[serde(default)]
	reason: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct TaskSummaryPayload {
	agent_id: String,
	tag: String,
	counts: Value,
	#[serde(default)]
	active_tasks: Option<Vec<Value>>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct PayloadError {
	code: String,
	message: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
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

#[derive(Deserialize, Serialize, Clone, Debug)]
struct DiffPatchResponsePayload {
	agent_id: String,
	path: String,
	status: String,
	is_binary: bool,
	#[serde(default)]
	patch: Option<String>,
	#[serde(default)]
	error: Option<PayloadError>,
}

#[derive(Clone, Debug)]
struct PatchEntry {
	status: String,
	is_binary: bool,
	patch: Option<String>,
	error: Option<PayloadError>,
}

#[derive(Clone, Debug)]
struct AgentInfo {
	status: Option<AgentStatusPayload>,
	diff: Option<DiffSummaryPayload>,
	last_seen: DateTime<Utc>,
	patches: HashMap<String, PatchEntry>,
	diff_fingerprint: Option<String>,
}

#[derive(Debug)]
enum HubEvent {
	Connected,
	Disconnected,
	AgentStatus(AgentStatusPayload),
	DiffSummary(DiffSummaryPayload),
	TaskSummary(TaskSummaryPayload),
	DiffPatchResponse(DiffPatchResponsePayload, Option<String>),
}

#[derive(Debug)]
enum HubCommand {
	RequestPatch {
		agent_id: String,
		path: String,
		context_lines: i32,
		include_untracked: bool,
		request_id: String,
	},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
	Agents,
	Files,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputMode {
	Normal,
	Search,
}

struct App {
	session_id: String,
	connected: bool,
	agents: BTreeMap<String, AgentInfo>,
	order: Vec<String>,
	selected_agent: usize,
	selected_file: usize,
	patch_scroll: usize,
	focus: Focus,
	input_mode: InputMode,
	file_query: String,
	request_counter: u64,
	pending_requests: HashMap<String, String>,
}

impl App {
	fn new(session_id: String) -> Self {
		Self {
			session_id,
			connected: false,
			agents: BTreeMap::new(),
			order: Vec::new(),
			selected_agent: 0,
			selected_file: 0,
			patch_scroll: 0,
			focus: Focus::Agents,
			input_mode: InputMode::Normal,
			file_query: String::new(),
			request_counter: 0,
			pending_requests: HashMap::new(),
		}
	}

	fn apply_event(&mut self, event: HubEvent) {
		match event {
			HubEvent::Connected => self.connected = true,
			HubEvent::Disconnected => self.connected = false,
			HubEvent::AgentStatus(status) => {
				let agent_id = status.agent_id.clone();
				let entry = self
					.agents
					.entry(agent_id.clone())
					.or_insert_with(|| AgentInfo {
						status: None,
						diff: None,
						last_seen: Utc::now(),
						patches: HashMap::new(),
						diff_fingerprint: None,
					});
				entry.status = Some(status);
				entry.last_seen = Utc::now();
				self.ensure_order(&agent_id);
			}
			HubEvent::DiffSummary(diff) => {
				let agent_id = diff.agent_id.clone();
				let fingerprint = diff_fingerprint(&diff);
				let entry = self
					.agents
					.entry(agent_id.clone())
					.or_insert_with(|| AgentInfo {
						status: None,
						diff: None,
						last_seen: Utc::now(),
						patches: HashMap::new(),
						diff_fingerprint: None,
					});
				if entry.diff_fingerprint.as_ref() != Some(&fingerprint) {
					entry.patches.clear();
					self.pending_requests
						.retain(|key, _| !key.starts_with(&format!("{agent_id}|")));
					entry.diff_fingerprint = Some(fingerprint);
					self.patch_scroll = 0;
				}
				entry.diff = Some(diff);
				entry.last_seen = Utc::now();
				self.ensure_order(&agent_id);
				self.clamp_selected_file();
			}
			HubEvent::TaskSummary(_) => {}
			HubEvent::DiffPatchResponse(payload, request_id) => {
				let agent_id = payload.agent_id.clone();
				let path = payload.path.clone();
				let key = request_key(&agent_id, &path);
				if let Some(pending_id) = self.pending_requests.get(&key) {
					if let Some(request_id) = &request_id {
						if request_id != pending_id {
							return;
						}
					}
					self.pending_requests.remove(&key);
				}
				if let Some(entry) = self.agents.get_mut(&agent_id) {
					entry.patches.insert(
						path,
						PatchEntry {
							status: payload.status,
							is_binary: payload.is_binary,
							patch: payload.patch,
							error: payload.error,
						},
					);
				}
			}
		}
	}

	fn ensure_order(&mut self, agent_id: &str) {
		if !self.order.iter().any(|id| id == agent_id) {
			self.order.push(agent_id.to_string());
			self.order.sort();
		}
		if self.selected_agent >= self.order.len() {
			self.selected_agent = self.order.len().saturating_sub(1);
		}
	}

	fn move_agent_selection(&mut self, delta: i32) {
		if self.order.is_empty() {
			self.selected_agent = 0;
			return;
		}
		let next = self.selected_agent as i32 + delta;
		let next = next.clamp(0, (self.order.len() - 1) as i32);
		if self.selected_agent != next as usize {
			self.selected_agent = next as usize;
			self.selected_file = 0;
			self.patch_scroll = 0;
		}
	}

	fn move_file_selection(&mut self, delta: i32) {
		let len = self.current_files_len();
		if len == 0 {
			self.selected_file = 0;
			return;
		}
		let next = self.selected_file as i32 + delta;
		let next = next.clamp(0, (len - 1) as i32);
		if self.selected_file != next as usize {
			self.selected_file = next as usize;
			self.patch_scroll = 0;
		}
	}

	fn current_files_len(&self) -> usize {
		self.selected_agent()
			.and_then(|agent| agent.diff.as_ref())
			.map(|diff| self.filtered_files(diff).len())
			.unwrap_or(0)
	}

	fn clamp_selected_file(&mut self) {
		let len = self.current_files_len();
		if len == 0 {
			self.selected_file = 0;
			return;
		}
		if self.selected_file >= len {
			self.selected_file = len - 1;
		}
	}

	fn selected_agent_id(&self) -> Option<&str> {
		self.order.get(self.selected_agent).map(|s| s.as_str())
	}

	fn selected_agent(&self) -> Option<&AgentInfo> {
		let id = self.selected_agent_id()?;
		self.agents.get(id)
	}

	fn filtered_files<'a>(&self, diff: &'a DiffSummaryPayload) -> Vec<&'a DiffFile> {
		if self.file_query.is_empty() {
			return diff.files.iter().collect();
		}
		let query = self.file_query.to_lowercase();
		diff
			.files
			.iter()
			.filter(|file| file.path.to_lowercase().contains(&query))
			.collect()
	}

	fn selected_file<'a>(&self, diff: &'a DiffSummaryPayload) -> Option<&'a DiffFile> {
		let files = self.filtered_files(diff);
		files.get(self.selected_file).copied()
	}

	fn selected_patch(&self) -> Option<&PatchEntry> {
		let agent_id = self.selected_agent_id()?;
		let agent = self.agents.get(agent_id)?;
		let diff = agent.diff.as_ref()?;
		let file = self.selected_file(diff)?;
		agent.patches.get(&file.path)
	}

	fn current_patch_lines_len(&self) -> usize {
		self.selected_patch()
			.and_then(|entry| entry.patch.as_ref())
			.map(|patch| patch.lines().count())
			.unwrap_or(0)
	}

	fn scroll_patch(&mut self, delta: i32) {
		let len = self.current_patch_lines_len();
		if len == 0 {
			self.patch_scroll = 0;
			return;
		}
		let next = self.patch_scroll as i32 + delta;
		let next = next.clamp(0, (len.saturating_sub(1)) as i32);
		self.patch_scroll = next as usize;
	}

	fn start_search(&mut self) {
		self.input_mode = InputMode::Search;
		self.file_query.clear();
		self.selected_file = 0;
		self.patch_scroll = 0;
	}

	fn update_search(&mut self, key: KeyEvent) {
		match key.code {
			KeyCode::Char(ch) => {
				if !key.modifiers.contains(KeyModifiers::CONTROL) {
					self.file_query.push(ch);
					self.selected_file = 0;
					self.patch_scroll = 0;
				}
			}
			KeyCode::Backspace => {
				self.file_query.pop();
				self.selected_file = 0;
				self.patch_scroll = 0;
			}
			KeyCode::Enter | KeyCode::Esc => {
				self.input_mode = InputMode::Normal;
				self.clamp_selected_file();
			}
			_ => {}
		}
	}

	fn next_request_id(&mut self) -> String {
		self.request_counter = self.request_counter.saturating_add(1);
		format!("req-{}", self.request_counter)
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let config = load_config();
	init_logging();
	let (hub_tx, mut hub_rx) = mpsc::channel(256);
	let (cmd_tx, cmd_rx) = mpsc::channel(64);
	let hub_cfg = config.clone();
	tokio::spawn(async move {
		hub_loop(hub_cfg, hub_tx, cmd_rx).await;
	});

	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen)?;
	let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;
	let mut app = App::new(config.session_id.clone());
	let mut events = EventStream::new();
	let mut ticker = tokio::time::interval(Duration::from_millis(200));

	loop {
		terminal.draw(|frame| render_ui(frame, &app))?;
		tokio::select! {
			_ = ticker.tick() => {},
			maybe_event = events.next() => {
				if let Some(Ok(event)) = maybe_event {
					if handle_input(event, &mut app, &cmd_tx) {
						break;
					}
				}
			},
			Some(event) = hub_rx.recv() => {
				app.apply_event(event);
			},
		}
	}

	disable_raw_mode()?;
	execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
	terminal.show_cursor()?;
	Ok(())
}

fn render_ui(frame: &mut ratatui::Frame, app: &App) {
	let size = frame.size();
	let layout = Layout::default()
		.direction(Direction::Vertical)
		.constraints([Constraint::Length(3), Constraint::Min(0)])
		.split(size);
	let header = render_header(app);
	frame.render_widget(header, layout[0]);

	let body = Layout::default()
		.direction(Direction::Horizontal)
		.constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
		.split(layout[1]);
	let list = render_agent_list(app);
	let mut list_state = ListState::default();
	if !app.order.is_empty() {
		list_state.select(Some(app.selected_agent));
	}
	frame.render_stateful_widget(list, body[0], &mut list_state);
	let right = Layout::default()
		.direction(Direction::Vertical)
		.constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
		.split(body[1]);
	let diffs = render_file_list(app);
	let mut file_state = ListState::default();
	if app.current_files_len() > 0 {
		file_state.select(Some(app.selected_file));
	}
	frame.render_stateful_widget(diffs, right[0], &mut file_state);
	let patch = render_patch_view(app);
	frame.render_widget(patch, right[1]);
}

fn render_header(app: &App) -> Paragraph<'static> {
	let status = if app.connected { "connected" } else { "disconnected" };
	let focus = match app.focus {
		Focus::Agents => "agents",
		Focus::Files => "files",
	};
	let search = if app.input_mode == InputMode::Search {
		format!("search: {}", app.file_query)
	} else if app.file_query.is_empty() {
		"search: off".to_string()
	} else {
		format!("search: {}", app.file_query)
	};
	let line = Line::from(format!(
		"Mission Control | session={} | hub={} | focus={} | {}",
		app.session_id, status, focus, search
	));
	Paragraph::new(line).block(Block::default().borders(Borders::ALL).title("Status"))
}

fn render_agent_list(app: &App) -> List<'static> {
	let items: Vec<ListItem> = if app.order.is_empty() {
		vec![ListItem::new("No agents connected")]
	} else {
		app.order
			.iter()
			.map(|agent_id| {
				let status = app
					.agents
					.get(agent_id)
					.and_then(|info| info.status.as_ref().map(|s| s.status.clone()))
					.unwrap_or_else(|| "unknown".to_string());
				let diff_count = app
					.agents
					.get(agent_id)
					.and_then(|info| info.diff.as_ref().map(|diff| diff.files.len()))
					.unwrap_or(0);
				ListItem::new(format!("{agent_id}  {status}  ({diff_count})"))
			})
			.collect()
	};

	let mut list = List::new(items).block(Block::default().borders(Borders::ALL).title("Agents"));
	if app.focus == Focus::Agents {
		list = list
			.highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
			.highlight_symbol("> ");
	}
	list
}

fn render_file_list(app: &App) -> List<'static> {
	let mut items = Vec::new();
	if let Some(agent) = app.selected_agent() {
		if let Some(diff) = &agent.diff {
			let files = app.filtered_files(diff);
			if files.is_empty() {
				items.push(ListItem::new("No diffs"));
			} else {
				for file in files {
					let status = format_status(&file.status);
					let counts = format!("+{} -{}", file.additions, file.deletions);
					items.push(ListItem::new(format!("{status} {counts} {}", file.path)));
				}
			}
		} else {
			items.push(ListItem::new("No diff summary"));
		}
	} else {
		items.push(ListItem::new("Select an agent"));
	}

	let mut list = List::new(items).block(Block::default().borders(Borders::ALL).title("Diff Files"));
	if app.focus == Focus::Files {
		list = list
			.highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
			.highlight_symbol("> ");
	}
	list
}

fn render_patch_view(app: &App) -> Paragraph<'static> {
	let mut lines = Vec::new();
	if let Some(agent) = app.selected_agent() {
		if let Some(diff) = &agent.diff {
			lines.push(Line::from(format!("Repo: {}", diff.repo_root)));
			if diff.git_available {
				lines.extend(format_diff_summary(&diff.summary));
			} else if let Some(reason) = &diff.reason {
				lines.push(Line::from(format!("Diff unavailable: {reason}")));
			}
			if let Some(file) = app.selected_file(diff) {
				lines.push(Line::from(""));
				lines.push(Line::from(format!("File: {}", file.path)));
				lines.push(Line::from(format!("Status: {}", file.status)));
				if let Some(entry) = app.selected_patch() {
					if let Some(error) = &entry.error {
						lines.push(Line::from(format!("Error: {} ({})", error.message, error.code)));
					} else if entry.is_binary {
						lines.push(Line::from("Binary file; patch unavailable"));
					} else if let Some(patch) = &entry.patch {
						lines.push(Line::from(""));
						for line in patch.lines() {
							lines.push(Line::from(line.to_string()));
						}
					} else {
						lines.push(Line::from("Patch unavailable"));
					}
				} else {
					lines.push(Line::from("Press Enter to request patch"));
				}
			}
		} else {
			lines.push(Line::from("No diff summary"));
		}
	} else {
		lines.push(Line::from("No agent selected"));
	}
	let text = Text::from(lines);
	Paragraph::new(text)
		.block(Block::default().borders(Borders::ALL).title("Patch"))
		.scroll((app.patch_scroll as u16, 0))
		.wrap(Wrap { trim: false })
}

fn format_diff_summary(summary: &DiffSummaryCounts) -> Vec<Line<'static>> {
	vec![
		Line::from(format!(
			"Staged: files={} +{} -{}",
			summary.staged.files, summary.staged.additions, summary.staged.deletions
		)),
		Line::from(format!(
			"Unstaged: files={} +{} -{}",
			summary.unstaged.files, summary.unstaged.additions, summary.unstaged.deletions
		)),
		Line::from(format!("Untracked files: {}", summary.untracked.files)),
	]
}

fn format_status(status: &str) -> String {
	match status {
		"added" => "A".to_string(),
		"deleted" => "D".to_string(),
		"renamed" => "R".to_string(),
		"untracked" => "?".to_string(),
		_ => "M".to_string(),
	}
}

fn handle_input(event: Event, app: &mut App, cmd_tx: &mpsc::Sender<HubCommand>) -> bool {
	match event {
		Event::Key(key) if key.kind == KeyEventKind::Press => {
			if app.input_mode == InputMode::Search {
				app.update_search(key);
				return false;
			}
			return handle_normal_input(key, app, cmd_tx);
		}
		_ => {}
	}
	false
}

fn handle_normal_input(key: KeyEvent, app: &mut App, cmd_tx: &mpsc::Sender<HubCommand>) -> bool {
	match key.code {
		KeyCode::Char('q') => return true,
		KeyCode::Esc => {
			toggle_floating_panes();
		}
		KeyCode::Tab => {
			app.focus = match app.focus {
				Focus::Agents => Focus::Files,
				Focus::Files => Focus::Agents,
			};
		}
		KeyCode::Char('/') => {
			if app.focus == Focus::Files {
				app.start_search();
			}
		}
		KeyCode::Down | KeyCode::Char('j') => match app.focus {
			Focus::Agents => app.move_agent_selection(1),
			Focus::Files => app.move_file_selection(1),
		},
		KeyCode::Up | KeyCode::Char('k') => match app.focus {
			Focus::Agents => app.move_agent_selection(-1),
			Focus::Files => app.move_file_selection(-1),
		},
		KeyCode::PageDown => app.scroll_patch(10),
		KeyCode::PageUp => app.scroll_patch(-10),
		KeyCode::Enter => {
			if app.focus == Focus::Files {
				send_patch_request(app, cmd_tx, false);
			}
		}
		KeyCode::Char('r') => {
			if app.focus == Focus::Files {
				send_patch_request(app, cmd_tx, true);
			}
		}
		_ => {}
	}
	false
}

fn toggle_floating_panes() {
	if std::env::var("ZELLIJ").is_err() && std::env::var("ZELLIJ_SESSION_NAME").is_err() {
		return;
	}
	let _ = std::process::Command::new("zellij")
		.args(["action", "toggle-floating-panes"])
		.status();
}

fn send_patch_request(app: &mut App, cmd_tx: &mpsc::Sender<HubCommand>, force: bool) {
	let agent_id = match app.selected_agent_id() {
		Some(id) => id.to_string(),
		None => return,
	};
	let (path, should_skip) = {
		let agent = match app.agents.get(&agent_id) {
			Some(agent) => agent,
			None => return,
		};
		let diff = match agent.diff.as_ref() {
			Some(diff) => diff,
			None => return,
		};
		let file = match app.selected_file(diff) {
			Some(file) => file,
			None => return,
		};
		let should_skip = if !force {
			agent
				.patches
				.get(&file.path)
				.map(|entry| entry.patch.is_some() || entry.error.is_some() || entry.is_binary)
				.unwrap_or(false)
		} else {
			false
		};
		(file.path.clone(), should_skip)
	};
	if should_skip {
		return;
	}
	let request_id = app.next_request_id();
	app.pending_requests
		.insert(request_key(&agent_id, &path), request_id.clone());
	let cmd = HubCommand::RequestPatch {
		agent_id,
		path,
		context_lines: 3,
		include_untracked: true,
		request_id,
	};
	let _ = cmd_tx.try_send(cmd);
}

async fn hub_loop(config: Config, tx: mpsc::Sender<HubEvent>, mut cmd_rx: mpsc::Receiver<HubCommand>) {
	let mut backoff = Duration::from_secs(1);
	loop {
		let connect = connect_async(config.hub_url.clone()).await;
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
		let hello = build_hello(&config);
		if ws
			.send(tokio_tungstenite::tungstenite::Message::Text(hello))
			.await
			.is_err()
		{
			warn!("hub_hello_error");
			let _ = ws.close(None).await;
			continue;
		}
		let _ = tx.send(HubEvent::Connected).await;
		loop {
			tokio::select! {
				Some(msg) = ws.next() => {
					match msg {
						Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
							if let Some(event) = parse_event(&config, &text) {
								let _ = tx.send(event).await;
							}
						}
						Ok(_) => {}
						Err(_) => break,
					}
				}
				Some(cmd) = cmd_rx.recv() => {
					if let Some(text) = command_to_message(&config, cmd) {
						if ws.send(tokio_tungstenite::tungstenite::Message::Text(text)).await.is_err() {
							break;
						}
					}
				}
				else => break,
			}
		}
		let _ = tx.send(HubEvent::Disconnected).await;
		let _ = ws.close(None).await;
	}
}

fn parse_event(config: &Config, text: &str) -> Option<HubEvent> {
	let envelope: Envelope = serde_json::from_str(text).ok()?;
	if envelope.session_id != config.session_id {
		return None;
	}
	match envelope.r#type.as_str() {
		"agent_status" => {
			let payload: AgentStatusPayload = serde_json::from_value(envelope.payload).ok()?;
			Some(HubEvent::AgentStatus(payload))
		}
		"diff_summary" => {
			let payload: DiffSummaryPayload = serde_json::from_value(envelope.payload).ok()?;
			Some(HubEvent::DiffSummary(payload))
		}
		"task_summary" => {
			let payload: TaskSummaryPayload = serde_json::from_value(envelope.payload).ok()?;
			Some(HubEvent::TaskSummary(payload))
		}
		"diff_patch_response" => {
			let payload: DiffPatchResponsePayload = serde_json::from_value(envelope.payload).ok()?;
			Some(HubEvent::DiffPatchResponse(payload, envelope.request_id))
		}
		_ => None,
	}
}

fn build_hello(config: &Config) -> String {
	let payload = HelloPayload {
		client_id: config.client_id.clone(),
		role: "subscriber".to_string(),
		capabilities: vec!["diff_patch_request".to_string()],
		agent_id: None,
	};
	let envelope = Envelope {
		version: PROTOCOL_VERSION.to_string(),
		r#type: "hello".to_string(),
		session_id: config.session_id.clone(),
		sender_id: config.client_id.clone(),
		timestamp: Utc::now().to_rfc3339(),
		payload: serde_json::to_value(payload).unwrap_or(Value::Null),
		request_id: None,
	};
	serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string())
}

fn command_to_message(config: &Config, cmd: HubCommand) -> Option<String> {
	match cmd {
		HubCommand::RequestPatch {
			agent_id,
			path,
			context_lines,
			include_untracked,
			request_id,
		} => {
			let payload = DiffPatchRequestPayload {
				agent_id,
				path,
				context_lines: Some(context_lines),
				include_untracked: Some(include_untracked),
				request_id: Some(request_id.clone()),
			};
			let envelope = Envelope {
				version: PROTOCOL_VERSION.to_string(),
				r#type: "diff_patch_request".to_string(),
				session_id: config.session_id.clone(),
				sender_id: config.client_id.clone(),
				timestamp: Utc::now().to_rfc3339(),
				payload: serde_json::to_value(payload).unwrap_or(Value::Null),
				request_id: Some(request_id),
			};
			serde_json::to_string(&envelope).ok()
		}
	}
}

fn diff_fingerprint(diff: &DiffSummaryPayload) -> String {
	let mut fingerprint = String::new();
	for file in &diff.files {
		fingerprint.push_str(&file.path);
		fingerprint.push('|');
		fingerprint.push_str(&file.status);
		fingerprint.push('|');
		fingerprint.push_str(&file.additions.to_string());
		fingerprint.push('|');
		fingerprint.push_str(&file.deletions.to_string());
		fingerprint.push(';');
	}
	fingerprint
}

fn request_key(agent_id: &str, path: &str) -> String {
	format!("{agent_id}|{path}")
}

fn load_config() -> Config {
	let session_id = resolve_session_id();
	let hub_url = resolve_hub_url(&session_id);
	let client_id = format!("mission-control-{}", std::process::id());
	Config {
		session_id,
		hub_url,
		client_id,
	}
}

fn init_logging() {
	let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
	let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn resolve_session_id() -> String {
	if let Ok(value) = std::env::var("AOC_SESSION_ID") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	if let Ok(value) = std::env::var("ZELLIJ_SESSION_NAME") {
		if !value.trim().is_empty() {
			return value;
		}
	}
	format!("pid-{}", std::process::id())
}

fn resolve_hub_url(session_id: &str) -> Url {
	if let Ok(value) = std::env::var("AOC_HUB_URL") {
		if !value.trim().is_empty() {
			return Url::parse(&value).expect("invalid hub url");
		}
	}
	let addr = if let Ok(value) = std::env::var("AOC_HUB_ADDR") {
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

fn next_backoff(current: Duration) -> Duration {
	let next = current + current;
	if next > Duration::from_secs(10) {
		Duration::from_secs(10)
	} else {
		next
	}
}
