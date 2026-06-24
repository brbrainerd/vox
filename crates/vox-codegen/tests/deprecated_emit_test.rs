use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::{hir::lower_module, lexer::lex, parser::parse_script};

fn emit_first_fn(src: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let hir = lower_module(&module);
    emit_fn(
        hir.functions.first().unwrap(),
        Some(&hir.inferred_types),
        &[],
    )
}

#[test]
fn deprecated_fn_emits_rust_attribute() {
    let rust = emit_first_fn("@deprecated(\"use v2\")\nfn old() -> i64 { 1 }");
    assert!(
        rust.contains("#[deprecated"),
        "expected #[deprecated], got:\n{rust}"
    );
    assert!(rust.contains("use v2"), "reason should be in the note");
}
