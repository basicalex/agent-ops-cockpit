use ratatui::style::{Color, Modifier, Style};

pub const HEADER_STYLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const SELECTED_STYLE: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);
pub const NORMAL_STYLE: Style = Style::new().fg(Color::Reset);

pub mod icons {
    pub const CHECK: &str = "";
    pub const PENDING: &str = "";
    pub const IN_PROGRESS: &str = "";
    pub const BLOCKED: &str = "";
    pub const REVIEW: &str = "";
    pub const AGENT: &str = "󰚩";
    pub const PRIORITY_HIGH: &str = "🔥";
    pub const PRIORITY_MED: &str = "";
    pub const PRIORITY_LOW: &str = "";
    pub const EXPANDED: &str = "▼";
    pub const COLLAPSED: &str = "▶";
}

pub fn status_color(status: &str) -> Color {
    match status.to_lowercase().as_str() {
        "done" => Color::Green,
        "in-progress" => Color::Blue,
        "blocked" => Color::Red,
        "review" => Color::Yellow,
        "cancelled" => Color::Red,
        _ => Color::DarkGray,
    }
}

pub fn priority_color(priority: &str) -> Color {
    match priority.to_lowercase().as_str() {
        "high" => Color::Red,
        "medium" => Color::Yellow,
        "low" => Color::Blue,
        _ => Color::Gray,
    }
}
