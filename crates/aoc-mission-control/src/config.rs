//! Configuration and environment resolution.
//!
//! Extracted from main.rs (Phase 2).

use super::*;
use std::{io, time::Duration};
use tracing_subscriber::EnvFilter;

pub(crate) fn load_config() -> Config {
    let session_id = resolve_session_id();
    let pane_id = resolve_pane_id();
    let tab_scope = resolve_tab_scope();
    let pulse_socket_path = resolve_pulse_socket_path(&session_id);
    let mission_theme = resolve_mission_theme_mode();
    let mission_custom_theme = resolve_custom_mission_theme();
    let pulse_vnext_enabled = resolve_pulse_vnext_enabled();
    let overview_enabled = resolve_overview_enabled();
    let _runtime_mode = resolve_runtime_mode();
    let start_view = resolve_start_view();
    let fleet_plane_filter = resolve_fleet_plane_filter();
    let layout_source = resolve_layout_source();
    let client_id = format!("aoc-mission-control-{}", std::process::id());
    let project_root = resolve_project_root();
    let mind_project_scoped = resolve_mind_project_scoped();
    let state_dir = resolve_state_dir();
    Config {
        session_id,
        pane_id,
        tab_scope,
        pulse_socket_path,
        mission_theme,
        mission_custom_theme,
        pulse_vnext_enabled,
        overview_enabled,
        start_view,
        fleet_plane_filter,
        layout_source,
        client_id,
        project_root,
        mind_project_scoped,
        state_dir,
    }
}

pub(crate) fn resolve_local_layout_refresh_ms() -> u64 {
    std::env::var("AOC_MISSION_CONTROL_LAYOUT_REFRESH_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|value| value.clamp(LOCAL_LAYOUT_REFRESH_MS_MIN, LOCAL_LAYOUT_REFRESH_MS_MAX))
        .unwrap_or(LOCAL_LAYOUT_REFRESH_MS_DEFAULT)
}

pub(crate) fn parse_bool_flag(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub(crate) fn resolve_pulse_vnext_enabled() -> bool {
    std::env::var("AOC_PULSE_VNEXT_ENABLED")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(true)
}

pub(crate) fn resolve_overview_enabled() -> bool {
    std::env::var("AOC_PULSE_OVERVIEW_ENABLED")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(true)
}

pub(crate) fn parse_runtime_mode(value: &str) -> Option<RuntimeMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pulse-pane" | "pulse_pane" | "pulse" => Some(RuntimeMode::MissionControl),
        "mission-control" | "mission_control" | "mission" | "mc" => {
            Some(RuntimeMode::MissionControl)
        }
        _ => None,
    }
}

pub(crate) fn parse_start_view(value: &str) -> Option<Mode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "overview" | "ov" => Some(Mode::Overview),
        "overseer" => Some(Mode::Overseer),
        "mind" => Some(Mode::Mind),
        "fleet" | "detached" | "subagents" => Some(Mode::Fleet),
        "work" => Some(Mode::Work),
        "diff" => Some(Mode::Diff),
        "health" => Some(Mode::Health),
        _ => None,
    }
}

pub(crate) fn parse_fleet_plane_filter(value: &str) -> Option<FleetPlaneFilter> {
    match value.trim().to_ascii_lowercase().as_str() {
        "all" => Some(FleetPlaneFilter::All),
        "delegated" | "specialist" | "subagents" => Some(FleetPlaneFilter::Delegated),
        "mind" => Some(FleetPlaneFilter::Mind),
        _ => None,
    }
}

pub(crate) fn resolve_runtime_mode() -> RuntimeMode {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--mode=") {
            if let Some(mode) = parse_runtime_mode(value) {
                return mode;
            }
        }
        if arg == "--mode" {
            if let Some(value) = args.next() {
                if let Some(mode) = parse_runtime_mode(&value) {
                    return mode;
                }
            }
        }
    }

    if let Some(mode) = std::env::var("AOC_MISSION_CONTROL_MODE")
        .ok()
        .as_deref()
        .and_then(parse_runtime_mode)
    {
        return mode;
    }

    if std::env::var("AOC_PULSE_LIGHT_PANE")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(false)
    {
        return RuntimeMode::MissionControl;
    }

    RuntimeMode::MissionControl
}

pub(crate) fn resolve_local_snapshot_refresh_secs() -> u64 {
    std::env::var("AOC_MISSION_CONTROL_SNAPSHOT_REFRESH_SECS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map(|value| {
            value.clamp(
                LOCAL_SNAPSHOT_REFRESH_SECS_MIN,
                LOCAL_SNAPSHOT_REFRESH_SECS_MAX,
            )
        })
        .unwrap_or(LOCAL_SNAPSHOT_REFRESH_SECS_DEFAULT)
}

pub(crate) fn resolve_layout_source() -> LayoutSource {
    match std::env::var("AOC_PULSE_LAYOUT_SOURCE") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "local" => LayoutSource::Local,
            "hybrid" => LayoutSource::Hybrid,
            _ => LayoutSource::Hub,
        },
        Err(_) => LayoutSource::Hub,
    }
}

pub(crate) fn resolve_start_view() -> Option<Mode> {
    std::env::var("AOC_MISSION_CONTROL_START_VIEW")
        .ok()
        .as_deref()
        .and_then(parse_start_view)
}

pub(crate) fn resolve_fleet_plane_filter() -> FleetPlaneFilter {
    std::env::var("AOC_MISSION_CONTROL_FLEET_PLANE")
        .ok()
        .as_deref()
        .and_then(parse_fleet_plane_filter)
        .unwrap_or(FleetPlaneFilter::All)
}

pub(crate) fn parse_mission_theme_mode(value: &str) -> Option<MissionThemeMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "terminal" | "auto" => Some(MissionThemeMode::Terminal),
        "dark" => Some(MissionThemeMode::Dark),
        "light" => Some(MissionThemeMode::Light),
        _ => None,
    }
}

pub(crate) fn resolve_mission_theme_mode() -> MissionThemeMode {
    std::env::var("AOC_MISSION_CONTROL_THEME")
        .ok()
        .or_else(|| std::env::var("AOC_PULSE_THEME").ok())
        .as_deref()
        .and_then(parse_mission_theme_mode)
        .unwrap_or(MissionThemeMode::Terminal)
}

pub(crate) fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stdout_enabled = matches!(
        std::env::var("AOC_LOG_STDOUT").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    );
    if stdout_enabled {
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(io::sink)
            .try_init();
    }
}

pub(crate) fn resolve_session_id() -> String {
    if let Ok(value) = std::env::var("ZELLIJ_SESSION_NAME") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = std::env::var("AOC_SESSION_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    format!("pid-{}", std::process::id())
}

pub(crate) fn resolve_pane_id() -> String {
    if let Ok(value) = std::env::var("AOC_PANE_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = std::env::var("ZELLIJ_PANE_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    String::new()
}

pub(crate) fn resolve_tab_scope() -> Option<String> {
    for key in ["AOC_TAB_SCOPE", "AOC_TAB_NAME", "ZELLIJ_TAB_NAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub(crate) fn resolve_pulse_socket_path(session_id: &str) -> PathBuf {
    if let Ok(value) = std::env::var("AOC_PULSE_SOCK") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    let runtime_dir = if let Ok(value) = std::env::var("XDG_RUNTIME_DIR") {
        if !value.trim().is_empty() {
            PathBuf::from(value)
        } else {
            PathBuf::from("/tmp")
        }
    } else if let Ok(uid) = std::env::var("UID") {
        PathBuf::from(format!("/run/user/{uid}"))
    } else {
        PathBuf::from("/tmp")
    };
    runtime_dir
        .join("aoc")
        .join(session_slug(session_id))
        .join("pulse.sock")
}

pub(crate) fn resolve_project_root() -> PathBuf {
    if let Ok(value) = std::env::var("AOC_PROJECT_ROOT") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub(crate) fn resolve_mind_project_scoped() -> bool {
    std::env::var("AOC_MIND_PROJECT_SCOPED")
        .ok()
        .and_then(|value| parse_bool_flag(&value))
        .unwrap_or(false)
}

pub(crate) fn resolve_state_dir() -> PathBuf {
    if let Ok(value) = std::env::var("XDG_STATE_HOME") {
        if !value.trim().is_empty() {
            return PathBuf::from(value).join("aoc");
        }
    }
    if let Ok(value) = std::env::var("HOME") {
        return PathBuf::from(value)
            .join(".local")
            .join("state")
            .join("aoc");
    }
    PathBuf::from(".aoc/state")
}

pub(crate) fn session_slug(session_id: &str) -> String {
    let mut slug = String::with_capacity(session_id.len());
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            slug.push(ch);
        } else {
            slug.push('-');
        }
    }
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-').to_string();
    let base = if slug.is_empty() {
        "session".to_string()
    } else {
        slug
    };
    let hash = stable_hash_hex(session_id);
    let short = if base.len() > 48 {
        &base[..48]
    } else {
        base.as_str()
    };
    format!("{short}-{hash}")
}

pub(crate) fn stable_hash_hex(input: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in input.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("{hash:08x}")
}

pub(crate) fn next_backoff(current: Duration) -> Duration {
    let next = current + current;
    if next > Duration::from_secs(10) {
        Duration::from_secs(10)
    } else {
        next
    }
}

pub(crate) fn sanitize_component(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
