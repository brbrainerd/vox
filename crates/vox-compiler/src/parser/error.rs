use crate::ast::span::Span;

/// High-level parse failure category (stable for tooling; see `docs/src/reference/parser-ambiguity-inventory.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParseErrorClass {
    /// Generic / uncategorized until call sites adopt a finer class.
    #[default]
    Other,
    /// Token mismatch in `Parser::expect`.
    ExpectToken,
    /// Unknown or misplaced top-level construct.
    TopLevel,
    /// Declaration / attribute head or tail.
    Declaration,
    /// Misplaced or unknown token inside a Path C / `@component` reactive body (`state`, `view:`, …).
    ReactiveComponentMember,
    Expression,
    Statement,
    TypeExpr,
    /// Tombstoned / archived construct that is no longer supported.
    Tombstoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParseSeverity {
    #[default]
    Error,
    Warning,
}

/// Machine-readable fix for a retired construct, so an LLM/codemod can auto-apply
/// the replacement from data rather than parsing the English message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// The retired spelling, e.g. `@table`.
    pub from: String,
    /// The replacement spelling, e.g. `table`.
    pub to: String,
    /// Stable diagnostic code, e.g. `vox/decorator/table-retired`.
    pub code: String,
}

/// A parse error with detailed context.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub expected: Vec<String>,
    pub found: Option<String>,
    pub class: ParseErrorClass,
    pub severity: ParseSeverity,
    /// Set on `Tombstoned` diagnostics so tooling can auto-fix from data.
    pub replacement: Option<Replacement>,
}

impl ParseError {
    /// Build a parse diagnostic (span + message + optional expected/found hints).
    #[must_use]
    pub fn new(
        span: Span,
        message: impl Into<String>,
        expected: Vec<String>,
        found: Option<String>,
    ) -> Self {
        Self::classified(span, message, expected, found, ParseErrorClass::Other)
    }

    /// Same as [`ParseError::new`] with an explicit [`ParseErrorClass`].
    #[must_use]
    pub fn classified(
        span: Span,
        message: impl Into<String>,
        expected: Vec<String>,
        found: Option<String>,
        class: ParseErrorClass,
    ) -> Self {
        Self {
            message: message.into(),
            span,
            expected,
            found,
            class,
            severity: ParseSeverity::Error,
            replacement: None,
        }
    }

    /// Build a parse warning.
    #[must_use]
    pub fn warning(span: Span, message: impl Into<String>, class: ParseErrorClass) -> Self {
        Self {
            message: message.into(),
            span,
            expected: vec![],
            found: None,
            class,
            severity: ParseSeverity::Warning,
            replacement: None,
        }
    }

    /// A retired-construct diagnostic carrying a machine-readable [`Replacement`].
    /// During the warning-first rollout `severity` is `Warning` (both spellings
    /// parse); the final flip passes `Error` to make the old spelling illegal.
    #[must_use]
    pub fn retired_decorator(
        span: Span,
        from: impl Into<String>,
        to: impl Into<String>,
        code: impl Into<String>,
        severity: ParseSeverity,
    ) -> Self {
        let (from, to) = (from.into(), to.into());
        Self {
            message: format!("`{from}` is retired; use `{to}`"),
            span,
            expected: vec![to.clone()],
            found: Some(from.clone()),
            class: ParseErrorClass::Tombstoned,
            severity,
            replacement: Some(Replacement {
                from,
                to,
                code: code.into(),
            }),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::span::Span;

    #[test]
    fn retired_decorator_builds_machine_readable_payload() {
        let e = ParseError::retired_decorator(
            Span::new(0, 6),
            "@table",
            "table",
            "vox/decorator/table-retired",
            ParseSeverity::Warning,
        );
        assert_eq!(e.class, ParseErrorClass::Tombstoned);
        assert_eq!(e.severity, ParseSeverity::Warning);
        assert_eq!(e.message, "`@table` is retired; use `table`");
        assert_eq!(e.expected, vec!["table".to_string()]);
        assert_eq!(e.found.as_deref(), Some("@table"));
        let r = e.replacement.as_ref().expect("replacement payload present");
        assert_eq!(r.from, "@table");
        assert_eq!(r.to, "table");
        assert_eq!(r.code, "vox/decorator/table-retired");
    }

    #[test]
    fn retired_decorator_can_be_hard_error_at_flip() {
        // The warning→error flip passes ParseSeverity::Error; the payload is unchanged.
        let e = ParseError::retired_decorator(
            Span::new(0, 6),
            "@query",
            "query",
            "vox/decorator/query-retired",
            ParseSeverity::Error,
        );
        assert_eq!(e.severity, ParseSeverity::Error);
        assert_eq!(e.replacement.unwrap().to, "query");
    }
}
