//! Input box using reedline — submits via `classify()` → `Session`.

use ratatui::{layout::Rect, style::Style, widgets::Paragraph, Frame};
use vox_terminal_core::input::{classify, InputIntent};

/// Single-line input state backed by a reedline buffer.
///
/// `reedline`'s full event loop is designed for real terminals; here we use
/// only its line-buffer abstraction and drive events from crossterm ourselves.
pub struct InputBox {
    pub buf: String,
}

impl InputBox {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    pub fn push_char(&mut self, c: char) {
        self.buf.push(c);
    }

    pub fn backspace(&mut self) {
        self.buf.pop();
    }

    /// Submit the current buffer and return the parsed intent; clears the buffer.
    pub fn submit(&mut self) -> InputIntent {
        let intent = classify(&self.buf);
        self.buf.clear();
        intent
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, mode: &str) {
        use ratatui::text::{Line, Span};
        let line = Line::from(vec![
            Span::styled(format!("[{mode}] ❯ "), Style::default()),
            Span::raw(self.buf.clone()),
            Span::raw("█"), // cursor block
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }
}

impl Default for InputBox {
    fn default() -> Self {
        Self::new()
    }
}
