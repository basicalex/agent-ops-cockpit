use zellij_tile::prelude::*;

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
use uuid::Uuid;

use zjstatus::{
    config::{self, ModuleConfig, UpdateEventMask, ZellijState},
    frames, pipe,
    widgets::{
        command::{CommandResult, CommandWidget},
        datetime::DateTimeWidget,
        mode::ModeWidget,
        notification::NotificationWidget,
        pipe::PipeWidget,
        session::SessionWidget,
        swap_layout::SwapLayoutWidget,
        tabs::TabsWidget,
        widget::Widget,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SharedTabSnapshot {
    tabs: Vec<SharedTabInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SharedTabInfo {
    position: usize,
    name: String,
    active: bool,
    panes_to_hide: usize,
    is_fullscreen_active: bool,
    is_sync_panes_active: bool,
    are_floating_panes_visible: bool,
    active_swap_layout_name: Option<String>,
    is_swap_layout_dirty: bool,
    viewport_rows: usize,
    viewport_columns: usize,
    display_area_rows: usize,
    display_area_columns: usize,
    selectable_tiled_panes_count: usize,
    selectable_floating_panes_count: usize,
}

impl From<&TabInfo> for SharedTabInfo {
    fn from(tab: &TabInfo) -> Self {
        Self {
            position: tab.position,
            name: tab.name.clone(),
            active: tab.active,
            panes_to_hide: tab.panes_to_hide,
            is_fullscreen_active: tab.is_fullscreen_active,
            is_sync_panes_active: tab.is_sync_panes_active,
            are_floating_panes_visible: tab.are_floating_panes_visible,
            active_swap_layout_name: tab.active_swap_layout_name.clone(),
            is_swap_layout_dirty: tab.is_swap_layout_dirty,
            viewport_rows: tab.viewport_rows,
            viewport_columns: tab.viewport_columns,
            display_area_rows: tab.display_area_rows,
            display_area_columns: tab.display_area_columns,
            selectable_tiled_panes_count: tab.selectable_tiled_panes_count,
            selectable_floating_panes_count: tab.selectable_floating_panes_count,
        }
    }
}

impl From<SharedTabInfo> for TabInfo {
    fn from(tab: SharedTabInfo) -> Self {
        Self {
            position: tab.position,
            name: tab.name,
            active: tab.active,
            panes_to_hide: tab.panes_to_hide,
            is_fullscreen_active: tab.is_fullscreen_active,
            is_sync_panes_active: tab.is_sync_panes_active,
            are_floating_panes_visible: tab.are_floating_panes_visible,
            active_swap_layout_name: tab.active_swap_layout_name,
            is_swap_layout_dirty: tab.is_swap_layout_dirty,
            viewport_rows: tab.viewport_rows,
            viewport_columns: tab.viewport_columns,
            display_area_rows: tab.display_area_rows,
            display_area_columns: tab.display_area_columns,
            selectable_tiled_panes_count: tab.selectable_tiled_panes_count,
            selectable_floating_panes_count: tab.selectable_floating_panes_count,
            other_focused_clients: Vec::new(),
        }
    }
}

#[derive(Default)]
struct State {
    pending_events: Vec<Event>,
    got_permissions: bool,
    state: ZellijState,
    userspace_configuration: BTreeMap<String, String>,
    module_config: config::ModuleConfig,
    widget_map: BTreeMap<String, Arc<dyn Widget>>,
    err: Option<anyhow::Error>,
}

#[cfg(not(test))]
register_plugin!(State);

#[cfg(feature = "tracing")]
fn init_tracing() {
    use std::fs::File;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let file = File::create("/host/.zjstatus.log");
    let file = match file {
        Ok(file) => file,
        Err(error) => panic!("Error: {:?}", error),
    };
    let debug_log = tracing_subscriber::fmt::layer().with_writer(Arc::new(file));

    tracing_subscriber::registry().with(debug_log).init();

    tracing::info!("tracing initialized");
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        #[cfg(feature = "tracing")]
        init_tracing();

        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the RunCommands permission to run "cargo test" in a floating window
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::RunCommands,
        ]);

        self.module_config = match ModuleConfig::new(&configuration) {
            Ok(mc) => mc,
            Err(e) => {
                self.err = Some(e);
                return;
            }
        };

        let mut subscriptions = vec![EventType::PermissionRequestResult];
        let required_mask = self.module_config.required_event_mask();
        let needs_frames = self.module_config.needs_frame_events();
        let needs_tabs = required_mask & config::UpdateEventMask::Tab as u8 != 0;

        if !self.module_config.disable_mouse {
            subscriptions.push(EventType::Mouse);
        }
        if required_mask & config::UpdateEventMask::Mode as u8 != 0 || needs_frames {
            subscriptions.push(EventType::ModeUpdate);
        }
        if needs_tabs {
            subscriptions.push(EventType::TabUpdate);
            // Zellij's TabUpdate can lag tab reordering (MoveTab left/right) until a later
            // tab lifecycle event. SessionUpdate carries the current session tab roster and
            // arrives for reorders, so use it as the freshness path for the tab bar too.
            subscriptions.push(EventType::SessionUpdate);
        }
        if needs_frames {
            subscriptions.push(EventType::PaneUpdate);
            if !needs_tabs {
                subscriptions.push(EventType::SessionUpdate);
            }
        }
        if required_mask & config::UpdateEventMask::Command as u8 != 0 {
            subscriptions.push(EventType::RunCommandResult);
        }
        subscribe(&subscriptions);
        self.widget_map = register_widgets(&configuration);
        self.userspace_configuration = configuration;
        self.pending_events = Vec::new();
        self.got_permissions = false;
        let uid = Uuid::new_v4();

        self.state = ZellijState {
            cols: 0,
            command_results: BTreeMap::new(),
            pipe_results: BTreeMap::new(),
            mode: ModeInfo::default(),
            panes: PaneManifest::default(),
            plugin_uuid: uid.to_string(),
            tabs: Vec::new(),
            sessions: Vec::new(),
            start_time: Local::now(),
            cache_mask: 0,
            incoming_notification: None,
            runtime_theme: Default::default(),
            runtime_tab_metadata: BTreeMap::new(),
            pending_runtime_tab_metadata: BTreeMap::new(),
        };
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        pipe::handle_pipe_message(&mut self.state, &pipe_message)
    }

    #[tracing::instrument(skip_all, fields(event_type))]
    fn update(&mut self, event: Event) -> bool {
        if let Event::PermissionRequestResult(PermissionStatus::Granted) = event {
            self.got_permissions = true;

            while !self.pending_events.is_empty() {
                tracing::debug!("processing cached event");
                let ev = self.pending_events.pop();

                self.handle_event(ev.unwrap());
            }
        }

        if !self.got_permissions {
            tracing::debug!("caching event");
            self.pending_events.push(event);

            return false;
        }

        self.handle_event(event)
    }

    #[tracing::instrument(skip_all)]
    fn render(&mut self, _rows: usize, cols: usize) {
        if !self.got_permissions {
            return;
        }

        if let Some(err) = &self.err {
            println!("Error: {:?}", err);

            return;
        }

        self.refresh_tabs_from_shared_snapshot();
        self.state.cols = cols;

        tracing::debug!("{:?}", self.state.mode.session_name);

        let output = self
            .module_config
            .render_bar(self.state.clone(), self.widget_map.clone());

        print!("{}", output);
    }
}

impl State {
    fn apply_tab_snapshot(&mut self, tab_info: Vec<TabInfo>) {
        self.apply_tab_snapshot_inner(tab_info, true);
    }

    fn apply_tab_snapshot_inner(&mut self, tab_info: Vec<TabInfo>, publish_shared: bool) {
        self.state.runtime_tab_metadata = config::reconcile_runtime_tab_metadata(
            &self.state.tabs,
            &tab_info,
            &self.state.runtime_tab_metadata,
        );
        config::apply_pending_runtime_tab_metadata(
            &tab_info,
            &mut self.state.runtime_tab_metadata,
            &mut self.state.pending_runtime_tab_metadata,
        );
        self.state.tabs = tab_info;
        if publish_shared {
            self.write_shared_tab_snapshot();
        }
    }

    fn refresh_tabs_from_shared_snapshot(&mut self) {
        let Some(path) = self.shared_tab_snapshot_path() else {
            return;
        };
        let Ok(payload) = fs::read_to_string(path) else {
            return;
        };
        let Ok(snapshot) = serde_json::from_str::<SharedTabSnapshot>(&payload) else {
            return;
        };
        if snapshot.tabs.is_empty() {
            return;
        }
        let tab_info = snapshot.tabs.into_iter().map(TabInfo::from).collect();
        self.apply_tab_snapshot_inner(tab_info, false);
        self.state.cache_mask |= UpdateEventMask::Tab as u8;
    }

    fn write_shared_tab_snapshot(&self) {
        let Some(path) = self.shared_tab_snapshot_path() else {
            return;
        };
        let snapshot = SharedTabSnapshot {
            tabs: self.state.tabs.iter().map(SharedTabInfo::from).collect(),
        };
        let Ok(payload) = serde_json::to_string(&snapshot) else {
            return;
        };
        let tmp_path = path.with_extension(format!("{}.tmp", self.state.plugin_uuid));
        if fs::write(&tmp_path, payload).is_ok() {
            let _ = fs::rename(tmp_path, path);
        }
    }

    fn shared_tab_snapshot_path(&self) -> Option<PathBuf> {
        let session_name = self.state.mode.session_name.as_deref()?.trim();
        if session_name.is_empty() {
            return None;
        }
        let session_key = sanitize_snapshot_key(session_name);
        Some(PathBuf::from(format!(
            "/host/tmp/aoc-zjstatus-tabs-{session_key}.json"
        )))
    }

    fn handle_event(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::Mouse(mouse_info) => {
                tracing::Span::current().record("event_type", "Event::Mouse");
                tracing::debug!(mouse = ?mouse_info);

                self.module_config.handle_mouse_action(
                    self.state.clone(),
                    mouse_info,
                    self.widget_map.clone(),
                );
            }
            Event::ModeUpdate(mode_info) => {
                tracing::Span::current().record("event_type", "Event::ModeUpdate");
                tracing::debug!(mode = ?mode_info.mode);
                tracing::debug!(mode = ?mode_info.session_name);

                self.state.mode = mode_info;
                self.state.cache_mask = UpdateEventMask::Mode as u8;

                should_render = true;
            }
            Event::PaneUpdate(pane_info) => {
                tracing::Span::current().record("event_type", "Event::PaneUpdate");
                tracing::debug!(pane_count = ?pane_info.panes.len());

                frames::hide_frames_conditionally(
                    &frames::FrameConfig::new(
                        self.module_config.hide_frame_for_single_pane,
                        self.module_config.hide_frame_except_for_search,
                        self.module_config.hide_frame_except_for_fullscreen,
                        self.module_config.hide_frame_except_for_scroll,
                    ),
                    &self.state.tabs,
                    &pane_info,
                    &self.state.mode,
                    get_plugin_ids(),
                    false,
                );

                self.state.panes = pane_info;
                self.state.cache_mask = UpdateEventMask::Tab as u8;

                should_render = true;
            }
            Event::PermissionRequestResult(result) => {
                tracing::Span::current().record("event_type", "Event::PermissionRequestResult");
                tracing::debug!(result = ?result);
                set_selectable(false);
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                tracing::Span::current().record("event_type", "Event::RunCommandResult");
                tracing::debug!(
                    exit_code = ?exit_code,
                    stdout = ?String::from_utf8(stdout.clone()),
                    stderr = ?String::from_utf8(stderr.clone()),
                    context = ?context
                );

                self.state.cache_mask = UpdateEventMask::Command as u8;

                if let Some(name) = context.get("name") {
                    let stdout = match String::from_utf8(stdout) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    let stderr = match String::from_utf8(stderr) {
                        Ok(s) => s,
                        Err(_) => "".to_owned(),
                    };

                    self.state.command_results.insert(
                        name.to_owned(),
                        CommandResult {
                            exit_code,
                            stdout,
                            stderr,
                            context,
                        },
                    );
                }
            }
            Event::SessionUpdate(session_info, _) => {
                tracing::Span::current().record("event_type", "Event::SessionUpdate");

                let current_session = session_info.iter().find(|s| s.is_current_session);

                if let Some(current_session) = current_session {
                    frames::hide_frames_conditionally(
                        &frames::FrameConfig::new(
                            self.module_config.hide_frame_for_single_pane,
                            self.module_config.hide_frame_except_for_search,
                            self.module_config.hide_frame_except_for_fullscreen,
                            self.module_config.hide_frame_except_for_scroll,
                        ),
                        &current_session.tabs,
                        &current_session.panes,
                        &self.state.mode,
                        get_plugin_ids(),
                        false,
                    );

                    self.apply_tab_snapshot(current_session.tabs.clone());
                }

                self.state.sessions = session_info;
                self.state.cache_mask = UpdateEventMask::Tab as u8 | UpdateEventMask::Session as u8;

                should_render = true;
            }
            Event::TabUpdate(tab_info) => {
                tracing::Span::current().record("event_type", "Event::TabUpdate");
                tracing::debug!(tab_count = ?tab_info.len());

                self.state.cache_mask = UpdateEventMask::Tab as u8;
                self.apply_tab_snapshot(tab_info);

                should_render = true;
            }
            _ => (),
        };
        should_render
    }
}

fn sanitize_snapshot_key(input: &str) -> String {
    let sanitized: String = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn register_widgets(configuration: &BTreeMap<String, String>) -> BTreeMap<String, Arc<dyn Widget>> {
    let mut widget_map = BTreeMap::<String, Arc<dyn Widget>>::new();

    widget_map.insert(
        "command".to_owned(),
        Arc::new(CommandWidget::new(configuration)),
    );
    widget_map.insert(
        "datetime".to_owned(),
        Arc::new(DateTimeWidget::new(configuration)),
    );
    widget_map.insert("pipe".to_owned(), Arc::new(PipeWidget::new(configuration)));
    widget_map.insert(
        "swap_layout".to_owned(),
        Arc::new(SwapLayoutWidget::new(configuration)),
    );
    widget_map.insert("mode".to_owned(), Arc::new(ModeWidget::new(configuration)));
    widget_map.insert(
        "session".to_owned(),
        Arc::new(SessionWidget::new(configuration)),
    );
    widget_map.insert("tabs".to_owned(), Arc::new(TabsWidget::new(configuration)));
    widget_map.insert(
        "notifications".to_owned(),
        Arc::new(NotificationWidget::new(configuration)),
    );

    tracing::debug!("registered widgets: {:?}", widget_map.keys());

    widget_map
}
