//! `<list>.get(i) is Some(x)` must emit type-correct Rust.
//!
//! The interpreter's list `.get` is value-semantic (`v.get(i).cloned()` →
//! `Option<T>`), but codegen emits a bare `Vec::get` (→ `Option<&T>`). Comparing
//! that against `Some(owned)` (`Option<T>`) fails to type-check
//! (`expected Option<&String>, found Option<String>`). Codegen must `.cloned()`
//! the borrowing `.get` so both sides are owned.
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
fn get_is_some_clones_the_borrow() {
    let out = emit_first_fn("fn f(xs: List[str]) to bool { return xs.get(0) is Some(\"a\") }");
    assert!(
        out.contains(".cloned()"),
        "`.get(i) is Some(..)` must `.cloned()` the borrowing get so it compares \
         as Option<T> not Option<&T>; got:\n{out}"
    );
}

#[test]
fn get_isnt_some_clones_the_borrow() {
    let out = emit_first_fn("fn f(xs: List[str]) to bool { return xs.get(0) isnt Some(\"a\") }");
    assert!(
        out.contains(".cloned()"),
        "`.get(i) isnt Some(..)` must `.cloned()` the borrowing get; got:\n{out}"
    );
}

#[test]
fn assert_get_is_some_clones_the_borrow() {
    // `assert(x is y)` is special-cased to `assert_eq!(x, y)` — it must apply the
    // same `.cloned()` normalization as the plain `is` path.
    let out = emit_first_fn("fn f(xs: List[str]) { assert(xs.get(0) is Some(\"a\")) }");
    assert!(
        out.contains("assert_eq!") && out.contains(".cloned()"),
        "`assert(.get(i) is Some(..))` must emit assert_eq! with a cloned get; got:\n{out}"
    );
}

#[test]
fn plain_string_eq_is_unaffected() {
    // A non-get `is` comparison must NOT gain a spurious `.cloned()`.
    let out = emit_first_fn("fn f(a: str, b: str) to bool { return a is b }");
    assert!(
        !out.contains(".cloned()"),
        "plain str equality should be untouched; got:\n{out}"
    );
}
