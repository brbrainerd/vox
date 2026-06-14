//! Shared assertion helpers for compiler diagnostic tests.
//!
//! Provides typed helpers over `Vec<Diagnostic>` so test files don't
//! repeat the same filter/map patterns.

use vox_compiler::typeck::{Diagnostic, diagnostics::TypeckSeverity};

/// Returns `true` if `diags` contains at least one error-severity diagnostic.
pub fn has_error(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == TypeckSeverity::Error)
}

/// Returns the `message` field of every error-severity diagnostic in `diags`.
pub fn error_messages(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .map(|d| d.message.clone())
        .collect()
}

/// Returns the `message` field of every warning-severity diagnostic in `diags`.
pub fn warning_messages(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Warning)
        .map(|d| d.message.clone())
        .collect()
}

/// Asserts that `diags` contains zero error-severity diagnostics.
///
/// Prints all error messages on failure.
#[track_caller]
pub fn assert_no_errors(diags: &[Diagnostic]) {
    let errs = error_messages(diags);
    assert!(
        errs.is_empty(),
        "Expected no type errors, got:\n{}",
        errs.join("\n")
    );
}

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::typeck::diagnostics::TypeckSeverity;

    fn make_diag(msg: &str, sev: TypeckSeverity) -> vox_compiler::typeck::Diagnostic {
        let span = Span { start: 0, end: 0 };
        match sev {
            TypeckSeverity::Error => {
                vox_compiler::typeck::Diagnostic::error(msg.to_string(), span, "")
            }
            TypeckSeverity::Warning => {
                vox_compiler::typeck::Diagnostic::warning(msg.to_string(), span, "")
            }
        }
    }

    #[test]
    fn error_messages_returns_only_error_messages() {
        let diags = vec![
            make_diag("type mismatch", TypeckSeverity::Error),
            make_diag("unused variable", TypeckSeverity::Warning),
            make_diag("undefined name", TypeckSeverity::Error),
        ];
        let msgs = error_messages(&diags);
        assert_eq!(msgs.len(), 2);
        assert!(msgs.contains(&"type mismatch".to_string()));
        assert!(msgs.contains(&"undefined name".to_string()));
        assert!(!msgs.iter().any(|m| m.contains("unused")));
    }

    #[test]
    fn error_messages_empty_when_no_errors() {
        let diags = vec![make_diag("unused variable", TypeckSeverity::Warning)];
        let msgs = error_messages(&diags);
        assert!(msgs.is_empty());
    }

    #[test]
    fn warning_messages_returns_only_warning_messages() {
        let diags = vec![
            make_diag("type mismatch", TypeckSeverity::Error),
            make_diag("unused variable", TypeckSeverity::Warning),
            make_diag("dead code", TypeckSeverity::Warning),
        ];
        let msgs = warning_messages(&diags);
        assert_eq!(msgs.len(), 2);
        assert!(msgs.contains(&"unused variable".to_string()));
        assert!(msgs.contains(&"dead code".to_string()));
        assert!(!msgs.iter().any(|m| m.contains("mismatch")));
    }

    #[test]
    fn warning_messages_empty_when_no_warnings() {
        let diags = vec![make_diag("type mismatch", TypeckSeverity::Error)];
        let msgs = warning_messages(&diags);
        assert!(msgs.is_empty());
    }

    #[test]
    fn assert_no_errors_passes_on_empty_diags() {
        let diags: Vec<vox_compiler::typeck::Diagnostic> = vec![];
        assert_no_errors(&diags); // should not panic
    }

    #[test]
    fn assert_no_errors_passes_with_only_warnings() {
        let diags = vec![make_diag("unused", TypeckSeverity::Warning)];
        assert_no_errors(&diags); // should not panic
    }

    #[test]
    #[should_panic(expected = "Expected no type errors")]
    fn assert_no_errors_panics_on_error_diagnostic() {
        let diags = vec![make_diag("type mismatch", TypeckSeverity::Error)];
        assert_no_errors(&diags);
    }

    #[test]
    #[should_panic(expected = "type mismatch")]
    fn assert_no_errors_panic_includes_error_message() {
        let diags = vec![make_diag("type mismatch", TypeckSeverity::Error)];
        assert_no_errors(&diags);
    }
}
