use vox_codegen_rust::{emit::emit_api_client, generate as generate_rust};
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::diagnostics::Severity;
use vox_typeck::typecheck_module;

const REFERENCE_SRC: &str = include_str!("../../../examples/greaterfool_reference.vox");
const PIPELINE_SRC: &str = include_str!("../../../examples/chatbot_pipeline.vox");

#[test]
fn greaterfool_reference_passes_pipeline() {
    let tokens = lex(REFERENCE_SRC);
    let module = parse(tokens).expect("reference example should parse");
    let diagnostics = typecheck_module(&module, REFERENCE_SRC);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
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
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    let output = generate_rust(&hir, "gf_parity_ref").expect("rust codegen should succeed");
    let main_rs = output
        .files
        .get("src/main.rs")
        .expect("main.rs should exist");
    let api_client = emit_api_client(&hir);

    assert!(main_rs.contains("AuthConfig::from_env"));
    assert!(main_rs.contains("RateLimiter::from_env"));
    assert!(main_rs.contains("RequestContext::with_request_id"));
    assert!(main_rs.contains("authorize_request"));

    assert!(api_client.contains("buildHeaders"));
    assert!(api_client.contains("x-request-id"));
    assert!(api_client.contains("buildSse"));
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
    let diagnostics = typecheck_module(&module, PIPELINE_SRC);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "pipeline example should typecheck cleanly: {:?}",
        errors
    );
}
