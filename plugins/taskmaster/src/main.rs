use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::time::SystemTime;
use zellij_tile::prelude::*;

#[derive(Default)]
struct State {
    tasks: Vec<Task>,
    filtered: Vec<usize>,
    selected: usize,
    filter: FilterMode,
    show_detail: bool,
    last_error: Option<String>,
    last_mtime: Option<SystemTime>,
    refresh_secs: f64,
    permissions_granted: bool,
    current_tag: String,
    task_root: Option<TaskRoot>,
    pending_tasks: bool,
    pending_state: bool,
    pending_root: bool,
    cwd: Option<PathBuf>,
    root: Option<PathBuf>,
    root_file: Option<PathBuf>,
    roots: Vec<PathBuf>,
    root_index: usize,
}

register_plugin!(State);

#[derive(Debug, Deserialize, Clone)]
struct TaskRoot {
    #[serde(flatten)]
    tags: BTreeMap<String, TaskTag>,
}

#[derive(Debug, Deserialize, Clone)]
struct TaskTag {
    #[serde(default)]
    tasks: Vec<Task>,
}

#[derive(Debug, Deserialize, Clone)]
struct Task {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    details: String,
    #[serde(default, rename = "testStrategy")]
    test_strategy: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    priority: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    subtasks: Vec<Subtask>,
}

#[derive(Debug, Deserialize, Clone)]
struct Subtask {
    id: u64,
    title: String,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterMode {
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
    fn label(self) -> &'static str {
        match self {
            FilterMode::All => "all",
            FilterMode::Pending => "pending",
            FilterMode::Done => "done",
        }
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.refresh_secs = configuration
            .get("refresh_secs")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0)
            .unwrap_or(5.0);
        self.cwd = configuration
            .get("cwd")
            .map(PathBuf::from)
            .or_else(|| env_path("AOC_PROJECT_ROOT"))
            .or_else(|| env_path("ZELLIJ_PROJECT_ROOT"));
        self.root = configuration
            .get("root")
            .map(PathBuf::from)
            .or_else(|| env_path("AOC_PROJECT_ROOT"))
            .or_else(|| env_path("ZELLIJ_PROJECT_ROOT"));
        self.root_file = configuration
            .get("root_file")
            .map(PathBuf::from)
            .or_else(|| env_path("AOC_PROJECT_ROOT_FILE"))
            .or_else(default_root_file);
        self.roots = parse_roots(configuration.get("roots"));
        if let Some(root) = self.root.clone() {
            self.set_root(root);
        } else if let Some(cwd) = self.cwd.clone() {
            if self.roots.is_empty() {
                self.set_root(cwd);
            }
        }
        set_selectable(true);
        subscribe(&[
            EventType::Timer,
            EventType::Key,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
        ]);
        request_permission(&[PermissionType::RunCommands]);
        set_timeout(self.refresh_secs);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::Timer(_) => {
                self.refresh();
                set_timeout(self.refresh_secs);
                true
            }
            Event::Key(key) => self.handle_key(key),
            Event::RunCommandResult(_, stdout, stderr, context) => {
                self.handle_command_result(stdout, stderr, context);
                true
            }
            Event::PermissionRequestResult(status) => {
                match status {
                    PermissionStatus::Granted => {
                        self.permissions_granted = true;
                        self.pending_root = false;
                        self.pending_tasks = false;
                        self.pending_state = false;
                        self.last_error = None;
                        self.refresh();
                    }
                    PermissionStatus::Denied => {
                        self.permissions_granted = false;
                        self.last_error = Some("RunCommands permission denied.".to_string());
                    }
                }
                self.invalidate_render();
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let mut lines = Vec::new();
        let tags_line = self.render_tags_line();
        let total_tasks = self.tasks.len();
        let header = format!(
            "{}TASKMASTER{}  Tag: {}{}{}  Filter: {}{}{}  Tasks: {}{}/{}{}",
            COLOR_BOLD,
            COLOR_RESET,
            COLOR_CYAN,
            if self.current_tag.is_empty() {
                "?"
            } else {
                &self.current_tag
            },
            COLOR_RESET,
            COLOR_YELLOW,
            self.filter.label(),
            COLOR_RESET,
            COLOR_GREEN,
            self.filtered.len(),
            total_tasks,
            COLOR_RESET
        );
        lines.push(truncate_visible(&header, cols));
        if let Some(root_line) = self.render_root_line() {
            lines.push(truncate_visible(&root_line, cols));
        }
        if let Some(tags) = tags_line {
            lines.push(truncate_visible(&tags, cols));
        }
        let keys_line = "Keys: [a] all  [p] pending  [d] done  [j/k] move  [enter] detail  [x] done  [o] reopen  [[] prev tag  []] next tag  [t] next root  [r] refresh";
        lines.push(truncate_visible(keys_line, cols));
        lines.push(String::new());

        if let Some(err) = &self.last_error {
            let msg = format!("{}Last error:{} {}", COLOR_RED, COLOR_RESET, err);
            lines.push(truncate_visible(&msg, cols));
            lines.push(String::new());
        }

        if self.filtered.is_empty() {
            lines.push("No tasks found.".to_string());
            emit_lines_if_changed(self, lines, rows, cols);
            return;
        }

        lines.push(truncate_visible("  S  ID  Status       Pri   Title", cols));
        lines.push(truncate_visible("  -  --  -----------  ----  ----------------------------------------", cols));

        for (idx, task_index) in self.filtered.iter().enumerate() {
            if lines.len() >= rows {
                break;
            }
            let task = &self.tasks[*task_index];
            let marker = if idx == self.selected { ">" } else { " " };
            let status_pad = pad_right(&task.status, 11);
            let prio_pad = pad_right(&task.priority, 4);
            let symbol = status_symbol(&task.status);
            let status_col = colorize_status(&status_pad, &task.status);
            let prio_col = colorize_priority(&prio_pad, &task.priority);
            let row = format!(
                "{} {} {:>3}  {}  {}  {}",
                marker,
                symbol,
                task.id,
                status_col,
                prio_col,
                task.title
            );
            let row = truncate_visible(&row, cols);
            lines.push(row);
        }

        if self.show_detail {
            if lines.len() < rows {
                lines.push(String::new());
            }
            if let Some(task) = self.selected_task() {
                lines.push(truncate_visible(
                    &format!("Details for #{}: {}", task.id, task.title),
                    cols,
                ));
                if !task.description.is_empty() {
                    lines.extend(wrap_block("Description", &task.description, cols));
                }
                if !task.details.is_empty() {
                    lines.extend(wrap_block("Details", &task.details, cols));
                }
                if !task.test_strategy.is_empty() {
                    lines.extend(wrap_block("Test", &task.test_strategy, cols));
                }
                if !task.dependencies.is_empty() {
                    lines.push(truncate_visible(
                        &format!("Dependencies: {}", task.dependencies.join(", ")),
                        cols,
                    ));
                }
                if !task.subtasks.is_empty() {
                    lines.push(truncate_visible("Subtasks:", cols));
                    for sub in &task.subtasks {
                        let subline = format!("  {}.{} {:<40} {}", task.id, sub.id, sub.title, sub.status);
                        lines.push(truncate_visible(&subline, cols));
                        if lines.len() >= rows {
                            break;
                        }
                    }
                }
            }
        }

        emit_lines_if_changed(self, lines, rows, cols);
    }
}

impl State {
    const CTX_ACTION: &'static str = "aoc_taskmaster_action";
    const ACTION_READ_TASKS: &'static str = "read_tasks";
    const ACTION_READ_STATE: &'static str = "read_state";
    const ACTION_READ_ROOT: &'static str = "read_root";

    fn refresh(&mut self) {
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

    fn request_root_via_command(&mut self) -> bool {
        if self.pending_root {
            return true;
        }
        let mut context = BTreeMap::new();
        context.insert(Self::CTX_ACTION.to_string(), Self::ACTION_READ_ROOT.to_string());
        run_command_with_env_variables_and_cwd(
            &[
                "sh",
                "-lc",
                "root_file=\"${AOC_PROJECT_ROOT_FILE:-${XDG_STATE_HOME:-$HOME/.local/state}/aoc/project_root}\"; \
                  root=\"${AOC_PROJECT_ROOT:-${ZELLIJ_PROJECT_ROOT:-}}\"; \
                  if [ -n \"$root\" ]; then printf '%s' \"$root\"; \
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

    fn handle_command_result(
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
            self.invalidate_render();
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
                self.invalidate_render();
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
        self.filtered = self
            .tasks
            .iter()
            .enumerate()
            .filter_map(|(idx, task)| {
                let status = task.status.to_lowercase();
                match self.filter {
                    FilterMode::All => Some(idx),
                    FilterMode::Pending => {
                        if status == "pending" || status == "in-progress" || status == "review" {
                            Some(idx)
                        } else {
                            None
                        }
                    }
                    FilterMode::Done => {
                        if status == "done" || status == "cancelled" {
                            Some(idx)
                        } else {
                            None
                        }
                    }
                }
            })
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        if !key.key_modifiers.is_empty() {
            return false;
        }
        match key.bare_key {
            BareKey::Char('a') => {
                self.filter = FilterMode::All;
                self.apply_filter();
                self.invalidate_render();
                true
            }
            BareKey::Char('p') => {
                self.filter = FilterMode::Pending;
                self.apply_filter();
                self.invalidate_render();
                true
            }
            BareKey::Char('d') => {
                self.filter = FilterMode::Done;
                self.apply_filter();
                self.invalidate_render();
                true
            }
            BareKey::Char('j') | BareKey::Down => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                    self.invalidate_render();
                }
                true
            }
            BareKey::Char('k') | BareKey::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.invalidate_render();
                }
                true
            }
            BareKey::Char('[') => {
                self.select_prev_tag();
                self.invalidate_render();
                true
            }
            BareKey::Char(']') => {
                self.select_next_tag();
                self.invalidate_render();
                true
            }
            BareKey::Enter => {
                self.show_detail = !self.show_detail;
                self.invalidate_render();
                true
            }
            BareKey::Char('r') => {
                self.last_mtime = None;
                self.refresh();
                self.invalidate_render();
                true
            }
            BareKey::Char('t') => {
                self.select_next_root();
                self.invalidate_render();
                true
            }
            BareKey::Char('x') => {
                if let Some(task) = self.selected_task() {
                    exec_cmd(&[
                        "task-master",
                        "set-status",
                        &task.id,
                        "done",
                    ]);
                }
                true
            }
            BareKey::Char('o') => {
                if let Some(task) = self.selected_task() {
                    exec_cmd(&[
                        "task-master",
                        "set-status",
                        &task.id,
                        "pending",
                    ]);
                }
                true
            }
            _ => false,
        }
    }

    fn selected_task(&self) -> Option<&Task> {
        self.filtered
            .get(self.selected)
            .and_then(|idx| self.tasks.get(*idx))
    }

    fn render_tags_line(&self) -> Option<String> {
        let root = self.task_root.as_ref()?;
        if root.tags.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for tag in root.tags.keys() {
            if tag == &self.current_tag {
                parts.push(format!("{}[{}]{}", COLOR_CYAN, tag, COLOR_RESET));
            } else {
                parts.push(tag.to_string());
            }
        }
        let line = format!("Tags: {}", parts.join("  "));
        Some(line)
    }

    fn render_root_line(&self) -> Option<String> {
        let root = self.root.as_ref()?;
        Some(format!("Root: {}{}{}", COLOR_BLUE, root.to_string_lossy(), COLOR_RESET))
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

    fn invalidate_render(&mut self) {
    }
}

const COLOR_RESET: &str = "\x1b[0m";
const COLOR_BOLD: &str = "\x1b[1m";
const COLOR_RED: &str = "\x1b[31m";
const COLOR_GREEN: &str = "\x1b[32m";
const COLOR_YELLOW: &str = "\x1b[33m";
const COLOR_BLUE: &str = "\x1b[34m";
const COLOR_CYAN: &str = "\x1b[36m";
const COLOR_DIM: &str = "\x1b[2m";

fn status_symbol(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "done" => format!("{}X{}", COLOR_GREEN, COLOR_RESET),
        "cancelled" => format!("{}!{}", COLOR_RED, COLOR_RESET),
        "in-progress" => format!("{}+{}", COLOR_BLUE, COLOR_RESET),
        "review" => format!("{}?{}", COLOR_CYAN, COLOR_RESET),
        "pending" => format!("{}*{}", COLOR_YELLOW, COLOR_RESET),
        _ => format!("{}~{}", COLOR_DIM, COLOR_RESET),
    }
}

fn colorize_status(label: &str, status: &str) -> String {
    let color = match status.to_lowercase().as_str() {
        "done" => COLOR_GREEN,
        "cancelled" => COLOR_RED,
        "in-progress" => COLOR_BLUE,
        "review" => COLOR_CYAN,
        "pending" => COLOR_YELLOW,
        _ => COLOR_DIM,
    };
    format!("{}{}{}", color, label, COLOR_RESET)
}

fn colorize_priority(label: &str, priority: &str) -> String {
    let color = match priority.to_lowercase().as_str() {
        "high" => COLOR_RED,
        "medium" => COLOR_YELLOW,
        "low" => COLOR_BLUE,
        _ => COLOR_DIM,
    };
    format!("{}{}{}", color, label, COLOR_RESET)
}

fn pad_right(input: &str, width: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in input.chars() {
        if count >= width {
            break;
        }
        out.push(ch);
        count += 1;
    }
    while count < width {
        out.push(' ');
        count += 1;
    }
    out
}

fn wrap_block(label: &str, value: &str, cols: usize) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(truncate_visible(&format!("{}:", label), cols));
    let width = cols.saturating_sub(2);
    if width == 0 {
        return lines;
    }
    for raw in value.lines() {
        let mut buf = String::new();
        let mut count = 0usize;
        for ch in raw.chars() {
            if count >= width {
                lines.push(truncate_visible(&format!("  {}", buf), cols));
                buf.clear();
                count = 0;
            }
            buf.push(ch);
            count += 1;
        }
        if !buf.is_empty() || raw.is_empty() {
            lines.push(truncate_visible(&format!("  {}", buf), cols));
        }
    }
    lines
}

fn truncate_visible(input: &str, cols: usize) -> String {
    if cols == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut visible = 0usize;
    let mut in_escape = false;
    for ch in input.chars() {
        if in_escape {
            out.push(ch);
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            out.push(ch);
            continue;
        }
        if visible >= cols {
            break;
        }
        out.push(ch);
        visible += 1;
    }
    out
}

fn emit_lines_if_changed(_state: &mut State, lines: Vec<String>, rows: usize, cols: usize) {
    let mut padded = Vec::with_capacity(rows);
    for line in lines {
        if padded.len() >= rows {
            break;
        }
        padded.push(truncate_visible(&line, cols));
    }
    while padded.len() < rows {
        padded.push(String::new());
    }
    for line in &padded {
        println!("{}", line);
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

fn env_path(key: &str) -> Option<PathBuf> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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
