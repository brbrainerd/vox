//! Value-semantic list methods must emit value-returning Rust.
//!
//! The interpreter's list `.push` is value-semantic (`eval/builtins.rs`: clones
//! the vec, pushes, and returns the NEW list), but Rust's `Vec::push` mutates in
//! place and returns `()`. So `xs = xs.push(y)` naively emits
//! `xs = xs.clone().push(y)` → assigning `()` to a `Vec` (E0308). Codegen must
//! emit a block that returns the updated vec.
//!
//! Fast: asserts on emitted strings — no crate compile needed.

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
fn push_emits_value_returning_block_not_unit() {
    let out =
        emit_first_fn("fn f(xs: List[str]) to int { let ys = xs.push(\"a\") return ys.len() }");
    // Must NOT be the broken `<recv>.push(<arg>)` (returns `()`); must be a block
    // that pushes then yields the vec.
    assert!(
        out.contains("__lst.push(") && out.contains("__lst }"),
        "`xs.push(y)` must emit a value-returning block (push then yield the vec); got:\n{out}"
    );
}

#[test]
fn non_push_method_is_unaffected() {
    // A read method like `.len()` must NOT gain the push block.
    let out = emit_first_fn("fn f(xs: List[str]) to int { return xs.len() as int }");
    assert!(
        !out.contains("__lst.push("),
        "non-push methods must be untouched; got:\n{out}"
    );
}
