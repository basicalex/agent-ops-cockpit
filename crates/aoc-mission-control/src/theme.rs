//! Mission Control theme types and theme construction helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MissionTheme {
    pub(crate) surface: Color,
    pub(crate) border: Color,
    pub(crate) title: Color,
    pub(crate) text: Color,
    pub(crate) muted: Color,
    pub(crate) accent: Color,
    pub(crate) ok: Color,
    pub(crate) warn: Color,
    pub(crate) critical: Color,
    pub(crate) info: Color,
}

pub(crate) fn mission_theme(mode: MissionThemeMode) -> MissionTheme {
    match mode {
        MissionThemeMode::Terminal => MissionTheme {
            surface: Color::Reset,
            border: Color::DarkGray,
            title: Color::Cyan,
            text: Color::Reset,
            muted: Color::DarkGray,
            accent: Color::Blue,
            ok: Color::Green,
            warn: Color::Yellow,
            critical: Color::Red,
            info: Color::Cyan,
        },
        MissionThemeMode::Dark => MissionTheme {
            surface: Color::Rgb(17, 26, 46),
            border: Color::Rgb(71, 85, 105),
            title: Color::Rgb(191, 219, 254),
            text: Color::Rgb(226, 232, 240),
            muted: Color::Rgb(148, 163, 184),
            accent: Color::Rgb(56, 189, 248),
            ok: Color::Rgb(34, 197, 94),
            warn: Color::Rgb(245, 158, 11),
            critical: Color::Rgb(239, 68, 68),
            info: Color::Rgb(59, 130, 246),
        },
        MissionThemeMode::Light => MissionTheme {
            surface: Color::Rgb(245, 247, 250),
            border: Color::Rgb(148, 163, 184),
            title: Color::Rgb(30, 64, 175),
            text: Color::Rgb(15, 23, 42),
            muted: Color::Rgb(100, 116, 139),
            accent: Color::Rgb(2, 132, 199),
            ok: Color::Rgb(22, 163, 74),
            warn: Color::Rgb(217, 119, 6),
            critical: Color::Rgb(220, 38, 38),
            info: Color::Rgb(37, 99, 235),
        },
    }
}

pub(crate) fn parse_hex_color(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

pub(crate) fn resolve_custom_mission_theme() -> Option<MissionTheme> {
    let env_color = |key: &str| -> Option<Color> {
        std::env::var(key).ok().as_deref().and_then(parse_hex_color)
    };
    let env_color_any =
        |keys: &[&str]| -> Option<Color> { keys.iter().find_map(|key| env_color(key)) };

    Some(MissionTheme {
        // Inherit the pane/terminal background so Mission Control matches the rest of the AOC layout.
        surface: Color::Reset,
        border: env_color_any(&["AOC_THEME_BG_ELEVATED", "AOC_THEME_BLACK"])?,
        title: env_color_any(&["AOC_THEME_UI_ACCENT", "AOC_THEME_BLUE"])?,
        text: env_color_any(&["AOC_THEME_UI_PRIMARY", "AOC_THEME_FG"])?,
        muted: env_color_any(&["AOC_THEME_UI_MUTED", "AOC_THEME_WHITE"])?,
        accent: env_color_any(&["AOC_THEME_UI_ACCENT", "AOC_THEME_BLUE"])?,
        ok: env_color_any(&["AOC_THEME_UI_SUCCESS", "AOC_THEME_GREEN"])?,
        warn: env_color_any(&["AOC_THEME_UI_WARNING", "AOC_THEME_YELLOW"])?,
        critical: env_color_any(&["AOC_THEME_UI_DANGER", "AOC_THEME_RED"])?,
        info: env_color_any(&["AOC_THEME_UI_INFO", "AOC_THEME_CYAN"])?,
    })
}

pub(crate) fn mind_theme(theme: MissionTheme) -> aoc_mind::MindTheme {
    aoc_mind::MindTheme {
        muted: theme.muted,
        info: theme.info,
        accent: theme.accent,
        warn: theme.warn,
        ok: theme.ok,
        critical: theme.critical,
        text: theme.text,
        title: theme.title,
    }
}

pub(crate) fn mind_lane_color(lane: MindLaneFilter, theme: MissionTheme) -> Color {
    aoc_mind::mind_lane_color(lane, mind_theme(theme))
}

pub(crate) fn mind_status_color(status: MindObserverFeedStatus, theme: MissionTheme) -> Color {
    aoc_mind::mind_status_color(status, mind_theme(theme))
}

pub(crate) fn detached_job_attention_color(job: &InsightDetachedJob, theme: MissionTheme) -> Color {
    aoc_mind::detached_job_attention_color(job, mind_theme(theme))
}

pub(crate) fn detached_job_status_color(
    status: InsightDetachedJobStatus,
    theme: MissionTheme,
) -> Color {
    aoc_mind::detached_job_status_color(status, mind_theme(theme))
}

pub(crate) fn format_age(age: Option<i64>) -> String {
    aoc_mind::format_age(age)
}

pub(crate) fn age_meter(age: Option<i64>, online: bool) -> &'static str {
    aoc_mind::age_meter(age, online)
}

pub(crate) fn age_color(age: Option<i64>, online: bool, theme: MissionTheme) -> Color {
    aoc_mind::age_color(age, online, mind_theme(theme))
}

pub(crate) fn normalize_lifecycle(raw: &str) -> String {
    aoc_mind::normalize_lifecycle(raw)
}

pub(crate) fn lifecycle_color(lifecycle: &str, online: bool, theme: MissionTheme) -> Color {
    aoc_mind::lifecycle_color(lifecycle, online, mind_theme(theme))
}
