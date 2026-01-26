use aoc_core::{ProjectData, Task, TaskStatus};
use ratatui::widgets::TableState;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zellij_tile::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct DisplayRow {
    pub task_idx: usize,
    pub subtask_path: Vec<usize>,
    pub depth: usize,
}

#[derive(Default)]
pub struct State {
    pub project: Option<ProjectData>,
    pub tasks: Vec<Task>,
    pub table_state: TableState,
    pub display_rows: Vec<DisplayRow>,
    pub expanded_tasks: HashSet<String>,
    pub filter: FilterMode,
    pub search_query: String,
    pub input_mode: InputMode,
    pub show_detail: bool,
    pub show_help: bool,
    pub focus: FocusMode,
    pub subtask_cursor: usize,
    pub last_error: Option<String>,
    pub last_error_action: Option<String>,
    pub refresh_secs: f64,
    pub permissions_granted: bool,
    pub current_tag: String,
    pub last_tasks_payload: Option<String>,
    pub needs_render: bool,
    pub pending_tasks: bool,
    pub pending_state: bool,
    pub plugin_pane_id: Option<u32>,
    pub pane_manifest: Option<PaneManifest>,
    pub root: Option<PathBuf>,
    pub projects_base: PathBuf, // Base directory for projects (default: ~/dev)
    pub tasks_path: Option<PathBuf>,
    pub last_tasks_mtime: Option<SystemTime>,
    pub last_state_updated: Option<String>,
    pub last_render_rows: u16,
    pub last_render_cols: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusMode {
    #[default]
    List,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    None,
    Search,
    Root,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    All,
    Pending,
    Done,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskmasterState {
    #[serde(default)]
    current_tag: Option<String>,
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default, flatten)]
    _extra: HashMap<String, Value>,
}

impl FilterMode {
    pub fn label(self) -> &'static str {
        match self {
            FilterMode::All => "all",
            FilterMode::Pending => "pending",
            FilterMode::Done => "done",
        }
    }

    pub fn next(self) -> Self {
        match self {
            FilterMode::All => FilterMode::Pending,
            FilterMode::Pending => FilterMode::Done,
            FilterMode::Done => FilterMode::All,
        }
    }
}

impl State {
    pub const CTX_ACTION: &'static str = "aoc_taskmaster_action";
    pub const ACTION_READ_TASKS: &'static str = "read_tasks";

    pub fn new() -> Self {
        Self {
            projects_base: PathBuf::from(
                std::env::var("HOME").unwrap_or_else(|_| "/home/ceii".to_string()) + "/dev",
            ),
            ..Self::default()
        }
    }

    pub fn load_config(&mut self, configuration: BTreeMap<String, String>) {
        self.refresh_secs = configuration
            .get("refresh_secs")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0);

        // Set projects_base with a sensible default
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/ceii".to_string());
        let default_base = format!("{}/dev", home);

        self.projects_base = configuration
            .get("projects_base")
            .filter(|s| !s.trim().is_empty())
            .map(|s| PathBuf::from(s.trim()))
            .unwrap_or_else(|| PathBuf::from(&default_base));

        // Direct project_root config (still used as initial hint, but pane title takes precedence)
        if let Some(root) = configuration.get("project_root") {
            if !root.trim().is_empty() {
                self.root = Some(PathBuf::from(root.trim()));
            }
        }

        self.needs_render = true;
    }

    pub fn refresh(&mut self) {
        if !self.permissions_granted {
            return;
        }

        // Try to sync root from pane manifest (Agent pane title is source of truth)
        self.sync_root_from_manifest();

        if self.root.is_some() {
            self.refresh_tasks_with_root();
        } else {
            self.last_error = Some("No project root discovered".to_string());
            self.mark_dirty();
        }
    }

    pub fn handle_pane_update(&mut self, manifest: PaneManifest) {
        self.pane_manifest = Some(manifest);
        self.sync_root_from_manifest();
    }

    /// Derive project root from the Agent pane's title in our tab.
    ///
    /// The Agent pane is named: "Agent [<agent_id>]" where agent_id is the
    /// project directory name (e.g., "agent-ops-cockpit").
    ///
    /// We derive the root as: projects_base/agent_id (e.g., ~/dev/agent-ops-cockpit)
    fn sync_root_from_manifest(&mut self) {
        let Some(manifest) = &self.pane_manifest else {
            return;
        };

        // Get our plugin ID to find which tab we're in
        if self.plugin_pane_id.is_none() {
            let ids = get_plugin_ids();
            self.plugin_pane_id = Some(ids.plugin_id);
        }
        let Some(plugin_id) = self.plugin_pane_id else {
            return;
        };

        // Find the tab containing our plugin instance
        let mut target_tab: Option<usize> = None;
        for (tab_index, panes) in &manifest.panes {
            if panes
                .iter()
                .any(|pane| pane.is_plugin && pane.id == plugin_id)
            {
                target_tab = Some(*tab_index);
                break;
            }
        }

        let Some(tab_index) = target_tab else {
            return;
        };

        let Some(panes) = manifest.panes.get(&tab_index) else {
            return;
        };

        // Find the Agent pane by title pattern: "Agent [<agent_id>]"
        let agent_pane = panes
            .iter()
            .find(|pane| !pane.is_plugin && pane.title.starts_with("Agent ["));

        let Some(agent_pane) = agent_pane else {
            return;
        };

        // Extract agent_id from title: "Agent [agent-ops-cockpit]" -> "agent-ops-cockpit"
        let agent_id = extract_agent_id_from_title(&agent_pane.title);

        let Some(agent_id) = agent_id else {
            return;
        };

        // Derive project root from agent_id
        let new_root = self.projects_base.join(&agent_id);

        // Only update if the root changed
        if self.root.as_ref() != Some(&new_root) {
            // Verify the directory exists before switching
            if new_root.is_dir() {
                self.root = Some(new_root);
                // Reset cached data when root changes
                self.last_tasks_payload = None;
                self.last_state_updated = None;
                self.tasks_path = None;
                self.project = None;
                self.tasks.clear();
            }
        }
    }

    fn refresh_tasks_with_root(&mut self) {
        let Some(root) = &self.root else {
            self.last_error = Some("No project root discovered".to_string());
            self.mark_dirty();
            return;
        };

        // Build absolute path to tasks.json
        let tasks_path = root.join(".taskmaster/tasks/tasks.json");

        // Try direct fs read first
        if let Ok(content) = std::fs::read_to_string(&tasks_path) {
            if let Ok(project) = serde_json::from_str::<ProjectData>(&content) {
                self.tasks_path = Some(tasks_path.clone());
                self.project = Some(project);
                self.apply_current_tag();
                self.refresh_state();
                self.update_tasks_mtime(&tasks_path);
                self.last_error = None;
                self.mark_dirty();
                return;
            }
        }

        // Fall back to shell command if direct read fails
        self.request_tasks_via_command();
    }

    fn request_tasks_via_command(&mut self) {
        if self.pending_tasks {
            return;
        }
        let Some(root) = &self.root else {
            return;
        };

        let mut context = BTreeMap::new();
        context.insert(
            Self::CTX_ACTION.to_string(),
            Self::ACTION_READ_TASKS.to_string(),
        );

        // Use the discovered root path directly
        let tasks_path = root.join(".taskmaster/tasks/tasks.json");
        let path_str = tasks_path.to_string_lossy();
        let cmd = format!("echo 'PATH:{}' && cat '{}'", path_str, path_str);

        run_command(&["sh", "-c", &cmd], context);
        self.pending_tasks = true;
    }

    pub fn handle_command_result(
        &mut self,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        context: BTreeMap<String, String>,
    ) {
        let action = context.get(Self::CTX_ACTION).map(String::as_str);
        if action == Some(Self::ACTION_READ_TASKS) {
            self.pending_tasks = false;
            if !stderr.is_empty() {
                self.last_error = Some(format!(
                    "Error reading tasks: {}",
                    String::from_utf8_lossy(&stderr)
                ));
                self.mark_dirty();
                return;
            }
            let mut data = String::from_utf8_lossy(&stdout).to_string();
            // Try to parse PATH header
            if data.starts_with("PATH:") {
                if let Some(newline_idx) = data.find('\n') {
                    let path_str = &data[5..newline_idx];
                    self.tasks_path = Some(PathBuf::from(path_str.trim()));
                    if data.len() > newline_idx + 1 {
                        data = data[newline_idx + 1..].to_string();
                    } else {
                        data = String::new();
                    }
                }
            }

            if let Ok(project) = serde_json::from_str::<ProjectData>(&data) {
                self.project = Some(project);
                self.apply_current_tag();
                self.refresh_state();
                if let Some(path) = self.tasks_path.clone() {
                    self.update_tasks_mtime(&path);
                }
                self.last_error = None;
            } else {
                self.last_error = Some(format!("Failed to parse tasks.json. Len: {}", data.len()));
            }
            self.mark_dirty();
        }
    }

    fn apply_current_tag(&mut self) {
        if let Some(proj) = &self.project {
            let tag = if self.current_tag.is_empty() {
                "master"
            } else {
                &self.current_tag
            };
            if let Some(ctx) = proj.tags.get(tag) {
                self.tasks = ctx.tasks.clone();
            }
            self.recalc_display_rows();
        }
    }

    pub fn recalc_display_rows(&mut self) {
        let mut rows = Vec::new();
        for (idx, task) in self.tasks.iter().enumerate() {
            let match_filter = match self.filter {
                FilterMode::All => true,
                FilterMode::Pending => matches!(
                    task.status,
                    TaskStatus::Pending
                        | TaskStatus::InProgress
                        | TaskStatus::Review
                        | TaskStatus::Blocked
                ),
                FilterMode::Done => matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled),
            };
            if !match_filter {
                continue;
            }
            rows.push(DisplayRow {
                task_idx: idx,
                subtask_path: vec![],
                depth: 0,
            });

            if self.expanded_tasks.contains(&task.id) {
                for (s_i, _sub) in task.subtasks.iter().enumerate() {
                    rows.push(DisplayRow {
                        task_idx: idx,
                        subtask_path: vec![s_i],
                        depth: 1,
                    });
                }
            }
        }
        self.display_rows = rows;
        if self.table_state.selected().unwrap_or(0) >= self.display_rows.len() {
            self.table_state
                .select(Some(self.display_rows.len().saturating_sub(1)));
        }
    }

    pub fn toggle_status(&mut self) {
        let idx = self.table_state.selected().unwrap_or(0);
        if idx >= self.display_rows.len() {
            return;
        }
        let row = &self.display_rows[idx];
        let task_idx = row.task_idx;

        if let Some(sub_idx) = row.subtask_path.first() {
            // Toggle Subtask
            let new_status = match self.tasks[task_idx].subtasks[*sub_idx].status {
                TaskStatus::Done => TaskStatus::Pending,
                _ => TaskStatus::Done,
            };
            self.tasks[task_idx].subtasks[*sub_idx].status = new_status.clone();

            // Update Project
            if let Some(proj) = &mut self.project {
                let tag = if self.current_tag.is_empty() {
                    "master"
                } else {
                    &self.current_tag
                };
                if let Some(ctx) = proj.tags.get_mut(tag) {
                    if task_idx < ctx.tasks.len() && *sub_idx < ctx.tasks[task_idx].subtasks.len() {
                        ctx.tasks[task_idx].subtasks[*sub_idx].status = new_status;
                    }
                }
            }
        } else {
            // Toggle Main Task
            let new_status = match self.tasks[task_idx].status {
                TaskStatus::Done => TaskStatus::Pending,
                _ => TaskStatus::Done,
            };

            // Update View
            self.tasks[task_idx].status = new_status.clone();

            // Update Project
            if let Some(proj) = &mut self.project {
                let tag = if self.current_tag.is_empty() {
                    "master"
                } else {
                    &self.current_tag
                };
                if let Some(ctx) = proj.tags.get_mut(tag) {
                    if task_idx < ctx.tasks.len() {
                        ctx.tasks[task_idx].status = new_status;
                    }
                }
            }
        }

        self.save_project();
        self.mark_dirty();
    }

    fn save_project(&mut self) {
        let path = if let Some(p) = &self.tasks_path {
            p.clone()
        } else {
            self.last_error =
                Some("Cannot save: Unknown tasks.json path (read-only mode)".to_string());
            return;
        };

        if let Some(proj) = &self.project {
            if let Ok(json) = serde_json::to_string_pretty(proj) {
                // Try direct write first
                if std::fs::write(&path, &json).is_err() {
                    // Fallback to RunCommand
                    self.save_project_via_command(&path, &json);
                } else {
                    self.last_error = None;
                    self.update_tasks_mtime(&path);
                    self.touch_state_file();
                }
            }
        }
    }

    fn save_project_via_command(&mut self, path: &PathBuf, json: &str) {
        let path_str = path.to_string_lossy();
        // Use HEREDOC with quoted delimiter to prevent variable expansion and handle quotes
        let cmd = format!(
            "cat <<'EOF_AOC_TASKMASTER' > \"{}\"\\n{}\\nEOF_AOC_TASKMASTER",
            path_str, json
        );
        let mut context = BTreeMap::new();
        context.insert("action".to_string(), "save".to_string());
        run_command(&["sh", "-c", &cmd], context);
    }

    fn update_tasks_mtime(&mut self, path: &Path) {
        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                self.last_tasks_mtime = Some(modified);
            }
        }
    }

    fn resolve_state_path(&self) -> Option<PathBuf> {
        // Derive from tasks_path if available
        if let Some(tasks_path) = &self.tasks_path {
            let mut path = tasks_path.clone();
            path.pop(); // remove tasks.json
            path.pop(); // remove tasks/
            path.push("state.json");
            return Some(path);
        }
        // Fallback to discovered root
        if let Some(root) = &self.root {
            let path = root.join(".taskmaster/state.json");
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    fn refresh_state(&mut self) {
        let Some(path) = self.resolve_state_path() else {
            return;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(state) = serde_json::from_str::<TaskmasterState>(&content) else {
            return;
        };
        if let Some(tag) = state.current_tag.clone() {
            if tag != self.current_tag {
                self.current_tag = tag;
                self.apply_current_tag();
            }
        }
        if let Some(last_updated) = state.last_updated {
            if self.last_state_updated.as_ref() != Some(&last_updated) {
                self.last_state_updated = Some(last_updated);
            }
        }
    }

    fn touch_state_file(&mut self) {
        let Some(path) = self.resolve_state_path() else {
            return;
        };
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_millis().to_string(),
            Err(_) => return,
        };
        let mut state = if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str::<Value>(&content).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };
        if let Value::Object(map) = &mut state {
            map.insert("lastUpdated".to_string(), Value::String(now));
            if !self.current_tag.is_empty() {
                map.insert(
                    "currentTag".to_string(),
                    Value::String(self.current_tag.clone()),
                );
            }
        }
        if let Ok(payload) = serde_json::to_string_pretty(&state) {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, payload);
        }
    }

    pub fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Down | BareKey::Char('j') => {
                let i = match self.table_state.selected() {
                    Some(i) => {
                        if self.display_rows.is_empty() {
                            0
                        } else if i >= self.display_rows.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.table_state.select(Some(i));
                return true;
            }
            BareKey::Up | BareKey::Char('k') => {
                let i = match self.table_state.selected() {
                    Some(i) => {
                        if self.display_rows.is_empty() {
                            0
                        } else if i == 0 {
                            self.display_rows.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.table_state.select(Some(i));
                return true;
            }
            BareKey::Char('r') => {
                self.refresh();
                return true;
            }
            BareKey::Enter => {
                if self.show_help {
                    self.show_help = false;
                }
                self.show_detail = !self.show_detail;
                if !self.show_detail {
                    self.focus = FocusMode::List;
                }
                return true;
            }
            BareKey::Char('?') => {
                self.show_help = !self.show_help;
                return true;
            }
            BareKey::Char(' ') => {
                let idx = self.table_state.selected().unwrap_or(0);
                if idx < self.display_rows.len() {
                    let task_idx = self.display_rows[idx].task_idx;
                    // Check if task has subtasks before expanding
                    if !self.tasks[task_idx].subtasks.is_empty() {
                        let id = self.tasks[task_idx].id.clone();
                        if self.expanded_tasks.contains(&id) {
                            self.expanded_tasks.remove(&id);
                        } else {
                            self.expanded_tasks.insert(id);
                        }
                        self.recalc_display_rows();
                    }
                }
                return true;
            }
            BareKey::Tab => {
                if self.show_detail {
                    self.focus = match self.focus {
                        FocusMode::List => FocusMode::Details,
                        FocusMode::Details => FocusMode::List,
                    };
                }
                return true;
            }
            BareKey::Char('x') => {
                self.toggle_status();
                return true;
            }
            BareKey::Char('f') => {
                self.filter = self.filter.next();
                self.recalc_display_rows();
                self.mark_dirty();
                return true;
            }
            BareKey::Char('t') => {
                self.cycle_tag();
                return true;
            }
            _ => false,
        }
    }

    fn cycle_tag(&mut self) {
        if let Some(proj) = &self.project {
            let mut tags: Vec<&String> = proj.tags.keys().collect();
            tags.sort(); // Stable order

            if tags.is_empty() {
                return;
            }

            let current = if self.current_tag.is_empty() {
                "master".to_string()
            } else {
                self.current_tag.clone()
            };

            let pos = tags.iter().position(|&t| *t == current).unwrap_or(0);
            let next_idx = (pos + 1) % tags.len();
            self.current_tag = tags[next_idx].clone();

            self.apply_current_tag();
            self.mark_dirty();
        }
    }

    pub fn mark_dirty(&mut self) {
        self.needs_render = true;
        self.update_pane_title();
    }

    fn update_pane_title(&self) {
        let total = self.tasks.len();
        let done = self
            .tasks
            .iter()
            .filter(|t| format!("{:?}", t.status).to_lowercase() == "done")
            .count();
        let percent = if total > 0 {
            (done as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Progress Bar (ASCII)
        let bar_width = 10;
        let filled = (percent / 100.0 * bar_width as f64) as usize;
        let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(bar_width - filled));

        let tag = if self.current_tag.is_empty() {
            "master"
        } else {
            &self.current_tag
        };

        let title = format!(
            "Taskmaster [{}] {} {}/{} | Filter: {} | ? Help",
            tag,
            bar,
            done,
            total,
            self.filter.label()
        );

        let ids = get_plugin_ids();
        rename_plugin_pane(ids.plugin_id, title);
    }

    pub fn take_render(&mut self) -> bool {
        let val = self.needs_render;
        self.needs_render = false;
        val
    }
}

/// Extract agent_id from pane title.
/// Pattern: "Agent [<agent_id>]" -> Some("agent_id")
fn extract_agent_id_from_title(title: &str) -> Option<String> {
    // Look for "Agent [" prefix and extract content until "]"
    let prefix = "Agent [";
    if !title.starts_with(prefix) {
        return None;
    }
    let rest = &title[prefix.len()..];
    let end = rest.find(']')?;
    let agent_id = &rest[..end];
    if agent_id.is_empty() {
        return None;
    }
    Some(agent_id.to_string())
}
