#![allow(missing_docs)]

use vox_codegen::codegen_rust::{emit::emit_api_client, generate as generate_rust};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_module;

const REFERENCE_SRC: &str = include_str!("fixtures/greaterfool_reference.vox");
const PIPELINE_SRC: &str = include_str!("fixtures/chatbot_pipeline.vox");

#[test]
fn greaterfool_reference_passes_pipeline() {
    let tokens = lex(REFERENCE_SRC);
    let module = parse(tokens).expect("reference example should parse");
    let diagnostics = typecheck_module(&module, "");
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "reference example should typecheck cleanly: {:?}",
        errors
    );
}

#[test]
fn greaterfool_reference_emits_secure_runtime_defaults() {
    let tokens = lex(REFERENCE_SRC);
    let module = parse(tokens).expect("reference example should parse");
    let hir = lower_module(&module);
    let output = generate_rust(
        &hir,
        "gf_parity_ref",
        vox_codegen::codegen_rust::RustAppShell::default(),
    )
    .expect("rust codegen should succeed");
    let main_rs = output
        .files
        .get("src/main.rs")
        .expect("main.rs should exist");
    let api_client = emit_api_client(&hir);

    insta::assert_snapshot!("greaterfool_ref_main_rs_emit", main_rs);
    insta::assert_snapshot!("greaterfool_ref_api_client_emit", api_client);
}

#[test]
fn compression_layer_pipeline_is_low_k_and_compiles() {
    let non_empty_lines = PIPELINE_SRC
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count();
    assert!(
        non_empty_lines <= 80,
        "pipeline should remain low-complexity; got {} non-comment lines",
        non_empty_lines
    );

    let tokens = lex(PIPELINE_SRC);
    let module = parse(tokens).expect("pipeline example should parse");
    let diagnostics = typecheck_module(&module, "");
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "pipeline example should typecheck cleanly: {:?}",
        errors
    );
}
