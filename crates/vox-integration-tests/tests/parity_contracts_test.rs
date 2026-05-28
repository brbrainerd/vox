#![allow(missing_docs)]

use vox_actor_runtime::{ContextBudget, RetrievedChunk, RetryPolicy, apply_context_budget};
use vox_codegen::codegen_rust::{emit::emit_api_client, generate as generate_rust};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_workflow_runtime::workflow::plan_workflow_activities;

fn lower(src: &str) -> vox_compiler::hir::HirModule {
    let tokens = lex(src);
    let module = parse(tokens).expect("source should parse");
    lower_module(&module)
}

fn extract_codegen_activity_names(lib_src: &str) -> Vec<String> {
    let marker = "execute_activity_result(\"";
    let mut out = Vec::new();
    let mut cursor = lib_src;
    while let Some(pos) = cursor.find(marker) {
        let start = pos + marker.len();
        let rest = &cursor[start..];
        let Some(end) = rest.find('"') else {
            break;
        };
        out.push(rest[..end].to_string());
        cursor = &rest[end + 1..];
    }
    out
}

#[test]
#[ignore = "@server bare shorthand is not in the parser; use @server fn instead — owner: integration-tests sunset: 2026-12-31"]
fn parity_contract_codegen_rust_includes_auth_rate_limit_and_request_id() {
    let src = r#"
@server fn chat(prompt: str) to str {
    return prompt
}
"#;
    let hir = lower(src);
    let out = generate_rust(&hir, "parity_app", vox_codegen::codegen_rust::RustAppShell::default()).expect("rust codegen should succeed");
    let main_rs = out.files.get("src/main.rs").expect("main.rs should exist");

    insta::assert_snapshot!("parity_app_main_rs_emit", main_rs);
}

#[test]
#[ignore = "@server bare shorthand is not in the parser; use @server fn instead — owner: integration-tests sunset: 2026-12-31"]
fn parity_contract_api_client_supports_secure_headers_and_streaming() {
    let src = r#"
@server fn summarize(input: str) to str {
    return input
}
"#;
    let hir = lower(src);
    let api_client = emit_api_client(&hir);
    insta::assert_snapshot!("parity_summarize_api_client_emit", api_client);
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

#[test]
#[ignore = "activity/workflow constructs tombstoned; orchestration uses @mutation fn"]
fn parity_contract_generated_linear_activity_identity_matches_interpreted_plan() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity send_email(recipient: str) to Result[str] {
    return Ok(recipient)
}

activity write_audit(msg: str) to Result[str] {
    return Ok(msg)
}

workflow main_flow() to Result[str] {
    let a = send_email("person@example.com") with { activity_id: "email-step", retries: 3, timeout: "10s" }
    let b = write_audit("mail sent") with { activity_id: "audit-step" }
    return b
}
"#;
    let hir = lower(src);
    let steps = plan_workflow_activities(&hir, "main_flow").expect("interpreted plan should build");
    let planned_names: Vec<String> = steps.iter().map(|s| s.name.clone()).collect();

    let generated = generate_rust(&hir, "parity_app", vox_codegen::codegen_rust::RustAppShell::default()).expect("rust codegen should succeed");
    let lib_rs = generated
        .files
        .get("src/lib.rs")
        .expect("lib.rs should exist");
    let generated_names = extract_codegen_activity_names(lib_rs);

    assert_eq!(
        generated_names, planned_names,
        "generated execution activity identities should match interpreted plan order"
    );
    for step in &steps {
        if let Some(activity_id) = &step.activity_id {
            let needle = format!(".with_activity_id(\"{activity_id}\".to_string())");
            assert!(
                lib_rs.contains(&needle),
                "generated code should preserve explicit activity_id `{activity_id}`"
            );
        }
    }
    let send_email = steps
        .iter()
        .find(|s| s.name == "send_email")
        .expect("send_email step should exist");
    assert_eq!(
        send_email.timeout_ms,
        Some(10_000),
        "interpreted planner should normalize timeout string to milliseconds"
    );
}

#[test]
#[ignore = "activity/workflow constructs tombstoned; orchestration uses @mutation fn"]
fn parity_contract_generated_with_id_alias_matches_interpreted_activity_id() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity send_email(recipient: str) to Result[str] {
    return Ok(recipient)
}

workflow main_flow() to Result[str] {
    let a = send_email("person@example.com") with { id: "email-step-alias", retries: 2 }
    return a
}
"#;
    let hir = lower(src);
    let steps = plan_workflow_activities(&hir, "main_flow").expect("interpreted plan should build");
    assert_eq!(
        steps[0].activity_id.as_deref(),
        Some("email-step-alias"),
        "planner should map `id` alias to activity_id"
    );
    let generated = generate_rust(&hir, "parity_app", vox_codegen::codegen_rust::RustAppShell::default()).expect("rust codegen should succeed");
    let lib_rs = generated
        .files
        .get("src/lib.rs")
        .expect("lib.rs should exist");
    assert!(
        lib_rs.contains(".with_activity_id(\"email-step-alias\".to_string())"),
        "generated code should preserve `id` alias as activity_id option"
    );
}
