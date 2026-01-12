use crate::theme::{colors, icons};

pub struct ProgressBar {
    pub total: usize,
    pub current: usize,
    pub width: usize,
}

impl ProgressBar {
    pub fn new(current: usize, total: usize, width: usize) -> Self {
        Self {
            current,
            total,
            width,
        }
    }

    pub fn render(&self) -> String {
        if self.total == 0 {
            return format!("[{}]", " ".repeat(self.width));
        }
        let percent = (self.current as f64 / self.total as f64).clamp(0.0, 1.0);
        let filled = (percent * self.width as f64).round() as usize;
        let empty = self.width.saturating_sub(filled);

        let bar = "█".repeat(filled);
        let space = "░".repeat(empty);

        let color = if percent < 0.33 {
            colors::RED
        } else if percent < 0.66 {
            colors::YELLOW
        } else {
            colors::GREEN
        };

        format!("{}{}{}{}{}{}", colors::DIM, "[", color, bar, colors::DIM, space)
            .replace("][", "")
            + &format!("{}{}", "]", colors::RESET)
    }
}

pub struct UiTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub column_widths: Vec<usize>,
}

impl UiTable {
    pub fn new(headers: Vec<&str>) -> Self {
        Self {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: Vec::new(),
            column_widths: headers.iter().map(|s| s.len()).collect(),
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        if row.len() != self.headers.len() {
            return;
        }
        for (i, cell) in row.iter().enumerate() {
            let len = strip_ansi(cell).chars().count();
            if len > self.column_widths[i] {
                self.column_widths[i] = len;
            }
        }
        self.rows.push(row);
    }

    pub fn render(&self, cols: usize) -> Vec<String> {
        let mut lines = Vec::new();
        
        let header_line = self.render_row(&self.headers, true);
        lines.push(truncate_visible(&header_line, cols));
        
        let mut sep = String::new();
        for (i, width) in self.column_widths.iter().enumerate() {
             sep.push_str(&"-".repeat(*width));
             if i < self.column_widths.len() - 1 {
                 sep.push_str("  ");
             }
        }
        lines.push(truncate_visible(&sep, cols));

        for row in &self.rows {
            let line = self.render_row(row, false);
            lines.push(truncate_visible(&line, cols));
        }

        lines
    }

    fn render_row(&self, row: &[String], is_header: bool) -> String {
        let mut out = String::new();
        for (i, cell) in row.iter().enumerate() {
            let width = self.column_widths[i];
            let cell_len = strip_ansi(cell).chars().count();
            let pad = width.saturating_sub(cell_len);
            
            if is_header {
                out.push_str(colors::BOLD);
            }
            out.push_str(cell);
            if is_header {
                out.push_str(colors::RESET);
            }
            
            if i < row.len() - 1 {
                out.push_str(&" ".repeat(pad));
                out.push_str("  "); 
            }
        }
        out
    }
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        out.push(ch);
    }
    out
}

pub fn status_symbol(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "done" => format!("{}{}{}", colors::SUCCESS, icons::CHECK, colors::RESET),
        "cancelled" => format!("{}{}{}", colors::ERROR, icons::BLOCKED, colors::RESET),
        "in-progress" => format!("{}{}{}", colors::INFO, icons::IN_PROGRESS, colors::RESET),
        "review" => format!("{}{}{}", colors::CYAN, icons::UNKNOWN, colors::RESET),
        "pending" => format!("{}{}{}", colors::WARNING, icons::PENDING, colors::RESET),
        _ => format!("{}{}{}", colors::DIM, icons::PENDING, colors::RESET),
    }
}

pub fn colorize_status(label: &str, status: &str) -> String {
    let color = match status.to_lowercase().as_str() {
        "done" => colors::SUCCESS,
        "cancelled" => colors::ERROR,
        "in-progress" => colors::INFO,
        "review" => colors::CYAN,
        "pending" => colors::WARNING,
        _ => colors::DIM,
    };
    format!("{}{}{}", color, label, colors::RESET)
}

pub fn colorize_priority(label: &str, priority: &str) -> String {
    let color = match priority.to_lowercase().as_str() {
        "high" => colors::RED,
        "medium" => colors::YELLOW,
        "low" => colors::BLUE,
        _ => colors::DIM,
    };
    // Removed unused icon variable
    format!("{}{}{}", color, label, colors::RESET)
}

pub fn pad_right(input: &str, width: usize) -> String {
    let mut out = String::new();
    let stripped = strip_ansi(input);
    let count = stripped.chars().count();
    
    out.push_str(input);
    if count < width {
        out.push_str(&" ".repeat(width - count));
    }
    out
}

pub fn wrap_block(label: &str, value: &str, cols: usize) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(truncate_visible(&format!("{}{}:{}", colors::BOLD, label, colors::RESET), cols));
    let width = cols.saturating_sub(2);
    if width == 0 {
        return lines;
    }
    for raw in value.lines() {
        let mut buf = String::new();
        let mut count = 0usize;
        for ch in raw.chars() {
            if count >= width {
                lines.push(truncate_visible(&format!("  {}", buf), cols));
                buf.clear();
                count = 0;
            }
            buf.push(ch);
            count += 1;
        }
        if !buf.is_empty() || raw.is_empty() {
            lines.push(truncate_visible(&format!("  {}", buf), cols));
        }
    }
    lines
}

pub fn draw_subtask_tree(
    subtasks: &[crate::model::Subtask], 
    indent: usize, 
    cols: usize,
    cursor: usize,
    is_focused: bool
) -> Vec<String> {
    let mut lines = Vec::new();
    // Flatten logic is simpler for single level, but recursive needs care.
    // For now, assuming flat subtasks list in UI cursor logic for simplicity 
    // (since model is flat Vec<Subtask> mostly, but we have recursive struct).
    // If recursive, cursor tracking is hard.
    // Let's assume we flatten them for display? 
    // Or just support top-level subtasks navigation for now (Task #15 implemented recursive rendering but model has Vec<Subtask>).
    // Let's support top-level cursor for now to match state.subtask_cursor.
    
    for (i, sub) in subtasks.iter().enumerate() {
        let is_last = i == subtasks.len() - 1;
        let branch = if is_last { "└─ " } else { "├─ " };
        let symbol = status_symbol(&sub.status);
        let prefix = "  ".repeat(indent);
        
        let mut line = format!("{}{}{} {}", prefix, branch, symbol, sub.title);
        
        // Highlight logic
        if is_focused && i == cursor && indent == 0 {
             line = format!("{}{}{}", colors::MAGENTA, line, colors::RESET);
             // Add a pointer?
             line = format!("{} {}", ">", line); 
        } else {
             line = format!("  {}", line);
        }
        
        lines.push(truncate_visible(&line, cols));
        
        if !sub.subtasks.is_empty() {
            // Recursive children - not selectable yet with current simple cursor
            let child_indent = indent + 1;
            // Pass a dummy cursor for children or handle deep selection later
            lines.extend(draw_subtask_tree(&sub.subtasks, child_indent, cols, 9999, false));
        }
    }
    lines
}

pub fn draw_error_modal(err: &str, rows: usize, cols: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let border = colors::RED;
    let reset = colors::RESET;
    
    // Calculate dimensions
    let width = (cols as f64 * 0.8) as usize;
    let height = (rows as f64 * 0.4) as usize;
    let start_x = (cols - width) / 2;
    let start_y = (rows - height) / 2;
    
    // Prepare blank canvas for full screen to "dim" background (not possible in simple TUI without layering, 
    // so we will just overwrite lines in the main render loop or return a full screen buffer)
    // Actually, in Zellij, we just return lines. To "modal" it, we replace the content or overlay it.
    // Since we output line-by-line, we can't easily "overlay" on top of existing content in a single pass 
    // without a buffer system. 
    // Simplified approach: Return a full-screen message blocking the UI.
    
    for _ in 0..start_y {
        lines.push(String::new());
    }
    
    let h_line = "─".repeat(width.saturating_sub(2));
    lines.push(format!("{}{}{}{}{}{}", " ".repeat(start_x), border, "┌", h_line, "┐", reset));
    
    let msg_lines = wrap_block("Error", err, width.saturating_sub(4));
    // wrap_block includes a header line we might not want, so let's just manual wrap
    
    let content_height = height.saturating_sub(2);
    let mut current_line = 0;
    
    for line in err.lines() {
        if current_line >= content_height { break; }
        // Simple truncation for now, real wrapping is complex without a crate
        let line_width = width.saturating_sub(4);
        let truncated = truncate_visible(line, line_width);
        lines.push(format!("{}{}{}{}{}{}{}", " ".repeat(start_x), border, "│ ", reset, truncated, " ".repeat(line_width - truncated.len()), format!("{} {}│{}", reset, border, reset)));
        current_line += 1;
    }
    
    while current_line < content_height {
        lines.push(format!("{}{}{}{}{}", " ".repeat(start_x), border, "│", " ".repeat(width.saturating_sub(2)), "│"));
        current_line += 1;
    }
    
    lines.push(format!("{}{}{}{}{}{}", " ".repeat(start_x), border, "└", h_line, "┘", reset));
    
    lines.push(format!("{:^width$}", "[Esc/Enter] to dismiss", width = cols));
    
    lines
}

pub fn truncate_visible(input: &str, cols: usize) -> String {
    if cols == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut visible = 0usize;
    let mut in_escape = false;
    for ch in input.chars() {
        if in_escape {
            out.push(ch);
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            out.push(ch);
            continue;
        }
        if visible >= cols {
            break;
        }
        out.push(ch);
        visible += 1;
    }
    out
}