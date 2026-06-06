/// Source span for tracking positions in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// Map a UTF-8 **byte** offset into `text` to LSP-style **0-based** line and column.
///
/// Column counts **Unicode scalar values** on the current line (same convention as the Vox LSP
/// before any UTF-16 code-unit adjustment).
#[must_use]
pub fn byte_offset_to_line_col_zero_based(text: &str, byte_index: usize) -> (u32, u32) {
    let byte_index = byte_index.min(text.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for (byte_idx, c) in text.char_indices() {
        if byte_idx >= byte_index {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}
