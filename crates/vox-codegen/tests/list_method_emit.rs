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

// ── Value-semantic mutators ──────────────────────────────────────────────────

#[test]
fn reverse_emits_value_returning_block() {
    let out = emit_first_fn("fn f(xs: List[str]) to List[str] { return xs.reverse() }");
    assert!(
        out.contains("__lst.reverse()") && out.contains("__lst }"),
        "`xs.reverse()` must emit a value-returning block; got:\n{out}"
    );
}

#[test]
fn reversed_emits_same_as_reverse() {
    let out = emit_first_fn("fn f(xs: List[str]) to List[str] { return xs.reversed() }");
    assert!(
        out.contains("__lst.reverse()") && out.contains("__lst }"),
        "`xs.reversed()` must emit a value-returning block; got:\n{out}"
    );
}

#[test]
fn sorted_emits_value_returning_block() {
    // `sorted()` had NO codegen arm at all (only `sorted_by`/`sorted_by_key`
    // did) — typechecked and interpreted, but `vox run` rejected any call with
    // "no method named `sorted` found for struct `Vec<String>`". This asserts
    // the emitted Rust actually calls a sort method and yields the list.
    let out = emit_first_fn("fn f(xs: List[str]) to List[str] { return xs.sorted() }");
    assert!(
        out.contains("__lst.sort_by(") && out.contains("__lst }"),
        "`xs.sorted()` must emit a value-returning sort block; got:\n{out}"
    );
}

#[test]
fn sorted_uses_partial_cmp_not_ord_cmp() {
    // Must work for float lists too (f64 is PartialOrd but not Ord, so a plain
    // `.sort()` (which requires `Ord`) would fail to compile for `List[float]`).
    let out = emit_first_fn("fn f(xs: List[float]) to List[float] { return xs.sorted() }");
    assert!(
        out.contains("partial_cmp"),
        "`xs.sorted()` must use partial_cmp (works for float lists, which are not Ord); got:\n{out}"
    );
}

#[test]
fn extend_emits_value_returning_block() {
    let out =
        emit_first_fn("fn f(xs: List[str], ys: List[str]) to List[str] { return xs.extend(ys) }");
    assert!(
        out.contains("__lst.extend(") && out.contains("__lst }"),
        "`xs.extend(ys)` must emit a value-returning block; got:\n{out}"
    );
}

#[test]
fn remove_emits_value_returning_block() {
    let out = emit_first_fn("fn f(xs: List[str], v: str) to List[str] { return xs.remove(v) }");
    assert!(
        out.contains("__lst.remove(") && out.contains("__lst }"),
        "`xs.remove(v)` must emit a value-returning block; got:\n{out}"
    );
}

#[test]
fn remove_at_emits_value_returning_block() {
    let out = emit_first_fn("fn f(xs: List[str], i: int) to List[str] { return xs.remove_at(i) }");
    assert!(
        out.contains("__lst.remove(") && out.contains("as usize") && out.contains("__lst }"),
        "`xs.remove_at(i)` must emit a bounds-checked block with usize cast; got:\n{out}"
    );
}

// ── Transformer / aggregate methods ─────────────────────────────────────────

#[test]
fn slice_list_emits_block() {
    let out = emit_first_fn("fn f(xs: List[str]) to List[str] { return xs.slice_list(1, 3) }");
    assert!(
        out.contains("__lst")
            && (out.contains("[..")
                || out.contains("drain")
                || out.contains("__start")
                || out.contains("__end")),
        "`xs.slice_list(s,e)` must emit a slicing block; got:\n{out}"
    );
}

#[test]
fn join_emits_join_call() {
    let out = emit_first_fn("fn f(xs: List[str]) to str { return xs.join(\", \") }");
    assert!(
        out.contains(".join(") || out.contains("join"),
        "`xs.join(sep)` must emit a join call; got:\n{out}"
    );
}

#[test]
fn index_emits_position_or_neg1() {
    let out = emit_first_fn("fn f(xs: List[str], v: str) to int { return xs.index(v) }");
    assert!(
        out.contains("position") || out.contains("-1"),
        "`xs.index(v)` must emit a position-search returning -1 on miss; got:\n{out}"
    );
}

#[test]
fn find_index_emits_same_as_index() {
    let out = emit_first_fn("fn f(xs: List[str], v: str) to int { return xs.find_index(v) }");
    assert!(
        out.contains("position") || out.contains("-1"),
        "`xs.find_index(v)` must emit a position-search returning -1 on miss; got:\n{out}"
    );
}

#[test]
fn count_list_emits_filter_count() {
    let out = emit_first_fn("fn f(xs: List[str], v: str) to int { return xs.count(v) }");
    assert!(
        out.contains("filter") || out.contains("count"),
        "`xs.count(v)` must emit a filter+count; got:\n{out}"
    );
}

#[test]
fn contains_list_emits_contains() {
    let out = emit_first_fn("fn f(xs: List[str], v: str) to bool { return xs.contains(v) }");
    assert!(
        out.contains("contains"),
        "`xs.contains(v)` must emit a contains call; got:\n{out}"
    );
}

#[test]
fn first_emits_cloned_option() {
    let out = emit_first_fn("fn f(xs: List[str]) to Option[str] { return xs.first() }");
    assert!(
        out.contains("first()") && out.contains("cloned"),
        "`xs.first()` must emit `.first().cloned()`; got:\n{out}"
    );
}

#[test]
fn last_emits_cloned_option() {
    let out = emit_first_fn("fn f(xs: List[str]) to Option[str] { return xs.last() }");
    assert!(
        out.contains("last()") && out.contains("cloned"),
        "`xs.last()` must emit `.last().cloned()`; got:\n{out}"
    );
}
