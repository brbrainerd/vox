//! P1-T3 acceptance: `@remote fn` is parsed and typechecked correctly.

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn diags(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
}

#[test]
fn remote_fn_with_serializable_params_is_accepted() {
    // int and str are serializable — no diagnostics expected.
    let src = r#"
        @remote
        fn send_greeting(user_id: int, msg: str) to str {
            return msg
        }
    "#;
    let ds = diags(src);
    let remote_diags: Vec<_> = ds
        .iter()
        .filter(|d| d.code.as_deref() == Some("vox/remote/non-serializable-param"))
        .collect();
    assert!(
        remote_diags.is_empty(),
        "@remote fn with int/str params should have no serialization diagnostic; got: {remote_diags:?}"
    );
}

#[test]
fn remote_fn_with_raw_fn_param_is_rejected() {
    // A function-typed parameter is not serializable.
    let src = r#"
        @remote
        fn bad_dispatch(callback: fn(int) to str) to str {
            return callback(1)
        }
    "#;
    let ds = diags(src);
    assert!(
        ds.iter()
            .any(|d| d.code.as_deref() == Some("vox/remote/non-serializable-param")),
        "expected vox/remote/non-serializable-param for fn-typed param; got: {:?}",
        ds.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

#[test]
fn non_remote_fn_with_fn_param_is_not_flagged() {
    // Without @remote, fn-typed params are fine.
    let src = r#"
        fn local_higher_order(callback: fn(int) to str) to str {
            return callback(1)
        }
    "#;
    let ds = diags(src);
    let remote_diags: Vec<_> = ds
        .iter()
        .filter(|d| d.code.as_deref() == Some("vox/remote/non-serializable-param"))
        .collect();
    assert!(
        remote_diags.is_empty(),
        "non-@remote fn should never get serialization diagnostics; got: {remote_diags:?}"
    );
}
