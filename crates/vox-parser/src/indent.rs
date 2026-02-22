//! Indentation tracking for the parser.
//! The lexer already injects Indent/Dedent/Newline tokens,
//! so this module provides utilities for the parser to track block depth.

/// Tracks indentation context during parsing.
pub struct IndentTracker {
    depth: usize,
}

impl IndentTracker {
    pub fn new() -> Self {
        Self { depth: 0 }
    }

    /// Enter a new block (after seeing an Indent token).
    pub fn enter_block(&mut self) {
        self.depth += 1;
    }

    /// Exit a block (after seeing a Dedent token).
    pub fn exit_block(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }

    /// Current indentation depth.
    pub fn depth(&self) -> usize {
        self.depth
    }
}

impl Default for IndentTracker {
    fn default() -> Self {
        Self::new()
    }
}
