//! P1-T1 acceptance: `DurablePromise[T]` is recognised as a stdlib type constructor.

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn diags(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
}

#[test]
fn durable_promise_is_a_known_type_constructor() {
    // A function returning DurablePromise[i32] should not produce
    // "unknown type" diagnostics for the DurablePromise name itself.
    let src = r#"
        fn pretend() -> DurablePromise[int] {
            return panic("compiler intrinsic")
        }
    "#;
    let ds = diags(src);
    let unknown_type_diags: Vec<_> = ds
        .iter()
        .filter(|d| d.message.contains("unknown type") && d.message.contains("DurablePromise"))
        .collect();
    assert!(
        unknown_type_diags.is_empty(),
        "DurablePromise should be a built-in type ctor; got: {unknown_type_diags:?}"
    );
}

#[test]
fn durable_promise_demands_one_type_argument() {
    // Bare `DurablePromise` without type arg should produce the arity diagnostic.
    let src = r#"
        fn bad() -> DurablePromise {
            return panic("missing type arg")
        }
    "#;
    let ds = diags(src);
    assert!(
        ds.iter()
            .any(|d| d.code.as_deref() == Some("vox/types/durable-promise-arity")),
        "expected vox/types/durable-promise-arity diagnostic; got: {:?}",
        ds.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}
