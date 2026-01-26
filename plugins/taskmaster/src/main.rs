mod backend;
mod state;
mod theme;
mod ui;

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

use backend::BufferBackend;
use ratatui::Terminal;
use state::{FocusMode, State};

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.load_config(configuration);
        self.plugin_pane_id = Some(get_plugin_ids().plugin_id);
        set_selectable(true);
        subscribe(&[
            EventType::Timer,
            EventType::Key,
            EventType::Mouse,
            EventType::PaneUpdate,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
        ]);
        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ChangeApplicationState,
        ]);
        set_timeout(0.1);
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
            }
            Event::Mouse(mouse) => {
                match mouse {
                    Mouse::LeftClick(line, col) => {
                        // Handle "click to select, click again to toggle details"
                        // Coordinate mapping relies on UI layout in render_main
                        // Top Border: 0
                        // Header: 1
                        // Data Row 0: 2
                        // ...

                        let header_height = 2; // Border + Header
                        let visual_row = (line as isize) - (header_height as isize);

                        if visual_row >= 0 {
                            let mut width = self.last_render_cols;
                            if self.show_detail || self.show_help {
                                width = self.last_render_cols / 2;
                            }

                            // Only handle clicks in the task list area
                            if (col as u16) < width {
                                // Add offset if we supported scroll offset.
                                // Since we disabled scroll, offset should be 0 unless list auto-scrolled?
                                // Actually ratatui manages offset. We can try to get it.
                                let offset = self.table_state.offset();
                                let target_idx = (visual_row as usize) + offset;

                                if target_idx < self.display_rows.len() {
                                    let current_selected = self.table_state.selected().unwrap_or(0);
                                    if target_idx == current_selected {
                                        // Clicked again -> Toggle Details
                                        self.show_detail = !self.show_detail;
                                        if self.show_help {
                                            self.show_help = false;
                                        } // Clear help if detailing
                                        if !self.show_detail {
                                            self.focus = FocusMode::List;
                                        }
                                    } else {
                                        // Select new task
                                        self.table_state.select(Some(target_idx));
                                        if self.show_detail {
                                            // Keep detail open, just switch context
                                        }
                                    }
                                    self.mark_dirty();
                                    self.take_render()
                                } else {
                                    // Clicked empty space
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    Mouse::ScrollDown(_) => {
                        let i = self.table_state.selected().unwrap_or(0);
                        if i < self.display_rows.len().saturating_sub(1) {
                            self.table_state.select(Some(i + 1));
                            self.mark_dirty();
                            self.take_render()
                        } else {
                            false
                        }
                    }
                    Mouse::ScrollUp(_) => {
                        let i = self.table_state.selected().unwrap_or(0);
                        if i > 0 {
                            self.table_state.select(Some(i - 1));
                            self.mark_dirty();
                            self.take_render()
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            Event::RunCommandResult(_, stdout, stderr, context) => {
                self.handle_command_result(stdout, stderr, context);
                self.take_render()
            }
            Event::PaneUpdate(pane_manifest) => {
                self.handle_pane_update(pane_manifest);
                self.take_render()
            }
            Event::PermissionRequestResult(_) => {
                self.permissions_granted = true;
                self.refresh();
                self.take_render()
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if rows == 0 || cols == 0 {
            return;
        }

        // Cache dimensions for mouse handling
        self.last_render_rows = rows as u16;
        self.last_render_cols = cols as u16;

        let backend = BufferBackend::new(cols as u16, rows as u16);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                ui::render(f, self);
            })
            .unwrap();

        let s = terminal.backend().render_to_string();
        print!("{}", s);
    }
}
