//! Keyboard and event input handling.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn handle_input(event: Event, app: &mut App, refresh_requested: &mut bool) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            handle_key(key, app, refresh_requested)
        }
        _ => false,
    }
}

pub(crate) fn handle_key(key: KeyEvent, app: &mut App, refresh_requested: &mut bool) -> bool {
    if app.mode == Mode::Mind && app.mind_search_editing {
        match key.code {
            KeyCode::Esc => {
                app.mind_search_editing = false;
                app.status_note = Some("mind search edit cancelled".to_string());
                return false;
            }
            KeyCode::Enter => {
                app.mind_search_editing = false;
                app.mind_search_selected = 0;
                app.status_note = Some(if app.mind_search_query.trim().is_empty() {
                    "mind search cleared".to_string()
                } else {
                    format!("mind search: {}", app.mind_search_query.trim())
                });
                app.scroll = 0;
                return false;
            }
            KeyCode::Backspace => {
                app.mind_search_query.pop();
                return false;
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.mind_search_query.push(ch);
                return false;
            }
            _ => return false,
        }
    }

    if matches!(key.code, KeyCode::Char('?') | KeyCode::F(1)) {
        app.help_open = !app.help_open;
        return false;
    }
    if key.code == KeyCode::Esc && app.help_open {
        app.help_open = false;
        return false;
    }
    if app.help_open {
        return false;
    }

    match key.code {
        KeyCode::Char('q') => true,
        KeyCode::Char('1') => {
            if app.config.overview_enabled {
                app.mode = Mode::Overview;
            } else {
                app.mode = Mode::Overseer;
                app.status_note = Some("overview disabled; switched to Overseer".to_string());
            }
            app.scroll = 0;
            false
        }
        KeyCode::Char('2') => {
            app.mode = Mode::Overseer;
            app.scroll = 0;
            false
        }
        KeyCode::Char('3') | KeyCode::Char('m') => {
            app.mode = Mode::Mind;
            app.scroll = 0;
            false
        }
        KeyCode::Char('4') => {
            app.mode = Mode::Fleet;
            app.scroll = 0;
            false
        }
        KeyCode::Char('5') => {
            app.mode = Mode::Work;
            app.scroll = 0;
            false
        }
        KeyCode::Char('6') => {
            app.mode = Mode::Diff;
            app.scroll = 0;
            false
        }
        KeyCode::Char('7') => {
            app.mode = Mode::Health;
            app.scroll = 0;
            false
        }
        KeyCode::Tab => {
            app.cycle_mode();
            app.scroll = 0;
            false
        }
        KeyCode::Enter => {
            if app.mode == Mode::Fleet {
                app.focus_selected_fleet_project();
            } else {
                app.focus_selected_overview_tab();
            }
            false
        }
        KeyCode::Char('x') => {
            if app.mode == Mode::Fleet {
                app.cancel_selected_fleet_job();
            } else {
                app.stop_selected_overview_agent();
            }
            false
        }
        KeyCode::Char('e') => {
            app.capture_selected_pane_evidence();
            false
        }
        KeyCode::Char('E') => {
            app.follow_selected_pane_live();
            false
        }
        KeyCode::Char('o') => {
            app.request_manual_observer_run();
            false
        }
        KeyCode::Char('i') => {
            if app.mode == Mode::Fleet {
                app.launch_fleet_followup(false);
            }
            false
        }
        KeyCode::Char('h') => {
            if app.mode == Mode::Fleet {
                app.launch_fleet_followup(true);
            }
            false
        }
        KeyCode::Char('c') => {
            if app.mode == Mode::Overseer {
                app.request_overseer_consultation(ConsultationPacketKind::Review);
            }
            false
        }
        KeyCode::Char('u') => {
            if app.mode == Mode::Overseer {
                app.request_overseer_consultation(ConsultationPacketKind::HelpRequest);
            }
            false
        }
        KeyCode::Char('s') => {
            if app.mode == Mode::Overseer {
                app.request_spawn_worker();
            }
            false
        }
        KeyCode::Char('d') => {
            if app.mode == Mode::Overseer {
                app.request_delegate_worker();
            }
            false
        }
        KeyCode::Char('O') => {
            app.request_insight_dispatch_chain();
            false
        }
        KeyCode::Char('b') => {
            app.request_insight_bootstrap(true);
            false
        }
        KeyCode::Char('B') => {
            app.request_insight_bootstrap(false);
            false
        }
        KeyCode::Char('F') => {
            app.request_mind_force_finalize();
            false
        }
        KeyCode::Char('C') => {
            app.request_mind_compaction_rebuild();
            false
        }
        KeyCode::Char('R') => {
            app.request_mind_t3_requeue();
            false
        }
        KeyCode::Char('H') => {
            app.request_mind_handshake_rebuild();
            false
        }
        KeyCode::Char('t') => {
            if app.mode == Mode::Mind {
                app.toggle_mind_lane();
            }
            false
        }
        KeyCode::Char('v') => {
            if app.mode == Mode::Mind {
                app.toggle_mind_scope();
            }
            false
        }
        KeyCode::Char('p') => {
            if app.mode == Mode::Mind {
                app.toggle_mind_provenance();
            }
            false
        }
        KeyCode::Char('/') => {
            if app.mode == Mode::Mind {
                app.mind_search_editing = true;
                app.mind_search_selected = 0;
                app.status_note = Some("editing mind search query".to_string());
            }
            false
        }
        KeyCode::Char('n') => {
            if app.mode == Mode::Mind && !app.mind_search_query.trim().is_empty() {
                app.mind_search_selected = app.mind_search_selected.saturating_add(1);
                app.status_note = Some("mind search next result".to_string());
            }
            false
        }
        KeyCode::Char('N') => {
            if app.mode == Mode::Mind && !app.mind_search_query.trim().is_empty() {
                app.mind_search_selected = app.mind_search_selected.saturating_sub(1);
                app.status_note = Some("mind search previous result".to_string());
            }
            false
        }
        KeyCode::Char('f') => {
            if app.mode == Mode::Fleet {
                app.toggle_fleet_plane_filter();
            }
            false
        }
        KeyCode::Char('A') => {
            if app.mode == Mode::Fleet {
                app.toggle_fleet_active_only();
            }
            false
        }
        KeyCode::Char('S') => {
            if app.mode == Mode::Fleet {
                app.toggle_fleet_sort_mode();
            }
            false
        }
        KeyCode::Left | KeyCode::Char('[') => {
            if app.mode == Mode::Fleet {
                app.move_fleet_job_selection(-1);
            }
            false
        }
        KeyCode::Right | KeyCode::Char(']') => {
            if app.mode == Mode::Fleet {
                app.move_fleet_job_selection(1);
            }
            false
        }
        KeyCode::Char('a') => {
            if app.mode == Mode::Overview {
                app.toggle_overview_sort_mode();
            }
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.mode == Mode::Overview {
                app.move_overview_selection(1);
            } else if app.mode == Mode::Fleet {
                app.move_fleet_selection(1);
            } else {
                app.scroll = app.scroll.saturating_add(1);
            }
            false
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.mode == Mode::Overview {
                app.move_overview_selection(-1);
            } else if app.mode == Mode::Fleet {
                app.move_fleet_selection(-1);
            } else {
                app.scroll = app.scroll.saturating_sub(1);
            }
            false
        }
        KeyCode::Char('g') => {
            if app.mode == Mode::Overview {
                app.selected_overview = 0;
            }
            if app.mode == Mode::Fleet {
                app.selected_fleet = 0;
                app.selected_fleet_job = 0;
            }
            app.scroll = 0;
            false
        }
        KeyCode::Char('r') => {
            *refresh_requested = true;
            false
        }
        _ => false,
    }
}
