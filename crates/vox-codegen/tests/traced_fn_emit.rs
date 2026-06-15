//! `@traced` codegen: assert that `emit_fn` prepends a `#[tracing::instrument]`
//! attribute on functions decorated with `@traced`.
//!
//! Fast: asserts on `emit_fn` output strings — no crate compile needed.

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;

fn emit_first_fn(src: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let f = hir.functions.first().expect("at least one function");
    emit_fn(f, Some(&hir.inferred_types), &[])
}

#[test]
fn traced_fn_emits_instrument_attr() {
    // Catches: @traced parsed + lowered but emit_fn not reading is_traced.
    let rust = emit_first_fn("@traced\nfn greet() to int { return 1 }");
    assert!(
        rust.contains("tracing::instrument") || rust.contains("info_span!"),
        "a @traced fn must emit a span; got:\n{rust}"
    );
    assert!(
        rust.contains("greet"),
        "span must reference the fn name; got:\n{rust}"
    );
}

#[test]
fn untraced_fn_has_no_instrument_attr() {
    // Regression guard: untraced fns must not gain the attribute.
    let rust = emit_first_fn("fn plain() to int { return 1 }");
    assert!(
        !rust.contains("tracing::instrument"),
        "an untraced fn must NOT emit a span; got:\n{rust}"
    );
}
