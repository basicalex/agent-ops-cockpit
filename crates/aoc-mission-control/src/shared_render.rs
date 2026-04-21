//! Shared render/layout helpers.
//!
//! Extracted from main.rs (Phase 2).

use super::*;

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100u16.saturating_sub(percent_y)) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100u16.saturating_sub(percent_y)) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100u16.saturating_sub(percent_x)) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100u16.saturating_sub(percent_x)) / 2),
        ])
        .split(vertical[1])[1]
}

pub(crate) fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

pub(crate) fn short_project(project_root: &str, max: usize) -> String {
    let leaf = Path::new(project_root)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(project_root);
    ellipsize(leaf, max)
}

pub(crate) fn scope_summary(scopes: &[String]) -> String {
    if scopes.is_empty() {
        return "local".to_string();
    }
    if scopes.len() == 1 {
        return scopes[0].clone();
    }
    format!("{}+{}", scopes[0], scopes.len() - 1)
}

pub(crate) fn ellipsize(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    if max <= 3 {
        return "...".chars().take(max).collect();
    }
    let prefix: String = input.chars().take(max - 3).collect();
    format!("{prefix}...")
}

pub(crate) fn fit_fields(fields: &[String], max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut output = String::new();
    for field in fields {
        if field.trim().is_empty() {
            continue;
        }
        let candidate = if output.is_empty() {
            field.clone()
        } else {
            format!("{output} | {field}")
        };
        if candidate.chars().count() <= max {
            output = candidate;
            continue;
        }
        if output.is_empty() {
            return ellipsize(field, max);
        }
        break;
    }
    output
}

pub(crate) fn is_compact(width: u16) -> bool {
    width < COMPACT_WIDTH
}
