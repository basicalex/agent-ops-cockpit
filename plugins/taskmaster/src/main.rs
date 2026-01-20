mod model;
mod state;
mod theme;
mod ui;

use std::collections::{BTreeMap, BTreeSet};
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
            EventType::Mouse,
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
            Event::Mouse(mouse) => {
                match mouse {
                    Mouse::ScrollDown(_) => {
                         self.handle_key(KeyWithModifier { bare_key: BareKey::Down, key_modifiers: BTreeSet::new() });
                    }
                    Mouse::ScrollUp(_) => {
                         self.handle_key(KeyWithModifier { bare_key: BareKey::Up, key_modifiers: BTreeSet::new() });
                    }
                    Mouse::LeftClick(line, _col) => {
                         if line >= 0 {
                             let line = line as usize;
                             if line >= self.list_start_y && line < self.list_start_y + self.viewport_height {
                                 let row_idx = line - self.list_start_y;
                                 let task_idx = self.scroll_offset + row_idx;
                                 if task_idx < self.display_rows.len() {
                                     self.selected = task_idx;
                                     self.mark_dirty();
                                 }
                             }
                         }
                    }
                    _ => {}
                }
                self.take_render()
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

        // Line 1: Title + Tag Bar
        let tag_line = self.render_tags_line().unwrap_or_default();
        let title_line = if tag_line.is_empty() {
             format!("{}AOC TASKMASTER{} [{}]", colors::BOLD, colors::RESET, if self.current_tag.is_empty() { "master" } else { &self.current_tag })
        } else {
             // If we have tags, show them instead of just the current one in brackets
             tag_line
        };
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

        // --- Calculate Layout ---
        let header_height = lines.len();
        let footer_height = 1;
        let mut available_height = rows.saturating_sub(header_height + footer_height);
        
        if self.show_detail {
            // Give details 40% of available height, minimum 5 lines
            let mut details_height = (available_height * 4 / 10).max(5);
            // Ensure we don't take more than available
            if details_height > available_height {
                details_height = available_height;
            }
            available_height = available_height.saturating_sub(details_height);
        }

        // --- Task List (Table) ---
        // table header takes 2 lines (header + separator)
        let list_content_height = available_height.saturating_sub(2);
        self.viewport_height = list_content_height;
        
        if self.display_rows.is_empty() {
             lines.push(format!("{}No tasks found for filter: {}{}", colors::DIM, self.filter.label(), colors::RESET));
             // Fill empty space
             while lines.len() < header_height + available_height {
                 lines.push(String::new());
             }
        } else {
            let mut table = UiTable::new(vec!["", "S", "ID", "P", "Title"]);
            
            // Handle scrolling bounds
            if self.scroll_offset > self.display_rows.len() {
                self.scroll_offset = self.display_rows.len().saturating_sub(1);
            }
            
            self.list_start_y = lines.len() + 2; // Current lines + table header (1) + separator (1)
            
            for (i, row) in self.display_rows.iter().enumerate().skip(self.scroll_offset).take(list_content_height) {
                let task = &self.tasks[row.task_idx];
                let marker = if i == self.selected { format!("{}>{}", colors::BOLD, colors::RESET) } else { " ".to_string() };
                
                if row.depth == 0 {
                    let symbol = status_symbol(&task.status);
                    let prio = colorize_priority(&task.priority, &task.priority);
                    
                    let agent_prefix = if task.active_agent { 
                        format!("{}{} {}", colors::MAGENTA, theme::icons::AGENT, colors::RESET) 
                    } else { 
                        "".to_string() 
                    };
                    
                    let expand_icon = if !task.subtasks.is_empty() {
                         if self.expanded_tasks.contains(&task.id) { "▼ " } else { "▶ " }
                    } else {
                         ""
                    };

                    let title = format!("{}{}{}", expand_icon, agent_prefix, task.title);
                    
                    table.add_row(vec![
                        marker,
                        symbol, 
                        task.id.clone(),
                        prio,
                        title
                    ]);
                } else {
                    // Subtask Render
                    // Resolve subtask from path
                    let mut current_sub = &task.subtasks[row.subtask_path[0]];
                    for &idx in &row.subtask_path[1..] {
                         if idx < current_sub.subtasks.len() {
                             current_sub = &current_sub.subtasks[idx];
                         }
                    }
                    
                    let symbol = status_symbol(&current_sub.status);
                    let indent = "  ".repeat(row.depth);
                    let title = format!("{}└─ {}", indent, current_sub.title);
                    
                    table.add_row(vec![
                        marker,
                        symbol,
                        "".to_string(),
                        "".to_string(),
                        title
                    ]);
                }
            }
            
            lines.extend(table.render(cols));
            
            // Fill remaining list area if table is shorter than viewport
            let rendered_table_height = table.rows.len() + 2; // rows + header + sep
            let needed_padding = available_height.saturating_sub(rendered_table_height);
            for _ in 0..needed_padding {
                lines.push(String::new());
            }
        }

        // --- Details Pane (Split) ---
        if self.show_detail {
            lines.push(truncate_visible(&"─".repeat(cols), cols)); // Splitter
            
            // We have `details_height` lines remaining (minus 1 for splitter we just added?)
            // Actually available_height calculation subtracted details_height. 
            // We just added lines up to (header + available_height).
            // So we are at the start of details block.
            
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
                 format!("{}[j/k] Nav [Space] Expand [x] Done [Tab] Details [/] Search [C] Root{}", colors::DIM, colors::RESET)
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
