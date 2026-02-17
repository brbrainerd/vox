use vox_ast::span::Span;

/// Type checking diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Severity {
    Error,
    Warning,
}

/// A structured diagnostic emitted by the type checker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub expected_type: Option<String>,
    pub found_type: Option<String>,
    pub suggestions: Vec<String>,
}
