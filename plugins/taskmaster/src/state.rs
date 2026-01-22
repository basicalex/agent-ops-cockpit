use aoc_core::{ProjectData, Task, TaskPriority, TaskStatus};
use ratatui::widgets::TableState;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::time::SystemTime;
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
    pub pending_root: bool,
    pub cwd: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub root_file: Option<PathBuf>,
    pub roots: Vec<PathBuf>,
    pub root_index: usize,
    pub ignore_refresh_until: Option<SystemTime>,
    pub tasks_path: Option<PathBuf>, // New: Path for saving
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
    pub const ACTION_READ_STATE: &'static str = "read_state";
    pub const ACTION_READ_ROOT: &'static str = "read_root";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_config(&mut self, configuration: BTreeMap<String, String>) {
        self.refresh_secs = configuration
            .get("refresh_secs")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(5.0);
        self.needs_render = true;
    }

    pub fn refresh(&mut self) {
        if !self.permissions_granted {
            return;
        }

        // Try direct read first
        let path = PathBuf::from(".taskmaster/tasks/tasks.json");
        if std::fs::metadata(&path).is_ok() {
            self.tasks_path = Some(path.clone());
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(project) = serde_json::from_str::<ProjectData>(&content) {
                    self.project = Some(project);
                    self.apply_current_tag();
                    self.last_error = None;
                    self.mark_dirty();
                    return; // Success, skip shell command
                }
            }
        }

        self.request_tasks_via_command();
    }

    fn request_tasks_via_command(&mut self) {
        if self.pending_tasks {
            return;
        }
        let mut context = BTreeMap::new();
        context.insert(
            Self::CTX_ACTION.to_string(),
            Self::ACTION_READ_TASKS.to_string(),
        );

        // Robust command to find the file content
        let cmd = "root=\"${AOC_PROJECT_ROOT:-${ZELLIJ_PROJECT_ROOT:-}}\"; \
                   if [ -z \"$root\" ] && [ -f \"${XDG_STATE_HOME:-$HOME/.local/state}/aoc/project_root\" ]; then \
                       root=$(cat \"${XDG_STATE_HOME:-$HOME/.local/state}/aoc/project_root\"); \
                   fi; \
                   if [ -z \"$root\" ]; then root=\".\"; fi; \
                   echo \"PATH:$root/.taskmaster/tasks/tasks.json\"; \
                   cat \"$root/.taskmaster/tasks/tasks.json\"";

        run_command(&["sh", "-c", cmd], context);
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
                // Ignore error if we read via fs successfully? No, fs check failed to reach here.
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
                }
            }
        }
    }

    fn save_project_via_command(&mut self, path: &PathBuf, json: &str) {
        let path_str = path.to_string_lossy();
        // Use HEREDOC with quoted delimiter to prevent variable expansion and handle quotes
        let cmd = format!(
            "cat <<'EOF_AOC_TASKMASTER' > \"{}\"\n{}\nEOF_AOC_TASKMASTER",
            path_str, json
        );
        let mut context = BTreeMap::new();
        context.insert("action".to_string(), "save".to_string());
        run_command(&["sh", "-c", &cmd], context);
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
                self.show_detail = !self.show_detail;
                if !self.show_detail {
                    self.focus = FocusMode::List;
                }
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
            _ => false,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.needs_render = true;
    }

    pub fn take_render(&mut self) -> bool {
        let val = self.needs_render;
        self.needs_render = false;
        val
    }
}
