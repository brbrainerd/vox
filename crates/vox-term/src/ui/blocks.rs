//! Block list widget — renders `Session::blocks()` via the VT grid.

use ratatui::{layout::Rect, Frame};
use vox_terminal_core::block::{Block, BlockStatus};

use crate::vt::VtGrid;

/// Render all blocks into `area`. Each block gets a header line (input + status)
/// and its output rendered through a VT grid.
pub fn render(frame: &mut Frame, area: Rect, blocks: &[Block], grid: &mut VtGrid) {
    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
        widgets::Paragraph,
    };

    let mut lines: Vec<Line> = Vec::new();
    for block in blocks {
        let status_color = match block.status {
            BlockStatus::Ok => Color::Green,
            BlockStatus::Failed => Color::Red,
            BlockStatus::Running => Color::Yellow,
            BlockStatus::Cancelled => Color::DarkGray,
        };
        let header = Line::from(vec![
            Span::styled("❯ ", Style::default().fg(status_color)),
            Span::raw(block.input.clone()),
        ]);
        lines.push(header);

        // Feed raw output into VT grid and render.
        let raw: Vec<u8> = block.output.iter().flat_map(|c| c.text.bytes()).collect();
        grid.feed(&raw);
        for vt_line in grid.render_lines() {
            let trimmed: String = vt_line
                .spans
                .iter()
                .map(|s| s.content.trim_end())
                .collect::<Vec<_>>()
                .join("");
            if !trimmed.is_empty() {
                lines.push(vt_line);
            }
        }
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}
