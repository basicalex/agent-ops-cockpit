use anyhow::{Context, Result};
use aoc_core::{ProjectData, TagContext, Task, TaskStatus};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    widgets::{ListState, TableState},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const ALL_TAG_VIEW: &str = "__all__";

#[derive(Debug, Clone, Default)]
pub struct DisplayRow {
    pub task_idx: usize,
    pub subtask_path: Vec<usize>,
    pub depth: usize,
    pub tag_name: String,
}

#[derive(Debug, Clone)]
pub struct TagItem {
    pub name: String,
    pub total: usize,
    pub done: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusMode {
    #[default]
    List,
    Details,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    All,
    Pending,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    TaskNumber,
    Tag,
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

pub struct App {
    pub root: PathBuf,
    pub tasks_path: PathBuf,
    pub state_path: PathBuf,
    pub project: Option<ProjectData>,
    pub tasks: Vec<Task>,
    pub task_tags: Vec<String>,
    pub display_rows: Vec<DisplayRow>,
    pub expanded_tasks: HashSet<String>,
    pub table_state: TableState,
    pub filter: FilterMode,
    pub sort_mode: SortMode,
    pub current_tag: String,
    pub show_detail: bool,
    pub show_help: bool,
    pub show_tag_selector: bool,
    pub focus: FocusMode,
    pub last_error: Option<String>,
    pub last_tasks_mtime: Option<SystemTime>,
    pub last_state_mtime: Option<SystemTime>,
    pub last_state_updated: Option<String>,
    pub list_area: Option<Rect>,
    pub details_area: Option<Rect>,
    pub details_scroll: u16,
    pub details_max_scroll: u16,
    pub last_title: Option<String>,
    pub dirty: bool,
    pub should_quit: bool,
    pub tag_items: Vec<TagItem>,
    pub tag_list_state: ListState,
    pub tag_color_seed: u64,
}

#[derive(Debug, Clone)]
struct SelectionKey {
    tag_name: String,
    task_id: String,
    subtask_id: Option<u32>,
}

pub fn resolve_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    if let Some(root) = find_taskmaster_root(&cwd) {
        return Ok(root);
    }

    if let Ok(root) = std::env::var("AOC_PROJECT_ROOT") {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            let root_path = PathBuf::from(trimmed);
            if is_taskmaster_root(&root_path) {
                return Ok(root_path);
            }
        }
    }

    Ok(cwd)
}

fn find_taskmaster_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(path) = current {
        if is_taskmaster_root(path) {
            return Some(path.to_path_buf());
        }
        current = path.parent();
    }
    None
}

fn is_taskmaster_root(path: &Path) -> bool {
    let tasks_path = path.join(".taskmaster/tasks/tasks.json");
    if tasks_path.exists() {
        return true;
    }
    let state_path = path.join(".taskmaster/state.json");
    if state_path.exists() {
        return true;
    }
    let tm_dir = path.join(".taskmaster");
    tm_dir.is_dir()
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

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::TaskNumber => "task#",
            SortMode::Tag => "tag",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SortMode::TaskNumber => SortMode::Tag,
            SortMode::Tag => SortMode::TaskNumber,
        }
    }
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        let tasks_path = root.join(".taskmaster/tasks/tasks.json");
        let state_path = root.join(".taskmaster/state.json");

        Self {
            root,
            tasks_path,
            state_path,
            project: None,
            tasks: Vec::new(),
            task_tags: Vec::new(),
            display_rows: Vec::new(),
            expanded_tasks: HashSet::new(),
            table_state: TableState::default(),
            filter: FilterMode::All,
            sort_mode: SortMode::TaskNumber,
            current_tag: String::new(),
            show_detail: false,
            show_help: false,
            show_tag_selector: false,
            focus: FocusMode::List,
            last_error: None,
            last_tasks_mtime: None,
            last_state_mtime: None,
            last_state_updated: None,
            list_area: None,
            details_area: None,
            details_scroll: 0,
            details_max_scroll: 0,
            last_title: None,
            dirty: false,
            should_quit: false,
            tag_items: Vec::new(),
            tag_list_state: ListState::default(),
            tag_color_seed: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn on_tick(&mut self) {
        self.refresh(false);
    }

    pub fn refresh(&mut self, force: bool) {
        self.read_state(force);
        self.read_tasks(force);
        self.sync_tag_items();
        self.maybe_update_pane_title();
    }

    pub fn sync_pane_title(&mut self) {
        self.last_title = None;
        self.maybe_update_pane_title();
    }

    fn read_state(&mut self, force: bool) {
        let Ok(metadata) = std::fs::metadata(&self.state_path) else {
            return;
        };
        let modified = metadata.modified().ok();
        if !force && self.last_state_mtime == modified {
            return;
        }

        let Ok(content) = std::fs::read_to_string(&self.state_path) else {
            return;
        };
        let Ok(state) = serde_json::from_str::<TaskmasterState>(&content) else {
            return;
        };

        if sync_tags_enabled() {
            if let Some(tag) = state.current_tag.clone() {
                if self.current_tag != ALL_TAG_VIEW && tag != self.current_tag {
                    self.current_tag = tag;
                    self.apply_current_tag();
                }
            }
        }
        if let Some(last_updated) = state.last_updated {
            if self.last_state_updated.as_ref() != Some(&last_updated) {
                self.last_state_updated = Some(last_updated);
            }
        }

        self.last_state_mtime = modified;
    }

    fn read_tasks(&mut self, force: bool) {
        if !self.tasks_path.exists() {
            self.set_error(format!(
                "tasks.json not found at {}",
                self.tasks_path.display()
            ));
            self.project = None;
            self.tasks.clear();
            self.task_tags.clear();
            self.display_rows.clear();
            return;
        }

        let metadata = match std::fs::metadata(&self.tasks_path) {
            Ok(meta) => meta,
            Err(err) => {
                self.set_error(format!("Failed to read tasks metadata: {err}"));
                return;
            }
        };

        let modified = metadata.modified().ok();
        if !force && self.last_tasks_mtime == modified {
            return;
        }

        let content = match std::fs::read_to_string(&self.tasks_path) {
            Ok(data) => data,
            Err(err) => {
                self.set_error(format!("Failed to read tasks.json: {err}"));
                return;
            }
        };

        let project = match parse_project_compat(&content) {
            Ok(data) => data,
            Err(err) => {
                self.set_error(format!("Failed to parse tasks.json: {err}"));
                return;
            }
        };

        if let Err(err) = validate_project(&project) {
            self.set_error(err);
            return;
        }

        self.project = Some(project);
        self.last_tasks_mtime = modified;
        self.last_error = None;
        self.apply_current_tag();
    }

    fn set_error(&mut self, message: String) {
        self.last_error = Some(message);
        self.mark_dirty();
    }

    fn apply_current_tag(&mut self) {
        let selection = self.current_selection_key();
        let Some(project) = &self.project else {
            return;
        };

        let tag = self.current_tag_or_default();
        if self.current_tag.is_empty() {
            self.current_tag = tag.clone();
        }

        if tag == ALL_TAG_VIEW {
            let mut combined: Vec<(String, Task)> = Vec::new();
            let mut tag_names: Vec<String> = project.tags.keys().cloned().collect();
            tag_names.sort();

            for tag_name in tag_names {
                if let Some(ctx) = project.tags.get(&tag_name) {
                    for task in &ctx.tasks {
                        combined.push((tag_name.clone(), task.clone()));
                    }
                }
            }

            match self.sort_mode {
                SortMode::TaskNumber => {
                    combined.sort_by(|(tag_a, task_a), (tag_b, task_b)| {
                        task_sort_key(&task_a.id)
                            .cmp(&task_sort_key(&task_b.id))
                            .then_with(|| tag_a.cmp(tag_b))
                            .then_with(|| task_a.id.cmp(&task_b.id))
                    });
                }
                SortMode::Tag => {
                    combined.sort_by(|(tag_a, task_a), (tag_b, task_b)| {
                        tag_a
                            .cmp(tag_b)
                            .then_with(|| task_sort_key(&task_a.id).cmp(&task_sort_key(&task_b.id)))
                            .then_with(|| task_a.id.cmp(&task_b.id))
                    });
                }
            }

            self.tasks = combined.iter().map(|(_, task)| task.clone()).collect();
            self.task_tags = combined.into_iter().map(|(tag_name, _)| tag_name).collect();
        } else if let Some(ctx) = project.tags.get(&tag) {
            self.tasks = ctx.tasks.clone();
            self.task_tags = vec![tag; self.tasks.len()];
        } else if let Some((first_tag, ctx)) = project.tags.iter().next() {
            let first = first_tag.clone();
            self.current_tag = first.clone();
            self.tasks = ctx.tasks.clone();
            self.task_tags = vec![first; self.tasks.len()];
        } else {
            self.tasks.clear();
            self.task_tags.clear();
        }

        self.recalc_display_rows();
        self.restore_selection(selection);
    }

    fn sync_tag_items(&mut self) {
        if !self.show_tag_selector {
            return;
        }
        let selected_name = self
            .tag_list_state
            .selected()
            .and_then(|idx| self.tag_items.get(idx))
            .map(|item| item.name.clone());

        self.tag_items = self.build_tag_items();
        if self.tag_items.is_empty() {
            self.tag_list_state.select(None);
            return;
        }

        if let Some(name) = selected_name {
            if let Some(idx) = self.tag_items.iter().position(|item| item.name == name) {
                self.tag_list_state.select(Some(idx));
                return;
            }
        }

        self.ensure_tag_selection();
    }

    fn build_tag_items(&self) -> Vec<TagItem> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let mut items: Vec<TagItem> = project
            .tags
            .iter()
            .map(|(name, ctx)| TagItem {
                name: name.clone(),
                total: ctx.tasks.len(),
                done: ctx
                    .tasks
                    .iter()
                    .filter(|task| task.status.is_done())
                    .count(),
            })
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        if !items.is_empty() {
            let total: usize = items.iter().map(|item| item.total).sum();
            let done: usize = items.iter().map(|item| item.done).sum();
            items.insert(
                0,
                TagItem {
                    name: ALL_TAG_VIEW.to_string(),
                    total,
                    done,
                },
            );
        }
        items
    }

    fn ensure_tag_selection(&mut self) {
        if self.tag_items.is_empty() {
            self.tag_list_state.select(None);
            return;
        }
        let current = self.current_tag_or_default();
        let idx = self
            .tag_items
            .iter()
            .position(|item| item.name == current)
            .unwrap_or(0);
        self.tag_list_state.select(Some(idx));
    }

    fn open_tag_selector(&mut self) {
        self.tag_items = self.build_tag_items();
        self.show_tag_selector = true;
        self.show_help = false;
        self.focus = FocusMode::Tags;
        self.ensure_tag_selection();
    }

    fn close_tag_selector(&mut self) {
        self.show_tag_selector = false;
        if self.show_detail {
            self.focus = FocusMode::Details;
        } else {
            self.focus = FocusMode::List;
        }
    }

    fn toggle_tag_selector(&mut self) {
        if self.show_tag_selector {
            self.close_tag_selector();
        } else {
            self.open_tag_selector();
        }
    }

    fn move_tag_selection(&mut self, delta: isize) {
        if self.tag_items.is_empty() {
            self.tag_list_state.select(None);
            return;
        }
        let current = self.tag_list_state.selected().unwrap_or(0) as isize;
        let len = self.tag_items.len() as isize;
        let mut next = current + delta;
        if next < 0 {
            next = len - 1;
        }
        if next >= len {
            next = 0;
        }
        self.tag_list_state.select(Some(next as usize));
    }

    fn apply_selected_tag(&mut self) {
        let Some(idx) = self.tag_list_state.selected() else {
            return;
        };
        let Some(item) = self.tag_items.get(idx) else {
            return;
        };
        self.current_tag = item.name.clone();
        self.apply_current_tag();
        self.touch_state_file();
    }

    fn handle_tag_selector_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc | KeyCode::Char('T') => {
                self.close_tag_selector();
                return true;
            }
            KeyCode::Enter => {
                if self.focus == FocusMode::Tags {
                    self.apply_selected_tag();
                }
                self.close_tag_selector();
                return true;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusMode::List => FocusMode::Tags,
                    _ => FocusMode::List,
                };
                return true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.focus == FocusMode::Tags {
                    self.move_tag_selection(1);
                    return true;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.focus == FocusMode::Tags {
                    self.move_tag_selection(-1);
                    return true;
                }
            }
            KeyCode::Char('t') => {
                return true;
            }
            _ => {}
        }
        false
    }

    pub fn recalc_display_rows(&mut self) {
        let mut rows = Vec::new();
        for (idx, task) in self.tasks.iter().enumerate() {
            let match_filter = match self.filter {
                FilterMode::All => true,
                FilterMode::Pending => !task.status.is_done(),
                FilterMode::Done => task.status.is_done(),
            };

            if !match_filter {
                continue;
            }

            rows.push(DisplayRow {
                task_idx: idx,
                subtask_path: vec![],
                depth: 0,
                tag_name: self
                    .task_tags
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| self.current_tag_or_default()),
            });

            if self.expanded_tasks.contains(&task.id) {
                for (s_i, _) in task.subtasks.iter().enumerate() {
                    rows.push(DisplayRow {
                        task_idx: idx,
                        subtask_path: vec![s_i],
                        depth: 1,
                        tag_name: self
                            .task_tags
                            .get(idx)
                            .cloned()
                            .unwrap_or_else(|| self.current_tag_or_default()),
                    });
                }
            }
        }

        self.display_rows = rows;
        if self.display_rows.is_empty() {
            self.table_state.select(None);
            return;
        }
        if let Some(selected) = self.table_state.selected() {
            if selected >= self.display_rows.len() {
                self.table_state
                    .select(Some(self.display_rows.len().saturating_sub(1)));
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.show_tag_selector && self.handle_tag_selector_key(key) {
            self.maybe_update_pane_title();
            return;
        }
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                    if self.show_detail {
                        self.focus = FocusMode::Details;
                    } else {
                        self.focus = FocusMode::List;
                    }
                } else if self.show_detail {
                    self.show_detail = false;
                    self.details_scroll = 0;
                    self.details_max_scroll = 0;
                    self.focus = FocusMode::List;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
            }
            KeyCode::Char('r') => {
                self.refresh(true);
            }
            KeyCode::Enter => {
                if self.show_help {
                    self.show_help = false;
                }
                if self.show_tag_selector {
                    self.show_tag_selector = false;
                }
                self.show_detail = !self.show_detail;
                if !self.show_detail {
                    self.focus = FocusMode::List;
                } else {
                    self.focus = FocusMode::Details;
                }
            }
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
                if self.show_help {
                    self.show_tag_selector = false;
                    self.focus = FocusMode::Details;
                } else if self.show_tag_selector {
                    self.focus = FocusMode::Tags;
                } else if self.show_detail {
                    self.focus = FocusMode::Details;
                } else {
                    self.focus = FocusMode::List;
                }
            }
            KeyCode::Char(' ') => {
                self.toggle_expand();
            }
            KeyCode::Tab => {
                if self.show_tag_selector {
                    self.focus = match self.focus {
                        FocusMode::List => FocusMode::Tags,
                        FocusMode::Tags => FocusMode::List,
                        FocusMode::Details => FocusMode::Tags,
                    };
                } else if self.show_detail || self.show_help {
                    self.focus = match self.focus {
                        FocusMode::List => FocusMode::Details,
                        FocusMode::Details => FocusMode::List,
                        FocusMode::Tags => FocusMode::List,
                    };
                }
            }
            KeyCode::Char('x') => {
                self.toggle_status();
            }
            KeyCode::Char('a') => {
                self.toggle_active_agent();
            }
            KeyCode::Char('f') => {
                self.filter = self.filter.next();
                self.recalc_display_rows();
            }
            KeyCode::Char('s') => {
                self.cycle_sort_mode();
            }
            KeyCode::Char('t') => {
                self.cycle_tag();
            }
            KeyCode::Char('T') => {
                self.toggle_tag_selector();
            }
            _ => {}
        }

        self.maybe_update_pane_title();
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_left_click(event.column, event.row);
            }
            MouseEventKind::Moved => {
                self.update_focus_from_hover(event.column, event.row);
            }
            MouseEventKind::ScrollUp => {
                self.handle_scroll(-1);
            }
            MouseEventKind::ScrollDown => {
                self.handle_scroll(1);
            }
            _ => {}
        }

        self.maybe_update_pane_title();
    }

    pub fn update_layout(&mut self, list_area: Rect, details_area: Option<Rect>) {
        self.list_area = Some(list_area);
        self.details_area = details_area;
        if details_area.is_none() {
            self.details_scroll = 0;
            self.details_max_scroll = 0;
        }
    }

    fn toggle_expand(&mut self) {
        let idx = self.table_state.selected().unwrap_or(0);
        if idx >= self.display_rows.len() {
            return;
        }

        let task_idx = self.display_rows[idx].task_idx;
        if self.tasks[task_idx].subtasks.is_empty() {
            return;
        }

        let id = self.tasks[task_idx].id.clone();
        if self.expanded_tasks.contains(&id) {
            self.expanded_tasks.remove(&id);
        } else {
            self.expanded_tasks.insert(id);
        }
        self.recalc_display_rows();
    }

    fn current_selection_key(&self) -> Option<SelectionKey> {
        let selected = self.table_state.selected()?;
        let row = self.display_rows.get(selected)?;
        let task = self.tasks.get(row.task_idx)?;
        let subtask_id = row
            .subtask_path
            .first()
            .and_then(|idx| task.subtasks.get(*idx).map(|sub| sub.id));
        Some(SelectionKey {
            tag_name: row.tag_name.clone(),
            task_id: task.id.clone(),
            subtask_id,
        })
    }

    fn restore_selection(&mut self, key: Option<SelectionKey>) {
        let mut restored = false;
        if let Some(key) = key {
            for (idx, row) in self.display_rows.iter().enumerate() {
                let task = match self.tasks.get(row.task_idx) {
                    Some(task) => task,
                    None => continue,
                };
                if task.id != key.task_id {
                    continue;
                }
                if row.tag_name != key.tag_name {
                    continue;
                }

                let row_subtask_id = row
                    .subtask_path
                    .first()
                    .and_then(|sub_idx| task.subtasks.get(*sub_idx).map(|sub| sub.id));

                if key.subtask_id == row_subtask_id {
                    self.table_state.select(Some(idx));
                    restored = true;
                    break;
                }
            }
        }

        if restored {
            return;
        }

        if self.display_rows.is_empty() {
            self.table_state.select(None);
            return;
        }

        let selected = self.table_state.selected();
        match selected {
            Some(index) if index < self.display_rows.len() => {}
            Some(_) => {
                self.table_state
                    .select(Some(self.display_rows.len().saturating_sub(1)));
                self.details_scroll = 0;
            }
            None => {
                self.table_state.select(Some(0));
                self.details_scroll = 0;
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.display_rows.is_empty() {
            return;
        }

        let current = self.table_state.selected().unwrap_or(0) as isize;
        let len = self.display_rows.len() as isize;
        let mut next = current + delta;
        if next < 0 {
            next = len - 1;
        }
        if next >= len {
            next = 0;
        }

        self.table_state.select(Some(next as usize));
        self.details_scroll = 0;
    }

    pub fn toggle_status(&mut self) {
        let idx = self.table_state.selected().unwrap_or(0);
        if idx >= self.display_rows.len() {
            return;
        }
        let row = &self.display_rows[idx];
        let task_idx = row.task_idx;
        let tag_name = row.tag_name.clone();
        let task_id = self.tasks[task_idx].id.clone();

        if let Some(sub_idx) = row.subtask_path.first() {
            let new_status = match self.tasks[task_idx].subtasks[*sub_idx].status {
                TaskStatus::Done => TaskStatus::Pending,
                _ => TaskStatus::Done,
            };
            self.tasks[task_idx].subtasks[*sub_idx].status = new_status.clone();

            if let Some(project) = &mut self.project {
                if let Some(ctx) = project.tags.get_mut(&tag_name) {
                    if let Some(project_task_idx) =
                        ctx.tasks.iter().position(|task| task.id == task_id)
                    {
                        if *sub_idx < ctx.tasks[project_task_idx].subtasks.len() {
                            ctx.tasks[project_task_idx].subtasks[*sub_idx].status = new_status;
                        }
                    }
                }
            }
        } else {
            let new_status = match self.tasks[task_idx].status {
                TaskStatus::Done => TaskStatus::Pending,
                _ => TaskStatus::Done,
            };
            self.tasks[task_idx].status = new_status.clone();

            if let Some(project) = &mut self.project {
                if let Some(ctx) = project.tags.get_mut(&tag_name) {
                    if let Some(project_task_idx) =
                        ctx.tasks.iter().position(|task| task.id == task_id)
                    {
                        ctx.tasks[project_task_idx].status = new_status;
                    }
                }
            }
        }

        self.save_project();
        self.recalc_display_rows();
    }

    fn toggle_active_agent(&mut self) {
        let idx = self.table_state.selected().unwrap_or(0);
        if idx >= self.display_rows.len() {
            return;
        }
        let row = &self.display_rows[idx];
        if !row.subtask_path.is_empty() {
            return;
        }

        let task_idx = row.task_idx;
        let tag_name = row.tag_name.clone();
        let task_id = self.tasks[task_idx].id.clone();
        let new_value = !self.tasks[task_idx].active_agent;
        self.tasks[task_idx].active_agent = new_value;

        if let Some(project) = &mut self.project {
            if let Some(ctx) = project.tags.get_mut(&tag_name) {
                if let Some(project_task_idx) = ctx.tasks.iter().position(|task| task.id == task_id)
                {
                    ctx.tasks[project_task_idx].active_agent = new_value;
                }
            }
        }

        self.save_project();
    }

    pub fn current_tag_or_default(&self) -> String {
        if !self.current_tag.is_empty() {
            return self.current_tag.clone();
        }
        self.inferred_default_tag()
    }

    fn inferred_default_tag(&self) -> String {
        let Some(project) = &self.project else {
            return "master".to_string();
        };

        if project.tags.len() > 1 {
            return ALL_TAG_VIEW.to_string();
        }

        if project.tags.contains_key("master") {
            return "master".to_string();
        }

        let mut tag_names: Vec<&String> = project.tags.keys().collect();
        tag_names.sort();
        if let Some(first) = tag_names.first() {
            return (*first).clone();
        }

        "master".to_string()
    }

    pub fn display_tag_name<'a>(&self, tag: &'a str) -> &'a str {
        if tag == ALL_TAG_VIEW {
            "all"
        } else {
            tag
        }
    }

    fn handle_left_click(&mut self, column: u16, row: u16) {
        if let Some(area) = self.list_area {
            if contains(area, column, row) {
                self.focus = FocusMode::List;
                if let Some(idx) = self.row_from_coords(area, column, row) {
                    if idx < self.display_rows.len() {
                        let current = self.table_state.selected().unwrap_or(0);
                        if idx == current {
                            if self.show_help {
                                self.show_help = false;
                            }
                            if self.show_tag_selector {
                                self.show_tag_selector = false;
                            }
                            self.show_detail = !self.show_detail;
                            if self.show_detail {
                                self.focus = FocusMode::Details;
                            } else {
                                self.focus = FocusMode::List;
                            }
                        } else {
                            self.table_state.select(Some(idx));
                            self.details_scroll = 0;
                        }
                    }
                }
                return;
            }
        }

        if let Some(area) = self.details_area {
            if contains(area, column, row) {
                if self.show_tag_selector {
                    self.focus = FocusMode::Tags;
                    let inner = Rect {
                        x: area.x.saturating_add(1),
                        y: area.y.saturating_add(1),
                        width: area.width.saturating_sub(2),
                        height: area.height.saturating_sub(2),
                    };
                    if let Some(idx) = self.row_from_inner_coords(inner, column, row) {
                        if idx < self.tag_items.len() {
                            self.tag_list_state.select(Some(idx));
                        }
                    }
                } else {
                    self.focus = FocusMode::Details;
                }
            }
        }
    }

    fn update_focus_from_hover(&mut self, column: u16, row: u16) {
        if let Some(area) = self.list_area {
            if contains(area, column, row) {
                self.focus = FocusMode::List;
                return;
            }
        }

        if let Some(area) = self.details_area {
            if contains(area, column, row) {
                self.focus = if self.show_tag_selector {
                    FocusMode::Tags
                } else {
                    FocusMode::Details
                };
            }
        }
    }

    fn handle_scroll(&mut self, delta: i16) {
        if self.show_tag_selector && self.focus == FocusMode::Tags {
            if delta < 0 {
                self.move_tag_selection(-1);
            } else {
                self.move_tag_selection(1);
            }
            return;
        }

        if self.show_detail && self.focus == FocusMode::Details && !self.show_help {
            if delta < 0 {
                self.details_scroll = self.details_scroll.saturating_sub(1);
            } else {
                let next = self.details_scroll.saturating_add(1);
                self.details_scroll = next.min(self.details_max_scroll);
            }
            return;
        }

        if delta < 0 {
            self.move_selection(-1);
        } else {
            self.move_selection(1);
        }
    }

    fn row_from_coords(&self, area: Rect, column: u16, row: u16) -> Option<usize> {
        if !contains(area, column, row) {
            return None;
        }

        let header_height = 2u16;
        if area.height <= header_height + 1 {
            return None;
        }

        let data_start = area.y.saturating_add(header_height);
        let data_end = area.y.saturating_add(area.height.saturating_sub(1));
        if row < data_start || row >= data_end {
            return None;
        }

        let row_index = (row - data_start) as usize;
        let offset = self.table_state.offset();
        Some(offset + row_index)
    }

    fn row_from_inner_coords(&self, area: Rect, column: u16, row: u16) -> Option<usize> {
        if !contains(area, column, row) {
            return None;
        }

        if area.height == 0 {
            return None;
        }

        let row_index = (row - area.y) as usize;
        Some(row_index)
    }

    fn cycle_tag(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        let mut tags: Vec<String> = project.tags.keys().cloned().collect();
        tags.sort();
        tags.insert(0, ALL_TAG_VIEW.to_string());
        if tags.is_empty() {
            return;
        }

        let current = self.current_tag_or_default();
        let pos = tags.iter().position(|t| *t == current).unwrap_or(0);
        let next_idx = (pos + 1) % tags.len();
        self.current_tag = tags[next_idx].clone();
        self.apply_current_tag();
        self.touch_state_file();
    }

    fn cycle_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.apply_current_tag();
    }

    fn save_project(&mut self) {
        let Some(project) = &self.project else {
            self.set_error("Cannot save tasks: no project loaded".to_string());
            return;
        };

        let json = match serde_json::to_string_pretty(project) {
            Ok(payload) => payload,
            Err(err) => {
                self.set_error(format!("Failed to serialize tasks.json: {err}"));
                return;
            }
        };

        if let Err(err) = write_atomic(&self.tasks_path, &json) {
            self.set_error(format!("Failed to write tasks.json: {err}"));
            return;
        }

        self.last_error = None;
        self.update_tasks_mtime();
        self.touch_state_file();
    }

    fn update_tasks_mtime(&mut self) {
        if let Ok(metadata) = std::fs::metadata(&self.tasks_path) {
            if let Ok(modified) = metadata.modified() {
                self.last_tasks_mtime = Some(modified);
            }
        }
    }

    fn touch_state_file(&mut self) {
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_millis().to_string(),
            Err(_) => return,
        };

        let mut state = if let Ok(content) = std::fs::read_to_string(&self.state_path) {
            serde_json::from_str::<Value>(&content).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };

        if let Value::Object(map) = &mut state {
            map.insert("lastUpdated".to_string(), Value::String(now));
            if sync_tags_enabled()
                && !self.current_tag.is_empty()
                && self.current_tag != ALL_TAG_VIEW
            {
                map.insert(
                    "currentTag".to_string(),
                    Value::String(self.current_tag.clone()),
                );
            }
        }

        if let Ok(payload) = serde_json::to_string_pretty(&state) {
            if let Err(err) = write_atomic(&self.state_path, &payload) {
                self.set_error(format!("Failed to write state.json: {err}"));
                return;
            }
        }

        if let Ok(metadata) = std::fs::metadata(&self.state_path) {
            if let Ok(modified) = metadata.modified() {
                self.last_state_mtime = Some(modified);
            }
        }
    }

    fn build_pane_title(&self) -> String {
        let current_tag = self.current_tag_or_default();
        let tag = self.display_tag_name(current_tag.as_str());
        let total = self.tasks.len();
        let done = self.tasks.iter().filter(|t| t.status.is_done()).count();
        let percent = if total > 0 {
            (done as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let bar_width = 10;
        let filled = (percent / 100.0 * bar_width as f64) as usize;
        let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(bar_width - filled));

        let title = format!(
            "[{}] {} {}/{} | Filter: {} | Sort: {}",
            tag,
            bar,
            done,
            total,
            self.filter.label(),
            self.sort_mode.label()
        );

        title
    }

    fn maybe_update_pane_title(&mut self) {
        if std::env::var("ZELLIJ_SESSION_NAME").is_err() {
            return;
        }

        let title = self.build_pane_title();
        if self.last_title.as_deref() == Some(&title) {
            return;
        }

        if rename_pane(&title) {
            self.last_title = Some(title);
        }
    }
}

fn task_sort_key(id: &str) -> (u64, String) {
    let digits: String = id.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    let number = digits.parse::<u64>().unwrap_or(u64::MAX);
    (number, id.to_string())
}

fn rename_pane(title: &str) -> bool {
    let Ok(_session) = std::env::var("ZELLIJ_SESSION_NAME") else {
        return false;
    };

    if let Ok(pane_id) = std::env::var("ZELLIJ_PANE_ID") {
        let support = PANE_ID_SUPPORT.load(Ordering::Relaxed);
        if support != 2 {
            let status = Command::new("zellij")
                .args(["action", "rename-pane", "--pane-id", &pane_id, title])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if matches!(status, Ok(status) if status.success()) {
                PANE_ID_SUPPORT.store(1, Ordering::Relaxed);
                return true;
            }
            PANE_ID_SUPPORT.store(2, Ordering::Relaxed);
        }
    }

    emit_pane_title(title)
}

static PANE_ID_SUPPORT: AtomicU8 = AtomicU8::new(0);

fn emit_pane_title(title: &str) -> bool {
    let mut stdout = io::stdout();
    let payload = format!("\x1b]0;{}\x07", title);
    if stdout.write_all(payload.as_bytes()).is_err() {
        return false;
    }
    stdout.flush().is_ok()
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn write_atomic(path: &Path, payload: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory {}", parent.display()))?;
    }

    let temp_path = match path.file_name() {
        Some(name) => path.with_file_name(format!("{}.tmp", name.to_string_lossy())),
        None => path.with_extension("tmp"),
    };

    std::fs::write(&temp_path, payload)
        .with_context(|| format!("Failed to write temp file {}", temp_path.display()))?;
    std::fs::rename(&temp_path, path)
        .with_context(|| format!("Failed to replace {}", path.display()))?;
    Ok(())
}

fn sync_tags_enabled() -> bool {
    matches!(
        std::env::var("AOC_TASKMASTER_SYNC_TAG").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE")
    )
}

fn parse_project_compat(content: &str) -> std::result::Result<ProjectData, String> {
    if let Ok(project) = serde_json::from_str::<ProjectData>(content) {
        return Ok(project);
    }

    let raw: Value = serde_json::from_str(content)
        .map_err(|err| format!("tasks.json is not valid JSON: {err}"))?;
    let root = raw
        .as_object()
        .ok_or_else(|| "tasks.json root must be a JSON object".to_string())?;

    if let Some(tasks_value) = root.get("tasks") {
        let tasks: Vec<Task> = serde_json::from_value(tasks_value.clone())
            .map_err(|err| format!("legacy tasks array is invalid: {err}"))?;

        let mut extra = HashMap::new();
        if let Some(metadata) = root.get("metadata") {
            extra.insert("metadata".to_string(), metadata.clone());
        }

        let mut tags = HashMap::new();
        tags.insert("master".to_string(), TagContext { tasks, extra });
        return Ok(ProjectData { tags });
    }

    if let Some(tags_value) = root.get("tags") {
        let tags_obj = tags_value.as_object().ok_or_else(|| {
            "legacy wrapped tags format requires object at key 'tags'".to_string()
        })?;

        let mut tags = HashMap::new();
        for (tag_name, tag_ctx_value) in tags_obj {
            let tag_ctx: TagContext = serde_json::from_value(tag_ctx_value.clone())
                .map_err(|err| format!("invalid tag context for '{tag_name}': {err}"))?;
            tags.insert(tag_name.clone(), tag_ctx);
        }
        return Ok(ProjectData { tags });
    }

    Err(
        "unsupported tasks format; expected top-level tags map, legacy {\"tasks\": [...]}, or wrapped {\"tags\": {...}}"
            .to_string(),
    )
}

fn validate_project(project: &ProjectData) -> std::result::Result<(), String> {
    for (tag, tag_ctx) in &project.tags {
        for task in &tag_ctx.tasks {
            if let Some(prd) = &task.aoc_prd {
                if prd.path.trim().is_empty() {
                    return Err(format!(
                        "Invalid tasks.json: task [{}] in tag '{}' has empty aocPrd.path",
                        task.id, tag
                    ));
                }
            }
            for sub in &task.subtasks {
                if sub.extra.contains_key("aocPrd") {
                    return Err(format!(
                        "Invalid tasks.json: subtask [{}] in task [{}] tag '{}' has unsupported aocPrd",
                        sub.id, task.id, tag
                    ));
                }
            }
        }
    }
    Ok(())
}
