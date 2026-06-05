use vox_research_shim::research::claims::{Claim, extract_claims_with_model};

#[tokio::test]
async fn extract_claims_without_available_model_falls_back_to_empty() {
    let claims = extract_claims_with_model("test query", None, None, None, None).await;
    assert!(
        claims.is_empty(),
        "offline claim extraction should fail closed when no LLM candidate succeeds"
    );
}

#[test]
fn claim_default_fields_set() {
    let c = Claim {
        text: "X".into(),
        claim_id: 0,
        is_numeric: false,
        is_recent: false,
        is_named_event: false,
    };
    assert_eq!(c.text, "X");
}
