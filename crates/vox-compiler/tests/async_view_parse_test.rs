//! Acceptance tests for `Async[T] when { fetching => … empty => … error e => … ok x => … }`.

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const FULL_WHEN: &str = r#"
fn v(d: Int) -> Int {
    when d {
        fetching => 0
        empty => 0
        error e => -1
        ok x => x
    }
}
"#;

#[test]
fn when_view_parses_to_async_view_expr() {
    parse(lex(FULL_WHEN)).expect("when{} must parse successfully");
}

#[test]
fn when_view_lowers_to_hir_async_view() {
    let m = parse(lex(FULL_WHEN)).expect("parse");
    let hir = lower_module(&m);
    // The function body must contain an AsyncView node somewhere.
    let fn_body = &hir.functions.first().expect("one fn").body;
    let has_async_view = fn_body.iter().any(|stmt| {
        use vox_compiler::hir::HirStmt;
        match stmt {
            HirStmt::Return { value: Some(e), .. } => {
                matches!(e, vox_compiler::hir::HirExpr::AsyncView(_))
            }
            HirStmt::Expr { expr: e, .. } => {
                matches!(e, vox_compiler::hir::HirExpr::AsyncView(_))
            }
            _ => false,
        }
    });
    assert!(
        has_async_view,
        "lowered HIR must contain HirExpr::AsyncView; body: {:?}",
        fn_body
    );
}

#[test]
fn when_view_partial_arms_parse() {
    // Only fetching + ok — the parser allows optional arms.
    let src = r#"
fn v(d: Int) -> Int {
    when d {
        fetching => 0
        ok x => x
    }
}
"#;
    parse(lex(src)).expect("partial when{} must parse");
}

#[test]
fn when_view_source_expr_is_arbitrary() {
    // Source can be any expression — a function call here.
    let src = r#"
fn v() -> Int {
    when fetch_data() {
        fetching => 0
        empty => -1
        error e => -2
        ok x => x
    }
}
"#;
    parse(lex(src)).expect("when{} with call source must parse");
}
