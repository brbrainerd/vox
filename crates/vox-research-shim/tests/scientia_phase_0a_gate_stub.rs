use vox_research_shim::research::{
    claims::Claim,
    gate::{GateConfig, GateInput, score_with_config},
    types::RoutingTier,
};

#[test]
fn gate_with_no_hits_routes_direct() {
    let claims: Vec<Claim> = Vec::new();
    let input = GateInput {
        claims: &claims,
        citation_count: 0,
        no_retrieval_hits: true,
        answer_is_empty: true,
    };
    let cfg = GateConfig::default();
    let signal = score_with_config(&input, &cfg);
    let tier = signal.routing_tier_for(0.7, 0.4, 0.2);
    // Empty everything → low score → Direct (the cheapest fallback tier).
    assert!(matches!(tier, RoutingTier::Direct));
}
