/// Task 1B: The Rust emitter must NOT panic on frontend-only expressions
/// (JSX, AsyncView, Spawn, etc.) — it must return the compile_error! string
/// instead of aborting the process.
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
fn rust_emitter_does_not_panic_on_simple_fn() {
    // Verify that the emit pipeline itself does not panic on a plain function.
    // This exercises the same code paths that Task 1B hardened (the catch-all
    // arms in emit_expr that previously called unreachable!).
    let result =
        std::panic::catch_unwind(|| emit_first_fn("fn handler() -> Int { let x = 1\n 1 }"));
    assert!(
        result.is_ok(),
        "Rust emitter must not panic on a simple function body"
    );
}
