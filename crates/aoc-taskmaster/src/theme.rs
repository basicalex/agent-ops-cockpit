use ratatui::style::{Color, Modifier, Style};

pub const HEADER_STYLE: Style = Style::new()
    .fg(Color::Rgb(142, 192, 124))
    .add_modifier(Modifier::BOLD);
pub const SELECTED_STYLE: Style = Style::new()
    .bg(Color::Rgb(131, 165, 152))
    .fg(Color::Black)
    .add_modifier(Modifier::BOLD);

pub fn zebra_row_style(index: usize) -> Style {
    let bg = if index % 2 == 0 {
        Color::Rgb(18, 20, 26)
    } else {
        Color::Rgb(24, 27, 34)
    };
    Style::new().bg(bg)
}

pub fn tag_badge_style(tag: &str, seed: u64) -> Style {
    let palette = [
        Color::Rgb(131, 165, 152),
        Color::Rgb(69, 133, 136),
        Color::Rgb(142, 192, 124),
        Color::Rgb(104, 157, 106),
        Color::Rgb(184, 187, 38),
        Color::Rgb(152, 151, 26),
        Color::Rgb(250, 189, 47),
        Color::Rgb(215, 153, 33),
        Color::Rgb(254, 128, 25),
        Color::Rgb(214, 93, 14),
        Color::Rgb(211, 134, 155),
        Color::Rgb(177, 98, 134),
        Color::Rgb(189, 174, 147),
        Color::Rgb(168, 153, 132),
    ];
    let mut hash: u64 = 1469598103934665603 ^ seed;
    for b in tag.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    let color = palette[(hash as usize) % palette.len()];
    Style::new().fg(color).add_modifier(Modifier::BOLD)
}
pub mod icons {
    pub const CHECK: &str = "x";
    pub const PENDING: &str = ".";
    pub const IN_PROGRESS: &str = ">";
    pub const BLOCKED: &str = "!";
    pub const REVIEW: &str = "?";
    pub const AGENT: &str = "*";
    pub const PRIORITY_HIGH: &str = "!";
    pub const PRIORITY_MED: &str = "~";
    pub const PRIORITY_LOW: &str = "-";
    pub const EXPANDED: &str = "v";
    pub const COLLAPSED: &str = ">";
}

pub fn status_color(status: &str) -> Color {
    match status.to_lowercase().as_str() {
        "done" => Color::Rgb(184, 187, 38),
        "in-progress" => Color::Rgb(131, 165, 152),
        "blocked" => Color::Rgb(254, 128, 25),
        "review" => Color::Rgb(250, 189, 47),
        "cancelled" => Color::Rgb(214, 93, 14),
        _ => Color::Rgb(146, 131, 116),
    }
}

pub fn priority_color(priority: &str) -> Color {
    match priority.to_lowercase().as_str() {
        "high" => Color::Rgb(254, 128, 25),
        "medium" => Color::Rgb(250, 189, 47),
        "low" => Color::Rgb(131, 165, 152),
        _ => Color::Rgb(146, 131, 116),
    }
}
