use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::io;

pub struct BufferBackend {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl BufferBackend {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); (width * height) as usize],
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.cells
            .resize((width * height) as usize, Cell::default());
        // Resize clears or preserves? Ratatui usually clears buffer on resize.
        // Let's just reset to default to be safe.
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
    }

    pub fn render_to_string(&self) -> String {
        let mut output = String::with_capacity((self.width * self.height * 10) as usize);

        // Zellij plugin: Disable line wrapping to prevent scroll-creep on exact-width writes
        output.push_str("\u{1b}[?7l");
        // Move cursor home
        output.push_str("\u{1b}[H");

        let mut last_fg = Color::Reset;
        let mut last_bg = Color::Reset;
        let mut last_modifier = Modifier::empty();

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                let cell = &self.cells[idx];

                // Diff style
                if cell.fg != last_fg || cell.bg != last_bg || cell.modifier != last_modifier {
                    output.push_str("\x1b[0m"); // Reset first to be safe

                    let mut codes = Vec::new();
                    if cell.fg != Color::Reset {
                        codes.push(color_to_ansi(cell.fg, true));
                    }
                    if cell.bg != Color::Reset {
                        codes.push(color_to_ansi(cell.bg, false));
                    }
                    if cell.modifier.contains(Modifier::BOLD) {
                        codes.push("1".to_string());
                    }
                    if cell.modifier.contains(Modifier::DIM) {
                        codes.push("2".to_string());
                    }
                    if cell.modifier.contains(Modifier::ITALIC) {
                        codes.push("3".to_string());
                    }
                    if cell.modifier.contains(Modifier::UNDERLINED) {
                        codes.push("4".to_string());
                    }
                    if cell.modifier.contains(Modifier::REVERSED) {
                        codes.push("7".to_string());
                    }

                    if !codes.is_empty() {
                        output.push_str("\x1b[");
                        output.push_str(&codes.join(";"));
                        output.push('m');
                    }

                    last_fg = cell.fg;
                    last_bg = cell.bg;
                    last_modifier = cell.modifier;
                }

                output.push_str(cell.symbol());
            }
            if y < self.height - 1 {
                output.push_str("\x1b[0m\n");
            } else {
                output.push_str("\x1b[0m");
            }
            last_fg = Color::Reset;
            last_bg = Color::Reset;
            last_modifier = Modifier::empty();
        }

        // Restore line wrapping
        output.push_str("\u{1b}[?7h");

        output
    }
}

fn color_to_ansi(color: Color, is_fg: bool) -> String {
    let base = if is_fg { 30 } else { 40 };
    match color {
        Color::Reset => {
            if is_fg {
                "39".to_string()
            } else {
                "49".to_string()
            }
        }
        Color::Black => format!("{}", base),
        Color::Red => format!("{}", base + 1),
        Color::Green => format!("{}", base + 2),
        Color::Yellow => format!("{}", base + 3),
        Color::Blue => format!("{}", base + 4),
        Color::Magenta => format!("{}", base + 5),
        Color::Cyan => format!("{}", base + 6),
        Color::Gray => format!("90"),
        Color::DarkGray => format!("90"),
        Color::White => format!("{}", base + 7),
        Color::Indexed(i) => {
            if is_fg {
                format!("38;5;{}", i)
            } else {
                format!("48;5;{}", i)
            }
        }
        Color::Rgb(r, g, b) => {
            if is_fg {
                format!("38;2;{};{};{}", r, g, b)
            } else {
                format!("48;2;{};{};{}", r, g, b)
            }
        }
        _ => "".to_string(),
    }
}

impl Backend for BufferBackend {
    fn draw<'a, I>(&mut self, content: I) -> Result<(), io::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            if x < self.width && y < self.height {
                let idx = (y * self.width + x) as usize;
                self.cells[idx] = cell.clone();
            }
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
    fn show_cursor(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
    fn get_cursor(&mut self) -> Result<(u16, u16), io::Error> {
        Ok((0, 0))
    }
    fn set_cursor(&mut self, _x: u16, _y: u16) -> Result<(), io::Error> {
        Ok(())
    }
    fn clear(&mut self) -> Result<(), io::Error> {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        Ok(())
    }
    fn size(&self) -> Result<Rect, io::Error> {
        Ok(Rect::new(0, 0, self.width, self.height))
    }
    fn window_size(&mut self) -> Result<WindowSize, io::Error> {
        Ok(WindowSize {
            columns_rows: (self.width, self.height).into(),
            pixels: (0, 0).into(),
        })
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}
