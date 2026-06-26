//! VT-grid renderer: wraps `alacritty_terminal` to feed raw PTY bytes into a
//! VT100/VT220 grid and project it as `Vec<ratatui::text::Line>` (cells → styled spans).
//!
//! Bytes are parsed by `vte::ansi::Processor::advance(&mut term, bytes)`.
//! The `Term` grid handles ANSI SGR colors/attrs, cursor, scrollback, and
//! alternate-screen. Consecutive cells with identical style are coalesced into
//! one ratatui `Span` for efficiency.

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::Dimensions,
    index::{Column, Line, Point},
    term::{
        Config,
        cell::{Cell, Flags},
    },
    vte::ansi::{Color, NamedColor, Processor, Rgb},
};
use ratatui::{
    style::{Color as RColor, Modifier, Style},
    text::{Line as RLine, Span},
};

struct NoopListener;
impl EventListener for NoopListener {
    fn send_event(&self, _: Event) {}
}

struct GridSize {
    cols: usize,
    lines: usize,
}

impl Dimensions for GridSize {
    fn columns(&self) -> usize {
        self.cols
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn total_lines(&self) -> usize {
        self.lines
    }
}

/// A VT100/VT220 terminal grid backed by `alacritty_terminal`.
pub struct VtGrid {
    term: Term<NoopListener>,
    parser: Processor,
    cols: usize,
    rows: usize,
}

impl VtGrid {
    pub fn new(cols: u16, rows: u16) -> Self {
        let size = GridSize {
            cols: cols as usize,
            lines: rows as usize,
        };
        let term = Term::new(Config::default(), &size, NoopListener);
        Self {
            term,
            parser: Processor::new(),
            cols: cols as usize,
            rows: rows as usize,
        }
    }

    /// Feed raw bytes (PTY output) into the VT grid.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    /// Resize the grid.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = GridSize {
            cols: cols as usize,
            lines: rows as usize,
        };
        self.term.resize(size);
        self.cols = cols as usize;
        self.rows = rows as usize;
    }

    /// Project the visible screen into ratatui `Line`s with styled `Span`s.
    pub fn render_lines(&self) -> Vec<RLine<'static>> {
        let mut out = Vec::with_capacity(self.rows);
        for row in 0..self.rows {
            let line = Line(row as i32);
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut cur_style = Style::default();
            let mut cur_text = String::new();

            for col in 0..self.cols {
                let cell = self.term.grid()[Point::new(line, Column(col))].clone();
                let style = cell_to_style(&cell);
                let ch = if cell.c == '\0' { ' ' } else { cell.c };

                if style == cur_style {
                    cur_text.push(ch);
                } else {
                    if !cur_text.is_empty() {
                        spans.push(Span::styled(cur_text.clone(), cur_style));
                        cur_text.clear();
                    }
                    cur_style = style;
                    cur_text.push(ch);
                }
            }
            if !cur_text.is_empty() {
                spans.push(Span::styled(cur_text, cur_style));
            }
            out.push(RLine::from(spans));
        }
        out
    }
}

fn cell_to_style(cell: &Cell) -> Style {
    let mut style = Style::default();
    if let Some(c) = ansi_color_to_ratatui(cell.fg) {
        style = style.fg(c);
    }
    if let Some(c) = ansi_color_to_ratatui(cell.bg) {
        style = style.bg(c);
    }
    if cell.flags.contains(Flags::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.flags.contains(Flags::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.flags.contains(Flags::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.flags.contains(Flags::STRIKEOUT) {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if cell.flags.contains(Flags::DIM) {
        style = style.add_modifier(Modifier::DIM);
    }
    style
}

fn ansi_color_to_ratatui(color: Color) -> Option<RColor> {
    match color {
        Color::Named(n) => Some(named_to_ratatui(n)),
        Color::Spec(Rgb { r, g, b }) => Some(RColor::Rgb(r, g, b)),
        Color::Indexed(i) => Some(RColor::Indexed(i)),
    }
}

fn named_to_ratatui(n: NamedColor) -> RColor {
    match n {
        NamedColor::Black | NamedColor::DimBlack => RColor::Black,
        NamedColor::Red | NamedColor::DimRed => RColor::Red,
        NamedColor::Green | NamedColor::DimGreen => RColor::Green,
        NamedColor::Yellow | NamedColor::DimYellow => RColor::Yellow,
        NamedColor::Blue | NamedColor::DimBlue => RColor::Blue,
        NamedColor::Magenta | NamedColor::DimMagenta => RColor::Magenta,
        NamedColor::Cyan | NamedColor::DimCyan => RColor::Cyan,
        NamedColor::White | NamedColor::DimWhite => RColor::White,
        NamedColor::BrightBlack => RColor::DarkGray,
        NamedColor::BrightRed => RColor::LightRed,
        NamedColor::BrightGreen => RColor::LightGreen,
        NamedColor::BrightYellow => RColor::LightYellow,
        NamedColor::BrightBlue => RColor::LightBlue,
        NamedColor::BrightMagenta => RColor::LightMagenta,
        NamedColor::BrightCyan => RColor::LightCyan,
        NamedColor::BrightWhite => RColor::Gray,
        _ => RColor::Reset,
    }
}
