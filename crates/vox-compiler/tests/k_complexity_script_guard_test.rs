//! Script K-complexity guards — syntactic configurability and retired surface errors.

use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::pipeline::run_frontend_str;

#[test]
fn macro_rules_ident_still_errors_e091() {
    let source = "macro_rules! my_macro { () => {} }";
    let res = run_frontend_str(source, "test.vox").expect("frontend");
    assert_eq!(res.diagnostics.len(), 1);
    assert_eq!(res.diagnostics[0].code.as_deref(), Some("E091"));
}

#[test]
fn macro_ident_still_errors_e091() {
    let source = "macro foo() { }";
    let res = run_frontend_str(source, "test.vox").expect("frontend");
    assert!(
        res.diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("E091")),
        "expected E091 for macro, got {} diagnostic(s)",
        res.diagnostics.len()
    );
}

#[test]
fn retired_component_fn_errors_at_parse() {
    let src = "@component fn Chat() to Element { return 0 }";
    let err = parse_script(lex(src));
    assert!(err.is_err(), "retired @component fn must fail parse");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("Retired")
            || msg.contains("retired")
            || msg.contains("tombstoned")
            || msg.contains("Tombstoned"),
        "expected retirement/tombstone message, got {msg}"
    );
}
