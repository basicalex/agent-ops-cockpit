mod model;
mod state;
mod theme;
mod ui;

use std::collections::BTreeMap;
use std::time::SystemTime;
use zellij_tile::prelude::*;

use state::State;
use theme::colors;
use ui::*;

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.load_config(configuration);
        set_selectable(true);
        subscribe(&[
            EventType::Timer,
            EventType::Key,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
        ]);
        request_permission(&[PermissionType::RunCommands]);
        self.ignore_refresh_until = Some(SystemTime::now() + std::time::Duration::from_secs(2));
        set_timeout(self.refresh_secs);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::Timer(_) => {
                self.refresh();
                set_timeout(self.refresh_secs);
                self.take_render()
            }
            Event::Key(key) => {
                if self.handle_key(key) {
                    self.mark_dirty();
                    self.take_render()
                } else {
                    false
                }
            },
            Event::RunCommandResult(_, stdout, stderr, context) => {
                self.handle_command_result(stdout, stderr, context);
                self.take_render()
            }
            Event::PermissionRequestResult(status) => {
                match status {
                    PermissionStatus::Granted => {
                        self.permissions_granted = true;
                        self.pending_root = false;
                        self.pending_tasks = false;
                        self.pending_state = false;
                        if self.last_error_action.as_deref() == Some(State::ACTION_READ_ROOT)
                            || self.last_error_action.as_deref() == Some(State::ACTION_READ_TASKS)
                            || self.last_error_action.as_deref() == Some(State::ACTION_READ_STATE)
                        {
                            self.clear_error();
                        }
                        self.refresh();
                        self.mark_dirty();
                    }
                    PermissionStatus::Denied => {
                        self.permissions_granted = false;
                        self.set_error_with_action(
                            "RunCommands permission denied.".to_string(),
                            None,
                        );
                    }
                }
                self.take_render()
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if rows == 0 || cols == 0 {
            return;
        }

        // --- Error Modal ---
        if let Some(err) = &self.last_error {
            let lines = draw_error_modal(err, rows, cols);
            print_lines_checked(self, lines, rows, cols);
            return;
        }

        let mut lines = Vec::new();

        // --- Header HUD ---
        let total = self.tasks.len();
        let done = self.tasks.iter().filter(|t| t.status.to_lowercase() == "done").count();

        // Line 1: Title + Tag
        let title_line = format!(
            "{}AOC TASKMASTER{} [{}]",
            colors::BOLD,
            colors::RESET,
            if self.current_tag.is_empty() { "master" } else { &self.current_tag }
        );
        lines.push(truncate_visible(&title_line, cols));

        // Line 2: Progress Bar
        // Width = cols - (label + counts) ~ cols - 20
        let bar_width = cols.saturating_sub(25).min(40).max(10);
        let bar = ProgressBar::new(done, total, bar_width).render();
        lines.push(truncate_visible(&format!(
            "Progress: {} {}{}/{}{}", 
            bar, colors::BOLD, done, total, colors::RESET
        ), cols));

        // Line 3: Filter + Root
        lines.push(truncate_visible(&format!(
            "Filter: {}{}{}   Root: {}", 
            colors::YELLOW, self.filter.label(), colors::RESET,
            self.root.as_ref().map(|p| p.to_string_lossy()).unwrap_or("?".into())
        ), cols));

        // Line 4: Search Bar / Root Bar
        if self.input_mode == state::InputMode::Search || !self.search_query.is_empty() {
            let prefix = if self.input_mode == state::InputMode::Search { format!("{}Search:{} ", colors::CYAN, colors::RESET) } else { "Search: ".to_string() };
            lines.push(truncate_visible(&format!("{}{}_", prefix, self.search_query), cols));
        } else if self.input_mode == state::InputMode::Root {
            let prefix = format!("{}Set Root:{} ", colors::BLUE, colors::RESET);
            lines.push(truncate_visible(&format!("{}{}_", prefix, self.root_query), cols));
        }

        lines.push(String::new()); // Spacer

        // --- Task List (Table) ---
        if self.filtered.is_empty() {
             lines.push(format!("{}No tasks found for filter: {}{}", colors::DIM, self.filter.label(), colors::RESET));
        } else {
            let mut table = UiTable::new(vec!["", "S", "ID", "P", "Title"]);
            
            for (idx, task_index) in self.filtered.iter().enumerate() {
                // Reserve space for help footer (1 line) + some details if needed
                if lines.len() + table.rows.len() >= rows.saturating_sub(2) { 
                    break; 
                }
                
                let task = &self.tasks[*task_index];
                let marker = if idx == self.selected { format!("{}>{}", colors::BOLD, colors::RESET) } else { " ".to_string() };
                let symbol = status_symbol(&task.status);
                let prio = colorize_priority(&task.priority, &task.priority);
                
                let agent_prefix = if task.active_agent { 
                    format!("{}{} {}", colors::MAGENTA, theme::icons::AGENT, colors::RESET) 
                } else { 
                    "".to_string() 
                };
                let title = format!("{}{}", agent_prefix, task.title);
                
                table.add_row(vec![
                    marker,
                    symbol, 
                    task.id.clone(),
                    prio,
                    title
                ]);
            }
            
            lines.extend(table.render(cols));
        }

        // --- Details Pane (Split) ---
        if self.show_detail {
            lines.push(String::new());
            if let Some(task) = self.selected_task() {
                let detail_header = if self.focus == state::FocusMode::Details {
                    format!("{}Details: #{} {} [FOCUSED]{}", colors::MAGENTA, task.id, task.title, colors::RESET)
                } else {
                    format!("{}Details: #{} {}{}", colors::BOLD, task.id, task.title, colors::RESET)
                };
                lines.push(truncate_visible(&detail_header, cols));
                
                // Show progress bar for task if it has subtasks
                if !task.subtasks.is_empty() {
                    let (done, total) = task.completion_stats();
                    let bar = ProgressBar::new(done, total, 20).render();
                    lines.push(truncate_visible(&format!("  Progress: {} {}/{} Done", bar, done, total), cols));
                    lines.push(String::new());
                }

                if !task.description.is_empty() {
                    lines.extend(wrap_block("Desc", &task.description, cols));
                }
                if !task.dependencies.is_empty() {
                    lines.push(truncate_visible(&format!("{}Deps:{} {}", colors::DIM, colors::RESET, task.dependencies.join(", ")), cols));
                }
                
                if !task.subtasks.is_empty() {
                    lines.push(truncate_visible(&format!("{}Subtasks:{}", colors::DIM, colors::RESET), cols));
                    let is_focused = self.focus == state::FocusMode::Details;
                    lines.extend(draw_subtask_tree(&task.subtasks, 0, cols, self.subtask_cursor, is_focused));
                }
            } else {
                 lines.push("Select a task to view details.".to_string());
            }
        }

        // --- Footer / Help ---
        // Ensure we fit in rows
        if lines.len() > rows {
            lines.truncate(rows);
        }
        // If we have space, show help
        if lines.len() < rows {
             let help = if self.input_mode == state::InputMode::Search {
                 format!("{}[Enter/Esc] Stop searching{}", colors::DIM, colors::RESET)
             } else if self.input_mode == state::InputMode::Root {
                 format!("{}[Enter] Apply [Esc] Cancel{}", colors::DIM, colors::RESET)
             } else              if self.focus == state::FocusMode::Details {
                 format!("{}[Tab/^o] List [j/k] Nav [Space] Toggle [e] Edit{}", colors::DIM, colors::RESET)
             } else {
                 format!("{}[j/k] Nav [x] Done [Tab] Details [/] Search [C] Root [r] Refresh{}", colors::DIM, colors::RESET)
             };
             // Push to bottom
             while lines.len() < rows - 1 {
                 lines.push(String::new());
             }
             lines.push(truncate_visible(&help, cols));
        }

        print_lines_checked(self, lines, rows, cols);
    }
}

fn print_lines_checked(state: &mut State, lines: Vec<String>, rows: usize, cols: usize) {
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

    if state.last_render_output.as_ref() == Some(&padded) {
        return;
    }
    state.last_render_output = Some(padded.clone());

    // Move cursor to home (top-left) to avoid scrollback growth
    print!("\u{1b}[H");
    for line in &padded {
        println!("{}", line);
    }
}
