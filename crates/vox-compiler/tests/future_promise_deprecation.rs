//! P1-T2 acceptance: `Future[T]` and `Promise[T]` produce deprecation warnings with rewrite fix.

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn diag_codes(src: &str) -> Vec<String> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn future_t_deprecation_warning() {
    let codes = diag_codes("fn f() to Future[int] { return 0 }");
    assert!(
        codes.iter().any(|c| c == "vox/types/future-deprecated"),
        "expected deprecation warning; got {codes:?}"
    );
}

#[test]
fn promise_t_deprecation_warning() {
    let codes = diag_codes("fn f() to Promise[str] { return \"\" }");
    assert!(
        codes.iter().any(|c| c == "vox/types/promise-deprecated"),
        "expected deprecation warning; got {codes:?}"
    );
}

#[test]
fn fix_suggests_durable_promise() {
    let src = "fn f() to Future[int] { return 0 }";
    let m = parse(lex(src)).expect("parse");
    let diags = typecheck_ast_module(src, &m);
    let dep = diags
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/types/future-deprecated"))
        .expect("must have deprecation diag");
    let fix = dep
        .fixes
        .iter()
        .find(|f| f.replacement.contains("DurablePromise"))
        .expect("must offer a DurablePromise rewrite fix");
    assert_eq!(fix.replacement.trim(), "DurablePromise[int]");
}
