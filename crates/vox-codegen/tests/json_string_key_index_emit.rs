//! String-keyed JSON indexing must lower to object lookup, not usize array
//! indexing.
//!
//! `j.get(k)` / `j[k]` with `j: Json, k: str` previously emitted
//! `(j).get((k) as usize).cloned()` — E0308 (String vs usize), E0605
//! (non-primitive cast `String as usize`), and E0599 (`cloned` on the already
//! owned `Option<VoxJson>` that `VoxJson::get(String)` returns). The interpreter
//! (`eval/expr.rs` Index arm `(Object, Str)`) does a keyed lookup; codegen must
//! match. Numeric keys must keep the `usize` list path.
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
fn get_with_string_var_key_is_object_lookup() {
    let out = emit_first_fn("fn f(j: Json, k: str) to Option[Json] { return j.get(k) }");
    assert!(
        !out.contains("as usize"),
        "string-keyed .get must not cast the key to usize; got:\n{out}"
    );
    assert!(
        !out.contains(".cloned()"),
        "VoxJson::get(String) already returns an owned Option — no .cloned(); got:\n{out}"
    );
    assert!(out.contains(".get("), "expected a .get lookup; got:\n{out}");
}

#[test]
fn subscript_with_string_var_key_is_object_lookup() {
    let out = emit_first_fn("fn f(j: Json, k: str) to Option[Json] { return j[k] }");
    assert!(
        !out.contains("as usize"),
        "string-keyed subscript must not cast the key to usize; got:\n{out}"
    );
    assert!(
        !out.contains(".cloned()"),
        "string-keyed subscript lowers to owned VoxJson::get — no .cloned(); got:\n{out}"
    );
}

#[test]
fn subscript_with_string_literal_key_is_object_lookup() {
    let out = emit_first_fn("fn f(j: Json) to Option[Json] { return j[\"a\"] }");
    assert!(
        !out.contains("as usize"),
        "literal string subscript must not cast the key to usize; got:\n{out}"
    );
}

#[test]
fn subscript_with_concatenated_string_key_is_object_lookup() {
    // `k1 + k2` is a `HirExpr::Binary` typed `str`; it must take the object
    // path (interp: `Object[Str]` lookup returns Some(42) — verified live).
    let out =
        emit_first_fn("fn f(j: Json, k1: str, k2: str) to Option[Json] { return j[k1 + k2] }");
    assert!(
        !out.contains("as usize"),
        "concatenated string key must not cast to usize; got:\n{out}"
    );
    assert!(
        !out.contains(".cloned()"),
        "concatenated string key lowers to owned VoxJson::get — no .cloned(); got:\n{out}"
    );
}

#[test]
fn mixed_int_float_equality_promotes_int_side() {
    // Interp promotes Int↔Float in equality (`eval/value.rs`:
    // `(Int(a), Float(b)) => (*a as f64) == *b`); `1 is 1.0` prints `true`.
    // Without promotion the emitted `a == b` is `i64 == f64` — E0277.
    let out = emit_first_fn("fn f(a: int, b: float) to bool { return a is b }");
    assert!(
        out.contains("as f64"),
        "mixed int/float equality must cast the int side to f64; got:\n{out}"
    );
    let out2 = emit_first_fn("fn f(a: int, b: float) to bool { return a isnt b }");
    assert!(
        out2.contains("as f64"),
        "mixed int/float inequality must cast the int side to f64; got:\n{out2}"
    );
}

#[test]
fn int_indexing_keeps_usize_list_path() {
    let out = emit_first_fn("fn f(xs: List[str], i: int) to Option[str] { return xs[i] }");
    assert!(
        out.contains("as usize") && out.contains(".cloned()"),
        "integer indexing must stay Vec::get(usize).cloned(); got:\n{out}"
    );
}

#[test]
fn int_get_keeps_usize_list_path() {
    let out = emit_first_fn("fn f(xs: List[str], i: int) to Option[str] { return xs.get(i) }");
    assert!(
        out.contains("as usize") && out.contains(".cloned()"),
        "integer .get must stay Vec::get(usize).cloned(); got:\n{out}"
    );
}
