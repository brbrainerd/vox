//! Indexed `for v, i in list` must bind both value and index in the loop body.
//!
//! Without enumerate lowering, emitted Rust references `i` but never defines it
//! (`E0425 cannot find value i`). Fast string assertions — no crate compile.

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
fn indexed_for_loop_binds_index_in_body() {
    let out = emit_first_fn(
        "fn weighted_sum(values: List[int]) to int {
            let mut total = 0
            for v, i in values {
                total = total + v * (i + 1)
            }
            return total
        }",
    );

    assert!(
        out.contains(".enumerate()"),
        "`for v, i in list` must emit `.enumerate()`; got:\n{out}"
    );
    assert!(
        out.contains("for (i, v) in") || out.contains("for (i,v) in"),
        "enumerate loop must bind index before value; got:\n{out}"
    );
    assert!(
        out.contains("let i = i as i64"),
        "index binding must be shadowed as i64 for Vox int arithmetic; got:\n{out}"
    );
    assert!(
        out.contains("(i + 1)"),
        "loop body must reference the index binding; got:\n{out}"
    );
}

#[test]
fn indexed_for_loop_index_used_in_string_concat() {
    // Mirrors `index_labels` in examples/golden/tuple_destructure.vox.
    let out = emit_first_fn(
        "fn index_labels(items: List[str]) to List[str] {
            let mut result: List[str] = []
            for v, i in items {
                result.push(str(i) + \":\" + v)
            }
            return result
        }",
    );

    assert!(
        out.contains(".enumerate()") && out.contains("as_string(&i)"),
        "indexed loop body must reference index in expressions; got:\n{out}"
    );
}

#[test]
fn plain_for_loop_has_no_enumerate() {
    let out = emit_first_fn(
        "fn sum(values: List[int]) to int {
            let mut total = 0
            for v in values {
                total = total + v
            }
            return total
        }",
    );

    assert!(
        !out.contains(".enumerate()"),
        "`for v in list` must not emit enumerate; got:\n{out}"
    );
    assert!(
        out.contains("for v in"),
        "plain for-loop must bind value only; got:\n{out}"
    );
}
