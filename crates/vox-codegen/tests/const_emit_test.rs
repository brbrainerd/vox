//! Verifies that `HirConst` nodes in `HirModule.consts` are emitted as Rust
//! `const` declarations by `emit_lib`.
//!
//! Vox syntax: top-level `let NAME = VALUE` is parsed as `Decl::Const` and
//! lowered into `HirModule.consts` (see vox-compiler/tests/const_lowering_test.rs).
//!
//! Pattern mirrors `binary_op_emit.rs`: parse → lower → emit_lib → assert on
//! output string (no compile step required).

use vox_codegen::codegen_rust::emit::emit_lib;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

fn emit_lib_for(src: &str) -> String {
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    emit_lib(&hir)
}

#[test]
fn const_int_emits_rust_const() {
    let out = emit_lib_for("let MAX = 3");
    assert!(
        out.contains("const MAX"),
        "expected `const MAX` in emitted lib, got:\n{out}"
    );
    assert!(
        out.contains("3"),
        "expected literal `3` in emitted const, got:\n{out}"
    );
}

#[test]
fn const_string_emits_static_str() {
    let out = emit_lib_for(r#"let BASE_URL = "https://example.com""#);
    assert!(
        out.contains("const BASE_URL"),
        "expected `const BASE_URL`, got:\n{out}"
    );
    assert!(
        out.contains("https://example.com"),
        "expected URL literal in emitted const, got:\n{out}"
    );
}
