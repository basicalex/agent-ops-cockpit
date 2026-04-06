use std::ops::Sub;

use chrono::{Duration, Local};
use zellij_tile::prelude::PipeMessage;

use crate::{
    config::{RuntimeTheme, UpdateEventMask, ZellijState},
    widgets::{command::TIMESTAMP_FORMAT, notification},
};

/// Parses the line protocol and updates the state accordingly
///
/// The protocol is as follows:
///
/// zjstatus::command_name::args
///
/// It first starts with `zjstatus` as a prefix to indicate that the line is
/// used for the line protocol and zjstatus should parse it. It is followed
/// by the command name and then the arguments. The following commands are
/// available:
///
/// - `rerun` - Reruns the command with the given name (like in the config) as
///             argument. E.g. `zjstatus::rerun::command_1`
///
/// The function returns a boolean indicating whether the state has been
/// changed and the UI should be re-rendered.
#[tracing::instrument(skip(state))]
pub fn parse_protocol(state: &mut ZellijState, input: &str) -> bool {
    tracing::debug!("parsing protocol");
    let lines = input.split('\n').collect::<Vec<&str>>();

    let mut should_render = false;
    for line in lines {
        let line_renders = process_line(state, line);

        if line_renders {
            should_render = true;
        }
    }

    should_render
}

pub fn handle_pipe_message(state: &mut ZellijState, pipe_message: &PipeMessage) -> bool {
    if pipe_message.name == "aoc_theme" {
        return update_aoc_theme_from_payload(state, pipe_message.payload.as_deref().unwrap_or(""));
    }

    if let Some(input) = pipe_message.payload.as_deref() {
        return parse_protocol(state, input);
    }

    false
}

#[tracing::instrument(skip_all)]
fn process_line(state: &mut ZellijState, line: &str) -> bool {
    let parts = line.split("::").collect::<Vec<&str>>();

    if parts.len() < 3 {
        return false;
    }

    if parts[0] != "zjstatus" {
        return false;
    }

    tracing::debug!("command: {}", parts[1]);

    let mut should_render = false;
    #[allow(clippy::single_match)]
    match parts[1] {
        "rerun" => {
            rerun_command(state, parts[2]);

            should_render = true;
        }
        "notify" => {
            notify(state, parts[2]);

            should_render = true;
        }
        "pipe" => {
            if parts.len() < 4 {
                return false;
            }

            pipe(state, parts[2], parts[3]);

            should_render = true;
        }
        _ => {}
    }

    should_render
}

fn pipe(state: &mut ZellijState, name: &str, content: &str) {
    tracing::debug!("saving pipe result {name} {content}");
    state
        .pipe_results
        .insert(name.to_owned(), content.to_owned());
}

fn notify(state: &mut ZellijState, message: &str) {
    state.incoming_notification = Some(notification::Message {
        body: message.to_string(),
        received_at: Local::now(),
    });
}

fn rerun_command(state: &mut ZellijState, command_name: &str) {
    let command_result = state.command_results.get(command_name);

    if command_result.is_none() {
        return;
    }

    let mut command_result = command_result.unwrap().clone();

    let ts = Sub::<Duration>::sub(Local::now(), Duration::try_days(1).unwrap());

    command_result.context.insert(
        "timestamp".to_string(),
        ts.format(TIMESTAMP_FORMAT).to_string(),
    );

    state.command_results.remove(command_name);
    state
        .command_results
        .insert(command_name.to_string(), command_result.clone());
}

fn update_aoc_theme_from_payload(state: &mut ZellijState, payload: &str) -> bool {
    let mut next_theme = RuntimeTheme::default();

    for raw_line in payload.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };

        let value = raw_value.trim().trim_matches('"').to_string();
        match key.trim() {
            "AOC_THEME_BLUE" => next_theme.blue = value,
            "AOC_THEME_GREEN" => next_theme.green = value,
            "AOC_THEME_RED" => next_theme.red = value,
            "AOC_THEME_ORANGE" => next_theme.orange = value,
            "AOC_THEME_YELLOW" => next_theme.yellow = value,
            "AOC_THEME_CYAN" => next_theme.cyan = value,
            "AOC_THEME_MAGENTA" => next_theme.magenta = value,
            "AOC_THEME_BLACK" => next_theme.black = value,
            "AOC_THEME_WHITE" => next_theme.white = value,
            "AOC_THEME_BG_BASE" => next_theme.bg_base = value,
            "AOC_THEME_BG_ELEVATED" => next_theme.bg_elevated = value,
            "AOC_THEME_BG_SUBTLE" => next_theme.bg_subtle = value,
            "AOC_THEME_BG_ACCENT" => next_theme.bg_accent = value,
            "AOC_THEME_UI_PRIMARY" => next_theme.ui_primary = value,
            "AOC_THEME_UI_MUTED" => next_theme.ui_muted = value,
            "AOC_THEME_UI_ACCENT" => next_theme.ui_accent = value,
            _ => {}
        }
    }

    if next_theme == RuntimeTheme::default() || next_theme == state.runtime_theme {
        return false;
    }

    state.runtime_theme = next_theme;
    state.cache_mask = UpdateEventMask::Tab as u8;
    true
}
