// B-092 / B-093 / endpoint-readiness: LSP validate_document integration tests
use tower_lsp_server::ls_types::{DiagnosticSeverity, NumberOrString};
use vox_lsp::validate_document;

/// B-092: validate_document returns type error diagnostic at correct byte offset.
#[test]
fn b092_validate_document_error_at_correct_offset() {
    let src = r#"fn add(a: int, b: int) to int:
    ret a + b

fn main() to int:
    ret add(1, "oops")
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

/// Endpoint-grade pattern: validate_document emits vox/endpoint-readiness hint for @server or route.
#[test]
fn endpoint_readiness_hint_emitted_for_server_fn() {
    let src = r#"
@server fn chat(msg: str) to str:
    ret msg
"#;
    let diags = validate_document(src);
    let hints: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.severity == Some(DiagnosticSeverity::HINT)
                && d.code.as_ref().map(|c| match c {
                    NumberOrString::String(s) => s == "vox/endpoint-readiness",
                    _ => false,
                }) == Some(true)
        })
        .collect();
    assert!(
        !hints.is_empty(),
        "Document with @server fn should get endpoint-readiness hint, got {} diags: {:?}",
        diags.len(),
        diags
            .iter()
            .map(|d| (&d.message, d.code.as_ref()))
            .collect::<Vec<_>>(),
    );
    assert!(
        hints[0].message.contains("auth") && hints[0].message.contains("rate-limit"),
        "Hint should mention auth and rate-limit: {}",
        hints[0].message,
    );
}

/// When document has @server/route but no workflow/activity, emit vox/workflow-resilience hint.
#[test]
fn workflow_resilience_hint_emitted_when_no_workflow_or_activity() {
    let src = r#"
@server fn chat(msg: str) to str:
    ret msg
"#;
    let diags = validate_document(src);
    let resilience: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.severity == Some(DiagnosticSeverity::HINT)
                && d.code.as_ref().map(|c| match c {
                    NumberOrString::String(s) => s == "vox/workflow-resilience",
                    _ => false,
                }) == Some(true)
        })
        .collect();
    assert!(
        !resilience.is_empty(),
        "Document with @server and no workflow/activity should get workflow-resilience hint",
    );
    assert!(
        resilience[0].message.contains("workflow") && resilience[0].message.contains("retries"),
        "Hint should mention workflow and retries: {}",
        resilience[0].message,
    );
}

/// D3: goto_definition_finds_fn_decl
#[test]
fn goto_definition_finds_fn_decl() {
    use tower_lsp_server::ls_types::{Position, Uri};
    let src = "fn my_target() to int:\n    ret 42\n\nfn caller() to int:\n    ret my_target()\n";
    let uri: Uri = "file:///test.vox".parse().unwrap();
    // caller line is 3, "    ret my_target()" is line 4
    let pos = Position { line: 4, character: 10 };
    let loc = vox_lsp::definition_at(src, pos, uri.clone(), None);
    assert!(loc.is_some(), "Definition should be found for my_target");
    let loc = loc.unwrap();
    assert_eq!(loc.range.start.line, 0);
}
