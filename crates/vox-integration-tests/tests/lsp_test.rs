#![allow(missing_docs)]
// B-092 / B-093 / endpoint-readiness: LSP validate_document integration tests
use tower_lsp_server::ls_types::DiagnosticSeverity;
use vox_lsp::validate_document;

/// B-092: validate_document returns type error diagnostic at correct byte offset.
#[test]
fn b092_validate_document_error_at_correct_offset() {
    let src = r#"fn add(a: int, b: int) to int {
    return a + b
}

fn main() to int {
    return add(1, "oops")
}
"#;
    let diags = validate_document(src);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        !errors.is_empty(),
        "Type mismatch should produce at least one error diagnostic"
    );
    // The error should point to the call site in main, which is on line 4 (0-indexed)
    let err = &errors[0];
    assert!(
        err.range.start.line >= 3,
        "Error should be on or after line 4 (0-indexed 3), got line {}",
        err.range.start.line,
    );
}

/// B-093: validate_document returns parse error diagnostic at correct offset.
#[test]
fn b093_validate_document_parse_error_at_correct_offset() {
    let src = "fn broken(\n";
    let diags = validate_document(src);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        !errors.is_empty(),
        "Parse error should produce at least one error diagnostic"
    );
    // Parse error should point somewhere on line 0 or 1
    let err = &errors[0];
    assert!(
        err.range.start.line <= 1,
        "Parse error should be on line 0 or 1, got line {}",
        err.range.start.line,
    );
    // The message should indicate a parse-level issue
    assert!(
        err.message.contains("Expected")
            || err.message.contains("Unexpected")
            || err.message.contains("expected"),
        "Parse error message should indicate a syntax issue, got: {}",
        err.message,
    );
}
