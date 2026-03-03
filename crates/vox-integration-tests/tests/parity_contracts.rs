use vox_codegen_rust::{emit::emit_api_client, generate as generate_rust};
use vox_hir::lower_module;
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_runtime::{apply_context_budget, ContextBudget, RetrievedChunk, RetryPolicy};

fn lower(src: &str) -> vox_hir::HirModule {
    let tokens = lex(src);
    let module = parse(tokens).expect("source should parse");
    lower_module(&module)
}

#[test]
fn parity_contract_codegen_rust_includes_auth_rate_limit_and_request_id() {
    let src = r#"
@server fn chat(prompt: str) to str:
    ret prompt
"#;
    let hir = lower(src);
    let out = generate_rust(&hir, "parity_app").expect("rust codegen should succeed");
    let main_rs = out.files.get("src/main.rs").expect("main.rs should exist");

    assert!(
        main_rs.contains("AuthConfig::from_env"),
        "generated server should initialize auth config"
    );
    assert!(
        main_rs.contains("RateLimiter::from_env"),
        "generated server should initialize rate limiter"
    );
    assert!(
        main_rs.contains("x-request-id"),
        "generated server should thread request id"
    );
    assert!(
        main_rs.contains("authorize_request"),
        "generated server should enforce auth"
    );
}

#[test]
fn parity_contract_api_client_supports_secure_headers_and_streaming() {
    let src = r#"
@server fn summarize(input: str) to str:
    ret input
"#;
    let hir = lower(src);
    let api_client = emit_api_client(&hir);
    assert!(
        api_client.contains("buildHeaders"),
        "api client should centralize security headers"
    );
    assert!(
        api_client.contains("x-request-id"),
        "api client should include request id header"
    );
    assert!(
        api_client.contains("buildSse"),
        "api client should include stream helper"
    );
}

#[test]
fn parity_contract_context_budget_preserves_provenance() {
    let chunks = vec![
        RetrievedChunk {
            id: "c1".into(),
            source: "doc-a".into(),
            text: "0123456789".into(),
            score: 0.95,
        },
        RetrievedChunk {
            id: "c2".into(),
            source: "doc-b".into(),
            text: "abcdefghij".into(),
            score: 0.85,
        },
    ];
    let (selected, provenance) = apply_context_budget(
        chunks,
        ContextBudget {
            max_chunks: 2,
            max_chars: 12,
        },
    );
    assert_eq!(
        selected.len(),
        2,
        "should select both chunks under max_chunks"
    );
    assert_eq!(
        provenance.len(),
        2,
        "should produce provenance for each selected chunk"
    );
    assert!(
        provenance.iter().any(|p| p.truncated),
        "should mark truncated chunk when char budget is exceeded"
    );
}

#[test]
fn parity_contract_retry_policy_defaults_are_production_like() {
    let policy = RetryPolicy::default();
    assert!(
        policy.max_attempts >= 3,
        "retry policy should retry multiple times"
    );
    assert!(
        policy.base_delay_ms >= 100,
        "retry should include backoff delay"
    );
}
