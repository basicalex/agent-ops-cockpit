use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::time::SystemTime;
use zellij_tile::prelude::*;

use crate::model::{Task, TaskRoot};
use crate::theme::colors;
use crate::ui::*;

#[derive(Default)]
pub struct State {
    pub tasks: Vec<Task>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter: FilterMode,
    pub search_query: String,
    pub root_query: String,          // New: for editing root path
    pub is_searching: bool,
    pub input_mode: InputMode,      // New: track what we are inputting
    pub show_detail: bool,
    pub focus: FocusMode,           // New: Track which pane has focus
    pub subtask_cursor: usize,      // New: Selected subtask index
    pub last_error: Option<String>,
    pub last_mtime: Option<SystemTime>,
    pub refresh_secs: f64,
    pub permissions_granted: bool,
    pub current_tag: String,
    pub task_root: Option<TaskRoot>,
    pub pending_tasks: bool,
    pub pending_state: bool,
    pub pending_root: bool,
    pub cwd: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub root_file: Option<PathBuf>,
    pub roots: Vec<PathBuf>,
    pub root_index: usize,
    pub ignore_refresh_until: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    List,
    Details,
}

impl Default for FocusMode {
    fn default() -> Self {
        FocusMode::List
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    None,
    Search,
    Root,
}

impl Default for InputMode {
    fn default() -> Self {
        InputMode::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    All,
    Pending,
    Done,
}

impl Default for FilterMode {
    fn default() -> Self {
        FilterMode::All
    }
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

    pub fn load_config(&mut self, configuration: BTreeMap<String, String>) {
        self.refresh_secs = configuration
            .get("refresh_secs")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0)
            .unwrap_or(5.0);
        self.cwd = configuration
            .get("cwd")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        self.root = configuration
            .get("root")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        self.root_file = configuration
            .get("root_file")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| self.env_path("AOC_PROJECT_ROOT_FILE"))
            .or_else(default_root_file);
        self.roots = parse_roots(configuration.get("roots"));
        if let Some(root) = self.root.clone() {
            self.set_root(root);
        } else if let Some(cwd) = self.cwd.clone() {
            if self.roots.is_empty() {
                self.set_root(cwd);
            }
        } else if let Ok(cwd) = env::current_dir() {
             if self.roots.is_empty() {
                 self.set_root(cwd);
             }
        }
    }

    pub fn refresh(&mut self) {
        if !self.permissions_granted {
            return;
        }
        if self.root.is_none() {
            if self.request_root_via_command() {
                return;
            }
        }
        if self.root.is_some() || self.cwd.is_some() {
            self.request_state_via_command();
            self.request_tasks_via_command();
            return;
        }
    }

    fn check_ignore_refresh(&mut self) -> bool {
        if let Some(until) = self.ignore_refresh_until {
            if until.duration_since(SystemTime::now()).is_ok() {
                // 'until' is in the future
                return false;
            } else {
                self.ignore_refresh_until = None;
            }
        }
        true
    }

    fn request_root_via_command(&mut self) -> bool {
        if self.pending_root {
            return true;
        }
        if !self.check_ignore_refresh() {
            return true; // Pretend we are pending/busy to avoid loops, but do nothing
        }
        let mut context = BTreeMap::new();
        context.insert(Self::CTX_ACTION.to_string(), Self::ACTION_READ_ROOT.to_string());
        run_command_with_env_variables_and_cwd(
            &[
                "sh",
                "-c",
                "root_file=\"${AOC_PROJECT_ROOT_FILE:-${XDG_STATE_HOME:-$HOME/.local/state}/aoc/project_root}\"; \
                  root=\"${AOC_PROJECT_ROOT:-${ZELLIJ_PROJECT_ROOT:-}}\"; \
                  if [ -n \"$root\" ]; then printf \'%s\' \"$root\"; \
                  elif [ -f \"$root_file\" ]; then cat \"$root_file\"; \
                  else pwd; fi",
            ],
            BTreeMap::new(),
            PathBuf::from("/"),
            context,
        );
        self.pending_root = true;
        true
    }

    fn request_tasks_via_command(&mut self) {
        if self.pending_tasks {
            return;
        }
        if !self.check_ignore_refresh() {
            return;
        }
        let mut context = BTreeMap::new();
        context.insert(Self::CTX_ACTION.to_string(), Self::ACTION_READ_TASKS.to_string());
        if let Some(root) = self.root.clone() {
            run_command_with_env_variables_and_cwd(
                &["cat", ".taskmaster/tasks/tasks.json"],
                BTreeMap::new(),
                root,
                context,
            );
        } else if let Some(cwd) = self.cwd.clone() {
            run_command_with_env_variables_and_cwd(
                &["cat", ".taskmaster/tasks/tasks.json"],
                BTreeMap::new(),
                cwd,
                context,
            );
        } else {
            run_command(&["cat", ".taskmaster/tasks/tasks.json"], context);
        }
        self.pending_tasks = true;
    }

    fn request_state_via_command(&mut self) {
        if self.pending_state {
            return;
        }
        if !self.check_ignore_refresh() {
            return;
        }
        let mut context = BTreeMap::new();
        context.insert(Self::CTX_ACTION.to_string(), Self::ACTION_READ_STATE.to_string());
        if let Some(root) = self.root.clone() {
            run_command_with_env_variables_and_cwd(
                &["cat", ".taskmaster/state.json"],
                BTreeMap::new(),
                root,
                context,
            );
        } else if let Some(cwd) = self.cwd.clone() {
            run_command_with_env_variables_and_cwd(
                &["cat", ".taskmaster/state.json"],
                BTreeMap::new(),
                cwd,
                context,
            );
        } else {
            run_command(&["cat", ".taskmaster/state.json"], context);
        }
        self.pending_state = true;
    }

    pub fn handle_command_result(
        &mut self,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        context: BTreeMap<String, String>,
    ) {
        let action = context.get(Self::CTX_ACTION).map(String::as_str);
        if action == Some(Self::ACTION_READ_ROOT) {
            self.pending_root = false;
            if !stderr.is_empty() {
                self.last_error = Some(String::from_utf8_lossy(&stderr).to_string());
                return;
            }
            let data = String::from_utf8_lossy(&stdout).to_string();
            let trimmed = data.trim();
            if !trimmed.is_empty() {
                self.set_root(PathBuf::from(trimmed));
                self.last_mtime = None;
                self.refresh();
            }
            return;
        }
        if action == Some(Self::ACTION_READ_TASKS) {
            self.pending_tasks = false;
            if !stderr.is_empty() {
                self.last_error = Some(String::from_utf8_lossy(&stderr).to_string());
                return;
            }
            let data = String::from_utf8_lossy(&stdout).to_string();
            self.update_tasks_from_json(&data);
            self.apply_current_tag();
            self.last_error = None;
            return;
        }
        if action == Some(Self::ACTION_READ_STATE) {
            self.pending_state = false;
            if !stderr.is_empty() {
                self.last_error = Some(String::from_utf8_lossy(&stderr).to_string());
                return;
            }
            let data = String::from_utf8_lossy(&stdout).to_string();
            if let Some(tag) = Self::read_tag_from_json(&data) {
                self.current_tag = tag;
                self.apply_current_tag();
                self.last_error = None;
            }
            return;
        }

        if !stderr.is_empty() {
            self.last_error = Some(String::from_utf8_lossy(&stderr).to_string());
        } else if !stdout.is_empty() {
            self.last_error = Some(String::from_utf8_lossy(&stdout).to_string());
        }
    }

    fn update_tasks_from_json(&mut self, data: &str) {
        let parsed: Result<TaskRoot, _> = serde_json::from_str(data);
        match parsed {
            Ok(root) => {
                self.task_root = Some(root);
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(format!("Failed to parse tasks.json: {}", err));
            }
        }
    }

    fn apply_current_tag(&mut self) {
        let Some(root) = self.task_root.as_ref() else {
            return;
        };
        let requested = if self.current_tag.is_empty() {
            "master".to_string()
        } else {
            self.current_tag.clone()
        };
        let fallback = root.tags.keys().next().cloned();
        let tag = if root.tags.contains_key(&requested) {
            requested
        } else {
            fallback.unwrap_or(requested)
        };
        self.current_tag = tag.clone();
        self.tasks = root
            .tags
            .get(&tag)
            .map(|tag| tag.tasks.clone())
            .unwrap_or_default();
        self.apply_filter();
    }

    fn read_tag_from_json(data: &str) -> Option<String> {
        let json: serde_json::Value = serde_json::from_str(data).ok()?;
        json.get("currentTag")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn apply_filter(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered = self
            .tasks
            .iter()
            .enumerate()
            .filter_map(|(idx, task)| {
                let status = task.status.to_lowercase();
                let status_match = match self.filter {
                    FilterMode::All => true,
                    FilterMode::Pending => {
                        status == "pending" || status == "in-progress" || status == "review"
                    }
                    FilterMode::Done => {
                        status == "done" || status == "cancelled"
                    }
                };

                if !status_match {
                    return None;
                }

                if !query.is_empty() {
                    let title = task.title.to_lowercase();
                    let desc = task.description.to_lowercase();
                    if !title.contains(&query) && !desc.contains(&query) {
                        return None;
                    }
                }

                Some(idx)
            })
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        if self.last_error.is_some() {
            match key.bare_key {
                BareKey::Enter | BareKey::Esc => {
                    self.clear_error();
                    self.refresh(); // Try to reload
                    return true;
                }
                _ => return true, // Consume all keys when error is shown
            }
        }

        match self.input_mode {
            InputMode::Search => {
                match key.bare_key {
                    BareKey::Enter | BareKey::Esc => {
                        self.input_mode = InputMode::None;
                        self.is_searching = false;
                        return true;
                    }
                    BareKey::Backspace => {
                        self.search_query.pop();
                        self.apply_filter();
                        return true;
                    }
                    BareKey::Char(c) => {
                        self.search_query.push(c);
                        self.apply_filter();
                        return true;
                    }
                    _ => return true,
                }
            }
            InputMode::Root => {
                match key.bare_key {
                    BareKey::Enter => {
                        let new_root = self.root_query.trim().to_string();
                        if !new_root.is_empty() {
                            self.set_root(PathBuf::from(new_root));
                            self.last_mtime = None;
                            self.refresh();
                        }
                        self.input_mode = InputMode::None;
                        return true;
                    }
                    BareKey::Esc => {
                        self.input_mode = InputMode::None;
                        return true;
                    }
                    BareKey::Backspace => {
                        self.root_query.pop();
                        return true;
                    }
                    BareKey::Char(c) => {
                        self.root_query.push(c);
                        return true;
                    }
                    _ => return true,
                }
            }
            InputMode::None => {}
        }

        if !key.key_modifiers.is_empty() {
            return false;
        }

        // Global keys
        match key.bare_key {
             BareKey::Tab => {
                 if self.show_detail {
                     self.focus = match self.focus {
                         FocusMode::List => FocusMode::Details,
                         FocusMode::Details => FocusMode::List,
                     };
                     // Reset cursor when entering details
                     if self.focus == FocusMode::Details {
                         self.subtask_cursor = 0;
                     }
                 }
                 return true;
             }
             _ => {}
        }

        // Details Focus Mode
        if self.focus == FocusMode::Details {
            match key.bare_key {
                BareKey::Char('j') | BareKey::Down => {
                    if let Some(task) = self.selected_task() {
                        if self.subtask_cursor + 1 < task.subtasks.len() {
                            self.subtask_cursor += 1;
                            return true;
                        }
                    }
                }
                BareKey::Char('k') | BareKey::Up => {
                    if self.subtask_cursor > 0 {
                        self.subtask_cursor -= 1;
                        return true;
                    }
                }
                BareKey::Char(' ') | BareKey::Char('x') => {
                    self.optimistic_toggle_subtask();
                    return true;
                }
                BareKey::Char('e') => {
                    if let Some(task) = self.selected_task() {
                         let root = self.root.as_ref().or(self.cwd.as_ref());
                         let root_str = root.map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".to_string());
                         
                        // Open a new pane to edit the task, ensuring we CD first
                        exec_cmd(&["bash", "-c", &format!(
                            "cd \"{}\" && zellij run --name \"Edit Task #{}\" -- task-master update {} --edit", 
                            root_str, task.id, task.id
                        )]);
                    }
                    return true;
                }
                BareKey::Esc => {
                    self.focus = FocusMode::List;
                    return true;
                }
                _ => {}
            }
            return false; // Don't fall through to List keys if focused on Details
        }

        // List Focus Mode (Standard)
        match key.bare_key {
            BareKey::Char('/') => {
                self.input_mode = InputMode::Search;
                self.is_searching = true;
                true
            }
            BareKey::Char('C') => {
                self.root_query = self.root.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                self.input_mode = InputMode::Root;
                true
            }
            BareKey::Char('f') => {
                self.filter = self.filter.next();
                self.apply_filter();
                true
            }
            BareKey::Char('a') => {
                self.optimistic_toggle_agent();
                true
            }
            BareKey::Char('p') => {
                self.filter = FilterMode::Pending;
                self.apply_filter();
                true
            }
            BareKey::Char('d') => {
                self.filter = FilterMode::Done;
                self.apply_filter();
                true
            }
            BareKey::Char('j') | BareKey::Down => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                    return true;
                }
                false
            }
            BareKey::Char('k') | BareKey::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    return true;
                }
                false
            }
            BareKey::Char('[') => {
                self.select_prev_tag();
                true
            }
            BareKey::Char(']') => {
                self.select_next_tag();
                true
            }
            BareKey::Enter => {
                self.show_detail = !self.show_detail;
                if !self.show_detail {
                    self.focus = FocusMode::List;
                } else {
                    // Auto-focus details on open? Maybe not, keep explicit Tab
                    // self.focus = FocusMode::Details; 
                }
                true
            }
            BareKey::Char('r') => {
                self.last_mtime = None;
                self.refresh();
                true
            }
            BareKey::Char('t') => {
                self.select_next_root();
                true
            }
            BareKey::Char('x') => {
                self.optimistic_toggle_status("done".to_string());
                true
            }
            BareKey::Char('o') => {
                self.optimistic_toggle_status("pending".to_string());
                true
            }
            _ => false,
        }
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.filtered
            .get(self.selected)
            .and_then(|idx| self.tasks.get(*idx))
    }

    pub fn render_tags_line(&self) -> Option<String> {
        let root = self.task_root.as_ref()?;
        if root.tags.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for tag in root.tags.keys() {
            if tag == &self.current_tag {
                parts.push(format!("{}[{}]{}", colors::CYAN, tag, colors::RESET));
            } else {
                parts.push(tag.to_string());
            }
        }
        let line = format!("Tags: {}", parts.join("  "));
        Some(line)
    }

    pub fn render_root_line(&self) -> Option<String> {
        let root = self.root.as_ref()?;
        Some(format!("Root: {}{}{}", colors::BLUE, root.to_string_lossy(), colors::RESET))
    }

    pub fn set_error(&mut self, msg: String) {
        self.last_error = Some(msg);
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    fn save_to_disk(&self, id: &str, action: &str, value: &str) {
        match action {
            "status" => {
                exec_cmd(&["task-master", "set-status", id, value]);
            }
            "active-agent" => {
                exec_cmd(&["task-master", "update", id, "--active-agent", value]);
            }
            "subtask" => {
                // value format: "SUBTASK_INDEX:STATUS" (e.g. "0:done")
                // Since task-master might not support granular subtask updates, we use python patcher
                self.save_subtask_via_python(id, value);
            }
            _ => {}
        }
    }

    fn save_subtask_via_python(&self, task_id: &str, payload: &str) {
        // payload: "index:status"
        let parts: Vec<&str> = payload.split(':').collect();
        if parts.len() != 2 { return; }
        let idx = parts[0];
        let status = parts[1];

        // Python script to patch the JSON safely
        let script = format!(
            "import json, sys; \
            path='.taskmaster/tasks/tasks.json'; \
            data=json.load(open(path)); \
            root=data.get('master', {{}}); \
            tasks=root.get('tasks', []); \
            found=next((t for t in tasks if str(t.get('id')) == '{}'), None); \
            if found and 'subtasks' in found and len(found['subtasks']) > {}: \
                found['subtasks'][{}]['status'] = '{}'; \
                json.dump(data, open(path, 'w'), indent=2); \
            ",
            task_id, idx, idx, status
        );

        exec_cmd(&["python3", "-c", &script]);
    }

    pub fn optimistic_toggle_subtask(&mut self) -> Option<()> {
        if self.focus != FocusMode::Details { return None; }
        
        let task_idx = self.filtered.get(self.selected)?;
        let task = &mut self.tasks[*task_idx];
        
        if task.subtasks.is_empty() || self.subtask_cursor >= task.subtasks.len() {
            return None;
        }

        let sub = &mut task.subtasks[self.subtask_cursor];
        let new_status = if sub.status == "done" { "pending" } else { "done" };
        sub.status = new_status.to_string();
        
        let id = task.id.clone();
        let payload = format!("{}:{}", self.subtask_cursor, new_status);
        
        self.save_to_disk(&id, "subtask", &payload);
        self.ignore_refresh_until = Some(SystemTime::now() + std::time::Duration::from_secs(2)); // Faster unlock for subtasks
        
        Some(())
    }

    pub fn optimistic_toggle_status(&mut self, status: String) -> Option<()> {
        if self.selected >= self.filtered.len() {
            return None;
        }
        let task_idx = self.filtered[self.selected];
        
        let task = &mut self.tasks[task_idx];
        if task.status == status {
            return None; 
        }
        let id = task.id.clone();
        task.status = status.clone();
        
        self.apply_filter();
        self.save_to_disk(&id, "status", &status);
        self.ignore_refresh_until = Some(SystemTime::now() + std::time::Duration::from_secs(3));
        
        Some(())
    }

    pub fn optimistic_toggle_agent(&mut self) -> Option<()> {
        if self.selected >= self.filtered.len() {
            return None;
        }
        let task_idx = self.filtered[self.selected];
        
        let task = &mut self.tasks[task_idx];
        task.active_agent = !task.active_agent;
        let id = task.id.clone();
        let val_str = if task.active_agent { "true" } else { "false" };
        
        self.save_to_disk(&id, "active-agent", val_str);
        self.ignore_refresh_until = Some(SystemTime::now() + std::time::Duration::from_secs(3));
        
        Some(())
    }

    fn select_prev_tag(&mut self) {
        let Some(root) = self.task_root.as_ref() else {
            return;
        };
        if root.tags.is_empty() {
            return;
        }
        let tags: Vec<&String> = root.tags.keys().collect();
        let current = if self.current_tag.is_empty() {
            tags[0].as_str()
        } else {
            self.current_tag.as_str()
        };
        let mut idx = tags
            .iter()
            .position(|tag| tag.as_str() == current)
            .unwrap_or(0);
        if idx == 0 {
            idx = tags.len() - 1;
        } else {
            idx -= 1;
        }
        self.current_tag = tags[idx].to_string();
        self.apply_current_tag();
    }

    fn select_next_tag(&mut self) {
        let Some(root) = self.task_root.as_ref() else {
            return;
        };
        if root.tags.is_empty() {
            return;
        }
        let tags: Vec<&String> = root.tags.keys().collect();
        let current = if self.current_tag.is_empty() {
            tags[0].as_str()
        } else {
            self.current_tag.as_str()
        };
        let idx = tags
            .iter()
            .position(|tag| tag.as_str() == current)
            .unwrap_or(0);
        let next = (idx + 1) % tags.len();
        self.current_tag = tags[next].to_string();
        self.apply_current_tag();
    }

    fn select_next_root(&mut self) {
        if self.roots.is_empty() {
            return;
        }
        self.root_index = (self.root_index + 1) % self.roots.len();
        let next = self.roots[self.root_index].clone();
        self.set_root(next);
        self.request_state_via_command();
        self.request_tasks_via_command();
    }

    fn set_root(&mut self, root: PathBuf) {
        self.root = Some(root.clone());
        if let Some(pos) = self.roots.iter().position(|item| item == &root) {
            self.root_index = pos;
        } else {
            self.roots.push(root);
            self.root_index = self.roots.len().saturating_sub(1);
        }
    }

    fn env_path(&self, key: &str) -> Option<PathBuf> {
        env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }
}

fn parse_roots(raw: Option<&String>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let Some(raw) = raw else {
        return roots;
    };
    for entry in raw.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        roots.push(PathBuf::from(trimmed));
    }
    roots
}

fn default_root_file() -> Option<PathBuf> {
    let state_home = env::var("XDG_STATE_HOME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var("HOME")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state_home.join("aoc").join("project_root"))
}
