mod backend;
mod state;
mod theme;
mod ui;

use std::collections::BTreeMap;
use std::time::SystemTime;
use zellij_tile::prelude::*;

use backend::BufferBackend;
use ratatui::Terminal;
use state::State;

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
            }
            Event::Mouse(mouse) => {
                match mouse {
                    Mouse::ScrollDown(_) => {
                        let i = self.table_state.selected().unwrap_or(0);
                        if i < self.display_rows.len().saturating_sub(1) {
                            self.table_state.select(Some(i + 1));
                            self.mark_dirty();
                        }
                    }
                    Mouse::ScrollUp(_) => {
                        let i = self.table_state.selected().unwrap_or(0);
                        if i > 0 {
                            self.table_state.select(Some(i - 1));
                            self.mark_dirty();
                        }
                    }
                    _ => {}
                }
                self.take_render()
            }
            Event::RunCommandResult(_, stdout, stderr, context) => {
                self.handle_command_result(stdout, stderr, context);
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
