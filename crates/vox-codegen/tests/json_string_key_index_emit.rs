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
