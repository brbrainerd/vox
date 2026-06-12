//! Match-arm blocks with a trailing expression must lower to Rust tail expressions
//! (no semicolon on the last stmt), not `()` from `expr;`.

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module;

fn emit_first_fn(src: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir.functions.first().expect("at least one function");
    emit_fn(f, Some(&hir.inferred_types), &[])
}

#[test]
fn match_arm_block_trailing_expr_has_no_semicolon() {
    let out = emit_first_fn(
        r#"fn f() to Option[str] {
            return match Some(1) {
                Some(x) => {
                    let y = x + 1
                    Some(str(y))
                }
                None => None
            }
        }"#,
    );
    assert!(
        out.contains("Some(x) => {\n") || out.contains("Some(x) => {"),
        "expected match arm block; got:\n{out}"
    );
    assert!(
        !out.contains("Some(str(y));\n    }"),
        "trailing match-arm expression must not get a semicolon; got:\n{out}"
    );
    assert!(
        out.contains("Some(as_string(&y))\n") || out.contains("Some(str(y))\n"),
        "expected tail expression without semicolon; got:\n{out}"
    );
}
