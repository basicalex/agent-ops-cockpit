use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
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
    current_tag: String,
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
        subscribe(&[EventType::Timer, EventType::Key, EventType::RunCommandResult]);
        request_permission(&[PermissionType::RunCommands]);
        self.refresh();
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
            Event::RunCommandResult(_, stdout, stderr, _) => {
                if !stderr.is_empty() {
                    self.last_error = Some(String::from_utf8_lossy(&stderr).to_string());
                } else if !stdout.is_empty() {
                    self.last_error = Some(String::from_utf8_lossy(&stdout).to_string());
                }
                self.refresh();
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let mut lines = Vec::new();
        let header = format!(
            "Taskmaster (tag: {}) [filter: {}] [tasks: {}]",
            if self.current_tag.is_empty() {
                "?"
            } else {
                &self.current_tag
            },
            self.filter.label(),
            self.filtered.len()
        );
        lines.push(truncate(&header, cols));
        lines.push(truncate(
            "[a] all  [p] pending  [d] done  [j/k] move  [enter] detail  [x] done  [o] reopen  [r] refresh",
            cols,
        ));
        lines.push(String::new());

        if let Some(err) = &self.last_error {
            lines.push(truncate(&format!("Last error: {}", err), cols));
            lines.push(String::new());
        }

        if self.filtered.is_empty() {
            lines.push("No tasks found.".to_string());
            emit_lines(lines, rows);
            return;
        }

        for (idx, task_index) in self.filtered.iter().enumerate() {
            if lines.len() >= rows {
                break;
            }
            let task = &self.tasks[*task_index];
            let marker = if idx == self.selected { ">" } else { " " };
            let mut row = format!(
                "{} {:>3} {:<40} {:<10} {:<7}",
                marker,
                task.id,
                task.title,
                task.status,
                task.priority
            );
            row = truncate(&row, cols);
            lines.push(row);
        }

        if self.show_detail {
            if lines.len() < rows {
                lines.push(String::new());
            }
            if let Some(task) = self.selected_task() {
                lines.push(truncate(
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
                    lines.push(truncate(
                        &format!("Dependencies: {}", task.dependencies.join(", ")),
                        cols,
                    ));
                }
                if !task.subtasks.is_empty() {
                    lines.push(truncate("Subtasks:", cols));
                    for sub in &task.subtasks {
                        let subline = format!("  {}.{} {:<40} {}", task.id, sub.id, sub.title, sub.status);
                        lines.push(truncate(&subline, cols));
                        if lines.len() >= rows {
                            break;
                        }
                    }
                }
            }
        }

        emit_lines(lines, rows);
    }
}

impl State {
    fn tasks_path() -> PathBuf {
        PathBuf::from(".taskmaster/tasks/tasks.json")
    }

    fn state_path() -> PathBuf {
        PathBuf::from(".taskmaster/state.json")
    }

    fn read_current_tag() -> Option<String> {
        let path = Self::state_path();
        let data = fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&data).ok()?;
        json.get("currentTag")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn refresh(&mut self) {
        let path = Self::tasks_path();
        let metadata = fs::metadata(&path).ok();
        if let Some(meta) = metadata {
            if let Ok(modified) = meta.modified() {
                if let Some(prev) = self.last_mtime {
                    if modified <= prev {
                        return;
                    }
                }
                self.last_mtime = Some(modified);
            }
        }
        match fs::read_to_string(&path) {
            Ok(data) => {
                let parsed: Result<TaskRoot, _> = serde_json::from_str(&data);
                match parsed {
                    Ok(root) => {
                        self.current_tag = Self::read_current_tag().unwrap_or_else(|| "master".to_string());
                        let tasks = root
                            .tags
                            .get(&self.current_tag)
                            .or_else(|| root.tags.values().next())
                            .map(|tag| tag.tasks.clone())
                            .unwrap_or_default();
                        self.tasks = tasks;
                        self.apply_filter();
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_error = Some(format!("Failed to parse tasks.json: {}", err));
                    }
                }
            }
            Err(err) => {
                self.last_error = Some(format!("Failed to read tasks.json: {}", err));
            }
        }
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
                }
                true
            }
            BareKey::Char('k') | BareKey::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            BareKey::Enter => {
                self.show_detail = !self.show_detail;
                true
            }
            BareKey::Char('r') => {
                self.last_mtime = None;
                self.refresh();
                true
            }
            BareKey::Char('x') => {
                if let Some(task) = self.selected_task() {
                    exec_cmd(&[
                        "task-master",
                        "set-status",
                        "--id",
                        &task.id,
                        "--status",
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
                        "--id",
                        &task.id,
                        "--status",
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
}

fn wrap_block(label: &str, value: &str, cols: usize) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(truncate(&format!("{}:", label), cols));
    let width = cols.saturating_sub(2);
    for raw in value.lines() {
        let mut start = 0;
        let bytes = raw.as_bytes();
        while start < bytes.len() {
            let end = (start + width).min(bytes.len());
            let slice = &raw[start..end];
            lines.push(truncate(&format!("  {}", slice), cols));
            start = end;
        }
    }
    lines
}

fn truncate(input: &str, cols: usize) -> String {
    if cols == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (i, ch) in input.chars().enumerate() {
        if i >= cols {
            break;
        }
        out.push(ch);
    }
    out
}

fn emit_lines(lines: Vec<String>, rows: usize) {
    let mut count = 0;
    for line in lines {
        if count >= rows {
            break;
        }
        println!("{}", line);
        count += 1;
    }
    while count < rows {
        println!();
        count += 1;
    }
}
