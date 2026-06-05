use vox_research_shim::research::{
    claims::Claim, provider::ProviderRegistry, verifier::verify_claims_with_config,
};

#[tokio::test]
async fn verify_claims_without_evidence_returns_empty() {
    let claims = vec![Claim {
        text: "X".into(),
        claim_id: 0,
        is_numeric: false,
        is_recent: false,
        is_named_event: false,
    }];
    let registry = ProviderRegistry::default();
    let cfg = vox_research_shim::research::verifier::VerifierConfig::default();
    let verdicts = verify_claims_with_config(&claims, "q", &[], &registry, &cfg, None, None).await;
    assert!(
        verdicts.is_empty(),
        "verifier needs retrieved evidence before producing claim verdicts"
    );
}
