use vox_ast::span::Span;

/// A parse error with detailed context.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub expected: Vec<String>,
    pub found: Option<String>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.expected.is_empty() {
            write!(f, " (expected: {})", self.expected.join(", "))?;
        }
        if let Some(ref found) = self.found {
            write!(f, " (found: {found})")?;
        }
        Ok(())
    }
}
